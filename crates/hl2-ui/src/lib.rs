//! Source-resource HUD behavior shared by hosts through explicit draw commands.
pub mod canvas;
pub mod closecaption;
pub mod config;
pub mod console;
pub mod fades;
pub mod gameui;
pub mod hud;
pub mod input;
#[cfg(windows)]
mod native_font;
pub mod proportional;
pub mod tuning;
pub mod viewport;

/// Embedded MIT-licensed fallback / developer font bytes (ProggyClean).
pub const PROGGY_CLEAN: &[u8] = include_bytes!("../assets/fonts/ProggyClean.ttf");
