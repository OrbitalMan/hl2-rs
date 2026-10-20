//! Software cursor rendering for fullscreen mode (`cl_software_cursor`).
//!
//! On high-DPI displays in exclusive fullscreen mode, the operating system hardware cursor
//! can render at 1x point size (appearing microscopic). When `cl_software_cursor` is enabled,
//! the OS cursor is hidden in fullscreen and drawn as a crisp, integer-scaled 2D quad directly
//! in the UI presentation pass.

use glam::Vec2;
use hl2_ui::canvas::{Canvas, FilterMode, Texture2D, WHITE, draw_texture};
use hl2_ui::gameui::placement::CursorHint;
use std::collections::HashMap;

const ARROW_1X: &[&str] = &[
    "#...............",
    "##..............",
    "#O#.............",
    "#OO#............",
    "#OOO#...........",
    "#OOOO#..........",
    "#OOOOO#.........",
    "#OOOOOO#........",
    "#OOOOOOO#.......",
    "#OOOOOOOO#......",
    "#OOOOO#####.....",
    "#OO#OO#.........",
    "#O#.#OO#........",
    "##..#OO#........",
    "#....#OO#.......",
    ".....#OO#.......",
    "......##........",
    "................",
    "................",
    "................",
];

const NWSE_1X: &[&str] = &[
    "######..........",
    "#OOOO#..........",
    "#OOOO#..........",
    "#OO##...........",
    "#O#.#O#.........",
    "##...#O#........",
    "......#O#.......",
    ".......#O#......",
    "........#O#...##",
    ".........#O#.#O#",
    "...........##OO#",
    "..........#OOOO#",
    "..........#OOOO#",
    "..........######",
    "................",
    "................",
];

const NESW_1X: &[&str] = &[
    "..........######",
    "..........#OOOO#",
    "..........#OOOO#",
    "...........##OO#",
    ".........#O#.#O#",
    "........#O#...##",
    ".......#O#......",
    "......#O#.......",
    "##...#O#........",
    "#O#.#O#.........",
    "#OO##...........",
    "#OOOO#..........",
    "#OOOO#..........",
    "######..........",
    "................",
    "................",
];

#[cfg(target_os = "macos")]
const BORDER_COLOR: [u8; 4] = [255, 255, 255, 255]; // white border on Mac
#[cfg(target_os = "macos")]
const FILL_COLOR: [u8; 4] = [0, 0, 0, 255]; // black fill on Mac

#[cfg(not(target_os = "macos"))]
const BORDER_COLOR: [u8; 4] = [0, 0, 0, 255]; // black border on Windows
#[cfg(not(target_os = "macos"))]
const FILL_COLOR: [u8; 4] = [255, 255, 255, 255]; // white fill on Windows

fn rasterize_cursor(hint: CursorHint, scale: u32) -> Texture2D {
    let pattern = match hint {
        CursorHint::Arrow => ARROW_1X,
        CursorHint::ResizeNwSe => NWSE_1X,
        CursorHint::ResizeNeSw => NESW_1X,
    };
    let scale = scale.clamp(1, 8);
    let base_h = pattern.len() as u32;
    let base_w = pattern.first().map_or(0, |row| row.len()) as u32;
    let width = base_w * scale;
    let height = base_h * scale;

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for (row_idx, row) in pattern.iter().enumerate() {
        for (col_idx, ch) in row.chars().enumerate() {
            let color: [u8; 4] = match ch {
                '#' => BORDER_COLOR,
                'O' => FILL_COLOR,
                _ => continue,
            };
            let start_y = row_idx as u32 * scale;
            let start_x = col_idx as u32 * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    let y = start_y + dy;
                    let x = start_x + dx;
                    let offset = ((y * width + x) * 4) as usize;
                    rgba[offset..offset + 4].copy_from_slice(&color);
                }
            }
        }
    }

    let texture = Texture2D::from_rgba8(width as u16, height as u16, &rgba);
    texture.set_filter(FilterMode::Nearest);
    texture
}

