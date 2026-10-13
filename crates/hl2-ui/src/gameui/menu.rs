//! The game menu (main menu, its HL2/Episode categories and the in-game menu) and the VGUI
//! message box used for the quit confirmation.
//!
//! Entries come from `GameMenu.res`. Out of game the current `MenuCategory` is shown in file
//! order; in game the uncategorized `OnlyInGame` entries are shown sorted by `InGameOrder`.
//! Commands without a Rust implementation (Load/Save Game, Achievements, Workshop, New Game for a
//! game whose maps are not mounted) are shown disabled, so they are visibly placeholders.

use super::localize::Localization;
use super::resources::{MenuItem, Visibility};
use super::widgets::{
    close_button_rect, ButtonState, ButtonTracker, Painter, CLIENT_INSET_X, CLIENT_TOP, FONT_TALL,
};
use crate::canvas::Rect;
use glam::{vec2, Vec2};
use source_assets::keyvalues::Entry;
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
    Workshop,
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
            "openworkshopdialog" => Self::Workshop,
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
    pub kind: super::widgets::WidgetKind,
    pub rollover_sound: Option<&'static str>,
}

impl MenuEntry {
    /// Returns whether this menu entry is eligible to play a rollover sound.
    /// Only enabled non-spacer menu buttons play rollover sounds.
    pub fn has_rollover_sound(&self) -> bool {
        self.kind.has_rollover_sound()
            && self.rollover_sound.is_some()
            && self.enabled
            && !self.spacer
    }
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
    /// Switched to an expansion submenu (e.g. "hl2", "episodic", "ep2").
    SwitchExpansion(String),
    /// Switched to a category submenu (e.g. "main", "extras").
    SwitchCategory(String),
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
                let enabled = !item.is_spacer() && self.implemented(&command);
                let (kind, rollover_sound) = if item.is_spacer() {
                    (super::widgets::WidgetKind::Other, None)
                } else {
                    (
                        super::widgets::WidgetKind::MenuButton,
                        Some("ui/buttonrollover.wav"),
                    )
                };
                MenuEntry {
                    enabled,
                    spacer: item.is_spacer(),
                    label,
                    command,
                    kind,
                    rollover_sound,
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
            // The Extras category opens the extras submenu (Lost Coast, Workshop placeholder).
            MenuCommand::SwitchCategory(category) => {
                category == MAIN_CATEGORY
                    || self
                        .items
                        .iter()
                        .any(|i| i.category.as_deref() == Some(category.as_str()))
            }
            MenuCommand::NewGame(game) => self.new_game_maps.contains_key(game),
            MenuCommand::OpenOptions
            | MenuCommand::Quit
            | MenuCommand::Resume
            | MenuCommand::ReturnToMainMenu => true,
            MenuCommand::LoadGame
            | MenuCommand::SaveGame
            | MenuCommand::Achievements
            | MenuCommand::Workshop
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
                MenuOutcome::SwitchExpansion(game)
            }
            MenuCommand::SwitchCategory(category) => {
                self.set_category(&category, loc);
                MenuOutcome::SwitchCategory(category)
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
    pub armed: Option<usize>,
    pub pressed: Option<usize>,
    pub has_close: bool,
    last_armed: Option<usize>,
    sounds: Vec<String>,
    tracker: ButtonTracker<usize>,
}

/// Message box geometry in layout units relative to the box.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageBoxLayout {
    pub size: Vec2,
    pub text_at: Vec2,
    pub buttons: Vec<Rect>,
    pub close: Option<Rect>,
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
            has_close: false,
            last_armed: None,
            sounds: Vec::new(),
            tracker: ButtonTracker::new(),
        }
    }

    /// Configures whether the message box displays a close (X) button in the title bar.
    pub fn with_close(mut self, has_close: bool) -> Self {
        self.has_close = has_close;
        self
    }

    /// Drains any queued UI sound cues (e.g. rollover or release activation).
    pub fn take_sounds(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sounds)
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

    /// The Return to Main Menu confirmation dialog (`#GameUI_ConfirmMainMenu_Title`, `#GameUI_MainMenuWarning`, `#GameUI_OK`, `#GameUI_Cancel`).
    pub fn return_to_main_menu(loc: &Localization) -> Self {
        Self::new(
            loc.text("#GameUI_ConfirmMainMenu_Title"),
            loc.text("#GameUI_MainMenuWarning"),
            loc.text("#GameUI_OK"),
            Some(loc.text("#GameUI_Cancel")),
        )
    }

    /// Box size from the message width plus 100 in both directions, at least wide enough for twice
    /// the buttons plus 30; buttons sit at the quarter points, 15 above the client bottom (the SDK
    /// `vgui::MessageBox::PerformLayout` rules).
    pub fn layout(&self, measure: impl Fn(&str) -> f32) -> MessageBoxLayout {
        let lines: Vec<&str> = self.text.lines().collect();
        let line_count = lines.len().max(1);
        let text_w = lines.iter().map(|l| measure(l)).fold(0.0, f32::max).ceil();
        let button = |label: &str| {
            vec2(
                (measure(label) + 10.).ceil().max(64.),
                (FONT_TALL + 10.).max(24.),
            )
        };
        let ok = button(&self.ok);
        let cancel = self.cancel.as_deref().map(button);
        let width = (text_w + 100.).max((ok.x + cancel.map_or(0., |c| c.x)) * 2. + 30.);
        let size = vec2(width, FONT_TALL * line_count as f32 + 100.);
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
        let close = self.has_close.then(|| close_button_rect(size));
        MessageBoxLayout {
            size,
            text_at,
            buttons,
            close,
        }
    }

    /// Pointer in box-relative layout units. Returns `Some(true)` for OK, `Some(false)` for Cancel.
    pub fn input(
        &mut self,
        layout: &MessageBoxLayout,
        pointer: Option<Vec2>,
        down: bool,
        click: bool,
        released: bool,
        enter: bool,
    ) -> Option<bool> {
        let hovered = pointer.and_then(|p| {
            layout
                .buttons
                .iter()
                .position(|b| b.contains(p))
                .or_else(|| {
                    layout
                        .close
                        .and_then(|c| c.contains(p).then_some(layout.buttons.len()))
                })
        });
        self.armed = hovered;
        self.last_armed = hovered;
        let activated = self.tracker.update(hovered, down, click, released);
        self.pressed = self.tracker.pressed();
        let enter_activated = enter.then_some(0);
        let final_activated = activated.or(enter_activated);
        if final_activated.is_some() {
            self.sounds.push("ui/buttonclickrelease.wav".to_string());
        }
        match final_activated {
            Some(0) => Some(true),
            Some(_) => Some(false),
            None => None,
        }
    }

    pub fn draw(&self, painter: &Painter, layout: &MessageBoxLayout) {
        let close_state = layout.close.map(|_| {
            let close_index = layout.buttons.len();
            ButtonState {
                armed: self.armed == Some(close_index),
                pressed: self.pressed == Some(close_index),
                enabled: true,
                default: false,
            }
        });
        painter.frame_with_close(layout.size, &self.title, close_state);
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

/// Reference design vertical resolution for Source UI coordinates (480p).
pub const BASE_SCREEN_HEIGHT: f32 = 480.;
/// Default X offset for the main menu title in 480p units (`Main.Title1.X`).
pub const DEFAULT_TITLE_X: f32 = 76.;
/// Default Y offset for the main menu title in 480p units (`Main.Title1.Y`).
pub const DEFAULT_TITLE_Y: f32 = 145.;
/// Default X offset for the submenu secondary title in 480p units (`Main.Title2.X`).
pub const DEFAULT_TITLE2_X: f32 = 76.;
/// Default Y offset for the submenu secondary title in 480p units (`Main.Title2.Y`).
pub const DEFAULT_TITLE2_Y: f32 = 172.;
/// Default X offset for the menu buttons in 480p units (`Main.Menu.X`).
pub const DEFAULT_MENU_X: f32 = 76.;
/// Default Y offset for the menu buttons in 480p units (`Main.Menu.Y`).
pub const DEFAULT_MENU_Y: f32 = 240.;
/// Default title text height in 480p units.
pub const DEFAULT_TITLE_TALL: f32 = 32.;
/// Default item height from scheme (`MainMenu.MenuItemHeight`).
pub const DEFAULT_ITEM_TALL: f32 = 26.;
/// Default menu font cell height.
pub const DEFAULT_FONT_TALL: f32 = 11.;
/// Base vertical spacing between menu buttons in 480p units (`BASE_SCREEN_HEIGHT`).
/// At 1280x800 reference resolution (scale 5/3), this evaluates to exactly 20.0 pixels, matching
/// original Source GameUI proportions while remaining resolution and DPI agnostic across scales.
pub const BASE_MENU_BUTTON_SPACING: f32 = 12.;
/// Vertical spacing between menu buttons at standard 1280x800 reference resolution.
pub const MENU_BUTTON_SPACING: f32 = 20.;

/// Computes the button spacing for a given 480p scale factor.
pub fn button_spacing(scale: f32) -> f32 {
    (BASE_MENU_BUTTON_SPACING * scale).round()
}
/// Base hit area width for menu items in 480p units.
pub const MENU_ROW_BASE_WIDTH: f32 = 250.;

/// Layout parameters for the GameMenu's title and items, loaded from schemes or defaulting to desktop coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuLayout {
    pub title_x: f32,
    pub title_y: f32,
    pub title2_x: f32,
    pub title2_y: f32,
    pub menu_x: f32,
    pub menu_y: f32,
    pub title_tall: f32,
    pub item_tall: f32,
    pub font_ranges: Vec<(f32, f32, f32)>,
    pub font_tall: f32,
}

