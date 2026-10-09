//! Viewport geometry, aspect ratio calculations, and UI canvas hit-test coordinate translation.

use crate::canvas::Canvas;
use glam::{UVec2, Vec2};

/// Describes the viewport placement within a window/display frame for aspect-ratio preservation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportBounds {
    /// Target rendering resolution (e.g. 1024x768).
    pub target_resolution: UVec2,
    /// Physical pixel offset within the host window frame (x = left stripe, y = top stripe).
    pub physical_offset: UVec2,
    /// Physical pixel dimensions of the active viewport area.
    pub physical_size: UVec2,
    /// Logical point offset within the host window frame.
    pub logical_offset: Vec2,
    /// Logical point dimensions of the active viewport area.
    pub logical_size: Vec2,
}

impl Default for ViewportBounds {
    fn default() -> Self {
        Self {
            target_resolution: UVec2::new(1280, 720),
            physical_offset: UVec2::ZERO,
            physical_size: UVec2::new(1280, 720),
            logical_offset: Vec2::ZERO,
            logical_size: Vec2::new(1280.0, 720.0),
        }
    }
}

impl ViewportBounds {
    /// Computes viewport bounds fitting `target_resolution` into a window of `physical_window_size`
    /// with display `scale_factor`, adding letterbox or pillarbox bars when aspect ratios differ.
    pub fn compute(
        target_resolution: UVec2,
        physical_window_size: UVec2,
        scale_factor: f32,
    ) -> Self {
        let win_w = physical_window_size.x.max(1);
        let win_h = physical_window_size.y.max(1);
        let scale = scale_factor.max(1.0);

        let target_w = target_resolution.x.max(1);
        let target_h = target_resolution.y.max(1);

        let target_aspect = target_w as f32 / target_h as f32;
        let win_aspect = win_w as f32 / win_h as f32;

        let (vp_w, vp_h, offset_x, offset_y) = if (win_aspect - target_aspect).abs() < 1e-4 {
            (win_w, win_h, 0, 0)
        } else if win_aspect > target_aspect {
            // Window is wider than target aspect ratio:
            // Pillarbox (vertical black stripes on left & right sides)
            let vp_h = win_h;
            let vp_w = ((win_h as f32 * target_aspect).round() as u32).min(win_w);
            let offset_x = (win_w - vp_w) / 2;
            (vp_w, vp_h, offset_x, 0)
        } else {
            // Window is taller than target aspect ratio:
            // Letterbox (horizontal black stripes on top & bottom sides)
            let vp_w = win_w;
            let vp_h = ((win_w as f32 / target_aspect).round() as u32).min(win_h);
            let offset_y = (win_h - vp_h) / 2;
            (vp_w, vp_h, 0, offset_y)
        };

        let physical_offset = UVec2::new(offset_x, offset_y);
        let physical_size = UVec2::new(vp_w.max(1), vp_h.max(1));
        let logical_offset = Vec2::new(offset_x as f32 / scale, offset_y as f32 / scale);
        let logical_size = Vec2::new(vp_w as f32 / scale, vp_h as f32 / scale);

        Self {
            target_resolution: UVec2::new(target_w, target_h),
            physical_offset,
            physical_size,
            logical_offset,
            logical_size,
        }
    }

    /// Whether black stripes are present (aspect ratio mismatch).
    pub fn has_black_stripes(&self) -> bool {
        self.physical_offset != UVec2::ZERO
    }

    /// True if vertical black stripes (pillarboxing) on left/right sides.
    pub fn is_vertical_stripes(&self) -> bool {
        self.physical_offset.x > 0
    }

    /// True if horizontal black stripes (letterboxing) on top/bottom sides.
    pub fn is_horizontal_stripes(&self) -> bool {
        self.physical_offset.y > 0
    }

