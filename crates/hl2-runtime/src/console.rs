//! Native input and rendering adapter for the shared pause menu/developer console.
pub use hl2_ui::console::{Effect, Mode};
use macroquad::prelude::*;
use source_assets::vpk::Vfs;
pub struct Console {
    source: hl2_ui::console::Console,
    painter: crate::hud::CanvasPainter,
}
impl std::ops::Deref for Console {
    type Target = hl2_ui::console::Console;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}
impl std::ops::DerefMut for Console {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.source
    }
}
impl Console {
    pub fn load(vfs: &Vfs) -> Self {
        Self {
            source: hl2_ui::console::Console::load(vfs, hl2_ui::canvas::Canvas::default()),
            painter: crate::hud::CanvasPainter::new().expect("console canvas renderer"),
        }
    }
    pub fn input(&mut self) -> Vec<Effect> {
        use hl2_ui::console::{Input, Key};
        self.source.canvas.resize(screen_width(), screen_height());
        let mut input = Input {
            pointer: Some(glam::Vec2::from_array([
                mouse_position().0,
                mouse_position().1,
            ])),
            click: is_mouse_button_pressed(MouseButton::Left),
            ..Default::default()
        };
        for (native, key) in [
            (KeyCode::Up, Key::Up),
            (KeyCode::Down, Key::Down),
            (KeyCode::Enter, Key::Enter),
            (KeyCode::Backspace, Key::Backspace),
            (KeyCode::Delete, Key::Delete),
            (KeyCode::Left, Key::Left),
            (KeyCode::Right, Key::Right),
            (KeyCode::Home, Key::Home),
            (KeyCode::End, Key::End),
            (KeyCode::PageUp, Key::PageUp),
            (KeyCode::PageDown, Key::PageDown),
            (KeyCode::Tab, Key::Tab),
        ] {
            if is_key_pressed(native) {
                input.keys.insert(key);
            }
        }
        while let Some(c) = get_char_pressed() {
            input.text.push(c);
        }
        self.source.input(&input)
    }
    pub fn draw(&self) {
        self.source.canvas.resize(screen_width(), screen_height());
        self.source.draw(get_time());
        self.painter.paint(&self.source.canvas);
    }
}
