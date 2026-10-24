//! One frame of UI input as the host delivers it: keys pressed and held, typed text, the
//! pointer and its left button, the wheel, modifiers and clipboard, and the frame time.
use crate::widgets::HoldRepeat;
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Up,
    Down,
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    /// Presses the focused dialog button, check box or radio button (SDK `Button`).
    Space,
}
impl Key {
    /// Keys that repeat while held: moving through lists and text, deleting. Enter, Tab, Home
    /// and End act once per press.
    pub const fn repeats(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::Backspace
                | Self::Delete
                | Self::PageUp
                | Self::PageDown
        )
    }
}

/// Keyboard repeat for every UI layer at once, applied to [`Input::keys`] before any layer reads
/// them: like an operating system, the latest pressed key repeats while held ([`HoldRepeat`]), and
/// any other press takes over or (when it does not repeat) stops it. Typed text already arrives
/// with the system's own repeat and is left alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyRepeat {
    key: Option<Key>,
    timer: HoldRepeat,
}
impl KeyRepeat {
    /// Adds this frame's repeat of the held key to `input.keys`.
    pub fn apply(&mut self, input: &mut Input) {
        if let Some(&pressed) = input.keys.iter().next_back() {
            self.key = pressed.repeats().then_some(pressed);
            self.timer.press(input.time);
        } else if let Some(key) = self.key {
            if !input.held_keys.contains(&key) {
                self.cancel();
            } else if self.timer.held(input.time) {
                input.keys.insert(key);
            }
        }
    }

    /// Stops the repeat until the next press.
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Input {
    pub keys: std::collections::BTreeSet<Key>,
    /// Keys held down this frame (pressed earlier or now); [`KeyRepeat`] repeats them.
    pub held_keys: std::collections::BTreeSet<Key>,
    pub text: String,
    pub pointer: Option<Vec2>,
    /// Left button went down this frame.
    pub click: bool,
    /// Left button is held (slider drags).
    pub held: bool,
    /// Left button was released this frame.
    pub released: bool,
    /// Mouse wheel steps this frame; positive scrolls up.
    pub wheel: f32,
    /// Keys and buttons pressed this frame as persisted binding names (`KeyW`, `SuperLeft`,
    /// `MOUSE1`, `MWHEELUP`), used when the keyboard page captures a new key.
    pub binding_keys: Vec<String>,
    /// Monotonic real time of this frame in seconds, used for double clicks. `None` (scripted
    /// input without times) disables double-click detection.
    pub time: Option<f64>,
    /// Control is held (console editing shortcuts); typed text is ignored meanwhile.
    pub ctrl: bool,
    /// Shift is held (Shift+Tab steps the completion list back).
    pub shift: bool,
    /// Alt (Option) is held: Alt+Up/Down opens a focused combo box's list (SDK `ComboBox`).
    pub alt: bool,
    /// Text pasted this frame (the host reads the clipboard on its paste shortcut).
    pub paste: Option<String>,
    /// The copy shortcut (Cmd+C on macOS, Ctrl+C elsewhere) was pressed this frame.
    pub copy: bool,
}
impl Input {
    pub fn pressed(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }

    /// What one UI layer receives of this frame: the pointer (position, button, wheel) when it
    /// owns it, the keyboard (keys, text, modifiers, clipboard) when it has the focus, and the
    /// time either way.
    pub fn share(&self, pointer: bool, keyboard: bool) -> Input {
        let mut share = if keyboard {
            self.clone()
        } else {
            Input {
                time: self.time,
                ..Input::default()
            }
        };
        if pointer {
            share.pointer = self.pointer;
            share.click = self.click;
            share.held = self.held;
            share.released = self.released;
            share.wheel = self.wheel;
        } else {
            share.pointer = None;
            share.click = false;
            share.held = false;
            share.released = false;
            share.wheel = 0.;
        }
        share
    }
}
