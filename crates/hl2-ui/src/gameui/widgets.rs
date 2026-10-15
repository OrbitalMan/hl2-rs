//! VGUI-style widget drawing and the small pieces of widget logic shared by the menus and dialogs.
//!
//! Colors come from the installed `SourceSchemeBase.res` (`BaseSettings` names resolved through
//! `Colors`); the defaults below are only used when the scheme cannot be read. They mirror the
//! desktop (`[$WIN32]`) values of the installed `SourceScheme.res`, because the 20th-anniversary
//! `hl2_complete/resource/SourceSchemeBase.res` is an empty file and the defaults are what the
//! game currently runs with on that layout (known gap, see `docs/gameui.md` 6a). Geometry is in
//! VGUI layout pixels; a [`Painter`] maps it to canvas pixels with an origin and a uniform scale.
//! The scale is `max(1, canvas_height / 720)`: retail draws these non-proportional dialogs at
//! 1:1 screen pixels, which becomes unreadably small on high-resolution canvases (documented
//! deviation).

use crate::canvas::{draw_texture_ex, Canvas, Color, DrawTextureParams, Rect, Texture2D, WHITE};
use crate::hud::FontFace;
use glam::{vec2, Vec2};
use source_assets::keyvalues::Entry;

/// Character-cell height of the scheme's `Default`/`DefaultBold` fonts at 1:1 scale.
pub const FONT_TALL: f32 = 16.;

/// Classification of interactive UI controls for sound and interaction behaviors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    /// Main menu and pause/submenu navigation buttons.
    MenuButton,
    /// Standard push button inside dialogs (OK, Cancel, Apply, Advanced, etc.).
    DialogButton,
    /// Radio button choice option.
    RadioButton,
    /// Dropdown / combo box list selector.
    Dropdown,
    /// Single-line or multi-line text input field.
    TextInput,
    /// Check box toggle button.
    CheckBox,
    /// Slider drag control.
    Slider,
    /// Non-interactive or decorative element.
    Other,
}

impl WidgetKind {
    /// Returns whether this control kind is eligible for the hover / rollover sound cue
    /// (`ui/buttonrollover.wav`).
    ///
    /// In authentic Source / Half-Life 2 GameUI, only top-level menu and submenu navigation buttons
    /// play a rollover sound when hovered or stepped. Dialog buttons, radio buttons, dropdowns, and
    /// text input fields inside dialogs do not play rollover sounds.
    #[inline]
    pub fn has_rollover_sound(self) -> bool {
        matches!(self, Self::MenuButton)
    }
}

/// Scheme colors used by the GameUI controls.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub frame_bg: Color,
    pub title_text: Color,
    pub text: Color,
    pub text_dull: Color,
    pub text_disabled: Color,
    pub text_disabled_shadow: Color,
    pub border_bright: Color,
    pub border_dark: Color,
    pub border_selection: Color,
    pub entry_bg: Color,
    pub selection_bg: Color,
    pub selection_text: Color,
    pub menu_bg: Color,
    pub list_bg: Color,
    pub slider_track: Color,
    pub slider_nob: Color,
    pub slider_text: Color,
    pub check: Color,
    pub arrow: Color,
    pub arrow_armed: Color,
    pub scrollbar_bg: Color,
    pub url_text: Color,
    pub tooltip_bg: Color,
    pub tooltip_text: Color,
    pub tooltip_border: Color,
    /// Push button colors per interaction state (`Button.TextColor` / `Button.BgColor`,
    /// `Button.ArmedTextColor` / `Button.ArmedBgColor`, `Button.DepressedTextColor` /
    /// `Button.DepressedBgColor`).
    pub button_text: Color,
    pub button_bg: Color,
    pub button_armed_text: Color,
    pub button_armed_bg: Color,
    pub button_depressed_text: Color,
    pub button_depressed_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        let rgba = Color::from_rgba;
        Self {
            frame_bg: rgba(132, 132, 132, 212),
            title_text: rgba(255, 255, 255, 255),
            text: rgba(255, 255, 255, 255),
            text_dull: rgba(190, 190, 190, 255),
            text_disabled: rgba(117, 117, 117, 255),
            text_disabled_shadow: rgba(30, 30, 30, 255),
            border_bright: rgba(200, 200, 200, 196),
            border_dark: rgba(40, 40, 40, 196),
            border_selection: rgba(0, 0, 0, 196),
            entry_bg: rgba(0, 0, 0, 128),
            selection_bg: rgba(255, 155, 0, 255),
            selection_text: rgba(0, 0, 0, 255),
            menu_bg: rgba(160, 160, 160, 64),
            list_bg: rgba(0, 0, 0, 128),
            slider_track: rgba(31, 31, 31, 255),
            slider_nob: rgba(108, 108, 108, 255),
            slider_text: rgba(180, 180, 180, 255),
            check: rgba(255, 255, 255, 255),
            arrow: rgba(190, 190, 190, 255),
            arrow_armed: rgba(255, 255, 255, 255),
            scrollbar_bg: rgba(255, 255, 255, 64),
            url_text: rgba(255, 155, 0, 255),
            tooltip_bg: rgba(255, 155, 0, 240),
            tooltip_text: rgba(0, 0, 0, 255),
            tooltip_border: rgba(40, 40, 40, 255),
            // The desktop scheme (`[$WIN32]`) keeps armed and depressed identical to the
            // normal colors: White text over Blank.
            button_text: rgba(255, 255, 255, 255),
            button_bg: rgba(0, 0, 0, 0),
            button_armed_text: rgba(255, 255, 255, 255),
            button_armed_bg: rgba(0, 0, 0, 0),
            button_depressed_text: rgba(255, 255, 255, 255),
            button_depressed_bg: rgba(0, 0, 0, 0),
        }
    }
}

impl Theme {
    /// Reads the scheme colors, keeping defaults for anything missing.
    pub fn from_scheme(root: &Entry) -> Self {
        let mut theme = Self::default();
        let fields: [(&str, &mut Color); 29] = [
            ("Frame.BgColor", &mut theme.frame_bg),
            ("FrameTitleBar.TextColor", &mut theme.title_text),
            ("Label.TextBrightColor", &mut theme.text),
            ("Label.TextDullColor", &mut theme.text_dull),
            ("Label.DisabledFgColor1", &mut theme.text_disabled),
            ("Label.DisabledFgColor2", &mut theme.text_disabled_shadow),
            ("Border.Bright", &mut theme.border_bright),
            ("Border.Dark", &mut theme.border_dark),
            ("Border.Selection", &mut theme.border_selection),
            ("TextEntry.BgColor", &mut theme.entry_bg),
            ("ListPanel.SelectedBgColor", &mut theme.selection_bg),
            ("ListPanel.SelectedTextColor", &mut theme.selection_text),
            ("Menu.BgColor", &mut theme.menu_bg),
            ("SectionedListPanel.BgColor", &mut theme.list_bg),
            ("Slider.TrackColor", &mut theme.slider_track),
            ("Slider.NobColor", &mut theme.slider_nob),
            ("Slider.TextColor", &mut theme.slider_text),
            ("CheckButton.Check", &mut theme.check),
            ("ComboBoxButton.ArrowColor", &mut theme.arrow),
            ("ComboBoxButton.ArmedArrowColor", &mut theme.arrow_armed),
            ("ScrollBarSlider.BgColor", &mut theme.scrollbar_bg),
            ("Tooltip.BgColor", &mut theme.tooltip_bg),
            ("Tooltip.TextColor", &mut theme.tooltip_text),
            ("Button.TextColor", &mut theme.button_text),
            ("Button.BgColor", &mut theme.button_bg),
            ("Button.ArmedTextColor", &mut theme.button_armed_text),
            ("Button.ArmedBgColor", &mut theme.button_armed_bg),
            (
                "Button.DepressedTextColor",
                &mut theme.button_depressed_text,
            ),
            ("Button.DepressedBgColor", &mut theme.button_depressed_bg),
        ];
        for (name, slot) in fields {
            if let Some(color) = scheme_color(root, name) {
                *slot = color;
            }
        }
        theme
    }

    /// Resolves the look of a bordered push button for an interaction state.
    ///
    /// Follows the SDK `Button` precedence: colors are depressed, then armed, then normal;
    /// the border is depressed, then key focus (the default button), then raised. Bordered
    /// buttons have no armed border, so hover only shows through the scheme's armed colors.
    /// Disabled buttons are never armed or depressed.
    pub fn button_style(&self, state: ButtonState) -> ButtonStyle {
        let (text, bg) = if state.depressed() {
            (self.button_depressed_text, self.button_depressed_bg)
        } else if state.hovered() {
            (self.button_armed_text, self.button_armed_bg)
        } else {
            (self.button_text, self.button_bg)
        };
        let border = if state.depressed() {
            ButtonBorder::Depressed
        } else if state.default && state.enabled {
            ButtonBorder::KeyFocus
        } else {
            ButtonBorder::Raised
        };
        ButtonStyle {
            border,
            text,
            bg,
            text_offset: if state.depressed() { 1. } else { 0. },
        }
    }

    /// Resolves the look of a property-sheet tab.
    ///
    /// Tabs (SDK `PageTab`) never enter the armed state and pick their border and text color
    /// from the active flag alone, so hover and press leave a tab's look unchanged, whether
    /// it is the active tab or not.
    pub fn tab_style(&self, active: bool, _state: ButtonState) -> TabStyle {
        TabStyle {
            text: if active { self.text } else { self.text_dull },
            open_bottom: active,
        }
    }
}

/// Border of a bordered push button (`ButtonBorder`, `ButtonDepressedBorder`,
/// `ButtonKeyFocusBorder`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonBorder {
    Raised,
    Depressed,
    KeyFocus,
}

/// Resolved push button look; see [`Theme::button_style`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonStyle {
    pub border: ButtonBorder,
    pub text: Color,
    pub bg: Color,
    /// Label shift while depressed, in layout pixels.
    pub text_offset: f32,
}

/// Resolved tab look; see [`Theme::tab_style`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabStyle {
    pub text: Color,
    /// The active tab has no bottom edge, so it opens into its page.
    pub open_bottom: bool,
}

/// Resolves a scheme color: a literal `"r g b a"` or a name in `BaseSettings`/`Colors`.
pub fn scheme_color(root: &Entry, name: &str) -> Option<Color> {
    let mut value = name;
    for _ in 0..8 {
        let values: Vec<u8> = value
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()
            .unwrap_or_default();
        if let [r, g, b, a] = values.as_slice() {
            return Some(Color::from_rgba(*r, *g, *b, *a));
        }
        value = root
            .get("BaseSettings")
            .and_then(|e| e.get(value))
            .or_else(|| root.get("Colors").and_then(|e| e.get(value)))?
            .text()?;
    }
    None
}

