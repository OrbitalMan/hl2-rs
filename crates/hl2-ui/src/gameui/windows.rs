//! Non-modal dialog windows over a base layer (the main or pause menu, or the game), as retail
//! VGUI frames behave: the Options dialog and the developer console stay open while the menu
//! behind them keeps working. This model keeps their z-order, which layer has the keyboard
//! focus, which layer the pointer goes to, and the focus fade of each window. It knows nothing
//! of what the layers draw; callers pass hit tests and the time.
//!
//! - A press on a window raises it to the top and focuses it (SDK `Frame::Activate` on a mouse
//!   press); a press on the base focuses the base, leaving the windows where they are. A base
//!   that takes no pointer (gameplay) ignores presses and never takes the focus.
//! - A press belongs to the layer it started on until the button is released, so it never
//!   moves between layers while dragged.
//! - A modal layer of a window (a sub-dialog, an open list) takes every press.
//! - The focused window fades its background in over `Frame.FocusTransitionEffectTime`
//!   (SDK `Frame::OnFrameFocusChanged`); an unfocused one fades out.

use glam::Vec2;

/// A dialog window over the base layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Window {
    Options,
    Console,
}

/// A layer that can take the pointer or the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The menu (or the game) under every window.
    Base,
    Window(Window),
}

/// The left button this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pointer {
    pub at: Option<Vec2>,
    /// Went down this frame.
    pub click: bool,
    /// Is down.
    pub held: bool,
    /// Went up this frame.
    pub released: bool,
}

/// How far a window is faded towards its focused look (0 out of focus, 1 in focus).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    from: f32,
    to: f32,
    start: f64,
}

impl Fade {
    fn at(&self, now: f64, duration: f32) -> f32 {
        if duration <= 0. {
            return self.to;
        }
        let t = (((now - self.start) / duration as f64) as f32).clamp(0., 1.);
        self.from + (self.to - self.from) * t
    }
}

/// The open windows, bottom to top, and the focus.
#[derive(Clone, Debug, Default)]
pub struct Windows {
    order: Vec<(Window, Fade)>,
    /// The base has the focus although windows are open.
    base_focused: bool,
    /// The layer the held button belongs to.
    press: Option<Layer>,
    /// Time of the current frame, stamped on focus changes.
    now: f64,
    /// `Frame.FocusTransitionEffectTime` in seconds.
    pub fade_time: f32,
}

impl Windows {
    /// No windows; focus fades take `fade_time` seconds (`Frame.FocusTransitionEffectTime`).
    pub fn new(fade_time: f32) -> Self {
        Self {
            fade_time,
            ..Self::default()
        }
    }

    /// Sets the time used for focus changes from now on (seconds, monotonic).
    pub fn set_time(&mut self, now: f64) {
        self.now = now;
    }

    pub fn is_open(&self, window: Window) -> bool {
        self.order.iter().any(|(w, _)| *w == window)
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The open windows, bottom to top.
    pub fn order(&self) -> impl DoubleEndedIterator<Item = Window> + '_ {
        self.order.iter().map(|(w, _)| *w)
    }

    pub fn top(&self) -> Option<Window> {
        self.order.last().map(|(w, _)| *w)
    }

    /// The layer with the keyboard focus: the top window unless the base took it.
    pub fn focus(&self) -> Layer {
        match self.top() {
            Some(top) if !self.base_focused => Layer::Window(top),
            _ => Layer::Base,
        }
    }

    /// Opens `window` on top with the focus (or raises it if it is open).
    pub fn open(&mut self, window: Window) {
        let fade = self.take(window).unwrap_or(Fade {
            from: 1.,
            to: 1.,
            start: self.now,
        });
        self.order.push((window, fade));
        self.set_focus(Layer::Window(window));
    }

    /// Raises an open `window` to the top and focuses it.
    pub fn raise(&mut self, window: Window) {
        if self.is_open(window) {
            self.open(window);
        }
    }

    /// Closes `window`. The focus stays on the base if the base had it, otherwise it goes to
    /// the window now on top.
    pub fn close(&mut self, window: Window) {
        self.take(window);
        if self.press == Some(Layer::Window(window)) {
            self.press = None;
        }
        if self.order.is_empty() {
            self.base_focused = false;
        }
        let focus = self.focus();
        self.set_focus(focus);
    }

    /// Closes every window.
    pub fn clear(&mut self) {
        self.order.clear();
        self.base_focused = false;
        self.press = None;
    }

    /// Gives the keyboard focus to the base; the windows keep their order.
    pub fn focus_base(&mut self) {
        if !self.order.is_empty() {
            self.set_focus(Layer::Base);
        }
    }

