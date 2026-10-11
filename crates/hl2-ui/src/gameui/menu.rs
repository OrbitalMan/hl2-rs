//! The game menu (main menu, its HL2/Episode categories and the in-game menu) and the VGUI
//! message box used for the quit confirmation.
//!
//! Entries come from `GameMenu.res`. Out of game the current `MenuCategory` is shown in file
//! order; in game the uncategorized `OnlyInGame` entries are shown sorted by `InGameOrder`.
//! Commands without a Rust implementation (Load/Save Game, Achievements, Extras, New Game for a
//! game whose maps are not mounted) are shown disabled, so they are visibly placeholders.

use super::localize::Localization;
use super::resources::{MenuItem, Visibility};
use super::widgets::{ButtonState, Painter, CLIENT_INSET_X, CLIENT_TOP, FONT_TALL};
use crate::canvas::Rect;
use glam::{vec2, Vec2};
use std::collections::BTreeMap;

/// A parsed `GameMenu.res` command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuCommand {
    /// `SwitchExpansion <game>`: opens that game's category.
    SwitchExpansion(String),
    SwitchCategory(String),
    OpenOptions,
    Quit,
    Resume,
    /// `OpenNewGameDialog <game>`.
    NewGame(String),
    LoadGame,
    SaveGame,
    Achievements,
    ReturnToMainMenu,
    Unsupported(String),
}

impl MenuCommand {
    pub fn parse(command: &str) -> Self {
        let mut words = command.split_whitespace();
        let verb = words.next().unwrap_or_default().to_ascii_lowercase();
        let argument = words.next().unwrap_or_default().to_ascii_lowercase();
        match verb.as_str() {
            "switchexpansion" => Self::SwitchExpansion(argument),
            "switchcategory" => Self::SwitchCategory(argument),
            "openoptionsdialog" => Self::OpenOptions,
            "quit" | "quitnoconfirm" => Self::Quit,
            "resumegame" => Self::Resume,
            "opennewgamedialog" => Self::NewGame(if argument.is_empty() {
                "hl2".into()
            } else {
                argument
            }),
            "openloadgamedialog" => Self::LoadGame,
            "opensavegamedialog" => Self::SaveGame,
            "openachievementsdialog" => Self::Achievements,
            "openmainmenudialog" | "disconnect" => Self::ReturnToMainMenu,
            _ => Self::Unsupported(command.to_owned()),
        }
    }
}

/// One visible row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub label: String,
    pub command: MenuCommand,
    pub spacer: bool,
    pub enabled: bool,
}

/// What activating an entry asks the owner to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    None,
    Resume,
    OpenOptions,
    ConfirmQuit,
    /// Start the named map (first chapter of a game).
    NewGame(String),
    ReturnToMainMenu,
    /// A disabled placeholder was activated; the label is reported to the console.
    Placeholder(String),
}

pub const MAIN_CATEGORY: &str = "main";

pub struct GameMenu {
    items: Vec<MenuItem>,
    new_game_maps: BTreeMap<String, String>,
    category: String,
    in_game: bool,
    entries: Vec<MenuEntry>,
    pub selection: Option<usize>,
}

impl GameMenu {
    pub fn new(
        items: Vec<MenuItem>,
        new_game_maps: BTreeMap<String, String>,
        loc: &Localization,
    ) -> Self {
        let mut menu = Self {
            items,
            new_game_maps,
            category: MAIN_CATEGORY.into(),
            in_game: false,
            entries: Vec::new(),
            selection: None,
        };
        menu.rebuild(loc);
        menu
    }

    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    pub fn category(&self) -> &str {
        &self.category
    }

    pub fn in_game(&self) -> bool {
        self.in_game
    }

    /// Switches between the main menu and the in-game menu; leaving the game resets the category.
    pub fn set_in_game(&mut self, in_game: bool, loc: &Localization) {
        if self.in_game != in_game {
            self.in_game = in_game;
            self.category = MAIN_CATEGORY.into();
            self.selection = None;
            self.rebuild(loc);
        }
    }

    fn set_category(&mut self, category: &str, loc: &Localization) {
        self.category = category.to_ascii_lowercase();
        self.selection = None;
        self.rebuild(loc);
    }

