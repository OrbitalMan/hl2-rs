//! Bevy host state for the portable Source-resource pause/console UI.
use bevy::prelude::*;
use hl2_ui::console::Mode;
use serde::Deserialize;
#[derive(Resource)]
pub struct Console {
    pub source: hl2_ui::console::Console,
    pub map_request: Option<String>,
    pub quit_requested: bool,
}
impl Console {
    pub fn new(source: hl2_ui::console::Console) -> Self {
        Self {
            source,
            map_request: None,
            quit_requested: false,
        }
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"mode":self.source.mode,"sv_cheats":self.source.cheats,"input":self.source.input,"output":self.source.output,"pending_map":self.map_request})
    }
}
#[derive(Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Escape,
    ToggleConsole,
    Resume,
    Command { command: String },
    Input { input: hl2_ui::console::Input },
}
impl Action {
    pub fn valid(&self) -> bool {
        match self {
            Self::Command { command } => command.len() <= 1024,
            Self::Input { input } => {
                input.text.len() <= 4096 && input.pointer.is_none_or(|p| p.is_finite())
            }
            _ => true,
        }
    }
}
impl crate::movement::Simulation {
    pub fn ui_action(
        &mut self,
        ui: &mut Console,
        game: &mut crate::gameplay::Gameplay,
        action: &Action,
    ) {
        let effects = match action {
            Action::Escape => {
                if !ui.source.paused() && game.selection.pending.is_some() {
                    game.actions.push(crate::gameplay::Action::Cancel);
                } else {
                    ui.source.escape();
                }
                vec![]
            }
            Action::ToggleConsole => {
                ui.source.toggle();
                vec![]
            }
            Action::Resume => {
                ui.source.mode = Mode::Gameplay;
                vec![]
            }
            Action::Command { command } => ui.source.submit(command),
            Action::Input { input } => ui.source.input(input),
        };
        self.console_effects(ui, game, effects);
    }
}
pub fn keys(native: &ButtonInput<KeyCode>) -> std::collections::BTreeSet<hl2_ui::console::Key> {
    use hl2_ui::console::Key;
    [
        (KeyCode::ArrowUp, Key::Up),
        (KeyCode::ArrowDown, Key::Down),
        (KeyCode::Enter, Key::Enter),
        (KeyCode::Backspace, Key::Backspace),
        (KeyCode::Delete, Key::Delete),
        (KeyCode::ArrowLeft, Key::Left),
        (KeyCode::ArrowRight, Key::Right),
        (KeyCode::Home, Key::Home),
        (KeyCode::End, Key::End),
        (KeyCode::PageUp, Key::PageUp),
        (KeyCode::PageDown, Key::PageDown),
        (KeyCode::Tab, Key::Tab),
    ]
    .into_iter()
    .filter_map(|(n, k)| native.just_pressed(n).then_some(k))
    .collect()
}
