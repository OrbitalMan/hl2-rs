//! Background visual debug mode: overlay, blur adjustment, and camera FOV tuning.

use bevy::prelude::KeyCode;
pub const BACKGROUND_ADJUST_KEYS: &[KeyCode] = &[
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
];

/// Derives a readable aspect ratio label (e.g. "16:9", "4:3", "Fit Display").
pub fn aspect_label(aspect: f32, flexible: bool) -> &'static str {
    if flexible {
        "Fit Display"
    } else if (aspect - 16. / 9.).abs() < 0.05 {
        "16:9"
    } else if (aspect - 16. / 10.).abs() < 0.05 {
        "16:10"
    } else if (aspect - 4. / 3.).abs() < 0.05 {
        "4:3"
    } else if (aspect - 5. / 4.).abs() < 0.05 {
        "5:4"
    } else if (aspect - 21. / 9.).abs() < 0.1 {
        "21:9"
    } else if (aspect - 32. / 9.).abs() < 0.1 {
        "32:9"
    } else if (aspect - 1.).abs() < 0.05 {
        "1:1"
    } else if (aspect - 3. / 4.).abs() < 0.05 {
        "3:4"
    } else if (aspect - 9. / 16.).abs() < 0.05 {
        "9:16"
    } else {
        "Custom"
    }
}

/// Runtime background layout visual analysis state.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundDebug {
    pub enabled: bool,
    pub blur_opacity: u8,
    pub fov_horizontal: Option<f32>,
    pub fov_vertical: Option<f32>,
    pub state_label: String,
    pub map_name: String,
    pub blur_texture_name: String,
    pub last_aspect: Option<f32>,
    pub pending_gameplay_finish: bool,
}

impl Default for BackgroundDebug {
    fn default() -> Self {
        Self {
            enabled: false,
            blur_opacity: 50,
            fov_horizontal: None,
            fov_vertical: None,
            state_label: "Ready".to_owned(),
            map_name: "background01".to_owned(),
            blur_texture_name: "background01_widescreen".to_owned(),
            last_aspect: None,
            pending_gameplay_finish: false,
        }
    }
}

impl BackgroundDebug {
    /// Current horizontal and vertical field of view in degrees.
    pub fn fov(&self, aspect: f32) -> (f32, f32) {
        let def_v = crate::camera::world_fov(75., 75., aspect, 0., true).to_degrees();
        let def_h = (2. * (aspect * (def_v.to_radians() * 0.5).tan()).atan()).to_degrees();
        let h = self.fov_horizontal.unwrap_or(def_h);
        let v = self.fov_vertical.unwrap_or(def_v);
        (h, v)
    }

    /// Formats the lines displayed in the debug overlay.
    pub fn lines(&self, aspect: f32) -> Vec<String> {
        let (fov_h, fov_v) = self.fov(aspect);
        vec![
            format!("state: {}  |  map: {}", self.state_label, self.map_name),
            format!("blur_texture: {}", self.blur_texture_name),
            format!(
                "blur_opacity: {}%  (background_blur_opacity 0..100)",
                self.blur_opacity
            ),
            format!(
                "fov: horizontal {:.2}°  vertical {:.2}°  (L/R: H, D/U: V)",
                fov_h, fov_v
            ),
            "Shift = 1.0  Alt = 0.01    hold to repeat    background_debug_mode 0".to_owned(),
        ]
    }

    /// Adjusts FOV based on fired arrow keys.
    pub fn adjust_fov(&mut self, fired: &[KeyCode], step: f32, aspect: f32) -> bool {
        let (mut h, mut v) = self.fov(aspect);
        let mut changed = false;
        for key in fired {
            match *key {
                KeyCode::ArrowLeft => {
                    h = ((h - step) * 100.0).round() / 100.0;
                    self.fov_horizontal = Some(h.clamp(1.0, 170.0));
                    changed = true;
                }
                KeyCode::ArrowRight => {
                    h = ((h + step) * 100.0).round() / 100.0;
                    self.fov_horizontal = Some(h.clamp(1.0, 170.0));
                    changed = true;
                }
                KeyCode::ArrowDown => {
                    v = ((v - step) * 100.0).round() / 100.0;
                    self.fov_vertical = Some(v.clamp(1.0, 170.0));
                    changed = true;
                }
                KeyCode::ArrowUp => {
                    v = ((v + step) * 100.0).round() / 100.0;
                    self.fov_vertical = Some(v.clamp(1.0, 170.0));
                    changed = true;
                }
                _ => {}
            }
        }
        changed
    }

