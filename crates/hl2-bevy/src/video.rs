//! Video mode, window resolution, and display settings management.

use bevy::prelude::*;
use bevy::window::PrimaryMonitor;

/// Applies video configuration to a Bevy Window, honoring display mode and avoiding
/// desynchronization between window presentation resolution and wgpu attachments.
pub(crate) fn apply_video_config_to_window(
    window: &mut bevy::window::Window,
    cfg: &hl2_ui::config::VideoSettings,
    monitors: &Query<&bevy::window::Monitor, With<PrimaryMonitor>>,
) {
    let target_size = bevy::math::UVec2::new(cfg.width, cfg.height);
    let new_mode = if cfg.borderless {
        let modes = monitors
            .iter()
            .flat_map(|m| m.video_modes.iter())
            .filter(|v| v.physical_size == target_size);
        let best = modes
            .max_by_key(|v| (v.refresh_rate_millihertz, v.bit_depth))
            .copied();
        bevy::window::WindowMode::Fullscreen(
            bevy::window::MonitorSelection::Current,
            best.map_or(
                bevy::window::VideoModeSelection::Current,
                bevy::window::VideoModeSelection::Specific,
            ),
        )
    } else {
        bevy::window::WindowMode::Windowed
    };

    if window.mode != new_mode {
        window.mode = new_mode;
    }
    if !cfg.borderless && window.resolution.physical_size() != target_size {
        window.resolution.set(cfg.width as f32, cfg.height as f32);
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
        assert!(matches!(window.unwrap().mode, WindowMode::Fullscreen(_, _)));
    }
}
