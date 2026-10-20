//! Custom scaled cursors for high-DPI fullscreen modes.
//!
//! In exclusive fullscreen modes, the display runs at native unscaled pixel resolution
//! (scale factor 1.0). High-DPI displays (e.g. 2x Retina on macOS or 200%-300% on Windows)
//! render the OS system cursor at 1x, making it appear tiny. This module generates
//! scaled cursor images (1x, 2x, 3x) matching authentic Source/OS cursor shapes,
//! active only while the game is focused in fullscreen mode.

use bevy::{
    asset::{Assets, Handle, RenderAssetUsages},
    ecs::{
        entity::Entity,
        query::With,
        resource::Resource,
        system::{Commands, Query, Res, ResMut},
    },
    image::Image,
    math::Vec2,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::{
        CursorIcon, CustomCursor, CustomCursorImage, PrimaryMonitor, PrimaryWindow,
        SystemCursorIcon, Window, WindowMode,
    },
};
use hl2_ui::gameui::placement::CursorHint;

/// 16x20 classic arrow cursor pattern.
/// '#' = black outline, 'O' = white interior, '.' = transparent.
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

/// 16x16 NWSE diagonal resize arrow pattern (hotspot at center 8, 8).
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

/// 16x16 NESW diagonal resize arrow pattern (hotspot at center 8, 8).
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

/// Rasterizes an ASCII pattern into an RGBA [`Image`] scaled by integer factor `scale`.
pub fn rasterize_cursor(
    pattern: &[&str],
    scale: u32,
    base_hotspot: (u16, u16),
) -> (Image, (u16, u16)) {
    let scale = scale.max(1);
    let base_h = pattern.len() as u32;
    let base_w = pattern.first().map_or(0, |row| row.len()) as u32;
    let width = base_w * scale;
    let height = base_h * scale;

    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for (row_idx, row) in pattern.iter().enumerate() {
        for (col_idx, ch) in row.chars().enumerate() {
            let color: [u8; 4] = match ch {
                '#' => [0, 0, 0, 255],       // black outline
                'O' => [255, 255, 255, 255], // white interior
                _ => [0, 0, 0, 0],           // transparent
            };
            if color[3] == 0 {
                continue;
            }
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

    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD,
    );
    let hotspot = (base_hotspot.0 * scale as u16, base_hotspot.1 * scale as u16);
    (image, hotspot)
}

/// Pre-created custom cursor assets scaled for 2x and 3x Retina/high-DPI modes.
#[derive(Resource, Clone)]
pub struct ScaledCursorImages {
    pub arrow_2x: Handle<Image>,
    pub arrow_3x: Handle<Image>,
    pub nwse_2x: Handle<Image>,
    pub nwse_3x: Handle<Image>,
    pub nesw_2x: Handle<Image>,
    pub nesw_3x: Handle<Image>,
}

impl ScaledCursorImages {
    /// Loads and registers the scaled cursor images into asset storage.
    pub fn new(images: &mut Assets<Image>) -> Self {
        let (img_arrow_2x, _) = rasterize_cursor(ARROW_1X, 2, (0, 0));
        let (img_arrow_3x, _) = rasterize_cursor(ARROW_1X, 3, (0, 0));
        let (img_nwse_2x, _) = rasterize_cursor(NWSE_1X, 2, (8, 8));
        let (img_nwse_3x, _) = rasterize_cursor(NWSE_1X, 3, (8, 8));
        let (img_nesw_2x, _) = rasterize_cursor(NESW_1X, 2, (8, 8));
        let (img_nesw_3x, _) = rasterize_cursor(NESW_1X, 3, (8, 8));

        Self {
            arrow_2x: images.add(img_arrow_2x),
            arrow_3x: images.add(img_arrow_3x),
            nwse_2x: images.add(img_nwse_2x),
            nwse_3x: images.add(img_nwse_3x),
            nesw_2x: images.add(img_nesw_2x),
            nesw_3x: images.add(img_nesw_3x),
        }
    }

    /// Resolves the cursor icon for a placement hint and scale factor (2x, 3x).
    /// If scale <= 1, falls back to the system cursor.
    pub fn icon(&self, hint: CursorHint, scale: u32) -> CursorIcon {
        if scale < 2 {
            return standard_cursor_icon(hint);
        }
        let (handle, hotspot) = match (hint, scale) {
            (CursorHint::Arrow, 2) => (self.arrow_2x.clone(), (0, 0)),
            (CursorHint::Arrow, _) => (self.arrow_3x.clone(), (0, 0)),
            (CursorHint::ResizeNwSe, 2) => (self.nwse_2x.clone(), (16, 16)),
            (CursorHint::ResizeNwSe, _) => (self.nwse_3x.clone(), (24, 24)),
            (CursorHint::ResizeNeSw, 2) => (self.nesw_2x.clone(), (16, 16)),
            (CursorHint::ResizeNeSw, _) => (self.nesw_3x.clone(), (24, 24)),
        };

        CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
            handle,
            texture_atlas: None,
            flip_x: false,
            flip_y: false,
            rect: None,
            hotspot,
        }))
    }
}