impl Default for MenuLayout {
    fn default() -> Self {
        // Stock desktop defaults, overridden by the mounted schemes below.
        Self {
            title_x: DEFAULT_TITLE_X,
            title_y: DEFAULT_TITLE_Y,
            title2_x: DEFAULT_TITLE2_X,
            title2_y: DEFAULT_TITLE2_Y,
            menu_x: DEFAULT_MENU_X,
            menu_y: DEFAULT_MENU_Y,
            title_tall: DEFAULT_TITLE_TALL,
            item_tall: DEFAULT_ITEM_TALL,
            font_ranges: vec![(1., 1080., 16.)],
            font_tall: DEFAULT_FONT_TALL,
        }
    }
}

/// Scaled screen coordinates and sizes for rendering and hit-testing the GameMenu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuGeometry {
    pub title: Vec2,
    pub title2: Vec2,
    pub menu: Vec2,
    pub title_tall: f32,
    pub font_tall: f32,
    pub item_tall: f32,
}

impl MenuGeometry {
    /// Canvas hit rectangle for menu row `index`.
    pub fn row_rect(&self, index: usize, canvas_height: f32) -> Rect {
        Rect::new(
            self.menu.x,
            self.menu.y + index as f32 * self.item_tall,
            MENU_ROW_BASE_WIDTH * canvas_height / BASE_SCREEN_HEIGHT,
            self.item_tall,
        )
    }

