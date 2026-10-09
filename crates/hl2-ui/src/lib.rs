//! Source-resource HUD behavior shared by hosts through explicit draw commands.
pub mod canvas;
pub mod config;
pub mod console;
pub mod hud;
pub mod input;
#[cfg(windows)]
mod native_font;
pub mod options;
pub mod viewport;
