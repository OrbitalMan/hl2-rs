//! Source HDR tone mapping scale: SDK viewpostprocess.cpp (new algorithm) chooses a goal
//! from a luminance histogram of the presented frame; retail materialsystem moves the
//! current scale toward it each frame. Hosts supply the histogram and frame time.

/// env_tonemap_controller state (SDK defaults mat_autoexposure_min/max 0.5/2,
/// mat_hdr_manual_tonemap_rate 1).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Control {
    pub min: f32,
    pub max: f32,
    pub rate: f32,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            min: 0.5,
            max: 2.,
            rate: 1.,
        }
    }
}

/// Luminance ranges: 16 bins over [0, 1] with (b / 16)^1.5 edges; the top bin also counts
/// everything brighter (SDK UpdateLuminanceRanges for mat_tonemap_algorithm 1).
pub const BINS: usize = 16;
pub fn bin_edges(bin: usize) -> (f32, f32) {
    let edge = |b: usize| (b as f32 / BINS as f32).powf(1.5);
    (edge(bin), edge(bin + 1))
}
/// Index of the bin holding a luminance, or None outside [0, inf).
pub fn bin_of(luminance: f32) -> Option<usize> {
    if luminance.is_nan() || luminance < 0. {
        return None;
    }
    Some(
        (0..BINS)
            .find(|&b| luminance < bin_edges(b).1)
            .unwrap_or(BINS - 1),
    )
}
/// SDK luminance of a presented (gamma-space) pixel.
pub fn luminance(rgb: [f32; 3]) -> f32 {
    rgb[0] * 0.2125 + rgb[1] * 0.7154 + rgb[2] * 0.0721
}

/// CLuminanceHistogramSystem::FindLocationOfPercentBrightPixels (new algorithm).
pub fn location_of_bright_pixels(
    histogram: &[u32; BINS],
    percent_bright: f32,
    snap_target: Option<f32>,
) -> Option<f32> {
    let total: u32 = histogram.iter().sum();
    if total == 0 {
        return None;
    }
    let mut range_tested = 0.;
    let mut pixels_tested = 0.;
    for bin in (0..BINS).rev() {
        let (min, max) = bin_edges(bin);
        let needed = percent_bright / 100. - pixels_tested;
        let share = histogram[bin] as f32 / total as f32;
        let range = max - min;
        if share >= needed {
            if let Some(target) = snap_target {
                if min <= target / 100. && max >= target / 100. {
                    return Some(target / 100.);
                }
            }
            let fraction = needed / share;
            let border = 1. - (range_tested + range * fraction);
            return Some(border.clamp(min, max));
        }
        pixels_tested += share;
        range_tested += range;
    }
    None
}

/// GetTargetTonemapScalar (new algorithm): place the brightest 2% at 60% luminance, and
/// keep the median at least 3%; relative to the scale the histogram was taken with.
pub fn target_scale(histogram: &[u32; BINS], last_scale: f32) -> f32 {
    const TARGET: f32 = 60.;
    let location = location_of_bright_pixels(histogram, 2., Some(TARGET))
        .unwrap_or(TARGET / 100.)
        .max(0.0001);
    let mut scalar = (TARGET / 100.) / location;
    if let Some(average) = location_of_bright_pixels(histogram, 50., None).filter(|&l| l > 0.) {
        scalar = scalar.max(0.03 / average);
    }
    (scalar * last_scale).max(0.001)
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AutoExposure {
    pub current: f32,
    pub goal: f32,
    history: Vec<f32>,
}
impl Default for AutoExposure {
    /// ResetToneMapping(1.0) at level init.
    fn default() -> Self {
        Self {
            current: 1.,
            goal: 1.,
            history: Vec::new(),
        }
    }
}
impl AutoExposure {
    /// SetToneMapScale: the clamped target becomes the goal; with 10 samples the goal is
    /// their V-weighted average (weights |i - 5| / 5), clamped again.
    pub fn set_target(&mut self, target: f32, control: Control) {
        if !target.is_finite() {
            return;
        }
        let target = target.clamp(control.min, control.max).max(0.001);
        self.goal = target;
        if self.history.len() == 10 {
            self.history.remove(0);
        }
        self.history.push(target);
        if self.history.len() == 10 {
            let (mut sum, mut weights) = (0., 0.);
            for (i, value) in self.history.iter().enumerate() {
                let weight = (i as f32 - 5.).abs() / 5.;
                sum += weight * value;
                weights += weight;
            }
            self.goal = (sum / weights).clamp(control.min, control.max);
        }
    }
    /// Retail materialsystem per-frame approach (algorithm 1): rate x 2, faster when
    /// darkening (mat_accelerate_adjust_exposure_down 3), step capped at 1/64.
    pub fn advance(&mut self, dt: f32, control: Control) {
        if dt <= 0. {
            return;
        }
        let mut rate = control.rate * 2.;
        if rate == 0. {
            self.current = self.goal;
            return;
        }
        if self.goal < self.current {
            let accelerated = 3. * rate;
            rate = ((self.current - self.goal) * (accelerated - rate) * (2. / 3.) + rate)
                .min(accelerated);
        }
        let mut step = rate * dt;
        if step >= 1. / 64. {
            step = 1. / 64.;
        }
        let step = step.clamp(0., 1.);
        let next = (1. - step) * self.current + step * self.goal;
        self.current = if next.is_finite() { next } else { self.goal };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bins_follow_sdk_edges() {
        assert_eq!(bin_edges(0), (0., (1f32 / 16.).powf(1.5)));
        assert_eq!(bin_of(0.), Some(0));
        assert_eq!(bin_of(0.99), Some(15));
        assert_eq!(bin_of(7.), Some(15));
        assert_eq!(bin_of(f32::NAN), None);
    }
    #[test]
    fn dark_frame_raises_and_bright_frame_lowers_the_scale() {
        // Everything in a low bin: brighten by roughly 0.6 / location.
        let mut dark = [0; BINS];
        dark[3] = 1000;
        let up = target_scale(&dark, 1.);
        assert!(up > 2., "{up}");
        // Brightest 2% in the top bin: darken.
        let mut bright = [0; BINS];
        bright[8] = 900;
        bright[15] = 100;
        let down = target_scale(&bright, 1.);
        assert!(down < 1., "{down}");
        // Bright pixels already in the 60% bin: sticky, unchanged.
        let mut steady = [0; BINS];
        steady[5] = 980;
        let sixty = bin_of(0.6).unwrap();
        steady[sixty] = 20;
        assert!((target_scale(&steady, 1.5) - 1.5).abs() < 1e-6);
    }
    #[test]
    fn goal_is_clamped_and_adaptation_follows_retail_rate() {
        let control = Control {
            min: 0.5,
            max: 2.,
            rate: 0.35,
        };
        let mut exposure = AutoExposure::default();
        exposure.set_target(10., control);
        assert_eq!(exposure.goal, 2.);
        exposure.advance(0.015, control);
        // step = 0.35 * 2 * 0.015 = 0.0105
        assert!((exposure.current - (1. + 0.0105)).abs() < 1e-6);
        // The step is capped at 1/64 for long frames.
        let mut slow = AutoExposure::default();
        slow.set_target(2., control);
        slow.advance(1., control);
        assert!((slow.current - (1. + 1. / 64.)).abs() < 1e-6);
        // Darkening is accelerated.
        let mut dark = AutoExposure {
            current: 2.,
            ..Default::default()
        };
        dark.set_target(0.5, control);
        dark.advance(0.015, control);
        assert!(2. - dark.current > 0.0105 * 1.5);
    }
}
