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
//! innermost layer; on the dialog it acts as Cancel. While the confirmation box, key capture or an
//! open combo list owns the pointer, the layer below is occluded: it arms, presses, activates and
//! shows tooltips for nothing, and its button tracker only follows the physical button state.
//! An open combo list is a [`widgets::PopupList`]: clicks anywhere close or select through it.
//!
//! Keyboard focus follows the SDK `FocusNavGroup` / `PropertyDialog` rules: Tab and Shift+Tab step
//! through the active tab, the page's controls by their layout `tabPosition` and OK / Cancel /
//! Apply, and wrap; a click focuses the control it presses. Space or Enter presses the focused
//! button, check box or radio button (activation on release), arrows move a radio group, step a
//! slider or a combo box and switch tabs; Alt+Up/Down opens a combo box's list; Enter elsewhere
//! presses the default button (docs/gameui.md section 6c).

use super::{
    display::{self, DisplayModes, Resolution},
    keys::{self, KeyRow},
    layout::{Align as LayoutAlign, Control, ControlKind},
    menu::MessageBox,
    placement::{self, Placement},
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
/// Share of the scheme's dim selection alpha (`SectionedListPanel.OutOfFocusSelectedBgColor`,
/// 128) kept on the action cell while a key is captured, so the orange key cell stands out
/// (owner tuning, 2026-10-15; not a retail value). The action text uses the normal row color,
/// since dark selection text would not read on the fainter fill.
const CAPTURE_ACTION_FADE: f32 = 0.5;
/// Rows scrolled per wheel step in lists.
const WHEEL_ROWS: usize = 3;

/// Property sheet tabs, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Gameplay,
    Keyboard,
    Mouse,
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
            Self::Gamepad => Some(Panel::Gamepad),
            Self::Audio => Some(Panel::Audio),
            Self::Video => Some(Panel::Video),
        }
    }
}

/// [`OptionsDialog::report`].
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct OptionsReport {
    pub tab: String,
    pub sub: Option<String>,
    pub focus: Option<String>,
    pub focus_visible: bool,
    pub changed: bool,
    pub list_open: bool,
    pub editing: bool,
    pub capturing_key: bool,
    /// The Resolution combo's choices for the draft's display mode and aspect ratio.
    pub resolutions: Vec<String>,
    /// The placed dialog on the canvas, `[x, y, width, height]` in pixels (filled by the
    /// console, which knows the canvas).
    pub frame: Option<[f32; 4]>,
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

/// A sub-dialog over the options dialog. Edits in a sub-dialog go to the draft; Cancel restores
/// the draft as it was when the sub-dialog opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sub {
    /// Brightness; its value stays in the draft until OK/Apply on the options dialog.
    Gamma,
    AudioCredits,
    VideoCredits,
    /// Keyboard > Advanced; its OK applies its settings at once, like retail.
    KeyboardAdvanced,
    /// Video > Advanced; likewise.
    VideoAdvanced,
}

impl Sub {
    fn is_credits(self) -> bool {
        matches!(self, Self::AudioCredits | Self::VideoCredits)
    }

    fn panel(self) -> Panel {
        match self {
            Self::Gamma => Panel::Gamma,
            Self::AudioCredits => Panel::AudioCredits,
            Self::VideoCredits => Panel::VideoCredits,
            Self::KeyboardAdvanced => Panel::KeyboardAdvanced,
            Self::VideoAdvanced => Panel::VideoAdvanced,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Gamma => "#GameUI_AdjustGamma_Title",
            Self::AudioCredits | Self::VideoCredits => "#GameUI_ThirdPartyTechCredits",
            Self::KeyboardAdvanced => "#GameUI_KeyboardAdvanced_Title",
            Self::VideoAdvanced => "#GameUI_VideoAdvanced_Title",
        }
    }

    fn fallback_size(self) -> Vec2 {
        match self {
            Self::Gamma => GAMMA_SIZE,
            Self::AudioCredits
            | Self::VideoCredits
            | Self::KeyboardAdvanced
            | Self::VideoAdvanced => CREDITS_SIZE,
        }
    }

    /// Copies the settings this sub-dialog's OK applies at once from `from` into `to`; returns
    /// whether it applies anything (retail advanced dialogs write their cvars on OK).
    fn apply_on_ok(self, from: &Config, to: &mut Config) -> bool {
        match self {
            Self::KeyboardAdvanced => to.keyboard = from.keyboard.clone(),
            Self::VideoAdvanced => {
                to.video.advanced = from.video.advanced.clone();
                to.video.fov = from.video.fov;
                to.video.vertical_fov = from.video.vertical_fov;
                to.video.viewmodel_fov = from.video.viewmodel_fov;
            }
            Self::Gamma | Self::AudioCredits | Self::VideoCredits => return false,
        }
        true
    }
}

/// Something under the pointer that reacts to clicks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Control(usize),
    Tab(usize),
    Ok,
    Cancel,
    Apply,
    Close,
}

impl Target {
    fn widget_kind(self, widgets: &[Widget]) -> widgets::WidgetKind {
        match self {
            Target::Control(i) => widgets
                .get(i)
                .map_or(widgets::WidgetKind::Other, |w| w.control.widget_kind()),
            Target::Tab(_) | Target::Ok | Target::Cancel | Target::Apply | Target::Close => {
                widgets::WidgetKind::DialogButton
            }
        }
    }

