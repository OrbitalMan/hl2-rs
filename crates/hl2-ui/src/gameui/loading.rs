//! GameUI map loading dialog with real loader stage progress reporting.
//!
//! Layout matches retail Source `LoadingDialogNoBannerSingle.res` / `LoadingDialogNoBanner.res`:
//! - Centered frame (380 x 112) with title `#GameUI_Loading` ("LOADING..."), no close button.
//! - Info label (x: 20, y: 34, w: 340, h: 24) showing the current localized stage or loading text.
//! - Progress bar (x: 20, y: 64, w: 340, h: 24) with sunken depressed border, transparent background,
//!   and segmented chunks (width 8, gap 4, inset 4 per Source SDK `ProgressBar.cpp`).

use crate::{
    canvas::{Color, Rect},
    gameui::{
        localize::Localization,
        widgets::{FrameSpec, Painter},
    },
};
use glam::Vec2;
use std::sync::Mutex;

/// Dimensions of the loading dialog frame (380 x 112 per `LoadingDialogNoBanner.res`).
pub const LOADING_DIALOG_SIZE: Vec2 = Vec2::new(380., 112.);

/// A real loader progress report: fraction completed (0.0..=1.0) and stage identifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadProgress {
    pub fraction: f32,
    pub stage: &'static str,
}

static PROGRESS: Mutex<Option<LoadProgress>> = Mutex::new(None);

/// Reports loader progress from any thread during map loading.
pub fn report_progress(fraction: f32, stage: &'static str) {
    if let Ok(mut lock) = PROGRESS.lock() {
        *lock = Some(LoadProgress {
            fraction: fraction.clamp(0.0, 1.0),
            stage,
        });
    }
}

/// Retrieves the latest progress report since the last reset.
pub fn current_progress() -> Option<LoadProgress> {
    PROGRESS.lock().ok().and_then(|lock| *lock)
}

/// Resets loader progress before beginning a new map load.
pub fn reset_progress() {
    if let Ok(mut lock) = PROGRESS.lock() {
        *lock = None;
    }
}

/// Kind of loading interface to display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoadingKind {
    /// Modal progress dialog (380x112 centered) for gameplay maps.
    #[default]
    Gameplay,
    /// Non-blocking bottom-right corner plaque with localized loading label for background maps.
    Background,
}

/// Dimensions of the background loading plaque (130 x 36 per Source VGUI `CLoadingDiscPanel`).
pub const BACKGROUND_LOADING_SIZE: Vec2 = Vec2::new(130., 36.);

/// Explicit state sequence for background map transitions.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum BackgroundTransitionState {
    /// No background transition is active.
    #[default]
    Idle,
    /// Background transition initiated for the target map.
    Starting { map: String },
    /// Background loader is actively streaming/processing the target map.
    Loading { map: String, elapsed: f32 },
    /// Background map has successfully loaded; transition completed.
    Finished { map: String },
    /// Background map loading failed; transition ended with error.
    Failed { map: String, reason: String },
}

impl BackgroundTransitionState {
    /// Initiates a background map transition to the given map.
    pub fn start(&mut self, map: impl Into<String>) {
        *self = Self::Starting { map: map.into() };
    }

    /// Advances the transition state over time (dt in seconds).
    pub fn update(&mut self, dt: f32) {
        match self {
            Self::Starting { map } => {
                *self = Self::Loading {
                    map: std::mem::take(map),
                    elapsed: dt.max(0.0),
                };
            }
            Self::Loading { elapsed, .. } => {
                *elapsed += dt.max(0.0);
            }
            Self::Finished { .. } | Self::Failed { .. } | Self::Idle => {}
        }
    }

    /// Completes the transition successfully.
    pub fn finish(&mut self) {
        match self {
            Self::Starting { map } | Self::Loading { map, .. } => {
                *self = Self::Finished {
                    map: std::mem::take(map),
                };
            }
            _ => *self = Self::Idle,
        }
    }

    /// Ends the transition due to a loading failure.
    pub fn fail(&mut self, reason: impl Into<String>) {
        let map = match self {
            Self::Starting { map } | Self::Loading { map, .. } => std::mem::take(map),
            Self::Finished { map } | Self::Failed { map, .. } => std::mem::take(map),
            Self::Idle => String::new(),
        };
        *self = Self::Failed {
            map,
            reason: reason.into(),
        };
    }

