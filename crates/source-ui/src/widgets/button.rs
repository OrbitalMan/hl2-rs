//! Shared push-button padding and painting.
use super::{Align, ButtonBorder, ButtonState, Painter, Rect};

/// Horizontal inset shared by push-button measurement and painting.
pub const BUTTON_TEXT_INSET: f32 = 6.;

impl Painter<'_> {
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
        let text_rect = Rect::new(
            rect.x + BUTTON_TEXT_INSET + offset,
            rect.y + offset,
            rect.w - 2. * BUTTON_TEXT_INSET,
            rect.h,
        );
        if state.enabled {
            self.text_in(label, text_rect, Align::Left, style.text, false);
        } else {
            self.label(label, text_rect, Align::Left, false);
        }
    }
}
