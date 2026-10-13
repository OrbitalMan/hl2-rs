//! User interface interaction, canvas viewport management, and menu hit testing.

use crate::video::VideoViewport;
use bevy::{
    app::AppExit,
    ecs::message::{MessageReader, MessageWriter},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryMonitor, PrimaryWindow, Window},
};

/// Maps a window cursor position in logical window coordinates to UI canvas coordinates
/// (game-frame pixels), offsetting for letterbox/pillarbox black stripes, scaling from the
/// presented size to the frame resolution and filtering out positions outside the frame.
pub fn cursor_to_canvas(
    cursor_pos: Option<Vec2>,
    viewport: Option<&VideoViewport>,
    window: &Window,
) -> Option<glam::Vec2> {
    hl2_ui::viewport::cursor_to_canvas(
        cursor_pos.map(|p| glam::Vec2::new(p.x, p.y)),
        viewport.map(|v| v.bounds()).as_ref(),
        glam::Vec2::new(window.width(), window.height()),
    )
}

/// Resizes the UI canvas to the game-frame resolution it is drawn into.
pub fn resize_canvas(
    canvas: &hl2_ui::canvas::Canvas,
    viewport: Option<&VideoViewport>,
    window: &Window,
) {
    hl2_ui::viewport::resize_canvas(
        canvas,
        viewport.map(|v| v.bounds()).as_ref(),
        glam::Vec2::new(window.width(), window.height()),
    );
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiInputSet;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            RunFixedMainLoop,
            update_ui
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                .in_set(UiInputSet)
                .before(crate::simulation::MovementControlsSet),
        );
    }
}

type UiInputDevices<'w, 's> = (
    Res<'w, ButtonInput<KeyCode>>,
    Res<'w, ButtonInput<MouseButton>>,
    MessageReader<'w, 's, bevy::input::keyboard::KeyboardInput>,
    MessageReader<'w, 's, bevy::input::mouse::MouseWheel>,
);

type UiDisplayDevices<'w, 's> = (
    Query<'w, 's, (&'static mut Window, &'static mut CursorOptions), With<PrimaryWindow>>,
    Query<'w, 's, &'static bevy::window::Monitor, With<PrimaryMonitor>>,
);