    /// Resets the transition to idle.
    pub fn reset(&mut self) {
        *self = Self::Idle;
    }

    /// Whether a background transition is actively in progress (Starting or Loading).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Starting { .. } | Self::Loading { .. })
    }

    /// Whether the transition has reached a terminal state (Finished or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished { .. } | Self::Failed { .. })
    }

    /// Whether the transition ended with a failure.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Whether the transition completed successfully.
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished { .. })
    }

    /// Target map name if a transition is active or completed/failed.
    pub fn target_map(&self) -> Option<&str> {
        match self {
            Self::Starting { map }
            | Self::Loading { map, .. }
            | Self::Finished { map }
            | Self::Failed { map, .. } => Some(map.as_str()),
            Self::Idle => None,
        }
    }

    /// Transition overlay dimming alpha (0.0..=0.45).
    pub fn overlay_alpha(&self) -> f32 {
        if self.is_active() {
            0.45
        } else {
            0.0
        }
    }
}

/// State of an active map loading dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadingDialog {
    pub map: String,
    pub kind: LoadingKind,
    pub stage: Option<&'static str>,
    pub fraction: f32,
    pub status_override: Option<String>,
    pub error: Option<String>,
    pub complete: bool,
    pub transition: BackgroundTransitionState,
}

impl LoadingDialog {
    /// Creates a new loading dialog for the given map name.
    pub fn new(map: impl Into<String>) -> Self {
        Self {
            map: map.into(),
            kind: LoadingKind::Gameplay,
            stage: None,
            fraction: 0.0,
            status_override: None,
            error: None,
            complete: false,
            transition: BackgroundTransitionState::Idle,
        }
    }

    /// Creates a new loading dialog for background map loading with a non-blocking corner plaque.
    pub fn new_background(map: impl Into<String>) -> Self {
        let map_str = map.into();
        Self {
            map: map_str.clone(),
            kind: LoadingKind::Background,
            stage: None,
            fraction: 0.0,
            status_override: None,
            error: None,
            complete: false,
            transition: BackgroundTransitionState::Starting { map: map_str },
        }
    }

    /// Whether this dialog is for a background map load.
    pub fn is_background(&self) -> bool {
        self.kind == LoadingKind::Background
    }

    /// Updates progress from the latest report. Ignored once failed or complete.
    pub fn update(&mut self, progress: Option<LoadProgress>) {
        if self.error.is_some() || self.complete {
            return;
        }
        if self.is_background() {
            self.transition.update(1.0 / 60.0);
        }
        if let Some(p) = progress {
            self.fraction = p.fraction.clamp(0.0, 1.0);
            self.stage = Some(p.stage);
        }
    }

    /// Sets a failure error message. Locks the dialog in the failed state.
    pub fn set_error(&mut self, error: impl Into<String>) {
        let err = error.into();
        if self.is_background() {
            self.transition.fail(&err);
        }
        self.error = Some(err);
    }

    /// Marks the loading operation as complete.
    pub fn set_complete(&mut self) {
        if self.is_background() {
            self.transition.finish();
        }
        self.fraction = 1.0;
        self.complete = true;
    }

    /// Whether the loading operation has failed.
    pub fn is_failed(&self) -> bool {
        self.error.is_some()
    }

    /// Whether the loading operation has completed.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Resolves the current status text to display above the progress bar.
    pub fn status_text(&self, loc: &Localization) -> String {
        if let Some(err) = &self.error {
            return err.clone();
        }
        if let Some(custom) = &self.status_override {
            return custom.clone();
        }
        if let Some(stage) = &self.stage {
            loc.text(stage)
        } else {
            let template = loc.text("#GameUI_LoadingFilename");
            if template.contains("%s1") {
                template.replace("%s1", &self.map)
            } else {
                format!("{template} {}", self.map)
            }
        }
    }

    /// Returns the origin and scale for centering a dialog with the given size on the canvas.
    pub fn layout_geometry_sized(canvas_size: Vec2, scale: f32, dialog_size: Vec2) -> (Vec2, f32) {
        let scaled_size = dialog_size * scale;
        let origin = Vec2::new(
            ((canvas_size.x - scaled_size.x) * 0.5).max(0.0).round(),
            ((canvas_size.y - scaled_size.y) * 0.5).max(0.0).round(),
        );
        (origin, scale)
    }

