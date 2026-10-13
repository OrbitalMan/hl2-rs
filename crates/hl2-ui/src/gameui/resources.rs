//! Loads every installed resource the GameUI needs, once, outside frame systems.
//!
//! Search order mirrors the anniversary install: loose files under `hl2_complete/` first (its
//! `gamemenu.res` and `merged_loc_all.txt` define the combined HL2/Episode menu), then the mounted
//! `hl2`/`platform` VFS. The VFS mount itself is not changed, so world/HUD content is unaffected.
//! Missing files leave the matching field empty; callers fall back to token names and built-in
//! menu structure rather than failing.

use super::{
    keys::{self, KeyRow},
    layout::Layout,
    localize::Localization,
};
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which game state a menu entry is shown in (`OnlyInGame` / `NotInGame` keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Always,
    OnlyInGame,
    NotInGame,
}

/// One `GameMenu.res` entry. An empty label is a spacer row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: String,
    pub command: String,
    /// `MenuCategory`; `None` for the in-game (pause) entries.
    pub category: Option<String>,
    pub order: i32,
    pub visibility: Visibility,
    /// `Caps`: the label is drawn upper-case.
    pub caps: bool,
}

impl MenuItem {
    pub fn is_spacer(&self) -> bool {
        self.label.is_empty()
    }
}

/// Parses `GameMenu.res`, keeping file order.
pub fn parse_game_menu(bytes: &[u8]) -> Option<Vec<MenuItem>> {
    let roots = keyvalues::parse_resource(bytes).ok()?;
    let root = roots.iter().find(|e| !e.children().is_empty())?;
    let items = root
        .children()
        .iter()
        .filter(|e| !e.children().is_empty())
        .map(|entry| {
            let text = |key: &str| entry.get(key).and_then(|e| e.text()).unwrap_or_default();
            let flag = |key: &str| text(key) == "1";
            MenuItem {
                label: text("label").trim().to_owned(),
                command: text("command").trim().to_owned(),
                category: Some(text("MenuCategory").trim().to_ascii_lowercase())
                    .filter(|c| !c.is_empty()),
                order: text("InGameOrder").trim().parse().unwrap_or(0),
                caps: flag("Caps"),
                visibility: if flag("OnlyInGame") {
                    Visibility::OnlyInGame
                } else if flag("NotInGame") {
                    Visibility::NotInGame
                } else {
                    Visibility::Always
                },
            }
        })
        .collect::<Vec<_>>();
    (!items.is_empty()).then_some(items)
}

