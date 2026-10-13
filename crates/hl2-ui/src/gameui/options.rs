//! The Options dialog: a VGUI property sheet whose pages are laid out from the installed
//! `resource/OptionsSub*.res` files and bound to [`Config`] through [`super::settings`].
//!
//! Edits go to a draft. OK returns the draft when it differs from the applied configuration,
//! Apply publishes it and keeps the dialog open, Cancel (or Escape) drops it. Nothing on the
//! Video page is applied before OK/Apply. Dialog geometry follows the SDK `PropertyDialog` /
//! `PropertySheet` rules (see `docs/gameui.md` section 3); the dialog uses the same uniform
//! scale as the other GameUI dialogs.
//!
//! Interaction is modal and layered, innermost first: key capture, open combo list, text entry,
//! confirmation box, sub-dialog (brightness / credits), then the dialog itself. Escape closes the
//! innermost layer; on the dialog it acts as Cancel.

use super::{
    display::DisplayModes,
    keys::{self, KeyRow},
    layout::{Align as LayoutAlign, Control, ControlKind},
    menu::MessageBox,
    resources::{GameUiResources, Image},
    settings::{self, Binding, Choice, Command, Draft, Env, Panel, Value},
    widgets::{
        self, Align, ButtonState, Painter, CLIENT_INSET_X, CLIENT_TOP, FONT_TALL, MENU_ROW,
        SCROLLBAR_WIDTH, SLIDER_HIT_TALL, SLIDER_TRACK_Y,
    },
};
use crate::canvas::{Rect, Texture2D};
use crate::config::Config;
use crate::console::{Input, Key};
use glam::{vec2, Vec2};
use std::sync::Arc;

/// Options dialog frame size (retail default `OptionsDialog` size).
pub const DIALOG_SIZE: Vec2 = Vec2::new(512., 406.);
/// `PropertyDialog` reserves this much below the sheet for the OK/Cancel/Apply row.
const SHEET_INSET_BOTTOM: f32 = 32.;
const CLIENT_INSET_BOTTOM: f32 = 8.;
/// `PropertySheet` tab strip height and the text inset on each side of a tab label.
const TAB_HEIGHT: f32 = 28.;
const TAB_TEXT_INSET: f32 = 12.;
const BUTTON_SIZE: Vec2 = Vec2::new(72., 24.);
/// Fallback sub-dialog sizes when the layout carries no frame record.
const GAMMA_SIZE: Vec2 = Vec2::new(290., 396.);
const CREDITS_SIZE: Vec2 = Vec2::new(500., 210.);
/// Key list header and row heights.
const KEY_HEADER: f32 = 20.;
/// Rows scrolled per wheel step in lists.
const WHEEL_ROWS: usize = 3;

/// Property sheet tabs, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Gameplay,
    Keyboard,
    Mouse,
    /// Empty placeholder page.
    Gamepad,
    Audio,
    Video,
}

impl Tab {
    pub const ALL: [Self; 6] = [
        Self::Gameplay,
        Self::Keyboard,
        Self::Mouse,
        Self::Gamepad,
        Self::Audio,
        Self::Video,
    ];

    pub fn token(self) -> &'static str {
        match self {
            Self::Gameplay => "#GameUI_Gameplay",
            Self::Keyboard => "#GameUI_Keyboard",
            Self::Mouse => "#GameUI_Mouse",
            Self::Gamepad => "#GameUI_Gamepad",
            Self::Audio => "#GameUI_Audio",
            Self::Video => "#GameUI_Video",
        }
    }

    pub fn panel(self) -> Option<Panel> {
        match self {
            Self::Gameplay => Some(Panel::Gameplay),
            Self::Keyboard => Some(Panel::Keyboard),
            Self::Mouse => Some(Panel::Mouse),
            Self::Gamepad => None,
            Self::Audio => Some(Panel::Audio),
            Self::Video => Some(Panel::Video),
        }
    }
}

/// What the owner of the dialog has to do after an input frame.
#[derive(Clone, Debug, PartialEq)]
pub enum OptionsOutcome {
    None,
    /// Apply and persist this configuration; the dialog stays open.
    Apply(Config),
    /// Close the dialog, applying and persisting the configuration if one is given.
    Close(Option<Config>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sub {
    /// Brightness dialog; `saved` is restored on Cancel.
    Gamma {
        saved: f32,
    },
    AudioCredits,
    VideoCredits,
}

impl Sub {
    fn is_credits(self) -> bool {
        matches!(self, Self::AudioCredits | Self::VideoCredits)
    }

    fn panel(self) -> Panel {
        match self {
            Self::Gamma { .. } => Panel::Gamma,
            Self::AudioCredits => Panel::AudioCredits,
            Self::VideoCredits => Panel::VideoCredits,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Gamma { .. } => "#GameUI_AdjustGamma_Title",
            Self::AudioCredits | Self::VideoCredits => "#GameUI_ThirdPartyTechCredits",
        }
    }

    fn fallback_size(self) -> Vec2 {
        match self {
            Self::Gamma { .. } => GAMMA_SIZE,
            Self::AudioCredits | Self::VideoCredits => CREDITS_SIZE,
        }
    }
}

/// Something under the pointer that reacts to clicks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Control(usize),
    Tab(usize),
    Ok,
    Cancel,
    Apply,
}

#[derive(Clone, Debug, PartialEq)]
struct OpenCombo {
    choice: Choice,
    combo: Rect,
    list: Rect,
    items: Vec<String>,
    first: usize,
    highlight: Option<usize>,
}

impl OpenCombo {
    fn rows(&self) -> usize {
        ((self.list.h / MENU_ROW).floor() as usize).max(1)
    }

    fn reveal(&mut self, index: usize) {
        let rows = self.rows();
        if index < self.first {
            self.first = index;
        } else if index >= self.first + rows {
            self.first = index + 1 - rows;
        }
    }