/// Cache of rasterized cursor textures by `(hint_id, scale)`.
#[derive(Default)]
pub struct SoftwareCursorCache {
    textures: HashMap<(u8, u32), Texture2D>,
    last_canvas_pointer: Option<Vec2>,
    was_at_top: bool,
    rehide_countdown: u32,
}

impl SoftwareCursorCache {
    fn hint_id(hint: CursorHint) -> u8 {
        match hint {
            CursorHint::Arrow => 0,
            CursorHint::ResizeNwSe => 1,
            CursorHint::ResizeNeSw => 2,
        }
    }

    /// Retrieves or lazily rasterizes the cursor texture for the given hint and scale.
    pub fn get(&mut self, hint: CursorHint, scale: u32) -> &Texture2D {
        let id = Self::hint_id(hint);
        let scale = scale.clamp(1, 8);
        self.textures
            .entry((id, scale))
            .or_insert_with(|| rasterize_cursor(hint, scale))
    }

    /// Resolves the canvas pointer position for software cursor rendering, clamping to the top
    /// edge when the mouse is at or near the top of the screen and preserving the cursor position
    /// while the window is focused even if the OS temporarily moves the pointer into the menu bar hot-zone.
    ///
    /// Returns `(pointer, edge_recovered)`:
    /// - `pointer`: Canvas coordinates to draw the software cursor, or `None` if outside.
    /// - `edge_recovered`: `true` if the cursor is recovering from the top edge back into the window interior.
    pub fn resolve_pointer(
        &mut self,
        cursor_pos: Option<bevy::math::Vec2>,
        viewport: Option<&hl2_ui::viewport::ViewportBounds>,
        window: &bevy::window::Window,
    ) -> (Option<glam::Vec2>, bool) {
        let (offset_x, offset_y, size_x, size_y, target_res) = if let Some(vp) = viewport {
            (
                vp.logical_offset.x,
                vp.logical_offset.y,
                vp.logical_size.x,
                vp.logical_size.y,
                vp.target_resolution.as_vec2(),
            )
        } else {
            (
                0.0,
                0.0,
                window.width(),
                window.height(),
                glam::Vec2::new(window.width(), window.height()),
            )
        };

        if let Some(pos) = cursor_pos {
            let rel_x = pos.x - offset_x;
            let rel_y = pos.y - offset_y;
            // The macOS autohide menu bar region in fullscreen spans ~24-32 pixels.
            let is_near_top = rel_y <= 32.0;
            if rel_x >= 0.0 && rel_x <= size_x {
                let clamped_rel_y = if rel_y < 0.0 { 0.0 } else { rel_y };
                if clamped_rel_y <= size_y {
                    let size = glam::Vec2::new(size_x, size_y).max(glam::Vec2::ONE);
                    let pointer = glam::Vec2::new(rel_x, clamped_rel_y) * target_res / size;
                    if self.was_at_top && !is_near_top {
                        // Assert re-hiding over multiple frames to eliminate any Cocoa cursor rect latency.
                        self.rehide_countdown = 15;
                    }
                    self.was_at_top = is_near_top;
                    self.last_canvas_pointer = Some(pointer);
                    let edge_recovered = if self.rehide_countdown > 0 {
                        self.rehide_countdown -= 1;
                        true
                    } else {
                        false
                    };
                    return (Some(pointer), edge_recovered);
                }
            }
            if self.was_at_top {
                self.rehide_countdown = 15;
            }
            self.was_at_top = false;
            let edge_recovered = if self.rehide_countdown > 0 {
                self.rehide_countdown -= 1;
                true
            } else {
                false
            };
            (None, edge_recovered)
        } else if window.focused && self.was_at_top {
            // Cursor position is None (e.g. pointer in macOS autohide menu bar zone),
            // but window still has focus and was last at the top: keep drawing at top edge.
            (
                self.last_canvas_pointer.map(|p| glam::Vec2::new(p.x, 0.0)),
                false,
            )
        } else {
            self.was_at_top = false;
            (None, false)
        }
    }