/// Menu structure used only when no `GameMenu.res` can be read: the same tokens and commands as
/// the anniversary file, so labels still come from localization.
pub fn fallback_game_menu() -> Vec<MenuItem> {
    let item = |label: &str, command: &str, category: Option<&str>, order, visibility| MenuItem {
        label: label.into(),
        command: command.into(),
        category: category.map(Into::into),
        order,
        visibility,
        caps: true,
    };
    use Visibility::*;
    let mut items = vec![
        item(
            "#GameUI_HalfLife2",
            "SwitchExpansion hl2",
            Some("main"),
            30,
            NotInGame,
        ),
        item(
            "#GameUI_HalfLife2EpisodeOne",
            "SwitchExpansion episodic",
            Some("main"),
            40,
            NotInGame,
        ),
        item(
            "#GameUI_HalfLife2EpisodeTwo",
            "SwitchExpansion ep2",
            Some("main"),
            50,
            NotInGame,
        ),
        item("", "", Some("main"), 65, NotInGame),
        item(
            "#GameUI_Extras",
            "SwitchCategory extras",
            Some("main"),
            70,
            NotInGame,
        ),
        item(
            "#GameUI_GameMenu_Options",
            "OpenOptionsDialog",
            Some("main"),
            90,
            NotInGame,
        ),
        item(
            "#GameUI_GameMenu_Quit",
            "Quit",
            Some("main"),
            100,
            NotInGame,
        ),
        item(
            "#GameUI_GameMenu_ResumeGame",
            "ResumeGame",
            None,
            10,
            OnlyInGame,
        ),
        item(
            "#GameUI_GameMenu_LoadGame",
            "OpenLoadGameDialog",
            None,
            30,
            OnlyInGame,
        ),
        item(
            "#GameUI_GameMenu_SaveGame",
            "OpenSaveGameDialog",
            None,
            40,
            OnlyInGame,
        ),
        item(
            "#GameUI_GameMenu_Achievements",
            "OpenAchievementsDialog",
            None,
            70,
            OnlyInGame,
        ),
        item(
            "#GameUI_GameMenu_Options",
            "OpenOptionsDialog",
            None,
            90,
            OnlyInGame,
        ),
        item(
            "#GameUI_GameMenu_ReturnToMainMenu",
            "OpenMainMenuDialog",
            None,
            99,
            OnlyInGame,
        ),
        item("#GameUI_GameMenu_Quit", "Quit", None, 100, OnlyInGame),
    ];
    for game in ["hl2", "episodic", "ep2", "lostcoast"] {
        let category = Some(game);
        items.extend([
            item(
                "#GameUI_GameMenu_NewGame",
                &format!("OpenNewGameDialog {game}"),
                category,
                20,
                NotInGame,
            ),
            item(
                "#GameUI_GameMenu_LoadGame",
                &format!("OpenLoadGameDialog {game}"),
                category,
                30,
                NotInGame,
            ),
            item(
                "#GameUI_GameMenu_Achievements",
                &format!("OpenAchievementsDialog {game}"),
                category,
                70,
                NotInGame,
            ),
            item("", "", category, 85, NotInGame),
            item(
                "#GameUI_GameMenu_Options",
                "OpenOptionsDialog",
                category,
                90,
                NotInGame,
            ),
            item(
                "#GameUI_Back",
                "SwitchCategory main",
                category,
                100,
                NotInGame,
            ),
        ]);
    }
    items.extend([
        item(
            "#GameUI_HalfLife2LostCoast",
            "SwitchExpansion lostcoast",
            Some("extras"),
            70,
            NotInGame,
        ),
        item(
            "#GameUI_Worskhop_MenuName",
            "openworkshopdialog",
            Some("extras"),
            72,
            NotInGame,
        ),
        item("", "", Some("extras"), 85, NotInGame),
        item(
            "#GameUI_Back",
            "SwitchCategory main",
            Some("extras"),
            100,
            NotInGame,
        ),
    ]);
    items
}

/// Normalizes expansion submenus (e.g. Lost Coast and Extras) so they conform to the standard
/// game submenu pattern across both installed GameMenu.res and fallback structures.
pub fn normalize_menu_categories(items: &mut Vec<MenuItem>) {
    items.retain(|i| {
        i.category.as_deref() != Some("extras") && i.category.as_deref() != Some("lostcoast")
    });
    let item = |label: &str, command: &str, category: &str, order, visibility| MenuItem {
        label: label.into(),
        command: command.into(),
        category: Some(category.into()),
        order,
        visibility,
        caps: true,
    };
    use Visibility::NotInGame;
    items.extend([
        item(
            "#GameUI_HalfLife2LostCoast",
            "SwitchExpansion lostcoast",
            "extras",
            70,
            NotInGame,
        ),
        item(
            "#GameUI_Worskhop_MenuName",
            "openworkshopdialog",
            "extras",
            72,
            NotInGame,
        ),
        item("", "", "extras", 85, NotInGame),
        item(
            "#GameUI_Back",
            "SwitchCategory main",
            "extras",
            100,
            NotInGame,
        ),
    ]);
    items.extend([
        item(
            "#GameUI_GameMenu_NewGame",
            "OpenNewGameDialog lostcoast",
            "lostcoast",
            20,
            NotInGame,
        ),
        item(
            "#GameUI_GameMenu_LoadGame",
            "OpenLoadGameDialog lostcoast",
            "lostcoast",
            30,
            NotInGame,
        ),
        item(
            "#GameUI_GameMenu_Achievements",
            "OpenAchievementsDialog lostcoast",
            "lostcoast",
            70,
            NotInGame,
        ),
        item("", "", "lostcoast", 85, NotInGame),
        item(
            "#GameUI_GameMenu_Options",
            "OpenOptionsDialog",
            "lostcoast",
            90,
            NotInGame,
        ),
        item(
            "#GameUI_Back",
            "SwitchCategory main",
            "lostcoast",
            100,
            NotInGame,
        ),
    ]);
}

