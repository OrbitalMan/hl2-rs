//! Video modes, the game frame and its presentation in the window.
//!
//! The configured video resolution is the resolution of the *game frame*: the sky, world,
//! viewmodel and HUD/menu cameras all render into one offscreen image of exactly that size.
//! A presentation camera then draws the frame into the window with linear filtering, scaled
//! to fit and centred (letterbox/pillarbox when the aspect ratios differ). The window size,
//! the display's native resolution and its scale factor therefore never change the rendering
//! resolution: full screen at 1280x720 renders 1280x720 pixels and upscales them to the
//! display, and a macOS window put in full screen with the green title-bar button behaves
//! the same way.
//!
//! In windowed mode, applying a resolution also sizes the window's client area to it (in
//! logical points, clamped to the monitor), as Source sizes a windowed game to its mode.

use bevy::camera::{RenderTarget, Viewport, visibility::RenderLayers};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::{
    Monitor, MonitorSelection, PrimaryMonitor, PrimaryWindow, Window, WindowMode, WindowPosition,
};

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
        /// Metrics of a monitor.
        pub fn of(monitor: &Monitor) -> Self {
            Self {
                physical_width: monitor.physical_width,
                physical_height: monitor.physical_height,
                scale_factor: monitor.scale_factor as f32,
            }
        }

        /// Queries display metrics from the primary monitor, if available.
        #[allow(dead_code)]
        pub fn from_monitors(monitors: &Query<&Monitor, With<PrimaryMonitor>>) -> Option<Self> {
            monitors.iter().next().map(Self::of)
        }

        /// The monitor's size in logical points.
        pub fn logical_size(&self) -> Vec2 {
            Vec2::new(self.physical_width as f32, self.physical_height as f32)
                / self.scale_factor.max(1.0)
        }

        /// Detects if a window currently in `WindowMode::Windowed` is actually occupying the full
        /// display (such as macOS native workspace fullscreen via the green zoom button).
        pub fn is_workspace_fullscreen(&self, window: &Window) -> bool {
            window.mode == WindowMode::Windowed
                && window.resolution.physical_width() == self.physical_width
                && window.resolution.physical_height() >= self.physical_height.saturating_sub(100)
        }
    }

    /// Resolutions offered on the Video page: the primary monitor's reported video modes,
    /// filtered and sorted by [`hl2_ui::gameui::display::usable_modes`]. Falls back to the
    /// current window size when the platform reports none.
    pub fn modes(
        monitors: &Query<&Monitor, With<PrimaryMonitor>>,
        window: &Window,
    ) -> Vec<hl2_ui::gameui::display::Resolution> {
        use hl2_ui::gameui::display::{Resolution, usable_modes};
        let monitor = monitors.iter().next();
        let reported = monitor
            .into_iter()
            .flat_map(|m| m.video_modes.iter())
            .map(|mode| Resolution::new(mode.physical_size.x, mode.physical_size.y));
        let size = monitor.map(|m| Resolution::new(m.physical_width, m.physical_height));
        let current = Resolution::new(
            window.resolution.physical_width(),
            window.resolution.physical_height(),
        );
        usable_modes(reported, size, current)
    }
}

pub mod mode {
    use super::*;
    use hl2_ui::config::DisplayMode;

    /// The Bevy window mode for a configured display mode on this platform.
    ///
    /// Full screen keeps the desktop's display mode: the game frame renders at the configured
    /// resolution and is scaled to the display. "Windowed (No Border)" is a borderless window
    /// covering the current monitor (Windows and Linux); macOS has no such choice and resolves a
    /// stored value to full screen.
    pub fn desired_mode(mode: DisplayMode) -> WindowMode {
        on_monitor(mode, MonitorSelection::Current)
    }

    /// [`desired_mode`] for window creation, where no current monitor exists yet.
    pub fn startup_mode(mode: DisplayMode) -> WindowMode {
        on_monitor(mode, MonitorSelection::Primary)
    }

    fn on_monitor(mode: DisplayMode, monitor: MonitorSelection) -> WindowMode {
        match mode.effective() {
            DisplayMode::Fullscreen => fullscreen(monitor),
            DisplayMode::Windowed => WindowMode::Windowed,
            DisplayMode::NoBorder => WindowMode::BorderlessFullscreen(monitor),
        }
    }

    /// macOS full screen is a full-screen window over the desktop's display mode. An exclusive
    /// mode change there can pick a non-scaled (1x) mode of the panel, which shrinks the
    /// desktop scaling for the whole session (a tiny mouse cursor); the game frame is scaled to
    /// the display instead.
    #[cfg(target_os = "macos")]
    fn fullscreen(monitor: MonitorSelection) -> WindowMode {
        WindowMode::BorderlessFullscreen(monitor)
    }