    /// Draws the software cursor onto the given canvas at the specified pointer position.
    pub fn draw(&mut self, canvas: &Canvas, hint: CursorHint, pointer: Vec2, scale: u32) {
        let scale = scale.clamp(1, 8);
        let texture = self.get(hint, scale).clone();
        let offset = match hint {
            CursorHint::Arrow => Vec2::ZERO,
            CursorHint::ResizeNwSe | CursorHint::ResizeNeSw => Vec2::splat(8.0 * scale as f32),
        };
        let pos = pointer - offset;
        draw_texture(canvas, &texture, pos.x, pos.y, WHITE);
    }
}

/// Resolves the integer scale factor for software cursor rendering.
///
/// Renders the software cursor at 2x the standard 1x hardware cursor size (32x40 pixels)
/// across all platforms.
pub const fn resolve_cursor_scale() -> u32 {
    2
}

/// On macOS, adjusts cursor position when transitioning between fullscreen (1x physical pixels)
/// and windowed/borderless (2x Retina points).
#[allow(dead_code)]
pub fn adjust_cursor_position_for_mode_switch(
    pos: bevy::math::Vec2,
    entering_fullscreen: bool,
    max_bounds: Option<bevy::math::Vec2>,
) -> bevy::math::Vec2 {
    let factor = if entering_fullscreen { 2.0 } else { 0.5 };
    let mut adjusted = pos * factor;
    if let Some(bounds) = max_bounds {
        adjusted = adjusted.clamp(bevy::math::Vec2::ZERO, bounds);
    } else {
        adjusted = adjusted.max(bevy::math::Vec2::ZERO);
    }
    adjusted
}

/// On macOS fullscreen mode, calculates the 2x accelerated cursor position given the current and previous positions.
#[allow(dead_code)]
pub fn accelerate_cursor_position_2x(
    curr_pos: bevy::math::Vec2,
    prev_pos: bevy::math::Vec2,
    max_bounds: bevy::math::Vec2,
) -> bevy::math::Vec2 {
    let delta = curr_pos - prev_pos;
    (curr_pos + delta).clamp(bevy::math::Vec2::ZERO, max_bounds)
}

#[cfg(target_os = "macos")]
static HARDWARE_CURSOR_HIDDEN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Hides the native OS hardware cursor on macOS via AppKit.
#[cfg(target_os = "macos")]
pub fn hide_hardware_cursor() {
    let was_hidden = HARDWARE_CURSOR_HIDDEN.swap(true, std::sync::atomic::Ordering::SeqCst);
    if !was_hidden {
        unsafe {
            let () = objc2::msg_send![objc2::class!(NSCursor), hide];
        }
    }
}

/// Restores the native OS hardware cursor on macOS via AppKit.
#[cfg(target_os = "macos")]
pub fn unhide_hardware_cursor() {
    let was_hidden = HARDWARE_CURSOR_HIDDEN.swap(false, std::sync::atomic::Ordering::SeqCst);
    if was_hidden {
        unsafe {
            let () = objc2::msg_send![objc2::class!(NSCursor), unhide];
        }
    }
}

/// On macOS, forces Winit and AppKit to re-invalidate cursor rects and restore the invisible
/// cursor when returning from the native menu bar hot-zone.
#[cfg(target_os = "macos")]
pub fn ensure_macos_hardware_cursor_hidden(window_entity: bevy::ecs::entity::Entity) {
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        if let Some(winit) = windows.get_window(window_entity) {
            winit.set_cursor_visible(true);
            winit.set_cursor_visible(false);
        }
    });
    hide_hardware_cursor();
}

/// On macOS, restores native cursor visibility in both Winit and AppKit.
#[cfg(target_os = "macos")]
pub fn restore_macos_hardware_cursor(window_entity: bevy::ecs::entity::Entity) {
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        if let Some(winit) = windows.get_window(window_entity) {
            winit.set_cursor_visible(true);
        }
    });
    unhide_hardware_cursor();
}