/// Decoded RGBA image (gamma test pattern, credit logos).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

/// The VGUI layout files the dialogs use, keyed by lower-case file stem.
pub const LAYOUT_FILES: &[&str] = &[
    "optionssubdifficulty",
    "optionssubkeyboard",
    "optionssubmouse",
    "optionssubaudio",
    "optionssubvideo",
    "optionssubvideogammadlg",
    "optionssubaudiothirdpartydlg",
    "optionssubvideothirdpartydlg",
];

/// Everything the GameUI reads from the installation.
#[derive(Clone, Debug, Default)]
pub struct GameUiResources {
    pub localization: Localization,
    pub game_menu: Vec<MenuItem>,
    pub layouts: BTreeMap<String, Layout>,
    pub key_rows: Vec<KeyRow>,
    /// Command -> default binding name from `cfg/config_default.cfg` (macOS overrides applied on macOS).
    pub default_bindings: BTreeMap<String, String>,
    pub gamma_image: Option<Image>,
    pub bink_image: Option<Image>,
    /// Installed voice languages (`hl2_sound_vo_<language>_dir.vpk`), lower-case, English first.
    pub spoken_languages: Vec<String>,
    /// Main-menu background map (`scripts/chapterbackgrounds.txt`, chapter 1).
    pub background_map: Option<String>,
    /// Category / expansion -> chapter 1 background map (`scripts/chapterbackgrounds.txt`).
    pub background_maps: BTreeMap<String, String>,
    /// Game -> first chapter map (`cfg/chapter1.cfg` of each mounted game).
    pub new_game_maps: BTreeMap<String, String>,
    /// Category -> menu title (two lines for episodes, e.g. "Half-Life 2\n--- Episode One ---").
    pub category_titles: BTreeMap<String, String>,
    /// Files that could not be read or parsed (reported in the console log).
    pub missing: Vec<String>,
}

impl GameUiResources {
    /// Returns the background map configured for the given category, if any.
    pub fn background_map_for(&self, category: &str) -> Option<&str> {
        self.background_maps
            .get(category)
            .map(String::as_str)
            .or(self.background_map.as_deref())
    }
}

