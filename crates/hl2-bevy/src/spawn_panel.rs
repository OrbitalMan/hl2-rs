//! F4 NPC spawn overlay drawing (DESIGN 14w step 4): a two-column panel on the HUD
//! canvas (classes | weapons and options) in the project's ProggyClean developer font
//! (hl2-ui/assets/fonts, MIT). Glyphs are rasterized once at setup; frame systems only
//! queue quads. The panel content comes from `hl2_simulation::ai::spawn::Overlay`.
use hl2_simulation::ai::spawn::{Panel, PanelRow};
use hl2_ui::canvas::{Canvas, Color, FilterMode, Texture2D, draw_texture};

struct Glyph {
    texture: Texture2D,
    x: f32,
    y: f32,
}
/// Printable ASCII of one monospace font size.
pub struct PanelFont {
    glyphs: Vec<Option<Glyph>>,
    advance: f32,
    line: f32,
    ascent: f32,
}
impl PanelFont {
    pub fn load(px: f32) -> Option<Self> {
        let bytes: &[u8] = include_bytes!("../../hl2-ui/assets/fonts/ProggyClean.ttf");
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
        let lines = font.horizontal_line_metrics(px)?;
        let glyphs = (32u8..127)
            .map(|c| {
                let (metrics, coverage) = font.rasterize(char::from(c), px);
                if metrics.width == 0 || metrics.height == 0 {
                    return None;
                }
                let rgba: Vec<u8> = coverage.iter().flat_map(|a| [255, 255, 255, *a]).collect();
                let texture =
                    Texture2D::from_rgba8(metrics.width as u16, metrics.height as u16, &rgba);
                texture.set_filter(FilterMode::Nearest);
                Some(Glyph {
                    texture,
                    x: metrics.xmin as f32,
                    y: -(metrics.ymin as f32 + metrics.height as f32),
                })
            })
            .collect();
        Some(Self {
            glyphs,
            advance: font.metrics('M', px).advance_width.round(),
            line: lines.new_line_size.ceil(),
            ascent: lines.ascent.round(),
        })
    }
    pub fn text(&self, canvas: &Canvas, text: &str, x: f32, top: f32, color: Color) {
        let baseline = top + self.ascent;
        for (i, c) in text.chars().enumerate() {
            let glyph = u8::try_from(c)
                .ok()
                .and_then(|c| c.checked_sub(32))
                .and_then(|i| self.glyphs.get(usize::from(i)))
                .and_then(Option::as_ref);
            if let Some(g) = glyph {
                draw_texture(
                    canvas,
                    &g.texture,
                    (x + i as f32 * self.advance + g.x).round(),
                    (baseline + g.y).round(),
                    color,
                );
            }
        }
    }
    /// Draw the overlay panel at the left of the screen below the F1 bar.
    pub fn draw(&self, canvas: &Canvas, panel: &Panel, message: Option<&str>) {
        const LEFT: f32 = 20.;
        const TOP: f32 = 118.;
        let pad = 10.;
        let row = (self.line * 1.15).round();
        let class_chars = panel
            .classes
            .iter()
            .map(|r| r.text.len() + 2)
            .max()
            .unwrap_or(0)
            .max(24);
        let option_chars = panel
            .options
            .iter()
            .map(|r| r.text.len() + 2)
            .max()
            .unwrap_or(0)
            .max(30);
        let column = class_chars as f32 * self.advance + 2. * pad;
        let options_width = option_chars as f32 * self.advance + 2. * pad;
        let title_width = panel.title.len() as f32 * self.advance + 2. * pad;
        let message_width = message.map_or(0., |m| (m.len() + 6) as f32 * self.advance + 2. * pad);
        let width = (column + options_width).max(title_width).max(message_width);
        let rows = panel.classes.len().max(panel.options.len()) + 1;
        let footer = usize::from(message.is_some());
        let height = (rows + 1 + footer) as f32 * row + 2. * pad;
        canvas.rectangle(
            LEFT,
            TOP,
            width,
            height,
            Color::new(0.025, 0.045, 0.07, 0.88),
        );
        let title = Color::from_rgba(240, 180, 90, 255);
        self.text(canvas, &panel.title, LEFT + pad, TOP + pad, title);
        let header = TOP + pad + row;
        let classes_focused = panel.classes.iter().any(|r| r.cursor);
        let heading = |focused: bool| {
            if focused {
                Color::new(1., 1., 1., 1.)
            } else {
                Color::new(0.55, 0.55, 0.55, 1.)
            }
        };
        self.text(
            canvas,
            "NPC class",
            LEFT + pad,
            header,
            heading(classes_focused),
        );
        self.text(
            canvas,
            "weapon / options",
            LEFT + column + pad,
            header,
            heading(!classes_focused),
        );
        let draw_rows = |rows: &[PanelRow], x: f32, w: f32| {
            for (i, r) in rows.iter().enumerate() {
                let top = header + (i + 1) as f32 * row;
                if r.cursor {
                    canvas.rectangle(x, top - 2., w, row, Color::new(0.85, 0.55, 0.15, 0.45));
                }
                let color = if r.selected || r.cursor {
                    Color::from_rgba(255, 238, 160, 255)
                } else {
                    Color::new(0.78, 0.78, 0.78, 1.)
                };
                let mark = if r.selected { "> " } else { "  " };
                self.text(canvas, &format!("{mark}{}", r.text), x + pad, top, color);
            }
        };
        draw_rows(&panel.classes, LEFT, column);
        draw_rows(&panel.options, LEFT + column, options_width);
        if let Some(message) = message {
            let top = header + rows as f32 * row;
            self.text(
                canvas,
                &format!("last: {message}"),
                LEFT + pad,
                top,
                Color::new(0.78, 0.78, 0.78, 1.),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_developer_font_rasterizes_printable_ascii_at_setup() {
        let font = super::PanelFont::load(18.).expect("ProggyClean");
        assert!(font.advance > 0. && font.line > font.ascent && font.ascent > 0.);
        // Space has no coverage; letters do.
        assert!(font.glyphs[0].is_none());
        assert!(font.glyphs[usize::from(b'A' - 32)].is_some());
    }
}