/// Fonts used by the controls. Missing fonts skip text drawing but keep layout and input working.
#[derive(Clone, Default)]
pub struct Fonts {
    pub(crate) text: Option<FontFace>,
    pub(crate) bold: Option<FontFace>,
}

/// Text alignment inside a rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Maps layout rectangles to canvas pixels and draws primitives.
pub struct Painter<'a> {
    pub canvas: &'a Canvas,
    pub fonts: &'a Fonts,
    pub theme: &'a Theme,
    pub origin: Vec2,
    pub scale: f32,
}

impl Painter<'_> {
    /// The dialog scale for a canvas height (see module documentation).
    pub fn scale_for(canvas_height: f32) -> f32 {
        (canvas_height / 720.).max(1.)
    }

    /// Computes the dialog scale for a given window logical height and display scale factor.
    pub fn scale_for_window(logical_height: f32, scale_factor: f32) -> f32 {
        Self::scale_for(logical_height * scale_factor.max(1.0))
    }

    pub fn at(&self, offset: Vec2) -> Painter<'_> {
        Painter {
            canvas: self.canvas,
            fonts: self.fonts,
            theme: self.theme,
            origin: self.origin + offset * self.scale,
            scale: self.scale,
        }
    }

    pub fn map(&self, rect: Rect) -> Rect {
        Rect::new(
            (self.origin.x + rect.x * self.scale).round(),
            (self.origin.y + rect.y * self.scale).round(),
            (rect.w * self.scale).round(),
            (rect.h * self.scale).round(),
        )
    }

    pub fn fill(&self, rect: Rect, color: Color) {
        let r = self.map(rect);
        self.canvas.rectangle(r.x, r.y, r.w, r.h, color);
    }

    /// One-pixel (scaled) outline with separate top-left and bottom-right colors.
    pub fn border(&self, rect: Rect, top_left: Color, bottom_right: Color) {
        let r = self.map(rect);
        let t = self.scale.round().max(1.);
        self.canvas.rectangle(r.x, r.y, r.w, t, top_left);
        self.canvas.rectangle(r.x, r.y, t, r.h, top_left);
        self.canvas
            .rectangle(r.x, r.y + r.h - t, r.w, t, bottom_right);
        self.canvas
            .rectangle(r.x + r.w - t, r.y, t, r.h, bottom_right);
    }

    pub fn raised(&self, rect: Rect) {
        self.border(rect, self.theme.border_bright, self.theme.border_dark);
    }

    pub fn sunken(&self, rect: Rect) {
        self.border(rect, self.theme.border_dark, self.theme.border_bright);
    }

    fn font(&self, bold: bool) -> Option<&FontFace> {
        if bold {
            self.fonts.bold.as_ref().or(self.fonts.text.as_ref())
        } else {
            self.fonts.text.as_ref()
        }
    }

    /// Text width in layout units (max line width for multi-line text).
    pub fn text_width(&self, text: &str, bold: bool) -> f32 {
        self.font(bold).map_or(
            text.lines()
                .map(|l| l.chars().count() as f32 * 7.)
                .fold(0.0, f32::max),
            |f| {
                text.lines()
                    .map(|l| f.width_cell(l, FONT_TALL))
                    .fold(0.0, f32::max)
            },
        )
    }

    /// Draws text with its cell top-left at a layout position. Multi-line text is split on newlines.
    pub fn text(&self, text: &str, at: Vec2, color: Color, bold: bool) {
        if let Some(font) = self.font(bold) {
            for (i, line) in text.lines().enumerate() {
                let p = self.origin + (at + vec2(0., i as f32 * FONT_TALL)) * self.scale;
                font.draw_cell(line, p.round(), FONT_TALL * self.scale, color);
            }
        }
    }

    /// Draws one line of text vertically centered in `rect`.
    pub fn text_in(&self, text: &str, rect: Rect, align: Align, color: Color, bold: bool) {
        let width = self.text_width(text, bold);
        let x = match align {
            Align::Left => rect.x,
            Align::Center => rect.x + (rect.w - width) * 0.5,
            Align::Right => rect.x + rect.w - width,
        };
        self.text(
            text,
            vec2(x, rect.y + (rect.h - FONT_TALL) * 0.5),
            color,
            bold,
        );
    }

    /// Label text: bright when enabled, VGUI's embossed two-tone gray when disabled.
    pub fn label(&self, text: &str, rect: Rect, align: Align, enabled: bool) {
        if enabled {
            self.text_in(text, rect, align, self.theme.text, false);
        } else {
            let shadow = Rect::new(rect.x + 1., rect.y + 1., rect.w, rect.h);
            self.text_in(text, shadow, align, self.theme.text_disabled_shadow, false);
            self.text_in(text, rect, align, self.theme.text_disabled, false);
        }
    }

    /// Draws multi-line text with word wrapping, respecting `rect` bounds and alignment.
    /// Lines exceeding `rect.h` are truncated so text never overflows its parent bounds.
    pub fn multiline_label(&self, text: &str, rect: Rect, align: Align, color: Color) {
        let max_w = rect.w.max(1.);
        let lines = wrap(text, max_w, |s| self.text_width(s, false));
        let mut y = rect.y;
        for line in lines {
            if y + FONT_TALL > rect.y + rect.h + FONT_TALL * 0.5 {
                break;
            }
            let line_w = self.text_width(&line, false);
            let x = match align {
                Align::Left => rect.x,
                Align::Center => rect.x + ((rect.w - line_w) * 0.5).max(0.),
                Align::Right => rect.x + (rect.w - line_w).max(0.),
            };
            self.text(&line, vec2(x, y), color, false);
            y += FONT_TALL;
        }
    }

    /// Word-wrapped text from the top-left of `rect` (north-west labels).
    pub fn wrapped(&self, text: &str, rect: Rect, color: Color) {
        self.multiline_label(text, rect, Align::Left, color);
    }

    /// A filled disc approximated by one rectangle per scaled pixel row.
    pub fn disc(&self, center: Vec2, radius: f32, color: Color) {
        let c = self.origin + center * self.scale;
        let r = radius * self.scale;
        let rows = (2. * r).ceil() as i32;
        for row in 0..rows {
            let dy = row as f32 + 0.5 - r;
            let half = (r * r - dy * dy).max(0.).sqrt();
            self.canvas.rectangle(
                (c.x - half).round(),
                (c.y - r + row as f32).round(),
                (2. * half).round(),
                1.,
                color,
            );
        }
    }

    /// A downward triangle (combo box arrow) built from row rectangles.
    pub fn arrow_down(&self, center: Vec2, half_width: f32, color: Color) {
        let c = self.origin + center * self.scale;
        let w = half_width * self.scale;
        let rows = w.ceil() as i32;
        for row in 0..rows {
            let half = w - row as f32;
            self.canvas.rectangle(
                (c.x - half).round(),
                (c.y - w * 0.5 + row as f32).round(),
                (2. * half).round().max(1.),
                1.,
                color,
            );
        }
    }

    /// Draws a checkmark inside a checkbox's box rectangle (`box_rect`).
    ///
    /// Matches the Source VGUI checkmark appearance: a 2-pixel-thick tick mark
    /// with a short descending left stroke and longer ascending right stroke.
    pub fn check_mark(&self, box_rect: Rect, color: Color, offset: f32) {
        let x0 = box_rect.x + offset;
        let y0 = box_rect.y + offset;
        // The checkmark is composed of 7 vertical columns (each 2 units tall) centered inside the 12x12 box:
        // Left arm descends from (3, 5) to apex at (5, 8); right arm ascends from (5, 8) to (9, 3).
        const COLUMNS: [(f32, f32); 7] = [
            (3., 5.),
            (4., 6.),
            (5., 7.),
            (6., 6.),
            (7., 5.),
            (8., 4.),
            (9., 3.),
        ];
        for (dx, dy) in COLUMNS {
            self.fill(Rect::new(x0 + dx, y0 + dy, 1., 2.), color);
        }
    }
}

/// Frame client area offsets (`Frame.ClientInsetX`, title height) at 1:1 scale.
pub const CLIENT_INSET_X: f32 = 8.;
/// Top of a frame's client area: inset 6 + caption 16 + border 7 + 1.
pub const CLIENT_TOP: f32 = 30.;
/// `Frame.TitleTextInsetX`.
pub const TITLE_INSET_X: f32 = 16.;
/// Close button dimensions at 1:1 scale (Source VGUI FrameButton: 18x16 unscaled).
pub const CLOSE_BUTTON_SIZE: Vec2 = vec2(18., 16.);
/// Inset of the close button from the top-right corner of the frame.
pub const CLOSE_BUTTON_INSET: Vec2 = vec2(24., 6.);

/// Authentic corner radius for VGUI rounded dialog frames (`FrameBorder` `backgroundtype 2`).
///
/// In the Source SDK / retail GameUI, rounded frame borders (`FrameBorder`) use 8px corner
/// textures at base resolution, scaling to ~5.0 layout points.
pub const FRAME_CORNER_RADIUS: f32 = 5.0;

/// Style configuration for VGUI dialog frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStyle {
    /// Corner radius in unscaled layout points.
    pub corner_radius: f32,
    /// Whether corners are rendered with rounded arcs.
    pub rounded: bool,
    /// Whether content, title bar, and close buttons are clipped to the rounded shape.
    pub clip_overflow: bool,
    /// Whether an outer 3D raised border is drawn.
    ///
    /// Per Source GameUI specifications (`sourcescheme.res` `FrameBorder`), dialog frames
    /// use `backgroundtype 2` with rounded corners and do NOT draw outer 3D raised borders.
    pub outer_3d_border: bool,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            corner_radius: FRAME_CORNER_RADIUS,
            rounded: true,
            clip_overflow: true,
            outer_3d_border: false,
        }
    }
}

impl FrameStyle {
    pub const DEFAULT: Self = Self {
        corner_radius: FRAME_CORNER_RADIUS,
        rounded: true,
        clip_overflow: true,
        outer_3d_border: false,
    };

