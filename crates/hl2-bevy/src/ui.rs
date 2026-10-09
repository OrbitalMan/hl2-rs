//! User interface interaction, canvas viewport management, and menu hit testing.

use crate::video::VideoViewport;
use bevy::{
    app::AppExit,
    ecs::message::{MessageReader, MessageWriter},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryMonitor, PrimaryWindow, Window},
};

/// Maps a window cursor position in logical window coordinates to UI canvas coordinates,
/// properly offsetting for letterbox/pillarbox black stripes and filtering out positions
/// outside the active viewport.
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

/// Resizes the UI canvas to match the active viewport logical size.
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
                .before(crate::movement::MovementControlsSet),
        );
    }
}

type UiInputDevices<'w, 's> = (
    Res<'w, ButtonInput<KeyCode>>,
    Res<'w, ButtonInput<MouseButton>>,
    MessageReader<'w, 's, bevy::input::keyboard::KeyboardInput>,
);

type UiDisplayDevices<'w, 's> = (
    Query<'w, 's, (&'static mut Window, &'static mut CursorOptions), With<PrimaryWindow>>,
    Query<'w, 's, &'static bevy::window::Monitor, With<PrimaryMonitor>>,
);

#[allow(clippy::too_many_arguments)]
pub fn update_ui(
    (keys, buttons, mut characters): UiInputDevices,
    (mut windows, monitors): UiDisplayDevices,
    video_viewport: Option<Res<VideoViewport>>,
    mut sim: ResMut<crate::movement::Simulation>,
    mut game: Option<ResMut<crate::gameplay::Gameplay>>,
    mut exit: MessageWriter<AppExit>,
    mut ui: Option<ResMut<crate::console::Console>>,
    global_volume: Option<ResMut<bevy::audio::GlobalVolume>>,
) {
    if keys.just_pressed(KeyCode::F10) {
        if let Some(ui) = ui.as_deref_mut() {
            ui.quit_requested = true;
        }
        exit.write(AppExit::Success);
    }
    let Ok((mut window, mut cursor)) = windows.single_mut() else {
        return;
    };
    let text: String = characters
        .read()
        .filter(|e| e.state == bevy::input::ButtonState::Pressed)
        .filter_map(|e| e.text.as_ref())
        .map(|s| s.as_str())
        .collect();
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
            let effects = ui.source.input(&hl2_ui::console::Input {
                text,
                keys: crate::console::keys(&keys),
                pointer: cursor_to_canvas(
                    window.cursor_position(),
                    video_viewport.as_deref(),
                    &window,
                ),
                click: buttons.just_pressed(MouseButton::Left),
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
        }
        if let Some(video_cfg) = sim.pending_video_config.take() {
            crate::video::apply_video_config_to_window(&mut window, &video_cfg, &monitors);
        }
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
            sim.jump_suppressed |=
                crate::input::action_pressed("+jump", KeyCode::Space, &keys, cfg);
        } else if !sim.paused
            && buttons.just_pressed(MouseButton::Left)
            && cursor.grab_mode == CursorGrabMode::None
        {
            sim.transition = true;
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
            sim.jump_suppressed |=
                crate::input::action_pressed("+jump", KeyCode::Space, &keys, cfg);
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
            sim.jump_suppressed |=
                crate::input::action_pressed("+jump", KeyCode::Space, &keys, cfg);
        } else if buttons.just_pressed(MouseButton::Left) {
            sim.transition = sim.paused || cursor.grab_mode == CursorGrabMode::None;
            if sim.transition {
                sim.jump_suppressed |=
                    crate::input::action_pressed("+jump", KeyCode::Space, &keys, cfg);
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

        // Click at left edge of 4:3 area on screen (x=240, y=100) -> canvas (0, 100)
        let p = cursor_to_canvas(Some(Vec2::new(240.0, 100.0)), Some(&viewport), &window);
        assert_eq!(p, Some(glam::Vec2::new(0.0, 100.0)));

        // Click in the center of 4:3 area on screen (x=960, y=540) -> canvas (720, 540)
        let p_center = cursor_to_canvas(Some(Vec2::new(960.0, 540.0)), Some(&viewport), &window);
        assert_eq!(p_center, Some(glam::Vec2::new(720.0, 540.0)));

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
        assert_eq!(canvas.width(), 1440.0);
        assert_eq!(canvas.height(), 1080.0);
    }

    #[test]
    fn update_ui_system_resizes_canvas_and_syncs_mode() {
        let mut app = App::new();
        app.insert_resource(crate::movement::Simulation::new(
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
        .insert_resource(crate::console::Console::new(Default::default()))
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
        .add_message::<bevy::input::keyboard::KeyboardInput>();

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

        let ui = app.world().resource::<crate::console::Console>();
        assert_eq!(ui.source.canvas.width(), 1440.0);
        assert_eq!(ui.source.canvas.height(), 1080.0);
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

        // Inside active area (x=512, y=224 + 100 = 324) -> canvas (512, 100)
        let p_inside = cursor_to_canvas(Some(Vec2::new(512.0, 324.0)), Some(&viewport), &window);
        assert_eq!(p_inside, Some(glam::Vec2::new(512.0, 100.0)));
    }

    #[test]
    fn update_ui_escape_key_toggles_menu() {
        let mut app = App::new();
        app.insert_resource(crate::movement::Simulation::new(
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
        .insert_resource(crate::console::Console::new(Default::default()))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<AppExit>()
        .add_message::<bevy::input::keyboard::KeyboardInput>();

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
        let ui = app.world().resource::<crate::console::Console>();
        assert_eq!(ui.source.mode, hl2_ui::console::Mode::Pause);
    }
}
