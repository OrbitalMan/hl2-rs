//! Interpret HL2's installed dialog/control names and localization for the toolkit.
use super::{Catalog, Localization};
use glam::vec2;
use source_ui::{
    canvas::{Rect, Texture2D},
    card_browser::{BrowserLayout, BrowserSpec, Card},
    layout::Layout,
};

fn rect(layout: &Layout, name: &str, parent: (f32, f32), fallback: Rect) -> Rect {
    layout.get(name).map_or(fallback, |c| {
        let (x, y, w, h) = c.rect(parent);
        Rect::new(x, y, w, h)
    })
}

impl Catalog {
    fn browser_layout(&self) -> BrowserLayout {
        let mut layout = BrowserLayout::default();
        if let Some(c) = self
            .dialog
            .get("NewGameDialog")
            .filter(|c| c.wide > 0. && c.tall > 0.)
        {
            layout.size = vec2(c.wide, c.tall);
        }
        let parent = (layout.size.x, layout.size.y);
        layout.previous = rect(&self.dialog, "Prev", parent, layout.previous);
        layout.next = rect(&self.dialog, "Next", parent, layout.next);
        layout.accept = rect(&self.dialog, "Play", parent, layout.accept);
        layout.cancel = rect(&self.dialog, "Cancel", parent, layout.cancel);
        layout.dividers = ["divider", "Div"]
            .into_iter()
            .filter_map(|name| self.dialog.get(name))
            .map(|c| {
                let (x, y, w, h) = c.rect(parent);
                Rect::new(x, y, w, h)
            })
            .collect();
        let panel = &mut layout.card;
        panel.size.x = self
            .panel
            .controls
            .iter()
            .map(|c| c.wide)
            .fold(panel.size.x, f32::max);
        let parent = (panel.size.x, panel.size.y);
        panel.heading = rect(&self.panel, "ChapterLabel", parent, panel.heading);
        panel.title = rect(&self.panel, "ChapterNameLabel", parent, panel.title);
        panel.image = rect(&self.panel, "LevelPic", parent, panel.image);
        panel.border = rect(&self.panel, "LevelPicBorder", parent, panel.border);
        layout
    }

    pub(super) fn browser_spec(&self, loc: &Localization) -> BrowserSpec {
        let label = |name, token| {
            self.dialog
                .get(name)
                .filter(|c| !c.label.is_empty())
                .map_or_else(|| loc.text(token), |c| loc.text(&c.label))
        };
        let cards = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| Card {
                heading: e.heading.clone(),
                title: e.title.clone(),
                image: self
                    .thumbnails
                    .get(i)
                    .and_then(Option::as_ref)
                    .map(|image| Texture2D::from_rgba8(image.width, image.height, &image.rgba)),
            })
            .collect();
        BrowserSpec {
            cards,
            layout: self.browser_layout(),
            title: loc.text(if self.title.is_empty() {
                "#GameUI_NewGame"
            } else {
                &self.title
            }),
            labels: [
                label("Prev", "#GameUI_Prev"),
                label("Next", "#GameUI_Next"),
                label("Play", "#GameUI_StartNewGame"),
                label("Cancel", "#GameUI_Cancel"),
            ],
            selection: self.selection,
            fill: self.fill,
            text: self.text,
            release_sound: Some("ui/buttonclickrelease.wav".into()),
        }
    }
}
