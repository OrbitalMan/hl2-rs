//! A reusable paged card dialog. Applications supply all content, styling and geometry;
//! acceptance returns a card index and never executes an application action.
mod input;
mod view;
use crate::{
    canvas::{Color, Rect, Texture2D},
    console::dialog::Scaling,
    placement::{self, Placement},
    widgets::{ButtonTracker, KeyPress, Painter},
};
use glam::{vec2, Vec2};

pub struct Card {
    pub heading: String,
    pub title: String,
    pub image: Option<Texture2D>,
}

#[derive(Clone, Debug)]
pub struct CardLayout {
    pub size: Vec2,
    pub heading: Rect,
    pub title: Rect,
    pub image: Rect,
    pub border: Rect,
}
impl Default for CardLayout {
    fn default() -> Self {
        Self {
            size: vec2(172., 146.),
            heading: Rect::new(0., 4., 172., 20.),
            title: Rect::new(0., 20., 172., 20.),
            image: Rect::new(10., 50., 152., 86.),
            border: Rect::new(0., 40., 168., 106.),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BrowserLayout {
    pub size: Vec2,
    pub card: CardLayout,
    pub card_top: f32,
    pub previous: Rect,
    pub next: Rect,
    pub accept: Rect,
    pub cancel: Rect,
    pub dividers: Vec<Rect>,
}
impl Default for BrowserLayout {
    fn default() -> Self {
        Self {
            size: vec2(600., 296.),
            card: CardLayout::default(),
            card_top: 44.,
            previous: Rect::new(24., 200., 72., 24.),
            next: Rect::new(500., 200., 72., 24.),
            accept: Rect::new(363., 252., 124., 24.),
            cancel: Rect::new(500., 252., 72., 24.),
            dividers: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct BrowserSpec {
    pub cards: Vec<Card>,
    pub layout: BrowserLayout,
    pub title: String,
    /// Previous page, next page, accept and cancel, already localized by the application.
    pub labels: [String; 4],
    pub selection: Option<Color>,
    pub fill: Option<Color>,
    pub text: Option<Color>,
    pub release_sound: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Card(usize),
    Previous,
    Next,
    Accept,
    Cancel,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Close,
    Accept(usize),
}

pub struct CardBrowser {
    pub selected: usize,
    pub first: usize,
    pub escape_held: bool,
    pub placement: Placement,
    spec: BrowserSpec,
    scaling: Scaling,
    display_titles: Vec<String>,
    tracker: ButtonTracker<Target>,
    keys: KeyPress<Target>,
    focus: Option<Target>,
    focus_visible: bool,
    hover: Option<Target>,
    sounds: Vec<String>,
}
impl CardBrowser {
    pub fn new(spec: BrowserSpec, scaling: Scaling) -> Self {
        Self {
            selected: 0,
            first: 0,
            escape_held: false,
            placement: Placement::default(),
            spec,
            scaling,
            display_titles: Vec::new(),
            tracker: ButtonTracker::capturing(),
            keys: KeyPress::default(),
            focus: Some(Target::Card(0)),
            focus_visible: false,
            hover: None,
            sounds: Vec::new(),
        }
    }
    pub fn open(&mut self) {
        self.selected = 0;
        self.first = 0;
        self.escape_held = false;
        self.tracker.reset();
        self.keys.reset();
        self.focus = Some(Target::Card(0));
        self.focus_visible = false;
        self.hover = None;
        self.placement.reset();
    }
    pub fn frame(&self, canvas: Vec2) -> placement::Frame {
        let size = self.spec.layout.size;
        let scale = (self.scaling.dialog_scale)(canvas);
        placement::Frame {
            origin: ((canvas - size * scale) * 0.5).round(),
            size,
            scale,
            canvas,
            min_size: None,
            close: crate::widgets::close_button_rect,
        }
    }
    pub fn bounds(&self, canvas: Vec2) -> Rect {
        let frame = self.frame(canvas);
        let placed = self.placement.place(&frame);
        Rect::new(
            placed.origin.x,
            placed.origin.y,
            placed.size.x * frame.scale,
            placed.size.y * frame.scale,
        )
    }
    fn panel_rect(&self, index: usize) -> Rect {
        let layout = &self.spec.layout;
        let size = layout.card.size;
        let x = if self.spec.cards.len() == 1 {
            (layout.size.x - size.x) * 0.5
        } else {
            let span = layout.next.x + layout.next.w - layout.previous.x;
            let gap = (span - size.x * 3.) / 2.;
            layout.previous.x + (size.x + gap) * (index - self.first) as f32
        };
        Rect::new(x, layout.card_top, size.x, size.y)
    }
    fn targets(&self) -> impl Iterator<Item = (Target, Rect)> + '_ {
        let cards = (self.first..(self.first + 3).min(self.spec.cards.len()))
            .map(|i| (Target::Card(i), self.panel_rect(i)));
        let layout = &self.spec.layout;
        let buttons = [
            (self.first > 0, Target::Previous, layout.previous),
            (
                self.first + 3 < self.spec.cards.len(),
                Target::Next,
                layout.next,
            ),
            (!self.spec.cards.is_empty(), Target::Accept, layout.accept),
            (true, Target::Cancel, layout.cancel),
        ]
        .into_iter()
        .filter(|(show, _, _)| *show)
        .map(|(_, target, rect)| (target, rect));
        cards.chain(buttons).chain(std::iter::once((
            Target::Close,
            crate::widgets::close_button_rect(layout.size),
        )))
    }
    pub fn prepare_text(&mut self, painter: &Painter<'_>) {
        self.display_titles = self
            .spec
            .cards
            .iter()
            .map(|card| {
                painter
                    .truncate_ellipsis(&card.title, self.spec.layout.card.title.w, true)
                    .into_owned()
            })
            .collect();
    }
    pub fn thumbnail_count(&self) -> usize {
        self.spec.cards.iter().filter(|c| c.image.is_some()).count()
    }
}
#[cfg(test)]
mod tests;
