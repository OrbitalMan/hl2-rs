//! Video mode, window resolution, DPI scaling, and display management.

use bevy::prelude::*;
use bevy::window::{Monitor, MonitorSelection, PrimaryMonitor, Window, WindowMode};

pub mod display {
    use super::*;

    /// Metrics and bounds for the primary display.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct DisplayMetrics {
        pub physical_width: u32,
        pub physical_height: u32,
        pub scale_factor: f32,
    }

    impl DisplayMetrics {
        /// Queries display metrics from the primary monitor, if available.
        pub fn from_monitors(monitors: &Query<&Monitor, With<PrimaryMonitor>>) -> Option<Self> {
            monitors.iter().next().map(|m| Self {
                physical_width: m.physical_width,
                physical_height: m.physical_height,
                scale_factor: m.scale_factor as f32,
            })
        }

        /// Detects if a window currently in `WindowMode::Windowed` is actually occupying the full
        /// display (such as macOS native workspace fullscreen via the green zoom button).
        ///
        /// When in workspace fullscreen, AppKit ignores `request_inner_size` resize requests, so
        /// modifying `window.resolution` causes immediate wgpu attachment extent desync.
        pub fn is_workspace_fullscreen(&self, window: &Window) -> bool {
            window.mode == WindowMode::Windowed
                && window.resolution.physical_width() == self.physical_width
                && window.resolution.physical_height() >= self.physical_height.saturating_sub(100)
        }
    }
}

pub mod resolution {
    use super::display::DisplayMetrics;
    use super::*;

    /// Planned resolution and client dimensions for a window.
    ///
    /// Honors the DPI formula:
    /// Physical Resolution = Logical Size * Scale Factor
    /// Logical Size = Physical Resolution / Scale Factor
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ResolutionPlan {
        pub physical_size: UVec2,
        pub logical_size: Vec2,
        pub scale_factor: f32,
    }

    impl ResolutionPlan {
        /// Computes the resolution plan for a requested target resolution.
        ///
        /// Clamps physical dimensions to display limits so the OS windowing system never clamps
        /// the window smaller than Bevy expects.
        pub fn compute(
            cfg_width: u32,
            cfg_height: u32,
            window: &Window,
            display: Option<&DisplayMetrics>,
        ) -> Self {
            let scale = window.scale_factor().max(1.0);
            let max_w = display.map_or(u32::MAX, |d| d.physical_width);
            let max_h = display.map_or(u32::MAX, |d| d.physical_height);

            let target_w = cfg_width.min(max_w);
            let target_h = cfg_height.min(max_h);

            let logical_w = target_w as f32 / scale;
            let logical_h = target_h as f32 / scale;

            Self {
                physical_size: UVec2::new(target_w, target_h),
                logical_size: Vec2::new(logical_w, logical_h),
                scale_factor: scale,
            }
        }

        /// Returns true if the window's physical resolution already matches this plan.
        pub fn matches_window(&self, window: &Window) -> bool {
            window.resolution.physical_width() == self.physical_size.x
                && window.resolution.physical_height() == self.physical_size.y
        }

        /// Applies this resolution plan to the window.
        pub fn apply_to_window(&self, window: &mut Window) {
            window
                .resolution
                .set(self.logical_size.x, self.logical_size.y);
        }
    }
}

pub mod mode {
    use super::*;

    /// Determines the desired window mode for a video configuration.
    pub fn desired_mode(borderless: bool) -> WindowMode {
        if borderless {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        }
    }
}

/// Applies video configuration to a Bevy Window, honoring display mode and avoiding
/// desynchronization between window presentation resolution and wgpu attachments.
pub(crate) fn apply_video_config_to_window(
    window: &mut Window,
    cfg: &hl2_ui::config::VideoSettings,
    monitors: &Query<&Monitor, With<PrimaryMonitor>>,
) {
    let target_mode = mode::desired_mode(cfg.borderless);
    let mode_changed = window.mode != target_mode;
    if mode_changed {
        window.mode = target_mode;
    }

    // Only set resolution when in windowed mode and not transitioning between modes.
    // Setting resolution in fullscreen or during mode switch causes wgpu render attachment size mismatches.
    if !mode_changed && window.mode == WindowMode::Windowed && !cfg.borderless {
        let display = display::DisplayMetrics::from_monitors(monitors);

        // Guard against resizing when in macOS workspace fullscreen (green button).
        let in_workspace_fs = display
            .as_ref()
            .is_some_and(|d| d.is_workspace_fullscreen(window));
        if !in_workspace_fs {
            let plan = resolution::ResolutionPlan::compute(
                cfg.width,
                cfg.height,
                window,
                display.as_ref(),
            );
            if !plan.matches_window(window) {
                plan.apply_to_window(window);
            }
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
            Monitor {
                name: Some("Test".into()),
                video_modes: vec![],
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
                      monitors: Query<&Monitor, With<PrimaryMonitor>>| {
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
                      monitors: Query<&Monitor, With<PrimaryMonitor>>| {
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
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
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
                 monitors: Query<&Monitor, With<PrimaryMonitor>>| {
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
                      monitors: Query<&Monitor, With<PrimaryMonitor>>| {
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

    #[test]
    fn changes_resolution_with_scale_factor() {
        let mut app = App::new();
        let mut res = WindowResolution::new(1280, 720);
        res.set_scale_factor(2.0);

        app.world_mut().spawn((
            Window {
                resolution: res,
                mode: WindowMode::Windowed,
                ..default()
            },
            PrimaryWindow,
        ));

        let cfg = hl2_ui::config::VideoSettings {
            width: 1920,
            height: 1080,
            borderless: false,
            fov: 75.0,
            hdr: hl2_ui::config::HdrMode::Full,
        };

        app.world_mut()
            .run_system_once(
                move |mut windows: Query<&mut Window, With<PrimaryWindow>>,
                      monitors: Query<&Monitor, With<PrimaryMonitor>>| {
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
        // Physical framebuffer matches requested resolution
        assert_eq!(window.resolution.physical_width(), 1920);
        assert_eq!(window.resolution.physical_height(), 1080);
        // Logical window size scaled by scale factor 2.0 (fits comfortably on screen)
        assert_eq!(window.width() as u32, 960);
        assert_eq!(window.height() as u32, 540);
    }

    #[test]
    fn workspace_fullscreen_protected_from_resolution_desync() {
        let mut app = App::new();
        app.world_mut().spawn((
            Monitor {
                name: Some("Retina Display".into()),
                video_modes: vec![],
                physical_position: IVec2::ZERO,
                physical_width: 3456,
                physical_height: 2168,
                refresh_rate_millihertz: Some(120000),
                scale_factor: 2.0,
            },
            PrimaryMonitor,
        ));
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(3456, 2168),
                mode: WindowMode::Windowed,
                ..default()
            },
            PrimaryWindow,
        ));

        let cfg = hl2_ui::config::VideoSettings {
            width: 1920,
            height: 1080,
            borderless: false,
            fov: 75.0,
            hdr: hl2_ui::config::HdrMode::Full,
        };

        app.world_mut()
            .run_system_once(
                move |mut windows: Query<&mut Window, With<PrimaryWindow>>,
                      monitors: Query<&Monitor, With<PrimaryMonitor>>| {
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
        // Preserved to avoid desync with macOS workspace fullscreen
        assert_eq!(window.resolution.physical_width(), 3456);
        assert_eq!(window.resolution.physical_height(), 2168);
    }
}