    /// Hit rectangles for all menu rows up to `count`.
    pub fn row_rects(&self, count: usize, canvas_height: f32) -> Vec<Rect> {
        (0..count)
            .map(|i| self.row_rect(i, canvas_height))
            .collect()
    }

    /// Canvas position for vertically centering the text inside row `index`.
    pub fn row_text_pos(&self, index: usize) -> Vec2 {
        self.menu
            + vec2(
                0.,
                (index as f32 * self.item_tall + (self.item_tall - self.font_tall) * 0.5).floor(),
            )
    }
}

impl MenuLayout {
    pub fn geometry(&self, height: f32) -> MenuGeometry {
        self.geometry_scaled(height, 1.0)
    }

    /// Computes geometry taking into account logical height and display scale factor.
    pub fn geometry_scaled(&self, height: f32, scale_factor: f32) -> MenuGeometry {
        let effective_height = height * scale_factor.max(1.0);
        let scale = effective_height / BASE_SCREEN_HEIGHT;
        let font_tall = self
            .font_ranges
            .iter()
            .find(|(low, high, _)| effective_height >= *low && effective_height <= *high)
            .map_or_else(|| (self.font_tall * scale).round(), |(_, _, tall)| *tall);
        // Vertical stack of buttons with proportional spacing so they do not squish on high-DPI screens.
        let item_tall = self.item_tall.max(font_tall + button_spacing(scale));
        MenuGeometry {
            title: vec2(
                (self.title_x * scale).trunc(),
                (self.title_y * scale).trunc(),
            ),
            title2: vec2(
                (self.title2_x * scale).trunc(),
                (self.title2_y * scale).trunc(),
            ),
            menu: vec2((self.menu_x * scale).trunc(), (self.menu_y * scale).trunc()),
            title_tall: (self.title_tall * scale).round(),
            font_tall,
            item_tall,
        }
    }

