//! Windows and focus: the console's single-focus window stack (Options, the developer
//! console, their modal layers), Escape, and routing each frame's input to the layers.

use super::*;
use source_ui::shell::{self, Host};

impl Console {
    pub fn modal_dialog_open(&self) -> bool {
        self.confirm_box.is_some()
            || self
                .loading_dialog
                .as_ref()
                .is_some_and(|l| !l.is_background())
    }

    /// The focused layer: a window ([`Mode::Options`], [`Mode::Console`]) or the base.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Moves the UI to `mode`: a window mode opens that window on top with the focus (without
    /// resetting its contents); a base mode (menus, gameplay) replaces the base and closes
    /// every window.
    pub fn set_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Options => {
                if !self.background_level && self.base == Mode::Gameplay {
                    self.base = Mode::Pause;
                }
                self.shell.windows.open(Window::Options);
            }
            Mode::Console => {
                if !self.background_level && self.base == Mode::Gameplay {
                    self.base = Mode::Pause;
                }
                self.shell.windows.open(Window::Console);
            }
            base => {
                self.base = base;
                self.shell.windows.clear();
            }
        }
        self.sync_mode();
    }

    /// Keeps `mode` on the focused layer after the windows changed.
    pub(super) fn sync_mode(&mut self) {
        self.mode = match self.shell.windows.focus() {
            Layer::Window(Window::Options | Window::SubDialog) => Mode::Options,
            Layer::Window(Window::Console) => Mode::Console,
            Layer::Window(Window::Confirmation) => {
                if self.options_open() {
                    Mode::Options
                } else {
                    self.base
                }
            }
            Layer::Base => self.base,
        };
    }

    /// Synchronizes sub-dialog and confirmation modal windows in `self.shell.windows`.
    pub(crate) fn sync_dialog_windows(&mut self) {
        let closed = self.shell.windows.clean_closed();
        for w in closed {
            match w {
                Window::SubDialog => self.options.closing_sub = None,
                Window::Confirmation => {
                    self.options.closing_confirm = None;
                    self.closing_confirm_box = None;
                }
                _ => {}
            }
        }

        let has_sub = self.options.has_sub();
        let has_confirm = self.confirm_box.is_some() || self.options.has_confirm();

        if has_sub && !self.shell.windows.is_active(Window::SubDialog) {
            self.shell.windows.open(Window::SubDialog);
        } else if !has_sub && self.shell.windows.is_active(Window::SubDialog) {
            self.shell.windows.close(Window::SubDialog);
        }

        if has_confirm && !self.shell.windows.is_active(Window::Confirmation) {
            self.shell.windows.open(Window::Confirmation);
        } else if !has_confirm && self.shell.windows.is_active(Window::Confirmation) {
            self.shell.windows.close(Window::Confirmation);
        }

        let time = self.ui_clock;
        self.options.sub_alpha = self
            .shell
            .windows
            .window_composite_alpha(Window::SubDialog, time);
        self.options.confirm_alpha = self
            .shell
            .windows
            .window_composite_alpha(Window::Confirmation, time);
        self.options.sub_theme = if self.shell.windows.is_open(Window::SubDialog) {
            Some(self.window_theme(Window::SubDialog, time))
        } else {
            None
        };
        self.options.confirm_theme = if self.shell.windows.is_open(Window::Confirmation) {
            Some(self.window_theme(Window::Confirmation, time))
        } else {
            None
        };
        self.options.parent_focused = self.shell.windows.focus() == Layer::Window(Window::Options);

        self.sync_mode();
    }

    /// The open windows, bottom to top.
    pub fn open_windows(&self) -> Vec<Window> {
        self.shell.windows.active_order().collect()
    }

    /// The developer console's frame `[x, y, width, height]` in canvas pixels while it is open,
    /// for run reports.
    pub fn console_frame_report(&self) -> Option<[f32; 4]> {
        self.console_open().then(|| {
            let r = self.window_bounds(Window::Console);
            [r.x, r.y, r.w, r.h]
        })
    }

    /// The developer console is open (focused or not).
    pub fn console_open(&self) -> bool {
        self.shell.windows.is_active(Window::Console)
    }

    /// The menu under the windows takes the pointer (gameplay under the console does not).
    pub(super) fn base_is_menu(&self) -> bool {
        matches!(self.base, Mode::MainMenu | Mode::Pause)
    }

    pub(super) fn close_window(&mut self, window: Window) {
        if window == Window::Console {
            self.shell.panel.placement.end();
        }
        self.shell.escape_held = false;
        self.options.escape_held = false;
        if let Some((box_, _)) = &mut self.confirm_box {
            box_.escape_held = false;
        }
        self.options.set_confirm_escape_held(false);
        self.shell.windows.close(window);
        self.reset_menu_interaction();
        self.sync_mode();
    }

    /// A window's frame look: faded towards its focused look (SDK `Frame` focus).
    pub(super) fn window_theme(&self, window: Window, time: f64) -> Theme {
        self.theme.focus_faded(
            self.shell.windows.focus_amount(window, time),
            self.shell.windows.focus() == Layer::Window(window),
        )
    }

    /// Whether `point` (canvas pixels) is on a window: its frame, or for the console also its
    /// completion popup, which can reach past the frame.
    pub(super) fn window_contains(&self, window: Window, point: Vec2) -> bool {
        self.window_bounds(window).contains(point)
            || window == Window::Console
                && self.completion_bounds().is_some_and(|r| r.contains(point))
    }

    /// The console's completion popup on the canvas, while it shows.
    pub(super) fn completion_bounds(&self) -> Option<Rect> {
        self.shell
            .panel
            .completion_bounds(&self.canvas, &self.theme, commands::COMMANDS, self)
    }

    /// A window's frame on the canvas, in canvas pixels.
    pub fn window_bounds(&self, window: Window) -> Rect {
        let canvas = vec2(self.canvas.width(), self.canvas.height());
        match window {
            Window::Options => self.options.frame_bounds(canvas),
            Window::Console => {
                let layout = self.shell.panel.dialog_layout(&self.canvas);
                Rect::new(
                    layout.origin.x,
                    layout.origin.y,
                    layout.size.x * layout.scale,
                    layout.size.y * layout.scale,
                )
            }
            Window::SubDialog => self.options.sub_bounds(canvas),
            Window::Confirmation => {
                if let Some((confirm, _)) = &self.confirm_box {
                    let frame = self.quit_frame(confirm);
                    Rect::new(
                        frame.origin.x,
                        frame.origin.y,
                        frame.size.x * frame.scale,
                        frame.size.y * frame.scale,
                    )
                } else if self.options.has_confirm() {
                    self.options.confirm_bounds(canvas)
                } else {
                    Rect::default()
                }
            }
        }
    }

    /// Bounding rectangle in canvas pixels of the currently active modal dialog, if any.
    pub fn modal_bounds(&self) -> Option<Rect> {
        if let Some(loading) = &self.loading_dialog {
            if !loading.is_background() {
                let scale = crate::proportional::dialog_scale(self.canvas_size());
                let canvas = vec2(self.canvas.width(), self.canvas.height());
                let (origin, _) =
                    crate::gameui::loading::LoadingDialog::layout_geometry(canvas, scale);
                let size = crate::gameui::loading::LOADING_DIALOG_SIZE * scale;
                return Some(Rect::new(origin.x, origin.y, size.x, size.y));
            }
        }
        if let Some((confirm, _)) = &self.confirm_box {
            let (origin, scale, layout) = self.quit_geometry(confirm);
            return Some(Rect::new(
                origin.x,
                origin.y,
                layout.size.x * scale,
                layout.size.y * scale,
            ));
        }
        None
    }

    /// Whether a point in canvas coordinates is covered by a modal dialog or an open window.
    pub fn is_occluded(&self, point: Vec2) -> bool {
        self.modal_bounds().is_some_and(|b| b.contains(point))
            || self
                .shell
                .windows
                .order()
                .any(|w| self.window_contains(w, point))
    }

    pub fn paused(&self) -> bool {
        self.mode != Mode::Gameplay
    }

    /// Escape went down. When it would close a dialog that has a close button (Options or one
    /// of its sub-dialogs, the console) or a confirmation dialog with a Cancel button, that
    /// button shows pressed/armed until Escape is released and the dialog closes then.
    /// Everything else Escape does happens at once.
    pub fn escape_pressed(&mut self) {
        self.sync_dialog_windows();
        if self.escape_closes_dialog() {
            self.shell.escape_held = true;
            self.options.escape_held = matches!(
                self.shell.windows.top(),
                Some(Window::Options | Window::SubDialog)
            );
            let is_confirm = self.shell.windows.top() == Some(Window::Confirmation);
            if let Some((box_, _)) = &mut self.confirm_box {
                box_.escape_held = is_confirm;
            }
            self.options.set_confirm_escape_held(is_confirm);
        } else {
            self.escape();
        }
    }

    /// Escape went up: closes the dialog whose close button or cancel button it held.
    pub fn escape_released(&mut self) {
        self.sync_dialog_windows();
        if std::mem::take(&mut self.shell.escape_held) {
            self.options.escape_held = false;
            if let Some((box_, _)) = &mut self.confirm_box {
                box_.escape_held = false;
            }
            self.options.set_confirm_escape_held(false);
            if self.escape_closes_dialog() {
                self.escape();
            }
        }
    }

    pub(super) fn escape_closes_dialog(&self) -> bool {
        if self.loading_dialog.is_some() || self.quit_placement.active() {
            return false;
        }
        match self.shell.windows.top() {
            Some(Window::Options | Window::SubDialog) => self.options.escape_closes(),
            Some(Window::Console) => {
                !self.shell.panel.placement.active() && !self.shell.panel.editor.can_dismiss()
            }
            Some(Window::Confirmation) => {
                if self.confirm_box.is_some() {
                    !self.quit_placement.active()
                } else {
                    self.options.escape_closes()
                }
            }
            None => false,
        }
    }

    pub fn escape(&mut self) {
        // A drag or resize in progress is undone first.
        let console_on_top = self.shell.windows.top() == Some(Window::Console);
        if self.quit_placement.cancel() || (console_on_top && self.shell.panel.placement.cancel()) {
            return;
        }
        // In the console Escape first ends a history search, then hides the completion list.
        if console_on_top && self.shell.panel.editor.dismiss() {
            return;
        }
        if let Some(loading) = &self.loading_dialog {
            if loading.is_failed() {
                self.loading_dialog = None;
                self.set_mode(Mode::MainMenu);
                self.main_menu.back(&self.gameui.localization);
                self.clear_menu_hover();
            }
            return;
        }
        if let Some((mut box_, kind)) = self.confirm_box.take() {
            box_.escape_held = false;
            self.closing_confirm_box = Some((box_, kind.clone()));
            self.reset_menu_interaction();
            if let ConfirmKind::KeepVideo(keep) = kind {
                let effect = self.revert_video(keep.previous);
                self.deferred.push(effect);
            }
            self.sync_dialog_windows();
            return;
        }
        // Escape closes the top window first, whichever layer has the focus.
        match self.shell.windows.top() {
            Some(Window::Confirmation) => {
                if !self.options.escape_closes() {
                    self.options.escape();
                } else {
                    self.options.dismiss_confirm();
                }
                self.sync_dialog_windows();
                return;
            }
            Some(Window::SubDialog) => {
                self.options.escape();
                self.sync_dialog_windows();
                return;
            }
            Some(Window::Options) => {
                if !self.options.escape() {
                    self.close_window(Window::Options);
                }
                return;
            }
            Some(Window::Console) => {
                self.close_window(Window::Console);
                self.clear_menu_hover();
                return;
            }
            None => {}
        }
        let menu = if self.background_level {
            Mode::MainMenu
        } else {
            Mode::Pause
        };
        let base = match self.base {
            Mode::Gameplay => menu,
            Mode::Pause => {
                if self.background_level {
                    Mode::MainMenu
                } else {
                    Mode::Gameplay
                }
            }
            Mode::MainMenu => {
                if self.main_menu.back(&self.gameui.localization) || self.background_level {
                    Mode::MainMenu
                } else {
                    Mode::Gameplay
                }
            }
            // Windows are never the base.
            window @ (Mode::Console | Mode::Options) => window,
        };
        self.set_mode(base);
    }

    /// Opens the console on top with the focus, or closes it (focused or not); it opens only
    /// while `con_enable` is on. Blocked when a modal dialog or blocking subdialog is open.
    pub fn toggle(&mut self) {
        if self.shell.windows.has_modal()
            || self.modal_dialog_open()
            || self.options.has_sub()
            || self.options.has_confirm()
        {
            return;
        }
        if self.console_open() {
            self.close_window(Window::Console);
            self.clear_menu_hover();
        } else if self.config.keyboard.developer_console {
            self.clear_menu_hover();
            if !self.background_level && self.base == Mode::Gameplay {
                self.base = Mode::Pause;
            }
            self.shell.windows.open(Window::Console);
            self.sync_mode();
            let frame = self.shell.panel.frame(&self.canvas);
            self.shell.panel.placement.reset_if_off_screen(&frame);
        }
    }

    pub fn input(&mut self, input: &Input) -> Vec<Effect> {
        let mut effects = std::mem::take(&mut self.deferred);
        effects.extend(shell::input(self, input));
        // A drag, resize or reset that ended this frame is saved at once.
        self.save_placements();
        effects
    }

    /// Saves the Options dialog's and the console's placement once a drag, resize or reset changed
    /// it (the previous input frame's).
    pub(super) fn save_placements(&mut self) {
        let mut changed = false;
        if let Some(options) = self.options.take_placement_changed() {
            self.config.dialogs.options = options;
            changed = true;
        }
        if self.shell.panel.placement.take_changed() {
            self.config.dialogs.console = self.shell.panel.placement.saved();
            changed = true;
        }
        if changed {
            if let Err(error) = self.config.save() {
                self.log(format!("Could not save settings: {error}"));
            }
        }
    }

    /// The pointer shape the window should show: a resize cursor over or while dragging one of the
    /// console's corner grips, otherwise the arrow.
    pub fn cursor_hint(&self) -> CursorHint {
        if !self.console_open() || self.confirm_box.is_some() {
            return CursorHint::Arrow;
        }
        self.shell
            .panel
            .placement
            .hint(&self.shell.panel.frame(&self.canvas), self.shell.pointer)
    }
}

