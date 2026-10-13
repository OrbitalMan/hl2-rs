//! GameUI map loading dialog with real loader stage progress reporting.
//!
//! Layout matches retail Source `LoadingDialogNoBannerSingle.res` / `LoadingDialogNoBanner.res`:
//! - Centered frame (380 x 112) with title `#GameUI_Loading` ("LOADING..."), no close button.
//! - Info label (x: 20, y: 34, w: 340, h: 24) showing the current localized stage or loading text.
//! - Progress bar (x: 20, y: 64, w: 340, h: 24) with sunken depressed border, transparent background,
//!   and segmented chunks (width 8, gap 4, inset 4 per Source SDK `ProgressBar.cpp`).

use crate::{
    canvas::{Color, Rect},
    gameui::{localize::Localization, widgets::Painter},
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

/// State of an active map loading dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadingDialog {
    pub map: String,
    pub stage: Option<&'static str>,
    pub fraction: f32,
    pub status_override: Option<String>,
    pub error: Option<String>,
    pub complete: bool,
}

impl LoadingDialog {
    /// Creates a new loading dialog for the given map name.
    pub fn new(map: impl Into<String>) -> Self {
        Self {
            map: map.into(),
            stage: None,
            fraction: 0.0,
            status_override: None,
            error: None,
            complete: false,
        }
    }

    /// Updates progress from the latest report. Ignored once failed or complete.
    pub fn update(&mut self, progress: Option<LoadProgress>) {
        if self.error.is_some() || self.complete {
            return;
        }
        if let Some(p) = progress {
            self.fraction = p.fraction.clamp(0.0, 1.0);
            self.stage = Some(p.stage);
        }
    }

    /// Sets a failure error message. Locks the dialog in the failed state.
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Marks the loading operation as complete.
    pub fn set_complete(&mut self) {
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
        if let Some(stage) = self.stage {
            if let Some(localized) = loc.lookup(stage) {
                return localized.to_owned();
            }
            // Standard Source retail token fallbacks
            match stage {
                "LoadingProgress_SpawningServer" => "Starting local game server...".into(),
                "LoadingProgress_LoadMap" => "Loading world...".into(),
                "LoadingProgress_PrecacheWorld" => "Initializing world...".into(),
                "LoadingProgress_SignonDataLocal" => "Initializing game data...".into(),
                "LoadingProgress_LoadResources" => "Loading resources...".into(),
                "LoadingProgress_SignonLocal" => "Initializing resources...".into(),
                _ => stage.into(),
            }
        } else if let Some(template) = loc.lookup("GameUI_LoadingFilename") {
            if template.contains("%s1") {
                template.replace("%s1", &self.map)
            } else {
                format!("Loading {} ...", self.map)
            }
        } else {
            format!("Loading {} ...", self.map)
        }
    }

    /// Returns the origin and scale for centering the loading dialog on the canvas.
    pub fn layout_geometry(canvas_size: Vec2, scale: f32) -> (Vec2, f32) {
        let scaled_size = LOADING_DIALOG_SIZE * scale;
        let origin = Vec2::new(
            ((canvas_size.x - scaled_size.x) * 0.5).max(0.0).round(),
            ((canvas_size.y - scaled_size.y) * 0.5).max(0.0).round(),
        );
        (origin, scale)
    }

    /// Draws the loading dialog centered within the painter's canvas bounds.
    pub fn draw(&self, painter: &Painter, loc: &Localization) {
        let (origin, scale) = Self::layout_geometry(
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

        let size = LOADING_DIALOG_SIZE;
        // Title: "#GameUI_Loading" -> "LOADING...", no close button
        let title_token = loc.localize("#GameUI_Loading");
        let title = if title_token == "#GameUI_Loading" {
            "LOADING...".into()
        } else {
            title_token
        };
        p.frame_with_close(size, &title, None);

        // Info label: x: 20, y: 34, w: 340, h: 24
        let label_text = self.status_text(loc);
        let text_color = if self.error.is_some() {
            Color::new(1.0, 0.35, 0.35, 1.0)
        } else {
            p.theme.text
        };
        p.text(&label_text, Vec2::new(20., 36.), text_color, false);

        // Progress bar: x: 20, y: 64, w: 340, h: 24
        let bar_rect = Rect::new(20., 64., 340., 24.);
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
}
