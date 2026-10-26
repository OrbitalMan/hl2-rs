//! Gameplay HUD presentation, after the flashlight has updated for the frame.
use super::Hud;
use crate::{gameplay::Gameplay, ui_host::Console};

pub(super) fn draw(hud: &Hud, game: &Gameplay, ui: &Console) {
    hud.source.draw_status(
        &game.inventory,
        &game.weapons,
        &game.selection,
        game.scene.time,
    );
    hud.source.draw_selection(
        &game.selection,
        &game.inventory,
        &game.weapons,
        game.scene.time,
    );
    if !ui.source.paused() {
        hud.source.draw_crosshair(&game.inventory);
    }
    hud.source.draw_power(
        &game.inventory,
        game.scene.flashlight.state(),
        game.scene.time,
    );
}