    /// Returns whether `point` (in frame layout coordinates) falls inside the styled frame bounds.
    pub fn contains_point(&self, point: Vec2, frame_size: Vec2) -> bool {
        if point.x < 0. || point.y < 0. || point.x > frame_size.x || point.y > frame_size.y {
            return false;
        }
        if !self.rounded || self.corner_radius <= 0. {
            return true;
        }
        let r = self
            .corner_radius
            .min(frame_size.x * 0.5)
            .min(frame_size.y * 0.5);
        // Top-left corner
        if point.x < r && point.y < r {
            let dx = r - point.x;
            let dy = r - point.y;
            return dx * dx + dy * dy <= r * r;
        }
        // Top-right corner
        if point.x > frame_size.x - r && point.y < r {
            let dx = point.x - (frame_size.x - r);
            let dy = r - point.y;
            return dx * dx + dy * dy <= r * r;
        }
        // Bottom-left corner
        if point.x < r && point.y > frame_size.y - r {
            let dx = r - point.x;
            let dy = point.y - (frame_size.y - r);
            return dx * dx + dy * dy <= r * r;
        }
        // Bottom-right corner
        if point.x > frame_size.x - r && point.y > frame_size.y - r {
            let dx = point.x - (frame_size.x - r);
            let dy = point.y - (frame_size.y - r);
            return dx * dx + dy * dy <= r * r;
        }
        true
    }

    /// Clips a child layout rectangle to stay strictly within the frame bounds,
    /// insetting from corners when `clip_overflow` is enabled.
    pub fn clip_rect(&self, rect: Rect, frame_size: Vec2) -> Option<Rect> {
        let mut x0 = rect.x.max(0.);
        let mut y0 = rect.y.max(0.);
        let mut x1 = (rect.x + rect.w).min(frame_size.x);
        let mut y1 = (rect.y + rect.h).min(frame_size.y);

        if x1 <= x0 || y1 <= y0 {
            return None;
        }

        if self.clip_overflow && self.rounded && self.corner_radius > 0. {
            let r = self
                .corner_radius
                .min(frame_size.x * 0.5)
                .min(frame_size.y * 0.5);
            let inset = r * 0.2929;
            if x0 < r && y0 < r {
                x0 = x0.max(inset);
                y0 = y0.max(inset);
            }
            if x1 > frame_size.x - r && y0 < r {
                x1 = x1.min(frame_size.x - inset);
                y0 = y0.max(inset);
            }
            if x0 < r && y1 > frame_size.y - r {
                x0 = x0.max(inset);
                y1 = y1.min(frame_size.y - inset);
            }
            if x1 > frame_size.x - r && y1 > frame_size.y - r {
                x1 = x1.min(frame_size.x - inset);
                y1 = y1.min(frame_size.y - inset);
            }
        }

        if x1 <= x0 || y1 <= y0 {
            None
        } else {
            Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
        }
    }
}

/// Calculates the layout rectangle of the close button for a frame of the given `size`.
pub fn close_button_rect(frame_size: Vec2) -> Rect {
    Rect::new(
        frame_size.x - CLOSE_BUTTON_INSET.x,
        CLOSE_BUTTON_INSET.y,
        CLOSE_BUTTON_SIZE.x,
        CLOSE_BUTTON_SIZE.y,
    )
}

/// Declarative specification for a VGUI frame window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSpec<'a> {
    pub size: Vec2,
    pub title: &'a str,
    /// Whether the frame displays a close (X) button in the top-right corner.
    pub has_close: bool,
    /// Style configuration for this frame (corners, borders, clipping).
    pub style: FrameStyle,
    /// Optional background fill color override. When `None`, defaults to `theme.frame_bg`.
    pub bg_color: Option<Color>,
}

impl<'a> FrameSpec<'a> {
    pub const fn new(size: Vec2, title: &'a str) -> Self {
        Self {
            size,
            title,
            has_close: true,
            style: FrameStyle::DEFAULT,
            bg_color: None,
        }
    }

    pub const fn without_close(mut self) -> Self {
        self.has_close = false;
        self
    }

    pub const fn with_style(mut self, style: FrameStyle) -> Self {
        self.style = style;
        self
    }

    pub const fn with_bg_color(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn close_rect(&self) -> Option<Rect> {
        if !self.has_close {
            return None;
        }
        let raw_rect = close_button_rect(self.size);
        if self.style.clip_overflow {
            self.style.clip_rect(raw_rect, self.size)
        } else {
            Some(raw_rect)
        }
    }
}

/// Button interaction state for drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub armed: bool,
    pub pressed: bool,
    pub enabled: bool,
    /// The dialog's default button (drawn with the selection border).
    pub default: bool,
}

impl ButtonState {
    pub const fn new(enabled: bool) -> Self {
        Self {
            armed: false,
            pressed: false,
            enabled,
            default: false,
        }
    }

    pub const fn active(enabled: bool, armed: bool, pressed: bool) -> Self {
        Self {
            armed,
            pressed,
            enabled,
            default: false,
        }
    }

    /// Drawn pushed in: pressed and enabled. Pressing overrides hover.
    pub const fn depressed(self) -> bool {
        self.pressed && self.enabled
    }

    /// Drawn with the hover (armed) look: armed, enabled and not depressed.
    pub const fn hovered(self) -> bool {
        self.armed && self.enabled && !self.depressed()
    }
}

/// Tracks pointer button press, hold, cancellation and release activation for interactive controls.
///
/// Two drag-off policies exist. The default ([`ButtonTracker::new`]) cancels the press when the
/// pointer leaves the control while held, as for menu items, which do not capture the mouse.
/// [`ButtonTracker::capturing`] follows the SDK `Button` default for dialog controls: a press
/// captures the mouse, the control stays pressed while the pointer is dragged off and back,
/// and the release activates it only while the pointer is over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonTracker<T: Copy + PartialEq> {
    pressed: Option<T>,
    was_down: bool,
    capture: bool,
}

impl<T: Copy + PartialEq> ButtonTracker<T> {
    pub const fn new() -> Self {
        Self {
            pressed: None,
            was_down: false,
            capture: false,
        }
    }

    /// A tracker whose presses capture the mouse until release (dialog controls).
    pub const fn capturing() -> Self {
        Self {
            pressed: None,
            was_down: false,
            capture: true,
        }
    }

    /// Current pressed control, if any.
    pub fn pressed(&self) -> Option<T> {
        self.pressed
    }

    /// Whether the given control is currently pressed.
    pub fn is_pressed(&self, target: T) -> bool {
        self.pressed == Some(target)
    }

    /// The control that may show its hover look: while a capturing press is held only the
    /// pressed control can be armed, since the captured mouse reaches no other control.
    pub fn armable(&self, hovered: Option<T>) -> Option<T> {
        match self.pressed {
            Some(target) if self.capture => hovered.filter(|h| *h == target),
            _ => hovered,
        }
    }

    /// Drops any press. The button edge state is kept: it describes the physical button, and
    /// clearing it would make a button still held from another layer's click read as a new
    /// press on the next frame.
    pub fn reset(&mut self) {
        self.pressed = None;
    }

    /// Advances the tracker on a frame when a layer above owns the pointer (an open popup
    /// list or a modal dialog): nothing is pressed or activated here, but the button edge
    /// state stays current, so the rest of a gesture the upper layer consumed is never read
    /// as a new press when this layer gets the pointer back.
    pub fn occlude(&mut self, down: bool) {
        self.pressed = None;
        self.was_down = down;
    }

    /// Updates the state machine for the current frame.
    ///
    /// - `hovered`: The interactive control under the cursor, if any.
    /// - `down`: Whether the mouse button is down (`click` or `held`).
    /// - `click`: Whether the mouse button went down this frame.
    /// - `released`: Whether the mouse button was released this frame.
    ///
    /// Returns `Some(T)` only on release while the cursor is still over the control.
    /// If the cursor moves away while the mouse button is held, a non-capturing tracker
    /// cancels the press, so a later release does not trigger the action; a capturing
    /// tracker keeps it, so releasing after moving back over the control still does.
    pub fn update(
        &mut self,
        hovered: Option<T>,
        down: bool,
        click: bool,
        released: bool,
    ) -> Option<T> {
        let is_release = released || (self.was_down && !down);
        let mut activated = None;

        if is_release {
            if let Some(target) = self.pressed {
                if hovered == Some(target) {
                    activated = Some(target);
                }
            }
            self.pressed = None;
        } else if click || (down && !self.was_down) {
            // Mouse button went down this frame.
            self.pressed = hovered;
        } else if down {
            // Mouse button is being held down.
            if let Some(target) = self.pressed.filter(|_| !self.capture) {
                if hovered != Some(target) {
                    // Cursor moved away while mouse button is held -> cancel pressed state.
                    self.pressed = None;
                }
            }
        } else {
            self.pressed = None;
        }

        self.was_down = down;
        activated
    }
}