    /// How far `window` is faded towards its focused look at `now` (0..1).
    pub fn focus_amount(&self, window: Window, now: f64) -> f32 {
        self.order
            .iter()
            .find(|(w, _)| *w == window)
            .map_or(0., |(_, fade)| fade.at(now, self.fade_time))
    }

    /// Which layer gets the pointer this frame, if any: the layer a held press belongs to,
    /// else a modal window, else the topmost window `hit` says is under the pointer, else the
    /// base when it takes the pointer; with no pointer position, the focused layer. A press focuses its layer and raises a window.
    pub fn route(
        &mut self,
        pointer: Pointer,
        modal: Option<Window>,
        base_takes_pointer: bool,
        hit: impl Fn(Window, Vec2) -> bool,
    ) -> Option<Layer> {
        let pressed = self
            .press
            .filter(|_| pointer.held || pointer.released)
            .filter(|layer| match layer {
                Layer::Window(w) => self.is_open(*w),
                Layer::Base => true,
            });
        let layer = pressed.or_else(|| {
            if let Some(window) = modal.filter(|w| self.is_open(*w)) {
                return Some(Layer::Window(window));
            }
            // Without a position (scripted or keyboard-only input) the focused layer gets the
            // button and the wheel.
            let Some(at) = pointer.at else {
                return match self.focus() {
                    Layer::Base => base_takes_pointer.then_some(Layer::Base),
                    window => Some(window),
                };
            };
            self.order
                .iter()
                .rev()
                .map(|(w, _)| *w)
                .find(|w| hit(*w, at))
                .map(Layer::Window)
                .or(base_takes_pointer.then_some(Layer::Base))
        });
        if pointer.click && pressed.is_none() {
            match layer {
                Some(Layer::Window(window)) => self.raise(window),
                Some(Layer::Base) => self.focus_base(),
                None => {}
            }
        }
        self.press = if pointer.released || !(pointer.click || pointer.held) {
            None
        } else if pointer.click && pressed.is_none() {
            layer
        } else {
            self.press
        };
        layer
    }

    fn take(&mut self, window: Window) -> Option<Fade> {
        let index = self.order.iter().position(|(w, _)| *w == window)?;
        Some(self.order.remove(index).1)
    }

