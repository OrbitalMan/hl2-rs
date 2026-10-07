//! Source HDR tone mapping (mat_hdr_level 2): every Source shader multiplies its output by
//! the tonemap scale (FinalOutput TONEMAP_SCALE_LINEAR), which Bevy carries as camera
//! exposure. Auto exposure samples a luminance histogram of the presented frame.
use crate::gameplay::Gameplay;
use bevy::{
    camera::Exposure,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use hl2_simulation::tonemap::{self, AutoExposure, BINS};
use std::sync::{Arc, Mutex};

/// Cameras whose Source materials take the tonemap scale.
#[derive(Component)]
pub struct ToneMapped;
/// Cameras that keep a fixed scale of 1 (LDR 2D sky faces, which already match native
/// until HDR sky textures are decoded).
#[derive(Component)]
pub struct UnitExposure;

/// Readback interval in frames; the SDK refreshes one of 17 histogram queries per frame.
const INTERVAL: u64 = 8;

#[derive(Resource)]
pub struct Tonemap {
    pub exposure: AutoExposure,
    pub forced: Option<f32>,
    histogram: Arc<Mutex<Option<[u32; BINS]>>>,
    pending: Arc<Mutex<bool>>,
    frames: u64,
    map: String,
    pub readbacks: u64,
}
impl Tonemap {
    pub fn new(forced: Option<f32>) -> Self {
        Self {
            exposure: AutoExposure::default(),
            forced,
            histogram: Default::default(),
            pending: Default::default(),
            frames: 0,
            map: String::new(),
            readbacks: 0,
        }
    }
    pub fn scale(&self) -> f32 {
        self.forced.unwrap_or(self.exposure.current)
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"scale": self.scale(), "forced": self.forced, "auto": self.exposure,
            "histogram": *self.histogram.lock().expect("tonemap histogram"), "readbacks": self.readbacks})
    }
}

/// Bevy view exposure = 2^-ev100 / 1.2.
pub fn ev100(scale: f32) -> f32 {
    -(1.2 * scale.max(1e-6)).log2()
}

pub fn update(
    mut commands: Commands,
    mut state: ResMut<Tonemap>,
    game: Res<Gameplay>,
    time: Res<Time<Real>>,
    mut cameras: Query<&mut Exposure, (With<ToneMapped>, Without<UnitExposure>)>,
    mut unit: Query<&mut Exposure, With<UnitExposure>>,
) {
    state.frames += 1;
    // ResetToneMapping(1.0) when a level starts.
    if state.map != game.world.name {
        state.map = game.world.name.clone();
        state.exposure = AutoExposure::default();
        *state.histogram.lock().expect("tonemap histogram") = None;
    }
    if state.forced.is_none() {
        let request = state.frames.is_multiple_of(INTERVAL) && {
            let mut pending = state.pending.lock().expect("tonemap pending");
            !std::mem::replace(&mut *pending, true)
        };
        if request {
            state.readbacks += 1;
            let (histogram, pending) = (state.histogram.clone(), state.pending.clone());
            commands.spawn(Screenshot::primary_window()).observe(
                move |event: On<ScreenshotCaptured>| {
                    if let Some(h) = histogram_of(&event.image) {
                        *histogram.lock().expect("tonemap histogram") = Some(h);
                    }
                    *pending.lock().expect("tonemap pending") = false;
                },
            );
        }
        let control = game.scene.tonemap;
        let latest = *state.histogram.lock().expect("tonemap histogram");
        if let Some(h) = latest {
            let current = state.exposure.current;
            state
                .exposure
                .set_target(tonemap::target_scale(&h, current), control);
        }
        state.exposure.advance(time.delta_secs(), control);
    }
    let ev = ev100(state.scale());
    for mut exposure in &mut cameras {
        if exposure.ev100 != ev {
            exposure.ev100 = ev;
        }
    }
    let one = ev100(1.);
    for mut exposure in &mut unit {
        if exposure.ev100 != one {
            exposure.ev100 = one;
        }
    }
}

/// SDK luminance histogram over the central 90% x 85% of the presented frame, sampling
/// every 4th pixel. Presented pixels are gamma encoded, as in the SDK's screen copy.
fn histogram_of(image: &Image) -> Option<[u32; BINS]> {
    use bevy::render::render_resource::TextureFormat;
    let swap = match image.texture_descriptor.format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => false,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => true,
        _ => return None,
    };
    let (width, height) = (image.width() as usize, image.height() as usize);
    let data = image.data.as_ref()?;
    if width == 0 || height == 0 || data.len() < width * height * 4 {
        return None;
    }
    let (x0, x1) = (
        (width as f32 * 0.05) as usize,
        (width as f32 * 0.95) as usize,
    );
    let (y0, y1) = (
        (height as f32 * 0.075) as usize,
        (height as f32 * 0.925) as usize,
    );
    let mut bins = [0u32; BINS];
    for y in (y0..y1).step_by(4) {
        for x in (x0..x1).step_by(4) {
            let p = &data[(y * width + x) * 4..][..4];
            let (r, b) = if swap { (p[2], p[0]) } else { (p[0], p[2]) };
            let rgb = [r, p[1], b].map(|c| c as f32 / 255.);
            if let Some(bin) = tonemap::bin_of(tonemap::luminance(rgb)) {
                bins[bin] += 1;
            }
        }
    }
    Some(bins)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ev100_round_trips_bevy_exposure() {
        for scale in [0.5f32, 1., 1.35, 2.] {
            let exposure = Exposure {
                ev100: ev100(scale),
            };
            assert!((exposure.exposure() - scale).abs() < 1e-5, "{scale}");
        }
    }
}
