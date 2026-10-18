//! UI tuning values embedded from `assets/ui/tuning.json` and parsed once: numbers that shape
//! the UI but come from no game resource.

use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Deserialize)]
struct Tuning {
    /// Width:height of the narrowest screen the 480-line UI design fits ([`crate::proportional`]).
    narrowest_design_aspect: (u32, u32),
    /// Width:height of the widest screen the menu keeps its retail size on; wider screens grow
    /// the menu ([`crate::proportional::widescreen_menu_scale`]).
    widest_design_aspect: (u32, u32),
    /// Largest growth of the menu on wide screens.
    widescreen_menu_scale_max: f32,
    /// Rows of the tallest menu: the stack whose bottom stays put when the menu grows.
    menu_stack_rows: u32,
    /// Seconds before an unconfirmed video change reverts (`gameui::video_confirm`).
    video_confirm_seconds: f64,
    /// Smallest and largest explicit vertical field of view of the Video > Advanced slider
    /// (below the smallest the slider means automatic).
    vertical_fov_range: (f32, f32),
    /// Smallest and largest viewmodel field of view of the Video > Advanced slider.
    viewmodel_fov_range: (f32, f32),
}

fn data() -> &'static Tuning {
    static DATA: LazyLock<Tuning> = LazyLock::new(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/ui/tuning.json"
        )))
        .expect("embedded UI tuning data is valid")
    });
    &DATA
}

/// Width / height of the narrowest screen the UI design fits.
pub fn narrowest_design_aspect() -> f32 {
    let (width, height) = data().narrowest_design_aspect;
    width as f32 / height as f32
}

/// Width / height of the widest screen the menu keeps its retail size on.
pub fn widest_design_aspect() -> f32 {
    let (width, height) = data().widest_design_aspect;
    width as f32 / height as f32
}

/// Largest growth of the menu on wide screens.
pub fn widescreen_menu_scale_max() -> f32 {
    data().widescreen_menu_scale_max
}

/// Rows of the tallest menu.
pub fn menu_stack_rows() -> f32 {
    data().menu_stack_rows as f32
}

/// Seconds before an unconfirmed video change reverts.
pub fn video_confirm_seconds() -> f64 {
    data().video_confirm_seconds
}

/// Smallest and largest explicit vertical field of view (degrees).
pub fn vertical_fov_range() -> (f32, f32) {
    data().vertical_fov_range
}

/// Smallest and largest viewmodel field of view (degrees).
pub fn viewmodel_fov_range() -> (f32, f32) {
    data().viewmodel_fov_range
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_data_parses_with_usable_values() {
        assert!(narrowest_design_aspect() >= 1.0, "a landscape design");
        assert!(video_confirm_seconds() >= 5.0, "time to read and answer");
        assert!(widest_design_aspect() > narrowest_design_aspect());
        assert!(widescreen_menu_scale_max() >= 1.0 && menu_stack_rows() >= 1.0);
        for (min, max) in [vertical_fov_range(), viewmodel_fov_range()] {
            assert!(0. < min && min < max && max < 180., "{min} {max}");
        }
    }
}
