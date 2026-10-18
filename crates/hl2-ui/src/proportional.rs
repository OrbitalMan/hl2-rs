//! Proportional UI scaling. Source scales its 480-line UI design by the screen height, which
//! fits every screen at least as wide as the narrowest retail aspect (5:4). On narrower screens
//! (square, portrait) the height would make the UI wider than the screen, so the scale follows
//! the [`design_height`]: the height of the widest-design area that fits the screen. On every
//! screen from 5:4 to ultrawide it equals the screen height, exactly as in retail.

use crate::gameui::menu::BASE_SCREEN_HEIGHT;
use glam::Vec2;

/// The height the UI scales by: the screen height, limited to the width divided by the
/// narrowest design aspect ([`crate::tuning::narrowest_design_aspect`]).
pub fn design_height(canvas: Vec2) -> f32 {
    canvas
        .y
        .min(canvas.x / crate::tuning::narrowest_design_aspect())
}

/// Pixels per 480-line design unit for sizes and horizontal offsets.
pub fn scale(canvas: Vec2) -> f32 {
    design_height(canvas) / BASE_SCREEN_HEIGHT
}

/// Pixels per 480-line design unit for vertical positions of HUD panels: the full screen height,
/// so panels keep their place on the screen (bottom panels stay at the bottom) while
/// [`scale`] limits their size.
pub fn y_scale(canvas: Vec2) -> f32 {
    canvas.y / BASE_SCREEN_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::vec2;

    #[test]
    fn landscape_screens_keep_the_height_and_narrow_ones_follow_the_width() {
        for size in [
            vec2(1024., 768.),
            vec2(1280., 1024.),
            vec2(1920., 1080.),
            vec2(5120., 1440.),
        ] {
            assert_eq!(design_height(size), size.y, "{size}");
        }
        assert_eq!(design_height(vec2(1080., 1080.)), 864.);
        assert_eq!(design_height(vec2(1080., 1920.)), 864.);
    }
}
