//! The UI shell around an app's own screens: the window stack with its focus
//! ([`crate::windows`]), the developer console dialog ([`ConsolePanel`]), Escape held over a
//! dialog it will close, keyboard repeat and the latest pointer. The app (its menus, dialogs and
//! base layer) keeps everything that is its own.

use crate::canvas::{Canvas, Rect};
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
    /// Synchronizes the app's own windows (its dialogs' layers) with the window stack.
    fn sync_windows(&mut self);
    /// A state of the app in which Escape closes no window (a loading dialog, the app's own box
    /// being dragged).
    fn escape_blocked(&self) -> bool;
    /// Escape closes the app's window `window` on top (on release, after showing its button).
    fn escape_closes(&self, window: Self::Window) -> bool;
    /// Escape is held over `top`: the app shows the button Escape will press.
    fn hold_escape(&mut self, top: Option<Self::Window>);
    /// Escape was let go, or a window closed: the app's held looks end.
    fn release_escape(&mut self);
    /// Undoes a drag or resize of the app's own in progress (a box over the windows).
    fn cancel_drag(&mut self) -> bool;
    /// Escape in a modal state of the app (a loading dialog, a box over the windows); `true`
    /// when that took it.
    fn escape_modal(&mut self) -> bool;
    /// Escape on the app's window `window`, on top.
    fn escape_window(&mut self, window: Self::Window);
    /// Escape on the base (no window open).
    fn escape_base(&mut self);
    /// A window closed: the app's interaction state follows.
    fn window_closed(&mut self);
    /// The console can neither open nor close now (a modal state of the app).
    fn console_blocked(&self) -> bool;
    /// The console may open (the app's setting).
    fn console_enabled(&self) -> bool;
    /// The console is about to open over the base.
    fn console_opening(&mut self);
    /// The console closed.
    fn console_closed(&mut self);
    /// The canvas the console dialog is placed on.
    fn canvas(&self) -> &Canvas;
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

/// Escape went down. When it would close a dialog that shows a close or Cancel button, that
/// button shows pressed until Escape is released and the dialog closes then; everything else
/// Escape does happens at once.
pub fn escape_pressed<H: Host>(host: &mut H) {
    host.sync_windows();
    if escape_closes_dialog(host) {
        host.shell_mut().escape_held = true;
        let top = host.shell().windows.top();
        host.hold_escape(top);
    } else {
        escape(host);
    }
}

/// Escape went up: closes the dialog whose close or Cancel button it held.
pub fn escape_released<H: Host>(host: &mut H) {
    host.sync_windows();
    if std::mem::take(&mut host.shell_mut().escape_held) {
        host.release_escape();
        if escape_closes_dialog(host) {
            escape(host);
        }
    }
}

/// Whether Escape closes the top window (on release, after showing the button it presses).
fn escape_closes_dialog<H: Host>(host: &H) -> bool {
    if host.escape_blocked() {
        return false;
    }
    match host.shell().windows.top() {
        Some(window) if window == H::Window::CONSOLE => {
            let panel = &host.shell().panel;
            !panel.placement.active() && !panel.editor.can_dismiss()
        }
        Some(window) => host.escape_closes(window),
        None => false,
    }
}

/// Escape: undoes a drag or resize in progress first, then in the console ends a history
/// search or hides the completion list, then answers the app's modal state, then closes the top
/// window, else it is the base's.
pub fn escape<H: Host>(host: &mut H) {
    // A drag or resize in progress is undone first.
    let console_on_top = host.shell().windows.top() == Some(H::Window::CONSOLE);
    if host.cancel_drag() || (console_on_top && host.shell_mut().panel.placement.cancel()) {
        return;
    }
    // In the console Escape first ends a history search, then hides the completion list.
    if console_on_top && host.shell_mut().panel.editor.dismiss() {
        return;
    }
    if host.escape_modal() {
        return;
    }
    // Escape closes the top window first, whichever layer has the focus.
    match host.shell().windows.top() {
        Some(window) if window == H::Window::CONSOLE => {
            close_window(host, window);
            host.console_closed();
        }
        Some(window) => host.escape_window(window),
        None => host.escape_base(),
    }
}

/// Opens the console on top with the focus, or closes it (focused or not); nothing happens while
/// a modal layer is open, and it opens only while the app enables it.
pub fn toggle<H: Host>(host: &mut H) {
    if host.shell().windows.has_modal() || host.console_blocked() {
        return;
    }
    if host.shell().windows.is_active(H::Window::CONSOLE) {
        close_window(host, H::Window::CONSOLE);
        host.console_closed();
    } else if host.console_enabled() {
        host.console_opening();
        host.shell_mut().windows.open(H::Window::CONSOLE);
        host.focus_changed();
        let canvas = host.canvas().clone();
        let panel = &mut host.shell_mut().panel;
        let frame = panel.frame(&canvas);
        panel.placement.reset_if_off_screen(&frame);
    }
}

/// Closes `window`: the console's drag or resize ends, Escape's held look goes and the window
/// fades out.
pub fn close_window<H: Host>(host: &mut H, window: H::Window) {
    if window == H::Window::CONSOLE {
        host.shell_mut().panel.placement.end();
    }
    host.shell_mut().escape_held = false;
    host.release_escape();
    host.shell_mut().windows.close(window);
    host.window_closed();
}