    fn rebuild(&mut self, loc: &Localization) {
        let mut items: Vec<&MenuItem> = if self.in_game {
            let mut items: Vec<_> = self
                .items
                .iter()
                .filter(|i| i.category.is_none() && i.visibility != Visibility::NotInGame)
                .collect();
            items.sort_by_key(|i| i.order);
            items
        } else {
            self.items
                .iter()
                .filter(|i| {
                    i.category.as_deref() == Some(self.category.as_str())
                        && i.visibility != Visibility::OnlyInGame
                })
                .collect()
        };
        // A trailing or doubled spacer adds nothing visible.
        items.dedup_by(|a, b| a.is_spacer() && b.is_spacer());
        self.entries = items
            .into_iter()
            .map(|item| {
                let command = MenuCommand::parse(&item.command);
                let label = if item.is_spacer() {
                    String::new()
                } else {
                    let text = loc.localize(&item.label);
                    if item.caps {
                        text.to_uppercase()
                    } else {
                        text
                    }
                };
                MenuEntry {
                    enabled: !item.is_spacer() && self.implemented(&command),
                    spacer: item.is_spacer(),
                    label,
                    command,
                }
            })
            .collect();
    }

    fn implemented(&self, command: &MenuCommand) -> bool {
        match command {
            MenuCommand::SwitchExpansion(game) => self
                .items
                .iter()
                .any(|i| i.category.as_deref() == Some(game.as_str())),
            // The Extras category (Workshop, Lost Coast) is a placeholder in this build.
            MenuCommand::SwitchCategory(category) => category == MAIN_CATEGORY,
            MenuCommand::NewGame(game) => self.new_game_maps.contains_key(game),
            MenuCommand::OpenOptions
            | MenuCommand::Quit
            | MenuCommand::Resume
            | MenuCommand::ReturnToMainMenu => true,
            MenuCommand::LoadGame
            | MenuCommand::SaveGame
            | MenuCommand::Achievements
            | MenuCommand::Unsupported(_) => false,
        }
    }

    /// Moves the keyboard selection by `step`, skipping spacers and disabled rows.
    pub fn step(&mut self, step: i32) {
        let count = self.entries.len() as i32;
        if count == 0 || !self.entries.iter().any(|e| e.enabled) {
            return;
        }
        let mut index = self
            .selection
            .map_or(if step > 0 { -1 } else { count }, |i| i as i32);
        loop {
            index = (index + step).rem_euclid(count);
            if self.entries[index as usize].enabled {
                self.selection = Some(index as usize);
                return;
            }
        }
    }

    /// Escape inside a category returns to the main category. Returns whether it was consumed.
    pub fn back(&mut self, loc: &Localization) -> bool {
        if !self.in_game && self.category != MAIN_CATEGORY {
            self.set_category(MAIN_CATEGORY, loc);
            true
        } else {
            false
        }
    }

    pub fn activate(&mut self, index: usize, loc: &Localization) -> MenuOutcome {
        let Some(entry) = self.entries.get(index).cloned() else {
            return MenuOutcome::None;
        };
        if entry.spacer {
            return MenuOutcome::None;
        }
        if !entry.enabled {
            return MenuOutcome::Placeholder(entry.label);
        }
        match entry.command {
            MenuCommand::SwitchExpansion(game) => {
                self.set_category(&game, loc);
                MenuOutcome::None
            }
            MenuCommand::SwitchCategory(category) => {
                self.set_category(&category, loc);
                MenuOutcome::None
            }
            MenuCommand::OpenOptions => MenuOutcome::OpenOptions,
            MenuCommand::Quit => MenuOutcome::ConfirmQuit,
            MenuCommand::Resume => MenuOutcome::Resume,
            MenuCommand::NewGame(game) => self
                .new_game_maps
                .get(&game)
                .cloned()
                .map_or(MenuOutcome::Placeholder(entry.label), MenuOutcome::NewGame),
            MenuCommand::ReturnToMainMenu => MenuOutcome::ReturnToMainMenu,
            _ => MenuOutcome::Placeholder(entry.label),
        }
    }
}

/// A VGUI `MessageBox`/`QueryBox`: title, one centered line and OK (+ optional Cancel).
#[derive(Clone, Debug, PartialEq)]
pub struct MessageBox {
    pub title: String,
    pub text: String,
    pub ok: String,
    pub cancel: Option<String>,
    armed: Option<usize>,
    pressed: Option<usize>,
}

/// Message box geometry in layout units relative to the box.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageBoxLayout {
    pub size: Vec2,
    pub text_at: Vec2,
    pub buttons: Vec<Rect>,
}

impl MessageBox {
    pub fn new(title: String, text: String, ok: String, cancel: Option<String>) -> Self {
        Self {
            title,
            text,
            ok,
            cancel,
            armed: None,
            pressed: None,
        }
    }

    /// The Quit confirmation (`#GameUI_QuitConfirmationTitle`/`Text`, `#GameUI_Quit`, `#GameUI_Cancel`).
    pub fn quit(loc: &Localization) -> Self {
        Self::new(
            loc.text("#GameUI_QuitConfirmationTitle"),
            loc.text("#GameUI_QuitConfirmationText"),
            loc.text("#GameUI_Quit"),
            Some(loc.text("#GameUI_Cancel")),
        )
    }

