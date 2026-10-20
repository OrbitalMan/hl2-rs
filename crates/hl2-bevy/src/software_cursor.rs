//! Software cursor rendering for fullscreen mode (`cl_software_cursor`).
//!
//! On high-DPI displays in exclusive fullscreen mode, the operating system hardware cursor
//! can render at 1x point size (appearing microscopic). When `cl_software_cursor` is enabled,
//! the OS cursor is hidden in fullscreen and drawn as a crisp, integer-scaled 2D quad directly
//! in the UI presentation pass.

use bevy::window::WindowMode;
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

fn rasterize_cursor(hint: CursorHint, scale: u32) -> Texture2D {
    let pattern = match hint {
        CursorHint::Arrow => ARROW_1X,
        CursorHint::ResizeNwSe => NWSE_1X,
        CursorHint::ResizeNeSw => NESW_1X,
    };
    let scale = scale.clamp(1, 4);
    let base_h = pattern.len() as u32;
    let base_w = pattern.first().map_or(0, |row| row.len()) as u32;
    let width = base_w * scale;
    let height = base_h * scale;

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for (row_idx, row) in pattern.iter().enumerate() {
        for (col_idx, ch) in row.chars().enumerate() {
            let color: [u8; 4] = match ch {
                '#' => [0, 0, 0, 255],
                'O' => [255, 255, 255, 255],
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
        let scale = scale.clamp(1, 4);
        self.textures
            .entry((id, scale))
            .or_insert_with(|| rasterize_cursor(hint, scale))
    }

    /// Draws the software cursor onto the given canvas at the specified pointer position.
    pub fn draw(&mut self, canvas: &Canvas, hint: CursorHint, pointer: Vec2, scale: u32) {
        let scale = scale.clamp(1, 4);
        let texture = self.get(hint, scale).clone();
        let offset = match hint {
            CursorHint::Arrow => Vec2::ZERO,
            CursorHint::ResizeNwSe | CursorHint::ResizeNeSw => Vec2::splat(8.0 * scale as f32),
        };
        let pos = pointer - offset;
        draw_texture(canvas, &texture, pos.x, pos.y, WHITE);
    }
}

/// Determines whether the software cursor should be active:
/// only when `cl_software_cursor` is enabled, in fullscreen mode, when focused, and when UI is active.
pub fn is_active(
    software_cursor_config: bool,
    window_mode: &WindowMode,
    window_focused: bool,
    ui_active: bool,
) -> bool {
    software_cursor_config
        && matches!(window_mode, WindowMode::Fullscreen(..))
        && window_focused
        && ui_active
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::{MonitorSelection, VideoModeSelection};

    #[test]
    fn test_is_active_conditions() {
        let fullscreen =
            WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current);
        let windowed = WindowMode::Windowed;
        let borderless = WindowMode::BorderlessFullscreen(MonitorSelection::Primary);

        // Active only when config=true, fullscreen, focused, and ui_active
        assert!(is_active(true, &fullscreen, true, true));

        // Inactive if config is false
        assert!(!is_active(false, &fullscreen, true, true));

        // Inactive in windowed or borderless mode
        assert!(!is_active(true, &windowed, true, true));
        assert!(!is_active(true, &borderless, true, true));

        // Inactive when window lost focus
        assert!(!is_active(true, &fullscreen, false, true));

        // Inactive when UI is not active (normal gameplay)
        assert!(!is_active(true, &fullscreen, true, false));
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
}
