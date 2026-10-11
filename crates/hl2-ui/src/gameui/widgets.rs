//! VGUI-style widget drawing and the small pieces of widget logic shared by the menus and dialogs.
//!
//! Colors come from the installed `SourceSchemeBase.res` (`BaseSettings` names resolved through
//! `Colors`); the defaults below are only used when the scheme cannot be read. Geometry is in
//! VGUI layout pixels; a [`Painter`] maps it to canvas pixels with an origin and a uniform scale.
//! The scale is `max(1, canvas_height / 720)`: retail draws these non-proportional dialogs at
//! 1:1 screen pixels, which becomes unreadably small on high-resolution canvases (documented
//! deviation).

use crate::canvas::{Canvas, Color, Rect};
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
        }
    }
}

impl Theme {
    /// Reads the scheme colors, keeping defaults for anything missing.
    pub fn from_scheme(root: &Entry) -> Self {
        let mut theme = Self::default();
        let fields: [(&str, &mut Color); 21] = [
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

/// Button interaction state for drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub armed: bool,
    pub pressed: bool,
    pub enabled: bool,
    /// The dialog's default button (drawn with the selection border).
    pub default: bool,
}

impl Painter<'_> {
    /// A titled VGUI frame of `size` at the painter origin.
    pub fn frame(&self, size: Vec2, title: &str) {
        let rect = Rect::new(0., 0., size.x, size.y);
        self.fill(rect, self.theme.frame_bg);
        self.raised(rect);
        self.text(title, vec2(TITLE_INSET_X, 8.), self.theme.title_text, true);
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
        let text_rect = Rect::new(rect.x + 6., rect.y, rect.w - 12., rect.h);
        if state.enabled {
            // Button.TextColor and Button.ArmedTextColor are both White in the scheme.
            self.text_in(label, text_rect, Align::Left, self.theme.text, false);
        } else {
            self.label(label, text_rect, Align::Left, false);
        }
    }
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
}