    /// Returns the origin and scale for centering the loading dialog on the canvas.
    pub fn layout_geometry(canvas_size: Vec2, scale: f32) -> (Vec2, f32) {
        Self::layout_geometry_sized(canvas_size, scale, LOADING_DIALOG_SIZE)
    }

    /// Returns the origin and scale for placing the background loading plaque in the bottom-right corner.
    pub fn background_plaque_geometry(canvas_size: Vec2, scale: f32) -> (Vec2, f32) {
        let scaled_size = BACKGROUND_LOADING_SIZE * scale;
        let margin = 24.0 * scale;
        let origin = Vec2::new(
            (canvas_size.x - scaled_size.x - margin).max(0.0).round(),
            (canvas_size.y - scaled_size.y - margin).max(0.0).round(),
        );
        (origin, scale)
    }

    /// Draws the non-blocking background loading plaque in the bottom-right corner.
    fn draw_background_plaque(&self, painter: &Painter, loc: &Localization) {
        let (origin, scale) = Self::background_plaque_geometry(
            Vec2::new(painter.canvas.width(), painter.canvas.height()),
            painter.scale,
        );
        let p = Painter {
            canvas: painter.canvas,
            fonts: painter.fonts,
            theme: painter.theme,
            origin: painter.origin + origin,
            scale,
        };
        // Routed through the shared dialog frame (FrameSpec with FrameStyle::DEFAULT: rounded corners, no outer 3D border)
        // Translucent dark plaque body matching Source VGUI CLoadingDiscPanel (Color(0, 0, 0, 140))
        let plaque_spec = FrameSpec::new(BACKGROUND_LOADING_SIZE, "")
            .without_close()
            .with_bg_color(Color::new(0.0, 0.0, 0.0, 0.65));
        p.frame_spec(plaque_spec, None);

        // Center localized `#GameUI_Loading` ("LOADING...") text within plaque
        let title = loc.text("#GameUI_Loading");
        let text_w = p.text_width(&title, false);
        let font_tall = crate::gameui::widgets::FONT_TALL;
        let text_x = ((BACKGROUND_LOADING_SIZE.x - text_w) * 0.5)
            .max(4.0)
            .round();
        let text_y = ((BACKGROUND_LOADING_SIZE.y - font_tall) * 0.5)
            .max(2.0)
            .round();
        p.text(&title, Vec2::new(text_x, text_y), p.theme.text, false);
    }

    /// Draws the backdrop blur overlay for background map loading across the canvas.
    /// This softens and blurs the underlying 3D background behind the main menu.
    /// Never applies to gameplay map loading.
    pub fn draw_background_overlay(&self, canvas: &crate::canvas::Canvas) {
        if !self.is_background() || !self.transition.is_active() {
            return;
        }
        let alpha = self.transition.overlay_alpha();
        if alpha > 0.0 {
            canvas.blur(
                0.,
                0.,
                canvas.width(),
                canvas.height(),
                Color::new(0.0, 0.0, 0.0, alpha),
            );
        }
    }