/// Standard OS cursor icon without custom scaling.
pub fn standard_cursor_icon(hint: CursorHint) -> CursorIcon {
    CursorIcon::System(match hint {
        CursorHint::Arrow => SystemCursorIcon::Default,
        CursorHint::ResizeNwSe => SystemCursorIcon::NwseResize,
        CursorHint::ResizeNeSw => SystemCursorIcon::NeswResize,
    })
}

/// Initializes scaled cursor assets on startup.
pub fn setup_cursor_images(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(ScaledCursorImages::new(&mut images));
}

/// Applies cursor hint with high-DPI scaling compensation in exclusive fullscreen mode.
/// Reverts strictly to standard system cursors in windowed, borderless, or unfocused states.
pub fn apply_cursor_hint(
    ui: Option<Res<crate::ui_host::Console>>,
    windows: Query<(Entity, &Window, Option<&CursorIcon>), With<PrimaryWindow>>,
    monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>,
    cursors: Option<Res<ScaledCursorImages>>,
    mut commands: Commands,
) {
    let Ok((entity, window, current)) = windows.single() else {
        return;
    };
    let hint = ui.map_or_else(Default::default, |ui| ui.source.cursor_hint());

    let is_fullscreen = matches!(window.mode, WindowMode::Fullscreen(..)) && window.focused;
    let icon = if is_fullscreen {
        let sys_scale = monitors
            .iter()
            .next()
            .map(|m| m.scale_factor.round().clamp(1.0, 4.0) as u32)
            .unwrap_or(1);
        if sys_scale >= 2
            && let Some(cursors) = &cursors
        {
            cursors.icon(hint, sys_scale)
        } else {
            standard_cursor_icon(hint)
        }
    } else {
        standard_cursor_icon(hint)
    };

    if current != Some(&icon) {
        commands.entity(entity).insert(icon);
    }
}

/// Tracks previous window mode and dimensions to adjust cursor placement across transitions.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct WindowTransitionCursorState {
    pub last_mode: Option<WindowMode>,
    pub last_size: Vec2,
    pub last_cursor: Option<Vec2>,
}