    pub fn apply_source_scheme(&mut self, root: &Entry) {
        if let Some(tall) = resource_number(root, "MainMenu.MenuItemHeight")
            .filter(|value| *value > 0. && *value <= 128.)
        {
            self.item_tall = tall;
        }
        if let Some(fonts) =
            resource_font(root, "MainMenuFont").or_else(|| resource_font(root, "MenuLarge"))
        {
            self.font_ranges.clear();
            for (tall, range) in fonts {
                if let Some((low, high)) = range {
                    self.font_ranges.push((low, high, tall));
                } else {
                    self.font_tall = tall;
                }
            }
        }
    }

    pub fn apply_client_scheme(&mut self, root: &Entry) {
        for (key, target) in [
            ("Main.Title1.X", &mut self.title_x),
            ("Main.Title1.Y", &mut self.title_y),
            ("Main.Title2.X", &mut self.title2_x),
            ("Main.Title2.Y", &mut self.title2_y),
            ("Main.Menu.X", &mut self.menu_x),
            ("Main.Menu.Y", &mut self.menu_y),
        ] {
            if let Some(value) = resource_number(root, key) {
                *target = value;
            }
        }
        if let Some(fonts) = resource_font(root, "ClientTitleFont") {
            self.title_tall = fonts[0].0;
        }
    }
}

