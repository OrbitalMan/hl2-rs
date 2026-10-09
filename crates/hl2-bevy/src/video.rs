//! Video mode, renderer viewport scaling, letterboxing/pillarboxing, and display management.

use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::{Monitor, MonitorSelection, PrimaryMonitor, PrimaryWindow, Window, WindowMode};

pub mod display {
    use super::*;

    /// Metrics and bounds for the primary display.
    #[derive(Clone, Copy, Debug, PartialEq)]
    #[allow(dead_code)]
    pub struct DisplayMetrics {
        pub physical_width: u32,
        pub physical_height: u32,
        pub scale_factor: f32,
    }

    #[allow(dead_code)]
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
        pub fn is_workspace_fullscreen(&self, window: &Window) -> bool {
            window.mode == WindowMode::Windowed
                && window.resolution.physical_width() == self.physical_width
                && window.resolution.physical_height() >= self.physical_height.saturating_sub(100)
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

/// Renderer output and viewport configuration for the presentation window frame.
///
/// The configured video resolution is the target renderer output resolution, not
/// the window's OS frame size. When the aspect ratio of the viewport does not
/// match the aspect ratio of the window frame, black stripes (letterbox or pillarbox)
/// are placed on the sides to preserve the exact aspect ratio without distortion.
#[derive(Resource, Clone, Debug)]
pub struct VideoViewport {
    /// Target logical resolution configured in video settings (e.g. 1920x1080 or 1024x768).
    pub target_resolution: UVec2,
    /// Physical viewport rect passed to Bevy camera viewports.
    pub physical_rect: Viewport,
    /// Top-left offset of the viewport in logical window coordinates.
    pub logical_offset: Vec2,
    /// Viewport size in logical window coordinates.
    pub logical_size: Vec2,
}

impl PartialEq for VideoViewport {
    fn eq(&self, other: &Self) -> bool {
        self.target_resolution == other.target_resolution
            && self.physical_rect.physical_position == other.physical_rect.physical_position
            && self.physical_rect.physical_size == other.physical_rect.physical_size
            && self.logical_offset == other.logical_offset
            && self.logical_size == other.logical_size
    }
}

impl Default for VideoViewport {
    fn default() -> Self {
        Self {
            target_resolution: UVec2::new(1280, 720),
            physical_rect: Viewport {
                physical_position: UVec2::ZERO,
                physical_size: UVec2::new(1280, 720),
                depth: 0.0..1.0,
            },
            logical_offset: Vec2::ZERO,
            logical_size: Vec2::new(1280.0, 720.0),
        }
    }
}

impl VideoViewport {
    /// Computes the letterbox/pillarbox viewport to fit target resolution inside a window.
    pub fn compute(target_w: u32, target_h: u32, window: &Window) -> Self {
        let win_w = window.resolution.physical_width().max(1);
        let win_h = window.resolution.physical_height().max(1);
        let scale = window.scale_factor().max(1.0);

        let bounds = hl2_ui::viewport::ViewportBounds::compute(
            glam::UVec2::new(target_w, target_h),
            glam::UVec2::new(win_w, win_h),
            scale,
        );

        let physical_rect = Viewport {
            physical_position: UVec2::new(bounds.physical_offset.x, bounds.physical_offset.y),
            physical_size: UVec2::new(bounds.physical_size.x, bounds.physical_size.y),
            depth: 0.0..1.0,
        };

        Self {
            target_resolution: UVec2::new(bounds.target_resolution.x, bounds.target_resolution.y),
            physical_rect,
            logical_offset: Vec2::new(bounds.logical_offset.x, bounds.logical_offset.y),
            logical_size: Vec2::new(bounds.logical_size.x, bounds.logical_size.y),
        }
    }

    /// Returns the portable viewport bounds.
    pub fn bounds(&self) -> hl2_ui::viewport::ViewportBounds {
        hl2_ui::viewport::ViewportBounds {
            target_resolution: glam::UVec2::new(self.target_resolution.x, self.target_resolution.y),
            physical_offset: glam::UVec2::new(
                self.physical_rect.physical_position.x,
                self.physical_rect.physical_position.y,
            ),
            physical_size: glam::UVec2::new(
                self.physical_rect.physical_size.x,
                self.physical_rect.physical_size.y,
            ),
            logical_offset: glam::Vec2::new(self.logical_offset.x, self.logical_offset.y),
            logical_size: glam::Vec2::new(self.logical_size.x, self.logical_size.y),
        }
    }

    /// Whether black stripes are present (aspect ratio mismatch).
    #[allow(dead_code)]
    pub fn has_black_stripes(&self) -> bool {
        self.bounds().has_black_stripes()
    }

    /// True if vertical black stripes (pillarboxing) on left/right sides.
    #[allow(dead_code)]
    pub fn is_vertical_stripes(&self) -> bool {
        self.bounds().is_vertical_stripes()
    }

    /// True if horizontal black stripes (letterboxing) on top/bottom sides.
    #[allow(dead_code)]
    pub fn is_horizontal_stripes(&self) -> bool {
        self.bounds().is_horizontal_stripes()
    }
}

/// Applies video configuration to a Bevy Window, honoring display mode.
///
/// Note: Window size is NOT modified here; video resolution is the renderer
/// viewport output resolution, which is placed into the window/fullscreen frame
/// with letterboxing/pillarboxing.
pub(crate) fn apply_video_config_to_window(
    window: &mut Window,
    cfg: &hl2_ui::config::VideoSettings,
    _monitors: &Query<&Monitor, With<PrimaryMonitor>>,
) {
    let target_mode = mode::desired_mode(cfg.borderless);
    if window.mode != target_mode {
        window.mode = target_mode;
    }
}

/// Updates the video viewport and synchronizes it across all window-rendering cameras.
pub(crate) fn update_viewports(
    mut video_viewport: ResMut<VideoViewport>,
    console: Option<Res<crate::console::Console>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, Without<crate::monitors::MonitorCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let (target_w, target_h) = if let Some(console) = &console {
        (
            console.source.config.video.width,
            console.source.config.video.height,
        )
    } else {
        (
            video_viewport.target_resolution.x,
            video_viewport.target_resolution.y,
        )
    };

    let computed = VideoViewport::compute(target_w, target_h, window);
    if *video_viewport != computed {
        *video_viewport = computed;
    }

    for mut camera in &mut cameras {
        let needs_update = match &camera.viewport {
            Some(existing) => {
                existing.physical_position != video_viewport.physical_rect.physical_position
                    || existing.physical_size != video_viewport.physical_rect.physical_size
            }
            None => true,
        };
        if needs_update {
            camera.viewport = Some(video_viewport.physical_rect.clone());
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
    fn applies_video_config_mode_safely() {
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
    fn resolution_does_not_mutate_window_frame_size() {
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
            width: 1024,
            height: 768,
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
        // Window frame size preserved (not mutated into 1024x768)
        assert_eq!(window.resolution.physical_width(), 1920);
        assert_eq!(window.resolution.physical_height(), 1080);
    }

    #[test]
    fn pillarbox_vertical_stripes_for_narrow_aspect_on_wide_window() {
        let window = Window {
            resolution: WindowResolution::new(1920, 1080),
            ..default()
        };
        // 4:3 target on 16:9 window: 1024x768 target in 1920x1080 window
        let vp = VideoViewport::compute(1024, 768, &window);
        assert!(vp.has_black_stripes());
        assert!(vp.is_vertical_stripes());
        assert!(!vp.is_horizontal_stripes());
        // 1080 * (4/3) = 1440 width. Remaining 480 split as 240 left & right.
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(1440, 1080));
        assert_eq!(vp.physical_rect.physical_position, UVec2::new(240, 0));
        assert_eq!(vp.logical_size, Vec2::new(1440.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::new(240.0, 0.0));
    }

    #[test]
    fn letterbox_horizontal_stripes_for_wide_aspect_on_narrow_window() {
        let window = Window {
            resolution: WindowResolution::new(1024, 768),
            ..default()
        };
        // 16:9 target on 4:3 window: 1920x1080 target in 1024x768 window
        let vp = VideoViewport::compute(1920, 1080, &window);
        assert!(vp.has_black_stripes());
        assert!(!vp.is_vertical_stripes());
        assert!(vp.is_horizontal_stripes());
        // 1024 / (16/9) = 576 height. Remaining 192 split as 96 top & bottom.
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(1024, 576));
        assert_eq!(vp.physical_rect.physical_position, UVec2::new(0, 96));
        assert_eq!(vp.logical_size, Vec2::new(1024.0, 576.0));
        assert_eq!(vp.logical_offset, Vec2::new(0.0, 96.0));
    }

    #[test]
    fn exact_aspect_ratio_fills_entire_frame() {
        let window = Window {
            resolution: WindowResolution::new(1920, 1080),
            ..default()
        };
        // 16:9 target on 16:9 window: 1280x720 target in 1920x1080 window
        let vp = VideoViewport::compute(1280, 720, &window);
        assert!(!vp.has_black_stripes());
        assert!(!vp.is_vertical_stripes());
        assert!(!vp.is_horizontal_stripes());
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(1920, 1080));
        assert_eq!(vp.physical_rect.physical_position, UVec2::ZERO);
        assert_eq!(vp.logical_size, Vec2::new(1920.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::ZERO);
    }

    #[test]
    fn viewport_with_scale_factor_calculates_correct_logical_offset() {
        let mut res = WindowResolution::new(3840, 2160);
        res.set_scale_factor(2.0);
        let window = Window {
            resolution: res,
            ..default()
        };
        // 4:3 target on 16:9 4K Retina display
        let vp = VideoViewport::compute(1024, 768, &window);
        // Physical: 2160 * (4/3) = 2880 width. Offset = (3840 - 2880) / 2 = 480.
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(2880, 2160));
        assert_eq!(vp.physical_rect.physical_position, UVec2::new(480, 0));
        // Logical: divided by scale factor 2.0
        assert_eq!(vp.logical_size, Vec2::new(1440.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::new(240.0, 0.0));
    }

    #[test]
    fn update_viewports_system_synchronizes_cameras() {
        let mut app = App::new();
        app.init_resource::<VideoViewport>();
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1920, 1080),
                ..default()
            },
            PrimaryWindow,
        ));
        let scene_camera = app
            .world_mut()
            .spawn(Camera {
                order: 0,
                ..default()
            })
            .id();
        let monitor_camera = app
            .world_mut()
            .spawn((
                crate::monitors::MonitorCamera,
                Camera {
                    order: -100,
                    ..default()
                },
            ))
            .id();

        app.world_mut().run_system_once(update_viewports).unwrap();

        let camera = app.world().get::<Camera>(scene_camera).unwrap();
        assert!(camera.viewport.is_some());
        assert_eq!(
            camera.viewport.as_ref().unwrap().physical_size,
            UVec2::new(1920, 1080)
        );

        let mon = app.world().get::<Camera>(monitor_camera).unwrap();
        // Monitor camera renders off-screen to its own target, viewport untouched
        assert!(mon.viewport.is_none());
    }
}