impl GameUiResources {
    pub fn load(vfs: &Vfs) -> Self {
        let reader = Reader::new(vfs);
        let mut res = Self::default();
        for file in [
            "resource/merged_loc_all.txt",
            "resource/gameui_english.txt",
            "resource/valve_english.txt",
            "resource/vgui_english.txt",
            "resource/hl2_english.txt",
        ] {
            match reader.read(file) {
                Some(bytes) if res.localization.add_resource(file, &bytes).is_some() => {}
                _ => res.missing.push(file.into()),
            }
        }
        res.game_menu = match reader
            .read("resource/gamemenu.res")
            .as_deref()
            .and_then(parse_game_menu)
        {
            Some(mut items) => {
                normalize_menu_categories(&mut items);
                items
            }
            None => {
                res.missing.push("resource/gamemenu.res".into());
                fallback_game_menu()
            }
        };
        for name in LAYOUT_FILES {
            let file = format!("resource/{name}.res");
            match reader.read(&file).as_deref().and_then(Layout::parse) {
                Some(layout) => {
                    res.layouts.insert((*name).to_owned(), layout);
                }
                None => res.missing.push(file),
            }
        }
        match reader.read_text("scripts/kb_act.lst") {
            Some(text) => res.key_rows = keys::parse_actions(&text),
            None => res.missing.push("scripts/kb_act.lst".into()),
        }
        match reader.read_text("cfg/config_default.cfg") {
            Some(text) => {
                res.default_bindings = keys::parse_default_binds(&text, cfg!(target_os = "macos"))
            }
            None => res.missing.push("cfg/config_default.cfg".into()),
        }
        res.gamma_image = image(vfs, "vgui/gamma");
        if res.gamma_image.is_none() {
            res.missing.push("materials/vgui/gamma.vmt".into());
        }
        res.bink_image = image(vfs, "vgui/techcredits/bink");
        res.spoken_languages = vfs
            .game_root()
            .map(|root| spoken_languages(&root.join("hl2")))
            .unwrap_or_default();
        if res.spoken_languages.is_empty() {
            res.spoken_languages.push("english".into());
        }
        res.background_maps = fallback_background_maps();
        for &(game, _, default_map) in INITIAL_BACKGROUND_MAPS {
            let map = if game == "main" || game == "hl2" {
                reader
                    .read_text("scripts/chapterbackgrounds.txt")
                    .and_then(|text| background_map(&text))
                    .unwrap_or_else(|| default_map.to_owned())
            } else {
                let exp_map = vfs
                    .game_root()
                    .and_then(|root| {
                        source_assets::vpk::Vfs::mount_expansion(root, Some(game)).ok()
                    })
                    .and_then(|exp_vfs| {
                        Reader::new(&exp_vfs).read_text("scripts/chapterbackgrounds.txt")
                    })
                    .and_then(|text| background_map(&text));
                exp_map.unwrap_or_else(|| default_map.to_owned())
            };
            res.background_maps.insert(game.to_owned(), map);
        }
        res.background_map = res
            .background_maps
            .get("hl2")
            .cloned()
            .or_else(|| res.background_maps.get("main").cloned());
        for &(game, cfg_path, default_map) in INITIAL_EPISODE_MAPS {
            let map = reader
                .read_text(cfg_path)
                .or_else(|| {
                    vfs.game_root()
                        .and_then(|root| std::fs::read_to_string(root.join(cfg_path)).ok())
                })
                .and_then(|text| chapter_map(&text))
                .unwrap_or_else(|| default_map.to_owned());
            res.new_game_maps.insert(game.to_owned(), map);
        }
        for category in ["main", "hl2", "episodic", "ep2", "lostcoast"] {
            let gameinfo_path = match category {
                "episodic" => "episodic/gameinfo.txt",
                "ep2" => "ep2/gameinfo.txt",
                "lostcoast" => "lostcoast/gameinfo.txt",
                _ => "gameinfo.txt",
            };
            let title = reader
                .read(gameinfo_path)
                .or_else(|| {
                    vfs.game_root()
                        .and_then(|root| std::fs::read(root.join(gameinfo_path)).ok())
                })
                .as_deref()
                .and_then(parse_gameinfo_title);
            let title =
                title.unwrap_or_else(|| category_title_fallback(category, &res.localization));
            res.category_titles.insert(category.to_owned(), title);
        }
        res
    }

    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts.get(name)
    }
}

/// Reads `hl2_complete/<path>` loose first, then the mounted VFS.
pub struct Reader<'a> {
    vfs: &'a Vfs,
    overlay: Option<PathBuf>,
}

impl<'a> Reader<'a> {
    pub fn new(vfs: &'a Vfs) -> Self {
        Self {
            vfs,
            overlay: vfs.game_root().map(|root| root.join("hl2_complete")),
        }
    }

    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        let normalized = source_assets::vpk::normalize(path).ok()?;
        self.overlay
            .as_ref()
            .and_then(|root| std::fs::read(root.join(&normalized)).ok())
            .or_else(|| self.vfs.read(&normalized).ok().flatten())
    }

    pub fn read_text(&self, path: &str) -> Option<String> {
        self.read(path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn image(vfs: &Vfs, material: &str) -> Option<Image> {
    let base = vfs.base_texture(material).ok()??;
    let file = format!(
        "materials/{}.vtf",
        base.trim_start_matches("materials/")
            .trim_end_matches(".vtf")
    );
    let decoded = source_assets::vtf::decode(&vfs.read(&file).ok()??, 512).ok()?;
    Some(Image {
        width: decoded.width,
        height: decoded.height,
        rgba: decoded.rgba,
    })
}

/// Languages with an installed `hl2_sound_vo_<language>_dir.vpk`, English first, then sorted.
pub fn spoken_languages(hl2: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(hl2)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let name = entry
                .ok()?
                .file_name()
                .to_string_lossy()
                .to_ascii_lowercase();
            name.strip_prefix("hl2_sound_vo_")?
                .strip_suffix("_dir.vpk")
                .map(str::to_owned)
        })
        .collect();
    found.sort_by_key(|l| (l != "english", l.clone()));
    found.dedup();
    found
}

