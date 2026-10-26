//! Chapter cards use installed control geometry, material thumbnails and cached localized text.
use super::{Dialog, Target};
use glam::{vec2, Vec2};
use source_ui::{
    canvas::{draw_texture_ex, DrawTextureParams, Rect, WHITE},
    widgets::{Align, ButtonState, FrameSpec, Painter},
};

impl Dialog {
    fn state(&self, target: Target) -> ButtonState {
        ButtonState::active(
            true,
            self.tracker.armable(self.hover) == Some(target),
            self.tracker.is_pressed(target)
                || self.keys.pressed() == Some(target)
                || target == Target::Close && self.escape_held,
        )
    }

    pub fn draw(&self, painter: &Painter<'_>, canvas: Vec2) {
        let frame = self.frame(canvas);
        let origin = self.placement.place(&frame).origin;
        let p = Painter {
            origin,
            scale: frame.scale,
            ..*painter
        };
        p.frame_spec(
            FrameSpec::new(frame.size, &self.title),
            Some(self.state(Target::Close)),
        );
        for name in ["divider", "Div"] {
            if let Some(c) = self.catalog.dialog.get(name) {
                let (x, y, w, h) = c.rect((frame.size.x, frame.size.y));
                // Some installed single-chapter layouts retain the wide dialog's divider.
                // Size decorative rules to the available interior using the authored inset.
                let wide = w.min((frame.size.x - 2. * x).max(0.));
                p.sunken(Rect::new(x, y, wide, h));
            }
        }
        for (target, rect) in self.targets() {
            match target {
                Target::Chapter(index) => self.draw_chapter(&p, index, rect),
                Target::Close => {}
                button => {
                    let label = match button {
                        Target::Previous => &self.labels[0],
                        Target::Next => &self.labels[1],
                        Target::Play => &self.labels[2],
                        _ => &self.labels[3],
                    };
                    p.button(rect, label, self.state(target));
                }
            }
            if self.focus_visible && self.focus == Some(target) {
                p.focus_ring(rect);
            }
        }
    }

    fn panel_control(&self, name: &str, panel: Rect, fallback: Rect) -> Rect {
        let r = self.catalog.panel.get(name).map_or(fallback, |c| {
            let (x, y, w, h) = c.rect((panel.w, panel.h));
            Rect::new(x, y, w, h)
        });
        Rect::new(panel.x + r.x, panel.y + r.y, r.w, r.h)
    }

    fn draw_chapter(&self, p: &Painter<'_>, index: usize, panel: Rect) {
        let entry = &self.catalog.entries[index];
        let border = self.panel_control("LevelPicBorder", panel, Rect::new(0., 40., 168., 106.));
        let selected = index == self.selected;
        let color = if selected {
            self.catalog.selection.unwrap_or(p.theme.selection_bg)
        } else {
            self.catalog.fill.unwrap_or(p.theme.frame_bg)
        };
        p.fill(border, color);
        let image = self.panel_control("LevelPic", panel, Rect::new(10., 50., 152., 86.));
        if let Some(Some(texture)) = self.textures.get(index) {
            // Retail ImagePanel does not scale these padded, power-of-two textures:
            // the panel clips the top-left image region before proportional UI scaling.
            let source = Rect::new(
                0.,
                0.,
                image.w.min(texture.width()),
                image.h.min(texture.height()),
            );
            let destination = p.map(Rect::new(image.x, image.y, source.w, source.h));
            draw_texture_ex(
                p.canvas,
                texture,
                destination.x,
                destination.y,
                WHITE,
                DrawTextureParams {
                    source: Some(source),
                    dest_size: Some(vec2(destination.w, destination.h)),
                },
            );
        }
        let heading = self.panel_control("ChapterLabel", panel, Rect::new(0., 4., 172., 20.));
        let title = self.panel_control("ChapterNameLabel", panel, Rect::new(0., 20., 172., 20.));
        p.text_in(
            &entry.heading,
            heading,
            Align::Left,
            self.catalog.text.unwrap_or(p.theme.text),
            true,
        );
        let title_text = self
            .display_titles
            .get(index)
            .map_or(entry.title.as_str(), String::as_str);
        p.text_in(
            title_text,
            title,
            Align::Left,
            self.catalog.text.unwrap_or(p.theme.text),
            true,
        );
        if self.state(Target::Chapter(index)).pressed {
            p.border(image, p.theme.border_dark, p.theme.border_bright);
        }
    }
}