    fn set_focus(&mut self, focus: Layer) {
        self.base_focused = focus == Layer::Base;
        let now = self.now;
        let duration = self.fade_time;
        for (window, fade) in &mut self.order {
            let to = if focus == Layer::Window(*window) {
                1.
            } else {
                0.
            };
            if fade.to != to {
                *fade = Fade {
                    from: fade.at(now, duration),
                    to,
                    start: now,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::vec2;

    /// Options covers x 0..100, the console x 50..150; the menu is everywhere else.
    fn hit(window: Window, p: Vec2) -> bool {
        match window {
            Window::Options => p.x < 100.,
            Window::Console => (50. ..150.).contains(&p.x),
        }
    }

    fn click(x: f32) -> Pointer {
        Pointer {
            at: Some(vec2(x, 0.)),
            click: true,
            held: true,
            released: false,
        }
    }

    fn held(x: f32) -> Pointer {
        Pointer {
            at: Some(vec2(x, 0.)),
            held: true,
            ..Default::default()
        }
    }

    fn release(x: f32) -> Pointer {
        Pointer {
            at: Some(vec2(x, 0.)),
            released: true,
            ..Default::default()
        }
    }

    fn hover(x: f32) -> Pointer {
        Pointer {
            at: Some(vec2(x, 0.)),
            ..Default::default()
        }
    }

    fn both() -> Windows {
        let mut windows = Windows::default();
        windows.open(Window::Options);
        windows.open(Window::Console);
        windows
    }

    #[test]
    fn the_pointer_goes_to_the_topmost_window_under_it_else_the_base() {
        let mut w = both();
        let route = |w: &mut Windows, x| w.route(hover(x), None, true, hit);
        assert_eq!(
            route(&mut w, 75.),
            Some(Layer::Window(Window::Console)),
            "overlap: top"
        );
        assert_eq!(route(&mut w, 25.), Some(Layer::Window(Window::Options)));
        assert_eq!(route(&mut w, 200.), Some(Layer::Base));
        assert_eq!(
            w.focus(),
            Layer::Window(Window::Console),
            "hovering never focuses"
        );
    }

    #[test]
    fn a_press_raises_and_focuses_its_window_or_focuses_the_base() {
        let mut w = both();
        w.route(click(25.), None, true, hit);
        w.route(release(25.), None, true, hit);
        assert_eq!(w.top(), Some(Window::Options));
        assert_eq!(w.focus(), Layer::Window(Window::Options));
        assert_eq!(
            w.route(hover(75.), None, true, hit),
            Some(Layer::Window(Window::Options)),
            "the raised window now wins the overlap"
        );
        w.route(click(200.), None, true, hit);
        w.route(release(200.), None, true, hit);
        assert_eq!(w.focus(), Layer::Base);
        assert_eq!(
            w.order().collect::<Vec<_>>(),
            [Window::Console, Window::Options],
            "focusing the base keeps the windows' order"
        );
    }

    #[test]
    fn a_press_stays_with_the_layer_it_started_on_until_release() {
        let mut w = both();
        assert_eq!(w.route(click(200.), None, true, hit), Some(Layer::Base));
        assert_eq!(
            w.route(held(75.), None, true, hit),
            Some(Layer::Base),
            "dragged onto a window"
        );
        assert_eq!(
            w.route(release(75.), None, true, hit),
            Some(Layer::Base),
            "released there"
        );
        assert_eq!(
            w.route(hover(75.), None, true, hit),
            Some(Layer::Window(Window::Console)),
            "free again after the release"
        );
        assert_eq!(
            w.route(click(25.), None, true, hit),
            Some(Layer::Window(Window::Options))
        );
        assert_eq!(
            w.route(held(200.), None, true, hit),
            Some(Layer::Window(Window::Options))
        );
        assert_eq!(
            w.route(release(200.), None, true, hit),
            Some(Layer::Window(Window::Options))
        );
        assert_eq!(
            w.focus(),
            Layer::Window(Window::Options),
            "the drag never refocused"
        );
    }

    #[test]
    fn a_modal_window_takes_every_press() {
        let mut w = both();
        assert_eq!(
            w.route(click(200.), Some(Window::Options), true, hit),
            Some(Layer::Window(Window::Options))
        );
        assert_eq!(w.top(), Some(Window::Options), "and is raised by it");
    }

    #[test]
    fn without_a_position_the_focused_layer_gets_the_pointer() {
        let mut w = both();
        let wheel = Pointer::default();
        assert_eq!(
            w.route(wheel, None, true, hit),
            Some(Layer::Window(Window::Console))
        );
        w.focus_base();
        assert_eq!(w.route(wheel, None, true, hit), Some(Layer::Base));
        assert_eq!(
            w.route(wheel, None, false, hit),
            None,
            "gameplay takes no pointer"
        );
    }

    #[test]
    fn a_base_without_pointer_takes_no_press_and_no_focus() {
        let mut w = Windows::default();
        w.open(Window::Console);
        assert_eq!(w.route(click(200.), None, false, hit), None);
        w.route(release(200.), None, false, hit);
        assert_eq!(w.focus(), Layer::Window(Window::Console));
    }

    #[test]
    fn closing_moves_the_focus_to_the_new_top_unless_the_base_had_it() {
        let mut w = both();
        w.close(Window::Console);
        assert_eq!(w.focus(), Layer::Window(Window::Options));
        w.open(Window::Console);
        w.focus_base();
        w.close(Window::Console);
        assert_eq!(w.focus(), Layer::Base);
        w.close(Window::Options);
        assert!(w.is_empty());
        assert_eq!(w.focus(), Layer::Base);
    }

    #[test]
    fn a_press_on_a_window_that_closes_is_forgotten() {
        let mut w = both();
        w.route(click(75.), None, true, hit);
        w.close(Window::Console);
        assert_eq!(
            w.route(held(200.), None, true, hit),
            Some(Layer::Base),
            "the rest of the press is not the console's any more"
        );
    }

    #[test]
    fn focus_fades_linearly_over_the_transition_time() {
        let mut w = Windows {
            fade_time: 0.3,
            ..Default::default()
        };
        w.set_time(10.);
        w.open(Window::Options);
        assert_eq!(w.focus_amount(Window::Options, 10.), 1., "opens focused");
        w.focus_base();
        assert_eq!(w.focus_amount(Window::Options, 10.), 1.);
        assert!((w.focus_amount(Window::Options, 10.15) - 0.5).abs() < 1e-4);
        assert!(w.focus_amount(Window::Options, 10.3) < 1e-4);
        w.set_time(10.15);
        w.raise(Window::Options);
        assert!(
            (w.focus_amount(Window::Options, 10.15) - 0.5).abs() < 1e-4,
            "a refocus mid-fade starts where it was"
        );
        assert!(w.focus_amount(Window::Options, 10.45) > 1. - 1e-4);
        assert_eq!(w.focus_amount(Window::Console, 10.45), 0., "not open");
    }
}
