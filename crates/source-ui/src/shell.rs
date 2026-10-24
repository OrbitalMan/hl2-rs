//! The UI shell around an app's own screens: the window stack with its focus
//! ([`crate::windows`]), the developer console dialog ([`ConsolePanel`]), Escape held over a
//! dialog it will close, keyboard repeat and the latest pointer. The app (its menus, dialogs and
//! base layer) keeps everything that is its own.

use crate::console::dialog::Scaling;
use crate::console::panel::ConsolePanel;
use crate::input::KeyRepeat;
use crate::windows::{WindowId, Windows};
use glam::Vec2;

/// The shell's state.
pub struct Shell<W> {
    /// The app's dialog windows and the console: non-modal windows over the base (retail VGUI
    /// frames), in z-order with the keyboard focus.
    pub windows: Windows<W>,
    /// The developer console dialog: output, history, command line, completion, its placement
    /// and drawing (kept while the game runs).
    pub panel: ConsolePanel,
    /// Escape is held and closes the top dialog on release.
    pub escape_held: bool,
    pub key_repeat: KeyRepeat,
    /// The pointer of the latest input frame (cursor hints).
    pub pointer: Option<Vec2>,
}

impl<W: WindowId> Shell<W> {
    /// No windows open; window focus fades take `fade_time` seconds and an unfocused window keeps
    /// `unfocused_alpha` of its focused look; the console dialog is sized with `scaling`.
    pub fn new(fade_time: f32, unfocused_alpha: f32, scaling: Scaling) -> Self {
        Self {
            windows: Windows::new(fade_time, unfocused_alpha),
            panel: ConsolePanel::new(scaling),
            escape_held: false,
            key_repeat: KeyRepeat::default(),
            pointer: None,
        }
    }
}