impl Painter<'_> {
    /// Draws a filled rectangle with rounded corners using the specified layout radius.
    pub fn rounded_fill(&self, rect: Rect, color: Color, radius: f32) {
        let r = self.map(rect);
        let cr = (radius * self.scale).round().min(r.w * 0.5).min(r.h * 0.5);
        if cr <= 0. {
            self.canvas.rectangle(r.x, r.y, r.w, r.h, color);
            return;
        }

        // Top bar
        self.canvas
            .rectangle(r.x + cr, r.y, r.w - 2. * cr, cr, color);
        // Middle bar
        self.canvas
            .rectangle(r.x, r.y + cr, r.w, r.h - 2. * cr, color);
        // Bottom bar
        self.canvas
            .rectangle(r.x + cr, r.y + r.h - cr, r.w - 2. * cr, cr, color);

        let k = 0.2929 * cr;

        // Top-Left corner: center (r.x + cr, r.y + cr)
        let tl_center = vec2(r.x + cr, r.y + cr);
        self.canvas.polygon([
            (tl_center, color),
            (vec2(r.x + cr, r.y), color),
            (vec2(r.x + k, r.y + k), color),
            (vec2(r.x, r.y + cr), color),
        ]);

        // Top-Right corner: center (r.x + r.w - cr, r.y + cr)
        let tr_center = vec2(r.x + r.w - cr, r.y + cr);
        self.canvas.polygon([
            (tr_center, color),
            (vec2(r.x + r.w, r.y + cr), color),
            (vec2(r.x + r.w - k, r.y + k), color),
            (vec2(r.x + r.w - cr, r.y), color),
        ]);

        // Bottom-Right corner: center (r.x + r.w - cr, r.y + r.h - cr)
        let br_center = vec2(r.x + r.w - cr, r.y + r.h - cr);
        self.canvas.polygon([
            (br_center, color),
            (vec2(r.x + r.w - cr, r.y + r.h), color),
            (vec2(r.x + r.w - k, r.y + r.h - k), color),
            (vec2(r.x + r.w, r.y + r.h - cr), color),
        ]);

        // Bottom-Left corner: center (r.x + cr, r.y + r.h - cr)
        let bl_center = vec2(r.x + cr, r.y + r.h - cr);
        self.canvas.polygon([
            (bl_center, color),
            (vec2(r.x, r.y + r.h - cr), color),
            (vec2(r.x + k, r.y + r.h - k), color),
            (vec2(r.x + cr, r.y + r.h), color),
        ]);
    }

    /// A titled VGUI frame of `size` at the painter origin.
    pub fn frame(&self, size: Vec2, title: &str) {
        self.frame_with_close(size, title, None);
    }

    /// A titled VGUI frame of `size` with an optional close (X) button.
    pub fn frame_with_close(&self, size: Vec2, title: &str, close: Option<ButtonState>) {
        self.frame_spec(FrameSpec::new(size, title), close);
    }

    /// Renders a titled VGUI frame according to the declarative [`FrameSpec`].
    pub fn frame_spec(&self, spec: FrameSpec, close: Option<ButtonState>) {
        let rect = Rect::new(0., 0., spec.size.x, spec.size.y);
        let bg = spec.bg_color.unwrap_or(self.theme.frame_bg);
        if spec.style.rounded {
            self.rounded_fill(rect, bg, spec.style.corner_radius);
        } else {
            self.fill(rect, bg);
        }
        if spec.style.outer_3d_border {
            self.raised(rect);
        }
        if !spec.title.is_empty() {
            self.text(
                spec.title,
                vec2(TITLE_INSET_X, 8.),
                self.theme.title_text,
                true,
            );
        }
        if let Some(state) = close {
            if spec.has_close {
                let close_rect = spec
                    .close_rect()
                    .unwrap_or_else(|| close_button_rect(spec.size));
                self.close_button(close_rect, state);
            }
        }
    }

    /// The close (X) button in a frame title bar.
    pub fn close_button(&self, rect: Rect, state: ButtonState) {
        if state.pressed && state.enabled {
            self.sunken(rect);
        } else {
            self.raised(rect);
        }
        let offset = if state.pressed && state.enabled {
            1.
        } else {
            0.
        };
        let text_rect = Rect::new(rect.x + offset, rect.y - 1. + offset, rect.w, rect.h);
        let color = if state.enabled {
            self.theme.title_text
        } else {
            self.theme.text_disabled
        };
        self.text_in("×", text_rect, Align::Center, color, false);
    }

    /// A push button drawn from [`Theme::button_style`]: raised border, sunken while pressed,
    /// selection border when default, and the scheme's armed colors while hovered.
    pub fn button(&self, rect: Rect, label: &str, state: ButtonState) {
        let style = self.theme.button_style(state);
        if style.bg.a > 0. {
            self.fill(rect, style.bg);
        }
        match style.border {
            ButtonBorder::Depressed => self.sunken(rect),
            ButtonBorder::Raised => self.raised(rect),
            ButtonBorder::KeyFocus => {
                self.raised(rect);
                let outer = Rect::new(rect.x - 1., rect.y - 1., rect.w + 2., rect.h + 2.);
                self.border(
                    outer,
                    self.theme.border_selection,
                    self.theme.border_selection,
                );
            }
        }
        let offset = style.text_offset;
        let text_rect = Rect::new(rect.x + 6. + offset, rect.y + offset, rect.w - 12., rect.h);
        if state.enabled {
            self.text_in(label, text_rect, Align::Left, style.text, false);
        } else {
            self.label(label, text_rect, Align::Left, false);
        }
    }

    /// `CheckButton`: a sunken box with a check mark, then the label.
    pub fn check_box(&self, rect: Rect, label: &str, checked: bool, state: ButtonState) {
        let cy = rect.y + rect.h * 0.5;
        let bx = Rect::new(
            rect.x + 2.,
            (cy - CHECK_SIZE * 0.5).round(),
            CHECK_SIZE,
            CHECK_SIZE,
        );
        let bg_color = if state.pressed && state.enabled {
            self.theme.border_dark
        } else {
            self.theme.entry_bg
        };
        self.fill(bx, bg_color);
        self.sunken(bx);
        if checked {
            let color = if state.enabled {
                self.theme.check
            } else {
                self.theme.text_disabled
            };
            let offset = if state.pressed && state.enabled {
                1.
            } else {
                0.
            };
            self.check_mark(bx, color, offset);
        }
        let text_rect = Rect::new(rect.x + LABEL_INDENT, rect.y, rect.w - LABEL_INDENT, rect.h);
        let text_color = if !state.enabled {
            self.theme.text_disabled
        } else if state.armed || state.pressed {
            self.theme.text
        } else {
            self.theme.text_dull
        };
        if state.enabled {
            self.text_in(label, text_rect, Align::Left, text_color, false);
        } else {
            self.label(label, text_rect, Align::Left, false);
        }
    }

    /// `RadioButton`: a sunken disc with a dot when selected, then the label.
    pub fn radio(&self, rect: Rect, label: &str, selected: bool, state: ButtonState) {
        let center = vec2(rect.x + 2. + CHECK_SIZE * 0.5, rect.y + rect.h * 0.5);
        self.disc(center, CHECK_SIZE * 0.5, self.theme.border_dark);
        let bg_color = if state.pressed && state.enabled {
            self.theme.border_dark
        } else {
            self.theme.entry_bg
        };
        self.disc(center, CHECK_SIZE * 0.5 - 1., bg_color);
        if selected {
            let color = if state.enabled {
                self.theme.check
            } else {
                self.theme.text_disabled
            };
            let offset = if state.pressed && state.enabled {
                vec2(0.5, 0.5)
            } else {
                Vec2::ZERO
            };
            self.disc(center + offset, 2.5, color);
        }
        let text_rect = Rect::new(rect.x + LABEL_INDENT, rect.y, rect.w - LABEL_INDENT, rect.h);
        let text_color = if !state.enabled {
            self.theme.text_disabled
        } else if state.armed || state.pressed {
            self.theme.text
        } else {
            self.theme.text_dull
        };
        if state.enabled {
            self.text_in(label, text_rect, Align::Left, text_color, false);
        } else {
            self.label(label, text_rect, Align::Left, false);
        }
    }

    /// `Slider` with tick marks and its left/right captions below the track.
    pub fn slider(&self, rect: Rect, fraction: f32, left: &str, right: &str, enabled: bool) {
        let (start, end) = slider_track(rect);
        let track_y = rect.y + SLIDER_TRACK_Y;
        let track = Rect::new(start, track_y - 2., end - start, 4.);
        self.fill(track, self.theme.slider_track);
        self.sunken(track);
        for i in 0..=SLIDER_TICKS {
            let x = start + (end - start) * i as f32 / SLIDER_TICKS as f32;
            self.fill(
                Rect::new(x.round(), track_y + 8., 1., 4.),
                self.theme.slider_text,
            );
        }
        let nob_x = start + (end - start) * fraction.clamp(0., 1.) - NOB_WIDTH * 0.5;
        let nob = Rect::new(nob_x, track_y - 8., NOB_WIDTH, 16.);
        self.fill(nob, self.theme.slider_nob);
        self.raised(nob);
        if rect.h >= 32. {
            let caption = Rect::new(rect.x, track_y + 12., rect.w, FONT_TALL);
            let color = if enabled {
                self.theme.slider_text
            } else {
                self.theme.text_disabled
            };
            self.text_in(left, caption, Align::Left, color, false);
            self.text_in(right, caption, Align::Right, color, false);
        }
    }

    /// `TextEntry`: sunken field; `caret` draws the insertion bar after the text.
    pub fn text_entry(&self, rect: Rect, text: &str, enabled: bool, caret: bool) {
        self.fill(rect, self.theme.entry_bg);
        self.sunken(rect);
        let inner = Rect::new(rect.x + 3., rect.y, rect.w - 6., rect.h);
        let color = if enabled {
            self.theme.text
        } else {
            self.theme.text_disabled
        };
        self.text_in(text, inner, Align::Left, color, false);
        if caret {
            let x = inner.x + self.text_width(text, false) + 1.;
            self.fill(Rect::new(x, rect.y + 4., 1., rect.h - 8.), self.theme.text);
        }
    }

    /// `ComboBox`: a text entry with a drop-down arrow button on its right edge.
    pub fn combo(&self, rect: Rect, text: &str, enabled: bool, armed: bool) {
        self.text_entry(rect, text, enabled, false);
        let button = Rect::new(
            rect.x + rect.w - rect.h,
            rect.y + 1.,
            rect.h - 1.,
            rect.h - 2.,
        );
        let color = if !enabled {
            self.theme.text_disabled
        } else if armed {
            self.theme.arrow_armed
        } else {
            self.theme.arrow
        };
        self.arrow_down(
            vec2(button.x + button.w * 0.5, button.y + button.h * 0.5),
            4.,
            color,
        );
    }

    /// The open drop-down list of a combo box (`rows` visible items starting at `first`).
    pub fn menu_list(&self, rect: Rect, items: &[String], first: usize, highlight: Option<usize>) {
        self.fill(rect, self.theme.frame_bg);
        self.fill(rect, self.theme.menu_bg);
        self.raised(rect);
        let rows = (rect.h / MENU_ROW).floor() as usize;
        for (row, (index, item)) in items.iter().enumerate().skip(first).take(rows).enumerate() {
            let line = Rect::new(
                rect.x + 1.,
                rect.y + row as f32 * MENU_ROW,
                rect.w - 2.,
                MENU_ROW,
            );
            let color = if highlight == Some(index) {
                self.fill(line, self.theme.selection_bg);
                self.theme.selection_text
            } else {
                self.theme.text
            };
            let text = Rect::new(line.x + 6., line.y, line.w - 12., line.h);
            self.text_in(item, text, Align::Left, color, false);
        }
        if items.len() > rows {
            self.scroll_bar(
                Rect::new(
                    rect.x + rect.w - SCROLLBAR_WIDTH,
                    rect.y,
                    SCROLLBAR_WIDTH,
                    rect.h,
                ),
                first,
                rows,
                items.len(),
            );
        }
    }

    /// A vertical scroll bar showing `visible` of `total` rows starting at `first`.
    pub fn scroll_bar(&self, rect: Rect, first: usize, visible: usize, total: usize) {
        self.fill(rect, self.theme.scrollbar_bg);
        if total == 0 {
            return;
        }
        let tall = (rect.h * visible as f32 / total as f32).clamp(8., rect.h);
        let top =
            rect.y + (rect.h - tall) * first as f32 / (total - visible.min(total)).max(1) as f32;
        let thumb = Rect::new(rect.x + 1., top, rect.w - 2., tall);
        self.fill(thumb, self.theme.slider_nob);
        self.raised(thumb);
    }

    /// `Divider`: a sunken two-pixel line.
    pub fn divider(&self, rect: Rect) {
        self.sunken(Rect::new(rect.x, rect.y, rect.w, rect.h.max(2.)));
    }

    /// `URLButton`: link-colored, underlined text with armed and pressed states.
    pub fn url(&self, rect: Rect, label: &str, state: ButtonState) {
        if !state.enabled {
            self.label(label, rect, Align::Left, false);
            return;
        }
        let (color, offset) = if state.pressed {
            (self.theme.text, vec2(1., 1.))
        } else if state.armed {
            (self.theme.text, vec2(0., 0.))
        } else {
            (self.theme.url_text, vec2(0., 0.))
        };
        let target_rect = Rect::new(rect.x + offset.x, rect.y + offset.y, rect.w, rect.h);
        self.text_in(label, target_rect, Align::Left, color, false);
        let width = self.text_width(label, false);
        let y = target_rect.y + (target_rect.h + FONT_TALL) * 0.5 - 1.;
        self.fill(Rect::new(target_rect.x, y.round(), width, 1.), color);
    }

    /// A property-sheet tab drawn from [`Theme::tab_style`]; the active tab is open towards
    /// its page.
    pub fn tab(&self, rect: Rect, label: &str, active: bool, state: ButtonState) {
        let style = self.theme.tab_style(active, state);
        let color = style.text;
        self.fill(rect, self.theme.frame_bg);
        let r = self.map(rect);
        let t = self.scale.round().max(1.);
        self.canvas
            .rectangle(r.x, r.y, r.w, t, self.theme.border_bright);
        self.canvas
            .rectangle(r.x, r.y, t, r.h, self.theme.border_bright);
        self.canvas
            .rectangle(r.x + r.w - t, r.y, t, r.h, self.theme.border_dark);
        if !style.open_bottom {
            self.canvas
                .rectangle(r.x, r.y + r.h - t, r.w, t, self.theme.border_bright);
        }
        self.text_in(label, rect, Align::Center, color, false);
    }

    /// Draws a texture stretched over a layout rectangle.
    pub fn image(&self, rect: Rect, texture: &Texture2D) {
        let r = self.map(rect);
        draw_texture_ex(
            self.canvas,
            texture,
            r.x,
            r.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(r.w, r.h)),
                ..Default::default()
            },
        );
    }

    /// Draws a tooltip box anchored at `pointer` in layout units, keeping it strictly inside `bounds`.
    pub fn tooltip(&self, text: &str, pointer: Vec2, bounds: Rect) {
        if text.trim().is_empty() {
            return;
        }
        let max_text_width = 280.;
        let lines = wrap(text, max_text_width, |s| self.text_width(s, false));
        if lines.is_empty() {
            return;
        }
        let mut max_line_w: f32 = 0.;
        for line in &lines {
            max_line_w = max_line_w.max(self.text_width(line, false));
        }
        let pad_x = 6.;
        let pad_y = 4.;
        let tip_w = max_line_w + pad_x * 2.;
        let tip_h = lines.len() as f32 * FONT_TALL + pad_y * 2.;

        let rect = tooltip_rect(pointer, vec2(tip_w, tip_h), bounds);
        self.fill(rect, self.theme.tooltip_bg);
        self.border(rect, self.theme.tooltip_border, self.theme.tooltip_border);
        for (i, line) in lines.iter().enumerate() {
            let line_y = rect.y + pad_y + i as f32 * FONT_TALL;
            self.text(
                line,
                vec2(rect.x + pad_x, line_y),
                self.theme.tooltip_text,
                false,
            );
        }
    }

    /// A Source VGUI segmented progress bar: sunken cavity border, transparent background,
    /// and segmented fill blocks (segment wide 8, gap 4, inset 4 per Source SDK `ProgressBar.cpp`).
    pub fn progress_bar(&self, rect: Rect, fraction: f32, fg: Color, bg: Color) {
        self.fill(rect, bg);
        self.sunken(rect);

        let inset = 4.;
        let seg_w = 8.;
        let seg_gap = 4.;
        let inner_x = rect.x + inset;
        let inner_y = rect.y + inset;
        let inner_w = (rect.w - 2. * inset).max(0.);
        let inner_h = (rect.h - 2. * inset).max(0.);
        if inner_w <= 0. || inner_h <= 0. {
            return;
        }

        let total_segs = ((inner_w + seg_gap) / (seg_w + seg_gap)).floor() as usize;
        let drawn_segs = ((total_segs as f32) * fraction.clamp(0.0, 1.0)).floor() as usize;
        for i in 0..drawn_segs {
            let seg_x = inner_x + (i as f32) * (seg_w + seg_gap);
            self.fill(Rect::new(seg_x, inner_y, seg_w, inner_h), fg);
        }
    }
}