    /// Draws the loading dialog centered within the painter's canvas bounds.
    pub fn draw(&self, painter: &Painter, loc: &Localization) {
        if self.is_background() {
            if self.transition.is_active() {
                self.draw_background_plaque(painter, loc);
            }
            return;
        }
        let label_text = self.status_text(loc);
        let text_color = if self.error.is_some() {
            Color::new(1.0, 0.35, 0.35, 1.0)
        } else {
            painter.theme.text
        };
        // Wrap status/warning text to 340px width (parent dialog width 380 - 20px padding each side)
        let max_label_w = 340.0;
        let lines = crate::gameui::widgets::wrap(&label_text, max_label_w, |s| {
            painter.text_width(s, false)
        });
        let line_count = lines.len().max(1);

        // Dynamically expand dialog height and shift progress bar if text wraps beyond 2 lines
        let font_tall = crate::gameui::widgets::FONT_TALL;
        let label_h = (line_count as f32 * font_tall).max(24.0);
        let bar_y = 34.0 + label_h + 6.0;
        let dialog_h = (bar_y + 24.0 + 16.0).max(LOADING_DIALOG_SIZE.y);
        let dialog_size = Vec2::new(LOADING_DIALOG_SIZE.x, dialog_h);

        let (origin, scale) = Self::layout_geometry_sized(
            Vec2::new(painter.canvas.width(), painter.canvas.height()),
            painter.scale,
            dialog_size,
        );
        let p = Painter {
            canvas: painter.canvas,
            fonts: painter.fonts,
            theme: painter.theme,
            origin: painter.origin + origin,
            scale,
        };

        let title = loc.text("#GameUI_Loading");
        p.frame_with_close(dialog_size, &title, None);

        // Render each line inside the label area
        let mut y = 34.0;
        for line in &lines {
            p.text(line, Vec2::new(20., y), text_color, false);
            y += font_tall;
        }

        // Progress bar: positioned below the label
        let bar_rect = Rect::new(20., bar_y, 340., 24.);
        let bar_bg = Color::new(0.0, 0.0, 0.0, 0.6);
        let bar_fg = p.theme.text;
        p.progress_bar(bar_rect, self.fraction, bar_fg, bar_bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_dialog_initial_state_and_progress_updates() {
        let loc = Localization::default();
        let mut dialog = LoadingDialog::new("d1_trainstation_01");
        assert_eq!(dialog.map, "d1_trainstation_01");
        assert_eq!(dialog.fraction, 0.0);
        assert!(!dialog.is_failed());
        assert!(!dialog.is_complete());
        assert_eq!(dialog.status_text(&loc), "Loading d1_trainstation_01 ...");

        dialog.update(Some(LoadProgress {
            fraction: 0.35,
            stage: "LoadingProgress_PrecacheWorld",
        }));
        assert_eq!(dialog.fraction, 0.35);
        assert_eq!(dialog.stage, Some("LoadingProgress_PrecacheWorld"));
        assert_eq!(dialog.status_text(&loc), "Initializing world...");
    }

    #[test]
    fn loading_dialog_stages_fallback_and_localization() {
        let mut loc = Localization::default();
        loc.insert("#GameUI_LoadingFilename", "Chargement de %s1 ...");
        loc.insert("#LoadingProgress_LoadMap", "Chargement du monde...");

        let mut dialog = LoadingDialog::new("test_map");
        assert_eq!(dialog.status_text(&loc), "Chargement de test_map ...");

        dialog.update(Some(LoadProgress {
            fraction: 0.15,
            stage: "LoadingProgress_LoadMap",
        }));
        assert_eq!(dialog.status_text(&loc), "Chargement du monde...");

        dialog.update(Some(LoadProgress {
            fraction: 0.75,
            stage: "LoadingProgress_LoadResources",
        }));
        // Unlocalized token falls back to built-in English string
        assert_eq!(dialog.status_text(&loc), "Loading resources...");
    }

    #[test]
    fn loading_dialog_failure_state_handling() {
        let loc = Localization::default();
        let mut dialog = LoadingDialog::new("missing_map");
        dialog.update(Some(LoadProgress {
            fraction: 0.1,
            stage: "LoadingProgress_LoadMap",
        }));
        assert!(!dialog.is_failed());

        dialog.set_error("owned map missing_map.bsp not found");
        assert!(dialog.is_failed());
        assert_eq!(
            dialog.status_text(&loc),
            "owned map missing_map.bsp not found"
        );

        // Progress updates after failure are ignored
        dialog.update(Some(LoadProgress {
            fraction: 0.8,
            stage: "LoadingProgress_LoadResources",
        }));
        assert_eq!(dialog.fraction, 0.1);
        assert_eq!(
            dialog.status_text(&loc),
            "owned map missing_map.bsp not found"
        );
    }

    #[test]
    fn loading_dialog_completion_state() {
        let mut dialog = LoadingDialog::new("d1_trainstation_02");
        assert!(!dialog.is_complete());
        dialog.set_complete();
        assert!(dialog.is_complete());
        assert_eq!(dialog.fraction, 1.0);

        // Progress updates after completion are ignored
        dialog.update(Some(LoadProgress {
            fraction: 0.2,
            stage: "LoadingProgress_LoadMap",
        }));
        assert_eq!(dialog.fraction, 1.0);
    }

    #[test]
    fn global_progress_reporting_and_reset() {
        reset_progress();
        assert_eq!(current_progress(), None);

        report_progress(0.45, "LoadingProgress_SignonDataLocal");
        assert_eq!(
            current_progress(),
            Some(LoadProgress {
                fraction: 0.45,
                stage: "LoadingProgress_SignonDataLocal",
            })
        );

        // Clamping check
        report_progress(1.5, "LoadingProgress_SignonLocal");
        assert_eq!(
            current_progress(),
            Some(LoadProgress {
                fraction: 1.0,
                stage: "LoadingProgress_SignonLocal",
            })
        );

        reset_progress();
        assert_eq!(current_progress(), None);
    }

    #[test]
    fn loading_dialog_scaling_and_layout_geometry() {
        // Standard 720p base (scale = 1.0): size is 380x112, centered on 1280x720
        let (origin_720, scale_720) = LoadingDialog::layout_geometry(Vec2::new(1280., 720.), 1.0);
        assert_eq!(scale_720, 1.0);
        assert_eq!(origin_720, Vec2::new(450., 304.));

        // 1280x800 standard reference (scale = 800/720)
        let scale_800 = Painter::scale_for(800.);
        let (origin_800, scale) = LoadingDialog::layout_geometry(Vec2::new(1280., 800.), scale_800);
        assert_eq!(scale, 800. / 720.);
        let scaled_size = LOADING_DIALOG_SIZE * scale;
        assert_eq!(
            origin_800,
            Vec2::new(
                ((1280. - scaled_size.x) * 0.5).round(),
                ((800. - scaled_size.y) * 0.5).round()
            )
        );

        // Native Retina / High-DPI 2560x1600 (scale = 1600/720)
        let scale_retina = Painter::scale_for_window(800., 2.0);
        let (origin_retina, scale_ret) =
            LoadingDialog::layout_geometry(Vec2::new(2560., 1600.), scale_retina);
        assert_eq!(scale_ret, 1600. / 720.);
        let scaled_size_retina = LOADING_DIALOG_SIZE * scale_ret;
        assert_eq!(
            origin_retina,
            Vec2::new(
                ((2560. - scaled_size_retina.x) * 0.5).round(),
                ((1600. - scaled_size_retina.y) * 0.5).round()
            )
        );
    }

    #[test]
    fn loading_dialog_multiline_status_wraps_and_expands_bounds() {
        use crate::canvas::Canvas;
        use crate::gameui::widgets::{Fonts, Theme};

        let loc = Localization::default();
        let mut dialog = LoadingDialog::new("ep1_citadel_00");
        dialog.set_error(
            "Warning: Map transition failed due to missing asset or parsing failure. Please verify game files.",
        );

        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.0,
        };

        dialog.draw(&painter, &loc);
        let quads = canvas.drain();

        // The background frame quad should have expanded height > LOADING_DIALOG_SIZE.y (112.0)
        let frame_quad = quads
            .iter()
            .find(|q| {
                q.destination.w == LOADING_DIALOG_SIZE.x && q.destination.h >= LOADING_DIALOG_SIZE.y
            })
            .expect("loading dialog frame quad");
        assert!(
            frame_quad.destination.h > LOADING_DIALOG_SIZE.y,
            "Expected dialog height to expand beyond 112px, got {}",
            frame_quad.destination.h
        );
    }

    #[test]
    fn background_loading_plaque_geometry_and_drawing() {
        use crate::canvas::Canvas;
        use crate::gameui::widgets::{Fonts, Theme};

        let mut loc = Localization::default();
        loc.insert("#GameUI_Loading", "LOADING...");
        let dialog = LoadingDialog::new_background("ep2_background01");
        assert!(dialog.is_background());
        assert_eq!(dialog.map, "ep2_background01");

        // 720p base canvas (1280x720, scale = 1.0)
        let (origin_720, scale_720) =
            LoadingDialog::background_plaque_geometry(Vec2::new(1280., 720.), 1.0);
        assert_eq!(scale_720, 1.0);
        let expected_x = 1280. - BACKGROUND_LOADING_SIZE.x - 24.0;
        let expected_y = 720. - BACKGROUND_LOADING_SIZE.y - 24.0;
        assert_eq!(origin_720, Vec2::new(expected_x, expected_y));

        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.0,
        };

        dialog.draw(&painter, &loc);
        let quads = canvas.drain();

        // Plaque is drawn via the shared dialog frame (rounded corners, no outer 3D border)
        let middle_bar = quads
            .iter()
            .find(|q| {
                q.destination.w == BACKGROUND_LOADING_SIZE.x
                    && q.destination.x == expected_x
                    && q.vertices.is_none()
            })
            .expect("background loading plaque middle bar in bottom-right corner");
        assert_eq!(middle_bar.color, Color::new(0.0, 0.0, 0.0, 0.65));
        assert!(!middle_bar.blur);

        // Verify corner polygon quads exist (4 rounded corner arcs from FrameSpec rounded_fill)
        let corner_polygons = quads
            .iter()
            .filter(|q| q.vertices.is_some() && q.destination.x >= expected_x)
            .count();
        assert_eq!(corner_polygons, 4);
    }