/// Maintains proportional cursor placement across window mode transitions (e.g. entering
/// or leaving fullscreen), preventing the cursor from jumping to the corner or off-screen.
pub fn sync_cursor_position_on_window_transition(
    mut state: ResMut<WindowTransitionCursorState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let current_mode = window.mode;
    let current_size = Vec2::new(window.width(), window.height());
    let current_cursor = window.cursor_position();

    if let (Some(last_mode), Some(last_cursor)) = (state.last_mode, state.last_cursor) {
        let mode_changed = (matches!(last_mode, WindowMode::Fullscreen(..))
            != matches!(current_mode, WindowMode::Fullscreen(..)))
            || (last_mode != current_mode);

        if mode_changed
            && state.last_size.x > 0.0
            && state.last_size.y > 0.0
            && current_size.x > 0.0
            && current_size.y > 0.0
        {
            let normalized = last_cursor / state.last_size;
            let target = (normalized * current_size).clamp(Vec2::ZERO, current_size - Vec2::ONE);
            window.set_cursor_position(Some(target));
        }
    }

    state.last_mode = Some(current_mode);
    state.last_size = current_size;
    state.last_cursor = window.cursor_position().or(current_cursor);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rasterize_cursor_scaling() {
        let (img_1x, hotspot_1x) = rasterize_cursor(ARROW_1X, 1, (0, 0));
        assert_eq!(img_1x.texture_descriptor.size.width, 16);
        assert_eq!(img_1x.texture_descriptor.size.height, 20);
        assert_eq!(hotspot_1x, (0, 0));

        let (img_2x, hotspot_2x) = rasterize_cursor(ARROW_1X, 2, (0, 0));
        assert_eq!(img_2x.texture_descriptor.size.width, 32);
        assert_eq!(img_2x.texture_descriptor.size.height, 40);
        assert_eq!(hotspot_2x, (0, 0));

        let (img_3x, hotspot_3x) = rasterize_cursor(ARROW_1X, 3, (0, 0));
        assert_eq!(img_3x.texture_descriptor.size.width, 48);
        assert_eq!(img_3x.texture_descriptor.size.height, 60);
        assert_eq!(hotspot_3x, (0, 0));

        let (nwse_2x, nwse_hotspot_2x) = rasterize_cursor(NWSE_1X, 2, (8, 8));
        assert_eq!(nwse_2x.texture_descriptor.size.width, 32);
        assert_eq!(nwse_2x.texture_descriptor.size.height, 32);
        assert_eq!(nwse_hotspot_2x, (16, 16));
    }

    #[test]
    fn test_standard_cursor_icon_fallbacks() {
        assert_eq!(
            standard_cursor_icon(CursorHint::Arrow),
            CursorIcon::System(SystemCursorIcon::Default)
        );
        assert_eq!(
            standard_cursor_icon(CursorHint::ResizeNwSe),
            CursorIcon::System(SystemCursorIcon::NwseResize)
        );
        assert_eq!(
            standard_cursor_icon(CursorHint::ResizeNeSw),
            CursorIcon::System(SystemCursorIcon::NeswResize)
        );
    }

    #[test]
    fn test_apply_cursor_hint_fullscreen_and_windowed_reversion() {
        use bevy::app::App;
        use bevy::asset::AssetApp;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::window::{Monitor, MonitorSelection, VideoModeSelection};

        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Image>();
        app.world_mut()
            .run_system_once(setup_cursor_images)
            .unwrap();

        // 1. In Windowed mode: must always use standard system cursor
        let window_entity = app
            .world_mut()
            .spawn((
                PrimaryWindow,
                Window {
                    mode: WindowMode::Windowed,
                    focused: true,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().run_system_once(apply_cursor_hint).unwrap();
        let icon = app.world().get::<CursorIcon>(window_entity);
        assert_eq!(icon, Some(&CursorIcon::System(SystemCursorIcon::Default)));

        // 2. Add a 2x Retina Monitor
        app.world_mut().spawn((
            PrimaryMonitor,
            Monitor {
                name: Some("Retina".into()),
                physical_width: 3456,
                physical_height: 2234,
                physical_position: bevy::math::IVec2::ZERO,
                refresh_rate_millihertz: Some(60000),
                scale_factor: 2.0,
                video_modes: Vec::new(),
            },
        ));

        // 3. Switch to Fullscreen, focused: must use 2x custom cursor
        {
            let mut entity = app.world_mut().entity_mut(window_entity);
            let mut win = entity.get_mut::<Window>().unwrap();
            win.mode =
                WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current);
        }

        app.world_mut().run_system_once(apply_cursor_hint).unwrap();
        let icon = app.world().get::<CursorIcon>(window_entity).unwrap();
        assert!(
            matches!(icon, CursorIcon::Custom(_)),
            "must use scaled custom cursor in focused fullscreen on high-DPI"
        );

        // 4. Lose focus in Fullscreen: must revert to standard system cursor
        {
            let mut entity = app.world_mut().entity_mut(window_entity);
            let mut win = entity.get_mut::<Window>().unwrap();
            win.focused = false;
        }

        app.world_mut().run_system_once(apply_cursor_hint).unwrap();
        let icon = app.world().get::<CursorIcon>(window_entity).unwrap();
        assert_eq!(
            icon,
            &CursorIcon::System(SystemCursorIcon::Default),
            "must revert to system cursor when leaving focus"
        );

        // 5. Restore focus but switch back to Windowed: must remain standard system cursor
        {
            let mut entity = app.world_mut().entity_mut(window_entity);
            let mut win = entity.get_mut::<Window>().unwrap();
            win.focused = true;
            win.mode = WindowMode::Windowed;
        }

        app.world_mut().run_system_once(apply_cursor_hint).unwrap();
        let icon = app.world().get::<CursorIcon>(window_entity).unwrap();
        assert_eq!(
            icon,
            &CursorIcon::System(SystemCursorIcon::Default),
            "must revert to system cursor in windowed mode"
        );
    }

    #[test]
    fn test_sync_cursor_position_on_window_transition() {
        use bevy::app::App;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::window::{MonitorSelection, VideoModeSelection, WindowResolution};

        let mut app = App::new();
        app.init_resource::<WindowTransitionCursorState>();

        // Start in windowed 1728x1080 with cursor in center (864, 540)
        let window_entity = app
            .world_mut()
            .spawn((
                PrimaryWindow,
                Window {
                    mode: WindowMode::Windowed,
                    resolution: WindowResolution::new(1728, 1080),
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut()
            .entity_mut(window_entity)
            .get_mut::<Window>()
            .unwrap()
            .set_cursor_position(Some(Vec2::new(864.0, 540.0)));

        // Run system to establish baseline
        app.world_mut()
            .run_system_once(sync_cursor_position_on_window_transition)
            .unwrap();

        // Switch to Fullscreen 3456x2160
        {
            let mut entity = app.world_mut().entity_mut(window_entity);
            let mut win = entity.get_mut::<Window>().unwrap();
            win.mode =
                WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current);
            win.resolution = WindowResolution::new(3456, 2160);
        }

        // Run transition system
        app.world_mut()
            .run_system_once(sync_cursor_position_on_window_transition)
            .unwrap();

        let new_pos = app
            .world()
            .get::<Window>(window_entity)
            .unwrap()
            .cursor_position()
            .unwrap();
        assert_eq!(
            new_pos,
            Vec2::new(1728.0, 1080.0),
            "cursor must scale proportionally into fullscreen"
        );

        // Switch back to Windowed 1728x1080
        {
            let mut entity = app.world_mut().entity_mut(window_entity);
            let mut win = entity.get_mut::<Window>().unwrap();
            win.mode = WindowMode::Windowed;
            win.resolution = WindowResolution::new(1728, 1080);
        }

        app.world_mut()
            .run_system_once(sync_cursor_position_on_window_transition)
            .unwrap();

        let restored_pos = app
            .world()
            .get::<Window>(window_entity)
            .unwrap()
            .cursor_position()
            .unwrap();
        assert_eq!(
            restored_pos,
            Vec2::new(864.0, 540.0),
            "cursor must restore proportionally into windowed mode"
        );
    }
}