/// Calculates the placed tooltip rectangle per Source SDK positioning and screen clamping rules.
pub fn tooltip_rect(pointer: Vec2, tip_size: Vec2, bounds: Rect) -> Rect {
    let mut x = pointer.x;
    let mut y = pointer.y + 20.;
    if x + tip_size.x > bounds.x + bounds.w {
        x = pointer.x - tip_size.x;
    }
    if y + tip_size.y > bounds.y + bounds.h {
        y = pointer.y - tip_size.y - 20.;
    }
    let x = x.clamp(bounds.x, (bounds.x + bounds.w - tip_size.x).max(bounds.x));
    let y = y.clamp(bounds.y, (bounds.y + bounds.h - tip_size.y).max(bounds.y));
    Rect::new(x, y, tip_size.x, tip_size.y)
}

/// Check box / radio indicator size and the label indent after it.
pub const CHECK_SIZE: f32 = 12.;
pub const LABEL_INDENT: f32 = 20.;
/// Track centre line inside a slider control and its tick count.
pub const SLIDER_TRACK_Y: f32 = 10.;
pub const SLIDER_TICKS: usize = 10;
/// Height of a slider's interactive band (track and nob).
pub const SLIDER_HIT_TALL: f32 = 22.;
/// Drop-down menu row height and the maximum rows shown before scrolling.
pub const MENU_ROW: f32 = 20.;
pub const MENU_MAX_ROWS: usize = 12;
pub const SCROLLBAR_WIDTH: f32 = 14.;

/// Rectangle of a combo box's open list below the combo.
pub fn menu_list_rect(combo: Rect, items: usize) -> Rect {
    let rows = items.clamp(1, MENU_MAX_ROWS);
    Rect::new(combo.x, combo.y + combo.h, combo.w, rows as f32 * MENU_ROW)
}

/// What the pointer is over while a popup list is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupHit {
    /// A list entry.
    Item(usize),
    /// Inside the list but not on an entry (scroll bar, empty rows).
    List,
    /// The control that opened the list (a combo box header).
    Anchor,
    /// Anywhere else.
    Outside,
}

/// Result of a pointer gesture on an open popup list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupEvent {
    /// An entry was chosen; the list closes.
    Select(usize),
    /// The list closes without a selection (anchor or outside click).
    Close,
}

/// An open popup list (a combo box drop-down, or any future popup menu) anchored to a control.
///
/// While open it is the topmost input layer: its owner routes all pointer input here and
/// occludes the layers below ([`ButtonTracker::occlude`]). Every close path acts on the release
/// of a gesture that started on the same kind of target, so one click never closes the list
/// and lets its remainder reach, or reopen from, the control underneath: releasing on an entry
/// after pressing in the list selects it (SDK `MenuItem` activates on release), releasing on
/// the anchor after pressing it closes the list, and releasing outside after pressing outside
/// closes it without activating anything.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupList {
    pub anchor: Rect,
    pub list: Rect,
    pub items: usize,
    pub first: usize,
    pub highlight: Option<usize>,
    pressed: Option<PopupHit>,
    was_down: bool,
}

impl PopupList {
    /// Opens below `anchor` with the `selected` entry highlighted and scrolled into view.
    pub fn below(anchor: Rect, items: usize, selected: Option<usize>) -> Self {
        let mut popup = Self {
            anchor,
            list: menu_list_rect(anchor, items),
            items,
            first: 0,
            highlight: selected,
            pressed: None,
            was_down: false,
        };
        if let Some(index) = selected {
            popup.reveal(index);
        }
        popup
    }

    /// Visible rows.
    pub fn rows(&self) -> usize {
        ((self.list.h / MENU_ROW).floor() as usize).max(1)
    }

    /// Scrolls so that `index` is visible.
    pub fn reveal(&mut self, index: usize) {
        let rows = self.rows();
        if index < self.first {
            self.first = index;
        } else if index >= self.first + rows {
            self.first = index + 1 - rows;
        }
    }

    /// Scrolls by wheel steps; positive scrolls up.
    pub fn scroll(&mut self, steps: i32) {
        let max = self.items.saturating_sub(self.rows()) as i32;
        self.first = (self.first as i32 - steps).clamp(0, max) as usize;
    }

    /// Moves the highlight by `delta` entries (arrow keys).
    pub fn step(&mut self, delta: i32) {
        if self.items == 0 {
            return;
        }
        let current = self.highlight.map_or(-1, |i| i as i32);
        let next = (current + delta).clamp(0, self.items as i32 - 1) as usize;
        self.highlight = Some(next);
        self.reveal(next);
    }

    /// What lies under `point`; `None` when the pointer is not over the canvas.
    pub fn hit(&self, point: Option<Vec2>) -> Option<PopupHit> {
        let p = point?;
        if self.list.contains(p) {
            let on_scroll_bar =
                self.items > self.rows() && p.x >= self.list.x + self.list.w - SCROLLBAR_WIDTH;
            let item = (!on_scroll_bar)
                .then(|| menu_list_hit(self.list, self.first, self.items, p))
                .flatten();
            return Some(item.map_or(PopupHit::List, PopupHit::Item));
        }
        Some(if self.anchor.contains(p) {
            PopupHit::Anchor
        } else {
            PopupHit::Outside
        })
    }

