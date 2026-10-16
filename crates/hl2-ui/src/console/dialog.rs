//! The console dialog's geometry and output row selection.
//!
//! Layout follows the SDK `CConsoleDialog` / `CConsolePanel::PerformLayout`: a titled frame whose
//! client area holds the history (`RichText`, `DepressedButtonBorder`), the entry and the Submit
//! button along the bottom. The panel is not proportional, so its insets are plain layout units.
//! The default placement is the retail one: proportional offsets (480-line basis) of 16 and 64
//! put the frame at `(w/2 - 16, 64)`, half the screen wide and `h - 2 * 64` tall.

use crate::canvas::Rect;
use crate::gameui::widgets::{close_button_rect, Painter, CLIENT_INSET_X, CLIENT_TOP};
use glam::{vec2, Vec2};

/// `Frame.ClientInsetY`: space under the client area.
const CLIENT_INSET_Y: f32 = 6.;
/// `CConsolePanel::PerformLayout` insets.
const INSET: f32 = 8.;
const ENTRY_TALL: f32 = 24.;
const TOP_TALL: f32 = 4.;
const ENTRY_INSET: f32 = 4.;
const SUBMIT_WIDE: f32 = 64.;
/// Pulls the Submit button away from the frame's resize grip.
const SUBMIT_INSET: f32 = 7.;
/// Retail proportional values are relative to a 480-line screen.
const PROPORTIONAL_BASE: f32 = 480.;

/// Where the console dialog and its controls are. Control rectangles are in layout units
/// relative to the frame; [`Self::canvas`] maps them to canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogLayout {
    /// Frame top-left in canvas pixels.
    pub origin: Vec2,
    /// Canvas pixels per layout unit ([`Painter::scale_for`]).
    pub scale: f32,
    /// Frame size in layout units.
    pub size: Vec2,
    pub history: Rect,
    pub entry: Rect,
    pub submit: Rect,
    pub close: Rect,
}

impl DialogLayout {
    pub fn new(canvas: Vec2) -> Self {
        let scale = Painter::scale_for(canvas.y);
        let proportional = |value: f32| (value * canvas.y / PROPORTIONAL_BASE).floor();
        let origin = vec2(
            (canvas.x * 0.5).floor() - proportional(16.),
            proportional(64.),
        )
        .max(Vec2::ZERO);
        // SDK `CConsolePanel` minimum size.
        let size = (vec2((canvas.x * 0.5).floor(), canvas.y - 2. * proportional(64.)) / scale)
            .round()
            .max(vec2(100., 100.));
        // Frame client area (`Frame::GetClientArea`), then the panel's own insets inside it.
        let (cx, cy) = (CLIENT_INSET_X, CLIENT_TOP);
        let (cw, ch) = (
            size.x - 2. * CLIENT_INSET_X,
            size.y - CLIENT_INSET_Y - CLIENT_TOP,
        );
        let history = Rect::new(
            cx + INSET,
            cy + INSET + TOP_TALL,
            cw - 2. * INSET,
            (ch - (ENTRY_INSET * 2. + INSET * 2. + TOP_TALL + ENTRY_TALL)).max(0.),
        );
        let bottom = cy + ch - (ENTRY_INSET * 2. + ENTRY_TALL);
        let submit_x = cw - (INSET + SUBMIT_WIDE + SUBMIT_INSET);
        Self {
            origin,
            scale,
            size,
            history,
            entry: Rect::new(
                cx + INSET,
                bottom,
                (submit_x - ENTRY_INSET - 2. * INSET).max(0.),
                ENTRY_TALL,
            ),
            submit: Rect::new(cx + submit_x, bottom, SUBMIT_WIDE, ENTRY_TALL),
            close: close_button_rect(size),
        }
    }

    /// A frame-relative layout rectangle in canvas pixels.
    pub fn canvas(&self, rect: Rect) -> Rect {
        Rect::new(
            (self.origin.x + rect.x * self.scale).round(),
            (self.origin.y + rect.y * self.scale).round(),
            (rect.w * self.scale).round(),
            (rect.h * self.scale).round(),
        )
    }

    /// A canvas point in frame-relative layout units.
    pub fn local(&self, point: Vec2) -> Vec2 {
        (point - self.origin) / self.scale
    }
}

/// Selected output rows, by line number since the console started, so a selection stays on its
/// lines while new output arrives and old lines scroll out of the buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Where the drag started, and where it is now.
    pub anchor: u64,
    pub head: u64,
}

impl Selection {
    pub fn at(line: u64) -> Self {
        Self {
            anchor: line,
            head: line,
        }
    }

    /// Whether `line` is selected.
    pub fn contains(&self, line: u64) -> bool {
        (self.anchor.min(self.head)..=self.anchor.max(self.head)).contains(&line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_follows_the_console_panel_insets_at_1024x768() {
        let layout = DialogLayout::new(vec2(1024., 768.));
        // Proportional 16 and 64 at 768 lines are 25 and 102 pixels.
        assert_eq!(layout.origin, vec2(487., 102.));
        let scale = 768. / 720.;
        assert_eq!(layout.scale, scale);
        assert_eq!(layout.size, (vec2(512., 564.) / scale).round());
        let (w, h) = (layout.size.x, layout.size.y);
        // Client area: x 8, y 30, w - 16 wide, down to 6 above the frame edge.
        assert_eq!(layout.history, Rect::new(16., 42., w - 32., h - 36. - 52.));
        let bottom = h - 6. - 8. - 24.;
        assert_eq!(layout.submit, Rect::new(w - 8. - 79., bottom, 64., 24.));
        assert_eq!(
            layout.entry,
            Rect::new(16., bottom, w - 16. - 79. - 4. - 16., 24.)
        );
        // History, entry and button stay inside the frame, entry left of the button.
        assert!(layout.entry.x + layout.entry.w < layout.submit.x);
        assert!(layout.history.y + layout.history.h < layout.entry.y);
        assert!(layout.submit.x + layout.submit.w < w);
        // Layout and canvas coordinates round-trip.
        let submit = layout.canvas(layout.submit);
        let centre = vec2(submit.x + submit.w * 0.5, submit.y + submit.h * 0.5);
        assert!(layout.submit.contains(layout.local(centre)));
    }

    #[test]
    fn small_canvases_keep_the_minimum_size() {
        let layout = DialogLayout::new(vec2(120., 100.));
        assert_eq!(layout.size, vec2(100., 100.));
        assert!(layout.history.h >= 0. && layout.entry.w >= 0.);
    }

    #[test]
    fn selections_cover_their_span_in_either_direction() {
        let down = Selection { anchor: 3, head: 5 };
        let up = Selection { anchor: 5, head: 3 };
        for selection in [down, up] {
            assert!(!selection.contains(2));
            assert!((3..=5).all(|line| selection.contains(line)));
            assert!(!selection.contains(6));
        }
        assert!(Selection::at(7).contains(7));
    }
}
