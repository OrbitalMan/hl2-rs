//! User interface interaction, canvas viewport management, and menu hit testing.

use crate::video::VideoViewport;
use bevy::{
    app::AppExit,
    ecs::message::{MessageReader, MessageWriter},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryMonitor, PrimaryWindow, Window, WindowMode},
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

/// Converts canvas coordinates back to logical window coordinates.
#[allow(dead_code)]
pub fn canvas_to_cursor(
    canvas_pos: glam::Vec2,
    viewport: Option<&VideoViewport>,
    window: &Window,
) -> Vec2 {
    let p = hl2_ui::viewport::canvas_to_cursor(
        canvas_pos,
        viewport.map(|v| v.bounds()).as_ref(),
        glam::Vec2::new(window.width(), window.height()),
    );
    Vec2::new(p.x, p.y)
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

/// Gives the Options dialog the display's resolutions each time it opens or the window is resized,
/// whatever triggered it (pointer, keyboard or a scripted replay).
pub fn refresh_display_modes(
    mut was_options: Local<bool>,
    windows: Query<Ref<Window>, With<PrimaryWindow>>,
    monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>,
    options: Option<Res<crate::Options>>,
    ui: Option<ResMut<crate::ui_host::Console>>,
) {
    let Some(mut ui) = ui else {
        return;
    };
    let is_options = ui.source.options_open();
    let opened = is_options && !*was_options;
    *was_options = is_options;
    if is_options
        && let Ok(window) = windows.single()
        && (opened || window.is_changed())
    {
        let fake = options.as_ref().and_then(|o| o.fake_monitor.as_ref());
        ui.source
            .set_display_modes(crate::video::display::modes(&monitors, &window, fake));
    }
}

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, refresh_display_modes);
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
    Option<Res<'w, Time<Real>>>,
    Option<ResMut<'w, bevy::clipboard::Clipboard>>,
);

type UiWindow<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Window, &'static mut CursorOptions), With<PrimaryWindow>>;