/// HL2's answers to the shell's control flow: the GameUI's mode, its modal layers (the loading
/// dialog, the quit box), the Options dialog's windows and the menu under them.
impl Host for Console {
    type Window = Window;
    type Effect = Effect;
    type Mode = Mode;
    /// The Options window that takes input this frame (the dialog, its sub-dialog or its
    /// confirmation box).
    type Frame = Window;

    fn shell(&self) -> &Shell<Window> {
        &self.shell
    }

    fn shell_mut(&mut self) -> &mut Shell<Window> {
        &mut self.shell
    }

    fn mode(&self) -> Mode {
        self.mode
    }

    fn capturing_key(&self) -> bool {
        self.options_open() && self.options.capturing_key()
    }

    fn modal_input(&mut self, input: &Input) -> Option<Vec<Effect>> {
        if self.modal_dialog_open() {
            // The menus are occluded: keep their tracker on the physical button so the rest of
            // a click that closes the modal layer is not read as a new menu press.
            self.menu_tracker.occlude(input.click || input.held);
        }
        if let Some(loading) = &self.loading_dialog {
            if loading.is_failed() && (input.pressed(Key::Enter) || input.click) {
                self.loading_dialog = None;
                self.set_mode(Mode::MainMenu);
                self.main_menu.back(&self.gameui.localization);
                self.clear_menu_hover();
            }
            return Some(Vec::new());
        }
        if self.confirm_box.is_some() {
            return Some(self.confirm_input(input));
        }
        None
    }