    fn has_rollover_sound(self, widgets: &[Widget]) -> bool {
        self.widget_kind(widgets).has_rollover_sound()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct OpenCombo {
    choice: Choice,
    items: Vec<String>,
    popup: widgets::PopupList,
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
        self.binding
            .is_some_and(|b| !matches!(b, Binding::ValueLabel(_)))
            || matches!(
                self.control.kind,
                ControlKind::Button | ControlKind::UrlButton
            )
    }

    /// Pressed by Space or Enter while focused (SDK `Button` and its check / radio subclasses).
    fn pushable(&self) -> bool {
        matches!(
            self.binding,
            Some(Binding::Toggle(_) | Binding::Skill(_) | Binding::Command(_))
        ) || matches!(
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

/// Active tooltip overlay state.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveTooltip {
    pub text: String,
    pub pointer: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
enum CreditItem {
    Label(String),
    Image { path: String, size: Vec2 },
}

pub struct OptionsDialog {
    res: Arc<GameUiResources>,
    modes: DisplayModes,
    tab: Tab,
    applied: Config,
    draft: Draft,
    pub(crate) armed: Option<Target>,
    pub(crate) pressed: Option<Target>,
    last_armed: Option<Target>,
    sounds: Vec<String>,
    tracker: widgets::ButtonTracker<Target>,
    drag: Option<(Value, Rect, widgets::SliderRange)>,
    combo: Option<OpenCombo>,
    editing: Option<Editing>,
    keys: KeyList,
    confirm: Option<MessageBox>,
    sub: Option<Sub>,
    /// The draft configuration when the open sub-dialog opened, restored by its Cancel.
    sub_saved: Option<Config>,
    gamma_texture: Option<Texture2D>,
    bink_texture: Option<Texture2D>,
    log: Vec<String>,
    pub(crate) active_tooltip: Option<ActiveTooltip>,
    /// Time of the current input frame, for double clicks.
    now: Option<f64>,
    key_clicks: widgets::DoubleClick<usize>,
    /// Wheel handling for lists: notched on Windows/Linux, accumulated on macOS.
    scroll_mode: widgets::ScrollMode,
    key_wheel: widgets::ScrollAccumulator,
    key_bar: widgets::ScrollBarState,
    /// Where the dialog was dragged (kept while the game runs).
    placement: Placement,
    /// Where the open sub-dialog and the confirmation box were dragged (centered on opening).
    sub_placement: Placement,
    confirm_placement: Placement,
    /// Keyboard focus of the top layer (page or sub-dialog); none until Tab or a click.
    focus: Option<Target>,
    /// The page's focus while a sub-dialog is open, given back when it closes.
    sub_return_focus: Option<Target>,
    key_press: widgets::KeyPress<Target>,
    /// Escape is held and will close the top dialog on release: its close button shows pressed.
    pub(crate) escape_held: bool,
    /// Focus is drawn only after Tab / Shift+Tab; a click focuses without drawing it
    /// (owner request, 2026-10-17).
    focus_visible: bool,
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
            last_armed: None,
            sounds: Vec::new(),
            tracker: widgets::ButtonTracker::capturing(),
            drag: None,
            combo: None,
            editing: None,
            keys: KeyList::default(),
            confirm: None,
            sub: None,
            sub_saved: None,
            gamma_texture: None,
            bink_texture: None,
            log: Vec::new(),
            active_tooltip: None,
            now: None,
            key_clicks: widgets::DoubleClick::default(),
            scroll_mode: widgets::ScrollMode::platform(),
            key_wheel: widgets::ScrollAccumulator::default(),
            key_bar: widgets::ScrollBarState::default(),
            placement: Placement::default(),
            sub_placement: Placement::default(),
            confirm_placement: Placement::default(),
            focus: None,
            sub_return_focus: None,
            key_press: widgets::KeyPress::default(),
            escape_held: false,
            focus_visible: false,
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

/// A dialog layer of `size` (layout units) centered on `canvas`, moveable by its caption.
fn layer_frame(canvas: Vec2, size: Vec2, close: fn(Vec2) -> Rect) -> placement::Frame {
    let scale = Painter::scale_for_canvas(canvas);
    placement::Frame {
        origin: centered(canvas, size, scale),
        size,
        scale,
        canvas,
        min_size: None,
        close,
    }
}

fn main_frame(canvas: Vec2) -> placement::Frame {
    layer_frame(canvas, DIALOG_SIZE, widgets::close_button_rect)
}

fn sub_frame(canvas: Vec2, size: Vec2) -> placement::Frame {
    layer_frame(canvas, size, widgets::close_button_rect)
}

/// The confirmation box has no title-bar close button.
fn confirm_frame(canvas: Vec2, size: Vec2) -> placement::Frame {
    layer_frame(canvas, size, |_| Rect::new(0., 0., 0., 0.))
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

/// Frame size of a sub-dialog: as authored ([`authored_frame_size`]), taller by the footer
/// reflow ([`footer_reflow`]) when content reaches into the footer.
fn frame_size(res: &GameUiResources, sub: Sub) -> Vec2 {
    let size = authored_frame_size(res, sub);
    let Some(layout) = res.layout(sub.panel().layout()) else {
        return size;
    };
    let controls = drawn_controls(sub.panel(), layout);
    let (shift, _) = footer_reflow(sub.panel(), &controls, (size.x, size.y));
    size + vec2(0., shift)
}

/// Frame size of a sub-dialog as authored: its layout's frame record; for a layout without one,
/// the extent of its controls with the left margin repeated on the right and bottom; else the
/// fallback.
fn authored_frame_size(res: &GameUiResources, sub: Sub) -> Vec2 {
    let Some(layout) = res.layout(sub.panel().layout()) else {
        return sub.fallback_size();
    };
    if let Some(frame) = layout
        .controls
        .iter()
        .find(|c| matches!(c.kind, ControlKind::Other(_)) && c.wide >= 200. && c.tall >= 100.)
    {
        return vec2(frame.wide, frame.tall);
    }
    let rects: Vec<(f32, f32, f32, f32)> = layout
        .controls
        .iter()
        .filter(|c| c.visible && !is_frame_chrome(c))
        .map(|c| c.rect((0., 0.)))
        .collect();
    let left = rects.iter().map(|r| r.0).fold(f32::INFINITY, f32::min);
    let right = rects.iter().map(|r| r.0 + r.2).fold(0., f32::max);
    let bottom = rects.iter().map(|r| r.1 + r.3).fold(0., f32::max);
    if rects.is_empty() || !left.is_finite() {
        return sub.fallback_size();
    }
    vec2(right + left, bottom + left)
}

/// Space kept between a dialog's lowest content control and its footer when the footer moves.
const FOOTER_GAP: f32 = 8.;

/// The controls a layout draws (visible, not frame chrome or the frame record itself).
fn drawn_controls(panel: Panel, layout: &super::layout::Layout) -> Vec<&Control> {
    layout
        .controls
        .iter()
        .filter(|c| c.visible || settings::force_visible(panel, &c.name))
        .filter(|c| !matches!(c.kind, ControlKind::Other(_)) && !is_frame_chrome(c))
        .collect()
}

fn control_binding(panel: Panel, control: &Control) -> Option<Binding> {
    settings::binding(panel, &control.name).or_else(|| settings::command_binding(&control.command))
}

/// A dialog footer that overlaps content moves below it.
///
/// VGUI places controls absolutely. The installed Video > Advanced layout keeps older macOS
/// (`[$OSX]`) footer positions that predate its Classic Effects row, so OK/Cancel land on that
/// combo. The footer is the OK/Cancel buttons plus the unbound caption over them (a label
/// without `associate`, at most two button heights above the button row: the installed caption
/// sits 26 units above it on Windows, 6 on macOS). When content reaches
/// into the footer, the footer moves down to `FOOTER_GAP` below the lowest content control, and
/// the frame grows by the same amount. Returns the shift and whether each control is footer.
fn footer_reflow(panel: Panel, controls: &[&Control], parent: (f32, f32)) -> (f32, Vec<bool>) {
    let is_button = |c: &Control| {
        matches!(
            control_binding(panel, c),
            Some(Binding::Command(Command::SubOk | Command::SubCancel))
        )
    };
    let buttons: Vec<(f32, f32)> = controls
        .iter()
        .filter(|c| is_button(c))
        .map(|c| (c.rect(parent).1, c.tall))
        .collect();
    let Some((button_top, button_tall)) =
        buttons.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0))
    else {
        return (0., vec![false; controls.len()]);
    };
    let footer: Vec<bool> = controls
        .iter()
        .map(|c| {
            is_button(c)
                || (c.kind == ControlKind::Label
                    && c.associate.is_empty()
                    && control_binding(panel, c).is_none()
                    && c.rect(parent).1 >= button_top - 2. * button_tall)
        })
        .collect();
    let footer_top = controls
        .iter()
        .zip(&footer)
        .filter(|(_, f)| **f)
        .map(|(c, _)| c.rect(parent).1)
        .fold(f32::INFINITY, f32::min);
    let content_bottom = controls
        .iter()
        .zip(&footer)
        .filter(|(_, f)| !**f)
        .map(|(c, _)| c.rect(parent).1 + c.tall)
        .fold(0., f32::max);
    ((content_bottom + FOOTER_GAP - footer_top).max(0.), footer)
}

/// Controls a VGUI `Frame` creates for itself (`frame_caption`, `frame_close`, resize grips):
/// the dialog frame draws its own title bar and close button.
fn is_frame_chrome(control: &Control) -> bool {
    control
        .name
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("frame_"))
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
    let controls = drawn_controls(panel, layout);
    let (shift, footer) = footer_reflow(panel, &controls, (area.w, area.h));
    let mut widgets: Vec<Widget> = controls
        .into_iter()
        .zip(footer)
        .map(|(control, in_footer)| {
            let (x, y, w, h) = control.rect((area.w, area.h));
            let y = if in_footer { y + shift } else { y };
            let binding = control_binding(panel, control);
            Widget {
                control,
                rect: Rect::new(area.x + x, area.y + y, w, h),
                binding,
                enabled: settings::enabled(binding, control.enabled, draft, applied),
            }
        })
        .collect();
    // SDK `Label`: "If the associate is disabled, so are we".
    let disabled: Vec<&str> = widgets
        .iter()
        .filter(|w| !w.enabled)
        .map(|w| w.control.name.as_str())
        .collect();
    for widget in &mut widgets {
        let associate = &widget.control.associate;
        if !associate.is_empty() && disabled.iter().any(|d| d.eq_ignore_ascii_case(associate)) {
            widget.enabled = false;
        }
    }
    widgets
}

/// Controls whose tops are this close (layout units) share a row of the Tab order: half a
/// standard 24-unit control row.
const FOCUS_ROW_TOLERANCE: f32 = 12.;

/// The Tab order of a layer's controls: enabled interactive controls as they read, left to right
/// in rows from top to bottom (owner decision, 2026-10-17; the installed layouts' `tabPosition`
/// values jump around the pages and leave controls out, so they are not used). A radio group is
/// one stop, its selected button (SDK `RadioButton`).
fn control_order(list: &[Widget], config: &Config) -> Vec<Target> {
    let mut stops: Vec<usize> = list
        .iter()
        .enumerate()
        .filter(|(_, w)| w.enabled && w.interactive())
        .filter(|(_, w)| match w.binding {
            Some(Binding::Skill(skill)) => skill == config.gameplay.skill,
            _ => true,
        })
        .map(|(i, _)| i)
        .collect();
    stops.sort_by(|a, b| list[*a].rect.y.total_cmp(&list[*b].rect.y));
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for i in stops {
        match rows.last_mut() {
            Some(row) if list[i].rect.y - list[row[0]].rect.y <= FOCUS_ROW_TOLERANCE => row.push(i),
            _ => rows.push(vec![i]),
        }
    }
    rows.into_iter()
        .flat_map(|mut row| {
            row.sort_by(|a, b| list[*a].rect.x.total_cmp(&list[*b].rect.x));
            row
        })
        .map(Target::Control)
        .collect()
}

/// Visible key-list rows below the header.
/// The key list's scroll bar: along the right edge, below the header.
fn key_bar_rect(list: Rect) -> Rect {
    Rect::new(
        list.x + list.w - 1. - SCROLLBAR_WIDTH,
        list.y + KEY_HEADER,
        SCROLLBAR_WIDTH,
        list.h - KEY_HEADER - 1.,
    )
}

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

    /// Restores a placement saved in the settings.
    pub fn set_placement(&mut self, placement: Placement) {
        self.placement = placement;
    }

    /// When the dialog opens on `canvas`: a saved placement that no longer lies fully on it (the
    /// resolution changed) goes back to the centered default.
    pub fn reset_placement_if_off_screen(&mut self, canvas: Vec2) {
        self.placement.reset_if_off_screen(&main_frame(canvas));
    }

    /// The dialog's placement to save, once after a drag or reset changed it.
    pub fn take_placement_changed(&mut self) -> Option<Option<crate::config::DialogPlacement>> {
        self.placement
            .take_changed()
            .then(|| self.placement.saved())
    }

    /// Opens the dialog over `config`, keeping the last selected tab.
    pub fn open(&mut self, config: Config) {
        self.draft = Draft::new(config.clone());
        self.applied = config;
        self.armed = None;
        self.pressed = None;
        self.last_armed = None;
        self.drag = None;
        self.combo = None;
        self.editing = None;
        self.keys.capture = None;
        self.confirm = None;
        self.sub = None;
        self.sub_saved = None;
        self.active_tooltip = None;
        self.placement.end();
        self.sub_placement.reset();
        self.confirm_placement.reset();
        self.focus = None;
        self.sub_return_focus = None;
        self.key_press.reset();
    }

    /// The settings were changed outside the dialog (an unconfirmed video change reverted): the
    /// applied state and the draft follow them.
    pub fn reset_applied(&mut self, config: Config) {
        self.draft = Draft::new(config.clone());
        self.applied = config;
        self.combo = None;
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// The configuration being edited.
    pub fn draft(&self) -> &Config {
        &self.draft.config
    }

    /// The brightness being previewed: the draft's gamma while the brightness sub-dialog is
    /// open (retail applies the slider immediately; Cancel restores the saved value).
    pub fn preview_gamma(&self) -> Option<f32> {
        matches!(self.sub, Some(Sub::Gamma)).then_some(self.draft.config.video.gamma)
    }

    /// The window's display mode changed outside the dialog (for example macOS native full
    /// screen entered with the green button): the applied state follows it, and so does the
    /// draft unless the user already picked a different mode there.
    pub fn sync_display_mode(&mut self, mode: crate::config::DisplayMode) {
        if self.draft.config.video.display_mode == self.applied.video.display_mode {
            self.draft.config.video.display_mode = mode;
        }
        self.applied.video.display_mode = mode;
    }

    /// Whether Apply would change anything.
    pub fn changed(&self) -> bool {
        self.draft.config != self.applied
    }

    /// Messages for the console (placeholder activations).
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// Drains any queued UI sound cues (e.g. rollover or release activation).
    pub fn take_sounds(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sounds)
    }

    /// Closes the innermost layer. Returns `false` when the dialog itself closes (Cancel).
    /// Whether Escape would close a dialog with a close button (the sub-dialog, else the Options
    /// dialog) rather than cancel a drag, key capture, list, text edit or confirmation box.
    pub fn escape_closes(&self) -> bool {
        !(self.placement.active()
            || self.sub_placement.active()
            || self.confirm_placement.active()
            || self.keys.capture.is_some()
            || self.combo.is_some()
            || self.editing.is_some()
            || self.confirm.is_some())
    }

    pub fn escape(&mut self) -> bool {
        // A drag or resize in progress is undone first.
        if self.confirm_placement.cancel() || self.sub_placement.cancel() || self.placement.cancel()
        {
            return true;
        }
        self.drag = None;
        self.last_armed = None;
        self.active_tooltip = None;
        if self.keys.capture.take().is_some() {
            self.focus = self.key_list_target();
            return true;
        }
        if self.combo.take().is_some()
            || self.editing.take().is_some()
            || self.confirm.take().is_some()
        {
            return true;
        }
        if self.sub.is_some() {
            self.cancel_sub();
            return true;
        }
        false
    }

    /// Returns the localized tooltip for `control_name` in `panel`, if the layout provides one.
    pub fn tip_for_control(&self, panel: Panel, control_name: &str) -> Option<String> {
        let layout = self.res.layout(panel.layout())?;
        let control = layout.get(control_name)?;
        if control.tooltip.trim().is_empty() {
            return None;
        }
        let text = self.res.localization.localize(&control.tooltip);
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Returns the tooltip text that should be shown for the given pointer position on the active tab, if any.
    pub fn tooltip_at(&self, canvas: Vec2, pointer: Vec2) -> Option<String> {
        let panel = self.tab.panel()?;
        let scale = Painter::scale_for_canvas(canvas);
        let origin = self.placement.place(&main_frame(canvas)).origin;
        let dialog_pointer = (pointer - origin) / scale;
        let page = page_rect();
        if !page.contains(dialog_pointer) {
            return None;
        }
        let widgets = layout_widgets(&self.res, panel, page, &self.draft, &self.applied);
        let hovered = widgets
            .iter()
            .rev()
            .find(|w| w.rect.contains(dialog_pointer))?;
        self.tip_for_control(panel, &hovered.control.name)
    }

    /// Currently active tooltip overlay (if any).
    pub fn active_tooltip(&self) -> Option<&ActiveTooltip> {
        self.active_tooltip.as_ref()
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
        self.active_tooltip = None;
        self.tracker.reset();
        self.key_press.reset();
        if matches!(self.focus, Some(Target::Control(_))) {
            self.focus = None;
        }
        self.tab = tab;
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
        self.now = input.time;
        let canvas = vec2(measure.canvas.width(), measure.canvas.height());
        let scale = Painter::scale_for_canvas(canvas);
        let local = |origin: Vec2| input.pointer.map(|p| (p - origin) / scale);
        if let Some(mut confirm) = self.confirm.take() {
            self.occlude(input);
            let layout = confirm.layout(|t| measure.text_width(t, false));
            let frame = confirm_frame(canvas, layout.size);
            if self.confirm_placement.input(
                &frame,
                input.pointer,
                input.click,
                input.held,
                input.time,
            ) {
                self.confirm = Some(confirm);
                return OptionsOutcome::None;
            }
            let pointer = local(self.confirm_placement.place(&frame).origin);
            let down = input.click || input.held;
            let outcome = confirm
                .input(&layout, pointer, down, input.click, input.released, false)
                .or_else(|| confirm.keys(input));
            self.sounds.extend(confirm.take_sounds());
            match outcome {
                Some(true) => self.use_default_keys(),
                Some(false) => {}
                None => self.confirm = Some(confirm),
            }
            return OptionsOutcome::None;
        }
        if let Some(sub) = self.sub {
            self.active_tooltip = None;
            let size = self.sub_frame_size(sub, |s| measure.text_width(s, false));
            if sub.is_credits()
                && !self.credits_fits_window(sub, canvas, |s| measure.text_width(s, false))
            {
                let scaled_size = size * scale;
                let msg = format!(
                    "Credits dialog content ({}x{}px) cannot fit window bounds ({}x{}px) at current resolution",
                    scaled_size.x.round(),
                    scaled_size.y.round(),
                    canvas.x.round(),
                    canvas.y.round(),
                );
                if !self.log.iter().any(|l| l == &msg) {
                    self.log.push(msg);
                }
            }
            let frame = sub_frame(canvas, size);
            let pointer = local(self.sub_placement.place(&frame).origin);
            // A combo list opened in the sub-dialog is the topmost layer there.
            if let Some(combo) = self.combo.take() {
                self.occlude(input);
                self.combo_input(combo, input, pointer);
                return OptionsOutcome::None;
            }
            if self
                .sub_placement
                .input(&frame, input.pointer, input.click, input.held, input.time)
            {
                self.occlude(input);
                return OptionsOutcome::None;
            }
            return self.sub_input(sub, size, input, pointer);
        }
        let main = main_frame(canvas);
        let pointer = local(self.placement.place(&main).origin);
        if let Some(row) = self.keys.capture {
            self.occlude(input);
            if let Some(key) = input.binding_keys.iter().find(|k| *k != "Escape") {
                if let Some(KeyRow::Action { command, .. }) = self.res.key_rows.get(row) {
                    keys::assign(&mut self.draft.config.keys, command, key);
                }
                self.keys.capture = None;
                // The key list leaves its edit mode with the keyboard (arrows and Enter go to it).
                self.focus = self.key_list_target();
            }
            return OptionsOutcome::None;
        }
        if let Some(combo) = self.combo.take() {
            self.occlude(input);
            self.combo_input(combo, input, pointer);
            return OptionsOutcome::None;
        }
        if self
            .placement
            .input(&main, input.pointer, input.click, input.held, input.time)
        {
            self.occlude(input);
            return OptionsOutcome::None;
        }
        self.page_input(input, pointer, measure)
    }

    /// The dialog's state for run reports: the open tab and sub-dialog, the keyboard focus (a
    /// control's layout name, `tab`, `ok`, `cancel` or `apply`), whether focus is drawn, and
    /// whether Apply would change anything.
    pub fn report(&self) -> OptionsReport {
        let list = match self.sub {
            Some(sub) => {
                let size = self.sub_frame_size(sub, |s| s.chars().count() as f32 * 7.);
                layout_widgets(
                    &self.res,
                    sub.panel(),
                    Rect::new(0., 0., size.x, size.y),
                    &self.draft,
                    &self.applied,
                )
            }
            None => self.tab.panel().map_or_else(Vec::new, |panel| {
                layout_widgets(&self.res, panel, page_rect(), &self.draft, &self.applied)
            }),
        };
        let focus = self.focus.map(|target| match target {
            Target::Control(i) => list
                .get(i)
                .map_or_else(String::new, |w| w.control.name.clone()),
            Target::Tab(_) => "tab".into(),
            Target::Ok => "ok".into(),
            Target::Cancel => "cancel".into(),
            Target::Apply => "apply".into(),
            Target::Close => "close".into(),
        });
        OptionsReport {
            tab: format!("{:?}", self.tab),
            sub: self.sub.map(|s| format!("{s:?}")),
            focus,
            focus_visible: self.focus_visible,
            changed: self.changed(),
            list_open: self.combo.is_some(),
            editing: self.editing.is_some(),
            capturing_key: self.keys.capture.is_some(),
            resolutions: self.resolution_labels(),
            frame: None,
        }
    }

    /// The Resolution combo's choices as shown (for the report).
    fn resolution_labels(&self) -> Vec<String> {
        let offered = settings::offered_resolutions(&self.modes, &self.draft);
        display::resolution_choices(&offered, self.draft.aspect)
            .into_iter()
            .map(Resolution::label)
            .collect()
    }

    /// The Keyboard page's key list as a focus target.
    fn key_list_target(&self) -> Option<Target> {
        let list = layout_widgets(
            &self.res,
            Panel::Keyboard,
            page_rect(),
            &self.draft,
            &self.applied,
        );
        list.iter()
            .position(|w| w.binding == Some(Binding::KeyList))
            .map(Target::Control)
    }

    /// The Keyboard page is waiting for a key to bind (held keys must not repeat into it).
    pub fn capturing_key(&self) -> bool {
        self.keys.capture.is_some()
    }

    /// The dialog's frame on `canvas`, in canvas pixels, where it is placed.
    pub fn frame_bounds(&self, canvas: Vec2) -> Rect {
        let frame = main_frame(canvas);
        let placed = self.placement.place(&frame);
        Rect::new(
            placed.origin.x,
            placed.origin.y,
            placed.size.x * frame.scale,
            placed.size.y * frame.scale,
        )
    }

    /// A layer that takes every click while it is open, wherever the pointer is: a sub-dialog
    /// (retail opens them modal), the confirmation box, an open drop-down list (an outside
    /// click closes it) or a key capture.
    pub fn modal_layer_open(&self) -> bool {
        self.sub.is_some()
            || self.confirm.is_some()
            || self.combo.is_some()
            || self.keys.capture.is_some()
    }

    /// A layer above the dialog page owns the pointer this frame.
    fn occlude(&mut self, input: &Input) {
        self.active_tooltip = None;
        self.armed = None;
        self.pressed = None;
        self.last_armed = None;
        self.tracker.occlude(input.held);
    }

    fn combo_input(&mut self, mut combo: OpenCombo, input: &Input, pointer: Option<Vec2>) {
        let popup = &mut combo.popup;
        popup.wheel(input.wheel, self.scroll_mode);
        if input.pressed(Key::Up) || input.pressed(Key::Down) {
            popup.step(if input.pressed(Key::Up) { -1 } else { 1 });
        }
        let event = popup.pointer(pointer, input.held, input.click, input.released, self.now);
        let pick = match event {
            Some(widgets::PopupEvent::Select(index)) => Some(Some(index)),
            Some(widgets::PopupEvent::Close) => Some(None),
            None if input.pressed(Key::Enter) => Some(popup.highlight),
            None => None,
        };
        match pick {
            Some(Some(index)) => {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
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
        if let Some((value, rect, range)) = self.drag {
            if input.held {
                if let Some(p) = pointer {
                    let fraction = widgets::slider_fraction_at(rect, p.x);
                    value.set(&mut self.draft.config, range.value(fraction));
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
        let armed_target = hit.filter(|target| match target {
            Target::Control(i) => widgets.get(*i).is_some_and(|w| w.enabled),
            _ => true,
        });
        if let Some(target) = armed_target {
            if armed_target != self.last_armed && target.has_rollover_sound(widgets) {
                self.sounds.push("ui/buttonrollover.wav".to_string());
            }
        }
        self.last_armed = armed_target;
        if input.click {
            let keep_editing = matches!(
                (&self.editing, pointer),
                (Some(editing), Some(p)) if editing.rect.contains(p)
            );
            if !keep_editing {
                self.commit_entry();
            }
            // SDK `Button::OnMousePressed` (and the other controls) request focus on a press;
            // focus taken by the mouse is not drawn.
            if let Some(target) = armed_target.filter(|t| *t != Target::Close) {
                self.focus = Some(target);
                self.focus_visible = false;
                self.key_press.reset();
            }
        }
        let is_slider = matches!(
            hit,
            Some(Target::Control(i)) if matches!(widgets.get(i), Some(w) if matches!(w.binding, Some(Binding::Slider(_))))
        );
        // A press captured by another control keeps the mouse while it crosses a slider.
        if is_slider && self.tracker.pressed().is_none() {
            self.tracker.reset();
            self.pressed = hit.filter(|_| input.held || input.click);
            if input.click {
                return hit;
            }
            return None;
        }
        let activated = self
            .tracker
            .update(hit, input.held, input.click, input.released);
        self.pressed = self.tracker.pressed();
        self.armed = self.tracker.armable(hit);
        activated
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
            (Target::Close, widgets::close_button_rect(DIALOG_SIZE)),
        ]);
        let active = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        let mut order = vec![Target::Tab(active)];
        order.extend(control_order(&list, &self.draft.config));
        order.extend([Target::Ok, Target::Cancel]);
        if self.changed() {
            order.push(Target::Apply);
        }
        if let Some(outcome) = self.focus_keys(&list, &order, input) {
            return outcome;
        }
        let key_list_focus = matches!(
            self.focus,
            Some(Target::Control(i)) if matches!(list.get(i).and_then(|w| w.binding), Some(Binding::KeyList))
        );
        if self.tab == Tab::Keyboard {
            self.key_list_keys(input, &list, key_list_focus);
            if self.key_list_bar(input, pointer, &list) {
                return OptionsOutcome::None;
            }
        }
        let clicked = self.track(&list, &extra, pointer, input);
        self.active_tooltip = if !input.click && !input.held {
            pointer.and_then(|dialog_p| {
                if !page.contains(dialog_p) {
                    return None;
                }
                let panel = self.tab.panel()?;
                let widget = list.iter().rev().find(|w| w.rect.contains(dialog_p))?;
                let tip = self.tip_for_control(panel, &widget.control.name)?;
                input.pointer.map(|canvas_p| ActiveTooltip {
                    text: tip,
                    pointer: canvas_p,
                })
            })
        } else {
            None
        };
        match clicked {
            Some(Target::Tab(i)) => {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
                self.select_tab(Tab::ALL[i]);
                self.focus = Some(Target::Tab(i));
            }
            Some(Target::Ok) => {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
                return self.ok();
            }
            Some(Target::Cancel) | Some(Target::Close) => {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
                return OptionsOutcome::Close(None);
            }
            Some(Target::Apply) => {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
                return self.apply();
            }
            Some(Target::Control(i)) => {
                if let (Some(widget), Some(p)) = (list.get(i), pointer) {
                    if !matches!(widget.binding, Some(Binding::Slider(_))) {
                        self.sounds.push("ui/buttonclickrelease.wav".to_string());
                    }
                    return self.activate(widget, p);
                }
            }
            None => {}
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
                if command.placeholder() {
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
                let range = value.slider_range(widget.control.range);
                value.set(&mut self.draft.config, range.value(fraction));
                self.drag = Some((value, widget.rect, range));
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
            Some(Binding::Choice(choice)) => self.open_combo(choice, widget.rect),
            Some(Binding::Command(command)) => return self.command(command),
            Some(Binding::KeyList) => self.key_list_click(widget.rect, pointer),
            Some(Binding::ValueLabel(_)) | None => {}
        }
        OptionsOutcome::None
    }

    fn open_combo(&mut self, choice: Choice, rect: Rect) {
        let env = Env {
            localization: &self.res.localization,
            languages: &self.res.spoken_languages,
            modes: &self.modes,
        };
        let items = choice.items(&env, &self.draft);
        let selected = choice.selected(&env, &self.draft);
        let popup = widgets::PopupList::below(rect, items.len(), selected);
        self.combo = Some(OpenCombo {
            choice,
            items,
            popup,
        });
    }

    /// Keyboard focus keys of the top layer, before the pointer: Tab / Shift+Tab, the focused
    /// control's own keys and Space / Enter on a focused button. Returns an outcome when the
    /// frame's keys were consumed. Text being edited keeps every key but Tab.
    fn focus_keys(
        &mut self,
        list: &[Widget],
        order: &[Target],
        input: &Input,
    ) -> Option<OptionsOutcome> {
        if self.focus.is_some_and(|f| !order.contains(&f)) {
            self.focus = None;
            self.key_press.reset();
        }
        if input.pressed(Key::Tab) {
            self.commit_entry();
            self.key_press.reset();
            self.focus = widgets::focus_step(self.focus, order, !input.shift);
            self.focus_visible = true;
            if let Some(Target::Control(i)) = self.focus {
                if let Some(
                    widget @ Widget {
                        binding: Some(Binding::Entry(_)),
                        ..
                    },
                ) = list.get(i)
                {
                    // A focused text entry edits at once (SDK `TextEntry` takes the caret).
                    let _ = self.activate(widget, widget.rect.center());
                }
            }
            return Some(OptionsOutcome::None);
        }
        if self.editing.is_some() {
            return None;
        }
        let focus = self.focus?;
        let widget = match focus {
            Target::Control(i) => list.get(i),
            _ => None,
        };
        // SDK `RadioButton`: arrows select the previous / next button of the group (wrapping).
        if let Some(Binding::Skill(skill)) = widget.and_then(|w| w.binding) {
            let step = if input.pressed(Key::Up) || input.pressed(Key::Left) {
                -1
            } else if input.pressed(Key::Down) || input.pressed(Key::Right) {
                1
            } else {
                0
            };
            if step != 0 {
                let mut group: Vec<(u32, usize, u8)> = list
                    .iter()
                    .enumerate()
                    .filter_map(|(i, w)| match w.binding {
                        Some(Binding::Skill(s)) if w.enabled => {
                            Some((w.control.sub_tab_position, i, s))
                        }
                        _ => None,
                    })
                    .collect();
                group.sort_unstable();
                if let Some(at) = group.iter().position(|(_, _, s)| *s == skill) {
                    let next = (at as i32 + step).rem_euclid(group.len() as i32) as usize;
                    let (_, index, next_skill) = group[next];
                    self.draft.config.gameplay.skill = next_skill;
                    self.focus = Some(Target::Control(index));
                }
                return Some(OptionsOutcome::None);
            }
        }
        let pushable = match focus {
            Target::Ok | Target::Cancel | Target::Apply => true,
            Target::Control(_) => widget.is_some_and(|w| w.pushable()),
            Target::Tab(_) | Target::Close => false,
        };
        if pushable || self.key_press.pressed().is_some() {
            let key = [Key::Space, Key::Enter]
                .into_iter()
                .find(|k| pushable && input.pressed(*k));
            let pressed = self.key_press.pressed();
            if let Some(target) = self
                .key_press
                .update(Some(focus), key, |k| input.held_keys.contains(&k))
            {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
                return Some(match target {
                    Target::Ok => self.ok(),
                    // A sub-dialog's own Close (credits) closes the sub-dialog only.
                    Target::Cancel if self.sub.is_some() => self.command(Command::SubCancel),
                    Target::Cancel => OptionsOutcome::Close(None),
                    Target::Apply => self.apply(),
                    Target::Control(i) => match list.get(i) {
                        Some(widget) => self.activate(widget, widget.rect.center()),
                        None => OptionsOutcome::None,
                    },
                    Target::Tab(_) | Target::Close => OptionsOutcome::None,
                });
            }
            if key.is_some() || pressed.is_some() {
                return Some(OptionsOutcome::None);
            }
        }
        match (focus, widget.and_then(|w| w.binding)) {
            // SDK `PropertySheet`: Left / Right on the focused tab switch pages.
            (Target::Tab(i), _) => {
                let next = if input.pressed(Key::Left) {
                    i.checked_sub(1)
                } else if input.pressed(Key::Right) {
                    Some(i + 1).filter(|n| *n < Tab::ALL.len())
                } else {
                    return None;
                };
                if let Some(next) = next {
                    self.sounds.push("ui/buttonclickrelease.wav".to_string());
                    self.select_tab(Tab::ALL[next]);
                    self.focus = Some(Target::Tab(next));
                }
                Some(OptionsOutcome::None)
            }
            // SDK `Slider`: Left / Down and Right / Up step by one unit, Page Down / Up by a tick.
            // The retail slider classes are not in the SDK; a unit is a hundredth of the range
            // and a tick a tenth (unverified).
            (Target::Control(_), Some(Binding::Slider(value))) => {
                let step = if input.pressed(Key::Left) || input.pressed(Key::Down) {
                    -0.01
                } else if input.pressed(Key::Right) || input.pressed(Key::Up) {
                    0.01
                } else if input.pressed(Key::PageDown) {
                    -0.1
                } else if input.pressed(Key::PageUp) {
                    0.1
                } else {
                    return None;
                };
                let range = value.slider_range(widget?.control.range);
                let fraction = range.fraction(value.get(&self.draft.config));
                value.set(
                    &mut self.draft.config,
                    range.value((fraction + step).clamp(0., 1.)),
                );
                Some(OptionsOutcome::None)
            }
            // SDK `ComboBox`: Up / Down (and Home / End) select without opening the list;
            // Alt+Up / Down opens it; Enter is taken by the combo box.
            (Target::Control(_), Some(Binding::Choice(choice))) => {
                let vertical = input.pressed(Key::Up) || input.pressed(Key::Down);
                if input.alt && vertical {
                    self.open_combo(choice, widget?.rect);
                    return Some(OptionsOutcome::None);
                }
                let env = Env {
                    localization: &self.res.localization,
                    languages: &self.res.spoken_languages,
                    modes: &self.modes,
                };
                let count = choice.items(&env, &self.draft).len();
                let selected = choice.selected(&env, &self.draft);
                let last = count.checked_sub(1)?;
                let next = if input.pressed(Key::Up) {
                    selected.map_or(0, |s| s.saturating_sub(1))
                } else if input.pressed(Key::Down) {
                    selected.map_or(0, |s| (s + 1).min(last))
                } else if input.pressed(Key::Home) {
                    0
                } else if input.pressed(Key::End) {
                    last
                } else if input.pressed(Key::Enter) {
                    return Some(OptionsOutcome::None);
                } else {
                    return None;
                };
                if Some(next) != selected {
                    choice.select(&env, &mut self.draft, next);
                }
                Some(OptionsOutcome::None)
            }
            _ => None,
        }
    }

    fn open_sub(&mut self, sub: Sub) {
        self.sub_placement.reset();
        self.sub_saved = Some(self.draft.config.clone());
        self.sub = Some(sub);
        self.sub_return_focus = self.focus.take();
        // The credits' Close button is their only control: it holds the focus (drawn after Tab).
        if sub.is_credits() {
            self.focus = Some(Target::Cancel);
        }
        self.key_press.reset();
    }

    /// Focus goes back to the page control that opened the closed sub-dialog.
    fn close_sub(&mut self) {
        self.sub = None;
        self.focus = self.sub_return_focus.take();
        self.key_press.reset();
    }

    /// Cancel, Escape or the title-bar close of a sub-dialog: its edits are dropped.
    fn cancel_sub(&mut self) {
        if let Some(saved) = self.sub_saved.take() {
            self.draft.config = saved;
        }
        self.close_sub();
    }

    /// OK of a sub-dialog: its edits stay in the draft, and an advanced dialog applies its own
    /// settings at once (other unapplied edits stay unapplied).
    fn ok_sub(&mut self) -> OptionsOutcome {
        self.commit_entry();
        self.sub_saved = None;
        let Some(sub) = self.sub else {
            return OptionsOutcome::None;
        };
        self.close_sub();
        let mut applied = self.applied.clone();
        if sub.apply_on_ok(&self.draft.config, &mut applied) && applied != self.applied {
            self.applied = applied;
            return OptionsOutcome::Apply(self.applied.clone());
        }
        OptionsOutcome::None
    }

    pub(crate) fn command(&mut self, command: Command) -> OptionsOutcome {
        self.tracker.reset();
        let loc = &self.res.localization;
        match command {
            Command::GammaDialog => self.open_sub(Sub::Gamma),
            Command::AudioCredits => self.open_sub(Sub::AudioCredits),
            Command::VideoCredits => self.open_sub(Sub::VideoCredits),
            Command::KeyAdvanced => self.open_sub(Sub::KeyboardAdvanced),
            Command::KeyDefaults => {
                self.confirm_placement.reset();
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
            Command::VideoAdvanced => self.open_sub(Sub::VideoAdvanced),
            // Placeholder: disabled, so activation only logs its label (see `activate`).
            Command::SteamInput => {}
            Command::SubOk => return self.ok_sub(),
            Command::SubCancel => self.cancel_sub(),
        }
        OptionsOutcome::None
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

    /// The key list's scroll bar while it has more rows than fit.
    fn key_bar_layout(&self, rect: Rect) -> Option<widgets::ScrollBarLayout> {
        let total = self.res.key_rows.len();
        key_list_scrollable(rect, total).then(|| {
            widgets::ScrollBarLayout::new(
                key_bar_rect(rect),
                self.keys.first,
                key_rows_visible(rect),
                total,
            )
        })
    }

    /// A gesture on the key list's scroll bar owns the pointer from press to release and
    /// scrolls live; the rest of the page is occluded meanwhile. Returns whether it took the
    /// frame.
    fn key_list_bar(&mut self, input: &Input, pointer: Option<Vec2>, list: &[Widget]) -> bool {
        let Some(rect) = Self::key_list_rect(list) else {
            return false;
        };
        let Some(layout) = self.key_bar_layout(rect) else {
            return false;
        };
        if !self.key_bar.wants(&layout, pointer, input.click) {
            return false;
        }
        let page = key_rows_visible(rect);
        if let Some(first) = self.key_bar.pointer(
            &layout,
            pointer,
            input.held,
            input.click,
            self.now,
            self.keys.first,
            page,
        ) {
            self.keys.first = first;
        }
        self.occlude(input);
        true
    }

    fn key_list_rect(list: &[Widget]) -> Option<Rect> {
        list.iter()
            .find(|w| w.binding == Some(Binding::KeyList))
            .map(|w| w.rect)
    }

    /// Wheel scrolling and, while the list has keyboard focus (or nothing has), arrow navigation
    /// and Enter-to-edit on the Keyboard page.
    fn key_list_keys(&mut self, input: &Input, list: &[Widget], focused: bool) {
        let Some(rect) = Self::key_list_rect(list) else {
            return;
        };
        let rows = &self.res.key_rows;
        let visible = key_rows_visible(rect);
        let max_first = rows.len().saturating_sub(visible);
        if self.combo.is_none() {
            let rows = self.scroll_mode.rows_per_step(WHEEL_ROWS) as i32;
            let steps = self.key_wheel.steps(input.wheel, self.scroll_mode) * rows;
            self.keys.first = (self.keys.first as i32 - steps).clamp(0, max_first as i32) as usize;
        }
        if !focused {
            return;
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
        let body = Rect::new(rect.x, rect.y + KEY_HEADER, rect.w, rect.h - KEY_HEADER);
        if !body.contains(p) {
            return;
        }
        // The scroll bar handles its own presses (`key_list_bar`).
        if key_list_scrollable(rect, total) && key_bar_rect(rect).contains(p) {
            return;
        }
        let index = self.keys.first + ((p.y - body.y) / MENU_ROW).floor() as usize;
        if matches!(self.res.key_rows.get(index), Some(KeyRow::Action { .. })) {
            self.keys.selected = Some(index);
            // A double click on an action row edits its key, like the Edit Key button.
            if self.key_clicks.register(index, self.now) {
                self.keys.capture = Some(index);
            }
        }
    }

    fn sub_input(
        &mut self,
        sub: Sub,
        size: Vec2,
        input: &Input,
        pointer: Option<Vec2>,
    ) -> OptionsOutcome {
        if sub.is_credits() {
            self.credits_input(size, input, pointer);
            return OptionsOutcome::None;
        }
        let res = self.res.clone();
        let area = Rect::new(0., 0., size.x, size.y);
        let list = layout_widgets(&res, sub.panel(), area, &self.draft, &self.applied);
        let order = control_order(&list, &self.draft.config);
        if let Some(outcome) = self.focus_keys(&list, &order, input) {
            return outcome;
        }
        let close_rect = widgets::close_button_rect(size);
        let extra = [(Target::Close, close_rect)];
        match self.track(&list, &extra, pointer, input) {
            Some(Target::Close) => {
                self.sounds.push("ui/buttonclickrelease.wav".to_string());
                self.command(Command::SubCancel)
            }
            Some(Target::Control(i)) => match (list.get(i), pointer) {
                (Some(widget), Some(p)) => {
                    if !matches!(widget.binding, Some(Binding::Slider(_))) {
                        self.sounds.push("ui/buttonclickrelease.wav".to_string());
                    }
                    self.activate(widget, p)
                }
                _ => OptionsOutcome::None,
            },
            _ => OptionsOutcome::None,
        }
    }

    fn credits_items(&self, sub: Sub) -> Vec<CreditItem> {
        let mut items = Vec::new();
        if let Some(layout) = self.res.layout(sub.panel().layout()) {
            let mut controls: Vec<&Control> = layout
                .controls
                .iter()
                .filter(|c| {
                    c.visible
                        && !c.name.eq_ignore_ascii_case("Button2")
                        && !matches!(c.kind, ControlKind::Button | ControlKind::Other(_))
                })
                .collect();
            controls.sort_by(|a, b| {
                a.y.resolve(CREDITS_SIZE.y)
                    .partial_cmp(&b.y.resolve(CREDITS_SIZE.y))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for c in controls {
                match c.kind {
                    ControlKind::Label => {
                        items.push(CreditItem::Label(c.label.clone()));
                    }
                    ControlKind::ImagePanel => {
                        items.push(CreditItem::Image {
                            path: c.image.clone(),
                            size: Vec2::new(c.wide, c.tall),
                        });
                    }
                    _ => {}
                }
            }
        }
        if items.is_empty() {
            match sub {
                Sub::AudioCredits => {
                    items.push(CreditItem::Label("#GameUI_Miles_Audio".into()));
                    items.push(CreditItem::Label("#GameUI_Miles_Voice".into()));
                    items.push(CreditItem::Label("#GameUI_Bink".into()));
                    items.push(CreditItem::Image {
                        path: "vgui/techcredits/bink".into(),
                        size: Vec2::new(100., 40.),
                    });
                }
                Sub::VideoCredits => {
                    items.push(CreditItem::Label("#GameUI_Bink".into()));
                    items.push(CreditItem::Image {
                        path: "vgui/techcredits/bink".into(),
                        size: Vec2::new(100., 40.),
                    });
                }
                _ => {}
            }
        }
        items
    }

    fn measure_credits_height(
        &self,
        items: &[CreditItem],
        content_w: f32,
        measure_w: impl Fn(&str) -> f32,
    ) -> f32 {
        let mut total_h = 0.0;
        let loc = &self.res.localization;
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                total_h += 10.0;
            }
            match item {
                CreditItem::Label(token) => {
                    let text = loc.localize(token);
                    let lines = widgets::wrap(&text, content_w, &measure_w);
                    total_h += lines.len() as f32 * widgets::FONT_TALL;
                }
                CreditItem::Image { size, .. } => {
                    total_h += size.y;
                }
            }
        }
        total_h
    }

    /// Frame size of a sub-dialog: sized flexibly to wrapped content for credits dialogs,
    /// or layout / fallback size for other sub-dialogs.
    pub(crate) fn sub_frame_size(&self, sub: Sub, measure_w: impl Fn(&str) -> f32) -> Vec2 {
        if sub.is_credits() {
            let items = self.credits_items(sub);
            let content_w = CREDITS_SIZE.x - 32.;
            let content_h = self.measure_credits_height(&items, content_w, measure_w);
            // 34. top inset (title bar) + content_h + 12. gap + 24. close button + 10. bottom margin = 46. bottom inset
            vec2(CREDITS_SIZE.x, 34. + content_h + 46.)
        } else {
            frame_size(&self.res, sub)
        }
    }

    /// Returns whether the credits subdialog content fits completely within the window at `canvas` resolution.
    pub(crate) fn credits_fits_window(
        &self,
        sub: Sub,
        canvas: Vec2,
        measure_w: impl Fn(&str) -> f32,
    ) -> bool {
        let scale = Painter::scale_for_canvas(canvas);
        let size = self.sub_frame_size(sub, measure_w);
        size.x * scale <= canvas.x && size.y * scale <= canvas.y
    }

    fn draw_credits_subdialog(&self, sp: &Painter, sub: Sub, size: Vec2, modal: bool) {
        let loc = &self.res.localization;
        let items = self.credits_items(sub);
        let content_w = size.x - 32.;
        let mut y_cursor = 34.;

        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                y_cursor += 10.0;
            }
            match item {
                CreditItem::Label(token) => {
                    let text = loc.localize(token);
                    let lines = widgets::wrap(&text, content_w, |s| sp.text_width(s, false));
                    for line in lines {
                        sp.text(&line, Vec2::new(16., y_cursor), sp.theme.text, false);
                        y_cursor += widgets::FONT_TALL;
                    }
                }
                CreditItem::Image {
                    path,
                    size: img_size,
                } => {
                    let texture = if path.to_ascii_lowercase().contains("bink") {
                        self.bink_texture.as_ref()
                    } else {
                        None
                    };
                    if let Some(tex) = texture {
                        sp.image(Rect::new(16., y_cursor, img_size.x, img_size.y), tex);
                    }
                    y_cursor += img_size.y;
                }
            }
        }

        let close_rect = Rect::new(size.x - 96., size.y - 34., 80., 24.);
        let close_armed = !modal && self.armed == Some(Target::Cancel);
        let close_pressed = !modal
            && (self.pressed == Some(Target::Cancel)
                || self.key_press.pressed() == Some(Target::Cancel));
        sp.button(
            close_rect,
            &loc.text("#GameUI_Close"),
            ButtonState {
                armed: close_armed,
                pressed: close_pressed,
                enabled: !modal,
                default: false,
            },
        );
        if !modal && self.focus_visible && self.focus == Some(Target::Cancel) {
            sp.focus_box(close_rect);
        }
    }

    fn credits_input(&mut self, size: Vec2, input: &Input, pointer: Option<Vec2>) {
        let close_rect = Rect::new(size.x - 96., size.y - 34., 80., 24.);
        let title_close_rect = widgets::close_button_rect(size);
        let extra = [
            (Target::Close, title_close_rect),
            (Target::Cancel, close_rect),
        ];

        // Tab keeps the focus on Close and shows it; Space / Enter press and release it.
        if self.focus_keys(&[], &[Target::Cancel], input).is_some() {
            return;
        }
        if let Some(Target::Close | Target::Cancel) = self.track(&[], &extra, pointer, input) {
            self.sounds.push("ui/buttonclickrelease.wav".to_string());
            self.command(Command::SubCancel);
        }
    }

    // ---- drawing -------------------------------------------------------------------------

    pub fn draw(&self, base: &Painter) {
        let canvas = vec2(base.canvas.width(), base.canvas.height());
        let scale = Painter::scale_for_canvas(canvas);
        let painter_at = |origin: Vec2| Painter {
            canvas: base.canvas,
            fonts: base.fonts,
            theme: base.theme,
            origin,
            scale,
        };
        let p = painter_at(self.placement.place(&main_frame(canvas)).origin);
        // Layers drawn over this dialog (sub-dialogs, the confirmation box) are opaque.
        let stacked = base.theme.stacked();
        let loc = &self.res.localization;
        let env = Env {
            localization: loc,
            languages: &self.res.spoken_languages,
            modes: &self.modes,
        };
        let modal = self.sub.is_some() || self.confirm.is_some();
        let state = |target: Target| {
            (
                !modal && self.armed == Some(target),
                !modal
                    && (self.pressed == Some(target) || self.key_press.pressed() == Some(target)),
            )
        };
        let focused = |target: Target| !modal && self.focus_visible && self.focus == Some(target);
        let (close_armed, close_pressed) = state(Target::Close);
        let close_pressed = close_pressed || (self.escape_held && self.sub.is_none());
        p.frame_with_close(
            DIALOG_SIZE,
            &loc.text("#GameUI_Options"),
            Some(ButtonState {
                armed: close_armed,
                pressed: close_pressed,
                enabled: !modal,
                default: false,
            }),
        );
        let tabs = self.tab_rects(&|t| p.text_width(t, false));
        let page = page_rect();
        let active = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
        p.raised_open_top(page, tabs[active]);
        for (i, rect) in tabs.iter().enumerate() {
            if i != active {
                let (armed, pressed) = state(Target::Tab(i));
                p.tab(
                    *rect,
                    &self.tab_label(Tab::ALL[i]),
                    false,
                    ButtonState::active(!modal, armed, pressed),
                );
            }
        }
        let (armed, pressed) = state(Target::Tab(active));
        p.tab(
            tabs[active],
            &self.tab_label(self.tab),
            true,
            ButtonState::active(!modal, armed, pressed),
        );
        if focused(Target::Tab(active)) {
            p.focus_box(tabs[active]);
        }
        if let Some(panel) = self.tab.panel() {
            let list = layout_widgets(&self.res, panel, page, &self.draft, &self.applied);
            for (i, widget) in list.iter().enumerate() {
                let (armed, pressed) = state(Target::Control(i));
                let focus = focused(Target::Control(i));
                let logical = !modal && self.focus == Some(Target::Control(i));
                self.draw_widget(&p, widget, &env, (armed, pressed), (focus, logical));
            }
        }
        let labels = ["#GameUI_OK", "#GameUI_Cancel", "#GameUI_Apply"];
        let targets = [Target::Ok, Target::Cancel, Target::Apply];
        for ((rect, label), target) in button_rects().into_iter().zip(labels).zip(targets) {
            let (armed, pressed) = state(target);
            let enabled = target != Target::Apply || self.changed();
            p.button(
                rect,
                &loc.text(label),
                ButtonState {
                    armed,
                    pressed,
                    enabled,
                    // No default-button border (owner request, 2026-10-17): focus is the
                    // dashed box alone.
                    default: false,
                },
            );
            if enabled && focused(target) {
                p.focus_box(rect);
            }
        }
        if self.sub.is_none() {
            self.draw_combo(&p);
        }
        if let Some(sub) = self.sub {
            let size = self.sub_frame_size(sub, |s| base.text_width(s, false));
            let sp = Painter {
                canvas: base.canvas,
                fonts: base.fonts,
                theme: &stacked,
                origin: self.sub_placement.place(&sub_frame(canvas, size)).origin,
                scale,
            };
            let sub_modal = self.confirm.is_some();
            let sub_close_armed = !sub_modal && self.armed == Some(Target::Close);
            let sub_close_pressed =
                !sub_modal && (self.pressed == Some(Target::Close) || self.escape_held);
            sp.frame_with_close(
                size,
                &loc.text(sub.title()),
                Some(ButtonState {
                    armed: sub_close_armed,
                    pressed: sub_close_pressed,
                    enabled: !sub_modal,
                    default: false,
                }),
            );
            if sub.is_credits() {
                self.draw_credits_subdialog(&sp, sub, size, sub_modal);
            } else {
                let area = Rect::new(0., 0., size.x, size.y);
                let list = layout_widgets(&self.res, sub.panel(), area, &self.draft, &self.applied);
                for (i, widget) in list.iter().enumerate() {
                    let target = Some(Target::Control(i));
                    let armed = !sub_modal && self.armed == target;
                    let pressed = !sub_modal
                        && (self.pressed == target || self.key_press.pressed() == target);
                    let logical = !sub_modal && self.focus == target;
                    let focus = logical && self.focus_visible;
                    self.draw_widget(&sp, widget, &env, (armed, pressed), (focus, logical));
                }
                self.draw_combo(&sp);
            }
        }
        if let Some(confirm) = &self.confirm {
            let layout = confirm.layout(|t| p.text_width(t, false));
            let cp = Painter {
                theme: &stacked,
                ..painter_at(
                    self.confirm_placement
                        .place(&confirm_frame(canvas, layout.size))
                        .origin,
                )
            };
            confirm.draw(&cp, &layout);
        }
        if let Some(tip) = &self.active_tooltip {
            let tip_painter = Painter {
                canvas: base.canvas,
                fonts: base.fonts,
                theme: base.theme,
                origin: Vec2::ZERO,
                scale,
            };
            let screen_layout_bounds = Rect::new(0., 0., canvas.x / scale, canvas.y / scale);
            tip_painter.tooltip(&tip.text, tip.pointer / scale, screen_layout_bounds);
        }
    }

    /// The open combo list, over the layer (page or sub-dialog) that opened it.
    fn draw_combo(&self, p: &Painter) {
        if let Some(OpenCombo { items, popup, .. }) = &self.combo {
            p.menu_list(
                popup.list,
                items,
                popup.first,
                popup.highlight,
                popup.bar_pressed(),
            );
        }
    }

    /// Draws a control; `focus` is (focus drawn, focus held): the focus look appears only after
    /// keyboard navigation, the key list dims its selection without focus.
    fn draw_widget(
        &self,
        p: &Painter,
        w: &Widget,
        env: &Env,
        (armed, pressed): (bool, bool),
        (focused, logical): (bool, bool),
    ) {
        self.draw_control(p, w, env, (armed, pressed), (focused, logical));
        if !focused || !w.enabled {
            return;
        }
        let pad = widgets::FOCUS_PAD;
        match w.control.kind {
            ControlKind::Button | ControlKind::UrlButton => p.focus_box(w.rect),
            // The whole control, box or circle and label, evenly padded.
            ControlKind::CheckButton | ControlKind::RadioButton => {
                let label = self.res.localization.localize(&w.control.label);
                p.focus_ring(widgets::toggle_content(w.rect, p.text_width(&label, false)).pad(pad));
            }
            ControlKind::Slider => p.focus_ring(widgets::slider_content(w.rect).pad(pad)),
            ControlKind::TextEntry | ControlKind::ListPanel => p.focus_ring(w.rect.pad(pad)),
            _ => {}
        }
    }

    fn draw_control(
        &self,
        p: &Painter,
        w: &Widget,
        env: &Env,
        (armed, pressed): (bool, bool),
        (focused, logical): (bool, bool),
    ) {
        let loc = &self.res.localization;
        let c = w.control;
        let label = loc.localize(&c.label);
        let config = &self.draft.config;
        match (&c.kind, w.binding) {
            (ControlKind::Label, Some(Binding::ValueLabel(value))) => {
                let align = match c.align {
                    LayoutAlign::Center => Align::Center,
                    LayoutAlign::East => Align::Right,
                    _ => Align::Left,
                };
                let text = value.label(config, loc);
                p.label(&text, w.rect, align, w.enabled);
            }
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
            (ControlKind::CheckButton, Some(Binding::Toggle(toggle))) => p.check_box(
                w.rect,
                &label,
                toggle.get(config),
                ButtonState::active(w.enabled, armed, pressed),
            ),
            (ControlKind::CheckButton, _) => {
                p.check_box(w.rect, &label, false, ButtonState::new(false))
            }
            (ControlKind::RadioButton, binding) => {
                let selected = binding == Some(Binding::Skill(config.gameplay.skill));
                p.radio(
                    w.rect,
                    &label,
                    selected,
                    ButtonState::active(w.enabled, armed, pressed),
                )
            }
            (ControlKind::ComboBox, Some(Binding::Choice(choice))) => {
                let open = self
                    .combo
                    .as_ref()
                    .is_some_and(|o| o.popup.anchor == w.rect);
                p.combo(
                    w.rect,
                    &choice.text(env, &self.draft),
                    w.enabled,
                    armed || open,
                    focused,
                )
            }
            (ControlKind::ComboBox, _) => p.combo(w.rect, "", false, false, false),
            (ControlKind::Slider, binding) => {
                let fraction = match binding {
                    Some(Binding::Slider(value)) => {
                        value.slider_range(c.range).fraction(value.get(config))
                    }
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
            (ControlKind::Button, _) => p.button(
                w.rect,
                &label,
                ButtonState {
                    armed,
                    pressed,
                    enabled: w.enabled,
                    default: false,
                },
            ),
            (ControlKind::UrlButton, _) => p.url(
                w.rect,
                &label,
                ButtonState::active(w.enabled, armed, pressed),
            ),
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
            (ControlKind::ListPanel, Some(Binding::KeyList)) => {
                self.draw_key_list(p, w.rect, logical)
            }
            _ => {}
        }
    }

    /// `focused`: the list holds keyboard focus; without it the selected row takes the scheme's
    /// out-of-focus selection (SDK `SectionedListPanel`).
    fn draw_key_list(&self, p: &Painter, rect: Rect, focused: bool) {
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
                    // SDK `SectionedListPanelHeader`: a one-pixel divider two pixels above the
                    // bottom of the header row, inset by one pixel on the left and two on the
                    // right, separating action groups.
                    p.fill(
                        Rect::new(header.x + 1., y + MENU_ROW - 2., inner_w - 3., 1.),
                        theme.list_divider,
                    );
                }
                KeyRow::Action { command, label } => {
                    // While capturing, only the key cell is highlighted (the retail list edits
                    // the key column in place); the action name keeps the normal row look.
                    let capturing = self.keys.capture == Some(index);
                    let selected = self.keys.selected == Some(index) && !capturing;
                    if capturing {
                        p.fill(
                            Rect::new(header.x + action_w, y, inner_w - action_w, MENU_ROW),
                            theme.selection_bg,
                        );
                    }
                    let color = if capturing {
                        // The action stays identifiable with the scheme's dimmer selection.
                        let fill = theme.selection_bg_dim;
                        p.fill(
                            Rect::new(header.x, y, action_w, MENU_ROW),
                            crate::canvas::Color {
                                a: fill.a * CAPTURE_ACTION_FADE,
                                ..fill
                            },
                        );
                        theme.text
                    } else if selected {
                        let (fill, text) = if focused {
                            (theme.selection_bg, theme.selection_text)
                        } else {
                            (theme.selection_bg_dim, theme.selection_text_dim)
                        };
                        p.fill(Rect::new(header.x, y, inner_w, MENU_ROW), fill);
                        text
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
                    let key = if capturing {
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
                key_bar_rect(rect),
                self.keys.first,
                visible,
                rows.len(),
                self.key_bar.pressed(),
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
            "xpos" "28" "ypos" "9" "wide" "150" "tall" "28" "labelText" "#GameUI_ReverseMouse"
            "tooltiptext" "#GameUI_ReverseMouseLabel" }
        "Slider" { "ControlName" "CCvarSlider" "fieldName" "Slider" "xpos" "200" "ypos" "40"
            "wide" "200" "tall" "40" "leftText" "#GameUI_Low" "rightText" "#GameUI_High" }
        "SensitivityLabel" { "ControlName" "TextEntry" "fieldName" "SensitivityLabel"
            "xpos" "400" "ypos" "40" "wide" "50" "tall" "24" }
        "Autoaim" { "ControlName" "ComboBox" "fieldName" "Autoaim" "xpos" "28" "ypos" "172"
            "wide" "175" "tall" "24" }
        "MouseFilter" { "ControlName" "CCvarToggleCheckButton" "fieldName" "MouseFilter"
            "xpos" "28" "ypos" "216" "wide" "150" "tall" "28" "labelText" "#GameUI_MouseFilter"
            "tooltiptext" "#GameUI_MouseFilterLabel" }
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
        "KeyAdvancedButton" { "ControlName" "Button" "fieldName" "KeyAdvancedButton" "xpos" "148"
            "ypos" "276" "wide" "111" "tall" "24" "labelText" "#GameUI_AdvancedEllipsis" }
        "ClearKeyButton" { "ControlName" "Button" "fieldName" "ClearKeyButton" "xpos" "384"
            "ypos" "276" "wide" "105" "tall" "24" "labelText" "#GameUI_ClearKey" }
    }"##;
    const DIFFICULTY: &str = r##""Resource/OptionsSubDifficulty.res" {
        "Skill1Radio" { "ControlName" "RadioButton" "fieldName" "Skill1Radio" "xpos" "20"
            "ypos" "20" "wide" "200" "tall" "24" "tabPosition" "1" "SubTabPosition" "1" }
        "Skill2Radio" { "ControlName" "RadioButton" "fieldName" "Skill2Radio" "xpos" "20"
            "ypos" "60" "wide" "200" "tall" "24" "tabPosition" "1" "SubTabPosition" "2" }
        "Skill3Radio" { "ControlName" "RadioButton" "fieldName" "Skill3Radio" "xpos" "20"
            "ypos" "100" "wide" "200" "tall" "24" "tabPosition" "1" "SubTabPosition" "3" }
    }"##;
    const KEYBOARD_ADVANCED: &str = r##""resource/OptionsSubKeyboardAdvancedDlg.res" {
        "FastSwitchCheck" { "ControlName" "CheckButton" "fieldName" "FastSwitchCheck" "xpos" "20"
            "ypos" "38" "wide" "240" "tall" "24" "labelText" "#GameUI_FastSwitchCheck" }
        "ConsoleCheck" { "ControlName" "CheckButton" "fieldName" "ConsoleCheck" "xpos" "20"
            "ypos" "66" "wide" "244" "tall" "24" "labelText" "#GameUI_DeveloperConsoleCheck" }
        "Button1" { "ControlName" "Button" "fieldName" "Button1" "xpos" "95" "ypos" "100"
            "wide" "80" "tall" "24" "labelText" "#GameUI_OK" "Command" "OK" "Default" "1" }
        "Button2" { "ControlName" "Button" "fieldName" "Button2" "xpos" "182" "ypos" "100"
            "wide" "80" "tall" "24" "labelText" "#GameUI_Cancel" "Command" "Close" }
    }"##;
    const GAMEPAD: &str = r##""Resource\OptionsSubMouse.res" {
        "JoyScale" { "ControlName" "CCvarSlider" "fieldName" "JoyScale" "xpos" "38" "ypos" "43"
            "wide" "260" "tall" "40" }
        "Autoaim" { "ControlName" "ComboBox" "fieldName" "Autoaim" "xpos" "38" "ypos" "116"
            "wide" "175" "tall" "24" }
        "AutoaimScaleLabel" { "ControlName" "Label" "fieldName" "AutoaimScaleLabel" "xpos" "38"
            "ypos" "165" "wide" "179" "tall" "24" "labelText" "#GameUI_AimAssistScale"
            "associate" "AutoaimScale" }
        "AutoaimScale" { "ControlName" "CCvarSlider" "fieldName" "AutoaimScale" "xpos" "38"
            "ypos" "189" "wide" "260" "tall" "40" "enabled" "0" }
        "WeaponSelection" { "ControlName" "ComboBox" "fieldName" "WeaponSelection" "xpos" "38"
            "ypos" "262" "wide" "175" "tall" "24" }
        "SteamInput" { "ControlName" "Button" "fieldName" "SteamInput" "xpos" "240" "ypos" "262"
            "wide" "240" "tall" "24" "labelText" "#GameUI_Open_SteamInput" "Command" "OpenSteamInput" }
    }"##;
    const VIDEO_ADVANCED: &str = r##""resource/OptionsSubVideoAdvancedDlg.res" {
        "OptionsSubVideoAdvancedDlg" { "ControlName" "COptionsSubVideoAdvancedDlg"
            "fieldName" "OptionsSubVideoAdvancedDlg" "wide" "482" "tall" "358" }
        "frame_close" { "ControlName" "Button" "fieldName" "frame_close" "xpos" "457" "ypos" "8"
            "wide" "18" "tall" "18" "labelText" "r" }
        "VSync" { "ControlName" "ComboBox" "fieldName" "VSync" "xpos" "330" "ypos" "196"
            "wide" "132" "tall" "24" }
        "FovSlider" { "ControlName" "CCvarSlider" "fieldName" "FovSlider" "xpos" "176"
            "ypos" "252" "wide" "140" "tall" "50" "minvalue" "75" "maxvalue" "110" }
        "FOVValueLabel" { "ControlName" "Label" "fieldName" "FOVValueLabel" "xpos" "276"
            "ypos" "232" "wide" "32" "tall" "24" "labelText" "75" "textAlignment" "east" }
        "Button1" { "ControlName" "Button" "fieldName" "Button1" "xpos" "290" "ypos" "316"
            "wide" "80" "tall" "24" "labelText" "#GameUI_OK" "Command" "OK" }
        "Button2" { "ControlName" "Button" "fieldName" "Button2" "xpos" "382" "ypos" "316"
            "wide" "80" "tall" "24" "labelText" "#GameUI_Cancel" "Command" "Close" }
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
        localization.insert("GameUI_Open_SteamInput", "Open Steam Input Configuration");
        localization.insert("GameUI_AdvancedEllipsis", "Advanced...");
        localization.insert("GameUI_ReverseMouseLabel", "Reverse mouse up-down axis");
        let mut layouts = BTreeMap::new();
        for (name, text) in [
            ("optionssubmouse", MOUSE),
            ("optionssubvideo", VIDEO),
            ("optionssubkeyboard", KEYBOARD),
            ("optionssubvideogammadlg", GAMMA),
            ("optionssubkeyboardadvanceddlg", KEYBOARD_ADVANCED),
            ("optionssubvideoadvanceddlg", VIDEO_ADVANCED),
            ("optionssubgamepad", GAMEPAD),
            ("optionssubdifficulty", DIFFICULTY),
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
            let first = self.frame(Input {
                pointer: Some(at),
                click: true,
                held: true,
                ..Default::default()
            });
            let second = self.frame(Input {
                pointer: Some(at),
                released: true,
                ..Default::default()
            });
            if !matches!(first, OptionsOutcome::None) {
                first
            } else {
                second
            }
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

    /// Name of the focused control of `panel` (page layers).
    fn focused_name(h: &Harness, panel: Panel) -> Option<String> {
        let list = layout_widgets(
            &h.dialog.res,
            panel,
            page_rect(),
            &h.dialog.draft,
            &h.dialog.applied,
        );
        match h.dialog.focus {
            Some(Target::Control(i)) => list.get(i).map(|w| w.control.name.clone()),
            _ => None,
        }
    }

    fn shift_tab(h: &mut Harness) -> OptionsOutcome {
        h.frame(Input {
            keys: [Key::Tab].into(),
            shift: true,
            ..Default::default()
        })
    }

    #[test]
    fn the_dialog_fits_square_and_portrait_screens() {
        let h = Harness::new();
        for canvas in [
            vec2(1024., 768.),
            vec2(1080., 1080.),
            vec2(1080., 1920.),
            vec2(720., 1280.),
        ] {
            let bounds = h.dialog.frame_bounds(canvas);
            assert!(
                bounds.x >= 0.
                    && bounds.y >= 0.
                    && bounds.x + bounds.w <= canvas.x
                    && bounds.y + bounds.h <= canvas.y,
                "{canvas}: {bounds:?}"
            );
        }
    }

    #[test]
    fn tab_order_reads_left_to_right_top_to_bottom() {
        // Two columns as on the installed Audio page, in a jumbled file order, with controls a
        // few units apart still sharing a row; the disabled combo is skipped.
        let layout = Layout::parse(
            br##""Resource/OptionsSubMouse.res" {
            "Autoaim" { "ControlName" "ComboBox" "fieldName" "Autoaim" "xpos" "254" "ypos" "116"
                "wide" "160" "tall" "24" }
            "MouseFilter" { "ControlName" "CCvarToggleCheckButton" "fieldName" "MouseFilter"
                "xpos" "254" "ypos" "10" "wide" "150" "tall" "28" }
            "SensitivityLabel" { "ControlName" "TextEntry" "fieldName" "SensitivityLabel"
                "xpos" "400" "ypos" "40" "wide" "50" "tall" "24" }
            "ReverseMouse" { "ControlName" "CCvarNegateCheckButton" "fieldName" "ReverseMouse"
                "xpos" "28" "ypos" "9" "wide" "150" "tall" "28" }
            "Slider" { "ControlName" "CCvarSlider" "fieldName" "Slider" "xpos" "200" "ypos" "40"
                "wide" "200" "tall" "40" }
            "MouseRaw" { "ControlName" "CCvarToggleCheckButton" "fieldName" "MouseRaw"
                "xpos" "28" "ypos" "120" "wide" "150" "tall" "28" }
            "AutoaimScale" { "ControlName" "ComboBox" "fieldName" "Autoaim" "xpos" "28"
                "ypos" "200" "wide" "160" "tall" "24" "enabled" "0" }
        }"##,
        )
        .unwrap();
        let mut res = (*resources()).clone();
        res.layouts.insert("optionssubmouse".into(), layout);
        let draft = Draft::new(Config::default());
        let list = layout_widgets(&res, Panel::Mouse, page_rect(), &draft, &draft.config);
        let names: Vec<&str> = control_order(&list, &draft.config)
            .into_iter()
            .map(|t| match t {
                Target::Control(i) => list[i].control.name.as_str(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            names,
            [
                "ReverseMouse",
                "MouseFilter",
                "Slider",
                "SensitivityLabel",
                "MouseRaw",
                "Autoaim"
            ]
        );
    }

    #[test]
    fn tab_walks_the_tab_strip_page_and_footer_and_keys_drive_each_control() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        assert_eq!(
            h.dialog.focus,
            Some(Target::Tab(2)),
            "a click focuses the tab"
        );
        h.key(Key::Tab);
        assert_eq!(
            focused_name(&h, Panel::Mouse).as_deref(),
            Some("ReverseMouse")
        );
        let invert = h.dialog.draft().mouse.invert;
        h.key(Key::Space);
        assert_ne!(
            h.dialog.draft().mouse.invert,
            invert,
            "Space toggles a check box"
        );
        h.key(Key::Enter);
        assert_eq!(
            h.dialog.draft().mouse.invert,
            invert,
            "so does Enter, not OK"
        );

        h.key(Key::Tab);
        assert_eq!(focused_name(&h, Panel::Mouse).as_deref(), Some("Slider"));
        let sensitivity = h.dialog.draft().mouse.sensitivity;
        h.key(Key::Right);
        assert!(
            h.dialog.draft().mouse.sensitivity > sensitivity,
            "Right raises a slider"
        );
        h.key(Key::PageDown);
        assert!(
            h.dialog.draft().mouse.sensitivity < sensitivity,
            "Page Down lowers a tick"
        );

        h.key(Key::Tab);
        assert_eq!(
            focused_name(&h, Panel::Mouse).as_deref(),
            Some("SensitivityLabel")
        );
        assert!(h.dialog.editing.is_some(), "a focused text entry edits");
        h.frame(Input {
            keys: [Key::Backspace].into(),
            ..Default::default()
        });
        h.key(Key::Tab);
        assert!(h.dialog.editing.is_none(), "Tab commits the entry");
        assert_eq!(focused_name(&h, Panel::Mouse).as_deref(), Some("Autoaim"));
        let aim = h.dialog.draft().mouse.aim_assist;
        h.key(Key::Down);
        assert_ne!(
            h.dialog.draft().mouse.aim_assist,
            aim,
            "Down selects the next item"
        );
        assert!(h.dialog.combo.is_none(), "without opening the list");
        h.frame(Input {
            keys: [Key::Down].into(),
            alt: true,
            ..Default::default()
        });
        assert!(h.dialog.combo.is_some(), "Alt+Down opens it");
        h.key(Key::Enter);
        assert!(h.dialog.combo.is_none());

        h.key(Key::Tab);
        assert_eq!(
            focused_name(&h, Panel::Mouse).as_deref(),
            Some("MouseFilter")
        );
        h.key(Key::Tab);
        assert_eq!(h.dialog.focus, Some(Target::Ok));
        h.key(Key::Tab);
        assert_eq!(h.dialog.focus, Some(Target::Cancel));
        h.key(Key::Tab);
        assert_eq!(
            h.dialog.focus,
            Some(Target::Apply),
            "Apply is a stop while enabled"
        );
        h.key(Key::Tab);
        assert_eq!(
            h.dialog.focus,
            Some(Target::Tab(2)),
            "wraps to the tab strip"
        );
        shift_tab(&mut h);
        assert_eq!(h.dialog.focus, Some(Target::Apply));
        assert!(matches!(h.key(Key::Enter), OptionsOutcome::Apply(_)));
        assert!(!h.dialog.changed());
        h.frame(Input::default());
        assert_eq!(h.dialog.focus, None, "Apply left the order once disabled");
    }

    #[test]
    fn arrows_switch_tabs_and_move_a_radio_group() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        h.key(Key::Left);
        assert_eq!(h.dialog.tab(), Tab::Keyboard);
        assert_eq!(h.dialog.focus, Some(Target::Tab(1)));
        h.key(Key::Left);
        assert_eq!(h.dialog.tab(), Tab::Gameplay);
        h.key(Key::Left);
        assert_eq!(h.dialog.tab(), Tab::Gameplay, "no wrap at the first tab");
        h.dialog.draft.config.gameplay.skill = 2;
        h.key(Key::Tab);
        assert_eq!(
            focused_name(&h, Panel::Gameplay).as_deref(),
            Some("Skill2Radio"),
            "the group's stop is its selected button"
        );
        h.key(Key::Down);
        assert_eq!(h.dialog.draft().gameplay.skill, 3);
        assert_eq!(
            focused_name(&h, Panel::Gameplay).as_deref(),
            Some("Skill3Radio")
        );
        h.key(Key::Right);
        assert_eq!(h.dialog.draft().gameplay.skill, 1, "the group wraps");
        h.key(Key::Up);
        assert_eq!(h.dialog.draft().gameplay.skill, 3);
        h.key(Key::Tab);
        assert_eq!(
            h.dialog.focus,
            Some(Target::Ok),
            "one stop for the whole group"
        );
    }

    #[test]
    fn focus_rings_enclose_check_boxes_and_mark_tabs_and_sliders() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        assert!(
            drawn(&h).iter().all(|q| q.color != h.theme.focus_border),
            "focus taken by a click is not drawn"
        );
        shift_tab(&mut h);
        h.key(Key::Tab);
        let boxes = |h: &Harness| -> Vec<Rect> {
            drawn(h)
                .into_iter()
                .filter(|q| q.color == h.theme.focus_border && q.destination.w == 1.)
                .map(|q| q.destination)
                .collect()
        };
        let tab = h.dialog.tab_rects(&|t| t.chars().count() as f32 * 7.)[2];
        assert!(
            boxes(&h).iter().all(|d| d.y >= DIALOG.y + tab.y),
            "the focused tab has a box"
        );
        assert!(!boxes(&h).is_empty());
        h.key(Key::Tab);
        let res = h.dialog.res.clone();
        let list = layout_widgets(
            &res,
            Panel::Mouse,
            page_rect(),
            &h.dialog.draft,
            &h.dialog.applied,
        );
        let check = list
            .iter()
            .find(|w| w.control.name == "ReverseMouse")
            .unwrap()
            .rect;
        let left = boxes(&h).iter().map(|d| d.x).fold(f32::INFINITY, f32::min);
        let top = boxes(&h).iter().map(|d| d.y).fold(f32::INFINITY, f32::min);
        let square = DIALOG.x + check.x + 2.;
        let square_top = DIALOG.y + check.y + (check.h - widgets::CHECK_SIZE) * 0.5;
        assert!(left < square, "the ring encloses the check box");
        assert_eq!(
            square - left,
            square_top - top,
            "as far beside the box as above it"
        );
        h.key(Key::Tab);
        let slider = list
            .iter()
            .find(|w| w.control.name == "Slider")
            .unwrap()
            .rect;
        assert!(
            boxes(&h)
                .iter()
                .all(|d| d.y >= DIALOG.y + slider.y - widgets::FOCUS_PAD
                    && d.y <= DIALOG.y + slider.y + slider.h + widgets::FOCUS_PAD),
            "the focused slider has a padded ring"
        );
        assert!(!boxes(&h).is_empty());
    }

    #[test]
    fn enter_acts_only_on_the_focused_control_and_the_key_list_takes_focus_back() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        assert_eq!(
            h.key(Key::Enter),
            OptionsOutcome::None,
            "no default button on a tab"
        );
        assert_eq!(h.dialog.tab(), Tab::Mouse);
        h.tab(Tab::Keyboard);
        let list_target = h.dialog.key_list_target();
        h.key(Key::Down);
        assert_eq!(
            h.dialog.keys.selected, None,
            "arrows stay with the focused tab"
        );
        h.click(page(30., 10. + KEY_HEADER + MENU_ROW * 1.5));
        assert_eq!(h.dialog.focus, list_target, "a row click focuses the list");
        h.key(Key::Enter);
        assert_eq!(h.dialog.keys.capture, Some(1));
        h.frame(Input {
            binding_keys: vec!["KeyQ".into()],
            ..Default::default()
        });
        assert_eq!(
            h.dialog.focus, list_target,
            "capture hands the keyboard back to the list"
        );
        h.key(Key::Down);
        assert_eq!(h.dialog.keys.selected, Some(2));
        // Without focus the selected row takes the dim out-of-focus selection.
        let fills = |h: &Harness| -> usize {
            drawn(h)
                .iter()
                .filter(|q| q.color == h.theme.selection_bg)
                .count()
        };
        assert_eq!(fills(&h), 1);
        h.key(Key::Tab);
        assert_eq!(fills(&h), 0);
    }

    #[test]
    fn the_report_names_the_tab_focus_and_pending_changes() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        let report = h.dialog.report();
        assert_eq!(
            (
                report.tab.as_str(),
                report.focus.as_deref(),
                report.focus_visible
            ),
            ("Mouse", Some("tab"), false),
            "a click focuses without showing it"
        );
        h.key(Key::Tab);
        h.key(Key::Space);
        let report = h.dialog.report();
        assert_eq!(report.focus.as_deref(), Some("ReverseMouse"));
        assert!(report.focus_visible && report.changed && report.sub.is_none());
    }

    #[test]
    fn a_held_space_presses_the_button_and_the_release_activates_it() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        shift_tab(&mut h);
        assert_eq!(
            h.dialog.focus,
            Some(Target::Cancel),
            "Apply is disabled: not a stop"
        );
        let down = Input {
            keys: [Key::Space].into(),
            held_keys: [Key::Space].into(),
            ..Default::default()
        };
        assert_eq!(h.frame(down), OptionsOutcome::None);
        assert_eq!(
            h.dialog.key_press.pressed(),
            Some(Target::Cancel),
            "drawn depressed"
        );
        let hold = Input {
            held_keys: [Key::Space].into(),
            ..Default::default()
        };
        assert_eq!(h.frame(hold), OptionsOutcome::None);
        assert_eq!(
            h.frame(Input::default()),
            OptionsOutcome::Close(None),
            "the release activates Cancel"
        );
    }

    #[test]
    fn credits_close_takes_focus_and_enter_closes_on_release() {
        let mut h = Harness::new();
        h.dialog.command(Command::AudioCredits);
        assert_eq!(
            h.dialog.focus,
            Some(Target::Cancel),
            "Close holds the focus"
        );
        assert!(!h.dialog.focus_visible, "drawn only after Tab");
        h.key(Key::Tab);
        assert_eq!(h.dialog.focus, Some(Target::Cancel));
        assert!(h.dialog.focus_visible, "Tab shows the focus");
        assert!(
            drawn(&h).iter().any(|q| q.color == h.theme.focus_border),
            "the focus box is drawn"
        );
        let down = Input {
            keys: [Key::Enter].into(),
            held_keys: [Key::Enter].into(),
            ..Default::default()
        };
        h.frame(down);
        assert_eq!(
            h.dialog.sub,
            Some(Sub::AudioCredits),
            "a press does not close"
        );
        assert_eq!(h.dialog.key_press.pressed(), Some(Target::Cancel));
        h.frame(Input::default());
        assert_eq!(h.dialog.sub, None, "the release closes the credits only");
    }

    #[test]
    fn a_sub_dialog_takes_focus_and_gives_it_back() {
        let mut h = Harness::new();
        h.tab(Tab::Video);
        while focused_name(&h, Panel::Video).as_deref() != Some("AdvancedButton") {
            h.key(Key::Tab);
        }
        h.key(Key::Space);
        assert_eq!(h.dialog.sub, Some(Sub::VideoAdvanced));
        assert_eq!(h.dialog.focus, None, "the sub-dialog starts unfocused");
        h.key(Key::Tab);
        assert!(matches!(h.dialog.focus, Some(Target::Control(_))));
        assert!(h.dialog.escape());
        assert_eq!(h.dialog.sub, None);
        assert_eq!(
            focused_name(&h, Panel::Video).as_deref(),
            Some("AdvancedButton")
        );
        assert_eq!(
            h.key(Key::Enter),
            OptionsOutcome::None,
            "Enter presses the button"
        );
        assert_eq!(h.dialog.sub, Some(Sub::VideoAdvanced));
    }

    #[test]
    fn geometry_follows_property_dialog_rules() {
        assert_eq!(page_rect(), Rect::new(8., 58., 496., 308.));
        let [ok, cancel, apply] = button_rects();
        assert_eq!((ok.x, cancel.x, apply.x, ok.y), (264., 344., 424., 370.));
    }

    #[test]
    fn gamepad_page_edits_its_own_settings() {
        let mut h = Harness::new();
        h.tab(Tab::Gamepad);
        assert!(h.dialog.take_log().is_empty(), "no longer a placeholder");
        let enabled = |h: &Harness, name: &str| {
            layout_widgets(
                &h.dialog.res,
                Panel::Gamepad,
                page_rect(),
                &h.dialog.draft,
                &Config::default(),
            )
            .iter()
            .find(|w| w.control.name == name)
            .unwrap()
            .enabled
        };
        assert!(!enabled(&h, "AutoaimScale"), "aim assist is off");
        assert!(
            !enabled(&h, "AutoaimScaleLabel"),
            "the label follows its associate"
        );
        h.click(page(38. + 80., 116. + 12.));
        h.click(page(38. + 80., 140. + MENU_ROW * 2.5));
        assert_eq!(h.dialog.draft().gamepad.aim_assist, AimAssist::Enhanced);
        assert_eq!(
            h.dialog.draft().mouse.aim_assist,
            AimAssist::Off,
            "not the mouse setting"
        );
        assert!(enabled(&h, "AutoaimScale") && enabled(&h, "AutoaimScaleLabel"));
        h.click(page(38. + 1., 43. + SLIDER_TRACK_Y));
        assert_eq!(h.dialog.draft().gamepad.sensitivity_scale, 0.5);
        h.click(page(38. + 80., 262. + 12.));
        h.click(page(38. + 80., 286. + MENU_ROW * 1.5));
        assert_eq!(
            h.dialog.draft().gamepad.weapon_selection,
            crate::config::WeaponSelectionStyle::Carousel
        );
        assert!(!enabled(&h, "SteamInput"));
        h.click(page(240. + 100., 262. + 12.));
        assert_eq!(
            h.dialog.take_log(),
            ["Open Steam Input Configuration: not implemented in HL2-RS yet."]
        );
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
        // Track spans x 204..388 of the page; the middle is half of 0.1..6.
        let start = page(296., 50.);
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

    /// Quads of the dialog drawn on its own (no fonts, so no glyph quads).
    fn drawn(h: &Harness) -> Vec<crate::canvas::Quad> {
        let painter = Painter {
            canvas: &h.canvas,
            fonts: &h.fonts,
            theme: &h.theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };
        h.dialog.draw(&painter);
        h.canvas.drain()
    }

    #[test]
    fn key_list_separates_groups_and_capture_highlights_only_the_key_cell() {
        let mut h = Harness::new();
        h.tab(Tab::Keyboard);
        // The clicked tab's dashed focus box shares the divider's color and height.
        h.dialog.focus = None;
        let theme = h.theme.clone();
        // Group separator under the section header row (row 0): one pixel high.
        let quads = drawn(&h);
        let dividers: Vec<_> = quads
            .iter()
            .filter(|q| q.color == theme.list_divider && q.destination.h == 1.)
            .collect();
        assert_eq!(dividers.len(), 1, "one section, one separator");
        let list = page(8., 10.);
        assert_eq!(
            dividers[0].destination.y,
            list.y + KEY_HEADER + MENU_ROW - 2.
        );

        // Selected row: the whole row is orange.
        h.click(page(30., 10. + KEY_HEADER + MENU_ROW * 1.5));
        let orange = |quads: &[crate::canvas::Quad]| {
            quads
                .iter()
                .filter(|q| q.color == theme.selection_bg)
                .map(|q| q.destination)
                .collect::<Vec<_>>()
        };
        let row = orange(&drawn(&h));
        assert_eq!(row.len(), 1);
        // Capturing: the key cell is orange and the action cell keeps a dimmer selection.
        h.click(page(280., 280.));
        assert_eq!(h.dialog.keys.capture, Some(1));
        let quads = drawn(&h);
        let dim: Vec<_> = quads
            .iter()
            .filter(|q| {
                q.color
                    == crate::canvas::Color {
                        a: theme.selection_bg_dim.a * CAPTURE_ACTION_FADE,
                        ..theme.selection_bg_dim
                    }
            })
            .map(|q| q.destination)
            .collect();
        let cell = orange(&quads);
        assert_eq!(dim.len(), 1);
        assert_eq!((dim[0].x, dim[0].y), (row[0].x, row[0].y));
        assert_eq!(
            dim[0].x + dim[0].w,
            cell[0].x,
            "action cell meets the key cell"
        );
        assert_eq!(cell.len(), 1);
        assert_eq!(cell[0].y, row[0].y);
        assert!(cell[0].x > row[0].x && cell[0].w < row[0].w);
        assert_eq!(
            cell[0].x + cell[0].w,
            row[0].x + row[0].w,
            "ends at the row edge"
        );
    }

    #[test]
    fn external_display_mode_changes_reach_an_open_dialog() {
        use crate::config::DisplayMode;
        let mut h = Harness::new();
        h.dialog.open(Config {
            video: crate::config::VideoSettings {
                display_mode: DisplayMode::Windowed,
                ..Default::default()
            },
            ..Config::default()
        });
        h.dialog.sync_display_mode(DisplayMode::Fullscreen);
        assert_eq!(h.dialog.draft().video.display_mode, DisplayMode::Fullscreen);
        assert!(!h.dialog.changed(), "nothing left to apply");
        // A mode the user already chose in the draft is kept.
        h.dialog.draft.config.video.display_mode = DisplayMode::Windowed;
        h.dialog.sync_display_mode(DisplayMode::Fullscreen);
        assert_eq!(h.dialog.draft().video.display_mode, DisplayMode::Windowed);
        assert!(h.dialog.changed());
    }

    #[test]
    fn key_list_scroll_bar_scrolls_while_held_and_blocks_row_clicks() {
        let mut h = Harness::new();
        let mut res = (*resources()).clone();
        res.key_rows = (0..40)
            .map(|i| KeyRow::Action {
                command: format!("+cmd{i}"),
                label: "#Valve_Jump".into(),
            })
            .collect();
        h.dialog.set_resources(Arc::new(res));
        h.dialog.open(Config::default());
        h.tab(Tab::Keyboard);
        let list = Rect::new(8., 10., 480., 258.);
        let bar = key_bar_rect(list);
        let layout = widgets::ScrollBarLayout::new(bar, 0, key_rows_visible(list), 40);
        let at = |p: Vec2| page(p.x, p.y);
        let nob = vec2(layout.nob.x + 5., layout.nob.y + 5.);
        // Press on the nob and drag: the list scrolls before the release.
        h.frame(Input {
            pointer: Some(at(nob)),
            click: true,
            held: true,
            ..Default::default()
        });
        h.frame(Input {
            pointer: Some(at(nob + vec2(0., 60.))),
            held: true,
            ..Default::default()
        });
        let dragged = h.dialog.keys.first;
        assert!(dragged > 0, "live drag");
        h.frame(Input {
            pointer: Some(at(nob + vec2(0., 60.))),
            released: true,
            ..Default::default()
        });
        assert_eq!(h.dialog.keys.first, dragged);
        assert_eq!(h.dialog.keys.selected, None, "the bar never selects a row");
        // The down arrow steps on press.
        let down = vec2(layout.down.x + 5., layout.down.y + 5.);
        h.click(at(down));
        assert_eq!(h.dialog.keys.first, dragged + 1);
    }

    #[test]
    fn double_click_on_a_key_row_starts_capture() {
        let mut h = Harness::new();
        h.tab(Tab::Keyboard);
        let row = page(30., 10. + KEY_HEADER + MENU_ROW * 1.5);
        let click_at = |h: &mut Harness, time: f64| {
            for (click, held, released) in [(true, true, false), (false, false, true)] {
                h.frame(Input {
                    pointer: Some(row),
                    click,
                    held,
                    released,
                    time: Some(time),
                    ..Default::default()
                });
            }
        };
        click_at(&mut h, 1.);
        assert_eq!(
            (h.dialog.keys.selected, h.dialog.keys.capture),
            (Some(1), None)
        );
        click_at(&mut h, 1.8);
        assert_eq!(
            h.dialog.keys.capture, None,
            "slow second click only selects"
        );
        click_at(&mut h, 2.1);
        assert_eq!(h.dialog.keys.capture, Some(1), "double click edits the key");
        h.frame(Input {
            binding_keys: vec!["MOUSE2".into()],
            ..Default::default()
        });
        assert_eq!(h.dialog.draft().keys.get("+forward").unwrap(), "MOUSE2");
        // Without frame times (scripted input) repeated clicks never start capture.
        h.click(row);
        h.click(row);
        assert_eq!(h.dialog.keys.capture, None);
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

    impl Harness {
        /// Press at `from`, move with the button held to `to`, release there.
        fn drag(&mut self, from: Vec2, to: Vec2) {
            for (pointer, click, held, released) in [
                (from, true, true, false),
                (to, false, true, false),
                (to, false, false, true),
            ] {
                self.frame(Input {
                    pointer: Some(pointer),
                    click,
                    held,
                    released,
                    ..Default::default()
                });
            }
        }
    }

    #[test]
    fn the_caption_drags_the_dialog_and_its_controls_follow() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        h.dialog.draft.config.mouse.invert = true;
        // Caption: the top 23 units. Dragging it 100 left and 50 down moves OK with it.
        h.drag(DIALOG + vec2(100., 10.), DIALOG + vec2(0., 60.));
        let moved = DIALOG + vec2(-100., 50.);
        let [ok, _, _] = button_rects();
        let old_ok = DIALOG + vec2(ok.x + 10., ok.y + 10.);
        assert_eq!(
            h.click(old_ok),
            OptionsOutcome::None,
            "nothing at the old spot"
        );
        let outcome = h.click(moved + vec2(ok.x + 10., ok.y + 10.));
        assert!(
            matches!(outcome, OptionsOutcome::Close(Some(_))),
            "{outcome:?}"
        );
        // The placement stays for the next opening.
        h.dialog.open(Config::default());
        assert_eq!(
            h.dialog
                .placement
                .place(&main_frame(vec2(1280., 720.)))
                .origin,
            moved
        );
    }

    #[test]
    fn escape_undoes_a_drag_and_sub_dialogs_move_alone() {
        let mut h = Harness::new();
        h.frame(Input {
            pointer: Some(DIALOG + vec2(100., 10.)),
            click: true,
            held: true,
            ..Default::default()
        });
        h.frame(Input {
            pointer: Some(DIALOG + vec2(300., 200.)),
            held: true,
            ..Default::default()
        });
        assert!(h.dialog.escape(), "Escape cancels the drag, not the dialog");
        let canvas = vec2(1280., 720.);
        assert_eq!(h.dialog.placement.place(&main_frame(canvas)).origin, DIALOG);
        // The brightness dialog (290 x 396, at 495, 162) moves without its parent.
        h.tab(Tab::Video);
        h.click(page(300., 125.));
        let gamma = vec2(495., 162.);
        h.drag(gamma + vec2(50., 10.), gamma + vec2(-150., 10.));
        let size = h.dialog.sub_frame_size(Sub::Gamma, |_| 0.);
        let placed = h
            .dialog
            .sub_placement
            .place(&sub_frame(canvas, size))
            .origin;
        assert_eq!(placed, gamma + vec2(-200., 0.));
        assert_eq!(h.dialog.placement.place(&main_frame(canvas)).origin, DIALOG);
        h.dialog.escape();
        h.click(page(300., 125.));
        let placed = h
            .dialog
            .sub_placement
            .place(&sub_frame(canvas, size))
            .origin;
        assert_eq!(placed, gamma, "a sub-dialog opens centered again");
    }

    #[test]
    fn a_caption_press_with_a_list_open_only_closes_the_list() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        h.click(page(40., 180.));
        assert!(h.dialog.combo.is_some());
        h.drag(DIALOG + vec2(100., 10.), DIALOG + vec2(0., 60.));
        assert!(h.dialog.combo.is_none());
        assert_eq!(
            h.dialog
                .placement
                .place(&main_frame(vec2(1280., 720.)))
                .origin,
            DIALOG,
            "the press closed the list and did not drag"
        );
    }

    #[test]
    fn video_advanced_opens_and_gamma_dialog_restores_on_cancel() {
        let mut h = Harness::new();
        h.tab(Tab::Video);
        h.click(page(300., 200.));
        assert_eq!(h.dialog.sub, Some(Sub::VideoAdvanced));
        assert!(h.dialog.take_log().is_empty(), "no longer a placeholder");
        assert!(h.dialog.escape(), "Escape closes the sub-dialog first");
        assert_eq!(h.dialog.sub, None);
        h.click(page(300., 125.));
        assert_eq!(h.dialog.sub, Some(Sub::Gamma));
        // Gamma frame 290x396 centered: origin (495, 162); slider track at y 122.
        let gamma = vec2(495., 162.);
        h.click(gamma + vec2(17. + 4., 122.));
        assert_eq!(h.dialog.draft().video.gamma, 1.6);
        assert_eq!(h.dialog.preview_gamma(), Some(1.6), "previewed live");
        h.click(gamma + vec2(200., 360.));
        assert!(h.dialog.sub.is_none());
        assert_eq!(h.dialog.preview_gamma(), None, "no preview once closed");
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
        // Unlocked in every display mode (retail: full screen only).
        h.dialog.open(Config {
            video: crate::config::VideoSettings {
                display_mode: crate::config::DisplayMode::Windowed,
                ..Default::default()
            },
            ..Config::default()
        });
        h.tab(Tab::Video);
        h.click(page(300., 125.));
        assert!(h.dialog.sub == Some(Sub::Gamma), "opens in windowed mode");
    }

    #[test]
    fn keyboard_advanced_cancel_restores_and_ok_applies_only_its_settings() {
        let mut h = Harness::new();
        h.tab(Tab::Keyboard);
        h.dialog.draft.config.mouse.invert = true;
        h.click(page(148. + 50., 276. + 12.));
        assert_eq!(h.dialog.sub, Some(Sub::KeyboardAdvanced));
        // No frame record: the controls span x 20..264, y 38..124, so the frame is 284 x 144
        // (left margin repeated), centered at (498, 288).
        let size = h.dialog.sub_frame_size(Sub::KeyboardAdvanced, |_| 0.);
        assert_eq!(size, vec2(284., 144.));
        let origin = vec2(498., 288.);
        h.click(origin + vec2(30., 50.));
        assert!(h.dialog.draft().keyboard.fast_switch);
        h.click(origin + vec2(182. + 40., 112.));
        assert_eq!(h.dialog.sub, None);
        assert!(!h.dialog.draft().keyboard.fast_switch, "Cancel restores");
        assert!(h.dialog.draft().mouse.invert, "earlier page edits stay");

        h.click(page(148. + 50., 276. + 12.));
        h.click(origin + vec2(30., 78.));
        let outcome = h.click(origin + vec2(95. + 40., 112.));
        let OptionsOutcome::Apply(applied) = outcome else {
            panic!("advanced OK applies at once: {outcome:?}");
        };
        assert!(!applied.keyboard.developer_console);
        assert!(
            !applied.mouse.invert,
            "other unapplied edits are not applied"
        );
        assert!(h.dialog.draft().mouse.invert, "and stay in the draft");
        assert!(h.dialog.changed(), "so Apply stays enabled for them");
    }

    #[test]
    fn footer_moves_below_content_it_overlaps_and_the_frame_grows() {
        // The installed macOS rows: the caption at 310 and OK/Cancel at 316 overlap the
        // Classic Effects combo (316..340).
        let overlapping = Layout::parse(
            br##""resource/OptionsSubVideoAdvancedDlg.res" {
            "ClassicEffectsLabel" { "ControlName" "Label" "fieldName" "ClassicEffectsLabel"
                "xpos" "22" "ypos" "292" "wide" "138" "tall" "24" "associate" "ClassicEffects" }
            "ClassicEffects" { "ControlName" "ComboBox" "fieldName" "ClassicEffects" "xpos" "22"
                "ypos" "316" "wide" "132" "tall" "24" }
            "Label11" { "ControlName" "Label" "fieldName" "Label11" "xpos" "18" "ypos" "310"
                "wide" "280" "tall" "24" "labelText" "#GameUI_RecommendedSettings" }
            "Button1" { "ControlName" "Button" "fieldName" "Button1" "xpos" "290" "ypos" "316"
                "wide" "80" "tall" "24" "Command" "OK" }
            "Button2" { "ControlName" "Button" "fieldName" "Button2" "xpos" "382" "ypos" "316"
                "wide" "80" "tall" "24" "Command" "Close" }
        }"##,
        )
        .unwrap();
        let controls: Vec<&Control> = overlapping.controls.iter().collect();
        let (shift, footer) = footer_reflow(Panel::VideoAdvanced, &controls, (482., 358.));
        assert_eq!(shift, 340. + FOOTER_GAP - 310.);
        assert_eq!(
            footer,
            [false, false, true, true, true],
            "caption and buttons"
        );
        // The Windows rows (caption 442, buttons 468) already clear the content.
        let mut clear = overlapping.clone();
        for (name, y) in [("Label11", 442.), ("Button1", 468.), ("Button2", 468.)] {
            clear.get_mut(name).unwrap().y = crate::gameui::layout::Coord::Start(y);
        }
        let controls: Vec<&Control> = clear.controls.iter().collect();
        assert_eq!(
            footer_reflow(Panel::VideoAdvanced, &controls, (482., 510.)).0,
            0.
        );
    }

    #[test]
    fn the_fov_slider_ring_stays_clear_of_the_value_label_text() {
        let layout = Layout::parse(VIDEO_ADVANCED.as_bytes()).unwrap();
        let rect_of = |name: &str| {
            let c = layout.controls.iter().find(|c| c.name == name).unwrap();
            Rect::new(c.x.resolve(482.), c.y.resolve(358.), c.wide, c.tall)
        };
        let label = rect_of("FOVValueLabel");
        let text_bottom = label.y + (label.h - widgets::FONT_TALL) * 0.5 + widgets::FONT_TALL;
        let ring = widgets::slider_content(rect_of("FovSlider")).pad(widgets::FOCUS_PAD);
        assert!(ring.y >= text_bottom, "{ring:?} vs {text_bottom}");
    }

    #[test]
    fn video_advanced_combo_and_fov_slider_apply_on_ok() {
        let mut h = Harness::new();
        h.tab(Tab::Video);
        h.click(page(300., 200.));
        assert_eq!(h.dialog.sub, Some(Sub::VideoAdvanced));
        // Frame record 482 x 358, centered at (399, 181).
        let origin = vec2(399., 181.);
        h.click(origin + vec2(330. + 66., 196. + 12.));
        assert!(
            h.dialog.combo.is_some(),
            "the combo opens inside the sub-dialog"
        );
        h.click(origin + vec2(330. + 66., 220. + MENU_ROW * 0.5));
        assert!(h.dialog.combo.is_none());
        assert!(!h.dialog.draft().video.advanced.vsync, "Disabled picked");
        // The FOV slider spans the layout's 75..110, not a page constant.
        h.click(origin + vec2(176. + 139., 252. + SLIDER_TRACK_Y));
        assert_eq!(h.dialog.draft().video.fov, 110.);
        let res = h.dialog.res.clone();
        let area = Rect::new(0., 0., 482., 358.);
        let widgets = layout_widgets(
            &res,
            Panel::VideoAdvanced,
            area,
            &h.dialog.draft,
            &Config::default(),
        );
        let label = widgets
            .iter()
            .find(|w| w.control.name == "FOVValueLabel")
            .unwrap();
        assert!(!label.interactive(), "the value label is display only");
        assert!(widgets.iter().all(|w| w.control.name != "frame_close"));
        let outcome = h.click(origin + vec2(290. + 40., 316. + 12.));
        let OptionsOutcome::Apply(applied) = outcome else {
            panic!("advanced OK applies at once: {outcome:?}");
        };
        assert!(!applied.video.advanced.vsync);
        assert_eq!(applied.video.fov, 110.);
    }

    #[test]
    fn the_vertical_and_viewmodel_fov_sliders_are_added_and_apply_on_ok() {
        let mut res = (*resources()).clone();
        crate::gameui::resources::apply_layout_addenda(&mut res.layouts);
        let draft = Draft::new(Config::default());
        let widgets = layout_widgets(
            &res,
            Panel::VideoAdvanced,
            Rect::new(0., 0., 482., 510.),
            &draft,
            &draft.config,
        );
        let find = |name: &str| {
            widgets
                .iter()
                .find(|w| w.control.name == name)
                .unwrap_or_else(|| panic!("{name}"))
        };
        for (slider, label, value) in [
            (
                "VerticalFovSlider",
                "VerticalFovValueLabel",
                Value::VerticalFov,
            ),
            (
                "ViewmodelFovSlider",
                "ViewmodelFovValueLabel",
                Value::ViewmodelFov,
            ),
        ] {
            assert_eq!(find(slider).binding, Some(Binding::Slider(value)));
            assert!(find(slider).interactive());
            assert_eq!(find(label).binding, Some(Binding::ValueLabel(value)));
            assert!(
                !find(label).interactive(),
                "the value label is display only"
            );
        }
        // They sit under the Field of view slider and clear of the divider at 352.
        let fov = find("FovSlider").control.rect((482., 510.));
        let vertical = find("VerticalFovSlider").control.rect((482., 510.));
        assert!(vertical.1 >= fov.1 + find("FovSlider").control.tall);
        assert!(vertical.1 + find("VerticalFovSlider").control.tall <= 352.);
        // Their labels are content, not footer captions: the footer reflow must not move them.
        let controls: Vec<&Control> = res.layouts["optionssubvideoadvanceddlg"]
            .controls
            .iter()
            .collect();
        let (shift, footer) = footer_reflow(Panel::VideoAdvanced, &controls, (482., 358.));
        for (control, is_footer) in controls.iter().zip(&footer) {
            if control.name.to_ascii_lowercase().contains("fov") {
                assert!(!is_footer, "{} is not footer", control.name);
            }
        }
        assert!(
            shift > 0.,
            "the dialog grows to hold the new sliders on the short layout"
        );
        let mut from = Config::default();
        from.video.vertical_fov = 90.;
        from.video.viewmodel_fov = 70.;
        let mut to = Config::default();
        assert!(Sub::VideoAdvanced.apply_on_ok(&from, &mut to));
        assert_eq!((to.video.vertical_fov, to.video.viewmodel_fov), (90., 70.));
    }

    #[test]
    #[ignore = "requires an owned Half-Life 2 installation"]
    fn owned_install_pages_tab_in_reading_order() {
        let root = source_assets::install::discover().expect("installed game");
        let vfs = source_assets::vpk::Vfs::mount(&root).unwrap();
        let res = GameUiResources::load(&vfs);
        let draft = Draft::new(Config::default());
        let order = |panel: Panel| -> Vec<String> {
            let list = layout_widgets(&res, panel, page_rect(), &draft, &draft.config);
            control_order(&list, &draft.config)
                .into_iter()
                .filter_map(|t| match t {
                    Target::Control(i) => Some(list[i].control.name.clone()),
                    _ => None,
                })
                .collect()
        };
        // Left to right, then top to bottom, as the installed pages read.
        assert_eq!(
            order(Panel::Mouse),
            [
                "ReverseMouse",
                "MouseFilter",
                "Slider",
                "SensitivityLabel",
                "Autoaim"
            ]
        );
        assert_eq!(
            order(Panel::Audio),
            [
                "SFXSlider",
                "MusicSlider",
                "SpeakerSetup",
                "SoundQuality",
                "AudioSpokenLanguage",
                "CloseCaptionCheck",
                "snd_mute_losefocus",
                "ThirdPartySoundCredits"
            ]
        );
        assert_eq!(
            order(Panel::Video),
            [
                "Resolution",
                "DisplayModeCombo",
                "AspectRatio",
                "GammaButton",
                "HudAspectRatio",
                "AdvancedButton",
                "HDR",
                "ThirdPartyVideoCredits"
            ]
        );
        let gamepad = order(Panel::Gamepad);
        eprintln!("gamepad {gamepad:?}");
        let read: Vec<(f32, f32)> = {
            let list = layout_widgets(&res, Panel::Gamepad, page_rect(), &draft, &draft.config);
            gamepad
                .iter()
                .map(|n| {
                    let r = list.iter().find(|w| &w.control.name == n).unwrap().rect;
                    (r.y, r.x)
                })
                .collect()
        };
        assert!(
            read.windows(2)
                .all(|w| w[1].0 > w[0].0 + FOCUS_ROW_TOLERANCE
                    || (w[1].0 - w[0].0).abs() <= FOCUS_ROW_TOLERANCE && w[1].1 > w[0].1),
            "Gamepad reads in rows: {gamepad:?}"
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
            (
                Panel::KeyboardAdvanced,
                &["FastSwitchCheck", "ConsoleCheck"],
            ),
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
        // Sub-dialog OK/Cancel buttons bind through their layout `Command`, frame chrome is
        // skipped, and every interactive control lies below the title bar inside the frame
        // (label boxes only start there: the installed `Label10` box overhangs the frame by 2,
        // and the frame clips it).
        let subs: &[Sub] = &[Sub::Gamma, Sub::KeyboardAdvanced, Sub::VideoAdvanced];
        for &sub in subs {
            let panel = sub.panel();
            let size = h.dialog.sub_frame_size(sub, |_| 0.);
            let area = Rect::new(0., 0., size.x, size.y);
            let widgets = layout_widgets(&h.dialog.res, panel, area, &draft, &Config::default());
            for w in &widgets {
                let r = w.rect;
                let inside_end = !w.interactive() || (r.x + r.w <= size.x && r.y + r.h <= size.y);
                assert!(
                    r.x >= 0. && r.y >= CLIENT_TOP && r.x < size.x && r.y < size.y && inside_end,
                    "{panel:?} {} {r:?} outside {size:?}",
                    w.control.name
                );
            }
            let overlaps = |a: Rect, b: Rect| {
                a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
            };
            let interactive: Vec<&Widget> = widgets.iter().filter(|w| w.interactive()).collect();
            for (i, a) in interactive.iter().enumerate() {
                for b in &interactive[i + 1..] {
                    assert!(
                        !overlaps(a.rect, b.rect),
                        "{panel:?}: {} overlaps {}",
                        a.control.name,
                        b.control.name
                    );
                }
            }
            let commands: Vec<_> = widgets
                .iter()
                .filter_map(|w| match w.binding {
                    Some(Binding::Command(c @ (Command::SubOk | Command::SubCancel))) => Some(c),
                    _ => None,
                })
                .collect();
            assert_eq!(commands, [Command::SubOk, Command::SubCancel], "{panel:?}");
            assert!(widgets.iter().all(|w| !is_frame_chrome(w.control)));
        }
        h.dialog.command(Command::KeyAdvanced);
        h.frame(Input::default());
        assert_eq!(h.dialog.sub, Some(Sub::KeyboardAdvanced));
    }

    impl Harness {
        /// A press, one held frame and a release at `at`, like a real click spanning frames.
        fn slow_click(&mut self, at: Vec2) -> OptionsOutcome {
            let mut outcome = OptionsOutcome::None;
            for (click, held, released) in [
                (true, true, false),
                (false, true, false),
                (false, false, true),
            ] {
                let frame = self.frame(Input {
                    pointer: Some(at),
                    click,
                    held,
                    released,
                    ..Default::default()
                });
                if !matches!(frame, OptionsOutcome::None) {
                    outcome = frame;
                }
            }
            outcome
        }

        fn hover(&mut self, at: Vec2) {
            self.frame(Input {
                pointer: Some(at),
                ..Default::default()
            });
        }
    }

    /// Mouse page: the Autoaim list (rows from y 196: Off, Basic, Enhanced) lies over the
    /// MouseFilter check box (y 216..244).
    const AUTOAIM: (f32, f32) = (40., 180.);
    const BASIC_OVER_FILTER: (f32, f32) = (40., 196. + MENU_ROW * 1.5);
    const REVERSE_MOUSE: (f32, f32) = (30., 20.);

    #[test]
    fn open_dropdown_list_gates_input_to_elements_behind_it() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        let behind = page(BASIC_OVER_FILTER.0, BASIC_OVER_FILTER.1);
        let outside = page(REVERSE_MOUSE.0, REVERSE_MOUSE.1);

        // List closed: the check box under the list area takes hover, tooltip and clicks.
        h.hover(behind);
        assert!(matches!(h.dialog.armed, Some(Target::Control(_))));
        assert!(h.dialog.active_tooltip().is_some());
        h.slow_click(behind);
        assert!(
            h.dialog.draft().mouse.filter,
            "closed list: click reaches the box"
        );
        h.slow_click(behind);
        assert!(!h.dialog.draft().mouse.filter);
        let _ = h.dialog.take_sounds();

        // List open: hovering an entry over the box, or anything outside the list, arms
        // nothing, shows no tooltip and plays nothing.
        h.slow_click(page(AUTOAIM.0, AUTOAIM.1));
        assert!(h.dialog.combo.is_some());
        let _ = h.dialog.take_sounds();
        for at in [behind, outside] {
            h.hover(at);
            assert_eq!(h.dialog.armed, None);
            assert_eq!(h.dialog.pressed, None);
            assert_eq!(h.dialog.active_tooltip(), None);
            assert!(h.dialog.take_sounds().is_empty());
        }

        // Clicking the entry selects it and the click is consumed: the box behind it does
        // not toggle on the press, held or release frames.
        h.slow_click(behind);
        assert!(h.dialog.combo.is_none());
        assert_eq!(h.dialog.draft().mouse.aim_assist, AimAssist::Basic);
        assert!(
            !h.dialog.draft().mouse.filter,
            "selection click must not reach the box"
        );
        assert_eq!(h.dialog.take_sounds(), vec!["ui/buttonclickrelease.wav"]);

        // Clicking outside closes the list without activating what was clicked.
        h.slow_click(page(AUTOAIM.0, AUTOAIM.1));
        assert!(h.dialog.combo.is_some());
        h.slow_click(outside);
        assert!(h.dialog.combo.is_none(), "outside click closes the list");
        assert!(
            !h.dialog.draft().mouse.invert,
            "outside click activates nothing"
        );
        assert_eq!(h.dialog.draft().mouse.aim_assist, AimAssist::Basic);

        // Clicking outside with the list closed works as usual.
        h.slow_click(outside);
        assert!(h.dialog.draft().mouse.invert);
    }

    #[test]
    fn dropdown_open_state_toggles_on_header_and_closes_in_the_release_phase() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);
        let header = page(AUTOAIM.0, AUTOAIM.1);
        let initial = h.dialog.draft().mouse.aim_assist;
        let press = |h: &mut Harness, at: Vec2, click: bool| {
            h.frame(Input {
                pointer: Some(at),
                click,
                held: true,
                ..Default::default()
            });
        };
        let release = |h: &mut Harness, at: Vec2| {
            h.frame(Input {
                pointer: Some(at),
                released: true,
                ..Default::default()
            });
        };

        // Closed -> open on a click, opening on release over the header.
        press(&mut h, header, true);
        assert!(h.dialog.combo.is_none(), "opens on release, not press");
        release(&mut h, header);
        assert!(h.dialog.combo.is_some());

        // Open -> closed on a header click: still open on press and hold, closed on release,
        // and it stays closed (one full press-release never closes and reopens).
        press(&mut h, header, true);
        press(&mut h, header, false);
        assert!(h.dialog.combo.is_some(), "header press does not close yet");
        release(&mut h, header);
        assert!(h.dialog.combo.is_none(), "header release closes the list");
        h.hover(header);
        assert!(
            h.dialog.combo.is_none(),
            "the same click does not reopen it"
        );
        assert_eq!(
            h.dialog.draft().mouse.aim_assist,
            initial,
            "no selection change"
        );
        // A quick two-frame header click toggles the same way.
        h.click(header);
        assert!(h.dialog.combo.is_some());
        h.click(header);
        assert!(h.dialog.combo.is_none());
        assert_eq!(h.dialog.draft().mouse.aim_assist, initial);

        // Open -> closed on entry selection, in the release phase.
        h.slow_click(header);
        let entry = page(BASIC_OVER_FILTER.0, BASIC_OVER_FILTER.1);
        press(&mut h, entry, true);
        assert!(h.dialog.combo.is_some(), "entry press does not select yet");
        release(&mut h, entry);
        assert!(h.dialog.combo.is_none());
        assert_eq!(h.dialog.draft().mouse.aim_assist, AimAssist::Basic);

        // Open -> closed on an outside click, in the release phase.
        h.slow_click(header);
        let outside = page(REVERSE_MOUSE.0, REVERSE_MOUSE.1);
        press(&mut h, outside, true);
        assert!(h.dialog.combo.is_some(), "outside press does not close yet");
        release(&mut h, outside);
        assert!(h.dialog.combo.is_none());
        assert!(!h.dialog.draft().mouse.invert);

        // Pressing the header and releasing elsewhere leaves the list open.
        h.slow_click(header);
        press(&mut h, header, true);
        release(&mut h, outside);
        assert!(h.dialog.combo.is_some());
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
    fn dialogs_over_the_options_dialog_are_opaque() {
        let mut h = Harness::new();
        // The frame fill of the topmost layer: the last untextured quad as wide as its frame.
        let frame_alpha = |h: &mut Harness, width: &dyn Fn(&OptionsDialog, &Painter) -> f32| {
            let painter = Painter {
                canvas: &h.canvas,
                fonts: &h.fonts,
                theme: &h.theme,
                origin: Vec2::ZERO,
                scale: 1.,
            };
            let width = width(&h.dialog, &painter);
            h.dialog.draw(&painter);
            h.canvas
                .drain()
                .iter()
                .filter(|q| q.texture.is_none() && q.destination.w == width)
                .map(|q| q.color.a)
                .next_back()
                .expect("frame quad")
        };
        assert!(
            frame_alpha(&mut h, &|_, _| DIALOG_SIZE.x) < 1.,
            "the options dialog over the game stays translucent"
        );
        for sub in [
            Sub::Gamma,
            Sub::AudioCredits,
            Sub::VideoCredits,
            Sub::KeyboardAdvanced,
            Sub::VideoAdvanced,
        ] {
            h.dialog.open_sub(sub);
            let width = |d: &OptionsDialog, p: &Painter| {
                d.sub_frame_size(sub, |s| p.text_width(s, false)).x
            };
            assert_eq!(frame_alpha(&mut h, &width), 1., "{sub:?}");
            h.dialog.cancel_sub();
        }
        h.tab(Tab::Keyboard);
        h.dialog.command(Command::KeyDefaults);
        let width = |d: &OptionsDialog, p: &Painter| {
            let confirm = d.confirm.as_ref().expect("confirmation box");
            confirm.layout(|t| p.text_width(t, false)).size.x
        };
        assert_eq!(frame_alpha(&mut h, &width), 1., "confirmation box");
    }

    #[test]
    fn options_dialog_buttons_follow_press_cancel_and_release_activation() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);

        // 1. OK button interaction
        let ok_rect = button_rects()[0];
        let ok_pos = DIALOG + vec2(ok_rect.x + 4., ok_rect.y + 4.);
        let away_pos = DIALOG + vec2(10., 10.);

        // Hover over OK
        h.frame(Input {
            pointer: Some(ok_pos),
            ..Default::default()
        });
        assert_eq!(h.dialog.armed, Some(Target::Ok));
        assert_eq!(h.dialog.pressed, None);

        // Press down on OK
        let outcome = h.frame(Input {
            pointer: Some(ok_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        assert_eq!(outcome, OptionsOutcome::None, "no action on press down");
        assert_eq!(h.dialog.pressed, Some(Target::Ok));

        // Move away while held: the press captured the mouse, so OK stays pressed
        h.frame(Input {
            pointer: Some(away_pos),
            held: true,
            ..Default::default()
        });
        assert_eq!(h.dialog.pressed, Some(Target::Ok), "captured on move away");
        assert_eq!(h.dialog.armed, None);

        // Release away: does not activate
        let outcome = h.frame(Input {
            pointer: Some(away_pos),
            released: true,
            ..Default::default()
        });
        assert_eq!(outcome, OptionsOutcome::None, "no action on release away");
        assert_eq!(h.dialog.pressed, None);

        // Press, drag off and back on, release over OK: activates (release-while-over)
        for (pointer, click) in [(ok_pos, true), (away_pos, false), (ok_pos, false)] {
            h.frame(Input {
                pointer: Some(pointer),
                click,
                held: true,
                ..Default::default()
            });
        }
        assert_eq!(h.dialog.pressed, Some(Target::Ok));
        let outcome = h.frame(Input {
            pointer: Some(ok_pos),
            released: true,
            ..Default::default()
        });
        assert_eq!(outcome, OptionsOutcome::Close(None));
        h.dialog.open(Config::default());
        h.tab(Tab::Mouse);

        // Click on OK properly (press then release over button)
        h.frame(Input {
            pointer: Some(ok_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        assert_eq!(h.dialog.pressed, Some(Target::Ok));
        let outcome = h.frame(Input {
            pointer: Some(ok_pos),
            released: true,
            ..Default::default()
        });
        assert_eq!(outcome, OptionsOutcome::Close(None));

        // Dragging a captured press across the sensitivity slider neither arms nor moves the
        // slider, and returning to OK still activates it.
        let slider_pos = page(300., 40. + SLIDER_TRACK_Y);
        let sensitivity = h.dialog.draft().mouse.sensitivity;
        for (pointer, click) in [(ok_pos, true), (slider_pos, false), (ok_pos, false)] {
            h.frame(Input {
                pointer: Some(pointer),
                click,
                held: true,
                ..Default::default()
            });
            assert_eq!(h.dialog.pressed, Some(Target::Ok));
            assert!(h.dialog.armed.is_none() || h.dialog.armed == Some(Target::Ok));
        }
        assert_eq!(h.dialog.draft().mouse.sensitivity, sensitivity);
        let outcome = h.frame(Input {
            pointer: Some(ok_pos),
            released: true,
            ..Default::default()
        });
        assert_eq!(outcome, OptionsOutcome::Close(None));
        h.dialog.open(Config::default());
        h.tab(Tab::Mouse);

        // 2. CheckBox interaction (Invert mouse at page(30., 20.))
        h.dialog.open(Config::default());
        h.tab(Tab::Mouse);
        let chk_pos = page(30., 20.);
        assert!(!h.dialog.draft().mouse.invert);

        // Press down on checkbox
        h.frame(Input {
            pointer: Some(chk_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        assert!(!h.dialog.draft().mouse.invert, "not toggled on press down");
        assert!(h.dialog.pressed.is_some());

        // Move away while held: still captured
        h.frame(Input {
            pointer: Some(away_pos),
            held: true,
            ..Default::default()
        });
        assert!(h.dialog.pressed.is_some());

        // Release away
        h.frame(Input {
            pointer: Some(away_pos),
            released: true,
            ..Default::default()
        });
        assert!(!h.dialog.draft().mouse.invert, "not toggled after cancel");

        // Click checkbox properly
        h.frame(Input {
            pointer: Some(chk_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        h.frame(Input {
            pointer: Some(chk_pos),
            released: true,
            ..Default::default()
        });
        assert!(h.dialog.draft().mouse.invert, "toggled on release");
    }

    #[test]
    fn options_dialog_sound_events_hover_and_activation() {
        let mut h = Harness::new();
        h.dialog.open(Config::default());
        let ok_rect = button_rects()[0];
        let ok_pos = DIALOG + vec2(ok_rect.x + 4., ok_rect.y + 4.);
        let away_pos = DIALOG + vec2(10., 10.);

        // 1. Hovering OK button does NOT emit rollover sound (dialog button)
        h.frame(Input {
            pointer: Some(ok_pos),
            ..Default::default()
        });
        assert!(h.dialog.take_sounds().is_empty());

        // 2. Hovering OK button again emits nothing
        h.frame(Input {
            pointer: Some(ok_pos),
            ..Default::default()
        });
        assert!(h.dialog.take_sounds().is_empty());

        // 3. Moving away emits nothing
        h.frame(Input {
            pointer: Some(away_pos),
            ..Default::default()
        });
        assert!(h.dialog.take_sounds().is_empty());

        // 4. Clicking and releasing OK button emits click sound
        h.frame(Input {
            pointer: Some(ok_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        assert!(h.dialog.take_sounds().is_empty());
        h.frame(Input {
            pointer: Some(ok_pos),
            released: true,
            ..Default::default()
        });
        assert_eq!(h.dialog.take_sounds(), vec!["ui/buttonclickrelease.wav"]);
    }

    #[test]
    fn options_dialog_close_button_behaves_like_cancel() {
        let mut h = Harness::new();
        h.dialog.open(Config::default());
        h.tab(Tab::Mouse);

        // Modify draft setting (invert mouse)
        let chk_pos = page(30., 20.);
        h.frame(Input {
            pointer: Some(chk_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        h.frame(Input {
            pointer: Some(chk_pos),
            released: true,
            ..Default::default()
        });
        assert!(h.dialog.draft().mouse.invert);
        let _ = h.dialog.take_sounds();

        let close_rect = widgets::close_button_rect(DIALOG_SIZE);
        let close_pos = DIALOG
            + vec2(
                close_rect.x + close_rect.w / 2.,
                close_rect.y + close_rect.h / 2.,
            );

        // Hover over close button: arms Close without emitting rollover sound
        h.frame(Input {
            pointer: Some(close_pos),
            ..Default::default()
        });
        assert_eq!(h.dialog.armed, Some(Target::Close));
        assert!(h.dialog.take_sounds().is_empty());

        // Click and release close button: activates Cancel outcome (Close(None))
        h.frame(Input {
            pointer: Some(close_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        assert_eq!(h.dialog.pressed, Some(Target::Close));

        let outcome = h.frame(Input {
            pointer: Some(close_pos),
            released: true,
            ..Default::default()
        });
        // Crucial: Close(None) discards draft changes, exactly like Cancel!
        assert_eq!(outcome, OptionsOutcome::Close(None));
        assert_eq!(h.dialog.take_sounds(), vec!["ui/buttonclickrelease.wav"]);
    }

    #[test]
    fn options_subdialog_close_button_cancels_and_restores() {
        let mut h = Harness::new();
        h.dialog.open(Config::default());
        h.dialog.open_sub(Sub::Gamma);
        h.dialog.draft.config.video.gamma = 1.6;

        let canvas = vec2(1280., 720.);
        let size = GAMMA_SIZE;
        let gamma_origin = centered(canvas, size, 1.);
        let close_rect = widgets::close_button_rect(size);
        let close_pos = gamma_origin
            + vec2(
                close_rect.x + close_rect.w / 2.,
                close_rect.y + close_rect.h / 2.,
            );

        // Click and release on subdialog close button
        h.frame(Input {
            pointer: Some(close_pos),
            click: true,
            held: true,
            ..Default::default()
        });
        h.frame(Input {
            pointer: Some(close_pos),
            released: true,
            ..Default::default()
        });

        // Subdialog is closed and gamma is restored to saved value
        assert_eq!(h.dialog.sub, None);
        assert_eq!(h.dialog.draft().video.gamma, 2.2);
    }

    #[test]
    fn options_tooltips_lookup_and_missing_case() {
        let mut h = Harness::new();
        h.tab(Tab::Mouse);

        // 1. Direct tip_for_control lookup:
        // Present tooltip:
        assert_eq!(
            h.dialog.tip_for_control(Panel::Mouse, "ReverseMouse"),
            Some("Reverse mouse up-down axis".to_string())
        );
        // Missing tooltip:
        assert_eq!(h.dialog.tip_for_control(Panel::Mouse, "Slider"), None);
        assert_eq!(
            h.dialog.tip_for_control(Panel::Mouse, "SensitivityLabel"),
            None
        );
        // Missing control:
        assert_eq!(h.dialog.tip_for_control(Panel::Mouse, "NonExistent"), None);

        // 2. Hover tip lookup via tooltip_at:
        // ReverseMouse is at (28, 9, 150, 28) on page
        let rev_ptr = page(35., 15.);
        assert_eq!(
            h.dialog.tooltip_at(vec2(1280., 720.), rev_ptr),
            Some("Reverse mouse up-down axis".to_string())
        );
        // Slider is at (200, 40, 200, 40) on page (has no tooltip)
        let slider_ptr = page(250., 50.);
        assert_eq!(h.dialog.tooltip_at(vec2(1280., 720.), slider_ptr), None);
        // Blank space on page
        assert_eq!(h.dialog.tooltip_at(vec2(1280., 720.), page(10., 10.)), None);

        // 3. Active tooltip lifecycle in frame input:
        // Hovering ReverseMouse sets active_tooltip
        h.frame(Input {
            pointer: Some(rev_ptr),
            ..Default::default()
        });
        assert_eq!(
            h.dialog.active_tooltip(),
            Some(&ActiveTooltip {
                text: "Reverse mouse up-down axis".to_string(),
                pointer: rev_ptr,
            })
        );

        // Hovering Slider (missing tip) clears active_tooltip
        h.frame(Input {
            pointer: Some(slider_ptr),
            ..Default::default()
        });
        assert_eq!(h.dialog.active_tooltip(), None);

        // Hovering ReverseMouse again restores active_tooltip
        h.frame(Input {
            pointer: Some(rev_ptr),
            ..Default::default()
        });
        assert!(h.dialog.active_tooltip().is_some());

        // Mouse click dismisses active_tooltip
        h.frame(Input {
            pointer: Some(rev_ptr),
            click: true,
            held: true,
            ..Default::default()
        });
        assert_eq!(h.dialog.active_tooltip(), None);

        // Moving outside page clears active_tooltip
        h.frame(Input {
            pointer: Some(vec2(10., 10.)),
            ..Default::default()
        });
        assert_eq!(h.dialog.active_tooltip(), None);
    }

    #[test]
    fn credits_subdialog_sizing_and_no_scrolling() {
        let mut h = Harness::new();
        let mut loc = Localization::default();
        loc.insert(
            "GameUI_Miles_Audio",
            "Miles Sound System Copyright © 1991-2001 RAD Game Tools, Inc. Unmatched quality audio processing.",
        );
        loc.insert(
            "GameUI_Miles_Voice",
            "Miles Voice Compression System Copyright © 1991-2001 RAD Game Tools, Inc.",
        );
        loc.insert(
            "GameUI_Bink",
            "Uses Bink Video. Copyright © 1997-2004 by RAD Game Tools, Inc.",
        );
        loc.insert("GameUI_Close", "Close");
        h.dialog.res = Arc::new(GameUiResources {
            localization: loc,
            ..Default::default()
        });

        h.dialog.sub = Some(Sub::AudioCredits);
        let painter = Painter {
            canvas: &h.canvas,
            fonts: &h.fonts,
            theme: &h.theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        // 1. Sizing is derived dynamically from content (Audio vs Video credits)
        let audio_items = h.dialog.credits_items(Sub::AudioCredits);
        let audio_content_h =
            h.dialog
                .measure_credits_height(&audio_items, CREDITS_SIZE.x - 32., |s| {
                    s.len() as f32 * 7.
                });
        let audio_size = h
            .dialog
            .sub_frame_size(Sub::AudioCredits, |s| s.len() as f32 * 7.);
        assert_eq!(audio_size.x, CREDITS_SIZE.x);
        assert_eq!(audio_size.y, 34. + audio_content_h + 46.);

        let video_items = h.dialog.credits_items(Sub::VideoCredits);
        let video_content_h =
            h.dialog
                .measure_credits_height(&video_items, CREDITS_SIZE.x - 32., |s| {
                    s.len() as f32 * 7.
                });
        let video_size = h
            .dialog
            .sub_frame_size(Sub::VideoCredits, |s| s.len() as f32 * 7.);
        assert_eq!(video_size.x, CREDITS_SIZE.x);
        assert_eq!(video_size.y, 34. + video_content_h + 46.);
        assert!(
            audio_size.y > video_size.y,
            "Audio credits with more items should be taller than video credits"
        );

        // 2. Draw audio credits and verify no scrollbar exists, nothing exceeds bounds, and nothing overlaps
        h.dialog.draw(&painter);
        let quads = h.canvas.drain();

        let real_audio_size = h
            .dialog
            .sub_frame_size(Sub::AudioCredits, |s| painter.text_width(s, false));
        let origin = centered(vec2(1280., 720.), real_audio_size, 1.);
        let dialog_bounds = Rect::new(origin.x, origin.y, real_audio_size.x, real_audio_size.y);
        let close_rect = Rect::new(
            origin.x + real_audio_size.x - 96.,
            origin.y + real_audio_size.y - 34.,
            80.,
            24.,
        );

        // Assert no scrollbar quad was rendered anywhere
        let sb_w = widgets::SCROLLBAR_WIDTH;
        let has_scrollbar = quads.iter().any(|q| {
            let r = q.destination;
            r.w == sb_w && r.h > 20.
        });
        assert!(
            !has_scrollbar,
            "Scrollbar must NOT be rendered; credits dialog is sized to content instead of scrolling"
        );

        // Assert Close button quad exists and is fully inside dialog bounds
        let has_close_button = quads.iter().any(|q| {
            let r = q.destination;
            r.x >= close_rect.x - 1.
                && r.x + r.w <= close_rect.x + close_rect.w + 1.
                && r.y >= close_rect.y - 1.
                && r.y + r.h <= close_rect.y + close_rect.h + 1.
        });
        assert!(
            has_close_button,
            "Expected Close button quad at bottom right inside dialog bounds"
        );

        // Assert every rendered subdialog quad is strictly within the dialog bounds (nothing exceeds bounds)
        let credits_frame_idx = quads
            .iter()
            .position(|q| q.destination.w == real_audio_size.x)
            .expect("audio credits frame quad");
        let subdialog_start_idx = credits_frame_idx.saturating_sub(1);
        let subdialog_quads = &quads[subdialog_start_idx..];
        for q in subdialog_quads {
            let r = q.destination;
            assert!(
                r.x >= dialog_bounds.x - 1. && r.x + r.w <= dialog_bounds.x + dialog_bounds.w + 1.,
                "Quad at ({}, {}) exceeds dialog horizontal bounds [{}, {}]",
                r.x,
                r.y,
                dialog_bounds.x,
                dialog_bounds.x + dialog_bounds.w
            );
            assert!(
                r.y >= dialog_bounds.y - 1. && r.y + r.h <= dialog_bounds.y + dialog_bounds.h + 1.,
                "Quad at ({}, {}) exceeds dialog vertical bounds [{}, {}]",
                r.x,
                r.y,
                dialog_bounds.y,
                dialog_bounds.y + dialog_bounds.h
            );
        }

        // 3. Close button dismisses credits subdialog
        let close_click_pos = vec2(close_rect.x + 10., close_rect.y + 10.);
        h.dialog.input(
            &Input {
                pointer: Some(close_click_pos),
                click: true,
                held: true,
                ..Default::default()
            },
            &painter,
        );
        h.dialog.input(
            &Input {
                pointer: Some(close_click_pos),
                released: true,
                ..Default::default()
            },
            &painter,
        );
        assert!(
            h.dialog.sub.is_none(),
            "Close button should dismiss credits subdialog"
        );
    }

    #[test]
    fn credits_dialog_reports_when_exceeding_window_at_small_resolutions() {
        let mut h = Harness::new();
        h.dialog.sub = Some(Sub::AudioCredits);

        let painter = Painter {
            canvas: &h.canvas,
            fonts: &h.fonts,
            theme: &h.theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        // Standard resolution fits cleanly
        assert!(
            h.dialog
                .credits_fits_window(Sub::AudioCredits, vec2(1280., 720.), |s| painter
                    .text_width(s, false)),
            "Should fit window at 1280x720"
        );
        assert!(
            h.dialog
                .credits_fits_window(Sub::AudioCredits, vec2(1024., 768.), |s| painter
                    .text_width(s, false)),
            "Should fit window at 1024x768"
        );

        // Very small resolution cannot fit window
        let small_canvas = vec2(400., 200.);
        assert!(
            !h.dialog
                .credits_fits_window(Sub::AudioCredits, small_canvas, |s| painter
                    .text_width(s, false)),
            "Cannot fit window at 400x200"
        );

        // Running input on small canvas logs report message to console log
        let small_canvas_obj = Canvas::default();
        small_canvas_obj.resize(400., 200.);
        let small_painter = Painter {
            canvas: &small_canvas_obj,
            fonts: &h.fonts,
            theme: &h.theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };
        h.dialog.input(&Input::default(), &small_painter);

        let log = h.dialog.take_log();
        assert!(
            log.iter().any(|msg| msg.contains("Credits dialog content")
                && msg.contains("cannot fit window bounds")),
            "Expected report in log when content cannot fit window at small resolution, got: {:?}",
            log
        );
    }
}