    /// Box size from the message width plus 100 in both directions, at least wide enough for twice
    /// the buttons plus 30; buttons sit at the quarter points, 15 above the client bottom (the SDK
    /// `vgui::MessageBox::PerformLayout` rules).
    pub fn layout(&self, measure: impl Fn(&str) -> f32) -> MessageBoxLayout {
        let text_w = measure(&self.text).ceil();
        let button = |label: &str| {
            vec2(
                (measure(label) + 10.).ceil().max(64.),
                (FONT_TALL + 10.).max(24.),
            )
        };
        let ok = button(&self.ok);
        let cancel = self.cancel.as_deref().map(button);
        let width = (text_w + 100.).max((ok.x + cancel.map_or(0., |c| c.x)) * 2. + 30.);
        let size = vec2(width, FONT_TALL + 100.);
        let right = size.x - CLIENT_INSET_X;
        let bottom = size.y - 6.;
        let y = bottom - ok.y - 15.;
        let text_at = vec2(
            (right / 2. - text_w / 2. + CLIENT_INSET_X).floor(),
            CLIENT_TOP + 5.,
        );
        let buttons = match cancel {
            None => vec![Rect::new(
                (right / 2. - ok.x / 2. + CLIENT_INSET_X).floor(),
                y,
                ok.x,
                ok.y,
            )],
            Some(cancel) => vec![
                Rect::new(
                    (right / 4. - ok.x / 2. + CLIENT_INSET_X).floor(),
                    y,
                    ok.x,
                    ok.y,
                ),
                Rect::new(
                    (3. * right / 4. - ok.x / 2. + CLIENT_INSET_X).floor(),
                    y,
                    cancel.x,
                    cancel.y,
                ),
            ],
        };
        MessageBoxLayout {
            size,
            text_at,
            buttons,
        }
    }

    /// Pointer in box-relative layout units. Returns `Some(true)` for OK, `Some(false)` for Cancel.
    pub fn input(
        &mut self,
        layout: &MessageBoxLayout,
        pointer: Option<Vec2>,
        click: bool,
        enter: bool,
    ) -> Option<bool> {
        self.armed = pointer.and_then(|p| layout.buttons.iter().position(|b| b.contains(p)));
        self.pressed = self.armed.filter(|_| click);
        if enter {
            return Some(true);
        }
        match self.pressed {
            Some(0) => Some(true),
            Some(_) => Some(false),
            None => None,
        }
    }