#[allow(clippy::too_many_arguments)]
pub fn update_ui(
    (keys, buttons, mut characters, mut wheels, real_time, mut clipboard): UiInputDevices,
    mut windows: UiWindow,
    video_viewport: Option<Res<VideoViewport>>,
    mut sim: ResMut<crate::simulation::Simulation>,
    mut game: Option<ResMut<crate::gameplay::Gameplay>>,
    mut exit: MessageWriter<AppExit>,
    mut ui: Option<ResMut<crate::ui_host::Console>>,
    global_volume: Option<ResMut<bevy::audio::GlobalVolume>>,
    mut _was_at_top: Local<bool>,
    mut _last_fullscreen: Local<Option<bool>>,
    mut _rehide_countdown: Local<u32>,
    mut _last_sw_active: Local<Option<bool>>,
    mut _last_cursor_pos: Local<Option<Vec2>>,
    mut _last_canvas_pointer: Local<Option<glam::Vec2>>,
    mut _mode_switch_settle: Local<u32>,
) {
    if keys.just_pressed(KeyCode::F10) {
        if let Some(ui) = ui.as_deref_mut() {
            ui.quit_requested = true;
        }
        exit.write(AppExit::Success);
    }
    #[allow(unused_mut)]
    let Ok((_window_entity, mut window, mut cursor)) = windows.single_mut() else {
        return;
    };
    let text: String = characters
        .read()
        .filter(|e| e.state == bevy::input::ButtonState::Pressed)
        .filter_map(|e| e.text.as_ref())
        .map(|s| s.as_str())
        .collect();
    let wheel = crate::ui_host::wheel_steps(wheels.read());
    // The UI clock runs every frame, also while a script drives the input (a keep-video
    // confirmation reverts at its timeout).
    if let (Some(ui), Some(game), Some(now)) = (
        ui.as_deref_mut(),
        game.as_deref_mut(),
        real_time.as_deref().map(Time::elapsed_secs_f64),
    ) {
        let effects = ui.source.tick(now);
        if !effects.is_empty() {
            sim.console_effects(ui, game, effects);
        }
    }
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
        let is_fullscreen = matches!(window.mode, WindowMode::Fullscreen(..));
        let _mode_switched = if let Some(was_fullscreen) = _last_fullscreen.replace(is_fullscreen) {
            if !was_fullscreen && is_fullscreen {
                ui.source.config.mouse.software_cursor = true;
                #[cfg(target_os = "macos")]
                {
                    let prev = (*_last_cursor_pos).or_else(|| window.cursor_position());
                    if let Some(pos) = prev {
                        let target = crate::software_cursor::adjust_cursor_position_for_mode_switch(
                            pos,
                            true,
                            Some(Vec2::new(window.width(), window.height())),
                        );
                        window.set_cursor_position(Some(target));
                        *_last_cursor_pos = Some(target);
                    }
                }
                true
            } else if was_fullscreen && !is_fullscreen {
                ui.source.config.mouse.software_cursor = false;
                #[cfg(target_os = "macos")]
                {
                    crate::software_cursor::restore_macos_hardware_cursor(_window_entity);
                    let prev = (*_last_cursor_pos).or_else(|| window.cursor_position());
                    if let Some(pos) = prev {
                        let target = crate::software_cursor::adjust_cursor_position_for_mode_switch(
                            pos,
                            false,
                            Some(Vec2::new(window.width(), window.height())),
                        );
                        window.set_cursor_position(Some(target));
                        *_last_cursor_pos = Some(target);
                    }
                }
                true
            } else {
                false
            }
        } else {
            ui.source.config.mouse.software_cursor = is_fullscreen;
            #[cfg(target_os = "macos")]
            if !is_fullscreen {
                crate::software_cursor::restore_macos_hardware_cursor(_window_entity);
            }
            false
        };

        if !is_fullscreen && ui.source.config.mouse.software_cursor {
            ui.source.config.mouse.software_cursor = false;
        }

        #[cfg(target_os = "macos")]
        if _mode_switched {
            *_mode_switch_settle = 30;
        }

        let sw_active = crate::software_cursor::is_active(
            ui.source.config.mouse.software_cursor,
            window.focused,
            ui.source.paused(),
            is_fullscreen,
        );

        #[cfg(target_os = "macos")]
        if *_mode_switch_settle > 0 {
            *_mode_switch_settle -= 1;
            *_last_cursor_pos = window.cursor_position();
        } else if is_fullscreen && sw_active && window.focused {
            if let Some(curr_pos) = window.cursor_position() {
                if let Some(prev_pos) = *_last_cursor_pos {
                    let delta = curr_pos - prev_pos;
                    if delta.length_squared() > 0.0 && delta.length() <= 150.0 {
                        let max_bounds = Vec2::new(window.width(), window.height());
                        let new_pos = crate::software_cursor::accelerate_cursor_position_2x(
                            curr_pos, prev_pos, max_bounds,
                        );
                        window.set_cursor_position(Some(new_pos));
                        *_last_cursor_pos = Some(new_pos);
                    } else {
                        *_last_cursor_pos = Some(curr_pos);
                    }
                } else {
                    *_last_cursor_pos = Some(curr_pos);
                }
            } else {
                *_last_cursor_pos = None;
            }
        } else {
            *_last_cursor_pos = window.cursor_position();
        }

        let before = ui.source.mode();
        resize_canvas(&ui.source.canvas, video_viewport.as_deref(), &window);
        if keys.just_pressed(KeyCode::Escape) && !cancel_selection {
            ui.source.escape_pressed();
        }
        if keys.just_released(KeyCode::Escape) {
            ui.source.escape_released();
        }
        if keys.just_pressed(KeyCode::Backquote) {
            ui.source.toggle();
        }
        if !window.focused && !ui.source.paused() {
            ui.source.set_mode(hl2_ui::console::Mode::Pause);
        }
        if ui.source.mode() == before
            && let Some(game) = game.as_deref_mut()
        {
            let pointer =
                cursor_to_canvas(window.cursor_position(), video_viewport.as_deref(), &window);
            #[cfg(target_os = "macos")]
            if let Some(p) = pointer {
                *_last_canvas_pointer = Some(p);
            }
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
                    ui.source.mode()
                );
            }
            let modifiers = crate::ui_host::modifiers(&keys);
            // A clipboard that answers later (not the desktop backends) is not waited for.
            let paste = modifiers
                .paste
                .then(|| clipboard.as_deref_mut()?.fetch_text().poll_result()?.ok())
                .flatten();
            let effects = ui.source.input(&hl2_ui::console::Input {
                text: if modifiers.ctrl || modifiers.command {
                    String::new()
                } else {
                    text
                },
                ctrl: modifiers.ctrl,
                shift: modifiers.shift,
                alt: modifiers.alt,
                paste,
                copy: modifiers.copy,
                keys: crate::ui_host::keys(&keys),
                held_keys: crate::ui_host::held_keys(&keys),
                pointer,
                click: buttons.just_pressed(MouseButton::Left),
                held: buttons.pressed(MouseButton::Left),
                released: buttons.just_released(MouseButton::Left),
                wheel,
                binding_keys: crate::ui_host::binding_keys(&keys, &buttons, wheel),
                time: real_time.as_deref().map(Time::elapsed_secs_f64),
            });
            if let Some(copied) = ui.source.take_copied()
                && let Some(clipboard) = clipboard.as_deref_mut()
                && let Err(error) = clipboard.set_text(copied)
            {
                warn!("[ui] console copy failed: {error:?}");
            }
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
        // Applied video settings (`sim.pending_video_config`) are carried out by
        // `video::apply_video_settings`.
        sim.transition = ui.source.mode() != before;
        sim.paused = ui.source.paused();
        let cfg = Some(&ui.source.config);
        #[cfg(target_os = "macos")]
        if let Some(was_sw) = _last_sw_active.replace(sw_active) {
            if was_sw && !sw_active {
                crate::software_cursor::restore_macos_hardware_cursor(_window_entity);
            } else if !was_sw && sw_active {
                crate::software_cursor::ensure_macos_hardware_cursor_hidden(_window_entity);
            }
        } else if sw_active {
            crate::software_cursor::ensure_macos_hardware_cursor_hidden(_window_entity);
        } else {
            crate::software_cursor::restore_macos_hardware_cursor(_window_entity);
        }
        if sim.transition || sim.paused {
            cursor.grab_mode = if sim.paused {
                CursorGrabMode::None
            } else {
                CursorGrabMode::Locked
            };
            #[cfg(target_os = "macos")]
            let prior_visible = cursor.visible;
            cursor.visible = sim.paused && !sw_active;
            #[cfg(target_os = "macos")]
            if sw_active {
                let is_at_top = window.cursor_position().is_none_or(|p| p.y <= 32.0);
                if *_was_at_top && !is_at_top {
                    *_rehide_countdown = 15;
                }
                *_was_at_top = is_at_top;
                if (prior_visible && !cursor.visible) || *_rehide_countdown > 0 {
                    if *_rehide_countdown > 0 {
                        *_rehide_countdown -= 1;
                    }
                    crate::software_cursor::ensure_macos_hardware_cursor_hidden(_window_entity);
                }
            } else {
                *_was_at_top = false;
                *_rehide_countdown = 0;
            }
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
    fn cursor_to_canvas_consistent_across_window_modes_and_scale() {
        let mut window = Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        };
        let viewport = VideoViewport {
            target_resolution: UVec2::new(1280, 720),
            physical_rect: bevy::camera::Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::new(1280, 720),
                depth: 0.0..1.0,
            },
            logical_offset: Vec2::ZERO,
            logical_size: Vec2::new(1280.0, 720.0),
        };

        // Inner client coordinates directly at (100, 200) -> (100, 200)
        let p = cursor_to_canvas(Some(Vec2::new(100.0, 200.0)), Some(&viewport), &window);
        assert_eq!(p, Some(glam::Vec2::new(100.0, 200.0)));

        // Scale factor 2.0 (Retina high-DPI)
        window.resolution.set_scale_factor(2.0);
        let p_retina = cursor_to_canvas(Some(Vec2::new(100.0, 200.0)), Some(&viewport), &window);
        assert_eq!(p_retina, Some(glam::Vec2::new(100.0, 200.0)));
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
        assert_eq!(ui.source.mode(), hl2_ui::console::Mode::Pause);
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
        assert_eq!(ui.source.mode(), hl2_ui::console::Mode::MainMenu);
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
