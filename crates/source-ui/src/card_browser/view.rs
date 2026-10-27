//! Generic card rendering over caller-provided content and geometry.
use super::{CardBrowser, Target};
use crate::{
    canvas::{draw_texture_ex, DrawTextureParams, Rect, WHITE},
    widgets::{Align, ButtonState, FrameSpec, Painter},
};
use glam::{vec2, Vec2};

impl CardBrowser {
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
            FrameSpec::new(frame.size, &self.spec.title),
            Some(self.state(Target::Close)),
        );
        for &rect in &self.spec.layout.dividers {
            let wide = rect.w.min((frame.size.x - 2. * rect.x).max(0.));
            p.sunken(Rect::new(rect.x, rect.y, wide, rect.h));
        }
        for (target, rect) in self.targets() {
            match target {
                Target::Card(index) => self.draw_card(&p, index, rect),
                Target::Close => {}
                button => {
                    let label = match button {
                        Target::Previous => &self.spec.labels[0],
                        Target::Next => &self.spec.labels[1],
                        Target::Accept => &self.spec.labels[2],
                        _ => &self.spec.labels[3],
                    };
                    p.button(rect, label, self.state(target));
                }
            }
            if self.focus_visible && self.focus == Some(target) {
                p.focus_ring(rect);
            }
        }
    }

    fn panel_control(rect: Rect, panel: Rect) -> Rect {
        Rect::new(panel.x + rect.x, panel.y + rect.y, rect.w, rect.h)
    }

    fn draw_card(&self, p: &Painter<'_>, index: usize, panel: Rect) {
        let card = &self.spec.cards[index];
        let border = Self::panel_control(self.spec.layout.card.border, panel);
        let selected = index == self.selected;
        let color = if selected {
            self.spec.selection.unwrap_or(p.theme.selection_bg)
        } else {
            self.spec.fill.unwrap_or(p.theme.frame_bg)
        };
        p.fill(border, color);
        let image = Self::panel_control(self.spec.layout.card.image, panel);
        if let Some(texture) = &card.image {
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
        let heading = Self::panel_control(self.spec.layout.card.heading, panel);
        let title = Self::panel_control(self.spec.layout.card.title, panel);
        p.text_in(
            &card.heading,
            heading,
            Align::Left,
            self.spec.text.unwrap_or(p.theme.text),
            true,
        );
        let title_text = self
            .display_titles
            .get(index)
            .map_or(card.title.as_str(), String::as_str);
        p.text_in(
            title_text,
            title,
            Align::Left,
            self.spec.text.unwrap_or(p.theme.text),
            true,
        );
        if self.state(Target::Card(index)).pressed {
            p.border(image, p.theme.border_dark, p.theme.border_bright);
        }
    }
}