    /// Maps a window cursor position in logical window coordinates to UI canvas coordinates.
    ///
    /// Returns `None` if the cursor position falls outside the active viewport region
    /// (e.g. over letterbox/pillarbox black stripes).
    pub fn cursor_to_canvas(&self, cursor_pos: Vec2) -> Option<Vec2> {
        let rel_x = cursor_pos.x - self.logical_offset.x;
        let rel_y = cursor_pos.y - self.logical_offset.y;

        if rel_x >= 0.0
            && rel_x <= self.logical_size.x
            && rel_y >= 0.0
            && rel_y <= self.logical_size.y
        {
            Some(Vec2::new(rel_x, rel_y))
        } else {
            None
        }
    }

    /// Resizes the given canvas to match this viewport's logical size.
    pub fn resize_canvas(&self, canvas: &Canvas) {
        canvas.resize(self.logical_size.x, self.logical_size.y);
    }
}

/// Maps a window cursor position in logical window coordinates to UI canvas coordinates,
/// properly offsetting for letterbox/pillarbox black stripes and filtering out positions
/// falling on black stripes outside the active viewport.
pub fn cursor_to_canvas(
    cursor_pos: Option<Vec2>,
    viewport: Option<&ViewportBounds>,
    window_logical_size: Vec2,
) -> Option<Vec2> {
    let pos = cursor_pos?;
    if let Some(vp) = viewport {
        vp.cursor_to_canvas(pos)
    } else if pos.x >= 0.0
        && pos.x <= window_logical_size.x
        && pos.y >= 0.0
        && pos.y <= window_logical_size.y
    {
        Some(pos)
    } else {
        None
    }
}