/// Chapter 1's background map from `scripts/chapterbackgrounds.txt`.
pub fn background_map(text: &str) -> Option<String> {
    let roots = keyvalues::parse_resource(text.as_bytes()).ok()?;
    roots
        .iter()
        .find_map(|root| root.get("1"))
        .and_then(|e| e.text())
        .map(|map| map.trim().to_owned())
        .filter(|map| !map.is_empty())
}

/// Initial background maps for each game and episode category.
pub const INITIAL_BACKGROUND_MAPS: &[(&str, &str, &str)] = &[
    ("main", "scripts/chapterbackgrounds.txt", "background01"),
    ("hl2", "scripts/chapterbackgrounds.txt", "background01"),
    (
        "episodic",
        "episodic/scripts/chapterbackgrounds.txt",
        "ep1_background01",
    ),
    (
        "ep2",
        "ep2/scripts/chapterbackgrounds.txt",
        "ep2_background01",
    ),
    (
        "lostcoast",
        "lostcoast/scripts/chapterbackgrounds.txt",
        "background01",
    ),
];

/// Fallback initial background maps for each game and episode category.
pub fn fallback_background_maps() -> BTreeMap<String, String> {
    INITIAL_BACKGROUND_MAPS
        .iter()
        .map(|(game, _, map)| ((*game).to_owned(), (*map).to_owned()))
        .collect()
}

/// Initial game and episode maps verified against retail Source installation configs.
pub const INITIAL_EPISODE_MAPS: &[(&str, &str, &str)] = &[
    ("hl2", "cfg/chapter1.cfg", "d1_trainstation_01"),
    ("episodic", "episodic/cfg/chapter1.cfg", "ep1_citadel_00"),
    ("ep2", "ep2/cfg/chapter1.cfg", "ep2_outland_01"),
    ("lostcoast", "lostcoast/cfg/chapter1.cfg", "d2_lostcoast"),
];

/// Fallback initial new game maps for each game and episode category.
pub fn fallback_new_game_maps() -> BTreeMap<String, String> {
    INITIAL_EPISODE_MAPS
        .iter()
        .map(|(game, _, map)| ((*game).to_owned(), (*map).to_owned()))
        .collect()
}

/// The map of a chapter config (`map d1_trainstation_01` or `playvideo_exitcommand ep1_recap map ep2_outland_01`).
pub fn chapter_map(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let clean = line.split("//").next()?;
        let mut words = clean.split_whitespace();
        while let Some(w) = words.next() {
            if w.eq_ignore_ascii_case("map") {
                return words.next().map(|m| m.trim_matches('"').to_owned());
            }
        }
        None
    })
}

/// Background map for startup without a `--map`, read through a freshly mounted VFS.
pub fn startup_background_map(vfs: &Vfs) -> Option<String> {
    Reader::new(vfs)
        .read_text("scripts/chapterbackgrounds.txt")
        .and_then(|text| background_map(&text))
}

/// Extracts the menu title from a `gameinfo.txt` file (`title` + optional `title2` joined by `\n`).
pub fn parse_gameinfo_title(bytes: &[u8]) -> Option<String> {
    let roots = keyvalues::parse_resource(bytes).ok()?;
    let root = roots.into_iter().next()?;
    let title = root.get("title")?.text()?.to_owned();
    if let Some(title2) = root.get("title2").and_then(|t| t.text()) {
        Some(format!("{title}\n{title2}"))
    } else {
        Some(title)
    }
}