    fn scroll(&mut self, steps: i32) {
        let max = self.items.len().saturating_sub(self.rows()) as i32;
        self.first = (self.first as i32 - steps).clamp(0, max) as usize;
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Editing {
    value: Value,
    text: String,
    rect: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct KeyList {
    /// Index into [`GameUiResources::key_rows`] of the selected action row.
    selected: Option<usize>,
    first: usize,
    /// Row waiting for a new key.
    capture: Option<usize>,
}

/// A laid-out control of the current page or sub-dialog, in layer coordinates.
struct Widget<'a> {
    control: &'a Control,
    rect: Rect,
    binding: Option<Binding>,
    enabled: bool,
}

impl Widget<'_> {
    fn interactive(&self) -> bool {
        self.binding.is_some()
            || matches!(
                self.control.kind,
                ControlKind::Button | ControlKind::UrlButton
            )
    }

    /// The area that reacts to the pointer: sliders only along their track band.
    fn hit_rect(&self) -> Rect {
        match self.control.kind {
            ControlKind::Slider => Rect::new(
                self.rect.x,
                self.rect.y + SLIDER_TRACK_Y - SLIDER_HIT_TALL * 0.5,
                self.rect.w,
                SLIDER_HIT_TALL,
            ),
            _ => self.rect,
        }
    }
}

pub struct OptionsDialog {
    res: Arc<GameUiResources>,
    modes: DisplayModes,
    tab: Tab,
    applied: Config,
    draft: Draft,
    armed: Option<Target>,
    pressed: Option<Target>,
    drag: Option<(Value, Rect)>,
    combo: Option<OpenCombo>,
    editing: Option<Editing>,
    keys: KeyList,
    confirm: Option<MessageBox>,
    sub: Option<Sub>,
    gamma_texture: Option<Texture2D>,
    bink_texture: Option<Texture2D>,
    log: Vec<String>,
}

impl Default for OptionsDialog {
    fn default() -> Self {
        let config = Config::default();
        Self {
            res: Arc::default(),
            modes: DisplayModes::default(),
            tab: Tab::Gameplay,
            draft: Draft::new(config.clone()),
            applied: config,
            armed: None,
            pressed: None,
            drag: None,
            combo: None,
            editing: None,
            keys: KeyList::default(),
            confirm: None,
            sub: None,
            gamma_texture: None,
            bink_texture: None,
            log: Vec::new(),
        }
    }
}

fn texture(image: &Option<Image>) -> Option<Texture2D> {
    image
        .as_ref()
        .filter(|i| i.rgba.len() == usize::from(i.width) * usize::from(i.height) * 4)
        .map(|i| Texture2D::from_rgba8(i.width, i.height, &i.rgba))
}

fn centered(canvas: Vec2, size: Vec2, scale: f32) -> Vec2 {
    ((canvas - size * scale) * 0.5).round()
}

/// The page area below the tabs, in dialog coordinates.
pub fn page_rect() -> Rect {
    let top = CLIENT_TOP + TAB_HEIGHT;
    Rect::new(
        CLIENT_INSET_X,
        top,
        DIALOG_SIZE.x - 2. * CLIENT_INSET_X,
        DIALOG_SIZE.y - top - CLIENT_INSET_BOTTOM - SHEET_INSET_BOTTOM,
    )
}

/// OK, Cancel and Apply, right-aligned 80 apart on the bottom row (SDK `PropertyDialog`).
pub fn button_rects() -> [Rect; 3] {
    let y = DIALOG_SIZE.y - CLIENT_INSET_BOTTOM - 28.;
    let apply = DIALOG_SIZE.x - CLIENT_INSET_X - 80.;
    let rect = |x: f32| Rect::new(x, y, BUTTON_SIZE.x, BUTTON_SIZE.y);
    [rect(apply - 160.), rect(apply - 80.), rect(apply)]
}

/// Frame size of a sub-dialog: its layout's frame record, else the fallback.
fn frame_size(res: &GameUiResources, sub: Sub) -> Vec2 {
    res.layout(sub.panel().layout())
        .and_then(|layout| {
            layout.controls.iter().find(|c| {
                matches!(c.kind, ControlKind::Other(_)) && c.wide >= 200. && c.tall >= 100.
            })
        })
        .map_or(sub.fallback_size(), |c| vec2(c.wide, c.tall))
}

fn layout_widgets<'a>(
    res: &'a GameUiResources,
    panel: Panel,
    area: Rect,
    draft: &Draft,
    applied: &Config,
) -> Vec<Widget<'a>> {
    let Some(layout) = res.layout(panel.layout()) else {
        return Vec::new();
    };
    layout
        .controls
        .iter()
        .filter(|c| c.visible || settings::force_visible(panel, &c.name))
        .filter(|c| !matches!(c.kind, ControlKind::Other(_)))
        .map(|control| {
            let (x, y, w, h) = control.rect((area.w, area.h));
            let binding = settings::binding(panel, &control.name);
            Widget {
                control,
                rect: Rect::new(area.x + x, area.y + y, w, h),
                binding,
                enabled: settings::enabled(binding, control.enabled, draft, applied),
            }
        })
        .collect()
}

/// Visible key-list rows below the header.
fn key_rows_visible(list: Rect) -> usize {
    (((list.h - KEY_HEADER) / MENU_ROW).floor() as usize).max(1)
}

fn key_list_scrollable(list: Rect, total: usize) -> bool {
    total > key_rows_visible(list)
}

impl OptionsDialog {
    /// Installs the GameUI resources; textures are created here, never per frame.
    pub fn set_resources(&mut self, res: Arc<GameUiResources>) {
        self.gamma_texture = texture(&res.gamma_image);
        self.bink_texture = texture(&res.bink_image);
        self.res = res;
        self.keys = KeyList::default();
    }

    /// Resolutions offered on the Video page (from the host's monitor query).
    pub fn set_display_modes(&mut self, modes: DisplayModes) {
        self.modes = modes;
    }