    #[test]
    fn background_loading_overlay_emits_blur_quad() {
        use crate::canvas::Canvas;

        let canvas = Canvas::default();
        canvas.resize(1920., 1080.);
        let dialog = LoadingDialog::new_background("ep2_background01");
        assert!(dialog.is_background());

        dialog.draw_background_overlay(&canvas);
        let quads = canvas.drain();
        assert_eq!(quads.len(), 1, "expected exactly 1 overlay quad");
        let overlay = &quads[0];
        assert!(overlay.blur, "overlay quad must have blur set");
        assert_eq!(overlay.destination.w, 1920.);
        assert_eq!(overlay.destination.h, 1080.);
        assert_eq!(overlay.color, Color::new(0.0, 0.0, 0.0, 0.45));
    }

    #[test]
    fn gameplay_loading_overlay_does_not_emit_blur_quad() {
        use crate::canvas::Canvas;
        use crate::gameui::widgets::{Fonts, Theme};

        let canvas = Canvas::default();
        canvas.resize(1920., 1080.);
        let dialog = LoadingDialog::new("d1_trainstation_01");
        assert!(!dialog.is_background());

        // Background overlay call must be a complete no-op for gameplay loading
        dialog.draw_background_overlay(&canvas);
        let overlay_quads = canvas.drain();
        assert!(
            overlay_quads.is_empty(),
            "gameplay loading dialog must not emit any background overlay"
        );

        // Standard draw must only produce normal dialog quads, never a blur quad
        let loc = Localization::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.0,
        };