    /// Feeds one frame of pointer state. Hovering an entry highlights it; see the type docs
    /// for when a gesture selects or closes.
    pub fn pointer(
        &mut self,
        point: Option<Vec2>,
        down: bool,
        click: bool,
        released: bool,
    ) -> Option<PopupEvent> {
        let hit = self.hit(point);
        if let Some(PopupHit::Item(index)) = hit {
            self.highlight = Some(index);
        }
        let mut event = None;
        if released || (self.was_down && !down) {
            event = match (self.pressed.take(), hit) {
                (Some(PopupHit::Item(_) | PopupHit::List), Some(PopupHit::Item(index))) => {
                    Some(PopupEvent::Select(index))
                }
                (Some(PopupHit::Anchor), Some(PopupHit::Anchor))
                | (Some(PopupHit::Outside), Some(PopupHit::Outside)) => Some(PopupEvent::Close),
                _ => None,
            };
        } else if click || (down && !self.was_down) {
            self.pressed = hit;
        }
        self.was_down = down;
        event
    }
}

/// Item index under a point in an open list, if any.
pub fn menu_list_hit(list: Rect, first: usize, items: usize, point: Vec2) -> Option<usize> {
    if !list.contains(point) {
        return None;
    }
    let index = first + ((point.y - list.y) / MENU_ROW).floor() as usize;
    (index < items).then_some(index)
}