/// Resizes a canvas to match the active viewport, or the window logical size if no viewport is active.
pub fn resize_canvas(
    canvas: &Canvas,
    viewport: Option<&ViewportBounds>,
    window_logical_size: Vec2,
) {
    if let Some(vp) = viewport {
        vp.resize_canvas(canvas);
    } else {
        canvas.resize(window_logical_size.x, window_logical_size.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pillarbox_vertical_stripes_for_narrow_aspect_on_wide_window() {
        // Target: 4:3 (1024x768). Window: 16:9 (1920x1080). Scale: 1.0.
        let vp = ViewportBounds::compute(UVec2::new(1024, 768), UVec2::new(1920, 1080), 1.0);
        assert_eq!(vp.physical_size, UVec2::new(1440, 1080));
        assert_eq!(vp.physical_offset, UVec2::new(240, 0));
        assert_eq!(vp.logical_size, Vec2::new(1440.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::new(240.0, 0.0));
        assert!(vp.has_black_stripes());
        assert!(vp.is_vertical_stripes());
        assert!(!vp.is_horizontal_stripes());
    }

    #[test]
    fn letterbox_horizontal_stripes_for_wide_aspect_on_narrow_window() {
        // Target: 16:9 (1280x720). Window: 4:3 (1024x768). Scale: 1.0.
        let vp = ViewportBounds::compute(UVec2::new(1280, 720), UVec2::new(1024, 768), 1.0);
        assert_eq!(vp.physical_size.x, 1024);
        assert_eq!(vp.physical_size.y, 576);
        assert_eq!(vp.physical_offset, UVec2::new(0, 96));
        assert_eq!(vp.logical_size, Vec2::new(1024.0, 576.0));
        assert_eq!(vp.logical_offset, Vec2::new(0.0, 96.0));
        assert!(vp.has_black_stripes());
        assert!(!vp.is_vertical_stripes());
        assert!(vp.is_horizontal_stripes());
    }

    #[test]
    fn exact_aspect_ratio_fills_entire_frame() {
        // Target: 16:9 (1920x1080). Window: 16:9 (1920x1080). Scale: 1.0.
        let vp = ViewportBounds::compute(UVec2::new(1920, 1080), UVec2::new(1920, 1080), 1.0);
        assert_eq!(vp.physical_size, UVec2::new(1920, 1080));
        assert_eq!(vp.physical_offset, UVec2::ZERO);
        assert_eq!(vp.logical_offset, Vec2::ZERO);
        assert!(!vp.has_black_stripes());
    }

    #[test]
    fn viewport_with_scale_factor_calculates_correct_logical_offset() {
        // Target: 4:3 (1024x768). Window: 16:9 Retina (3840x2160). Scale: 2.0.
        let vp = ViewportBounds::compute(UVec2::new(1024, 768), UVec2::new(3840, 2160), 2.0);
        assert_eq!(vp.physical_size, UVec2::new(2880, 2160));
        assert_eq!(vp.physical_offset, UVec2::new(480, 0));
        assert_eq!(vp.logical_size, Vec2::new(1440.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::new(240.0, 0.0));
    }

    #[test]
    fn cursor_to_canvas_maps_inside_and_rejects_outside_pillarbox() {
        let vp = ViewportBounds::compute(UVec2::new(1024, 768), UVec2::new(1920, 1080), 1.0);

        // Click at left edge of 4:3 area on screen (x=240, y=100) -> canvas (0, 100)
        let p = cursor_to_canvas(
            Some(Vec2::new(240.0, 100.0)),
            Some(&vp),
            Vec2::new(1920.0, 1080.0),
        );
        assert_eq!(p, Some(Vec2::new(0.0, 100.0)));

        // Click in the center of 4:3 area on screen (x=960, y=540) -> canvas (720, 540)
        let p_center = cursor_to_canvas(
            Some(Vec2::new(960.0, 540.0)),
            Some(&vp),
            Vec2::new(1920.0, 1080.0),
        );
        assert_eq!(p_center, Some(Vec2::new(720.0, 540.0)));

        // Click in black stripe on the left (x=100 < 240) -> None
        let p_left_stripe = cursor_to_canvas(
            Some(Vec2::new(100.0, 500.0)),
            Some(&vp),
            Vec2::new(1920.0, 1080.0),
        );
        assert_eq!(p_left_stripe, None);

        // Click in black stripe on the right (x=1700 > 1680) -> None
        let p_right_stripe = cursor_to_canvas(
            Some(Vec2::new(1700.0, 500.0)),
            Some(&vp),
            Vec2::new(1920.0, 1080.0),
        );
        assert_eq!(p_right_stripe, None);
    }

    #[test]
    fn cursor_to_canvas_maps_inside_and_rejects_outside_letterbox() {
        let vp = ViewportBounds::compute(UVec2::new(1280, 720), UVec2::new(1024, 1024), 1.0);
        // Top letterbox stripe is (1024 - 576)/2 = 224

        // Top stripe (y=100 < 224) -> None
        let p_top = cursor_to_canvas(
            Some(Vec2::new(512.0, 100.0)),
            Some(&vp),
            Vec2::new(1024.0, 1024.0),
        );
        assert_eq!(p_top, None);

        // Bottom stripe (y=850 > 800) -> None
        let p_bottom = cursor_to_canvas(
            Some(Vec2::new(512.0, 850.0)),
            Some(&vp),
            Vec2::new(1024.0, 1024.0),
        );
        assert_eq!(p_bottom, None);

        // Inside active area (x=512, y=324) -> canvas (512, 100)
        let p_inside = cursor_to_canvas(
            Some(Vec2::new(512.0, 324.0)),
            Some(&vp),
            Vec2::new(1024.0, 1024.0),
        );
        assert_eq!(p_inside, Some(Vec2::new(512.0, 100.0)));
    }

    #[test]
    fn resize_canvas_resizes_to_viewport_size() {
        let canvas = Canvas::default();
        let vp = ViewportBounds::compute(UVec2::new(1024, 768), UVec2::new(1920, 1080), 1.0);
        resize_canvas(&canvas, Some(&vp), Vec2::new(1920.0, 1080.0));
        assert_eq!(canvas.width(), 1440.0);
        assert_eq!(canvas.height(), 1080.0);

        // Without viewport, resizes to window size
        resize_canvas(&canvas, None, Vec2::new(800.0, 600.0));
        assert_eq!(canvas.width(), 800.0);
        assert_eq!(canvas.height(), 600.0);
    }
}