        dialog.draw(&painter, &loc);
        let draw_quads = canvas.drain();
        assert!(!draw_quads.is_empty());
        assert!(
            draw_quads.iter().all(|q| !q.blur),
            "no quad in gameplay loading dialog may have blur enabled"
        );
    }

    #[test]
    fn test_background_transition_success_sequence() {
        let mut state = BackgroundTransitionState::default();
        assert_eq!(state, BackgroundTransitionState::Idle);
        assert!(!state.is_active());
        assert!(!state.is_terminal());
        assert_eq!(state.overlay_alpha(), 0.0);

        state.start("ep1_background01");
        assert_eq!(
            state,
            BackgroundTransitionState::Starting {
                map: "ep1_background01".into()
            }
        );
        assert!(state.is_active());
        assert!(!state.is_terminal());
        assert_eq!(state.target_map(), Some("ep1_background01"));
        assert_eq!(state.overlay_alpha(), 0.45);

        state.update(0.1);
        assert_eq!(
            state,
            BackgroundTransitionState::Loading {
                map: "ep1_background01".into(),
                elapsed: 0.1,
            }
        );
        assert!(state.is_active());
        assert_eq!(state.overlay_alpha(), 0.45);

        state.update(0.05);
        if let BackgroundTransitionState::Loading { elapsed, .. } = &state {
            assert!((elapsed - 0.15).abs() < 1e-5);
        } else {
            panic!("expected Loading state");
        }

        state.finish();
        assert_eq!(
            state,
            BackgroundTransitionState::Finished {
                map: "ep1_background01".into()
            }
        );
        assert!(!state.is_active());
        assert!(state.is_terminal());
        assert!(state.is_finished());
        assert!(!state.is_failed());
        assert_eq!(state.overlay_alpha(), 0.0);
    }

    #[test]
    fn test_background_transition_failure_sequence() {
        let mut state = BackgroundTransitionState::default();
        state.start("ep2_background01");
        assert!(state.is_active());

        state.update(0.05);
        assert!(matches!(state, BackgroundTransitionState::Loading { .. }));

        state.fail("BSP file missing or corrupted");
        assert_eq!(
            state,
            BackgroundTransitionState::Failed {
                map: "ep2_background01".into(),
                reason: "BSP file missing or corrupted".into(),
            }
        );
        assert!(!state.is_active());
        assert!(state.is_terminal());
        assert!(state.is_failed());
        assert!(!state.is_finished());
        assert_eq!(state.overlay_alpha(), 0.0);
        assert_eq!(state.target_map(), Some("ep2_background01"));
    }

    #[test]
    fn test_background_loading_dialog_failure_ends_transition_and_suppresses_overlay() {
        use crate::canvas::Canvas;
        use crate::gameui::widgets::{Fonts, Theme};

        let canvas = Canvas::default();
        canvas.resize(1920., 1080.);
        let mut dialog = LoadingDialog::new_background("lostcoast_background01");
        assert!(dialog.is_background());
        assert!(dialog.transition.is_active());

        // Overlay is drawn when active
        dialog.draw_background_overlay(&canvas);
        let quads = canvas.drain();
        assert_eq!(quads.len(), 1);

        // Map loading fails
        dialog.set_error("IO error loading map");
        assert!(dialog.is_failed());
        assert!(!dialog.transition.is_active());
        assert!(dialog.transition.is_failed());
        assert!(dialog.transition.is_terminal());

        // Once failed, background overlay and plaque must NOT be drawn
        dialog.draw_background_overlay(&canvas);
        let overlay_quads = canvas.drain();
        assert!(
            overlay_quads.is_empty(),
            "failed background map load must suppress overlay"
        );

        let loc = Localization::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.0,
        };
        dialog.draw(&painter, &loc);
        let plaque_quads = canvas.drain();
        assert!(
            plaque_quads.is_empty(),
            "failed background map load must suppress corner plaque"
        );
    }

    #[test]
    fn test_background_loading_plaque_styled_with_shared_frame_rounded_corners_no_3d_border() {
        use crate::canvas::Canvas;
        use crate::gameui::widgets::{Fonts, Theme};

        let canvas = Canvas::default();
        canvas.resize(1920., 1080.);
        let dialog = LoadingDialog::new_background("episodic_background01");
        assert!(dialog.is_background());
        assert!(dialog.transition.is_active());

        let mut loc = Localization::default();
        loc.insert("#GameUI_Loading", "LOADING...");
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.0,
        };

        // Draw the background loading plaque
        dialog.draw(&painter, &loc);
        let quads = canvas.drain();
        assert!(!quads.is_empty());

        // 1. Verify corner polygon quads are emitted (4 corner arcs from FrameSpec rounded_fill)
        let polygon_quads: Vec<_> = quads.iter().filter(|q| q.vertices.is_some()).collect();
        assert_eq!(
            polygon_quads.len(),
            4,
            "background loading plaque must use rounded corners (4 corner polygons)"
        );

        // 2. Verify all background fill quads use the translucent plaque color (alpha ~0.65)
        let fill_color = Color::new(0.0, 0.0, 0.0, 0.65);
        for poly in &polygon_quads {
            if let Some(verts) = &poly.vertices {
                for (_, c) in verts {
                    assert_eq!(*c, fill_color);
                }
            }
        }

        // 3. Verify NO 3D raised border quads are emitted
        // (raised border draws 4 rectangles with theme.border_bright and theme.border_dark)
        let has_3d_border = quads.iter().any(|q| {
            q.vertices.is_none()
                && (q.color == theme.border_bright || q.color == theme.border_dark)
                && (q.destination.w <= 2.0 || q.destination.h <= 2.0)
        });
        assert!(
            !has_3d_border,
            "background loading plaque must NOT draw outer 3D raised border"
        );
    }

    #[test]
    fn test_gameplay_loading_dialog_uses_shared_frame_rounded_corners_no_3d_border() {
        use crate::canvas::Canvas;
        use crate::gameui::widgets::{Fonts, Theme};

        let canvas = Canvas::default();
        canvas.resize(1920., 1080.);
        let dialog = LoadingDialog::new("d1_trainstation_01");
        assert!(!dialog.is_background());

        let mut loc = Localization::default();
        loc.insert("#GameUI_Loading", "LOADING...");
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.0,
        };

        dialog.draw(&painter, &loc);
        let quads = canvas.drain();

        // Corner polygon quads emitted by FrameSpec rounded_fill
        let polygon_quads: Vec<_> = quads.iter().filter(|q| q.vertices.is_some()).collect();
        assert_eq!(
            polygon_quads.len(),
            4,
            "gameplay loading dialog must use rounded corners"
        );

        // Frame background color
        for poly in &polygon_quads {
            if let Some(verts) = &poly.vertices {
                for (_, c) in verts {
                    assert_eq!(*c, theme.frame_bg);
                }
            }
        }
    }
}
