//! Windows and focus: the console's single-focus window stack (Options, the developer
//! console, their modal layers), Escape, and routing each frame's input to the layers.

use super::*;

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
                self.windows.open(Window::Options);
            }
            Mode::Console => {
                if !self.background_level && self.base == Mode::Gameplay {
                    self.base = Mode::Pause;
                }
                self.windows.open(Window::Console);
            }
            base => {
                self.base = base;
                self.windows.clear();
            }
        }
        self.sync_mode();
    }

    /// Keeps `mode` on the focused layer after the windows changed.
    pub(super) fn sync_mode(&mut self) {
        self.mode = match self.windows.focus() {
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

    /// Synchronizes sub-dialog and confirmation modal windows in `self.windows`.
    pub(crate) fn sync_dialog_windows(&mut self) {
        let closed = self.windows.clean_closed();
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

        if has_sub && !self.windows.is_active(Window::SubDialog) {
            self.windows.open(Window::SubDialog);
        } else if !has_sub && self.windows.is_active(Window::SubDialog) {
            self.windows.close(Window::SubDialog);
        }

        if has_confirm && !self.windows.is_active(Window::Confirmation) {
            self.windows.open(Window::Confirmation);
        } else if !has_confirm && self.windows.is_active(Window::Confirmation) {
            self.windows.close(Window::Confirmation);
        }

        let time = self.ui_clock;
        self.options.sub_alpha = self.windows.window_composite_alpha(Window::SubDialog, time);
        self.options.confirm_alpha = self
            .windows
            .window_composite_alpha(Window::Confirmation, time);
        self.options.sub_theme = if self.windows.is_open(Window::SubDialog) {
            Some(self.window_theme(Window::SubDialog, time))
        } else {
            None
        };
        self.options.confirm_theme = if self.windows.is_open(Window::Confirmation) {
            Some(self.window_theme(Window::Confirmation, time))
        } else {
            None
        };
        self.options.parent_focused = self.windows.focus() == Layer::Window(Window::Options);

        self.sync_mode();
    }

    /// The open windows, bottom to top.
    pub fn open_windows(&self) -> Vec<Window> {
        self.windows.active_order().collect()
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
        self.windows.is_active(Window::Console)
    }

    /// The menu under the windows takes the pointer (gameplay under the console does not).
    pub(super) fn base_is_menu(&self) -> bool {
        matches!(self.base, Mode::MainMenu | Mode::Pause)
    }

    pub(super) fn close_window(&mut self, window: Window) {
        if window == Window::Console {
            self.panel.placement.end();
        }
        self.escape_held = false;
        self.options.escape_held = false;
        if let Some((box_, _)) = &mut self.confirm_box {
            box_.escape_held = false;
        }
        self.options.set_confirm_escape_held(false);
        self.windows.close(window);
        self.reset_menu_interaction();
        self.sync_mode();
    }

    /// A window's frame look: faded towards its focused look (SDK `Frame` focus).
    pub(super) fn window_theme(&self, window: Window, time: f64) -> Theme {
        self.theme.focus_faded(
            self.windows.focus_amount(window, time),
            self.windows.focus() == Layer::Window(window),
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
        self.panel
            .completion_bounds(&self.canvas, &self.theme, commands::COMMANDS, self)
    }

    /// A window's frame on the canvas, in canvas pixels.
    pub fn window_bounds(&self, window: Window) -> Rect {
        let canvas = vec2(self.canvas.width(), self.canvas.height());
        match window {
            Window::Options => self.options.frame_bounds(canvas),
            Window::Console => {
                let layout = self.panel.dialog_layout(&self.canvas);
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
            || self.windows.order().any(|w| self.window_contains(w, point))
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
            self.escape_held = true;
            self.options.escape_held = matches!(
                self.windows.top(),
                Some(Window::Options | Window::SubDialog)
            );
            let is_confirm = self.windows.top() == Some(Window::Confirmation);
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
        if std::mem::take(&mut self.escape_held) {
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
        match self.windows.top() {
            Some(Window::Options | Window::SubDialog) => self.options.escape_closes(),
            Some(Window::Console) => {
                !self.panel.placement.active() && !self.panel.editor.can_dismiss()
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
        let console_on_top = self.windows.top() == Some(Window::Console);
        if self.quit_placement.cancel() || (console_on_top && self.panel.placement.cancel()) {
            return;
        }
        // In the console Escape first ends a history search, then hides the completion list.
        if console_on_top && self.panel.editor.dismiss() {
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
        match self.windows.top() {
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
        if self.windows.has_modal()
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
            self.windows.open(Window::Console);
            self.sync_mode();
            let frame = self.panel.frame(&self.canvas);
            self.panel.placement.reset_if_off_screen(&frame);
        }
    }

    pub fn input(&mut self, input: &Input) -> Vec<Effect> {
        let mut effects = std::mem::take(&mut self.deferred);
        effects.extend(self.input_frame(input));
        // A drag, resize or reset that ended this frame is saved at once.
        self.save_placements();
        effects
    }

    pub(super) fn input_frame(&mut self, input: &Input) -> Vec<Effect> {
        // Held keys repeat for every layer below, except into a key binding being captured.
        let mut repeated = input.clone();
        if self.options_open() && self.options.capturing_key() {
            self.key_repeat.cancel();
        } else {
            self.key_repeat.apply(&mut repeated);
        }
        let mode = self.mode;
        let effects = self.layer_input(&repeated);
        if self.mode != mode {
            self.key_repeat.cancel();
        }
        effects
    }

    pub(super) fn layer_input(&mut self, input: &Input) -> Vec<Effect> {
        if let Some(time) = input.time {
            self.windows.set_time(time);
        }
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
            return Vec::new();
        }
        if self.confirm_box.is_some() {
            return self.confirm_input(input);
        }
        self.windows_input(input)
    }

    /// The Options dialog and the console are not modal (retail VGUI frames): the pointer goes
    /// to the topmost window under it, else to the menu behind them; a press raises and focuses
    /// its layer and stays with it until release; the keyboard goes to the focused layer. A
    /// modal layer inside Options (sub-dialog, its confirmation box, an open list, a key
    /// capture) takes every click. Every layer sees every frame, with only its share of it.
    pub(super) fn windows_input(&mut self, input: &Input) -> Vec<Effect> {
        self.sync_dialog_windows();
        let active_options_window = if self.confirm_box.is_none()
            && self.options.has_confirm()
            && self.windows.is_open(Window::Confirmation)
        {
            Window::Confirmation
        } else if self.options.has_sub() && self.windows.is_open(Window::SubDialog) {
            Window::SubDialog
        } else {
            Window::Options
        };
        let modal = if self.options_open() && self.options.modal_layer_open() {
            Some(active_options_window)
        } else {
            None
        };
        let completion = self.completion_bounds();
        let bounds: Vec<(Window, Rect)> = self
            .windows
            .order()
            .map(|w| (w, self.window_bounds(w)))
            .collect();
        let pointer = Pointer {
            at: input.pointer,
            click: input.click,
            held: input.held,
            released: input.released,
        };
        let base_takes_pointer = self.base_is_menu();
        let owner = self
            .windows
            .route(pointer, modal, base_takes_pointer, |window, at| {
                bounds.iter().any(|(w, r)| *w == window && r.contains(at))
                    || window == Window::Console && completion.is_some_and(|r| r.contains(at))
            });
        self.sync_mode();
        let focus = self.windows.focus();
        let mut effects = Vec::new();
        let windows: Vec<Window> = self.windows.order().collect();
        for window in windows {
            if !self.windows.is_open(window) {
                continue;
            }
            let layer = Layer::Window(window);
            let share = input.share(owner == Some(layer), focus == layer);
            effects.extend(match window {
                Window::Options | Window::SubDialog => {
                    if window == active_options_window {
                        self.options_input(&share)
                    } else {
                        Vec::new()
                    }
                }
                Window::Console => self.console_input(&share),
                Window::Confirmation => {
                    if self.confirm_box.is_some() {
                        self.confirm_input(input)
                    } else if window == active_options_window {
                        self.options_input(&share)
                    } else {
                        Vec::new()
                    }
                }
            });
            if self.confirm_box.is_some() || self.modal_dialog_open() {
                return effects;
            }
        }
        if self.base_is_menu() {
            let owns = owner == Some(Layer::Base);
            if !owns {
                // A window has the pointer: the menu only follows the button (occluded).
                self.menu_tracker.occlude(input.click || input.held);
            }
            let focus = self.windows.focus();
            let share = input.share(owns, focus == Layer::Base);
            effects.extend(self.menu_input(&share, owns));
        }
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
        if self.panel.placement.take_changed() {
            self.config.dialogs.console = self.panel.placement.saved();
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
        self.panel
            .placement
            .hint(&self.panel.frame(&self.canvas), self.pointer)
    }
}