/// Greedy word wrap by measured width; words longer than `width` are broken by character
/// so lines never exceed `width`.
pub fn wrap(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&candidate) > width {
                lines.push(std::mem::take(&mut line));
            }
            if line.is_empty() {
                if measure(word) <= width || width <= 0. {
                    line = word.to_owned();
                } else {
                    // Very long word exceeds max width: break by character
                    let mut chunk = String::new();
                    for ch in word.chars() {
                        let mut next = chunk.clone();
                        next.push(ch);
                        if !chunk.is_empty() && measure(&next) > width {
                            lines.push(std::mem::take(&mut chunk));
                        }
                        chunk.push(ch);
                    }
                    if !chunk.is_empty() {
                        line = chunk;
                    }
                }
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Continuous slider model: maps a value range to the track and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderRange {
    pub min: f32,
    pub max: f32,
}

impl SliderRange {
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn fraction(self, value: f32) -> f32 {
        if self.max == self.min {
            return 0.;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0., 1.)
    }

    pub fn value(self, fraction: f32) -> f32 {
        self.min + (self.max - self.min) * fraction.clamp(0., 1.)
    }

    pub fn clamp(self, value: f32) -> f32 {
        value.clamp(self.min.min(self.max), self.min.max(self.max))
    }
}

/// Slider geometry inside its control rectangle: the track spans the width minus the nob.
pub const NOB_WIDTH: f32 = 8.;

pub fn slider_track(rect: Rect) -> (f32, f32) {
    (rect.x + NOB_WIDTH * 0.5, rect.x + rect.w - NOB_WIDTH * 0.5)
}

/// Fraction of a pointer x position on a slider track.
pub fn slider_fraction_at(rect: Rect, x: f32) -> f32 {
    let (start, end) = slider_track(rect);
    if end <= start {
        return 0.;
    }
    ((x - start) / (end - start)).clamp(0., 1.)
}

/// Formats a slider entry the way the retail pages do (`" %.2f"`, shown trimmed).
pub fn format_value(value: f32) -> String {
    format!("{value:.2}")
}

/// Parses a typed entry value, rejecting non-finite input.
pub fn parse_value(text: &str) -> Option<f32> {
    text.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_value_and_fraction_round_trip_and_clamp() {
        let range = SliderRange::new(0.1, 6.0);
        assert!((range.value(range.fraction(3.0)) - 3.0).abs() < 1e-5);
        assert_eq!(range.fraction(-5.), 0.);
        assert_eq!(range.fraction(50.), 1.);
        assert_eq!(SliderRange::new(1., 1.).fraction(1.), 0.);
        let rect = Rect::new(10., 0., 108., 20.);
        assert_eq!(slider_fraction_at(rect, 0.), 0.);
        assert_eq!(slider_fraction_at(rect, 64.), 0.5);
        assert_eq!(slider_fraction_at(rect, 500.), 1.);
    }

    #[test]
    fn entry_values_format_with_two_decimals_and_reject_garbage() {
        assert_eq!(format_value(3.0), "3.00");
        assert_eq!(format_value(1.054), "1.05");
        assert_eq!(parse_value(" 2.5 "), Some(2.5));
        assert_eq!(parse_value("NaN"), None);
        assert_eq!(parse_value("abc"), None);
    }

    #[test]
    fn wrap_breaks_on_measured_width() {
        let lines = wrap("Adjust the slider until all three", 60., |s| {
            s.len() as f32 * 5.
        });
        assert_eq!(lines, ["Adjust the", "slider until", "all three"]);
        assert_eq!(wrap("", 10., |_| 0.), [""]);

        // Very long word exceeding width is broken character-by-character
        let long_word_lines = wrap("Supercalifragilistic", 30., |s| s.len() as f32 * 5.);
        assert_eq!(long_word_lines, ["Superc", "alifra", "gilist", "ic"]);
        for line in &long_word_lines {
            assert!(line.len() as f32 * 5. <= 30.);
        }

        // Localized multi-byte UTF-8 string with umlauts and symbols
        let loc_lines = wrap(
            "Urheberrecht © 1991-2001 RAD Game Tools. Überragend!",
            80.,
            |s| s.chars().count() as f32 * 5.,
        );
        for line in &loc_lines {
            assert!(line.chars().count() as f32 * 5. <= 80.);
        }

        // Explicit newlines preserved
        let multiline = wrap("First line\nSecond line is longer", 50., |s| {
            s.len() as f32 * 5.
        });
        assert_eq!(multiline, ["First line", "Second", "line is", "longer"]);

        // Zero width falls back safely
        let zero_lines = wrap("hello world", 0., |_| 10.);
        assert_eq!(zero_lines, ["hello", "world"]);
    }

    #[test]
    fn scheme_colors_resolve_names_and_keep_defaults() {
        let roots = source_assets::keyvalues::parse_resource(
            br#"Scheme { Colors { Orange "255 155 0 255" } BaseSettings { Frame.BgColor "1 2 3 4" ListPanel.SelectedBgColor Orange } }"#,
        )
        .unwrap();
        let theme = Theme::from_scheme(&roots[0]);
        assert_eq!(theme.frame_bg, Color::from_rgba(1, 2, 3, 4));
        assert_eq!(theme.selection_bg, Color::from_rgba(255, 155, 0, 255));
        assert_eq!(theme.slider_nob, Theme::default().slider_nob);
    }

    /// The desktop scheme's push button entries, with the console-only armed color.
    const BUTTON_SCHEME: &[u8] = br#"Scheme { Colors { White "255 255 255 255" Blank "0 0 0 0" }
        BaseSettings {
            Button.TextColor "White"
            Button.BgColor "Blank"
            Button.ArmedTextColor "White"
            Button.ArmedBgColor "Blank" [$WIN32]
            Button.ArmedBgColor "190 115 0 255" [$X360]
            Button.DepressedTextColor "White"
            Button.DepressedBgColor "Blank"
        } }"#;

    fn button_states() -> [(ButtonState, &'static str); 6] {
        [
            (ButtonState::active(true, false, false), "normal"),
            (ButtonState::active(true, true, false), "hover"),
            (ButtonState::active(true, true, true), "pressed over"),
            (
                ButtonState::active(true, false, true),
                "pressed, dragged off",
            ),
            (ButtonState::active(false, true, false), "disabled hover"),
            (ButtonState::active(false, true, true), "disabled pressed"),
        ]
    }

    #[test]
    fn desktop_scheme_push_buttons_have_no_visible_hover() {
        let roots = source_assets::keyvalues::parse_resource(BUTTON_SCHEME).unwrap();
        let theme = Theme::from_scheme(&roots[0]);
        assert_eq!(
            theme,
            Theme::default(),
            "defaults mirror the desktop scheme"
        );
        assert_eq!(theme.button_armed_bg, Color::from_rgba(0, 0, 0, 0));
        let normal = theme.button_style(ButtonState::new(true));
        let hover = theme.button_style(ButtonState::active(true, true, false));
        assert_eq!(
            normal, hover,
            "armed colors equal the normal colors on the desktop"
        );
    }

    #[test]
    fn button_style_resolves_hover_pressed_and_disabled_from_scheme_colors() {
        let rgba = Color::from_rgba;
        let theme = Theme {
            button_text: rgba(1, 1, 1, 255),
            button_bg: rgba(2, 2, 2, 0),
            button_armed_text: rgba(3, 3, 3, 255),
            button_armed_bg: rgba(4, 4, 4, 255),
            button_depressed_text: rgba(5, 5, 5, 255),
            button_depressed_bg: rgba(6, 6, 6, 255),
            ..Theme::default()
        };
        let normal = (ButtonBorder::Raised, theme.button_text, theme.button_bg, 0.);
        let hover = (
            ButtonBorder::Raised,
            theme.button_armed_text,
            theme.button_armed_bg,
            0.,
        );
        let pressed = (
            ButtonBorder::Depressed,
            theme.button_depressed_text,
            theme.button_depressed_bg,
            1.,
        );
        let expected = [normal, hover, pressed, pressed, normal, normal];
        for ((state, name), want) in button_states().into_iter().zip(expected) {
            let style = theme.button_style(state);
            assert_eq!(
                (style.border, style.text, style.bg, style.text_offset),
                want,
                "{name}"
            );
        }
        // The default button keeps its selection border while hovered; pressing replaces it
        // with the depressed border, and a disabled default button shows neither.
        let mut default = ButtonState::active(true, true, false);
        default.default = true;
        assert_eq!(theme.button_style(default).border, ButtonBorder::KeyFocus);
        assert_eq!(theme.button_style(default).text, theme.button_armed_text);
        default.pressed = true;
        assert_eq!(theme.button_style(default).border, ButtonBorder::Depressed);
        default.enabled = false;
        assert_eq!(theme.button_style(default).border, ButtonBorder::Raised);
    }

    #[test]
    fn tabs_ignore_hover_and_press_whether_selected_or_not() {
        let theme = Theme::default();
        for active in [false, true] {
            let normal = theme.tab_style(active, ButtonState::new(true));
            for (state, name) in button_states() {
                assert_eq!(theme.tab_style(active, state), normal, "{name}, {active}");
            }
        }
        assert_eq!(
            theme.tab_style(true, ButtonState::new(true)).text,
            theme.text
        );
        assert_eq!(
            theme.tab_style(false, ButtonState::new(true)).text,
            theme.text_dull
        );
    }

    #[test]
    fn hover_look_draws_through_the_shared_button_path() {
        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme {
            button_armed_bg: Color::from_rgba(4, 4, 4, 255),
            ..Theme::default()
        };
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };
        let rect = Rect::new(10., 10., 80., 24.);
        painter.button(rect, "", ButtonState::new(true));
        let normal = canvas.drain();
        painter.button(rect, "", ButtonState::active(true, true, false));
        let hover = canvas.drain();
        assert_eq!(hover.len(), normal.len() + 1, "armed background fill");
        painter.button(rect, "", ButtonState::active(false, true, false));
        assert_eq!(canvas.drain().len(), normal.len(), "disabled: no hover");
    }

    #[test]
    fn capturing_tracker_keeps_press_through_drag_off_and_activates_on_return() {
        let mut tracker = ButtonTracker::<usize>::capturing();
        tracker.update(Some(0), true, true, false);
        // Dragged off (nothing or another control under the pointer): still pressed, and
        // the other control cannot arm while the mouse is captured.
        assert_eq!(tracker.update(None, true, false, false), None);
        assert_eq!(tracker.pressed(), Some(0));
        assert_eq!(tracker.update(Some(1), true, false, false), None);
        assert_eq!(tracker.pressed(), Some(0));
        assert_eq!(tracker.armable(Some(1)), None);
        assert_eq!(tracker.armable(Some(0)), Some(0));
        // Back on and released over it: activates.
        tracker.update(Some(0), true, false, false);
        assert_eq!(tracker.update(Some(0), false, false, true), Some(0));
        assert_eq!(tracker.pressed(), None);
        assert_eq!(tracker.armable(Some(1)), Some(1), "capture ends on release");
        // Released while off: no activation.
        tracker.update(Some(0), true, true, false);
        tracker.update(Some(1), true, false, false);
        assert_eq!(tracker.update(Some(1), false, false, true), None);
        assert_eq!(tracker.pressed(), None);
        // A press that starts on nothing captures nothing.
        tracker.update(None, true, true, false);
        tracker.update(Some(2), true, false, false);
        assert_eq!(tracker.update(Some(2), false, false, true), None);
    }

    #[test]
    fn popup_list_closes_and_selects_only_on_release_of_its_own_gesture() {
        let anchor = Rect::new(0., 0., 100., 24.);
        let mut popup = PopupList::below(anchor, 3, Some(1));
        assert_eq!(popup.list, Rect::new(0., 24., 100., 60.));
        assert_eq!(popup.highlight, Some(1));
        let entry = |i: usize| Some(vec2(10., 24. + MENU_ROW * (i as f32 + 0.5)));
        let header = Some(vec2(10., 10.));
        let outside = Some(vec2(300., 300.));
        let gesture = |popup: &mut PopupList, press, release| {
            let mut events = vec![popup.pointer(press, true, true, false)];
            events.push(popup.pointer(press, true, false, false));
            events.push(popup.pointer(release, false, false, true));
            events
        };
        assert_eq!(popup.hit(entry(2)), Some(PopupHit::Item(2)));
        assert_eq!(popup.hit(header), Some(PopupHit::Anchor));
        assert_eq!(popup.hit(outside), Some(PopupHit::Outside));
        assert_eq!(popup.hit(None), None);
        // Entry: nothing on press or hold, selection on release; hover moves the highlight.
        let events = gesture(&mut popup, entry(2), entry(2));
        assert_eq!(events, vec![None, None, Some(PopupEvent::Select(2))]);
        assert_eq!(popup.highlight, Some(2));
        // Press on one entry, release on another: the released entry is chosen.
        let events = gesture(&mut popup, entry(0), entry(1));
        assert_eq!(events[2], Some(PopupEvent::Select(1)));
        // Header and outside: close on release only.
        assert_eq!(
            gesture(&mut popup, header, header),
            vec![None, None, Some(PopupEvent::Close)]
        );
        assert_eq!(
            gesture(&mut popup, outside, outside),
            vec![None, None, Some(PopupEvent::Close)]
        );
        // Mixed gestures do nothing: the list stays open.
        assert_eq!(gesture(&mut popup, outside, entry(0))[2], None);
        assert_eq!(gesture(&mut popup, header, outside)[2], None);
        assert_eq!(gesture(&mut popup, entry(0), header)[2], None);
        // A release without an observed press (the opening click) does nothing.
        let mut fresh = PopupList::below(anchor, 3, None);
        assert_eq!(fresh.pointer(header, false, false, true), None);
        // Keyboard stepping clamps and scrolls into view.
        fresh.step(-1);
        assert_eq!(fresh.highlight, Some(0));
        fresh.step(5);
        assert_eq!(fresh.highlight, Some(2));
    }

    #[test]
    fn occluded_tracker_never_reads_a_consumed_click_as_a_new_press() {
        let mut tracker = ButtonTracker::<usize>::capturing();
        // A layer above consumes the press and closes on it; the button is still held.
        tracker.occlude(true);
        assert_eq!(tracker.update(Some(0), true, false, false), None);
        assert_eq!(
            tracker.pressed(),
            None,
            "held after an occluded press is no press"
        );
        assert_eq!(tracker.update(Some(0), false, false, true), None);
        // reset() keeps the physical button state for the same reason.
        tracker.update(Some(0), true, true, false);
        tracker.reset();
        assert_eq!(tracker.update(Some(0), true, false, false), None);
        assert_eq!(tracker.update(Some(0), false, false, true), None);
        // The next real click works.
        tracker.update(Some(0), true, true, false);
        assert_eq!(tracker.update(Some(0), false, false, true), Some(0));
    }

    #[test]
    fn button_tracker_press_cancel_activate_state_machine() {
        let mut tracker = ButtonTracker::<usize>::new();
        assert_eq!(tracker.pressed(), None);
        assert!(!tracker.is_pressed(0));

        // 1. Mouse down on control 0 -> pressed state active, no activation yet
        let act = tracker.update(Some(0), true, true, false);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), Some(0));
        assert!(tracker.is_pressed(0));

        // 2. Mouse held while still over control 0 -> stays pressed, no activation
        let act = tracker.update(Some(0), true, false, false);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), Some(0));

        // 3. Mouse released over control 0 -> triggers activation!
        let act = tracker.update(Some(0), false, false, true);
        assert_eq!(act, Some(0));
        assert_eq!(tracker.pressed(), None);
        assert!(!tracker.is_pressed(0));

        // 4. Test cancel on move away:
        // Mouse down on control 1
        tracker.update(Some(1), true, true, false);
        assert_eq!(tracker.pressed(), Some(1));

        // Cursor moves away while mouse is held (hovered = None) -> pressed cancelled!
        let act = tracker.update(None, true, false, false);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), None);
        assert!(!tracker.is_pressed(1));

        // Cursor moves back over control 1 while mouse is still held -> stays cancelled!
        let act = tracker.update(Some(1), true, false, false);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), None);

        // Mouse released over control 1 after being cancelled -> does NOT trigger activation!
        let act = tracker.update(Some(1), false, false, true);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), None);

        // 5. Test release outside:
        tracker.update(Some(2), true, true, false);
        assert_eq!(tracker.pressed(), Some(2));
        // Cursor moves away and releases outside
        let act = tracker.update(None, false, false, true);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), None);

        // 6. Click started outside control, moving into control while held
        let act = tracker.update(None, true, true, false);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), None);
        let act = tracker.update(Some(3), true, false, false);
        assert_eq!(act, None);
        assert_eq!(tracker.pressed(), None);
        let act = tracker.update(Some(3), false, false, true);
        assert_eq!(act, None);
    }

    #[test]
    fn widget_pressed_states_drawing() {
        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        let rect = Rect::new(10., 10., 80., 24.);

        // Button normal vs pressed
        painter.button(rect, "Test", ButtonState::new(true));
        let quads_normal = canvas.drain();
        assert!(!quads_normal.is_empty());

        painter.button(rect, "Test", ButtonState::active(true, true, true));
        let quads_pressed = canvas.drain();
        assert!(!quads_pressed.is_empty());

        // CheckBox normal vs pressed
        painter.check_box(rect, "Check", false, ButtonState::new(true));
        let quads_chk_normal = canvas.drain();
        assert!(!quads_chk_normal.is_empty());

        painter.check_box(rect, "Check", true, ButtonState::active(true, true, true));
        let quads_chk_pressed = canvas.drain();
        assert!(!quads_chk_pressed.is_empty());

        // Radio normal vs pressed
        painter.radio(rect, "Radio", false, ButtonState::new(true));
        let quads_radio_normal = canvas.drain();
        assert!(!quads_radio_normal.is_empty());

        painter.radio(rect, "Radio", true, ButtonState::active(true, true, true));
        let quads_radio_pressed = canvas.drain();
        assert!(!quads_radio_pressed.is_empty());

        // URLButton normal vs pressed
        painter.url(rect, "Link", ButtonState::new(true));
        let quads_url_normal = canvas.drain();
        assert!(!quads_url_normal.is_empty());

        painter.url(rect, "Link", ButtonState::active(true, true, true));
        let quads_url_pressed = canvas.drain();
        assert!(!quads_url_pressed.is_empty());
    }

    #[test]
    fn frame_spec_and_close_button_drawing() {
        let size = vec2(300., 200.);
        let spec_with_close = FrameSpec::new(size, "Test Frame");
        assert!(spec_with_close.has_close);
        assert_eq!(
            spec_with_close.close_rect(),
            Some(Rect::new(size.x - 24., 6., 18., 16.))
        );

        let spec_no_close = spec_with_close.without_close();
        assert!(!spec_no_close.has_close);
        assert_eq!(spec_no_close.close_rect(), None);

        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        // Frame without close button
        painter.frame_with_close(size, "No Close", None);
        let quads_no_close = canvas.drain();
        assert!(!quads_no_close.is_empty());

        // Frame with close button
        painter.frame_with_close(size, "With Close", Some(ButtonState::new(true)));
        let quads_with_close = canvas.drain();
        assert!(quads_with_close.len() > quads_no_close.len());

        // Frame with close button pressed
        painter.frame_with_close(
            size,
            "With Close Pressed",
            Some(ButtonState::active(true, true, true)),
        );
        let quads_pressed = canvas.drain();
        assert_eq!(quads_pressed.len(), quads_with_close.len());
    }

    #[test]
    fn dialog_frame_style_rounded_corners_and_overflow_clipping() {
        // 1. Verify default FrameStyle enables rounding, enables overflow clipping, disables outer 3D border
        let default_style = FrameStyle::DEFAULT;
        assert!(default_style.rounded);
        assert!(default_style.clip_overflow);
        assert!(!default_style.outer_3d_border);
        assert_eq!(default_style.corner_radius, FRAME_CORNER_RADIUS);
        assert_eq!(FrameStyle::default(), default_style);

        // 2. Corner radius and bounds hit testing
        let size = vec2(200., 150.);
        // Interior point is contained
        assert!(default_style.contains_point(vec2(100., 75.), size));
        // Outside points
        assert!(!default_style.contains_point(vec2(-1., 50.), size));
        assert!(!default_style.contains_point(vec2(201., 50.), size));
        assert!(!default_style.contains_point(vec2(50., -1.), size));
        assert!(!default_style.contains_point(vec2(50., 151.), size));

        // Corner hit testing: radius is 5.0.
        // Extreme corner point (0.5, 0.5) is outside the circle (5-0.5)^2 + (5-0.5)^2 = 40.5 > 25
        assert!(!default_style.contains_point(vec2(0.5, 0.5), size));
        assert!(!default_style.contains_point(vec2(199.5, 0.5), size));
        assert!(!default_style.contains_point(vec2(0.5, 149.5), size));
        assert!(!default_style.contains_point(vec2(199.5, 149.5), size));

        // Point inside the corner arc (3.0, 3.0): (5-3)^2 + (5-3)^2 = 8 <= 25
        assert!(default_style.contains_point(vec2(3.0, 3.0), size));

        // When rounding is disabled, extreme corner (0.5, 0.5) is contained
        let square_style = FrameStyle {
            rounded: false,
            ..default_style
        };
        assert!(square_style.contains_point(vec2(0.5, 0.5), size));

        // 3. Overflow clipping
        let raw_corner = Rect::new(0., 0., 10., 10.);
        let clipped = default_style
            .clip_rect(raw_corner, size)
            .expect("clipped rect");
        assert!(clipped.x > 0.0);
        assert!(clipped.y > 0.0);

        // Completely outside rect returns None
        assert_eq!(
            default_style.clip_rect(Rect::new(-20., -20., 10., 10.), size),
            None
        );

        // 4. FrameSpec integration with FrameStyle
        let spec = FrameSpec::new(size, "Rounded Dialog");
        assert_eq!(spec.style, default_style);
        let close_rect = spec.close_rect().expect("close button rect");
        // Close button must be within the frame boundaries
        assert!(close_rect.x >= 0. && close_rect.x + close_rect.w <= size.x);
        assert!(close_rect.y >= 0. && close_rect.y + close_rect.h <= size.y);

        // 5. Canvas drawing verification:
        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        // Drawing with default FrameStyle emits rounded fill (includes corner polygon quads) and NO outer 3D border
        painter.frame_spec(spec.without_close(), None);
        let quads_rounded = canvas.drain();
        // 3 rectangular strips + 4 corner polygon quads = 7 quads
        assert_eq!(quads_rounded.len(), 7);
        let polygon_count = quads_rounded
            .iter()
            .filter(|q| q.vertices.is_some())
            .count();
        assert_eq!(polygon_count, 4);

        // In contrast, a frame with outer_3d_border: true and rounded: false emits 1 fill + 4 3D border quads = 5 quads
        let legacy_spec = spec.without_close().with_style(FrameStyle {
            rounded: false,
            outer_3d_border: true,
            ..default_style
        });
        painter.frame_spec(legacy_spec, None);
        let quads_legacy = canvas.drain();
        assert_eq!(quads_legacy.len(), 5);
        let legacy_polygons = quads_legacy.iter().filter(|q| q.vertices.is_some()).count();
        assert_eq!(legacy_polygons, 0);

        // 6. Non-dialog widgets unchanged: button and close button still draw 3D borders
        let btn_rect = Rect::new(10., 10., 80., 24.);
        painter.button(btn_rect, "OK", ButtonState::new(true));
        let btn_quads = canvas.drain();
        // A button draws 4 border quads (raised 3D border)
        assert_eq!(btn_quads.len(), 4);

        let close_btn_rect = Rect::new(50., 10., 18., 16.);
        painter.close_button(close_btn_rect, ButtonState::new(true));
        let close_quads = canvas.drain();
        // Close button draws 4 border quads (raised 3D border)
        assert_eq!(close_quads.len(), 4);
    }

    #[test]
    fn tooltip_geometry_and_screen_clamping() {
        let bounds = Rect::new(0., 0., 1280., 720.);
        let tip_size = vec2(200., 40.);

        // 1. Normal interior point: hangs right and down
        let normal = tooltip_rect(vec2(100., 100.), tip_size, bounds);
        assert_eq!(normal, Rect::new(100., 120., 200., 40.));

        // 2. Near right edge: flips to hang left
        let right = tooltip_rect(vec2(1200., 100.), tip_size, bounds);
        assert_eq!(right, Rect::new(1000., 120., 200., 40.));
        assert!(right.x + right.w <= bounds.w);

        // 3. Near bottom edge: flips to hang up
        let bottom = tooltip_rect(vec2(100., 700.), tip_size, bounds);
        assert_eq!(bottom, Rect::new(100., 640., 200., 40.));
        assert!(bottom.y + bottom.h <= bounds.h);

        // 4. Bottom-right corner: flips both left and up
        let corner = tooltip_rect(vec2(1250., 710.), tip_size, bounds);
        assert_eq!(corner, Rect::new(1050., 650., 200., 40.));
        assert!(corner.x >= bounds.x && corner.x + corner.w <= bounds.w);
        assert!(corner.y >= bounds.y && corner.y + corner.h <= bounds.h);

        // 5. Painter drawing tooltip produces canvas quads and empty text is ignored
        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };
        painter.tooltip("", vec2(100., 100.), bounds);
        assert!(canvas.drain().is_empty(), "Empty tooltip draws nothing");

        painter.tooltip("Hover tip text", vec2(100., 100.), bounds);
        let quads = canvas.drain();
        assert!(!quads.is_empty(), "Tooltip draws background and border");
    }

    #[test]
    fn progress_bar_drawing_and_segments() {
        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        // 1. Zero fraction draws background and sunken border but no segments
        let rect = Rect::new(20., 64., 340., 24.);
        let fg = Color::new(1., 1., 1., 1.);
        let bg = Color::new(0., 0., 0., 1.);
        painter.progress_bar(rect, 0.0, fg, bg);
        let quads_0 = canvas.drain();
        assert!(!quads_0.is_empty(), "Draws cavity and border");

        // 2. 50% fraction draws about half the segments
        painter.progress_bar(rect, 0.5, fg, bg);
        let quads_half = canvas.drain();
        assert!(quads_half.len() > quads_0.len(), "50% draws segment blocks");

        // 3. 100% fraction draws all segments
        painter.progress_bar(rect, 1.0, fg, bg);
        let quads_full = canvas.drain();
        assert!(
            quads_full.len() > quads_half.len(),
            "100% draws all segment blocks"
        );
    }

    #[test]
    fn check_box_checked_draws_white_checkmark_not_plain_square() {
        let canvas = Canvas::default();
        let fonts = Fonts::default();
        let theme = Theme::default();
        let painter = Painter {
            canvas: &canvas,
            fonts: &fonts,
            theme: &theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };

        let rect = Rect::new(10., 10., 120., 24.);

        // Unchecked: emits bg + 4 border quads
        painter.check_box(rect, "Test", false, ButtonState::new(true));
        let quads_unchecked = canvas.drain();
        assert_eq!(quads_unchecked.len(), 5);

        // Checked: emits bg + 4 border quads + 7 checkmark quads
        painter.check_box(rect, "Test", true, ButtonState::new(true));
        let quads_checked = canvas.drain();
        assert_eq!(quads_checked.len(), 12);

        // Checkmark quads should have theme.check color (white)
        let check_quads = &quads_checked[5..12];
        for quad in check_quads {
            assert_eq!(quad.color, theme.check);
            assert_eq!(quad.destination.w, 1.);
            assert_eq!(quad.destination.h, 2.);
        }

        // Verify shape: X coordinates span 15..=21 (bx.x is 12., columns are dx=3..=9)
        // Y coordinates descend to apex at dx=5 (bx.y is 16., dy=7 -> y=23) and ascend to dy=3 (y=19)
        let xs: Vec<f32> = check_quads.iter().map(|q| q.destination.x).collect();
        let ys: Vec<f32> = check_quads.iter().map(|q| q.destination.y).collect();
        assert_eq!(xs, vec![15., 16., 17., 18., 19., 20., 21.]);
        assert_eq!(ys, vec![21., 22., 23., 22., 21., 20., 19.]);

        // When pressed, checkmark should be shifted by 1px offset
        painter.check_box(rect, "Test", true, ButtonState::active(true, true, true));
        let quads_pressed = canvas.drain();
        let check_pressed = &quads_pressed[5..12];
        let xs_pressed: Vec<f32> = check_pressed.iter().map(|q| q.destination.x).collect();
        let ys_pressed: Vec<f32> = check_pressed.iter().map(|q| q.destination.y).collect();
        assert_eq!(xs_pressed, vec![16., 17., 18., 19., 20., 21., 22.]);
        assert_eq!(ys_pressed, vec![22., 23., 24., 23., 22., 21., 20.]);
    }

    #[test]
    fn scale_computation_for_scale_factors_and_window_sizes() {
        // Scale factor 1.0 (standard DPI)
        assert_eq!(Painter::scale_for_window(480., 1.0), 1.0);
        assert_eq!(Painter::scale_for_window(720., 1.0), 1.0);
        assert_eq!(Painter::scale_for_window(800., 1.0), 800. / 720.);
        assert_eq!(Painter::scale_for_window(1080., 1.0), 1080. / 720.);
        assert_eq!(Painter::scale_for_window(1440., 1.0), 1440. / 720.);
        assert_eq!(Painter::scale_for_window(2160., 1.0), 2160. / 720.);

        // Scale factor 2.0 (High-DPI / Retina)
        assert_eq!(Painter::scale_for_window(480., 2.0), 960. / 720.);
        assert_eq!(Painter::scale_for_window(720., 2.0), 1440. / 720.);
        assert_eq!(Painter::scale_for_window(800., 2.0), 1600. / 720.);
        assert_eq!(Painter::scale_for_window(1080., 2.0), 2160. / 720.);
        assert_eq!(Painter::scale_for_window(1440., 2.0), 2880. / 720.);
        assert_eq!(Painter::scale_for_window(2160., 2.0), 4320. / 720.);

        // Regression assertion: 1280x800 layout scale is unchanged
        let scale_800_1x = Painter::scale_for_window(800., 1.0);
        assert_eq!(scale_800_1x, Painter::scale_for(800.));
        assert_eq!(scale_800_1x, 800. / 720.);
    }
}
