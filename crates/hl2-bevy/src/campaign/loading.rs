//! Coordinate pending requests with the authoritative active worker, not UI queue flags.
use super::Request;
use crate::ui_host;

pub(super) fn preserve_gameplay_request(request: &Request, ui: &mut ui_host::Console) {
    if !request.background && ui.background_request {
        ui.source
            .log("Gameplay map load in progress; background map request ignored");
        ui.background_request = false;
        ui.map_request = None;
        ui.map_request_game = None;
        ui.source.start_loading(&request.map);
    }
}

/// A completed background must not replace a newer requested world or loading dialog.
/// Identical pending backgrounds are coalesced into this completion.
pub(super) fn superseded(request: &Request, ui: &mut ui_host::Console) -> bool {
    if !request.background || ui.map_request.is_none() {
        return false;
    }
    if ui.background_request
        && ui.map_request.as_deref() == Some(&request.map)
        && ui.map_request_game.as_deref() == Some(&request.game)
    {
        ui.map_request = None;
        ui.map_request_game = None;
        ui.background_request = false;
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{Request, preserve_gameplay_request, superseded, ui_host};
    fn request(background: bool) -> Request {
        Request {
            map: "background01".into(),
            game: "hl2".into(),
            landmark: String::new(),
            direct: true,
            background,
        }
    }
    fn queued(background: bool, game: &str, map: &str) -> ui_host::Console {
        let mut ui = ui_host::Console::new(Default::default());
        ui.map_request = Some(map.into());
        ui.map_request_game = Some(game.into());
        ui.background_request = background;
        ui
    }
    #[test]
    fn gameplay_worker_restores_modal_after_rejecting_background_request() {
        let mut ui = queued(true, "ep2", "ep2_background01");
        ui.source
            .start_background_loading("ep2", "ep2_background01");
        preserve_gameplay_request(&request(false), &mut ui);
        assert!(ui.map_request.is_none());
        assert!(!ui.background_request);
        assert!(ui.source.modal_dialog_open());
        assert!(!ui.source.is_background_loading());
    }
    #[test]
    fn background_completion_preserves_newer_background_or_chapter_request() {
        for background in [true, false] {
            let mut ui = queued(background, "ep2", "ep2_outland_01");
            assert!(superseded(&request(true), &mut ui));
            assert_eq!(ui.map_request.as_deref(), Some("ep2_outland_01"));
        }
        let mut ui = queued(true, "lostcoast", "background01");
        assert!(
            superseded(&request(true), &mut ui),
            "identity includes the game"
        );
    }
    #[test]
    fn identical_pending_background_is_coalesced_but_browser_close_keeps_worker() {
        let mut ui = queued(true, "hl2", "background01");
        assert!(!superseded(&request(true), &mut ui));
        assert!(ui.map_request.is_none());
        assert!(!ui.background_request);
        ui.source.set_mode(hl2_ui::console::Mode::NewGame);
        ui.source.set_mode(hl2_ui::console::Mode::MainMenu);
        assert!(!superseded(&request(true), &mut ui));
    }
}
