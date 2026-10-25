//! Installed HUD font ranges and rendering.
use super::number;
use glam::{vec2, Vec2};
use source_assets::keyvalues::Entry;
use source_ui::{canvas::Color, font::FontFace};

pub(super) struct FontRange {
    pub(super) minimum: f32,
    pub(super) maximum: f32,
    pub(super) tall: f32,
    pub(super) proportional: bool,
    pub(super) blur: f32,
    pub(super) scanlines: f32,
    #[cfg(windows)]
    pub(super) antialias: bool,
}
pub(super) struct HudFont {
    pub(super) face: FontFace,
    pub(super) ranges: Vec<FontRange>,
}
#[cfg(windows)]
impl FontRange {
    pub(super) fn monochrome_crosshair(&self) -> bool {
        !self.antialias && !self.proportional && self.blur == 0. && self.scanlines == 0.
    }
}
impl HudFont {
    pub(super) fn read(fonts: &Entry, name: &str, face: FontFace, fallback: f32) -> Self {
        let ranges = fonts
            .get(name)
            .map(Entry::children)
            .unwrap_or_default()
            .iter()
            .map(|entry| {
                let resolution = entry.get("yres").and_then(Entry::text).and_then(|s| {
                    let values = s
                        .split_whitespace()
                        .map(str::parse::<f32>)
                        .collect::<std::result::Result<Vec<_>, _>>()
                        .ok()?;
                    (values.len() == 2).then_some((values[0], values[1]))
                });
                FontRange {
                    minimum: resolution.map_or(0., |r| r.0),
                    maximum: resolution.map_or(f32::MAX, |r| r.1),
                    tall: number(entry, "tall", fallback),
                    proportional: resolution.is_none(),
                    blur: number(entry, "blur", 0.),
                    scanlines: number(entry, "scanlines", 0.),
                    #[cfg(windows)]
                    antialias: number(entry, "antialias", 1.) != 0.,
                }
            })
            .collect();
        Self { face, ranges }
    }
    pub(super) fn size(&self) -> f32 {
        let height = self.design_height();
        let range = self
            .ranges
            .iter()
            .find(|r| height >= r.minimum && height <= r.maximum);
        range.map_or(12., |r| {
            r.tall * if r.proportional { height / 480. } else { 1. }
        })
    }
    fn design_height(&self) -> f32 {
        crate::proportional::design_height(vec2(
            self.face.canvas.width(),
            self.face.canvas.height(),
        ))
    }
    pub(super) fn effects(&self) -> (u16, u16) {
        let height = self.design_height();
        let range = self
            .ranges
            .iter()
            .find(|r| height >= r.minimum && height <= r.maximum);
        range.map_or((0, 0), |r| {
            let scale = if r.proportional { height / 480. } else { 1. };
            (
                (r.blur * scale).round() as u16,
                (r.scanlines * scale).round() as u16,
            )
        })
    }
    pub(super) fn draw(&self, value: &str, x: f32, y: f32, color: Color) {
        let (blur, scanlines) = self.effects();
        self.face
            .draw(value, x, y, self.size(), (blur, scanlines), color);
    }
    pub(super) fn width(&self, value: &str) -> f32 {
        self.face.width(value, self.size())
    }
    pub(super) fn draw_cropped(&self, value: &str, origin: Vec2, rows: Vec2, color: Color) {
        self.face
            .draw_cropped(value, origin, self.size(), self.effects(), color, rows);
    }
}

/// Fixed HUD labels and icons, rasterized on the loading worker. A resize scales the
/// prepared glyph quads until the next map load; drawing never rasterizes or builds temporary glyph lists.
pub(super) struct PreparedText {
    font: HudFont,
    ranges: Vec<(f32, Vec<Vec<source_ui::canvas::Quad>>)>,
}
impl PreparedText {
    pub(super) fn new(font: HudFont, values: &[&str]) -> Self {
        let canvas = source_ui::canvas::Canvas::default();
        let mut face = font.face.clone();
        face.canvas = canvas.clone();
        let height = font.design_height();
        let mut ranges = Vec::new();
        for range in &font.ranges {
            let scale = if range.proportional {
                height / 480.
            } else {
                1.
            };
            let size = (range.tall * scale).round().clamp(1., 1024.);
            let effects = (
                (range.blur * scale).round() as u16,
                (range.scanlines * scale).round() as u16,
            );
            let runs = values
                .iter()
                .map(|value| {
                    face.draw(value, 0., 0., size, effects, source_ui::canvas::WHITE);
                    canvas.drain()
                })
                .collect();
            ranges.push((size, runs));
        }
        if ranges.is_empty() {
            let runs = values
                .iter()
                .map(|value| {
                    face.draw(value, 0., 0., 12., (0, 0), source_ui::canvas::WHITE);
                    canvas.drain()
                })
                .collect();
            ranges.push((12., runs));
        }
        Self { font, ranges }
    }
    pub(super) fn draw(&self, index: usize, origin: Vec2, color: Color) {
        let height = self.font.design_height();
        let range = self
            .font
            .ranges
            .iter()
            .position(|r| height >= r.minimum && height <= r.maximum)
            .unwrap_or(0);
        let (size, runs) = &self.ranges[range];
        let scale = self.font.size().round().clamp(1., 1024.) / size;
        for quad in &runs[index] {
            if let Some(texture) = &quad.texture {
                source_ui::canvas::draw_texture_ex(
                    &self.font.face.canvas,
                    texture,
                    origin.x + quad.destination.x * scale,
                    origin.y + quad.destination.y * scale,
                    color,
                    source_ui::canvas::DrawTextureParams {
                        source: Some(quad.source),
                        dest_size: Some(quad.destination.size() * scale),
                    },
                );
            }
        }
    }
}
