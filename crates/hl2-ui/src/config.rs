//! User settings and configuration persistence.
//! Stored in a JSON file alongside the executable, never inside the game install.
//!
//! Every section uses `#[serde(default)]`, so files written by older builds keep loading: missing
//! fields take their defaults, and the legacy `video.borderless` flag migrates to
//! [`VideoSettings::display_mode`] in [`Config::migrate`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// `mat_hdr_level`: 0 None, 1 Bloom, 2 Full.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HdrMode {
    #[default]
    None,
    Bloom,
    Full,
}

impl HdrMode {
    pub const ALL: [Self; 3] = [Self::None, Self::Bloom, Self::Full];

    pub fn level(self) -> u8 {
        self as u8
    }

    pub fn from_level(level: u8) -> Option<Self> {
        Self::ALL.get(usize::from(level)).copied()
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::None => "#GameUI_hdr_level0",
            Self::Bloom => "#GameUI_hdr_level1",
            Self::Full => "#GameUI_hdr_level2",
        }
    }
}

/// The Video page's Display mode combo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    #[default]
    Fullscreen,
    Windowed,
    /// "Windowed (No Border)": a borderless window covering the display. Not offered on macOS.
    NoBorder,
}

impl DisplayMode {
    /// The choices offered on this platform, in combo order. "Windowed (No Border)" is offered on
    /// Windows and Linux; the retail macOS build has no borderless mode, so it is omitted there.
    pub fn choices() -> &'static [Self] {
        if cfg!(target_os = "macos") {
            &[Self::Fullscreen, Self::Windowed]
        } else {
            &[Self::Fullscreen, Self::Windowed, Self::NoBorder]
        }
    }

    /// The mode actually used on this platform. A stored `NoBorder` is kept as written (so a
    /// config shared with a Windows/Linux build keeps it) but behaves as full screen on macOS.
    pub fn effective(self) -> Self {
        if Self::choices().contains(&self) {
            self
        } else {
            Self::Fullscreen
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Fullscreen => "#GameUI_Fullscreen",
            Self::Windowed => "#GameUI_Windowed",
            Self::NoBorder => "#GameUI_NoWindowBorder",
        }
    }
}

/// The Video page's HUD Aspect Ratio combo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HudAspect {
    #[default]
    Fit,
    Normal,
    Wide16x9,
    Wide16x10,
}

impl HudAspect {
    pub const ALL: [Self; 4] = [Self::Fit, Self::Normal, Self::Wide16x9, Self::Wide16x10];

    pub fn token(self) -> &'static str {
        match self {
            Self::Fit => "#GameUI_HUDAspectRatio_FitDisplay",
            Self::Normal => "#GameUI_HUDAspectRatio_4x3",
            Self::Wide16x9 => "#GameUI_HUDAspectRatio_16x9",
            Self::Wide16x10 => "#GameUI_HUDAspectRatio_16x10",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub width: u32,
    pub height: u32,
    pub display_mode: DisplayMode,
    pub fov: f32,
    pub hdr: HdrMode,
    pub hud_aspect: HudAspect,
    /// `mat_monitorgamma` (1.6 bright to 2.6 dark).
    pub gamma: f32,
    /// Legacy flag from older files; read once by [`Config::migrate`], never written.
    #[serde(skip_serializing)]
    pub borderless: Option<bool>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            display_mode: DisplayMode::Fullscreen,
            fov: 75.0,
            hdr: HdrMode::None,
            hud_aspect: HudAspect::Fit,
            gamma: 2.2,
            borderless: None,
        }
    }
}

/// Speaker configuration (`snd_surround_speakers`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speakers {
    Headphones,
    #[default]
    Two,
    Four,
    FiveOne,
    SevenOne,
}

impl Speakers {
    pub const ALL: [Self; 5] = [
        Self::Headphones,
        Self::Two,
        Self::Four,
        Self::FiveOne,
        Self::SevenOne,
    ];

    pub fn token(self) -> &'static str {
        match self {
            Self::Headphones => "#GameUI_Headphones",
            Self::Two => "#GameUI_2Speakers",
            Self::Four => "#GameUI_4Speakers",
            Self::FiveOne => "#GameUI_5Speakers",
            Self::SevenOne => "#GameUI_7Speakers",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundQuality {
    #[default]
    High,
    Medium,
    Low,
}

impl SoundQuality {
    pub const ALL: [Self; 3] = [Self::High, Self::Medium, Self::Low];

    pub fn token(self) -> &'static str {
        match self {
            Self::High => "#GameUI_High",
            Self::Medium => "#GameUI_Medium",
            Self::Low => "#GameUI_Low",
        }
    }
}

/// Captioning (`closecaption` / `cc_subtitles`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Captions {
    #[default]
    None,
    ClosedCaptions,
    Subtitles,
}