    #[cfg(not(target_os = "macos"))]
    fn fullscreen(monitor: MonitorSelection) -> WindowMode {
        use bevy::window::VideoModeSelection;
        WindowMode::Fullscreen(monitor, VideoModeSelection::Current)
    }
}

/// Render layer of the presentation pass (only the frame quad and its camera).
pub const PRESENT_LAYER: usize = 31;
/// The presentation camera draws after every game-frame camera.
const PRESENT_ORDER: isize = 100;
/// Frames during which a windowed-mode resize is re-requested until the OS applies it (window
/// mode transitions, such as leaving full screen, finish asynchronously).
const WINDOW_RESIZE_FRAMES: u32 = 120;
/// Room kept around a windowed window that would not fit the monitor (title bar, menu bar,
/// task bar), in logical points.
const WINDOW_MARGIN: Vec2 = Vec2::new(32.0, 96.0);

/// The offscreen game frame: every scene, viewmodel and HUD camera renders into it.
#[derive(Resource, Clone, Debug)]
pub struct GameFrame {
    pub image: Handle<Image>,
    /// Configured video resolution, in frame pixels.
    pub resolution: UVec2,
    /// Remaining frames to size a windowed-mode window to `resolution`.
    pub pending_window: Option<u32>,
}

/// The camera presenting the game frame in the window.
#[derive(Component)]
pub struct PresentCamera;

/// The quad showing the game frame.
#[derive(Component)]
pub struct PresentQuad;