pub fn resource_number(root: &Entry, key: &str) -> Option<f32> {
    root.get("BaseSettings")?
        .get(key)?
        .text()?
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

pub type FontRule = (f32, Option<(f32, f32)>);

pub fn resource_font(root: &Entry, name: &str) -> Option<Vec<FontRule>> {
    let entries = root.get("Fonts")?.get(name)?.children();
    let values: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            let tall = entry.get("tall")?.text()?.parse::<f32>().ok()?;
            if !tall.is_finite() || tall <= 0. || tall > 256. {
                return None;
            }
            let range = entry.get("yres").and_then(|y| y.text()).and_then(|text| {
                let mut parts = text.split_whitespace();
                let low = parts.next()?.parse::<f32>().ok()?;
                let high = parts.next()?.parse::<f32>().ok()?;
                (low.is_finite() && high.is_finite() && low <= high).then_some((low, high))
            });
            Some((tall, range))
        })
        .collect();
    (!values.is_empty()).then_some(values)
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
            ("GameUI_HalfLife2LostCoast", "Half-Life 2: Lost Coast"),
            ("GameUI_Worskhop_MenuName", "Workshop"),
            ("GameUI_Workshop_MenuName", "Workshop"),
            ("GameUI_LostCoast", "Lost Coast"),
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
        assert!(menu.entries()[4].enabled, "Extras opens the extras submenu");
        assert!(menu.entries()[5].enabled && menu.entries()[6].enabled);
    }

    #[test]
    fn game_categories_have_new_load_achievements_spacer_options_back() {
        let loc = loc();
        for (index, game, new_game_enabled) in
            [(0, "hl2", true), (1, "episodic", false), (2, "ep2", false)]
        {
            let mut menu = menu();
            assert_eq!(
                menu.activate(index, &loc),
                MenuOutcome::SwitchExpansion(game.into())
            );
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
            assert_eq!(
                menu.activate(5, &loc),
                MenuOutcome::SwitchCategory(MAIN_CATEGORY.into())
            );
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
        assert_eq!(menu.activate(3, &loc), MenuOutcome::None, "spacer");
        assert_eq!(
            menu.activate(4, &loc),
            MenuOutcome::SwitchCategory("extras".into())
        );
        assert_eq!(menu.category(), "extras");
        assert_eq!(
            menu.activate(1, &loc),
            MenuOutcome::Placeholder("WORKSHOP".into())
        );
        assert!(menu.back(&loc));
        assert_eq!(menu.category(), MAIN_CATEGORY);
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
        assert_eq!(menu.selection, Some(1));
        menu.step(1);
        assert_eq!(menu.selection, Some(2));
        menu.step(1);
        assert_eq!(menu.selection, Some(4), "spacer skipped, Extras enabled");
        menu.step(1);
        assert_eq!(menu.selection, Some(5));
        menu.step(1);
        assert_eq!(menu.selection, Some(6));
        menu.step(1);
        assert_eq!(menu.selection, Some(0), "wraps around");
        menu.step(-1);
        assert_eq!(menu.selection, Some(6));
    }

    #[test]
    fn extras_submenu_and_lostcoast_game_submenu() {
        let loc = loc();
        let maps = BTreeMap::from([
            ("hl2".to_owned(), "d1_trainstation_01".to_owned()),
            ("lostcoast".to_owned(), "d2_lostcoast".to_owned()),
        ]);
        let mut menu = GameMenu::new(fallback_game_menu(), maps, &loc);

        // Main menu EXTRAS button opens extras submenu
        assert_eq!(
            menu.activate(4, &loc),
            MenuOutcome::SwitchCategory("extras".into())
        );
        assert_eq!(menu.category(), "extras");
        assert_eq!(
            labels(&menu),
            ["HALF-LIFE 2: LOST COAST", "WORKSHOP", "", "BACK"]
        );
        assert!(menu.entries()[0].enabled, "Lost Coast entry enabled");
        assert!(
            !menu.entries()[1].enabled,
            "Workshop button is disabled placeholder"
        );
        assert!(menu.entries()[2].spacer, "spacer");
        assert!(menu.entries()[3].enabled, "Back button enabled");

        // Activating Workshop returns placeholder outcome
        assert_eq!(
            menu.activate(1, &loc),
            MenuOutcome::Placeholder("WORKSHOP".into())
        );

        // Activating Lost Coast entry opens lostcoast game submenu
        assert_eq!(
            menu.activate(0, &loc),
            MenuOutcome::SwitchExpansion("lostcoast".into())
        );
        assert_eq!(menu.category(), "lostcoast");
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
        assert!(menu.entries()[0].enabled, "New Game enabled for lostcoast");
        assert!(!menu.entries()[1].enabled, "Load Game placeholder");
        assert!(!menu.entries()[2].enabled, "Achievements placeholder");
        assert!(menu.entries()[3].spacer, "spacer");
        assert!(menu.entries()[4].enabled, "Options enabled");
        assert!(menu.entries()[5].enabled, "Back enabled");

        // New game in lostcoast starts d2_lostcoast
        assert_eq!(
            menu.activate(0, &loc),
            MenuOutcome::NewGame("d2_lostcoast".into())
        );

        // Options opens options
        assert_eq!(menu.activate(4, &loc), MenuOutcome::OpenOptions);

        // Back returns to main category
        assert_eq!(
            menu.activate(5, &loc),
            MenuOutcome::SwitchCategory(MAIN_CATEGORY.into())
        );
        assert_eq!(menu.category(), MAIN_CATEGORY);

        // Re-entering extras and testing Escape
        assert_eq!(
            menu.activate(4, &loc),
            MenuOutcome::SwitchCategory("extras".into())
        );
        assert_eq!(menu.category(), "extras");
        assert!(menu.back(&loc));
        assert_eq!(menu.category(), MAIN_CATEGORY);
    }

    #[test]
    fn extras_keyboard_selection_skips_workshop_placeholder_and_spacer() {
        let loc = loc();
        let mut menu = menu();
        menu.activate(4, &loc);
        assert_eq!(menu.category(), "extras");
        menu.step(1);
        assert_eq!(menu.selection, Some(0)); // HALF-LIFE 2: LOST COAST
        menu.step(1);
        assert_eq!(
            menu.selection,
            Some(3),
            "skips disabled Workshop (1) and spacer (2)"
        );
        menu.step(1);
        assert_eq!(menu.selection, Some(0), "wraps around to Lost Coast");
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

        // 1. Cancel button: press down -> pressed state active, no activation yet
        assert_eq!(
            quit.input(&layout, center(layout.buttons[1]), true, true, false, false),
            None
        );
        assert_eq!(quit.pressed, Some(1));

        // Release over Cancel button -> activates and returns Some(false)
        assert_eq!(
            quit.input(
                &layout,
                center(layout.buttons[1]),
                false,
                false,
                true,
                false
            ),
            Some(false)
        );
        assert_eq!(quit.pressed, None);

        // 2. OK button: press down and release
        assert_eq!(
            quit.input(&layout, center(layout.buttons[0]), true, true, false, false),
            None
        );
        assert_eq!(quit.pressed, Some(0));
        assert_eq!(
            quit.input(
                &layout,
                center(layout.buttons[0]),
                false,
                false,
                true,
                false
            ),
            Some(true)
        );
        assert_eq!(quit.pressed, None);

        // 3. Cancel on move away:
        assert_eq!(
            quit.input(&layout, center(layout.buttons[0]), true, true, false, false),
            None
        );
        assert_eq!(quit.pressed, Some(0));
        // Move pointer away while button is held
        assert_eq!(quit.input(&layout, None, true, false, false, false), None);
        assert_eq!(quit.pressed, None, "moving away cancels pressed state");
        // Release after move away does not trigger
        assert_eq!(quit.input(&layout, None, false, false, true, false), None);

        // 4. Enter accepts immediately
        assert_eq!(
            quit.input(&layout, None, false, false, false, true),
            Some(true),
            "Enter accepts"
        );
    }

    #[test]
    fn return_to_main_menu_multiline_layout() {
        let mut loc = Localization::default();
        loc.insert("GameUI_ConfirmMainMenu_Title", "RETURN TO MAIN MENU");
        loc.insert(
            "GameUI_MainMenuWarning",
            "Are you sure?\\nAll unsaved progress will be lost.",
        );
        loc.insert("GameUI_OK", "OK");
        loc.insert("GameUI_Cancel", "Cancel");
        let msg_box = MessageBox::return_to_main_menu(&loc);
        assert_eq!(
            msg_box.text,
            "Are you sure?\nAll unsaved progress will be lost."
        );
        let measure = |s: &str| s.len() as f32 * 6.;
        let layout = msg_box.layout(measure);
        // Height expands for 2 lines: 2 * FONT_TALL (16) + 100 = 132
        assert_eq!(layout.size.y, 132.);
    }

    #[test]
    fn native_menu_uses_480_coordinate_scaling_with_proportional_spacing() {
        let layout = MenuLayout::default();
        let at_720 = layout.geometry(720.);
        assert_eq!(at_720.title, vec2(114., 217.));
        assert_eq!(at_720.title2, vec2(114., 258.));
        assert_eq!(at_720.menu, vec2(114., 360.));
        assert_eq!(at_720.title_tall, 48.);
        assert_eq!(at_720.item_tall, 34.);
        assert_eq!(at_720.font_tall, 16.);
        let at_480 = layout.geometry(480.);
        assert_eq!(at_480.title, vec2(76., 145.));
        assert_eq!(at_480.title2, vec2(76., 172.));
        assert_eq!(at_480.menu, vec2(76., 240.));
        assert_eq!(at_480.font_tall, 16.);
        assert_eq!(at_480.title_tall, 32.);
        assert_eq!(at_480.item_tall, 28.);
        assert_eq!(layout.geometry(1440.).font_tall, 33.);
        assert_eq!(layout.geometry(1440.).item_tall, 69.);

        // Regression assertion: 1280x800 layout values are exactly unchanged
        let at_800 = layout.geometry(800.);
        assert_eq!(at_800.font_tall, 16.);
        assert_eq!(at_800.item_tall, 36.);
        assert_eq!(button_spacing(800. / BASE_SCREEN_HEIGHT), 20.);
        assert_eq!(at_800.title, vec2(126., 241.));
        assert_eq!(at_800.title2, vec2(126., 286.));
        assert_eq!(at_800.menu, vec2(126., 400.));
        assert_eq!(at_800.title_tall, 53.);

        // High-DPI / Retina consistency: scale factor 2.0 at 800 logical height doubles spacing
        let at_retina = layout.geometry_scaled(800., 2.0);
        assert_eq!(at_retina, layout.geometry(1600.));
        assert_eq!(at_retina.font_tall, 37.);
        assert_eq!(button_spacing(1600. / BASE_SCREEN_HEIGHT), 40.);
        assert_eq!(at_retina.item_tall, 77.);

        // Verify row rects and text position helpers
        let rects = at_480.row_rects(3, 480.);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], Rect::new(76., 240., 250., 28.));
        assert_eq!(rects[1], Rect::new(76., 268., 250., 28.));
        assert_eq!(at_480.row_text_pos(0), vec2(76., 246.));

        // Regression assertion: 800p row rect height is 36. and text is centered with 20px spacing
        let rects_800 = at_800.row_rects(2, 800.);
        assert_eq!(rects_800[0].h, 36.);
        assert_eq!(rects_800[1].h, 36.);
        assert_eq!(at_800.row_text_pos(0), vec2(126., 410.));
    }

    #[test]
    fn menu_resources_resolve_desktop_colors_ranges_and_reject_bad_heights() {
        use crate::canvas::WHITE;
        use crate::gameui::widgets::scheme_color;
        let resource = source_assets::keyvalues::parse_resource(br#"Scheme {
            Colors { White "255 255 255 255" } BaseSettings { MainMenu.TextColor White Main.Menu.X 76 }
            Fonts { MenuLarge { 1 { tall 16 yres "1 1080" } [$WINDOWS]
                1 { tall 24 yres "1 1080" } [$LINUX] 2 { tall 11 } 3 { tall NaN } } }
        }"#).unwrap();
        let root = &resource[0];
        assert_eq!(scheme_color(root, "MainMenu.TextColor"), Some(WHITE));
        assert_eq!(resource_number(root, "Main.Menu.X"), Some(76.));
        let ranges = resource_font(root, "MenuLarge").unwrap();
        #[cfg(target_os = "windows")]
        assert_eq!(ranges, vec![(16., Some((1., 1080.))), (11., None)]);
        #[cfg(target_os = "linux")]
        assert_eq!(ranges, vec![(24., Some((1., 1080.))), (11., None)]);
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        assert_eq!(ranges, vec![(11., None)]);
        assert!(resource_font(root, "Absent").is_none());
    }

    #[test]
    fn message_box_sound_events_hover_and_release() {
        let mut quit = MessageBox::new(
            "Title".into(),
            "Text".into(),
            "OK".into(),
            Some("Cancel".into()),
        );
        let layout = quit.layout(|_| 100.);
        let center = |r: Rect| Some(vec2(r.x + r.w / 2., r.y + r.h / 2.));

        // 1. Moving from outside to over button 0 emits no rollover sound (dialog button)
        quit.input(
            &layout,
            center(layout.buttons[0]),
            false,
            false,
            false,
            false,
        );
        assert!(quit.take_sounds().is_empty());

        // 2. Staying over button 0 on next frame does not emit sound
        quit.input(
            &layout,
            center(layout.buttons[0]),
            false,
            false,
            false,
            false,
        );
        assert!(quit.take_sounds().is_empty());

        // 3. Moving to button 1 emits no rollover sound
        quit.input(
            &layout,
            center(layout.buttons[1]),
            false,
            false,
            false,
            false,
        );
        assert!(quit.take_sounds().is_empty());

        // 4. Moving outside emits nothing
        quit.input(&layout, None, false, false, false, false);
        assert!(quit.take_sounds().is_empty());

        // 5. Press and release on button 0 emits release sound on activation (no rollover on entry)
        quit.input(&layout, center(layout.buttons[0]), true, true, false, false);
        assert!(quit.take_sounds().is_empty());
        quit.input(
            &layout,
            center(layout.buttons[0]),
            false,
            false,
            true,
            false,
        );
        assert_eq!(quit.take_sounds(), vec!["ui/buttonclickrelease.wav"]);

        // 6. Press, move away, and release emits no release sound
        quit.input(&layout, center(layout.buttons[0]), true, true, false, false);
        let _ = quit.take_sounds();
        quit.input(&layout, None, true, false, false, false);
        quit.input(&layout, None, false, false, true, false);
        assert!(quit.take_sounds().is_empty());
    }

    #[test]
    fn message_box_close_button_data_driven_cancel() {
        let mut msg_box = MessageBox::new(
            "Title".into(),
            "Text".into(),
            "OK".into(),
            Some("Cancel".into()),
        );
        // By default, MessageBox has no close button (matches Source SDK SetCloseButtonVisible(false))
        assert!(!msg_box.has_close);
        let layout_no_close = msg_box.layout(|_| 100.);
        assert_eq!(layout_no_close.close, None);

        // Data-driven close button enabled
        msg_box = msg_box.with_close(true);
        assert!(msg_box.has_close);
        let layout_with_close = msg_box.layout(|_| 100.);
        let close_rect = layout_with_close.close.expect("close button rect");
        assert_eq!(close_rect, close_button_rect(layout_with_close.size));

        let center = |r: Rect| Some(vec2(r.x + r.w / 2., r.y + r.h / 2.));

        // Hover over close button: does not emit rollover sound (dialog button)
        msg_box.input(
            &layout_with_close,
            center(close_rect),
            false,
            false,
            false,
            false,
        );
        assert!(msg_box.take_sounds().is_empty());

        // Click and release on close button: activates Cancel outcome (Some(false))
        msg_box.input(
            &layout_with_close,
            center(close_rect),
            true,
            true,
            false,
            false,
        );
        let outcome = msg_box.input(
            &layout_with_close,
            center(close_rect),
            false,
            false,
            true,
            false,
        );
        assert_eq!(outcome, Some(false), "Close button behaves like Cancel");
        assert_eq!(msg_box.take_sounds(), vec!["ui/buttonclickrelease.wav"]);
    }

    #[test]
    fn rollover_sound_eligibility_only_for_menu_buttons() {
        use super::super::layout::ControlKind;
        use super::super::widgets::WidgetKind;

        // Eligibility check on WidgetKind
        assert!(
            WidgetKind::MenuButton.has_rollover_sound(),
            "menu button yes"
        );
        assert!(
            !WidgetKind::DialogButton.has_rollover_sound(),
            "dialog button no"
        );
        assert!(!WidgetKind::RadioButton.has_rollover_sound(), "radio no");
        assert!(!WidgetKind::Dropdown.has_rollover_sound(), "dropdown no");
        assert!(!WidgetKind::TextInput.has_rollover_sound(), "text input no");

        // Eligibility check on ControlKind
        assert!(
            !ControlKind::Button.widget_kind().has_rollover_sound(),
            "dialog button no"
        );
        assert!(
            !ControlKind::RadioButton.widget_kind().has_rollover_sound(),
            "radio no"
        );
        assert!(
            !ControlKind::ComboBox.widget_kind().has_rollover_sound(),
            "dropdown no"
        );
        assert!(
            !ControlKind::TextEntry.widget_kind().has_rollover_sound(),
            "text input no"
        );

        // Eligibility check on MenuEntry
        let menu = menu();
        let main_entries = menu.entries();
        assert!(
            main_entries[0].has_rollover_sound(),
            "enabled menu button has rollover sound"
        );
        assert!(
            !main_entries[3].has_rollover_sound(),
            "spacer has no rollover sound"
        );
    }
}