impl Captions {
    pub const ALL: [Self; 3] = [Self::None, Self::ClosedCaptions, Self::Subtitles];

    pub fn token(self) -> &'static str {
        match self {
            Self::None => "#GameUI_NoClosedCaptions",
            Self::ClosedCaptions => "#GameUI_SubtitlesAndSoundEffects",
            Self::Subtitles => "#GameUI_Subtitles",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub volume: f32,
    pub music_volume: f32,
    pub speakers: Speakers,
    pub quality: SoundQuality,
    /// Spoken language (installed voice pack name, lower case). Applied after a restart.
    pub language: String,
    pub captions: Captions,
    /// `snd_mute_losefocus`.
    pub mute_losefocus: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            music_volume: 1.0,
            speakers: Speakers::Two,
            quality: SoundQuality::High,
            language: "english".into(),
            captions: Captions::None,
            mute_losefocus: true,
        }
    }
}

/// Aim-Assist combo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AimAssist {
    #[default]
    Off,
    Basic,
    Enhanced,
}

impl AimAssist {
    pub const ALL: [Self; 3] = [Self::Off, Self::Basic, Self::Enhanced];

    pub fn token(self) -> &'static str {
        match self {
            Self::Off => "#GameUI_SettingOff",
            Self::Basic => "#GameUI_SettingBasic",
            Self::Enhanced => "#GameUI_SettingEnhanced",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseSettings {
    pub sensitivity: f32,
    pub invert: bool,
    /// `m_filter`: average the last two frames of mouse movement.
    pub filter: bool,
    /// `m_rawinput`.
    pub raw_input: bool,
    /// `m_customaccel` enabled.
    pub acceleration: bool,
    /// `m_customaccel_exponent` (1.0 unaccelerated).
    pub accel_exponent: f32,
    pub aim_assist: AimAssist,
    /// `m_autoaim_scale`.
    pub aim_assist_scale: f32,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            sensitivity: 3.0,
            invert: false,
            filter: false,
            raw_input: false,
            acceleration: false,
            accel_exponent: 1.05,
            aim_assist: AimAssist::Off,
            aim_assist_scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameplaySettings {
    /// `skill`: 1 Easy, 2 Normal, 3 Hard.
    pub skill: u8,
    /// Overridden or unlocked main-menu background map (falls back to `scripts/chapterbackgrounds.txt`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_map: Option<String>,
}

impl Default for GameplaySettings {
    fn default() -> Self {
        Self {
            skill: 2,
            background_map: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub gameplay: GameplaySettings,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub mouse: MouseSettings,
    pub keys: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut keys = BTreeMap::new();
        keys.insert("+attack".into(), "MOUSE1".into());
        keys.insert("+attack2".into(), "MOUSE2".into());
        keys.insert("+forward".into(), "KeyW".into());
        keys.insert("+back".into(), "KeyS".into());
        keys.insert("+moveleft".into(), "KeyA".into());
        keys.insert("+moveright".into(), "KeyD".into());
        keys.insert("+jump".into(), "Space".into());
        // config_default.cfg: `bind_mac "ALT" "+duck"`, `bind_mac "LWIN" "+walk"`,
        // `unbind_mac "CTRL"`.
        if cfg!(target_os = "macos") {
            keys.insert("+duck".into(), "AltLeft".into());
            keys.insert("+walk".into(), "SuperLeft".into());
        } else {
            keys.insert("+duck".into(), "ControlLeft".into());
            keys.insert("+walk".into(), "AltLeft".into());
        }
        keys.insert("+speed".into(), "ShiftLeft".into());
        keys.insert("+use".into(), "KeyE".into());
        keys.insert("+reload".into(), "KeyR".into());
        keys.insert("lastinv".into(), "KeyQ".into());
        keys.insert("invprev".into(), "MWHEELUP".into());
        keys.insert("invnext".into(), "MWHEELDOWN".into());
        keys.insert("impulse".into(), "KeyG".into());
        keys.insert("loadout".into(), "F3".into());
        keys.insert("slot1".into(), "Digit1".into());
        keys.insert("slot2".into(), "Digit2".into());
        keys.insert("slot3".into(), "Digit3".into());
        keys.insert("slot4".into(), "Digit4".into());
        keys.insert("slot5".into(), "Digit5".into());
        keys.insert("slot6".into(), "Digit6".into());
        Self {
            gameplay: GameplaySettings::default(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            mouse: MouseSettings::default(),
            keys,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("hl2-config.json")))
            .unwrap_or_else(|| PathBuf::from("hl2-config.json"))
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        std::fs::read(&path)
            .ok()
            .and_then(|bytes| Self::from_json(&bytes))
            .unwrap_or_default()
    }

    /// Parses a config file and applies [`Config::migrate`]. Returns `None` for unreadable JSON.
    pub fn from_json(bytes: &[u8]) -> Option<Self> {
        let mut config: Self = serde_json::from_slice(bytes).ok()?;
        config.migrate();
        Some(config)
    }

    /// Upgrades legacy fields and brings out-of-range values back into their valid ranges.
    pub fn migrate(&mut self) {
        if let Some(borderless) = self.video.borderless.take() {
            self.video.display_mode = if borderless {
                DisplayMode::NoBorder
            } else {
                DisplayMode::Windowed
            };
        }
        let clamp = |value: &mut f32, min: f32, max: f32, default: f32| {
            *value = if value.is_finite() {
                value.clamp(min, max)
            } else {
                default
            };
        };
        let defaults = Self::default();
        clamp(&mut self.audio.volume, 0., 1., defaults.audio.volume);
        clamp(
            &mut self.audio.music_volume,
            0.,
            1.,
            defaults.audio.music_volume,
        );
        clamp(
            &mut self.mouse.sensitivity,
            0.1,
            20.,
            defaults.mouse.sensitivity,
        );
        clamp(
            &mut self.mouse.accel_exponent,
            1.,
            1.4,
            defaults.mouse.accel_exponent,
        );
        clamp(
            &mut self.mouse.aim_assist_scale,
            0.5,
            2.,
            defaults.mouse.aim_assist_scale,
        );
        clamp(&mut self.video.gamma, 1.6, 2.6, defaults.video.gamma);
        clamp(&mut self.video.fov, 50., 120., defaults.video.fov);
        self.gameplay.skill = self.gameplay.skill.clamp(1, 3);
        if let Some(map) = self.gameplay.background_map.take() {
            let clean = map
                .trim()
                .strip_suffix(".bsp")
                .unwrap_or(map.trim())
                .to_owned();
            if !clean.is_empty()
                && clean.len() <= 128
                && clean
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                self.gameplay.background_map = Some(clean);
            } else {
                self.gameplay.background_map = None;
            }
        }
        if self.audio.language.trim().is_empty() {
            self.audio.language = defaults.audio.language;
        }
        // Missing actions take their default unless that key already runs another command;
        // explicitly unbound actions are stored as "" and stay unbound.
        for (action, key) in defaults.keys {
            if !self.keys.contains_key(&action) && !self.keys.values().any(|bound| *bound == key) {
                self.keys.insert(action, key);
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(parent);
        }
        let data = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_and_serialization() {
        let default_cfg = Config::default();
        let serialized = serde_json::to_string_pretty(&default_cfg).unwrap();
        assert!(!serialized.contains("borderless"));
        let deserialized = Config::from_json(serialized.as_bytes()).unwrap();
        assert_eq!(default_cfg, deserialized);
        assert_eq!(deserialized.video.hdr, HdrMode::None);
        assert_eq!(deserialized.video.display_mode, DisplayMode::Fullscreen);
        assert_eq!(deserialized.audio.volume, 1.0);
        assert_eq!(deserialized.mouse.sensitivity, 3.0);
        assert_eq!(deserialized.gameplay.skill, 2);
        assert_eq!(deserialized.mouse.aim_assist, AimAssist::Off);
        assert_eq!(deserialized.audio.speakers, Speakers::Two);
        assert_eq!(deserialized.audio.captions, Captions::None);
        assert!(!deserialized.mouse.invert);
    }

    #[test]
    fn legacy_files_keep_loading_and_migrate_borderless() {
        let legacy = br#"{
            "video": { "width": 1920, "height": 1080, "borderless": false, "fov": 90.0, "hdr": "full" },
            "audio": { "volume": 0.5 },
            "mouse": { "sensitivity": 2.5, "invert": true },
            "keys": { "+jump": "KeyJ" }
        }"#;
        let config = Config::from_json(legacy).unwrap();
        assert_eq!(config.video.display_mode, DisplayMode::Windowed);
        assert_eq!(config.video.borderless, None);
        assert_eq!((config.video.width, config.video.height), (1920, 1080));
        assert_eq!(config.video.hdr, HdrMode::Full);
        assert_eq!(config.video.gamma, 2.2);
        assert_eq!(config.audio.volume, 0.5);
        assert_eq!(config.audio.music_volume, 1.0);
        assert!(config.mouse.invert);
        assert_eq!(config.mouse.accel_exponent, 1.05);
        assert_eq!(config.gameplay.skill, 2);
        assert_eq!(config.keys.get("+jump").map(String::as_str), Some("KeyJ"));

        let borderless = Config::from_json(br#"{ "video": { "borderless": true } }"#).unwrap();
        assert_eq!(borderless.video.display_mode, DisplayMode::NoBorder);
        let expected = if cfg!(target_os = "macos") {
            DisplayMode::Fullscreen
        } else {
            DisplayMode::NoBorder
        };
        assert_eq!(borderless.video.display_mode.effective(), expected);
        assert_eq!(
            DisplayMode::choices().contains(&DisplayMode::NoBorder),
            !cfg!(target_os = "macos")
        );
    }

    #[test]
    fn migrate_clamps_out_of_range_values() {
        let config = Config::from_json(
            br#"{ "gameplay": { "skill": 9 }, "video": { "gamma": 9.0 }, "mouse": { "accel_exponent": 0.2 }, "audio": { "language": " " } }"#,
        )
        .unwrap();
        assert_eq!(config.gameplay.skill, 3);
        assert_eq!(config.video.gamma, 2.6);
        assert_eq!(config.mouse.accel_exponent, 1.0);
        assert_eq!(config.audio.language, "english");
        assert!(Config::from_json(b"not json").is_none());
        assert_eq!(HdrMode::from_level(1), Some(HdrMode::Bloom));
        assert_eq!(HdrMode::from_level(3), None);
    }

    #[test]
    fn migrate_fills_missing_default_bindings_but_keeps_unbinds_and_taken_keys() {
        let config = Config::from_json(
            br#"{ "keys": { "+attack": "", "+jump": "MOUSE2", "+forward": "KeyZ" } }"#,
        )
        .unwrap();
        let key = |action: &str| config.keys.get(action).map(String::as_str);
        assert_eq!(key("+attack"), Some(""), "explicit unbind kept");
        assert_eq!(key("+attack2"), None, "MOUSE2 already runs +jump");
        assert_eq!(key("+forward"), Some("KeyZ"));
        assert_eq!(key("invnext"), Some("MWHEELDOWN"), "new default filled in");
        assert_eq!(key("+back"), Some("KeyS"));
    }

    #[test]
    fn background_map_persistence_roundtrip_and_migration() {
        let mut cfg = Config::default();
        assert_eq!(cfg.gameplay.background_map, None);
        let serialized = serde_json::to_string(&cfg).unwrap();
        assert!(!serialized.contains("background_map"));

        cfg.gameplay.background_map = Some("background02".into());
        let serialized = serde_json::to_string(&cfg).unwrap();
        assert!(serialized.contains("\"background_map\":\"background02\""));
        let deserialized = Config::from_json(serialized.as_bytes()).unwrap();
        assert_eq!(
            deserialized.gameplay.background_map.as_deref(),
            Some("background02")
        );

        // Migration strips .bsp extensions
        let with_bsp =
            Config::from_json(br#"{"gameplay":{"background_map":"background03.bsp"}}"#).unwrap();
        assert_eq!(
            with_bsp.gameplay.background_map.as_deref(),
            Some("background03")
        );

        // Migration discards invalid or blank map names
        let blank = Config::from_json(br#"{"gameplay":{"background_map":"   "}}"#).unwrap();
        assert_eq!(blank.gameplay.background_map, None);

        let invalid =
            Config::from_json(br#"{"gameplay":{"background_map":"bad map/name"}}"#).unwrap();
        assert_eq!(invalid.gameplay.background_map, None);
    }
}
