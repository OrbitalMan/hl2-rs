//! VGUI-style widget drawing and the small pieces of widget logic shared by the menus and dialogs.
//!
//! Colors come from the installed `SourceSchemeBase.res` (`BaseSettings` names resolved through
//! `Colors`); the defaults below are only used when the scheme cannot be read. Geometry is in
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
        }
    }
}

impl Theme {
    /// Reads the scheme colors, keeping defaults for anything missing.
    pub fn from_scheme(root: &Entry) -> Self {
        let mut theme = Self::default();
        let fields: [(&str, &mut Color); 23] = [
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
        ];
        for (name, slot) in fields {
            if let Some(color) = scheme_color(root, name) {
                *slot = color;
            }
        }
        theme
    }
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

    /// Text width in layout units.
    pub fn text_width(&self, text: &str, bold: bool) -> f32 {
        self.font(bold)
            .map_or(text.chars().count() as f32 * 7., |f| {
                f.width_cell(text, FONT_TALL)
            })
    }

    /// Draws text with its cell top-left at a layout position.
    pub fn text(&self, text: &str, at: Vec2, color: Color, bold: bool) {
        if let Some(font) = self.font(bold) {
            let p = self.origin + at * self.scale;
            font.draw_cell(text, p.round(), FONT_TALL * self.scale, color);
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

    /// Word-wrapped text from the top-left of `rect` (north-west labels).
    pub fn wrapped(&self, text: &str, rect: Rect, color: Color) {
        let mut y = rect.y;
        for line in wrap(text, rect.w, |s| self.text_width(s, false)) {
            if y + FONT_TALL > rect.y + rect.h + FONT_TALL * 0.5 {
                break;
            }
            self.text(&line, vec2(rect.x, y), color, false);
            y += FONT_TALL;
        }
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
}

impl<'a> FrameSpec<'a> {
    pub const fn new(size: Vec2, title: &'a str) -> Self {
        Self {
            size,
            title,
            has_close: true,
        }
    }

    pub const fn without_close(mut self) -> Self {
        self.has_close = false;
        self
    }

    pub fn close_rect(&self) -> Option<Rect> {
        self.has_close.then(|| close_button_rect(self.size))
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
}

/// Tracks pointer button press, hold, cancellation and release activation for interactive controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonTracker<T: Copy + PartialEq> {
    pressed: Option<T>,
    was_down: bool,
}

impl<T: Copy + PartialEq> ButtonTracker<T> {
    pub const fn new() -> Self {
        Self {
            pressed: None,
            was_down: false,
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

    /// Resets the tracker.
    pub fn reset(&mut self) {
        self.pressed = None;
        self.was_down = false;
    }

    /// Updates the state machine for the current frame.
    ///
    /// - `hovered`: The interactive control under the cursor, if any.
    /// - `down`: Whether the mouse button is down (`click` or `held`).
    /// - `click`: Whether the mouse button went down this frame.
    /// - `released`: Whether the mouse button was released this frame.
    ///
    /// Returns `Some(T)` only on release while the cursor is still over the control.
    /// If the cursor moves away while the mouse button is held, the pressed state is
    /// cancelled and subsequent release does not trigger the action.
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
            if let Some(target) = self.pressed {
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
    /// A titled VGUI frame of `size` at the painter origin.
    pub fn frame(&self, size: Vec2, title: &str) {
        self.frame_with_close(size, title, None);
    }

    /// A titled VGUI frame of `size` with an optional close (X) button.
    pub fn frame_with_close(&self, size: Vec2, title: &str, close: Option<ButtonState>) {
        let rect = Rect::new(0., 0., size.x, size.y);
        self.fill(rect, self.theme.frame_bg);
        self.raised(rect);
        self.text(title, vec2(TITLE_INSET_X, 8.), self.theme.title_text, true);
        if let Some(state) = close {
            self.close_button(close_button_rect(size), state);
        }
    }

    /// The close (X) button in a frame title bar.
    pub fn close_button(&self, rect: Rect, state: ButtonState) {
        if state.pressed && state.enabled {
            self.sunken(rect);
        } else {
            self.raised(rect);
        }
        let offset = if state.pressed && state.enabled { 1. } else { 0. };
        let text_rect = Rect::new(rect.x + offset, rect.y - 1. + offset, rect.w, rect.h);
        let color = if state.enabled {
            self.theme.title_text
        } else {
            self.theme.text_disabled
        };
        self.text_in("×", text_rect, Align::Center, color, false);
    }

    /// A push button: raised border, sunken while pressed, selection border when default.
    pub fn button(&self, rect: Rect, label: &str, state: ButtonState) {
        if state.pressed && state.enabled {
            self.sunken(rect);
        } else {
            self.raised(rect);
        }
        if state.default && state.enabled {
            let outer = Rect::new(rect.x - 1., rect.y - 1., rect.w + 2., rect.h + 2.);
            self.border(
                outer,
                self.theme.border_selection,
                self.theme.border_selection,
            );
        }
        let offset = if state.pressed && state.enabled { 1. } else { 0. };
        let text_rect = Rect::new(rect.x + 6. + offset, rect.y + offset, rect.w - 12., rect.h);
        if state.enabled {
            // Button.TextColor and Button.ArmedTextColor are both White in the scheme.
            self.text_in(label, text_rect, Align::Left, self.theme.text, false);
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
            let offset = if state.pressed && state.enabled { 1. } else { 0. };
            self.fill(
                Rect::new(bx.x + 3. + offset, bx.y + 3. + offset, bx.w - 6., bx.h - 6.),
                color,
            );
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

    /// A property-sheet tab; the active tab is taller and open towards its page.
    pub fn tab(&self, rect: Rect, label: &str, active: bool) {
        let color = if active {
            self.theme.text
        } else {
            self.theme.text_dull
        };
        self.fill(rect, self.theme.frame_bg);
        let r = self.map(rect);
        let t = self.scale.round().max(1.);
        self.canvas
            .rectangle(r.x, r.y, r.w, t, self.theme.border_bright);
        self.canvas
            .rectangle(r.x, r.y, t, r.h, self.theme.border_bright);
        self.canvas
            .rectangle(r.x + r.w - t, r.y, t, r.h, self.theme.border_dark);
        if !active {
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
            self.text(line, vec2(rect.x + pad_x, line_y), self.theme.tooltip_text, false);
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

/// Item index under a point in an open list, if any.
pub fn menu_list_hit(list: Rect, first: usize, items: usize, point: Vec2) -> Option<usize> {
    if !list.contains(point) {
        return None;
    }
    let index = first + ((point.y - list.y) / MENU_ROW).floor() as usize;
    (index < items).then_some(index)
}

/// Greedy word wrap by measured width; long words stay on their own line.
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
                lines.push(std::mem::replace(&mut line, word.to_owned()));
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
}