    /// Opens the dialog over `config`, keeping the last selected tab.
    pub fn open(&mut self, config: Config) {
        self.draft = Draft::new(config.clone());
        self.applied = config;
        self.armed = None;
        self.pressed = None;
        self.drag = None;
        self.combo = None;
        self.editing = None;
        self.keys.capture = None;
        self.confirm = None;
        self.sub = None;
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// The configuration being edited.
    pub fn draft(&self) -> &Config {
        &self.draft.config
    }

    /// Whether Apply would change anything.
    pub fn changed(&self) -> bool {
        self.draft.config != self.applied
    }

    /// Messages for the console (placeholder activations).
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// Closes the innermost layer. Returns `false` when the dialog itself closes (Cancel).
    pub fn escape(&mut self) -> bool {
        self.drag = None;
        if self.keys.capture.take().is_some()
            || self.combo.take().is_some()
            || self.editing.take().is_some()
            || self.confirm.take().is_some()
        {
            return true;
        }
        if let Some(sub) = self.sub.take() {
            if let Sub::Gamma { saved } = sub {
                self.draft.config.video.gamma = saved;
            }
            return true;
        }
        false
    }

    fn tab_label(&self, tab: Tab) -> String {
        self.res.localization.text(tab.token())
    }

    /// Tab rectangles as drawn: the active tab is taller and touches the page.
    fn tab_rects(&self, measure: &dyn Fn(&str) -> f32) -> Vec<Rect> {
        let mut x = CLIENT_INSET_X;
        Tab::ALL
            .iter()
            .map(|tab| {
                let w = (measure(&self.tab_label(*tab)) + 2. * TAB_TEXT_INSET).ceil();
                let rect = if *tab == self.tab {
                    Rect::new(x, CLIENT_TOP + 2., w, TAB_HEIGHT - 1.)
                } else {
                    Rect::new(x, CLIENT_TOP + 4., w, TAB_HEIGHT - 3.)
                };
                x += w + 1.;
                rect
            })
            .collect()
    }

    fn placeholder(&mut self, label: &str) {
        let label = label.trim().trim_end_matches("...").trim_end_matches('…');
        self.log
            .push(format!("{label}: not implemented in HL2-RS yet."));
    }

    fn select_tab(&mut self, tab: Tab) {
        self.commit_entry();
        self.combo = None;
        if self.tab != tab {
            self.tab = tab;
            if tab == Tab::Gamepad {
                let label = self.tab_label(tab);
                self.placeholder(&format!("{label} options"));
            }
        }
    }

    fn commit_entry(&mut self) {
        if let Some(editing) = self.editing.take() {
            if let Some(value) = widgets::parse_value(&editing.text) {
                editing.value.set(&mut self.draft.config, value);
            }
        }
    }

    /// One input frame. `measure` supplies the canvas size and text metrics.
    pub fn input(&mut self, input: &Input, measure: &Painter) -> OptionsOutcome {
        let canvas = vec2(measure.canvas.width(), measure.canvas.height());
        let scale = Painter::scale_for(canvas.y);
        let local = |origin: Vec2| input.pointer.map(|p| (p - origin) / scale);
        if let Some(mut confirm) = self.confirm.take() {
            let layout = confirm.layout(|t| measure.text_width(t, false));
            let pointer = local(centered(canvas, layout.size, scale));
            match confirm.input(&layout, pointer, input.click, input.pressed(Key::Enter)) {
                Some(true) => self.use_default_keys(),
                Some(false) => {}
                None => self.confirm = Some(confirm),
            }
            return OptionsOutcome::None;
        }
        if let Some(sub) = self.sub {
            let size = frame_size(&self.res, sub);
            let pointer = local(centered(canvas, size, scale));
            self.sub_input(sub, size, input, pointer);
            return OptionsOutcome::None;
        }
        let pointer = local(centered(canvas, DIALOG_SIZE, scale));
        if let Some(row) = self.keys.capture {
            if let Some(key) = input.binding_keys.iter().find(|k| *k != "Escape") {
                if let Some(KeyRow::Action { command, .. }) = self.res.key_rows.get(row) {
                    keys::assign(&mut self.draft.config.keys, command, key);
                }
                self.keys.capture = None;
            }
            return OptionsOutcome::None;
        }
        if let Some(combo) = self.combo.take() {
            self.combo_input(combo, input, pointer);
            return OptionsOutcome::None;
        }
        self.page_input(input, pointer, measure)
    }

    fn combo_input(&mut self, mut combo: OpenCombo, input: &Input, pointer: Option<Vec2>) {
        let count = combo.items.len();
        if input.wheel != 0. {
            combo.scroll(input.wheel.round() as i32);
        }
        if count > 0 && (input.pressed(Key::Up) || input.pressed(Key::Down)) {
            let step: i32 = if input.pressed(Key::Up) { -1 } else { 1 };
            let current = combo.highlight.map_or(-1, |i| i as i32);
            let next = (current + step).clamp(0, count as i32 - 1) as usize;
            combo.highlight = Some(next);
            combo.reveal(next);
        }
        let on_scroll_bar =
            |p: Vec2| count > combo.rows() && p.x >= combo.list.x + combo.list.w - SCROLLBAR_WIDTH;
        let hit = pointer
            .filter(|p| !on_scroll_bar(*p))
            .and_then(|p| widgets::menu_list_hit(combo.list, combo.first, count, p));
        if hit.is_some() {
            combo.highlight = hit;
        }
        let pick = if input.click {
            match (hit, pointer) {
                (Some(index), _) => Some(Some(index)),
                (None, Some(p)) if combo.list.contains(p) => None,
                _ => Some(None),
            }
        } else if input.pressed(Key::Enter) {
            Some(combo.highlight)
        } else {
            None
        };
        match pick {
            Some(Some(index)) => {
                let env = Env {
                    localization: &self.res.localization,
                    languages: &self.res.spoken_languages,
                    modes: &self.modes,
                };
                combo.choice.select(&env, &mut self.draft, index);
            }
            Some(None) => {}
            None => self.combo = Some(combo),
        }
    }

    /// Slider drags, text editing and hover for a layer. Returns the clicked target.
    fn track(
        &mut self,
        widgets: &[Widget],
        extra: &[(Target, Rect)],
        pointer: Option<Vec2>,
        input: &Input,
    ) -> Option<Target> {
        if let Some((value, rect)) = self.drag {
            if input.held {
                if let Some(p) = pointer {
                    let fraction = widgets::slider_fraction_at(rect, p.x);
                    value.set(&mut self.draft.config, value.range().value(fraction));
                }
                return None;
            }
            self.drag = None;
        }
        if let Some(editing) = self.editing.as_mut() {
            for c in input.text.chars() {
                if (c.is_ascii_digit() || c == '.' || c == '-') && editing.text.len() < 12 {
                    editing.text.push(c);
                }
            }
            if input.pressed(Key::Backspace) {
                editing.text.pop();
            }
            if input.pressed(Key::Enter) {
                self.commit_entry();
            }
        }
        let hit = pointer.and_then(|p| {
            extra
                .iter()
                .find(|(_, rect)| rect.contains(p))
                .map(|(target, _)| *target)
                .or_else(|| {
                    widgets
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, w)| w.interactive() && w.hit_rect().contains(p))
                        .map(|(i, _)| Target::Control(i))
                })
        });
        self.armed = hit;
        self.pressed = hit.filter(|_| input.held || input.click);
        if !input.click {
            return None;
        }
        let keep_editing = matches!(
            (&self.editing, pointer),
            (Some(editing), Some(p)) if editing.rect.contains(p)
        );
        if !keep_editing {
            self.commit_entry();
        }
        hit
    }

    fn page_input(
        &mut self,
        input: &Input,
        pointer: Option<Vec2>,
        measure: &Painter,
    ) -> OptionsOutcome {
        let res = self.res.clone();
        let page = page_rect();
        let list = self.tab.panel().map_or_else(Vec::new, |panel| {
            layout_widgets(&res, panel, page, &self.draft, &self.applied)
        });
        let tabs: Vec<(Target, Rect)> = self
            .tab_rects(&|t| measure.text_width(t, false))
            .into_iter()
            .enumerate()
            .map(|(i, r)| (Target::Tab(i), Rect::new(r.x, CLIENT_TOP, r.w, TAB_HEIGHT)))
            .collect();
        let [ok, cancel, apply] = button_rects();
        let mut extra = tabs;
        extra.extend([
            (Target::Ok, ok),
            (Target::Cancel, cancel),
            (Target::Apply, apply),
        ]);
        let was_editing = self.editing.is_some();
        if self.tab == Tab::Keyboard {
            self.key_list_keys(input, &list);
        }
        let clicked = self.track(&list, &extra, pointer, input);
        match clicked {
            Some(Target::Tab(i)) => self.select_tab(Tab::ALL[i]),
            Some(Target::Ok) => return self.ok(),
            Some(Target::Cancel) => return OptionsOutcome::Close(None),
            Some(Target::Apply) => return self.apply(),
            Some(Target::Control(i)) => {
                if let (Some(widget), Some(p)) = (list.get(i), pointer) {
                    return self.activate(widget, p);
                }
            }
            None => {}
        }
        let keyboard_selection = self.tab == Tab::Keyboard && self.keys.selected.is_some();
        if input.pressed(Key::Enter) && !was_editing && !keyboard_selection && clicked.is_none() {
            return self.ok();
        }
        OptionsOutcome::None
    }

    fn ok(&mut self) -> OptionsOutcome {
        self.commit_entry();
        OptionsOutcome::Close(self.changed().then(|| self.draft.config.clone()))
    }

    fn apply(&mut self) -> OptionsOutcome {
        self.commit_entry();
        if !self.changed() {
            return OptionsOutcome::None;
        }
        self.applied = self.draft.config.clone();
        OptionsOutcome::Apply(self.applied.clone())
    }

    fn activate(&mut self, widget: &Widget, pointer: Vec2) -> OptionsOutcome {
        if !widget.enabled {
            if let Some(Binding::Command(command)) = widget.binding {
                if command.placeholder().is_some() {
                    let label = self.res.localization.localize(&widget.control.label);
                    self.placeholder(&label);
                }
            }
            return OptionsOutcome::None;
        }
        match widget.binding {
            Some(Binding::Toggle(toggle)) => {
                let value = toggle.get(&self.draft.config);
                toggle.set(&mut self.draft.config, !value);
            }
            Some(Binding::Skill(skill)) => self.draft.config.gameplay.skill = skill,
            Some(Binding::Slider(value)) => {
                let fraction = widgets::slider_fraction_at(widget.rect, pointer.x);
                value.set(&mut self.draft.config, value.range().value(fraction));
                self.drag = Some((value, widget.rect));
            }
            Some(Binding::Entry(value)) => {
                if self.editing.as_ref().is_none_or(|e| e.value != value) {
                    self.editing = Some(Editing {
                        value,
                        text: widgets::format_value(value.get(&self.draft.config)),
                        rect: widget.rect,
                    });
                }
            }
            Some(Binding::Choice(choice)) => {
                let env = Env {
                    localization: &self.res.localization,
                    languages: &self.res.spoken_languages,
                    modes: &self.modes,
                };
                let items = choice.items(&env, &self.draft);
                let highlight = choice.selected(&env, &self.draft);
                let mut combo = OpenCombo {
                    choice,
                    combo: widget.rect,
                    list: widgets::menu_list_rect(widget.rect, items.len()),
                    items,
                    first: 0,
                    highlight,
                };
                if let Some(index) = highlight {
                    combo.reveal(index);
                }
                self.combo = Some(combo);
            }
            Some(Binding::Command(command)) => self.command(command),
            Some(Binding::KeyList) => self.key_list_click(widget.rect, pointer),
            None => {}
        }
        OptionsOutcome::None
    }

    fn command(&mut self, command: Command) {
        let loc = &self.res.localization;
        match command {
            Command::GammaDialog => {
                self.sub = Some(Sub::Gamma {
                    saved: self.draft.config.video.gamma,
                })
            }
            Command::AudioCredits => self.sub = Some(Sub::AudioCredits),
            Command::VideoCredits => self.sub = Some(Sub::VideoCredits),
            Command::KeyDefaults => {
                self.confirm = Some(MessageBox::new(
                    loc.text("#GameUI_KeyboardSettings"),
                    loc.text("#GameUI_KeyboardSettingsText"),
                    loc.text("#GameUI_OK"),
                    Some(loc.text("#GameUI_Cancel")),
                ))
            }
            Command::KeyEdit => self.keys.capture = self.keys.selected,
            Command::KeyClear => {
                if let Some(KeyRow::Action { command, .. }) =
                    self.keys.selected.and_then(|i| self.res.key_rows.get(i))
                {
                    self.draft
                        .config
                        .keys
                        .insert(command.clone(), keys::UNBOUND.into());
                }
            }
            Command::KeyAdvanced | Command::VideoAdvanced => {
                let label = command.placeholder().unwrap_or_default();
                self.placeholder(label);
            }
            Command::SubOk => self.sub = None,
            Command::SubCancel => {
                if let Some(Sub::Gamma { saved }) = self.sub {
                    self.draft.config.video.gamma = saved;
                }
                self.sub = None;
            }
        }
    }

    /// Resets the `kb_act.lst` actions to the installed defaults (actions without one become
    /// unbound, as retail `unbindall` + default config does); other bindings are kept unless a
    /// default takes their key.
    fn use_default_keys(&mut self) {
        let res = self.res.clone();
        let keys = &mut self.draft.config.keys;
        for row in &res.key_rows {
            if let KeyRow::Action { command, .. } = row {
                keys.insert(command.clone(), keys::UNBOUND.into());
            }
        }
        for row in &res.key_rows {
            if let KeyRow::Action { command, .. } = row {
                if let Some(key) = res.default_bindings.get(command) {
                    keys::assign(keys, command, key);
                }
            }
        }
    }

    fn key_list_rect(list: &[Widget]) -> Option<Rect> {
        list.iter()
            .find(|w| w.binding == Some(Binding::KeyList))
            .map(|w| w.rect)
    }

    /// Keyboard navigation, wheel scrolling and Enter-to-edit on the Keyboard page.
    fn key_list_keys(&mut self, input: &Input, list: &[Widget]) {
        let Some(rect) = Self::key_list_rect(list) else {
            return;
        };
        let rows = &self.res.key_rows;
        let visible = key_rows_visible(rect);
        let max_first = rows.len().saturating_sub(visible);
        if input.wheel != 0. && self.combo.is_none() {
            let steps = input.wheel.round() as i32 * WHEEL_ROWS as i32;
            self.keys.first = (self.keys.first as i32 - steps).clamp(0, max_first as i32) as usize;
        }
        let step = if input.pressed(Key::Up) {
            -1
        } else if input.pressed(Key::Down) {
            1
        } else {
            0
        };
        if step != 0 {
            let actions: Vec<usize> = rows
                .iter()
                .enumerate()
                .filter(|(_, r)| matches!(r, KeyRow::Action { .. }))
                .map(|(i, _)| i)
                .collect();
            let position = self
                .keys
                .selected
                .and_then(|s| actions.iter().position(|a| *a == s));
            let next = match position {
                None if step > 0 => actions.first(),
                None => actions.last(),
                Some(p) => {
                    actions.get((p as i32 + step).clamp(0, actions.len() as i32 - 1) as usize)
                }
            };
            if let Some(&next) = next {
                self.keys.selected = Some(next);
                if next < self.keys.first {
                    self.keys.first = next;
                } else if next >= self.keys.first + visible {
                    self.keys.first = (next + 1 - visible).min(max_first);
                }
            }
        }
        if input.pressed(Key::Enter) && self.editing.is_none() && self.keys.selected.is_some() {
            self.keys.capture = self.keys.selected;
        }
    }

    fn key_list_click(&mut self, rect: Rect, p: Vec2) {
        let total = self.res.key_rows.len();
        let visible = key_rows_visible(rect);
        let body = Rect::new(rect.x, rect.y + KEY_HEADER, rect.w, rect.h - KEY_HEADER);
        if !body.contains(p) {
            return;
        }
        if key_list_scrollable(rect, total) && p.x >= rect.x + rect.w - SCROLLBAR_WIDTH {
            let fraction = ((p.y - body.y) / body.h).clamp(0., 1.);
            self.keys.first = ((total - visible) as f32 * fraction).round() as usize;
            return;
        }
        let index = self.keys.first + ((p.y - body.y) / MENU_ROW).floor() as usize;
        if matches!(self.res.key_rows.get(index), Some(KeyRow::Action { .. })) {
            self.keys.selected = Some(index);
        }
    }

    fn sub_input(&mut self, sub: Sub, size: Vec2, input: &Input, pointer: Option<Vec2>) {
        let res = self.res.clone();
        let area = Rect::new(0., 0., size.x, size.y);
        let list = layout_widgets(&res, sub.panel(), area, &self.draft, &self.applied);
        let was_editing = self.editing.is_some();
        match self.track(&list, &[], pointer, input) {
            Some(Target::Control(i)) => {
                if let (Some(widget), Some(p)) = (list.get(i), pointer) {
                    self.activate(widget, p);
                }
            }
            _ if input.pressed(Key::Enter) && !was_editing => self.command(Command::SubOk),
            _ => {}
        }
    }

    // ---- drawing -------------------------------------------------------------------------

    pub fn draw(&self, base: &Painter) {
        let canvas = vec2(base.canvas.width(), base.canvas.height());
        let scale = Painter::scale_for(canvas.y);
        let painter_at = |origin: Vec2| Painter {
            canvas: base.canvas,
            fonts: base.fonts,
            theme: base.theme,
            origin,
            scale,
        };
        let p = painter_at(centered(canvas, DIALOG_SIZE, scale));
        let loc = &self.res.localization;
        let env = Env {
            localization: loc,
            languages: &self.res.spoken_languages,
            modes: &self.modes,
        };
        let modal = self.sub.is_some() || self.confirm.is_some();
        p.frame(DIALOG_SIZE, &loc.text("#GameUI_Options"));
        let tabs = self.tab_rects(&|t| p.text_width(t, false));
        let page = page_rect();
        p.raised(page);
        let active = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        for (i, rect) in tabs.iter().enumerate() {
            if i != active {
                p.tab(*rect, &self.tab_label(Tab::ALL[i]), false);
            }
        }
        p.tab(tabs[active], &self.tab_label(self.tab), true);
        let state = |target: Target| {
            (
                !modal && self.armed == Some(target),
                !modal && self.pressed == Some(target),
            )
        };
        if let Some(panel) = self.tab.panel() {
            let list = layout_widgets(&self.res, panel, page, &self.draft, &self.applied);
            for (i, widget) in list.iter().enumerate() {
                let (armed, pressed) = state(Target::Control(i));
                self.draw_widget(&p, widget, &env, armed, pressed);
            }
        }
        let labels = ["#GameUI_OK", "#GameUI_Cancel", "#GameUI_Apply"];
        let targets = [Target::Ok, Target::Cancel, Target::Apply];
        for ((rect, label), target) in button_rects().into_iter().zip(labels).zip(targets) {
            let (armed, pressed) = state(target);
            p.button(
                rect,
                &loc.text(label),
                ButtonState {
                    armed,
                    pressed,
                    enabled: target != Target::Apply || self.changed(),
                    default: target == Target::Ok,
                },
            );
        }
        if let Some(combo) = &self.combo {
            p.menu_list(combo.list, &combo.items, combo.first, combo.highlight);
        }
        if let Some(sub) = self.sub {
            let size = frame_size(&self.res, sub);
            let mut theme = base.theme.clone();
            if sub.is_credits() {
                theme.frame_bg.a = 1.0;
            }
            let sp = Painter {
                canvas: base.canvas,
                fonts: base.fonts,
                theme: &theme,
                origin: centered(canvas, size, scale),
                scale,
            };
            sp.frame(size, &loc.text(sub.title()));
            let area = Rect::new(0., 0., size.x, size.y);
            let list = layout_widgets(&self.res, sub.panel(), area, &self.draft, &self.applied);
            let sub_modal = self.confirm.is_some();
            for (i, widget) in list.iter().enumerate() {
                let target = Some(Target::Control(i));
                let armed = !sub_modal && self.armed == target;
                let pressed = !sub_modal && self.pressed == target;
                self.draw_widget(&sp, widget, &env, armed, pressed);
            }
        }
        if let Some(confirm) = &self.confirm {
            let layout = confirm.layout(|t| p.text_width(t, false));
            confirm.draw(&painter_at(centered(canvas, layout.size, scale)), &layout);
        }
    }

    fn draw_widget(&self, p: &Painter, w: &Widget, env: &Env, armed: bool, pressed: bool) {
        let loc = &self.res.localization;
        let c = w.control;
        let label = loc.localize(&c.label);
        let config = &self.draft.config;
        match (&c.kind, w.binding) {
            (ControlKind::Label, _) => {
                let multiline = c.align == LayoutAlign::NorthWest || c.tall >= FONT_TALL * 2.;
                if multiline {
                    let color = if w.enabled {
                        p.theme.text
                    } else {
                        p.theme.text_disabled
                    };
                    p.wrapped(&label, w.rect, color);
                } else {
                    let align = match c.align {
                        LayoutAlign::Center => Align::Center,
                        LayoutAlign::East => Align::Right,
                        _ => Align::Left,
                    };
                    p.label(&label, w.rect, align, w.enabled);
                }
            }
            (ControlKind::CheckButton, Some(Binding::Toggle(toggle))) => {
                p.check_box(w.rect, &label, toggle.get(config), w.enabled)
            }
            (ControlKind::CheckButton, _) => p.check_box(w.rect, &label, false, false),
            (ControlKind::RadioButton, binding) => {
                let selected = binding == Some(Binding::Skill(config.gameplay.skill));
                p.radio(w.rect, &label, selected, w.enabled)
            }
            (ControlKind::ComboBox, Some(Binding::Choice(choice))) => {
                let open = self.combo.as_ref().is_some_and(|o| o.combo == w.rect);
                p.combo(
                    w.rect,
                    &choice.text(env, &self.draft),
                    w.enabled,
                    armed || open,
                )
            }
            (ControlKind::ComboBox, _) => p.combo(w.rect, "", false, false),
            (ControlKind::Slider, binding) => {
                let fraction = match binding {
                    Some(Binding::Slider(value)) => value.range().fraction(value.get(config)),
                    _ => 0.,
                };
                p.slider(
                    w.rect,
                    fraction,
                    &loc.localize(&c.left_text),
                    &loc.localize(&c.right_text),
                    w.enabled,
                )
            }
            (ControlKind::TextEntry, binding) => {
                let (text, caret) = match (&self.editing, binding) {
                    (Some(editing), Some(Binding::Entry(value))) if editing.value == value => {
                        (editing.text.clone(), true)
                    }
                    (_, Some(Binding::Entry(value))) => {
                        (widgets::format_value(value.get(config)), false)
                    }
                    _ => (label.clone(), false),
                };
                p.text_entry(w.rect, &text, w.enabled, caret)
            }
            (ControlKind::Button, binding) => p.button(
                w.rect,
                &label,
                ButtonState {
                    armed,
                    pressed,
                    enabled: w.enabled,
                    default: binding == Some(Binding::Command(Command::SubOk)),
                },
            ),
            (ControlKind::UrlButton, _) => p.url(w.rect, &label, w.enabled),
            (ControlKind::Divider, _) => p.divider(w.rect),
            (ControlKind::ImagePanel, _) => {
                let image = c.image.to_ascii_lowercase();
                let texture = if image.contains("bink") {
                    self.bink_texture.as_ref()
                } else if image.contains("gamma") {
                    self.gamma_texture.as_ref()
                } else {
                    None
                };
                if let Some(texture) = texture {
                    p.image(w.rect, texture);
                }
            }
            (ControlKind::ListPanel, Some(Binding::KeyList)) => self.draw_key_list(p, w.rect),
            _ => {}
        }
    }

    fn draw_key_list(&self, p: &Painter, rect: Rect) {
        let loc = &self.res.localization;
        let theme = p.theme;
        p.fill(rect, theme.list_bg);
        p.sunken(rect);
        let rows = &self.res.key_rows;
        let scroll = key_list_scrollable(rect, rows.len());
        let inner_w = rect.w - 2. - if scroll { SCROLLBAR_WIDTH } else { 0. };
        let action_w = (inner_w * 0.6).round();
        let header = Rect::new(rect.x + 1., rect.y + 1., inner_w, KEY_HEADER - 1.);
        p.raised(Rect::new(header.x, header.y, action_w, header.h));
        p.raised(Rect::new(
            header.x + action_w,
            header.y,
            inner_w - action_w,
            header.h,
        ));
        let cell = |x: f32, y: f32, w: f32| Rect::new(x + 6., y, w - 12., MENU_ROW);
        p.text_in(
            &loc.text("#GameUI_Action"),
            cell(header.x, header.y, action_w),
            Align::Left,
            theme.text,
            false,
        );
        p.text_in(
            &loc.text("#GameUI_KeyButton"),
            cell(header.x + action_w, header.y, inner_w - action_w),
            Align::Left,
            theme.text,
            false,
        );
        let visible = key_rows_visible(rect);
        let top = rect.y + KEY_HEADER;
        for (line, (index, row)) in rows
            .iter()
            .enumerate()
            .skip(self.keys.first)
            .take(visible)
            .enumerate()
        {
            let y = top + line as f32 * MENU_ROW;
            match row {
                KeyRow::Section(title) => {
                    p.text_in(
                        &loc.localize(title),
                        cell(header.x, y, inner_w),
                        Align::Left,
                        theme.text,
                        true,
                    );
                }
                KeyRow::Action { command, label } => {
                    let selected = self.keys.selected == Some(index);
                    let color = if selected {
                        p.fill(
                            Rect::new(header.x, y, inner_w, MENU_ROW),
                            theme.selection_bg,
                        );
                        theme.selection_text
                    } else {
                        theme.text
                    };
                    p.text_in(
                        &loc.localize(label),
                        cell(header.x, y, action_w),
                        Align::Left,
                        color,
                        false,
                    );
                    let key = if self.keys.capture == Some(index) {
                        String::new()
                    } else {
                        self.draft
                            .config
                            .keys
                            .get(command)
                            .map(|k| keys::display_name(k))
                            .unwrap_or_default()
                    };
                    p.text_in(
                        &key,
                        cell(header.x + action_w, y, inner_w - action_w),
                        Align::Left,
                        color,
                        false,
                    );
                }
            }
        }
        if scroll {
            p.scroll_bar(
                Rect::new(
                    rect.x + rect.w - 1. - SCROLLBAR_WIDTH,
                    top,
                    SCROLLBAR_WIDTH,
                    rect.h - KEY_HEADER - 1.,
                ),
                self.keys.first,
                visible,
                rows.len(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;
    use crate::config::{AimAssist, HdrMode};
    use crate::gameui::{
        display::{AspectRatio, Resolution},
        layout::Layout,
        localize::Localization,
        widgets::{Fonts, Theme},
    };
    use std::collections::BTreeMap;

    const MOUSE: &str = r##""Resource/OptionsSubMouse.res" {
        "ReverseMouse" { "ControlName" "CCvarNegateCheckButton" "fieldName" "ReverseMouse"
            "xpos" "28" "ypos" "9" "wide" "150" "tall" "28" "labelText" "#GameUI_ReverseMouse" }
        "Slider" { "ControlName" "CCvarSlider" "fieldName" "Slider" "xpos" "200" "ypos" "40"
            "wide" "200" "tall" "40" "leftText" "#GameUI_Low" "rightText" "#GameUI_High" }
        "SensitivityLabel" { "ControlName" "TextEntry" "fieldName" "SensitivityLabel"
            "xpos" "400" "ypos" "40" "wide" "50" "tall" "24" }
        "Autoaim" { "ControlName" "ComboBox" "fieldName" "Autoaim" "xpos" "28" "ypos" "172"
            "wide" "175" "tall" "24" }
    }"##;
    const VIDEO: &str = r##""Resource/OptionsSubVideo.res" {
        "Resolution" { "ControlName" "ComboBox" "fieldName" "Resolution" "xpos" "36" "ypos" "48"
            "wide" "160" "tall" "24" }
        "AspectRatio" { "ControlName" "ComboBox" "fieldName" "AspectRatio" "xpos" "36"
            "ypos" "117" "wide" "160" "tall" "24" }
        "GammaButton" { "ControlName" "Button" "fieldName" "GammaButton" "xpos" "254"
            "ypos" "117" "wide" "214" "tall" "24" "enabled" "0" "labelText" "#GameUI_Brightness" }
        "AdvancedButton" { "ControlName" "Button" "fieldName" "AdvancedButton" "xpos" "254"
            "ypos" "190" "wide" "214" "tall" "24" "labelText" "#GameUI_AdvancedEllipsis" }
        "HDR" { "ControlName" "ComboBox" "fieldName" "HDR" "xpos" "36" "ypos" "264"
            "wide" "160" "tall" "24" "visible" "0" }
    }"##;
    const KEYBOARD: &str = r##""Resource/OptionsSubKeyboard.res" {
        "listpanel_keybindlist" { "ControlName" "SectionedListPanel"
            "fieldName" "listpanel_keybindlist" "xpos" "8" "ypos" "10" "wide" "480" "tall" "258" }
        "Defaults" { "ControlName" "Button" "fieldName" "Defaults" "xpos" "8" "ypos" "276"
            "wide" "134" "tall" "24" "labelText" "#GameUI_UseDefaults" }
        "ChangeKeyButton" { "ControlName" "Button" "fieldName" "ChangeKeyButton" "xpos" "272"
            "ypos" "276" "wide" "106" "tall" "24" "labelText" "#GameUI_SetNewKey" }
        "ClearKeyButton" { "ControlName" "Button" "fieldName" "ClearKeyButton" "xpos" "384"
            "ypos" "276" "wide" "105" "tall" "24" "labelText" "#GameUI_ClearKey" }
    }"##;
    const GAMMA: &str = r##""Resource/OptionsSubVideoGammaDlg.res" {
        "OptionsSubVideoGammaDlg" { "ControlName" "CGammaDialog" "fieldName" "OptionsSubVideoGammaDlg"
            "xpos" "0" "ypos" "0" "wide" "290" "tall" "396" }
        "Gamma" { "ControlName" "Slider" "fieldName" "Gamma" "xpos" "17" "ypos" "112"
            "wide" "200" "tall" "40" }
        "OKButton" { "ControlName" "Button" "fieldName" "OKButton" "xpos" "104" "ypos" "356"
            "wide" "72" "tall" "24" "labelText" "#vgui_ok" }
        "Button1" { "ControlName" "Button" "fieldName" "Button1" "xpos" "192" "ypos" "356"
            "wide" "72" "tall" "24" "labelText" "#vgui_Cancel" }
    }"##;

    /// Canvas 1280x720 => scale 1, dialog at (384, 157), page at (392, 215).
    const DIALOG: Vec2 = Vec2::new(384., 157.);

    fn page(x: f32, y: f32) -> Vec2 {
        DIALOG + vec2(page_rect().x + x, page_rect().y + y)
    }

    fn resources() -> Arc<GameUiResources> {
        let mut localization = Localization::default();
        localization.insert("GameUI_Gamepad", "Gamepad");
        localization.insert("GameUI_AdvancedEllipsis", "Advanced...");
        let mut layouts = BTreeMap::new();
        for (name, text) in [
            ("optionssubmouse", MOUSE),
            ("optionssubvideo", VIDEO),
            ("optionssubkeyboard", KEYBOARD),
            ("optionssubvideogammadlg", GAMMA),
        ] {
            layouts.insert(name.to_owned(), Layout::parse(text.as_bytes()).unwrap());
        }
        let key_rows = vec![
            KeyRow::Section("#Valve_Movement_Title".into()),
            KeyRow::Action {
                command: "+forward".into(),
                label: "#Valve_Move_Forward".into(),
            },
            KeyRow::Action {
                command: "+jump".into(),
                label: "#Valve_Jump".into(),
            },
        ];
        let default_bindings = BTreeMap::from([
            ("+forward".to_owned(), "KeyW".to_owned()),
            ("+jump".to_owned(), "Space".to_owned()),
        ]);
        Arc::new(GameUiResources {
            localization,
            layouts,
            key_rows,
            default_bindings,
            spoken_languages: vec!["english".into()],
            ..Default::default()
        })
    }

    struct Harness {
        dialog: OptionsDialog,
        canvas: Canvas,
        fonts: Fonts,
        theme: Theme,
    }

    impl Harness {
        fn new() -> Self {
            let mut dialog = OptionsDialog::default();
            dialog.set_resources(resources());
            dialog.set_display_modes(DisplayModes::new(vec![
                Resolution::new(1920, 1080),
                Resolution::new(1280, 720),
                Resolution::new(1024, 768),
            ]));
            dialog.open(Config::default());
            Self {
                dialog,
                canvas: Canvas::default(),
                fonts: Fonts::default(),
                theme: Theme::default(),
            }
        }

        fn frame(&mut self, input: Input) -> OptionsOutcome {
            let painter = Painter {
                canvas: &self.canvas,
                fonts: &self.fonts,
                theme: &self.theme,
                origin: Vec2::ZERO,
                scale: 1.,
            };
            let outcome = self.dialog.input(&input, &painter);
            self.dialog.draw(&painter);
            self.canvas.drain();
            outcome
        }

        fn click(&mut self, at: Vec2) -> OptionsOutcome {
            let outcome = self.frame(Input {
                pointer: Some(at),
                click: true,
                held: true,
                ..Default::default()
            });
            self.frame(Input {
                pointer: Some(at),
                ..Default::default()
            });
            outcome
        }

        fn key(&mut self, key: Key) -> OptionsOutcome {
            self.frame(Input {
                keys: [key].into(),
                ..Default::default()
            })
        }

        fn tab(&mut self, tab: Tab) {
            let index = Tab::ALL.iter().position(|t| *t == tab).unwrap();
            let rect = self.dialog.tab_rects(&|t| t.chars().count() as f32 * 7.)[index];
            self.click(DIALOG + vec2(rect.x + 4., rect.y + 4.));
            assert_eq!(self.dialog.tab(), tab);
        }

        fn button(&mut self, index: usize) -> OptionsOutcome {
            let rect = button_rects()[index];
            self.click(DIALOG + vec2(rect.x + 4., rect.y + 4.))
        }
    }

    #[test]
    fn geometry_follows_property_dialog_rules() {
        assert_eq!(page_rect(), Rect::new(8., 58., 496., 308.));
        let [ok, cancel, apply] = button_rects();
        assert_eq!((ok.x, cancel.x, apply.x, ok.y), (264., 344., 424., 370.));
    }

    #[test]
    fn tabs_switch_and_the_gamepad_placeholder_is_logged() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        h.tab(Tab::Gamepad);
        assert_eq!(
            h.dialog.take_log(),
            ["Gamepad options: not implemented in HL2-RS yet."]
        );
        h.tab(Tab::Video);
        assert!(h.dialog.take_log().is_empty());
    }

    #[test]
    fn ok_cancel_apply_follow_draft_semantics() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        assert_eq!(
            h.button(0),
            OptionsOutcome::Close(None),
            "OK without changes"
        );
        h.dialog.open(Config::default());
        h.click(page(30., 20.));
        assert!(h.dialog.draft().mouse.invert);
        assert!(h.dialog.changed());
        let OptionsOutcome::Apply(applied) = h.button(2) else {
            panic!("Apply publishes the draft");
        };
        assert!(applied.mouse.invert);
        assert!(!h.dialog.changed());
        assert_eq!(
            h.button(2),
            OptionsOutcome::None,
            "Apply disabled without changes"
        );
        h.click(page(30., 20.));
        assert_eq!(
            h.button(1),
            OptionsOutcome::Close(None),
            "Cancel drops the draft"
        );
        h.dialog.open(applied.clone());
        h.click(page(30., 20.));
        let OptionsOutcome::Close(Some(closed)) = h.button(0) else {
            panic!("OK returns the changed draft");
        };
        assert!(!closed.mouse.invert);
    }

    #[test]
    fn slider_drag_and_text_entry_edit_values() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        // Track spans x 204..396 of the page; the middle is half of 0.1..6.
        let start = page(300., 50.);
        h.frame(Input {
            pointer: Some(start),
            click: true,
            held: true,
            ..Default::default()
        });
        assert!((h.dialog.draft().mouse.sensitivity - 3.05).abs() < 0.01);
        h.frame(Input {
            pointer: Some(page(500., 90.)),
            held: true,
            ..Default::default()
        });
        assert_eq!(
            h.dialog.draft().mouse.sensitivity,
            6.,
            "drag continues off the track"
        );
        h.frame(Input::default());
        h.click(page(410., 50.));
        for _ in 0..4 {
            h.key(Key::Backspace);
        }
        h.frame(Input {
            text: "1x2.5".into(),
            ..Default::default()
        });
        h.key(Key::Enter);
        assert_eq!(
            h.dialog.draft().mouse.sensitivity,
            12.5,
            "typed values may exceed the slider"
        );
        h.click(page(410., 50.));
        h.frame(Input {
            text: "-".into(),
            keys: [Key::Backspace, Key::Backspace].into(),
            ..Default::default()
        });
        h.click(page(30., 20.));
        assert!(
            h.dialog.draft().mouse.sensitivity > 0.09,
            "invalid entries are clamped or kept"
        );
    }

    #[test]
    fn combos_select_items_and_aspect_resets_resolution() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        h.click(page(40., 180.));
        assert!(h.dialog.combo.is_some());
        // Rows below the combo (y 196): Off, Basic, Enhanced.
        h.click(page(40., 196. + MENU_ROW * 1.5));
        assert!(h.dialog.combo.is_none());
        assert_eq!(h.dialog.draft().mouse.aim_assist, AimAssist::Basic);
        h.tab(Tab::Video);
        h.click(page(40., 125.));
        h.click(page(40., 141. + MENU_ROW * 0.5));
        assert_eq!(h.dialog.draft.aspect, AspectRatio::Normal);
        assert_eq!(
            (h.dialog.draft().video.width, h.dialog.draft().video.height),
            (1024, 768)
        );
        h.click(page(40., 270.));
        h.key(Key::Down);
        h.key(Key::Down);
        h.key(Key::Enter);
        assert_eq!(
            h.dialog.draft().video.hdr,
            HdrMode::Full,
            "HDR combo is shown on every platform"
        );
        h.click(page(40., 125.));
        h.click(page(450., 300.));
        assert!(h.dialog.combo.is_none(), "clicking outside closes the list");
        assert_eq!(h.dialog.draft.aspect, AspectRatio::Normal);
    }

    #[test]
    fn key_capture_clear_and_defaults() {
        let mut h = Harness::new();
        h.tab(Tab::Keyboard);
        // Header 20, then the section row and +forward at row 1.
        h.click(page(30., 10. + KEY_HEADER + MENU_ROW * 1.5));
        assert_eq!(h.dialog.keys.selected, Some(1));
        h.click(page(280., 280.));
        assert_eq!(h.dialog.keys.capture, Some(1));
        h.frame(Input {
            binding_keys: vec!["MOUSE1".into()],
            ..Default::default()
        });
        assert_eq!(h.dialog.draft().keys.get("+forward").unwrap(), "MOUSE1");
        h.key(Key::Down);
        assert_eq!(h.dialog.keys.selected, Some(2));
        h.key(Key::Enter);
        assert!(h.dialog.escape(), "Escape cancels capture only");
        assert_eq!(h.dialog.draft().keys.get("+jump").unwrap(), "Space");
        h.key(Key::Enter);
        h.frame(Input {
            binding_keys: vec!["Escape".into(), "MOUSE1".into()],
            ..Default::default()
        });
        assert_eq!(h.dialog.draft().keys.get("+jump").unwrap(), "MOUSE1");
        assert_eq!(
            h.dialog.draft().keys.get("+forward").unwrap(),
            keys::UNBOUND,
            "a key runs one command"
        );
        h.click(page(390., 280.));
        assert_eq!(h.dialog.draft().keys.get("+jump").unwrap(), keys::UNBOUND);
        h.click(page(20., 280.));
        assert!(h.dialog.confirm.is_some());
        h.key(Key::Enter);
        assert_eq!(h.dialog.draft().keys.get("+forward").unwrap(), "KeyW");
        assert_eq!(h.dialog.draft().keys.get("+jump").unwrap(), "Space");
        assert_eq!(
            h.dialog.draft().keys.get("loadout").unwrap(),
            "F3",
            "other commands kept"
        );
    }

    #[test]
    fn placeholders_log_and_gamma_dialog_restores_on_cancel() {
        let mut h = Harness::new();
        h.tab(Tab::Video);
        h.click(page(300., 200.));
        assert_eq!(
            h.dialog.take_log(),
            ["Advanced: not implemented in HL2-RS yet."]
        );
        h.click(page(300., 125.));
        assert_eq!(h.dialog.sub, Some(Sub::Gamma { saved: 2.2 }));
        // Gamma frame 290x396 centered: origin (495, 162); slider track at y 122.
        let gamma = vec2(495., 162.);
        h.click(gamma + vec2(17. + 4., 122.));
        assert_eq!(h.dialog.draft().video.gamma, 1.6);
        h.click(gamma + vec2(200., 360.));
        assert!(h.dialog.sub.is_none());
        assert_eq!(
            h.dialog.draft().video.gamma,
            2.2,
            "Cancel restores the gamma"
        );
        h.click(page(300., 125.));
        h.click(gamma + vec2(17. + 4., 122.));
        h.click(gamma + vec2(110., 360.));
        assert_eq!(
            h.dialog.draft().video.gamma,
            1.6,
            "OK keeps it in the draft"
        );
    }

    #[test]
    #[ignore = "requires an owned Half-Life 2 installation"]
    fn owned_install_pages_bind_every_expected_control() {
        let root = source_assets::install::discover().expect("installed game");
        let vfs = source_assets::vpk::Vfs::mount(&root).unwrap();
        let res = Arc::new(GameUiResources::load(&vfs));
        let expected: &[(Panel, &[&str])] = &[
            (
                Panel::Gameplay,
                &["Skill1Radio", "Skill2Radio", "Skill3Radio"],
            ),
            (
                Panel::Mouse,
                &[
                    "ReverseMouse",
                    "MouseFilter",
                    "MouseRaw",
                    "MouseAccelerationCheckbox",
                    "Slider",
                    "SensitivityLabel",
                    "MouseAccelerationSlider",
                    "MouseAccelerationLabel",
                    "Autoaim",
                    "AutoaimScale",
                ],
            ),
            (
                Panel::Audio,
                &[
                    "SFXSlider",
                    "MusicSlider",
                    "SpeakerSetup",
                    "SoundQuality",
                    "AudioSpokenLanguage",
                    "CloseCaptionCheck",
                    "snd_mute_losefocus",
                    "ThirdPartySoundCredits",
                ],
            ),
            (
                Panel::Video,
                &[
                    "Resolution",
                    "AspectRatio",
                    "DisplayModeCombo",
                    "HudAspectRatio",
                    "HDR",
                    "GammaButton",
                    "AdvancedButton",
                    "ThirdPartyVideoCredits",
                ],
            ),
            (
                Panel::Keyboard,
                &[
                    "listpanel_keybindlist",
                    "Defaults",
                    "KeyAdvancedButton",
                    "ChangeKeyButton",
                    "ClearKeyButton",
                ],
            ),
            (
                Panel::Gamma,
                &["Gamma", "GammaEntry", "OKButton", "Button1"],
            ),
            (Panel::AudioCredits, &["Button2"]),
            (Panel::VideoCredits, &["Button2"]),
        ];
        let config = Config::default();
        let draft = Draft::new(config.clone());
        for (panel, names) in expected {
            let layout = res
                .layout(panel.layout())
                .unwrap_or_else(|| panic!("{panel:?}: {:?}", res.missing));
            for name in *names {
                assert!(layout.get(name).is_some(), "{panel:?} lacks {name}");
                assert!(
                    settings::binding(*panel, name).is_some(),
                    "{panel:?} {name}"
                );
            }
            for widget in layout_widgets(&res, *panel, page_rect(), &draft, &config) {
                for text in [&widget.control.label, &widget.control.left_text] {
                    if text.starts_with('#') {
                        assert!(
                            res.localization.lookup(text).is_some(),
                            "{panel:?} {} label {text}",
                            widget.control.name
                        );
                    }
                }
            }
        }
        let mut h = Harness::new();
        h.dialog.set_resources(res);
        h.dialog.open(config);
        for tab in Tab::ALL {
            h.dialog.select_tab(tab);
            h.frame(Input::default());
        }
        h.dialog.command(Command::GammaDialog);
        h.frame(Input::default());
        assert!(h.dialog.gamma_texture.is_some());
    }

    #[test]
    fn escape_closes_layers_innermost_first() {
        let mut h = Harness::new();
        h.tab(Tab::Video);
        h.click(page(300., 125.));
        h.click(page(40., 270.));
        assert!(h.dialog.sub.is_some());
        assert!(h.dialog.escape());
        assert!(h.dialog.sub.is_none());
        h.click(page(40., 270.));
        assert!(h.dialog.combo.is_some());
        assert!(h.dialog.escape());
        assert!(h.dialog.combo.is_none());
        assert!(!h.dialog.escape(), "Escape on the dialog closes it");
    }

    #[test]
    fn credits_dialogs_have_100_percent_opacity() {
        let mut h = Harness::new();
        let painter = Painter {
            canvas: &h.canvas,
            fonts: &h.fonts,
            theme: &h.theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        h.dialog.sub = Some(Sub::AudioCredits);
        h.dialog.draw(&painter);
        let quads = h.canvas.drain();
        let credits_frame = quads
            .iter()
            .find(|q| q.destination.w == CREDITS_SIZE.x && q.destination.h == CREDITS_SIZE.y)
            .expect("audio credits frame quad");
        assert_eq!(credits_frame.color.a, 1.0);

        h.dialog.sub = Some(Sub::VideoCredits);
        h.dialog.draw(&painter);
        let quads = h.canvas.drain();
        let video_credits_frame = quads
            .iter()
            .find(|q| q.destination.w == CREDITS_SIZE.x && q.destination.h == CREDITS_SIZE.y)
            .expect("video credits frame quad");
        assert_eq!(video_credits_frame.color.a, 1.0);
    }
}