/// Fallback titles for each menu category, resolving localized tokens if present with clean fallback strings.
pub fn category_title_fallback(category: &str, loc: &Localization) -> String {
    let hl2 = loc.lookup("GameUI_HalfLife2").unwrap_or("Half-Life 2");
    match category {
        "episodic" => {
            let ep1 = loc.lookup("GameUI_EpisodeOne").unwrap_or("Episode One");
            format!("{hl2}\n--- {ep1} ---")
        }
        "ep2" => {
            let ep2 = loc.lookup("GameUI_EpisodeTwo").unwrap_or("Episode Two");
            format!("{hl2}\n--- {ep2} ---")
        }
        "lostcoast" => {
            let lc = loc.lookup("GameUI_LostCoast").unwrap_or("Lost Coast");
            format!("{hl2}\n--- {lc} ---")
        }
        _ => hl2.to_owned(),
    }
}

/// Returns fallback titles for all recognized categories.
pub fn fallback_category_titles(loc: &Localization) -> BTreeMap<String, String> {
    let mut titles = BTreeMap::new();
    for category in ["main", "hl2", "episodic", "ep2", "lostcoast"] {
        titles.insert(category.to_owned(), category_title_fallback(category, loc));
    }
    titles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_menu_entries_keep_file_order_categories_and_spacers() {
        let menu = parse_game_menu(
            br##""GameMenu" {
                "Main_1" { "label" "#GameUI_HalfLife2" "command" "SwitchExpansion hl2" "InGameOrder" "30" "NotInGame" "1" "MenuCategory" "main" }
                "Main_4_spc" { "label" "" "command" "" "InGameOrder" "65" "NotInGame" "1" "MenuCategory" "Main" }
                "Pause_1" { "label" "#GameUI_GameMenu_ResumeGame" "command" "ResumeGame" "InGameOrder" "10" "OnlyInGame" "1" }
            }"##,
        )
        .unwrap();
        assert_eq!(menu.len(), 3);
        assert_eq!(menu[0].category.as_deref(), Some("main"));
        assert!(menu[1].is_spacer());
        assert_eq!(
            menu[1].category.as_deref(),
            Some("main"),
            "categories are case-insensitive"
        );
        assert_eq!(menu[2].category, None);
        assert_eq!(menu[2].visibility, Visibility::OnlyInGame);
        assert_eq!(menu[0].visibility, Visibility::NotInGame);
        assert!(parse_game_menu(b"\"GameMenu\" { }").is_none());
    }

    #[test]
    fn fallback_menu_has_every_requested_category() {
        let menu = fallback_game_menu();
        for category in ["main", "hl2", "episodic", "ep2", "extras", "lostcoast"] {
            assert!(
                menu.iter().any(|i| i.category.as_deref() == Some(category)),
                "{category}"
            );
        }
        assert!(
            menu.iter()
                .all(|i| i.is_spacer() || i.label.starts_with('#')),
            "labels stay tokens"
        );

        let extras_items: Vec<_> = menu
            .iter()
            .filter(|i| i.category.as_deref() == Some("extras"))
            .collect();
        assert_eq!(extras_items.len(), 4);
        assert_eq!(extras_items[0].label, "#GameUI_HalfLife2LostCoast");
        assert_eq!(extras_items[0].command, "SwitchExpansion lostcoast");
        assert_eq!(extras_items[1].label, "#GameUI_Worskhop_MenuName");
        assert_eq!(extras_items[1].command, "openworkshopdialog");
        assert!(extras_items[2].is_spacer());
        assert_eq!(extras_items[3].label, "#GameUI_Back");
        assert_eq!(extras_items[3].command, "SwitchCategory main");

        let lc_items: Vec<_> = menu
            .iter()
            .filter(|i| i.category.as_deref() == Some("lostcoast"))
            .collect();
        assert_eq!(lc_items.len(), 6);
        assert_eq!(lc_items[0].label, "#GameUI_GameMenu_NewGame");
        assert_eq!(lc_items[0].command, "OpenNewGameDialog lostcoast");
        assert_eq!(lc_items[1].label, "#GameUI_GameMenu_LoadGame");
        assert_eq!(lc_items[1].command, "OpenLoadGameDialog lostcoast");
        assert_eq!(lc_items[2].label, "#GameUI_GameMenu_Achievements");
        assert_eq!(lc_items[2].command, "OpenAchievementsDialog lostcoast");
        assert!(lc_items[3].is_spacer());
        assert_eq!(lc_items[4].label, "#GameUI_GameMenu_Options");
        assert_eq!(lc_items[4].command, "OpenOptionsDialog");
        assert_eq!(lc_items[5].label, "#GameUI_Back");
        assert_eq!(lc_items[5].command, "SwitchCategory main");
    }

    #[test]
    fn chapter_files_name_their_maps() {
        assert_eq!(
            background_map("\"chapters\"\n{\n\t1\t\"background01\"\n\t2\t\"background01\"\n}"),
            Some("background01".into())
        );
        assert_eq!(
            chapter_map("// comment\nmap d1_trainstation_01\n"),
            Some("d1_trainstation_01".into())
        );
        assert_eq!(
            chapter_map("map ep1_citadel_00"),
            Some("ep1_citadel_00".into())
        );
        assert_eq!(
            chapter_map("playvideo_exitcommand ep1_recap map ep2_outland_01"),
            Some("ep2_outland_01".into())
        );
        assert_eq!(
            chapter_map("map \"d2_lostcoast\""),
            Some("d2_lostcoast".into())
        );
        assert_eq!(chapter_map("echo hi"), None);
    }

    #[test]
    fn initial_episode_maps_table_and_fallbacks() {
        let maps = fallback_new_game_maps();
        assert_eq!(
            maps.get("hl2").map(String::as_str),
            Some("d1_trainstation_01")
        );
        assert_eq!(
            maps.get("episodic").map(String::as_str),
            Some("ep1_citadel_00")
        );
        assert_eq!(maps.get("ep2").map(String::as_str), Some("ep2_outland_01"));
        assert_eq!(
            maps.get("lostcoast").map(String::as_str),
            Some("d2_lostcoast")
        );
    }

    #[test]
    fn initial_background_maps_table_and_fallbacks() {
        let maps = fallback_background_maps();
        assert_eq!(maps.get("main").map(String::as_str), Some("background01"));
        assert_eq!(maps.get("hl2").map(String::as_str), Some("background01"));
        assert_eq!(
            maps.get("episodic").map(String::as_str),
            Some("ep1_background01")
        );
        assert_eq!(
            maps.get("ep2").map(String::as_str),
            Some("ep2_background01")
        );
        assert_eq!(
            maps.get("lostcoast").map(String::as_str),
            Some("background01")
        );

        let res = GameUiResources {
            background_maps: maps,
            ..Default::default()
        };
        assert_eq!(res.background_map_for("episodic"), Some("ep1_background01"));
        assert_eq!(res.background_map_for("ep2"), Some("ep2_background01"));
        assert_eq!(res.background_map_for("hl2"), Some("background01"));
        assert_eq!(res.background_map_for("unknown"), None);
    }

    #[test]
    fn parse_gameinfo_title_handles_single_and_two_line_titles() {
        let single = b"\"GameInfo\" { \"game\" \"HALF-LIFE 2\" \"title\" \"HALF-LIFE'\" }";
        assert_eq!(parse_gameinfo_title(single), Some("HALF-LIFE'".into()));

        let episode1 = b"\"GameInfo\" { \"game\" \"HALF-LIFE 2\" \"title\" \"HALF-LIFE'\" \"title2\" \"== episode one==\" }";
        assert_eq!(
            parse_gameinfo_title(episode1),
            Some("HALF-LIFE'\n== episode one==".into())
        );

        let episode2 = b"\"GameInfo\" { \"game\" \"HALF-LIFE 2\" \"title\" \"HALF-LIFE'\" \"title2\" \"== episode two ==\" }";
        assert_eq!(
            parse_gameinfo_title(episode2),
            Some("HALF-LIFE'\n== episode two ==".into())
        );
    }

    #[test]
    fn category_titles_fallback_provides_two_line_episode_titles() {
        let mut loc = Localization::default();
        loc.insert("GameUI_HalfLife2", "Half-Life 2");
        loc.insert("GameUI_EpisodeOne", "Episode One");
        loc.insert("GameUI_EpisodeTwo", "Episode Two");
        loc.insert("GameUI_LostCoast", "Lost Coast");

        let titles = fallback_category_titles(&loc);
        assert_eq!(titles.get("main").map(String::as_str), Some("Half-Life 2"));
        assert_eq!(titles.get("hl2").map(String::as_str), Some("Half-Life 2"));
        assert_eq!(
            titles.get("episodic").map(String::as_str),
            Some("Half-Life 2\n--- Episode One ---")
        );
        assert_eq!(
            titles.get("ep2").map(String::as_str),
            Some("Half-Life 2\n--- Episode Two ---")
        );
        assert_eq!(
            titles.get("lostcoast").map(String::as_str),
            Some("Half-Life 2\n--- Lost Coast ---")
        );

        let empty_loc = Localization::default();
        let fallback_empty = fallback_category_titles(&empty_loc);
        assert_eq!(
            fallback_empty.get("episodic").map(String::as_str),
            Some("Half-Life 2\n--- Episode One ---")
        );
        assert_eq!(
            fallback_empty.get("ep2").map(String::as_str),
            Some("Half-Life 2\n--- Episode Two ---")
        );
    }

    #[test]
    fn spoken_languages_list_installed_voice_packs_english_first() {
        let dir = std::env::temp_dir().join(format!("hl2-rs-vo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "hl2_sound_vo_russian_dir.vpk",
            "hl2_sound_vo_english_dir.vpk",
            "hl2_sound_vo_english_000.vpk",
            "hl2_pak_dir.vpk",
        ] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        assert_eq!(spoken_languages(&dir), ["english", "russian"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Owned-install check: `cargo test -p hl2-ui -- --ignored owned_install`.
    #[test]
    #[ignore = "requires an owned Half-Life 2 installation"]
    fn owned_install_resources_load() {
        let root = source_assets::install::discover().expect("installed game");
        let vfs = Vfs::mount(&root).unwrap();
        let res = GameUiResources::load(&vfs);
        assert!(
            res.missing.is_empty() || !res.missing.contains(&"resource/gamemenu.res".to_string()),
            "{:?}",
            res.missing
        );
        assert_eq!(
            res.localization.text("#GameUI_QuitConfirmationTitle"),
            "QUIT GAME"
        );
        assert_eq!(
            res.localization.text("#GameUI_NoWindowBorder"),
            "Windowed (No Border)"
        );
        assert_eq!(
            res.localization.text("#HL2RS_Absent_Token"),
            "HL2RS_Absent_Token"
        );
        assert!(res
            .game_menu
            .iter()
            .any(|i| i.command == "SwitchExpansion ep2"));
        assert!(res
            .layout("optionssubmouse")
            .and_then(|l| l.get("Autoaim"))
            .is_some());
        assert!(res.key_rows.len() > 30);
        assert_eq!(
            res.default_bindings.get("+forward").map(String::as_str),
            Some("KeyW")
        );
        assert!(res.gamma_image.is_some(), "{:?}", res.missing);
        assert_eq!(
            res.spoken_languages.first().map(String::as_str),
            Some("english")
        );
        assert_eq!(res.background_map.as_deref(), Some("background01"));
        assert_eq!(
            res.new_game_maps.get("hl2").map(String::as_str),
            Some("d1_trainstation_01")
        );
        assert_eq!(
            res.new_game_maps.get("episodic").map(String::as_str),
            Some("ep1_citadel_00")
        );
        assert_eq!(
            res.new_game_maps.get("ep2").map(String::as_str),
            Some("ep2_outland_01")
        );
        assert_eq!(
            res.new_game_maps.get("lostcoast").map(String::as_str),
            Some("d2_lostcoast")
        );
        assert_eq!(
            res.category_titles.get("episodic").map(String::as_str),
            Some("HALF-LIFE'\n== episode one==")
        );
        assert_eq!(
            res.category_titles.get("ep2").map(String::as_str),
            Some("HALF-LIFE'\n== episode two ==")
        );
        println!("missing: {:?}", res.missing);
    }
}