/// Determines whether the software cursor should be active:
/// when in exclusive fullscreen mode, `cl_software_cursor` is enabled, the window is focused, and the UI is active.
pub fn is_active(
    software_cursor_config: bool,
    window_focused: bool,
    ui_active: bool,
    is_fullscreen: bool,
) -> bool {
    is_fullscreen && software_cursor_config && window_focused && ui_active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_active_conditions() {
        // Active when config=true, focused, ui_active, and is_fullscreen
        assert!(is_active(true, true, true, true));

        // Inactive when not in fullscreen (e.g. windowed or borderless)
        assert!(!is_active(true, true, true, false));

        // Inactive if config is false
        assert!(!is_active(false, true, true, true));

        // Inactive when window lost focus
        assert!(!is_active(true, false, true, true));

        // Inactive when UI is not active (normal gameplay)
        assert!(!is_active(true, true, false, true));
    }

    #[test]
    fn test_rasterize_dimensions_and_scaling() {
        let arrow_1x = rasterize_cursor(CursorHint::Arrow, 1);
        assert_eq!(arrow_1x.width(), 16.0);
        assert_eq!(arrow_1x.height(), 20.0);
        assert_eq!(arrow_1x.filter(), FilterMode::Nearest);

        let arrow_2x = rasterize_cursor(CursorHint::Arrow, 2);
        assert_eq!(arrow_2x.width(), 32.0);
        assert_eq!(arrow_2x.height(), 40.0);

        let nwse_3x = rasterize_cursor(CursorHint::ResizeNwSe, 3);
        assert_eq!(nwse_3x.width(), 48.0);
        assert_eq!(nwse_3x.height(), 48.0);
    }

    #[test]
    fn test_cursor_colors_match_platform() {
        let arrow = rasterize_cursor(CursorHint::Arrow, 1);
        let pixels = &arrow.0.rgba;
        // Top-left pixel (0, 0) is '#' (border)
        let border_pixel = [pixels[0], pixels[1], pixels[2], pixels[3]];
        assert_eq!(border_pixel, BORDER_COLOR);

        #[cfg(target_os = "macos")]
        {
            assert_eq!(BORDER_COLOR, [255, 255, 255, 255]); // Mac: white border
            assert_eq!(FILL_COLOR, [0, 0, 0, 255]); // Mac: black fill
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(BORDER_COLOR, [0, 0, 0, 255]); // Windows: black border
            assert_eq!(FILL_COLOR, [255, 255, 255, 255]); // Windows: white fill
        }
    }

    #[test]
    fn test_draw_queues_quad_on_canvas() {
        let canvas = Canvas::default();
        let mut cache = SoftwareCursorCache::default();

        cache.draw(&canvas, CursorHint::Arrow, Vec2::new(100.0, 200.0), 2);
        let quads = canvas.drain();
        assert_eq!(quads.len(), 1);
        let quad = &quads[0];
        assert!(quad.texture.is_some());
        assert_eq!(quad.destination.x, 100.0);
        assert_eq!(quad.destination.y, 200.0);
        assert_eq!(quad.destination.w, 32.0);
        assert_eq!(quad.destination.h, 40.0);

        // Resize grips offset by center (8 * scale = 16)
        cache.draw(&canvas, CursorHint::ResizeNwSe, Vec2::new(100.0, 200.0), 2);
        let quads = canvas.drain();
        assert_eq!(quads.len(), 1);
        let quad = &quads[0];
        assert_eq!(quad.destination.x, 84.0); // 100 - 16
        assert_eq!(quad.destination.y, 184.0); // 200 - 16
        assert_eq!(quad.destination.w, 32.0);
        assert_eq!(quad.destination.h, 32.0);
    }

    #[test]
    fn test_resolve_pointer_top_edge_clamping_and_recovery() {
        let mut cache = SoftwareCursorCache::default();
        let window = bevy::window::Window {
            resolution: bevy::window::WindowResolution::new(1920, 1080),
            focused: true,
            ..Default::default()
        };

        // 1. Normal interior position: no recovery, not near top
        let (p, recovered) =
            cache.resolve_pointer(Some(bevy::math::Vec2::new(500.0, 300.0)), None, &window);
        assert_eq!(p, Some(glam::Vec2::new(500.0, 300.0)));
        assert!(!recovered);
        assert!(!cache.was_at_top);

        // 2. Near top edge (y = 2.0 <= 32.0): sets was_at_top = true, no recovery yet
        let (p, recovered) =
            cache.resolve_pointer(Some(bevy::math::Vec2::new(500.0, 2.0)), None, &window);
        assert_eq!(p, Some(glam::Vec2::new(500.0, 2.0)));
        assert!(!recovered);
        assert!(cache.was_at_top);

        // 3. Overshoot top bezel (y = -1.0): clamped to y = 0.0, remains was_at_top
        let (p, recovered) =
            cache.resolve_pointer(Some(bevy::math::Vec2::new(500.0, -1.0)), None, &window);
        assert_eq!(p, Some(glam::Vec2::new(500.0, 0.0)));
        assert!(!recovered);
        assert!(cache.was_at_top);

        // 4. Cursor position None while focused: persists at y = 0.0
        let (p, recovered) = cache.resolve_pointer(None, None, &window);
        assert_eq!(p, Some(glam::Vec2::new(500.0, 0.0)));
        assert!(!recovered);
        assert!(cache.was_at_top);

        // 5. Pulling back down into interior (y = 200.0): edge_recovered = true!
        let (p, recovered) =
            cache.resolve_pointer(Some(bevy::math::Vec2::new(500.0, 200.0)), None, &window);
        assert_eq!(p, Some(glam::Vec2::new(500.0, 200.0)));
        assert!(recovered);
        assert!(!cache.was_at_top);
    }

    #[test]
    fn test_resolve_cursor_scale() {
        assert_eq!(resolve_cursor_scale(), 2);
    }

    #[test]
    fn test_adjust_cursor_position_for_mode_switch() {
        let pos = bevy::math::Vec2::new(640.0, 400.0);
        // Entering fullscreen doubles coordinates
        let fs_pos = adjust_cursor_position_for_mode_switch(pos, true, None);
        assert_eq!(fs_pos, bevy::math::Vec2::new(1280.0, 800.0));

        // Leaving fullscreen halves coordinates
        let win_bounds = Some(bevy::math::Vec2::new(1280.0, 800.0));
        let win_pos = adjust_cursor_position_for_mode_switch(fs_pos, false, win_bounds);
        assert_eq!(win_pos, bevy::math::Vec2::new(640.0, 400.0));

        // Clamping to bounds on exit
        let oversized = bevy::math::Vec2::new(4000.0, 3000.0);
        let clamped = adjust_cursor_position_for_mode_switch(oversized, false, win_bounds);
        assert_eq!(clamped, bevy::math::Vec2::new(1280.0, 800.0));
    }

    #[test]
    fn test_accelerate_cursor_position_2x() {
        let max_bounds = bevy::math::Vec2::new(1920.0, 1080.0);
        let prev = bevy::math::Vec2::new(100.0, 100.0);

        // Positive movement is doubled (delta = +10, +5 -> +20, +10)
        let curr = bevy::math::Vec2::new(110.0, 105.0);
        let accelerated = accelerate_cursor_position_2x(curr, prev, max_bounds);
        assert_eq!(accelerated, bevy::math::Vec2::new(120.0, 110.0));

        // Zero delta produces no change
        let still = accelerate_cursor_position_2x(curr, curr, max_bounds);
        assert_eq!(still, curr);

        // Negative movement is doubled (delta = -20, -10 -> -40, -20)
        let curr_neg = bevy::math::Vec2::new(80.0, 90.0);
        let accelerated_neg = accelerate_cursor_position_2x(curr_neg, prev, max_bounds);
        assert_eq!(accelerated_neg, bevy::math::Vec2::new(60.0, 80.0));

        // Clamping to min boundary (0, 0)
        let far_neg = bevy::math::Vec2::new(10.0, 10.0);
        let clamped_zero = accelerate_cursor_position_2x(far_neg, prev, max_bounds);
        assert_eq!(clamped_zero, bevy::math::Vec2::ZERO);

        // Clamping to max boundary
        let near_max = bevy::math::Vec2::new(1910.0, 1070.0);
        let past_max = bevy::math::Vec2::new(1915.0, 1075.0);
        let clamped_max = accelerate_cursor_position_2x(past_max, near_max, max_bounds);
        assert_eq!(clamped_max, max_bounds);
    }
}