    /// A modal layer inside Options (sub-dialog, its confirmation box, an open list, a key
    /// capture) takes every click.
    fn prepare_windows(&mut self) -> (Option<Window>, Window) {
        self.sync_dialog_windows();
        let active_options_window = if self.confirm_box.is_none()
            && self.options.has_confirm()
            && self.shell.windows.is_open(Window::Confirmation)
        {
            Window::Confirmation
        } else if self.options.has_sub() && self.shell.windows.is_open(Window::SubDialog) {
            Window::SubDialog
        } else {
            Window::Options
        };
        let modal = if self.options_open() && self.options.modal_layer_open() {
            Some(active_options_window)
        } else {
            None
        };
        (modal, active_options_window)
    }

    fn window_bounds(&self, window: Window) -> Rect {
        Console::window_bounds(self, window)
    }

    fn console_popup_bounds(&self) -> Option<Rect> {
        self.completion_bounds()
    }

    fn base_takes_pointer(&self) -> bool {
        self.base_is_menu()
    }

    fn focus_changed(&mut self) {
        self.sync_mode();
    }

    fn window_input(
        &mut self,
        active_options_window: &Window,
        window: Window,
        share: &Input,
        input: &Input,
    ) -> Vec<Effect> {
        match window {
            Window::Options | Window::SubDialog => {
                if window == *active_options_window {
                    self.options_input(share)
                } else {
                    Vec::new()
                }
            }
            Window::Console => self.console_input(share),
            Window::Confirmation => {
                if self.confirm_box.is_some() {
                    self.confirm_input(input)
                } else if window == *active_options_window {
                    self.options_input(share)
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn input_blocked(&self) -> bool {
        self.confirm_box.is_some() || self.modal_dialog_open()
    }

    fn occlude_base(&mut self, down: bool) {
        self.menu_tracker.occlude(down);
    }

    fn base_input(&mut self, share: &Input, owns_pointer: bool) -> Vec<Effect> {
        self.menu_input(share, owns_pointer)
    }
}