#[allow(clippy::too_many_arguments)]
pub fn update_ui(
    (keys, buttons, mut characters, mut wheels): UiInputDevices,
    (mut windows, monitors): UiDisplayDevices,
    video_viewport: Option<Res<VideoViewport>>,
    mut sim: ResMut<crate::simulation::Simulation>,
    mut game: Option<ResMut<crate::gameplay::Gameplay>>,
    mut exit: MessageWriter<AppExit>,
    mut ui: Option<ResMut<crate::ui_host::Console>>,
    global_volume: Option<ResMut<bevy::audio::GlobalVolume>>,
) {
    if keys.just_pressed(KeyCode::F10) {
        if let Some(ui) = ui.as_deref_mut() {
            ui.quit_requested = true;
        }
        exit.write(AppExit::Success);
    }
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    let text: String = characters
        .read()
        .filter(|e| e.state == bevy::input::ButtonState::Pressed)
        .filter_map(|e| e.text.as_ref())
        .map(|s| s.as_str())
        .collect();
    let wheel = crate::ui_host::wheel_steps(wheels.read());
    if !sim.commands.is_empty() {
        return;
    }
    sim.transition = false;
    sim.focused = window.focused;
    let cancel_selection = keys.just_pressed(KeyCode::Escape)
        && !sim.paused
        && game.as_ref().is_some_and(|g| g.selection.pending.is_some());
    if cancel_selection && let Some(game) = game.as_deref_mut() {
        game.actions.push(crate::gameplay::Action::Cancel);
    }
    if let Some(ui) = ui.as_deref_mut() {
        let before = ui.source.mode;
        resize_canvas(&ui.source.canvas, video_viewport.as_deref(), &window);
        if keys.just_pressed(KeyCode::Escape) && !cancel_selection {
            ui.source.escape();
        }
        if keys.just_pressed(KeyCode::Backquote) {
            ui.source.toggle();
        }
        if !window.focused && !ui.source.paused() {
            ui.source.mode = hl2_ui::console::Mode::Pause;
        }
        if ui.source.mode == before
            && let Some(game) = game.as_deref_mut()
        {
            let pointer =
                cursor_to_canvas(window.cursor_position(), video_viewport.as_deref(), &window);
            if buttons.just_pressed(MouseButton::Left) {
                debug!(
                    "[ui] click cursor={:?} window={}x{} (scale {}) focused={} viewport={:?} -> canvas {:?} mode={:?}",
                    window.cursor_position(),
                    window.width(),
                    window.height(),
                    window.scale_factor(),
                    window.focused,
                    video_viewport.as_deref().map(|v| v.bounds()),
                    pointer,
                    ui.source.mode
                );
            }
            let effects = ui.source.input(&hl2_ui::console::Input {
                text,
                keys: crate::ui_host::keys(&keys),
                pointer,
                click: buttons.just_pressed(MouseButton::Left),
                held: buttons.pressed(MouseButton::Left),
                released: buttons.just_released(MouseButton::Left),
                wheel,
                binding_keys: crate::ui_host::binding_keys(&keys, &buttons, wheel),
            });
            let has_apply = effects
                .iter()
                .any(|e| matches!(e, hl2_ui::console::Effect::ApplyConfig(_)));
            sim.console_effects(ui, game, effects);
            if has_apply && let Some(mut gv) = global_volume {
                *gv = bevy::audio::GlobalVolume::from(bevy::audio::Volume::Linear(
                    ui.source.config.audio.volume,
                ));
            }
            let options = hl2_ui::console::Mode::Options;
            if before != options && ui.source.mode == options {
                ui.source
                    .set_display_modes(crate::video::display::modes(&monitors, &window));
            }
        }
        // Applied video settings (`sim.pending_video_config`) are carried out by
        // `video::apply_video_settings`.
        sim.transition = ui.source.mode != before;
        sim.paused = ui.source.paused();
        let cfg = Some(&ui.source.config);
        if sim.transition || sim.paused {
            cursor.grab_mode = if sim.paused {
                CursorGrabMode::None
            } else {
                CursorGrabMode::Locked
            };
            cursor.visible = sim.paused;
            sim.jump_suppressed |= crate::input::action_pressed("+jump", &keys, &buttons, cfg);
        } else if !sim.paused
            && buttons.just_pressed(MouseButton::Left)
            && cursor.grab_mode == CursorGrabMode::None
        {
            sim.transition = true;
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
            sim.jump_suppressed |= crate::input::action_pressed("+jump", &keys, &buttons, cfg);
        }
        if ui.quit_requested {
            exit.write(AppExit::Success);
        }
    } else {
        let cfg = ui.as_ref().map(|u| &u.source.config);
        if keys.just_pressed(KeyCode::Escape) && !cancel_selection || !window.focused {
            sim.transition = !sim.paused;
            sim.paused = true;
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
            sim.jump_suppressed |= crate::input::action_pressed("+jump", &keys, &buttons, cfg);
        } else if buttons.just_pressed(MouseButton::Left) {
            sim.transition = sim.paused || cursor.grab_mode == CursorGrabMode::None;
            if sim.transition {
                sim.jump_suppressed |= crate::input::action_pressed("+jump", &keys, &buttons, cfg);
            }
            sim.paused = false;
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn cursor_to_canvas_maps_inside_letterbox_viewport() {
        let window = Window {
            resolution: bevy::window::WindowResolution::new(1920, 1080),
            ..default()
        };
        let viewport = VideoViewport {
            target_resolution: UVec2::new(1024, 768),
            physical_rect: bevy::camera::Viewport {
                physical_position: UVec2::new(240, 0),
                physical_size: UVec2::new(1440, 1080),
                depth: 0.0..1.0,
            },
            logical_offset: Vec2::new(240.0, 0.0),
            logical_size: Vec2::new(1440.0, 1080.0),
        };

        // Click at top-left of the 4:3 area on screen (x=240, y=0) -> frame pixel (0, 0)
        let p = cursor_to_canvas(Some(Vec2::new(240.0, 0.0)), Some(&viewport), &window);
        assert_eq!(p, Some(glam::Vec2::new(0.0, 0.0)));

        // Click in the center of the 4:3 area (x=960, y=540) -> centre of the 1024x768 frame
        let p_center = cursor_to_canvas(Some(Vec2::new(960.0, 540.0)), Some(&viewport), &window);
        assert_eq!(p_center, Some(glam::Vec2::new(512.0, 384.0)));

        // Click in black stripe on the left (x=100 < 240) -> None
        let p_left_stripe =
            cursor_to_canvas(Some(Vec2::new(100.0, 500.0)), Some(&viewport), &window);
        assert_eq!(p_left_stripe, None);

        // Click in black stripe on the right (x=1700 > 1680) -> None
        let p_right_stripe =
            cursor_to_canvas(Some(Vec2::new(1700.0, 500.0)), Some(&viewport), &window);
        assert_eq!(p_right_stripe, None);
    }

    #[test]
    fn resize_canvas_resizes_to_viewport_logical_size() {
        let canvas = hl2_ui::canvas::Canvas::default();
        let window = Window {
            resolution: bevy::window::WindowResolution::new(1920, 1080),
            ..default()
        };
        let viewport = VideoViewport {
            target_resolution: UVec2::new(1024, 768),
            physical_rect: bevy::camera::Viewport {
                physical_position: UVec2::new(240, 0),
                physical_size: UVec2::new(1440, 1080),
                depth: 0.0..1.0,
            },
            logical_offset: Vec2::new(240.0, 0.0),
            logical_size: Vec2::new(1440.0, 1080.0),
        };

        resize_canvas(&canvas, Some(&viewport), &window);
        assert_eq!(canvas.width(), 1024.0);
        assert_eq!(canvas.height(), 768.0);
    }

    #[test]
    fn update_ui_system_resizes_canvas_and_syncs_mode() {
        let mut app = App::new();
        app.insert_resource(crate::simulation::Simulation::new(
            &modkit_core::World::default(),
            glam::Vec3::ZERO,
            0.,
            0.,
            false,
            vec![],
        ))
        .insert_resource(crate::gameplay::Gameplay::synthetic(
            modkit_core::World::default(),
        ))
        .insert_resource(crate::ui_host::Console::new(Default::default()))
        .insert_resource(VideoViewport {
            target_resolution: UVec2::new(1024, 768),
            physical_rect: bevy::camera::Viewport {
                physical_position: UVec2::new(240, 0),
                physical_size: UVec2::new(1440, 1080),
                depth: 0.0..1.0,
            },
            logical_offset: Vec2::new(240.0, 0.0),
            logical_size: Vec2::new(1440.0, 1080.0),
        })
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<AppExit>()
        .add_message::<bevy::input::keyboard::KeyboardInput>()
        .add_message::<bevy::input::mouse::MouseWheel>();

        app.world_mut().spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(1920, 1080),
                focused: true,
                ..default()
            },
            CursorOptions {
                grab_mode: CursorGrabMode::None,
                visible: true,
                ..default()
            },
            PrimaryWindow,
        ));

        app.world_mut().run_system_once(update_ui).unwrap();

        let ui = app.world().resource::<crate::ui_host::Console>();
        assert_eq!(ui.source.canvas.width(), 1024.0);
        assert_eq!(ui.source.canvas.height(), 768.0);
    }

    #[test]
    fn cursor_to_canvas_maps_inside_letterbox_horizontal_bars() {
        let window = Window {
            resolution: bevy::window::WindowResolution::new(1024, 1024),
            ..default()
        };
        // 16:9 target in a square 1024x1024 window -> top and bottom letterbox bars
        let viewport = VideoViewport {
            target_resolution: UVec2::new(1280, 720),
            physical_rect: bevy::camera::Viewport {
                physical_position: UVec2::new(0, 224),
                physical_size: UVec2::new(1024, 576),
                depth: 0.0..1.0,
            },
            logical_offset: Vec2::new(0.0, 224.0),
            logical_size: Vec2::new(1024.0, 576.0),
        };

        // Top stripe (y=100 < 224) -> None
        let p_top = cursor_to_canvas(Some(Vec2::new(512.0, 100.0)), Some(&viewport), &window);
        assert_eq!(p_top, None);

        // Bottom stripe (y=850 > 800) -> None
        let p_bottom = cursor_to_canvas(Some(Vec2::new(512.0, 850.0)), Some(&viewport), &window);
        assert_eq!(p_bottom, None);

        // Inside active area (x=512, y=224 + 100 = 324) -> 1280x720 frame pixel (640, 125)
        let p_inside = cursor_to_canvas(Some(Vec2::new(512.0, 324.0)), Some(&viewport), &window);
        assert_eq!(p_inside, Some(glam::Vec2::new(640.0, 125.0)));
    }

    #[test]
    fn update_ui_escape_key_toggles_menu() {
        let mut app = App::new();
        app.insert_resource(crate::simulation::Simulation::new(
            &modkit_core::World::default(),
            glam::Vec3::ZERO,
            0.,
            0.,
            false,
            vec![],
        ))
        .insert_resource(crate::gameplay::Gameplay::synthetic(
            modkit_core::World::default(),
        ))
        .insert_resource(crate::ui_host::Console::new(Default::default()))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<AppExit>()
        .add_message::<bevy::input::keyboard::KeyboardInput>()
        .add_message::<bevy::input::mouse::MouseWheel>();

        app.world_mut().spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(800, 600),
                focused: true,
                ..default()
            },
            CursorOptions::default(),
            PrimaryWindow,
        ));

        // Press Escape
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);

        app.world_mut().run_system_once(update_ui).unwrap();

        // UI state should now be in Pause menu
        let ui = app.world().resource::<crate::ui_host::Console>();
        assert_eq!(ui.source.mode, hl2_ui::console::Mode::Pause);
    }

    #[test]
    fn update_ui_keeps_the_background_level_main_menu_on_escape() {
        let mut app = App::new();
        let mut console = crate::ui_host::Console::new(Default::default());
        console.source.level_loaded(true);
        app.insert_resource(crate::simulation::Simulation::new(
            &modkit_core::World::default(),
            glam::Vec3::ZERO,
            0.,
            0.,
            false,
            vec![],
        ))
        .insert_resource(crate::gameplay::Gameplay::synthetic(
            modkit_core::World::default(),
        ))
        .insert_resource(console)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<AppExit>()
        .add_message::<bevy::input::keyboard::KeyboardInput>()
        .add_message::<bevy::input::mouse::MouseWheel>();
        app.world_mut().spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(800, 600),
                focused: true,
                ..default()
            },
            CursorOptions::default(),
            PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.world_mut().run_system_once(update_ui).unwrap();
        let ui = app.world().resource::<crate::ui_host::Console>();
        assert_eq!(ui.source.mode, hl2_ui::console::Mode::MainMenu);
        assert!(ui.source.background_level());
        let sim = app.world().resource::<crate::simulation::Simulation>();
        assert!(
            sim.paused(),
            "the background level stays paused behind the menu"
        );
        let (window, cursor) = app
            .world_mut()
            .query::<(&Window, &CursorOptions)>()
            .single(app.world())
            .map(|(w, c)| (w.focused, c.visible))
            .unwrap();
        assert!(window && cursor, "menu cursor is visible");
    }
}