    /// Checks if aspect ratio changed, updates baseline FOVs, and logs to console.
    pub fn check_aspect_change(
        &mut self,
        aspect: f32,
        aspect_label: &str,
        console: &mut hl2_ui::console::Console,
    ) {
        if !self.enabled {
            self.last_aspect = Some(aspect);
            return;
        }
        let changed = self
            .last_aspect
            .is_some_and(|last| (last - aspect).abs() > 1e-4);
        self.last_aspect = Some(aspect);
        if changed {
            let def_v = crate::camera::world_fov(75., 75., aspect, 0., true).to_degrees();
            let def_h = (2. * (aspect * (def_v.to_radians() * 0.5).tan()).atan()).to_degrees();
            self.fov_horizontal = Some(def_h);
            self.fov_vertical = Some(def_v);
            console.log(format!(
                "Changed aspect ratio to {aspect_label}, background FOV is set to {def_h:.2} / {def_v:.2}."
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_debug_default_fov_and_overrides() {
        let mut debug = BackgroundDebug::default();
        let aspect = 16. / 9.;
        let (def_h, def_v) = debug.fov(aspect);
        assert!((def_v - 59.85).abs() < 0.2);
        assert!((def_h - 91.31).abs() < 0.2);

        // Setting explicit horizontal FOV
        debug.fov_horizontal = Some(95.0);
        let (h, v) = debug.fov(aspect);
        assert_eq!(h, 95.0);
        assert_eq!(v, def_v);

        // Setting explicit vertical FOV
        debug.fov_vertical = Some(65.0);
        let (h, v) = debug.fov(aspect);
        assert_eq!(h, 95.0);
        assert_eq!(v, 65.0);
    }

    #[test]
    fn background_debug_arrow_adjustments() {
        let mut debug = BackgroundDebug::default();
        let aspect = 16. / 9.;
        let (init_h, init_v) = debug.fov(aspect);

        // ArrowRight increases horizontal FOV
        assert!(debug.adjust_fov(&[KeyCode::ArrowRight], 1.0, aspect));
        assert!((debug.fov_horizontal.unwrap() - (init_h + 1.0)).abs() < 0.02);

        // ArrowLeft decreases horizontal FOV
        assert!(debug.adjust_fov(&[KeyCode::ArrowLeft], 0.5, aspect));
        assert!((debug.fov_horizontal.unwrap() - (init_h + 0.5)).abs() < 0.02);

        // ArrowUp increases vertical FOV
        assert!(debug.adjust_fov(&[KeyCode::ArrowUp], 2.0, aspect));
        assert!((debug.fov_vertical.unwrap() - (init_v + 2.0)).abs() < 0.02);

        // ArrowDown decreases vertical FOV
        assert!(debug.adjust_fov(&[KeyCode::ArrowDown], 1.0, aspect));
        assert!((debug.fov_vertical.unwrap() - (init_v + 1.0)).abs() < 0.02);
    }

    #[test]
    fn background_debug_overlay_lines() {
        let debug = BackgroundDebug {
            enabled: true,
            blur_opacity: 50,
            map_name: "background01".to_owned(),
            blur_texture_name: "background01_widescreen".to_owned(),
            state_label: "Ready".to_owned(),
            ..Default::default()
        };
        let lines = debug.lines(16. / 9.);
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("state: Ready"));
        assert!(lines[0].contains("map: background01"));
        assert!(lines[1].contains("blur_texture: background01_widescreen"));
        assert!(lines[2].contains("blur_opacity: 50%"));
        assert!(lines[3].contains("fov: horizontal"));
        assert!(lines[3].contains("vertical"));
    }

    #[test]
    fn background_debug_aspect_change_logging() {
        let mut debug = BackgroundDebug {
            enabled: true,
            last_aspect: Some(4. / 3.),
            ..Default::default()
        };
        let mut console = hl2_ui::console::Console::default();
        debug.check_aspect_change(16. / 9., "16:9", &mut console);
        let log = console
            .shell
            .panel
            .output
            .back()
            .map(|l| l.text.as_str())
            .unwrap_or_default();
        assert!(log.contains("Changed aspect ratio to 16:9, background FOV is set to"));
    }
}
