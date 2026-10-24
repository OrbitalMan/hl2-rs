//! The UI shell around an app's own screens: the window stack with its focus
//! ([`crate::windows`]), the developer console dialog ([`ConsolePanel`]), Escape held over a
//! dialog it will close, keyboard repeat and the latest pointer. The app (its menus, dialogs and
//! base layer) keeps everything that is its own.

use crate::canvas::Rect;
use crate::console::dialog::Scaling;
use crate::console::panel::ConsolePanel;
use crate::input::{Input, KeyRepeat};
use crate::windows::{Layer, Pointer, WindowId, Windows};
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

/// What the shell's control flow asks the app, in the order it asks: the app's mode, its modal
/// layers, its dialog windows and its base layer.
pub trait Host {
    type Window: WindowId;
    /// What input hands back to the app's host.
    type Effect;
    /// The app's mode: the key repeat stops when input changes it.
    type Mode: Copy + PartialEq;
    /// What preparing a frame's windows tells the app's own window input.
    type Frame;

    fn shell(&self) -> &Shell<Self::Window>;
    fn shell_mut(&mut self) -> &mut Shell<Self::Window>;
    fn mode(&self) -> Self::Mode;
    /// A key binding is being captured: held keys do not repeat into it.
    fn capturing_key(&self) -> bool;
    /// The input a modal layer of the app takes (a loading dialog, a message box), which also
    /// occludes what is under it; `None` without one.
    fn modal_input(&mut self, input: &Input) -> Option<Vec<Self::Effect>>;
    /// Prepares the windows for this frame: the modal layer that takes every press, and what the
    /// app's window input needs.
    fn prepare_windows(&mut self) -> (Option<Self::Window>, Self::Frame);
    /// A window's frame on the canvas.
    fn window_bounds(&self, window: Self::Window) -> Rect;
    /// The console's completion popup on the canvas, while it shows (it reaches past the frame).
    fn console_popup_bounds(&self) -> Option<Rect>;
    /// The base layer takes the pointer (a menu; gameplay does not).
    fn base_takes_pointer(&self) -> bool;
    /// The focused layer may have changed.
    fn focus_changed(&mut self);
    /// A window's input: its `share` of the frame, and the whole `input` for windows that take
    /// it all.
    fn window_input(
        &mut self,
        frame: &Self::Frame,
        window: Self::Window,
        share: &Input,
        input: &Input,
    ) -> Vec<Self::Effect>;
    /// A modal layer opened: the windows and the base get nothing more this frame.
    fn input_blocked(&self) -> bool;
    /// A window has the pointer: the base only follows the button.
    fn occlude_base(&mut self, down: bool);
    /// The base's input: its share of the frame, and whether it owns the pointer.
    fn base_input(&mut self, share: &Input, owns_pointer: bool) -> Vec<Self::Effect>;
}

/// One frame of input: held keys repeat for every layer (except into a key binding being
/// captured), then a modal layer of the app takes it, else the windows and the base share it.
pub fn input<H: Host>(host: &mut H, input: &Input) -> Vec<H::Effect> {
    let mut repeated = input.clone();
    if host.capturing_key() {
        host.shell_mut().key_repeat.cancel();
    } else {
        host.shell_mut().key_repeat.apply(&mut repeated);
    }
    let mode = host.mode();
    let effects = layer_input(host, &repeated);
    if host.mode() != mode {
        host.shell_mut().key_repeat.cancel();
    }
    effects
}

fn layer_input<H: Host>(host: &mut H, input: &Input) -> Vec<H::Effect> {
    if let Some(time) = input.time {
        host.shell_mut().windows.set_time(time);
    }
    if let Some(effects) = host.modal_input(input) {
        return effects;
    }
    windows_input(host, input)
}

/// The windows are not modal (retail VGUI frames): the pointer goes to the topmost window under
/// it, else to the base behind them; a press raises and focuses its layer and stays with it
/// until release; the keyboard goes to the focused layer. A modal layer of a window takes every
/// click. Every layer sees every frame, with only its share of it.
fn windows_input<H: Host>(host: &mut H, input: &Input) -> Vec<H::Effect> {
    let (modal, frame) = host.prepare_windows();
    let completion = host.console_popup_bounds();
    let bounds: Vec<(H::Window, Rect)> = host
        .shell()
        .windows
        .order()
        .map(|w| (w, host.window_bounds(w)))
        .collect();
    let pointer = Pointer {
        at: input.pointer,
        click: input.click,
        held: input.held,
        released: input.released,
    };
    let base_takes_pointer = host.base_takes_pointer();
    let owner = host
        .shell_mut()
        .windows
        .route(pointer, modal, base_takes_pointer, |window, at| {
            bounds.iter().any(|(w, r)| *w == window && r.contains(at))
                || window == H::Window::CONSOLE && completion.is_some_and(|r| r.contains(at))
        });
    host.focus_changed();
    let focus = host.shell().windows.focus();
    let mut effects = Vec::new();
    let windows: Vec<H::Window> = host.shell().windows.order().collect();
    for window in windows {
        if !host.shell().windows.is_open(window) {
            continue;
        }
        let layer = Layer::Window(window);
        let share = input.share(owner == Some(layer), focus == layer);
        effects.extend(host.window_input(&frame, window, &share, input));
        if host.input_blocked() {
            return effects;
        }
    }
    if host.base_takes_pointer() {
        let owns = owner == Some(Layer::Base);
        if !owns {
            // A window has the pointer: the base only follows the button (occluded).
            host.occlude_base(input.click || input.held);
        }
        let focus = host.shell().windows.focus();
        let share = input.share(owns, focus == Layer::Base);
        effects.extend(host.base_input(&share, owns));
    }
    effects
}
