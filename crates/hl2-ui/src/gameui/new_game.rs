//! Resource-driven chapter browser; no map tables or filesystem work during input/drawing.
mod input;
mod resources;
mod view;

use super::{localize::Localization, resources::Image};
use glam::{vec2, Vec2};
use source_ui::{
    canvas::{Rect, Texture2D},
    layout::Layout,
    placement::{self, Placement},
    widgets::{ButtonTracker, KeyPress},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub map: String,
    pub heading: String,
    pub title: String,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub thumbnails: Vec<Option<Image>>,
    pub dialog: Layout,
    pub panel: Layout,
    pub selection: Option<source_ui::canvas::Color>,
    pub fill: Option<source_ui::canvas::Color>,
    pub text: Option<source_ui::canvas::Color>,
    pub title: String,
}

/// Retail discovers MOD chapter configs, not the chapter background list. Reject malformed
/// keys and configs without a map; numeric order keeps chapter9a before chapter10.
pub fn catalog<'a>(
    files: impl IntoIterator<Item = &'a str>,
    read: impl Fn(&str) -> Option<String>,
    game: &str,
    loc: &Localization,
) -> Vec<Entry> {
    let mut entries: Vec<_> = files
        .into_iter()
        .filter_map(|file| {
            let key = file.strip_prefix("cfg/chapter")?.strip_suffix(".cfg")?;
            let digits = key.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 || !key[digits..].bytes().all(|c| c.is_ascii_alphabetic()) {
                return None;
            }
            let map = super::resources::chapter_map(&read(file)?)?;
            Some(Entry {
                key: key.to_owned(),
                map,
                heading: format!("{} {key}", loc.text("#GameUI_Chapter")),
                title: loc.text(&format!("#{game}_Chapter{key}_Title")),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        let number = |key: &str| {
            key.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(u32::MAX)
        };
        number(&a.key)
            .cmp(&number(&b.key))
            .then_with(|| a.key.len().cmp(&b.key.len()))
            .then_with(|| a.key.cmp(&b.key))
    });
    entries.dedup_by(|a, b| a.key == b.key);
    entries
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Chapter(usize),
    Previous,
    Next,
    Play,
    Cancel,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Close,
    Start(crate::console::GameMap),
}

/// Prepared textures and labels persist while closed; opening a dialog does no asset IO.
pub struct Dialog {
    pub game: String,
    pub selected: usize,
    pub first: usize,
    pub escape_held: bool,
    pub placement: Placement,
    catalog: Catalog,
    textures: Vec<Option<Texture2D>>,
    title: String,
    display_titles: Vec<String>,
    labels: [String; 4],
    tracker: ButtonTracker<Target>,
    keys: KeyPress<Target>,
    focus: Option<Target>,
    focus_visible: bool,
    hover: Option<Target>,
    sounds: Vec<String>,
}

impl Dialog {
    pub fn new(game: &str, catalog: Catalog, loc: &Localization) -> Self {
        let textures = catalog
            .thumbnails
            .iter()
            .map(|image| {
                image
                    .as_ref()
                    .map(|i| Texture2D::from_rgba8(i.width, i.height, &i.rgba))
            })
            .collect();
        let label = |name, token| {
            catalog
                .dialog
                .get(name)
                .filter(|c| !c.label.is_empty())
                .map_or_else(|| loc.text(token), |c| loc.text(&c.label))
        };
        let title = loc.text(if catalog.title.is_empty() {
            "#GameUI_NewGame"
        } else {
            &catalog.title
        });
        let labels = [
            label("Prev", "#GameUI_Prev"),
            label("Next", "#GameUI_Next"),
            label("Play", "#GameUI_StartNewGame"),
            label("Cancel", "#GameUI_Cancel"),
        ];
        Self {
            game: game.to_owned(),
            selected: 0,
            first: 0,
            escape_held: false,
            placement: Placement::default(),
            catalog,
            textures,
            title,
            display_titles: Vec::new(),
            labels,
            tracker: ButtonTracker::capturing(),
            keys: KeyPress::default(),
            focus: Some(Target::Chapter(0)),
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
        self.focus = Some(Target::Chapter(0));
        self.focus_visible = false;
        self.hover = None;
        self.placement.reset();
    }

    fn size(&self) -> Vec2 {
        self.catalog
            .dialog
            .get("NewGameDialog")
            .map(|c| vec2(c.wide, c.tall))
            .filter(|v| v.x > 0. && v.y > 0.)
            .unwrap_or(vec2(600., 296.))
    }

    pub fn frame(&self, canvas: Vec2) -> placement::Frame {
        let size = self.size();
        let scale = crate::proportional::dialog_scale(canvas);
        placement::Frame {
            origin: ((canvas - size * scale) * 0.5).round(),
            size,
            scale,
            canvas,
            min_size: None,
            close: source_ui::widgets::close_button_rect,
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

    fn control(&self, name: &str, fallback: Rect) -> Rect {
        self.catalog.dialog.get(name).map_or(fallback, |c| {
            let (x, y, w, h) = c.rect((self.size().x, self.size().y));
            Rect::new(x, y, w, h)
        })
    }

    fn panel_rect(&self, index: usize) -> Rect {
        let wide = self
            .catalog
            .panel
            .controls
            .iter()
            .map(|c| c.wide)
            .fold(172., f32::max);
        if self.catalog.entries.len() == 1 {
            return Rect::new((self.size().x - wide) * 0.5, 44., wide, 146.);
        }
        let left = self.control("Prev", Rect::new(24., 200., 72., 24.)).x;
        let right = self.control("Next", Rect::new(500., 200., 72., 24.));
        let gap = ((right.x + right.w - left) - wide * 3.) / 2.;
        Rect::new(
            left + (wide + gap) * (index - self.first) as f32,
            44.,
            wide,
            146.,
        )
    }

    fn targets(&self) -> impl Iterator<Item = (Target, Rect)> + '_ {
        let cards = (self.first..(self.first + 3).min(self.catalog.entries.len()))
            .map(|i| (Target::Chapter(i), self.panel_rect(i)));
        let buttons = [
            (
                self.first > 0,
                Target::Previous,
                "Prev",
                Rect::new(24., 200., 72., 24.),
            ),
            (
                self.first + 3 < self.catalog.entries.len(),
                Target::Next,
                "Next",
                Rect::new(500., 200., 72., 24.),
            ),
            (
                !self.catalog.entries.is_empty(),
                Target::Play,
                "Play",
                Rect::new(363., 252., 124., 24.),
            ),
            (
                true,
                Target::Cancel,
                "Cancel",
                Rect::new(500., 252., 72., 24.),
            ),
        ]
        .into_iter()
        .filter(|(show, _, _, _)| *show)
        .map(|(_, target, name, fallback)| (target, self.control(name, fallback)));
        cards.chain(buttons).chain(std::iter::once((
            Target::Close,
            source_ui::widgets::close_button_rect(self.size()),
        )))
    }

    /// Cache ellipsized chapter labels once using the installed dialog font.
    pub fn prepare_text(&mut self, painter: &source_ui::widgets::Painter<'_>) {
        let width = self
            .catalog
            .panel
            .get("ChapterNameLabel")
            .map_or(172., |c| c.wide);
        self.display_titles = self
            .catalog
            .entries
            .iter()
            .map(|e| {
                painter
                    .truncate_ellipsis(&e.title, width, true)
                    .into_owned()
            })
            .collect();
    }

    pub fn report(&self, canvas: Vec2) -> serde_json::Value {
        let b = self.bounds(canvas);
        serde_json::json!({"game": self.game, "selected": self.selected, "first": self.first,
            "frame": [b.x,b.y,b.w,b.h], "entries": self.catalog.entries.iter().map(|e| {
                serde_json::json!({"key":e.key,"map":e.map,"heading":e.heading,"title":e.title})
            }).collect::<Vec<_>>(), "thumbnails": self.textures.iter().filter(|t| t.is_some()).count()})
    }
}

#[cfg(test)]
mod tests;