    pub fn draw(&self, painter: &Painter, layout: &MessageBoxLayout) {
        painter.frame(layout.size, &self.title);
        painter.text(&self.text, layout.text_at, painter.theme.text, false);
        let labels = std::iter::once(&self.ok).chain(self.cancel.as_ref());
        for (index, (rect, label)) in layout.buttons.iter().zip(labels).enumerate() {
            painter.button(
                *rect,
                label,
                ButtonState {
                    armed: self.armed == Some(index),
                    pressed: self.pressed == Some(index),
                    enabled: true,
                    default: index == 0,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameui::resources::fallback_game_menu;

    fn loc() -> Localization {
        let mut loc = Localization::default();
        for (token, text) in [
            ("GameUI_HalfLife2", "Half-Life 2"),
            ("GameUI_HalfLife2EpisodeOne", "HL2: Episode One"),
            ("GameUI_HalfLife2EpisodeTwo", "HL2: Episode Two"),
            ("GameUI_Extras", "Extras"),
            ("GameUI_GameMenu_Options", "Options"),
            ("GameUI_GameMenu_Quit", "Quit"),
            ("GameUI_GameMenu_NewGame", "New Game"),
            ("GameUI_GameMenu_LoadGame", "Load Game"),
            ("GameUI_GameMenu_Achievements", "Achievements"),
            ("GameUI_Back", "Back"),
            ("GameUI_QuitConfirmationTitle", "QUIT GAME"),
            (
                "GameUI_QuitConfirmationText",
                "Do you wish to stop playing now?",
            ),
            ("GameUI_Quit", "Quit game"),
            ("GameUI_Cancel", "Cancel"),
        ] {
            loc.insert(token, text);
        }
        loc
    }

    fn menu() -> GameMenu {
        let maps = BTreeMap::from([("hl2".to_owned(), "d1_trainstation_01".to_owned())]);
        GameMenu::new(fallback_game_menu(), maps, &loc())
    }

    fn labels(menu: &GameMenu) -> Vec<&str> {
        menu.entries().iter().map(|e| e.label.as_str()).collect()
    }

    #[test]
    fn main_menu_lists_games_spacer_extras_options_quit() {
        let menu = menu();
        assert_eq!(
            labels(&menu),
            [
                "HALF-LIFE 2",
                "HL2: EPISODE ONE",
                "HL2: EPISODE TWO",
                "",
                "EXTRAS",
                "OPTIONS",
                "QUIT"
            ]
        );
        assert!(menu.entries()[3].spacer);
        assert!(!menu.entries()[4].enabled, "Extras is a placeholder");
        assert!(menu.entries()[5].enabled && menu.entries()[6].enabled);
    }

    #[test]
    fn game_categories_have_new_load_achievements_spacer_options_back() {
        let loc = loc();
        for (index, game, new_game_enabled) in
            [(0, "hl2", true), (1, "episodic", false), (2, "ep2", false)]
        {
            let mut menu = menu();
            assert_eq!(menu.activate(index, &loc), MenuOutcome::None);
            assert_eq!(menu.category(), game);
            assert_eq!(
                labels(&menu),
                [
                    "NEW GAME",
                    "LOAD GAME",
                    "ACHIEVEMENTS",
                    "",
                    "OPTIONS",
                    "BACK"
                ]
            );
            assert_eq!(menu.entries()[0].enabled, new_game_enabled, "{game}");
            assert!(
                !menu.entries()[1].enabled && !menu.entries()[2].enabled,
                "placeholders"
            );
            assert_eq!(menu.activate(4, &loc), MenuOutcome::OpenOptions);
            assert_eq!(menu.activate(5, &loc), MenuOutcome::None);
            assert_eq!(
                menu.category(),
                MAIN_CATEGORY,
                "Back returns to the main category"
            );
        }
    }

    #[test]
    fn activation_outcomes_and_placeholders() {
        let loc = loc();
        let mut menu = menu();
        assert_eq!(menu.activate(6, &loc), MenuOutcome::ConfirmQuit);
        assert_eq!(
            menu.activate(4, &loc),
            MenuOutcome::Placeholder("EXTRAS".into())
        );
        assert_eq!(menu.activate(3, &loc), MenuOutcome::None, "spacer");
        menu.activate(0, &loc);
        assert_eq!(
            menu.activate(0, &loc),
            MenuOutcome::NewGame("d1_trainstation_01".into())
        );
        assert!(menu.back(&loc));
        assert!(!menu.back(&loc), "main category has nothing to go back to");
    }

    #[test]
    fn keyboard_selection_skips_spacers_and_disabled_rows() {
        let mut menu = menu();
        menu.step(1);
        assert_eq!(menu.selection, Some(0));
        menu.step(1);
        menu.step(1);
        menu.step(1);
        assert_eq!(menu.selection, Some(5), "spacer and Extras skipped");
        menu.step(1);
        menu.step(1);
        assert_eq!(menu.selection, Some(0), "wraps around");
        menu.step(-1);
        assert_eq!(menu.selection, Some(6));
    }

    #[test]
    fn in_game_menu_uses_uncategorized_entries_in_order() {
        let loc = loc();
        let mut menu = menu();
        menu.set_in_game(true, &loc);
        let commands: Vec<_> = menu.entries().iter().map(|e| e.command.clone()).collect();
        assert_eq!(commands.first(), Some(&MenuCommand::Resume));
        assert_eq!(commands.last(), Some(&MenuCommand::Quit));
        assert!(commands.contains(&MenuCommand::ReturnToMainMenu));
        assert!(!menu.back(&loc));
        menu.set_in_game(false, &loc);
        assert_eq!(menu.category(), MAIN_CATEGORY);
    }

    #[test]
    fn quit_box_uses_tokens_and_sdk_message_box_geometry() {
        let mut quit = MessageBox::quit(&loc());
        assert_eq!(quit.title, "QUIT GAME");
        assert_eq!(quit.text, "Do you wish to stop playing now?");
        assert_eq!(quit.cancel.as_deref(), Some("Cancel"));
        let measure = |s: &str| s.len() as f32 * 6.;
        let layout = quit.layout(measure);
        assert_eq!(layout.size, vec2(32. * 6. + 100., 116.));
        assert_eq!(layout.buttons.len(), 2);
        assert!(layout.buttons[0].x < layout.buttons[1].x);
        assert_eq!(layout.buttons[0].y + layout.buttons[0].h, 116. - 6. - 15.);
        assert!(
            (layout.text_at.x + 96. - layout.size.x / 2.).abs() <= 4.,
            "message centered"
        );
        let center = |r: Rect| Some(vec2(r.x + 2., r.y + 2.));
        assert_eq!(
            quit.input(&layout, center(layout.buttons[1]), true, false),
            Some(false)
        );
        assert_eq!(
            quit.input(&layout, center(layout.buttons[0]), true, false),
            Some(true)
        );
        assert_eq!(
            quit.input(&layout, center(layout.buttons[0]), false, false),
            None
        );
        assert_eq!(
            quit.input(&layout, None, false, true),
            Some(true),
            "Enter accepts"
        );
    }
}
