//! Video mode, window resolution, and display settings management.

use bevy::prelude::*;
use bevy::window::PrimaryMonitor;

/// Applies video configuration to a Bevy Window, honoring display mode and avoiding
/// desynchronization between window presentation resolution and wgpu attachments.
pub(crate) fn apply_video_config_to_window(
    window: &mut bevy::window::Window,
    cfg: &hl2_ui::config::VideoSettings,
    _monitors: &Query<&bevy::window::Monitor, With<PrimaryMonitor>>,
) {
    let new_mode = if cfg.borderless {
        bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
    } else {
        bevy::window::WindowMode::Windowed
    };

    let mode_changed = window.mode != new_mode;
    if mode_changed {
        window.mode = new_mode;
    }

    // Only set resolution when in windowed mode and not transitioning between modes.
    // Setting resolution in fullscreen or during mode switch causes wgpu render attachment size mismatches.
    if !mode_changed && window.mode == bevy::window::WindowMode::Windowed && !cfg.borderless {
        let current_logical_w = window.resolution.width().round() as u32;
        let current_logical_h = window.resolution.height().round() as u32;
        if current_logical_w != cfg.width || current_logical_h != cfg.height {
            window.resolution.set(cfg.width as f32, cfg.height as f32);
        }
    }
}

/// Source horizontal 4:3 field of view (degrees) to Bevy's vertical field of view.
pub(crate) fn vertical_fov(degrees: f32) -> f32 {
    2. * ((degrees.to_radians() / 2.).tan() / (4. / 3.)).atan()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::{PrimaryWindow, WindowMode, WindowResolution};

    #[test]
    fn applies_video_config_safely() {
        let mut app = App::new();
        app.world_mut().spawn((
            bevy::window::Monitor {
                name: Some("Test".into()),
                video_modes: vec![bevy::window::VideoMode {
                    physical_size: UVec2::new(1280, 720),
                    bit_depth: 32,
                    refresh_rate_millihertz: 60000,
                }],
                physical_position: IVec2::ZERO,
                physical_width: 1920,
                physical_height: 1080,
                refresh_rate_millihertz: Some(60000),
                scale_factor: 1.0,
            },
            PrimaryMonitor,
        ));
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1920, 1080),
                mode: WindowMode::Windowed,
                ..default()
            },
            PrimaryWindow,
        ));

        let cfg = hl2_ui::config::VideoSettings {
            width: 1280,
            height: 720,
            borderless: true,
            fov: 75.0,
            hdr: hl2_ui::config::HdrMode::Full,
        };

        app.world_mut()
            .run_system_once(
                move |mut windows: Query<&mut Window, With<PrimaryWindow>>,
                      monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>| {
                    let mut window = windows.single_mut().unwrap();
                    apply_video_config_to_window(&mut window, &cfg, &monitors);
                },
            )
            .unwrap();

        let window = app.world_mut().query::<&Window>().single(app.world());
        assert!(matches!(
            window.unwrap().mode,
            WindowMode::BorderlessFullscreen(_)
        ));
    }

    #[test]
    fn changes_resolution_in_windowed_mode() {
        let mut app = App::new();
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1920, 1080),
                mode: WindowMode::Windowed,
                ..default()
            },
            PrimaryWindow,
        ));

        let cfg = hl2_ui::config::VideoSettings {
            width: 1280,
            height: 720,
            borderless: false,
            fov: 75.0,
            hdr: hl2_ui::config::HdrMode::Full,
        };

        app.world_mut()
            .run_system_once(
                move |mut windows: Query<&mut Window, With<PrimaryWindow>>,
                      monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>| {
                    let mut window = windows.single_mut().unwrap();
                    apply_video_config_to_window(&mut window, &cfg, &monitors);
                },
            )
            .unwrap();

        let window = app
            .world_mut()
            .query::<&Window>()
            .single(app.world())
            .unwrap();
        assert_eq!(window.mode, WindowMode::Windowed);
        assert_eq!(window.width() as u32, 1280);
        assert_eq!(window.height() as u32, 720);
    }

    #[test]
    fn mode_transition_defers_resolution_change() {
        let mut app = App::new();
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1920, 1080),
                mode: WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current),
                ..default()
            },
            PrimaryWindow,
        ));

        let cfg = hl2_ui::config::VideoSettings {
            width: 1280,
            height: 720,
            borderless: false,
            fov: 75.0,
            hdr: hl2_ui::config::HdrMode::Full,
        };

        // First apply: transitions mode from BorderlessFullscreen to Windowed.
        // Resolution must NOT be mutated in the same frame to prevent wgpu attachment mismatch.
        app.world_mut()
            .run_system_once(
                |mut windows: Query<&mut Window, With<PrimaryWindow>>,
                 monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>| {
                    let mut window = windows.single_mut().unwrap();
                    let cfg = hl2_ui::config::VideoSettings {
                        width: 1280,
                        height: 720,
                        borderless: false,
                        fov: 75.0,
                        hdr: hl2_ui::config::HdrMode::Full,
                    };
                    apply_video_config_to_window(&mut window, &cfg, &monitors);
                },
            )
            .unwrap();

        let window = app
            .world_mut()
            .query::<&Window>()
            .single(app.world())
            .unwrap();
        assert_eq!(window.mode, WindowMode::Windowed);
        // Resolution preserved on transition frame
        assert_eq!(window.width() as u32, 1920);
        assert_eq!(window.height() as u32, 1080);

        // Subsequent frame while already Windowed applies resolution
        app.world_mut()
            .run_system_once(
                move |mut windows: Query<&mut Window, With<PrimaryWindow>>,
                      monitors: Query<&bevy::window::Monitor, With<PrimaryMonitor>>| {
                    let mut window = windows.single_mut().unwrap();
                    apply_video_config_to_window(&mut window, &cfg, &monitors);
                },
            )
            .unwrap();

        let window = app
            .world_mut()
            .query::<&Window>()
            .single(app.world())
            .unwrap();
        assert_eq!(window.width() as u32, 1280);
        assert_eq!(window.height() as u32, 720);
    }
}
