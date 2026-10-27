//! GameMenu row rendering; content and style remain supplied by owned resources.
use super::{draw_cell_text_outlined, Console, Mode};
use crate::gameui::menu::MenuGeometry;
use glam::{vec2, Vec2};
use source_ui::{canvas::Color, font::TextOutline, windows::Layer};

impl Console {
    pub(super) fn draw_menu_buttons(
        &self,
        shown: Mode,
        geometry: MenuGeometry,
        alpha_mult: f32,
        base_focus: f32,
    ) {
        let menu = if shown == Mode::MainMenu {
            &self.main_menu
        } else {
            &self.pause_menu
        };
        // With the keyboard elsewhere (a window has the focus), a row is armed only under
        // the pointer or while pressed.
        let menu_keyboard = self.shell.windows.focus() == Layer::Base;
        let modal_open = self.modal_dialog_open();
        for (i, entry) in menu.entries().iter().enumerate() {
            if entry.spacer {
                continue;
            }
            let is_pressed = !modal_open && self.menu_tracker.is_pressed(i);
            let selected = if menu_keyboard {
                menu.selection == Some(i)
            } else {
                self.last_menu_hover == Some(i)
            };
            let armed = !modal_open && (is_pressed || selected);
            let color = if !entry.enabled {
                self.theme.text_disabled
            } else if armed {
                self.style.menu_armed
            } else {
                self.style.menu
            };
            let border_color = if !entry.enabled {
                Color::new(0.0, 0.0, 0.0, 0.25)
            } else if armed {
                Color::new(0.0, 0.0, 0.0, 0.65)
            } else {
                Color::new(0.0, 0.0, 0.0, 0.45)
            };
            let color = if alpha_mult >= 1.0 - 1e-4 {
                color
            } else {
                Color::new(color.r, color.g, color.b, color.a * alpha_mult)
            };
            let border_alpha = if base_focus > 1e-4 {
                border_color.a * base_focus
            } else {
                0.0
            };
            let button_outline = if border_alpha > 1e-4 {
                TextOutline::new(Color::new(0.0, 0.0, 0.0, border_alpha), 1.0)
            } else {
                TextOutline::NONE
            };
            let offset = if is_pressed { vec2(1., 1.) } else { Vec2::ZERO };
            draw_cell_text_outlined(
                &entry.label,
                geometry.row_text_pos(i) + offset,
                geometry.font_tall,
                self.menu_font.as_ref(),
                self.menu_metrics,
                color,
                button_outline,
            );
        }
    }
}