/// A render-target image for the game frame, linearly filtered when scaled.
pub fn frame_image(resolution: UVec2) -> Image {
    let size = resolution.max(UVec2::ONE);
    let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

/// Creates the game frame and its presentation. `windowed` sizes the window to the frame
/// (clamped to the monitor) once the monitor is known.
pub(crate) fn install_frame(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    resolution: UVec2,
    windowed: bool,
) {
    let resolution = resolution.max(UVec2::ONE);
    let image = images.add(frame_image(resolution));
    commands.insert_resource(GameFrame {
        image: image.clone(),
        resolution,
        pending_window: windowed.then_some(WINDOW_RESIZE_FRAMES),
    });
    commands.spawn((
        PresentCamera,
        Camera2d,
        Camera {
            order: PRESENT_ORDER,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
        RenderLayers::layer(PRESENT_LAYER),
    ));
    commands.spawn((
        PresentQuad,
        Sprite {
            image,
            custom_size: Some(resolution.as_vec2()),
            ..default()
        },
        Transform::IDENTITY,
        RenderLayers::layer(PRESENT_LAYER),
    ));
}

/// Placement of the presented game frame in the window.
///
/// `target_resolution` is the game-frame resolution (what the HUD/menu canvas is laid out in);
/// the rects describe where the scaled frame appears inside the window, with black stripes
/// (letterbox or pillarbox) when the aspect ratios differ.
#[derive(Resource, Clone, Debug)]
pub struct VideoViewport {
    /// Game-frame resolution configured in video settings (e.g. 1920x1080 or 1024x768).
    pub target_resolution: UVec2,
    /// Presented frame rect in physical window pixels.
    pub physical_rect: Viewport,
    /// Top-left offset of the presented frame in logical window coordinates.
    pub logical_offset: Vec2,
    /// Presented frame size in logical window coordinates.
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
    /// Computes where a frame of the target resolution is presented inside a window.
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

/// Applies applied video settings: the window mode at once, the game-frame resolution, and in
/// windowed mode a window resize carried out by [`resize_window`].
pub(crate) fn apply_video_config_to_window(
    window: &mut Window,
    cfg: &hl2_ui::config::VideoSettings,
    frame: Option<&mut GameFrame>,
) {
    let target_mode = mode::desired_mode(cfg.display_mode);
    if window.mode != target_mode {
        window.mode = target_mode;
    }
    if let Some(frame) = frame {
        frame.resolution = UVec2::new(cfg.width, cfg.height).max(UVec2::ONE);
        frame.pending_window =
            (target_mode == WindowMode::Windowed).then_some(WINDOW_RESIZE_FRAMES);
    }
}

/// What a pending windowed-mode resize should do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowResize {
    /// The window left windowed mode; drop the request.
    Cancel,
    /// The window fills the display in OS full screen (macOS green button); keep the request
    /// for when it returns to a normal window, and leave it alone meanwhile.
    Wait,
    /// The window already has the requested size.
    Done,
    /// Request this logical client size.
    Resize(Vec2),
}

/// Decides a windowed-mode resize to the logical size of `resolution`, clamped to the monitor.
pub fn window_resize(
    window: &Window,
    monitor: Option<&Monitor>,
    resolution: UVec2,
) -> WindowResize {
    if window.mode != WindowMode::Windowed {
        return WindowResize::Cancel;
    }
    let metrics = monitor.map(display::DisplayMetrics::of);
    if metrics.is_some_and(|m| m.is_workspace_fullscreen(window)) {
        return WindowResize::Wait;
    }
    let available = metrics.map(|m| m.logical_size());
    let mut size = hl2_ui::viewport::windowed_size(
        glam::UVec2::new(resolution.x, resolution.y),
        available.map(|a| glam::Vec2::new(a.x, a.y)),
    );
    if let Some(available) = available
        && (size.x >= available.x || size.y >= available.y)
    {
        let room = (available - WINDOW_MARGIN).max(Vec2::ONE);
        size = hl2_ui::viewport::windowed_size(
            glam::UVec2::new(resolution.x, resolution.y),
            Some(glam::Vec2::new(room.x, room.y)),
        );
    }
    let size = Vec2::new(size.x, size.y);
    let current = Vec2::new(window.resolution.width(), window.resolution.height());
    if (current - size).abs().max_element() < 1.0 {
        WindowResize::Done
    } else {
        WindowResize::Resize(size)
    }
}

/// Carries out a pending windowed-mode resize, re-requesting it while the OS finishes a
/// window-mode transition. A window in OS full screen is left alone.
pub(crate) fn resize_window(
    frame: Option<ResMut<GameFrame>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor, With<PrimaryMonitor>>,
) {
    let Some(mut frame) = frame else {
        return;
    };
    let Some(frames) = frame.pending_window else {
        return;
    };
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let monitor = monitors.iter().next();
    if monitor.is_none() && frames > 1 {
        // The monitor appears a few frames after window creation; clamp against it.
        frame.pending_window = Some(frames - 1);
        return;
    }
    match window_resize(&window, monitor, frame.resolution) {
        WindowResize::Cancel | WindowResize::Done => frame.pending_window = None,
        WindowResize::Wait => {}
        WindowResize::Resize(size) => {
            window.resolution.set(size.x, size.y);
            window.position = WindowPosition::Centered(MonitorSelection::Current);
            frame.pending_window = frames.checked_sub(1).filter(|left| *left > 0);
        }
    }
}

type FrameCameras<'w, 's> = Query<
    'w,
    's,
    (&'static mut Camera, &'static mut RenderTarget),
    (
        Without<crate::monitors::MonitorCamera>,
        Without<PresentCamera>,
    ),
>;

/// Keeps the game frame at the configured resolution, routes every game camera into it and
/// places the presented frame in the window.
pub(crate) fn present_frame(
    frame: Option<Res<GameFrame>>,
    mut images: ResMut<Assets<Image>>,
    mut video_viewport: ResMut<VideoViewport>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: FrameCameras,
    mut quads: Query<&mut Sprite, With<PresentQuad>>,
) {
    let Some(frame) = frame else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    if images.get(&frame.image).map(Image::size) != Some(frame.resolution)
        && let Some(mut image) = images.get_mut(&frame.image)
    {
        *image = frame_image(frame.resolution);
    }
    for (mut camera, mut target) in &mut cameras {
        let routed = matches!(&*target, RenderTarget::Image(t) if t.handle == frame.image);
        if !routed {
            *target = RenderTarget::Image(frame.image.clone().into());
        }
        if camera.viewport.is_some() {
            camera.viewport = None;
        }
    }
    let computed = VideoViewport::compute(frame.resolution.x, frame.resolution.y, window);
    if *video_viewport != computed {
        *video_viewport = computed;
    }
    for mut quad in &mut quads {
        if quad.custom_size != Some(video_viewport.logical_size) {
            quad.custom_size = Some(video_viewport.logical_size);
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

    fn monitor(width: u32, height: u32, scale_factor: f64) -> Monitor {
        Monitor {
            name: Some("Test".into()),
            video_modes: vec![],
            physical_position: IVec2::ZERO,
            physical_width: width,
            physical_height: height,
            refresh_rate_millihertz: Some(60000),
            scale_factor,
        }
    }

    fn window(width: u32, height: u32, scale: f32, mode: WindowMode) -> Window {
        let mut resolution = WindowResolution::new(width, height);
        resolution.set_scale_factor(scale);
        Window {
            resolution,
            mode,
            ..default()
        }
    }

    fn frame(resolution: UVec2) -> GameFrame {
        GameFrame {
            image: Handle::default(),
            resolution,
            pending_window: None,
        }
    }

    #[test]
    fn applies_video_config_mode_safely() {
        let mut window = window(1920, 1080, 1.0, WindowMode::Windowed);
        let cfg = hl2_ui::config::VideoSettings {
            width: 1280,
            height: 720,
            display_mode: hl2_ui::config::DisplayMode::NoBorder,
            ..Default::default()
        };
        let mut frame = frame(UVec2::new(1920, 1080));
        apply_video_config_to_window(&mut window, &cfg, Some(&mut frame));
        if cfg!(target_os = "macos") {
            // No borderless choice on macOS: full screen, kept on the desktop's display mode.
            assert_eq!(
                window.mode,
                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            );
        } else {
            assert!(
                matches!(window.mode, WindowMode::BorderlessFullscreen(_)),
                "{:?}",
                window.mode
            );
        }
        assert_eq!(frame.resolution, UVec2::new(1280, 720));
        assert_eq!(
            frame.pending_window, None,
            "no window resize outside windowed"
        );
        // The OS window frame is not resized for full-screen modes.
        assert_eq!(window.resolution.physical_width(), 1920);
    }

    #[test]
    fn display_modes_map_to_window_modes() {
        use hl2_ui::config::DisplayMode;
        assert_eq!(
            mode::desired_mode(DisplayMode::Windowed),
            WindowMode::Windowed
        );
        let fullscreen = mode::desired_mode(DisplayMode::Fullscreen);
        let no_border = mode::desired_mode(DisplayMode::NoBorder);
        if cfg!(target_os = "macos") {
            assert_eq!(
                fullscreen,
                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            );
            assert_eq!(no_border, fullscreen);
        } else {
            assert!(matches!(
                fullscreen,
                WindowMode::Fullscreen(MonitorSelection::Current, _)
            ));
            assert_eq!(
                no_border,
                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            );
        }
    }

    #[test]
    fn windowed_resolution_requests_a_window_resize() {
        let mut window = window(1920, 1080, 1.0, WindowMode::Windowed);
        let cfg = hl2_ui::config::VideoSettings {
            width: 1024,
            height: 768,
            display_mode: hl2_ui::config::DisplayMode::Windowed,
            ..Default::default()
        };
        let mut frame = frame(UVec2::new(1920, 1080));
        apply_video_config_to_window(&mut window, &cfg, Some(&mut frame));
        assert_eq!(frame.resolution, UVec2::new(1024, 768));
        assert!(frame.pending_window.is_some());
        let monitor = monitor(2560, 1440, 1.0);
        assert_eq!(
            window_resize(&window, Some(&monitor), frame.resolution),
            WindowResize::Resize(Vec2::new(1024.0, 768.0))
        );
    }

    #[test]
    fn windowed_size_is_logical_on_high_dpi_displays() {
        // Retina 3024x1964 at 2x: a 1280x720 window is 1280x720 points (2560x1440 pixels);
        // the 1280x720 frame is upscaled 2x by the presentation pass.
        let window = window(1000, 600, 2.0, WindowMode::Windowed);
        let retina = monitor(3024, 1964, 2.0);
        assert_eq!(
            window_resize(&window, Some(&retina), UVec2::new(1280, 720)),
            WindowResize::Resize(Vec2::new(1280.0, 720.0))
        );
        let sized = self::window(2560, 1440, 2.0, WindowMode::Windowed);
        assert_eq!(
            window_resize(&sized, Some(&retina), UVec2::new(1280, 720)),
            WindowResize::Done
        );
    }

    #[test]
    fn oversized_windowed_resolution_is_clamped_to_the_monitor() {
        // The panel's native 3024x1964 does not fit 1512x982 points with a title bar: the window
        // shrinks uniformly inside the monitor, keeping the frame's aspect ratio.
        let window = window(1000, 600, 2.0, WindowMode::Windowed);
        let retina = monitor(3024, 1964, 2.0);
        let WindowResize::Resize(size) =
            window_resize(&window, Some(&retina), UVec2::new(3024, 1964))
        else {
            panic!("expected a resize");
        };
        assert!(size.x <= 1512.0 - 32.0 && size.y <= 982.0 - 96.0, "{size}");
        assert!((size.x / size.y - 3024.0 / 1964.0).abs() < 0.01, "{size}");
    }

    #[test]
    fn os_full_screen_window_is_left_alone_until_it_returns() {
        // macOS green-button full screen keeps WindowMode::Windowed but fills the display.
        let retina = monitor(3024, 1964, 2.0);
        let window = window(3024, 1964, 2.0, WindowMode::Windowed);
        assert_eq!(
            window_resize(&window, Some(&retina), UVec2::new(1280, 720)),
            WindowResize::Wait
        );
        let full = self::window(
            3024,
            1964,
            2.0,
            WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        );
        assert_eq!(
            window_resize(&full, Some(&retina), UVec2::new(1280, 720)),
            WindowResize::Cancel
        );
    }

    #[test]
    fn pillarbox_vertical_stripes_for_narrow_aspect_on_wide_window() {
        let window = window(1920, 1080, 1.0, WindowMode::Windowed);
        // 4:3 frame presented in a 16:9 window.
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
        let window = window(1024, 768, 1.0, WindowMode::Windowed);
        // 16:9 frame presented in a 4:3 window.
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
        let window = window(1920, 1080, 1.0, WindowMode::Windowed);
        // A 1280x720 frame is upscaled to fill a 1920x1080 window.
        let vp = VideoViewport::compute(1280, 720, &window);
        assert!(!vp.has_black_stripes());
        assert_eq!(vp.target_resolution, UVec2::new(1280, 720));
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(1920, 1080));
        assert_eq!(vp.physical_rect.physical_position, UVec2::ZERO);
        assert_eq!(vp.logical_size, Vec2::new(1920.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::ZERO);
    }

    #[test]
    fn viewport_with_scale_factor_calculates_correct_logical_offset() {
        let window = window(3840, 2160, 2.0, WindowMode::Windowed);
        // 4:3 frame on a 16:9 4K Retina display
        let vp = VideoViewport::compute(1024, 768, &window);
        // Physical: 2160 * (4/3) = 2880 width. Offset = (3840 - 2880) / 2 = 480.
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(2880, 2160));
        assert_eq!(vp.physical_rect.physical_position, UVec2::new(480, 0));
        // Logical: divided by scale factor 2.0
        assert_eq!(vp.logical_size, Vec2::new(1440.0, 1080.0));
        assert_eq!(vp.logical_offset, Vec2::new(240.0, 0.0));
    }

    #[test]
    fn game_cameras_render_into_the_frame_at_the_configured_resolution() {
        let mut app = App::new();
        app.init_resource::<VideoViewport>()
            .init_resource::<Assets<Image>>();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(frame_image(UVec2::new(640, 480)));
        app.insert_resource(GameFrame {
            image: image.clone(),
            resolution: UVec2::new(1280, 720),
            pending_window: None,
        });
        // Full screen on a Retina panel.
        app.world_mut().spawn((
            window(
                3024,
                1964,
                2.0,
                WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            ),
            PrimaryWindow,
        ));
        let scene_camera = app
            .world_mut()
            .spawn((
                Camera {
                    viewport: Some(Viewport::default()),
                    ..default()
                },
                RenderTarget::default(),
            ))
            .id();
        let monitor_camera = app
            .world_mut()
            .spawn((
                crate::monitors::MonitorCamera,
                Camera::default(),
                RenderTarget::default(),
            ))
            .id();
        let present_camera = app
            .world_mut()
            .spawn((PresentCamera, Camera::default(), RenderTarget::default()))
            .id();
        let quad = app.world_mut().spawn((PresentQuad, Sprite::default())).id();

        app.world_mut().run_system_once(present_frame).unwrap();

        let size = app
            .world()
            .resource::<Assets<Image>>()
            .get(&image)
            .map(Image::size);
        assert_eq!(
            size,
            Some(UVec2::new(1280, 720)),
            "frame follows the setting"
        );
        let target = app.world().get::<RenderTarget>(scene_camera).unwrap();
        assert!(matches!(target, RenderTarget::Image(t) if t.handle == image));
        assert!(
            app.world()
                .get::<Camera>(scene_camera)
                .unwrap()
                .viewport
                .is_none()
        );
        assert!(matches!(
            app.world().get::<RenderTarget>(monitor_camera).unwrap(),
            RenderTarget::Window(_)
        ));
        assert!(matches!(
            app.world().get::<RenderTarget>(present_camera).unwrap(),
            RenderTarget::Window(_)
        ));
        // 16:9 frame on a 3024x1964 panel: full width, letterboxed, in logical points.
        let vp = app.world().resource::<VideoViewport>();
        assert_eq!(vp.target_resolution, UVec2::new(1280, 720));
        assert_eq!(vp.physical_rect.physical_size, UVec2::new(3024, 1701));
        let sprite = app.world().get::<Sprite>(quad).unwrap();
        assert_eq!(sprite.custom_size, Some(vp.logical_size));
    }
}
