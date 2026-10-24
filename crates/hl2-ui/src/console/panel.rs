//! The developer console panel: output buffer, command line, history, completion list,
//! scrolling, selection and drawing.

use super::*;
use source_ui::console::command;

impl Console {
    pub fn log(&mut self, text: impl Into<String>) {
        self.print(Level::Normal, text);
    }

    /// Updates the most recent live viewmodel tuning snapshot instead of growing scrollback.
    pub fn log_viewmodel_tuning(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some((index, _)) = self
            .output
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| line.level == Level::ViewmodelTuning)
        {
            let number = self.output_start + index as u64;
            if let Some(line) = self.output.get_mut(index) {
                line.text = text;
                line.time = self.clock;
            }
            if let Ok(mut wrapped) = self.wrapped.lock() {
                wrapped.remove(&number);
            }
        } else {
            self.print(Level::ViewmodelTuning, text);
        }
    }

    /// Prints a line at `level`, stamped with the current time.
    pub fn print(&mut self, level: Level, text: impl Into<String>) {
        self.output.push_back(Line {
            level,
            text: text.into(),
            time: self.clock,
        });
        if self.output.len() > OUTPUT_LINES {
            self.output.pop_front();
            self.output_start += 1;
        }
        // Scrolled back, the view stays on its rows while new ones arrive below.
        let number = self.output_start + self.output.len() as u64 - 1;
        if let Some(line) = self
            .output
            .back()
            .filter(|line| self.scroll > 0 && (self.con_filter != 1 || self.filter_match(line)))
        {
            self.scroll += self.line_rows(number, line, self.wrap_metrics()).len();
        }
    }

    pub fn submit(&mut self, line: &str) -> Vec<Effect> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        self.print(Level::Input, format!("] {line}"));
        let commands = match command::tokenize(line) {
            Ok(commands) => commands,
            Err(error) => {
                self.log(error);
                return Vec::new();
            }
        };
        let mut effects = Vec::new();
        for tokens in commands {
            match self.interpret(&tokens) {
                Ok(Some(effect)) => effects.push(effect),
                Ok(None) => (),
                Err(error) => self.log(error),
            }
        }
        effects
    }

    /// Runs one command through its [`commands::Spec`] handler.
    pub(super) fn interpret(&mut self, tokens: &[String]) -> commands::Outcome {
        command::run(self, commands::COMMANDS, tokens)
    }

    /// The current value of an implemented cvar, as the console prints it.
    pub fn cvar_value(&self, name: &str) -> Option<String> {
        commands::find(name)?.value?(self)
    }

    /// The text being typed on the console line.
    pub fn input_line(&self) -> &str {
        self.editor.text()
    }

    /// Names that command arguments complete from (installed maps, the loaded map's entities).
    pub fn set_completion_sources(&mut self, sources: complete::Sources) {
        self.completion = sources;
        self.editor.refresh(&self.completion);
    }

    /// Restores the command history the host saved.
    pub fn set_history(&mut self, history: Vec<String>) {
        self.editor.set_history(history);
    }

    /// The command history, once after it changed (the host saves it then).
    pub fn take_changed_history(&mut self) -> Option<Vec<String>> {
        self.editor
            .take_history_changed()
            .then(|| self.editor.history().to_vec())
    }

    pub(super) fn console_input(&mut self, input: &Input) -> Vec<Effect> {
        if let Some(time) = input.time {
            self.clock = time;
        }
        // The caption and corner grips own the pointer before the history's row selection.
        self.pointer = input.pointer;
        let frame = self.console_frame();
        let placing = self.console_placement.input(
            &frame,
            input.pointer,
            input.click,
            input.held,
            input.time,
        );
        if !placing {
            if let Some(effects) = self.console_pointer(input) {
                return effects;
            }
        }
        let ctrl_key = |name: &str| input.ctrl && input.binding_keys.iter().any(|k| k == name);
        let editor = &mut self.editor;
        if !input.ctrl {
            let typed: String = input
                .text
                .chars()
                .filter(|c| *c != '`' && *c != '~')
                .collect();
            editor.insert(&typed);
        }
        if let Some(paste) = &input.paste {
            editor.paste(paste);
        }
        if ctrl_key("KeyR") {
            editor.search_back();
        }
        if ctrl_key("KeyU") {
            editor.kill_to_start();
        }
        if ctrl_key("KeyK") {
            editor.kill_to_end();
        }
        if ctrl_key("KeyW") {
            editor.delete_word_back();
        }
        if ctrl_key("KeyA") {
            editor.home();
        }
        if ctrl_key("KeyE") {
            editor.end();
        }
        if input.pressed(Key::Backspace) {
            if input.ctrl {
                editor.delete_word_back();
            } else {
                editor.backspace();
            }
        }
        if input.pressed(Key::Delete) {
            if input.ctrl {
                editor.delete_word_forward();
            } else {
                editor.delete();
            }
        }
        if input.pressed(Key::Left) {
            if input.ctrl {
                editor.word_left();
            } else {
                editor.left();
            }
        }
        if input.pressed(Key::Right) {
            if input.ctrl {
                editor.word_right();
            } else {
                editor.right();
            }
        }
        if input.pressed(Key::Home) {
            if input.ctrl {
                self.scroll = self.history_rows().len();
            } else {
                self.editor.home();
            }
        }
        if input.pressed(Key::End) {
            if input.ctrl {
                self.scroll = 0;
            } else {
                self.editor.end();
            }
        }
        if input.pressed(Key::PageUp) {
            self.scroll = (self.scroll + 10).min(self.history_rows().len().saturating_sub(1));
        }
        if input.pressed(Key::PageDown) {
            self.scroll = self.scroll.saturating_sub(10);
        }
        // Completion follows the edited line before Tab/Up/Down step through it.
        self.editor.refresh(&self.completion);
        if input.pressed(Key::Up) {
            self.editor.step(true, false);
        }
        if input.pressed(Key::Down) {
            self.editor.step(false, false);
        }
        if input.pressed(Key::Tab) {
            self.editor.step(input.shift, true);
            self.editor.refresh(&self.completion);
        }
        if ctrl_key("KeyL") {
            self.clear_output();
        }
        if input.copy {
            self.copied = self.selected_text();
        }
        if input.pressed(Key::Enter) {
            let line = self.editor.submit();
            self.scroll = 0;
            return self.submit(&line);
        }
        Vec::new()
    }

    /// Pointer input of the console dialog: the close and Submit buttons, the history scroll
    /// bar, the wheel, and dragging over history rows to select them. `Some` when a button
    /// ended this frame's input (the console closed, or the line was submitted).
    pub(super) fn console_pointer(&mut self, input: &Input) -> Option<Vec<Effect>> {
        let layout = self.dialog_layout();
        let view = HistoryView::new(&layout);
        let point = input.pointer.map(|p| layout.local(p));
        let down = input.click || input.held;
        let rows: Vec<u64> = self.history_rows().iter().map(|row| row.number).collect();
        let total = rows.len();
        let max_scroll = total.saturating_sub(view.rows);
        let mode = ScrollMode::platform();
        let steps = self.history_wheel.steps(input.wheel, mode) as i64
            * mode.rows_per_step(WHEEL_ROWS) as i64;
        self.scroll = (self.scroll as i64 + steps).clamp(0, max_scroll as i64) as usize;
        self.scroll = self.scroll.min(max_scroll);
        let first = total - view.rows.min(total) - self.scroll;
        let bar = ScrollBarLayout::new(view.bar, first, view.rows, total);
        if !self.selecting && self.history_bar.wants(&bar, point, input.click) {
            if let Some(first) = self.history_bar.pointer(
                &bar,
                point,
                down,
                input.click,
                input.time,
                first,
                view.rows,
            ) {
                self.scroll = max_scroll.saturating_sub(first);
            }
            self.console_buttons.occlude(down);
            self.console_hover = None;
            return None;
        }
        let hovered = point.and_then(|p| {
            [
                (ConsoleButton::Close, layout.close),
                (ConsoleButton::Submit, layout.submit),
            ]
            .into_iter()
            .find(|(_, rect)| rect.contains(p))
            .map(|(button, _)| button)
        });
        if self.selecting {
            self.console_buttons.occlude(down);
            self.console_hover = None;
        } else {
            self.console_hover = hovered;
            match self
                .console_buttons
                .update(hovered, down, input.click, input.released)
            {
                Some(ConsoleButton::Close) => {
                    self.console_hover = None;
                    self.toggle();
                    return Some(vec![Effect::PlaySound(RELEASE_SOUND.into())]);
                }
                Some(ConsoleButton::Submit) => {
                    let line = self.editor.submit();
                    self.scroll = 0;
                    let mut effects = vec![Effect::PlaySound(RELEASE_SOUND.into())];
                    effects.extend(self.submit(&line));
                    return Some(effects);
                }
                None => {}
            }
        }
        if let Some(p) = point {
            if input.click && hovered.is_none() && view.text.contains(p) {
                self.selecting = true;
                self.selection = view
                    .row_at(p.y, false)
                    .and_then(|row| rows.get(first + row))
                    .map(|number| Selection::at(*number));
            } else if self.selecting && down {
                // Dragging past the top or bottom scrolls the history a row per frame.
                if p.y < view.text.y {
                    self.scroll = (self.scroll + 1).min(max_scroll);
                } else if p.y > view.text.y + view.text.h {
                    self.scroll = self.scroll.saturating_sub(1);
                }
                let first = total - view.rows.min(total) - self.scroll;
                let row = view.row_at(p.y, true).unwrap_or(0);
                let line = rows.get(first + row).or(rows.last()).copied();
                if let (Some(selection), Some(line)) = (self.selection.as_mut(), line) {
                    selection.head = line;
                }
            }
        }
        if !down {
            self.selecting = false;
        }
        None
    }

    /// Empties the history (`clear`, Ctrl+L). Line numbers keep counting, so nothing printed
    /// later can take a selected line's number.
    pub(super) fn clear_output(&mut self) {
        self.output_start += self.output.len() as u64;
        self.output.clear();
        self.scroll = 0;
        self.selection = None;
        self.selecting = false;
    }

    /// The console dialog's frame: retail placement and minimum size, sizeable by its corners.
    pub(super) fn console_frame(&self) -> placement::Frame {
        let canvas = vec2(self.canvas.width(), self.canvas.height());
        let (origin, size) = DialogLayout::default_frame(canvas);
        placement::Frame {
            origin,
            size,
            scale: crate::proportional::dialog_scale(canvas),
            canvas,
            min_size: Some(dialog::MIN_SIZE),
            close: source_ui::widgets::close_button_rect,
        }
    }

    pub(super) fn dialog_layout(&self) -> DialogLayout {
        let frame = self.console_frame();
        let placed = self.console_placement.place(&frame);
        DialogLayout::at(frame.scale, placed.origin, placed.size)
    }

    /// The output the history shows (`con_filter_enable` 1 hides lines without
    /// `con_filter_text`), with each line's number.
    pub(super) fn shown_lines(&self) -> Vec<(u64, &Line)> {
        (self.output_start..)
            .zip(self.output.iter())
            .filter(|(_, line)| self.con_filter != 1 || self.filter_match(line))
            .collect()
    }

    /// A line as the history shows it, with its time under `con_timestamp`.
    pub(super) fn shown_text(&self, line: &Line) -> String {
        if self.con_timestamp {
            format!("[{:7.2}] {}", line.time, line.text)
        } else {
            line.text.clone()
        }
    }

    /// The history's text width and font size in canvas pixels, which rows wrap to.
    pub(super) fn wrap_metrics(&self) -> WrapMetrics {
        let layout = self.dialog_layout();
        WrapMetrics {
            width: layout.canvas(HistoryView::new(&layout).text).w,
            em: (CONSOLE_TEXT * layout.scale) as u16 as f32,
            font: self.mono.is_some(),
        }
    }

    /// Advance of `c` in the console font. A tab is eight spaces wide (SDK `RichText`); without
    /// the font every character takes a nominal monospace advance.
    pub(super) fn console_advance(&self, c: char, em: f32) -> f32 {
        if c == '\t' {
            return TAB_SPACES as f32 * self.console_advance(' ', em);
        }
        let Some(font) = self.mono.as_ref() else {
            return em * NOMINAL_ADVANCE;
        };
        match self.ascii_table(em) {
            Some(table) if c.is_ascii() => table[c as usize],
            _ => font.advance_em(c, em),
        }
    }

    /// The console font's ASCII advances at `em` (tab already eight spaces), built once per size.
    pub(super) fn ascii_table(&self, em: f32) -> Option<[f32; 128]> {
        let font = self.mono.as_ref()?;
        let mut table = self
            .ascii_advances
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if table.is_none_or(|(size, _)| size != em.to_bits()) {
            let mut advances = [0.; 128];
            for (code, advance) in advances.iter_mut().enumerate() {
                *advance = font.advance_em(char::from(code as u8), em);
            }
            advances[usize::from(b'\t')] = TAB_SPACES as f32 * advances[usize::from(b' ')];
            *table = Some((em.to_bits(), advances));
        }
        table.map(|(_, advances)| advances)
    }

    /// The rows line `number` wraps to, as byte ranges into its [`Self::shown_text`].
    pub(super) fn line_rows(
        &self,
        number: u64,
        line: &Line,
        metrics: WrapMetrics,
    ) -> Vec<Range<usize>> {
        let stamp = self.con_timestamp.then_some(line.time);
        let mut cache = self.wrapped.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = cache.get(&number).filter(|entry| {
            entry.metrics == metrics && entry.stamp == stamp && entry.text == line.text
        }) {
            return entry.rows.clone();
        }
        let table = self.ascii_table(metrics.em);
        let rows = wrap(&self.shown_text(line), metrics.width, |c| match table {
            Some(table) if c.is_ascii() => table[c as usize],
            _ => self.console_advance(c, metrics.em),
        });
        if cache.len() >= 2 * OUTPUT_LINES {
            cache.retain(|n, _| *n >= self.output_start);
        }
        cache.insert(
            number,
            Wrapped {
                metrics,
                stamp,
                text: line.text.clone(),
                rows: rows.clone(),
            },
        );
        rows
    }

    /// The history's rows: the shown lines wrapped to its width at word boundaries, a row for
    /// each line of text that holds newlines.
    pub(super) fn history_rows(&self) -> Vec<HistoryRow<'_>> {
        let metrics = self.wrap_metrics();
        self.shown_lines()
            .into_iter()
            .flat_map(|(number, line)| {
                self.line_rows(number, line, metrics)
                    .into_iter()
                    .map(move |text| HistoryRow { number, line, text })
            })
            .collect()
    }

    /// The selected history lines as text, one line each and whole however they wrap.
    pub(super) fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let lines: Vec<String> = self
            .shown_lines()
            .into_iter()
            .filter(|(number, _)| selection.contains(*number))
            .map(|(_, line)| self.shown_text(line))
            .collect();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// Text the copy shortcut copied from the history, for the host to put on the clipboard.
    pub fn take_copied(&mut self) -> Option<String> {
        self.copied.take()
    }

    pub(super) fn console_button_state(&self, button: ConsoleButton) -> ButtonState {
        let hovered = self.console_hover == Some(button);
        ButtonState {
            armed: self.console_buttons.armable(self.console_hover) == Some(button),
            pressed: (hovered && self.console_buttons.is_pressed(button))
                || (button == ConsoleButton::Close
                    && self.escape_held
                    && self.windows.top() == Some(Window::Console)),
            enabled: true,
            // SDK `CConsolePanel` makes Submit the default button.
            default: button == ConsoleButton::Submit,
        }
    }

    /// Whether `line` passes `con_filter_enable` 1 (and is drawn brighter under 2).
    pub(super) fn filter_match(&self, line: &Line) -> bool {
        self.con_filter_text.is_empty()
            || line
                .text
                .to_lowercase()
                .contains(&self.con_filter_text.to_lowercase())
    }

    pub(super) fn line_color(&self, line: &Line) -> Color {
        if self.con_filter == 2 && !self.con_filter_text.is_empty() && self.filter_match(line) {
            return self.style.dev_text;
        }
        match line.level {
            Level::Input | Level::Normal => self.style.text,
            Level::Developer => self.style.dev_text,
            Level::ViewmodelTuning => self.style.text,
            Level::Warning => WARNING_TEXT,
            Level::Error => ERROR_TEXT,
        }
    }

    /// The console dialog (SDK `CConsoleDialog`): a titled frame with the history, the entry
    /// and the Submit button, then the completion popup over everything.
    #[allow(dead_code)]
    pub(super) fn draw_console(&self, time: f64) {
        self.draw_console_with_occluders(time, &[]);
    }

    pub(super) fn draw_console_with_occluders(&self, time: f64, occluders: &[Rect]) {
        let layout = self.dialog_layout();
        let scale = layout.scale;
        let theme = self.window_theme(Window::Console, time);
        let painter = Painter {
            theme: &theme,
            ..self.painter(layout.origin, scale)
        };
        let em = (CONSOLE_TEXT * scale) as u16 as f32;
        let text = |content: &str, x: f32, y: f32, color: Color| {
            text(
                &self.canvas,
                content,
                vec2(x, y),
                em,
                1.,
                self.mono.as_ref(),
                color,
            );
        };
        let localization = &self.gameui.localization;
        let title = localization.text("#Console_Title");
        let spec = FrameSpec::new(layout.size, &title);

        // Frame background (unclipped):
        painter.frame_background(&spec);

        // Push occluders for console content:
        for occ in occluders {
            self.canvas.push_occluder(*occ);
        }

        // Frame foreground (title, close button):
        painter.frame_foreground(&spec, Some(self.console_button_state(ConsoleButton::Close)));

        // History (`RichText`, `DepressedButtonBorder`): filtered, wrapped, scrolled, one color
        // per level, the rows of selected lines highlighted.
        painter.fill(layout.history, self.style.entry);
        painter.sunken(layout.history);
        let view = HistoryView::new(&layout);
        let history = self.history_rows();
        let rows = view.rows.min(history.len());
        let scroll = self.scroll.min(history.len() - rows);
        let first = history.len() - rows - scroll;
        let area = layout.canvas(view.text);
        let row = CONSOLE_ROW * scale;
        for (i, shown) in history[first..first + rows].iter().enumerate() {
            let top = area.y + i as f32 * row;
            let selected = self.selection.is_some_and(|s| s.contains(shown.number));
            let color = if selected {
                self.canvas
                    .rectangle(area.x, top, area.w, row, self.style.selected_bg);
                self.style.selected_text
            } else {
                self.line_color(shown.line)
            };
            let content = self.shown_text(shown.line)[shown.text.clone()]
                .replace('\t', &" ".repeat(TAB_SPACES));
            text(&content, area.x, top + row - CONSOLE_DESCENT * scale, color);
        }
        painter.scroll_bar(
            view.bar,
            first,
            view.rows,
            history.len(),
            self.history_bar.pressed(),
        );

        // The entry (`TextEntry`), or the reverse history search in its place.
        painter.fill(layout.entry, painter.theme.entry_bg);
        painter.sunken(layout.entry);
        let console_focused = self.windows.focus() == Layer::Window(Window::Console);
        let entry_focused = !self.console_placement.active();
        let model = source_ui::widgets::TextInputModel::new(console_focused, entry_focused);
        if model.should_draw_focus_edge() {
            let edge = painter.theme.entry_focus_edge;
            painter.border(
                Rect::new(
                    layout.entry.x + 1.,
                    layout.entry.y + 1.,
                    layout.entry.w - 2.,
                    layout.entry.h - 2.,
                ),
                edge,
                edge,
            );
        }
        let (line, cursor) = match self.editor.search() {
            Some((query, found)) => {
                let label = localization
                    .text(crate::gameui::localize::placeholders::CONSOLE_REVERSE_SEARCH);
                let line = format!("({label})`{query}': {}", found.unwrap_or_default());
                let cursor = line.chars().count();
                (line, cursor)
            }
            None => (self.editor.text().to_owned(), self.editor.cursor()),
        };
        let inner = Rect::new(
            layout.entry.x + 3.,
            layout.entry.y,
            layout.entry.w - 6.,
            layout.entry.h,
        );
        // Scroll the line so the cursor stays inside the entry.
        let chars: Vec<char> = line.chars().collect();
        let width = |from: usize, to: usize| {
            painter.text_width(&chars[from..to].iter().collect::<String>(), false)
        };
        let mut start = 0;
        while start < cursor && width(start, cursor) > inner.w - 2. {
            start += 1;
        }
        let mut end = cursor;
        while end < chars.len() && width(start, end + 1) <= inner.w {
            end += 1;
        }
        let shown: String = chars[start..end].iter().collect();
        painter.text_in(&shown, inner, Align::Left, painter.theme.text, false);
        if model.should_draw_caret(time) {
            let x = inner.x + width(start, cursor) + 1.;
            painter.fill(
                Rect::new(x, inner.y + 4., 1., inner.h - 8.),
                painter.theme.text,
            );
        }
        painter.button(
            layout.submit,
            &localization.text("#Console_Submit"),
            self.console_button_state(ConsoleButton::Submit),
        );
        let entry = layout.canvas(layout.entry);
        self.draw_completion(scale, (entry.x, entry.y, entry.w, entry.h), &text);

        for _ in 0..occluders.len() {
            self.canvas.pop_occluder();
        }
    }

    /// The completion popup (SDK `CConsolePanel` completion list, VGUI menu colors): a help
    /// line for the command being typed, then up to `COMPLETION_ROWS` items, cvars with their
    /// value. It opens below the entry when it fits, else above it.
    /// The completion popup's rows and where it is drawn, if it shows.
    pub(super) fn completion(
        &self,
        scale: f32,
        (x, entry_y, w, entry_h): (f32, f32, f32, f32),
    ) -> Option<Completion> {
        let theme = &self.theme;
        let mut rows: Vec<Vec<(String, Color)>> = Vec::new();
        let mut highlighted = None;
        if self.editor.search().is_none() {
            if let Some(help) = complete::arg_help(self.editor.text()) {
                let mut runs = vec![(help.name.to_owned(), theme.text)];
                for (i, arg) in help.args.iter().enumerate() {
                    let color = if help.current == Some(i) {
                        theme.text
                    } else {
                        theme.text_dull
                    };
                    runs.push((format!(" {arg}"), color));
                }
                if help.cvar {
                    if let Some(value) = self.cvar_value(help.name) {
                        runs.push((format!(" = \"{value}\""), self.style.dev_text));
                    }
                }
                runs.push((format!("   {}", help.description), theme.label_text));
                rows.push(runs);
            }
        }
        let help_rows = rows.len();
        if let Some((items, index)) = self.editor.popup() {
            let more = items.len() > COMPLETION_ROWS;
            let shown = if more {
                COMPLETION_ROWS - 1
            } else {
                items.len()
            };
            // Keep the item stepped to in view.
            let first = index.map_or(0, |i| (i + 1).saturating_sub(shown));
            for (i, item) in items.iter().enumerate().skip(first).take(shown) {
                let mut runs = vec![(item.text.clone(), theme.text)];
                if let Some(value) = commands::find(&item.text)
                    .filter(|spec| spec.is_cvar())
                    .and_then(|spec| self.cvar_value(spec.name))
                {
                    runs.push((format!(" {value}"), self.style.dev_text));
                }
                if !item.detail.is_empty() {
                    runs.push((format!("   {}", item.detail), theme.label_text));
                }
                if index == Some(i) {
                    highlighted = Some(rows.len());
                }
                rows.push(runs);
            }
            if more {
                rows.push(vec![("...".to_owned(), theme.text_dull)]);
            }
        }
        if rows.is_empty() {
            return None;
        }
        // Like a VGUI menu, the list is as wide as its widest row (at least the entry), up to
        // the canvas edge.
        let widest = rows
            .iter()
            .map(|runs| {
                runs.iter()
                    .map(|(content, _)| self.console_text_width(content, scale))
                    .sum::<f32>()
            })
            .fold(0., f32::max);
        let w = w
            .max(widest + 12. * scale)
            .min(self.canvas.width() - x)
            .max(0.);
        let row = CONSOLE_ROW * scale;
        let h = rows.len() as f32 * row + 4. * scale;
        let below = entry_y + entry_h;
        let y = if below + h <= self.canvas.height() {
            below
        } else {
            (entry_y - h).max(0.)
        };
        Some(Completion {
            rows,
            highlighted,
            help_rows,
            rect: Rect::new(x, y, w, h),
        })
    }

    /// The completion popup over everything; it can reach past the console's frame.
    pub(super) fn draw_completion(
        &self,
        scale: f32,
        entry: (f32, f32, f32, f32),
        text: &dyn Fn(&str, f32, f32, Color),
    ) {
        let theme = &self.theme;
        let Some(Completion {
            rows,
            highlighted,
            help_rows,
            rect,
        }) = self.completion(scale, entry)
        else {
            return;
        };
        let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
        let row = CONSOLE_ROW * scale;
        self.canvas.rectangle(
            x,
            y,
            w,
            h,
            Color::new(theme.menu_bg.r, theme.menu_bg.g, theme.menu_bg.b, 1.),
        );
        rectangle_lines(&self.canvas, x, y, w, h, 1., theme.border_dark);
        let max_width = w - 12. * scale;
        for (r, runs) in rows.iter().enumerate() {
            let ry = y + 2. * scale + r as f32 * row;
            if highlighted == Some(r) {
                self.canvas
                    .rectangle(x + 1., ry, w - 2., row, theme.selection_bg);
            }
            if r == help_rows && help_rows > 0 {
                self.canvas
                    .rectangle(x + 1., ry, w - 2., 1., theme.border_dark);
            }
            let mut cx = x + 6. * scale;
            for (content, color) in runs {
                let color = if highlighted == Some(r) {
                    theme.selection_text
                } else {
                    *color
                };
                let mut clipped = String::new();
                for c in content.chars() {
                    clipped.push(c);
                    if cx - x + self.console_text_width(&clipped, scale) > max_width {
                        clipped.pop();
                        break;
                    }
                }
                text(
                    &clipped,
                    cx,
                    ry + (CONSOLE_ROW - CONSOLE_DESCENT) * scale,
                    color,
                );
                cx += self.console_text_width(&clipped, scale);
            }
        }
    }

    pub(super) fn console_text_width(&self, content: &str, scale: f32) -> f32 {
        self.mono.as_ref().map_or(0., |font| {
            font.width_em(content, (CONSOLE_TEXT * scale) as u16 as f32)
        })
    }
}
