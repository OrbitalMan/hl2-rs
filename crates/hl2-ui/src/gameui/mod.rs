//! Resource-driven GameUI: main menu, quit confirmation and the options dialog.
//!
//! Models in this module are plain data with explicit inputs and typed effects, so they are unit
//! tested without a window. Text, menu structure, control layout and key actions come from the
//! installed game (see `docs/gameui.md`); hosts only translate input and apply effects.
pub mod chapters;
pub mod display;
pub mod keys;
pub mod layout;
pub mod loading;
pub mod localize;
pub mod menu;
pub mod options;
pub mod placement;
pub mod recommendation;
pub mod resources;
pub mod scheme_fonts;
pub mod settings;
pub mod video_confirm;
pub mod widgets;
pub mod windows;
