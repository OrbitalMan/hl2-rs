//! User settings and configuration persistence.
//! Stored in a JSON file alongside the executable, never inside the game install.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HdrMode {
    None,
    Full,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoSettings {
    pub width: u32,
    pub height: u32,
    pub borderless: bool,
    pub fov: f32,
    pub hdr: HdrMode,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            borderless: false,
            fov: 75.0,
            hdr: HdrMode::Full,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSettings {
    pub volume: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { volume: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseSettings {
    pub sensitivity: f32,
    pub invert: bool,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            sensitivity: 3.0,
            invert: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub mouse: MouseSettings,
    pub keys: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut keys = BTreeMap::new();
        keys.insert("+forward".into(), "KeyW".into());
        keys.insert("+back".into(), "KeyS".into());
        keys.insert("+moveleft".into(), "KeyA".into());
        keys.insert("+moveright".into(), "KeyD".into());
        keys.insert("+jump".into(), "Space".into());
        keys.insert("+duck".into(), "ControlLeft".into());
        keys.insert("+speed".into(), "ShiftLeft".into());
        keys.insert("+walk".into(), "AltLeft".into());
        keys.insert("+use".into(), "KeyE".into());
        keys.insert("+reload".into(), "KeyR".into());
        keys.insert("lastinv".into(), "KeyQ".into());
        keys.insert("impulse".into(), "KeyG".into());
        keys.insert("loadout".into(), "F3".into());
        keys.insert("slot1".into(), "Digit1".into());
        keys.insert("slot2".into(), "Digit2".into());
        keys.insert("slot3".into(), "Digit3".into());
        keys.insert("slot4".into(), "Digit4".into());
        keys.insert("slot5".into(), "Digit5".into());
        keys.insert("slot6".into(), "Digit6".into());
        Self {
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
        if path.is_file() {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(cfg) = serde_json::from_slice::<Self>(&bytes) {
                    return cfg;
                }
            }
        }
        Self::default()
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
        let deserialized: Config = serde_json::from_str(&serialized).unwrap();
        assert_eq!(default_cfg, deserialized);
        assert_eq!(deserialized.video.hdr, HdrMode::Full);
        assert_eq!(deserialized.audio.volume, 1.0);
        assert_eq!(deserialized.mouse.sensitivity, 3.0);
        assert!(!deserialized.mouse.invert);
    }
}
