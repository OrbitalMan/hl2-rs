//! A bounded standalone console. Commands without a Rust implementation report an error.
use crate::gameui::{
    display::DisplayModes,
    menu::{GameMenu, MenuLayout, MenuOutcome, MessageBox, MessageBoxLayout},
    options::{OptionsDialog, OptionsOutcome},
    resources::{self, fallback_game_menu, GameUiResources},
    widgets::{scheme_color, ButtonTracker, Fonts, Painter, Theme},
};
use crate::{
    canvas::*,
    hud::{FontFace, TextOutline},
};
use glam::{vec2, Vec2};
use serde::Serialize;
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

const COMMANDS: &[(&str, &str)] = &[
    ("help", "help [command]: describe implemented commands"),
    (
        "find",
        "find <text>: search implemented command names and descriptions",
    ),
    (
        "sv_cheats",
        "sv_cheats [0|1]: query or enable local cheat commands",
    ),
    (
        "impulse",
        "impulse 101: grant the six implemented weapons, ammunition and suit (cheat)",
    ),
    (
        "noclip",
        "noclip [0|1]: toggle collision-free flight (cheat)",
    ),
    (
        "getpos",
        "getpos: print camera position and Source pitch/yaw/roll in degrees; getpos 2 is not implemented",
    ),
    (
        "setpos",
        "setpos <x> <y> [z]: move the player origin, preserving height if z is omitted (cheat)",
    ),
    (
        "setang",
        "setang <pitch> <yaw> [roll]: set Source view angles; roll must be zero (cheat)",
    ),
    (
        "map",
        "map <name>: load an installed map and reset the player inventory",
    ),
    ("clear", "clear: clear console output"),
    ("ent_fire", "ent_fire <targetname> [input=Use] [parameter] [delay]: queue entity input; names/globs supported (cheat)"),
    ("echo", "echo <text>: print text"),
    ("toggleconsole", "toggleconsole: close or open the console"),
    ("quit", "quit: exit and save the runtime report"),
    ("volume", "volume [0..1]: query or set master audio volume"),
    ("sensitivity", "sensitivity [value]: query or set mouse sensitivity"),
    ("fov", "fov [degrees]: query or set camera field of view"),
    ("mat_hdr_level", "mat_hdr_level [0|1|2]: query or set HDR level (0=None, 1=Bloom, 2=Full)"),
    ("mat_setvideomode", "mat_setvideomode <width> <height> <windowed 0|1>: set the video mode"),
    ("mainmenu", "mainmenu: show the main menu"),
    (
        "background_map",
        "background_map [name|default]: query or set the persistent main-menu background map",
    ),
    (
        "map_background",
        "map_background <name>: load an installed map as the main-menu background and persist it",
    ),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Mode {
    #[default]
    Gameplay,
    Pause,
    Console,
    MainMenu,
    Options,
}

#[derive(Debug, PartialEq)]
pub enum Effect {
    Loadout,
    Noclip(Option<bool>),
    Getpos,
    Setpos {
        x: f32,
        y: f32,
        z: Option<f32>,
    },
    Setang(glam::Vec3),
    Map(String),
    Fire {
        target: String,
        input: String,
        parameter: String,
        delay: f64,
    },
    Quit,
    ApplyConfig(Box<crate::config::Config>),
    /// Load the main-menu background map and show the main menu over it ("Return to main menu").
    MainMenuMap(String),
    /// Play a UI sound cue (e.g. button hover or release activation).
    PlaySound(String),
}

/// Action to perform upon confirming a message box dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmKind {
    Quit,
    ReturnToMainMenu,
}

pub struct Console {
    pub canvas: Canvas,
    pub mode: Mode,
    previous: Mode,
    pub cheats: bool,
    pub input: String,
    pub output: VecDeque<String>,
    history: Vec<String>,
    history_index: Option<usize>,
    history_draft: String,
    cursor: usize,
    scroll: usize,
    font: Option<FontFace>,
    menu_font: Option<FontFace>,
    menu_metrics: CellMetrics,
    title_font: Option<FontFace>,
    title_face: Option<Arc<fontdue::Font>>,
    title_metrics: CellMetrics,
    lostcoast_title_font: Option<FontFace>,
    lostcoast_title_face: Option<Arc<fontdue::Font>>,
    lostcoast_title_metrics: CellMetrics,
    title: String,
    category_titles: BTreeMap<String, String>,
    menu_layout: MenuLayout,
    mono: Option<FontFace>,
    developer_font: Option<FontFace>,
    /// Installed GameUI resources (localization, menu structure, dialog layouts, key actions).
    gameui: Arc<GameUiResources>,
    /// The out-of-game menu (main menu and its HL2/Episode categories).
    main_menu: GameMenu,
    /// The in-game (pause) menu.
    pause_menu: GameMenu,
    pub(crate) confirm_box: Option<(MessageBox, ConfirmKind)>,
    pub loading_dialog: Option<crate::gameui::loading::LoadingDialog>,
    pub background_transition: crate::gameui::loading::BackgroundTransitionState,
    /// The loaded map is a menu background: no HUD, and the main menu cannot be dismissed.
    background_level: bool,
    /// Currently loaded map name, if any.
    pub loaded_map: Option<String>,
    pending_map: Option<String>,
    theme: Theme,
    widget_fonts: Fonts,
    style: Style,
    pub menu_tracker: ButtonTracker<usize>,
    last_menu_hover: Option<usize>,
    pub options: OptionsDialog,
    /// The menu the options dialog was opened from (drawn behind it and restored on close).
    options_return: Mode,
    pub config: crate::config::Config,
}

/// Positive native font heights describe Windows character cells, not pixels per EM.
#[derive(Clone, Copy)]
struct CellMetrics {
    em_per_cell: f32,
    ascent_per_cell: f32,
}
impl Default for CellMetrics {
    fn default() -> Self {
        Self {
            em_per_cell: 1.,
            ascent_per_cell: 0.8,
        }
    }
}
impl CellMetrics {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let face = ttf_parser::Face::parse(bytes, 0).ok()?;
        let units = f32::from(face.units_per_em());
        let (ascent, descent) = face.tables().os2.map_or_else(
            || {
                (
                    f32::from(face.ascender()),
                    f32::from(face.descender()).abs(),
                )
            },
            |table| {
                (
                    f32::from(table.windows_ascender()),
                    f32::from(table.windows_descender()).abs(),
                )
            },
        );
        let height = ascent + descent;
        if height <= 0. || units <= 0. {
            return None;
        }
        Some(Self {
            em_per_cell: units / height,
            ascent_per_cell: ascent / height,
        })
    }
}

fn draw_cell_text_outlined(
    value: &str,
    origin: Vec2,
    tall: f32,
    font: Option<&FontFace>,
    metrics: CellMetrics,
    color: Color,
    outline: TextOutline,
) {
    let em = (tall * metrics.em_per_cell).max(1.);
    let raster_size = em.round().clamp(1., 1024.);
    if let Some(font) = font {
        font.draw_baseline_outlined(
            value,
            origin + vec2(0., tall * metrics.ascent_per_cell),
            raster_size,
            em / raster_size,
            color,
            outline,
        );
    }
}

fn draw_cell_text(
    _canvas: &Canvas,
    value: &str,
    origin: Vec2,
    tall: f32,
    font: Option<&FontFace>,
    metrics: CellMetrics,
    color: Color,
) {
    draw_cell_text_outlined(value, origin, tall, font, metrics, color, TextOutline::NONE);
}
fn text(
    _canvas: &Canvas,
    value: &str,
    origin: Vec2,
    em: f32,
    scale: f32,
    font: Option<&FontFace>,
    color: Color,
) {
    if let Some(font) = font {
        font.draw_baseline(value, origin, em, scale, color);
    }
}
pub(crate) fn rectangle_lines(
    canvas: &Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: Color,
) {
    canvas.rectangle(x, y, w, thickness, color);
    canvas.rectangle(x, y + h - thickness, w, thickness, color);
    canvas.rectangle(x, y, thickness, h, color);
    canvas.rectangle(x + w - thickness, y, thickness, h, color);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Up,
    Down,
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
}
#[derive(Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Input {
    pub keys: std::collections::BTreeSet<Key>,
    pub text: String,
    pub pointer: Option<Vec2>,
    /// Left button went down this frame.
    pub click: bool,
    /// Left button is held (slider drags).
    pub held: bool,
    /// Left button was released this frame.
    pub released: bool,
    /// Mouse wheel steps this frame; positive scrolls up.
    pub wheel: f32,
    /// Keys and buttons pressed this frame as persisted binding names (`KeyW`, `SuperLeft`,
    /// `MOUSE1`, `MWHEELUP`), used when the keyboard page captures a new key.
    pub binding_keys: Vec<String>,
}
impl Input {
    pub(crate) fn pressed(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }
}

struct Style {
    menu: Color,
    menu_armed: Color,
    panel: Color,
    entry: Color,
    text: Color,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            menu: WHITE,
            menu_armed: Color::from_rgba(200, 200, 200, 255),
            panel: Color::from_rgba(132, 132, 132, 212),
            entry: Color::from_rgba(0, 0, 0, 128),
            text: Color::from_rgba(221, 221, 221, 255),
        }
    }
}
impl Default for Console {
    fn default() -> Self {
        let config = crate::config::Config::default();
        let gameui = GameUiResources {
            game_menu: fallback_game_menu(),
            ..Default::default()
        };
        let main_menu = GameMenu::new(Vec::new(), Default::default(), &gameui.localization);
        let pause_menu = GameMenu::new(Vec::new(), Default::default(), &gameui.localization);
        let mut console = Self {
            canvas: Canvas::default(),
            mode: Mode::Gameplay,
            previous: Mode::Gameplay,
            cheats: false,
            input: String::new(),
            output: VecDeque::new(),
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            cursor: 0,
            scroll: 0,
            font: None,
            menu_font: None,
            menu_metrics: CellMetrics::default(),
            title_font: None,
            title_face: None,
            title_metrics: CellMetrics::default(),
            lostcoast_title_font: None,
            lostcoast_title_face: None,
            lostcoast_title_metrics: CellMetrics::default(),
            title: "HL2-RS".into(),
            category_titles: resources::fallback_category_titles(&gameui.localization),
            menu_layout: MenuLayout::default(),
            mono: None,
            developer_font: None,
            gameui: Arc::default(),
            main_menu,
            pause_menu,
            confirm_box: None,
            loading_dialog: None,
            background_transition: crate::gameui::loading::BackgroundTransitionState::Idle,
            background_level: false,
            loaded_map: None,
            pending_map: None,
            theme: Theme::default(),
            widget_fonts: Fonts::default(),
            style: Style::default(),
            menu_tracker: ButtonTracker::new(),
            last_menu_hover: None,
            options: OptionsDialog::default(),
            options_return: Mode::MainMenu,
            config,
        };
        console.set_resources(gameui);
        console.log("HL2-RS developer console. Type help for implemented commands.");
        console.log("Other Source commands are not implemented in this runtime.");
        console
    }
}

impl Console {
    pub fn load(vfs: &Vfs, canvas: Canvas) -> Self {
        let font = |path: &str| {
            vfs.read(path)
                .ok()
                .flatten()
                .and_then(|bytes| FontFace::load(&bytes, canvas.clone()).ok())
        };
        let developer_font = FontFace::load(
            include_bytes!("../assets/fonts/ProggyClean.ttf"),
            canvas.clone(),
        )
        .ok();
        let menu_font = font("resource/linux_fonts/liberationsans-regular.ttf")
            .or_else(|| font("resource/gordin-regular.ttf"))
            .or_else(|| developer_font.clone());
        let ui_font = font("resource/linux_fonts/liberationsans-regular.ttf")
            .or_else(|| font("resource/gordin-regular.ttf"))
            .or_else(|| developer_font.clone());
        let mono = font("resource/linux_fonts/liberationmono-regular.ttf")
            .or_else(|| font("resource/linux_fonts/DejaVuSansMono.ttf"))
            .or_else(|| developer_font.clone());
        // The scheme's Default/DefaultBold dialog fonts:
        let bold_font = font("resource/linux_fonts/liberationsans-bold.ttf")
            .or_else(|| font("resource/gordin-bold.ttf"))
            .or_else(|| ui_font.clone());
        let config = crate::config::Config::load_or_default();
        let mut console = Self {
            canvas: canvas.clone(),
            developer_font,
            widget_fonts: Fonts {
                text: ui_font.clone(),
                bold: bold_font,
            },
            font: ui_font,
            menu_font,
            mono,
            config,
            ..Default::default()
        };
        let gameui = GameUiResources::load(vfs);
        for file in &gameui.missing {
            console.log(format!("GameUI resource unavailable: {file}"));
        }
        console.set_resources(gameui);
        let menu_bytes = vfs
            .read("resource/linux_fonts/liberationsans-regular.ttf")
            .ok()
            .flatten()
            .or_else(|| vfs.read("resource/gordin-regular.ttf").ok().flatten());
        if let Some(bytes) = menu_bytes {
            console.menu_metrics = CellMetrics::from_bytes(&bytes).unwrap_or_default();
        }
        let title_bytes = vfs
            .read("resource/HL2EP2.ttf")
            .ok()
            .flatten()
            .or_else(|| vfs.read("resource/hl2ep2.ttf").ok().flatten())
            .or_else(|| {
                vfs.game_root()
                    .and_then(|root| std::fs::read(root.join("hl2/resource/hl2ep2.ttf")).ok())
            })
            .or_else(|| vfs.read("resource/HALFLIFE2.ttf").ok().flatten())
            .or_else(|| vfs.read("resource/halflife2.ttf").ok().flatten())
            .or_else(|| {
                vfs.game_root()
                    .and_then(|root| std::fs::read(root.join("hl2/resource/halflife2.ttf")).ok())
            });
        if let Some(bytes) = title_bytes {
            console.title_metrics = CellMetrics::from_bytes(&bytes).unwrap_or_default();
            console.title_font = FontFace::load(&bytes, canvas.clone()).ok();
            console.title_face = console.title_font.as_ref().map(FontFace::face);
            if console.title_font.is_some() {
                let is_hl2_logo_font = console
                    .title_face
                    .as_ref()
                    .is_some_and(|f| f.lookup_glyph_index('=') == 0);
                if is_hl2_logo_font {
                    // Owned 720px retail capture + GDI probe for halflife2.ttf: requested cell48 -> EM41,
                    // native title baseline262 relative to ClientScheme panelY217.
                    // Other resolutions scale this measured GDI/label relation; font rasterization remains approximate.
                    console.title_metrics = CellMetrics {
                        em_per_cell: 41. / 48.,
                        ascent_per_cell: 45. / 48.,
                    };
                }
                console.title = vfs
                    .read("gameinfo.txt")
                    .ok()
                    .flatten()
                    .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
                    .and_then(|roots| roots.into_iter().next())
                    .and_then(|root| {
                        root.get("title")
                            .and_then(|title| title.text())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "HALF-LIFE'".into());
            } else {
                console.title_face = None;
                console.title_metrics = CellMetrics::default();
                console.category_titles =
                    resources::fallback_category_titles(&console.gameui.localization);
                console.log("Owned HalfLife2 title font could not be loaded; using the HL2-RS fallback title.");
            }
        } else {
            console.category_titles =
                resources::fallback_category_titles(&console.gameui.localization);
            console.log("Owned HalfLife2 title font not found; using the HL2-RS fallback title.");
        }

        let lc_bytes = vfs
            .read("lostcoast/resource/lostcoast.ttf")
            .ok()
            .flatten()
            .or_else(|| vfs.read("resource/lostcoast.ttf").ok().flatten())
            .or_else(|| {
                vfs.game_root().and_then(|root| {
                    std::fs::read(root.join("lostcoast/resource/lostcoast.ttf")).ok()
                })
            });
        if let Some(bytes) = lc_bytes {
            console.lostcoast_title_metrics = CellMetrics::from_bytes(&bytes).unwrap_or_default();
            console.lostcoast_title_font = FontFace::load(&bytes, canvas.clone()).ok();
            console.lostcoast_title_face =
                console.lostcoast_title_font.as_ref().map(FontFace::face);
        }
        // Installed desktop GameUI colors. Layout/window controls remain an initial implementation.
        if let Some(root) = vfs
            .read("resource/SourceSchemeBase.res")
            .ok()
            .flatten()
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
        {
            console.theme = Theme::from_scheme(&root);
            console.style = Style {
                menu: scheme_color(&root, "MainMenu.TextColor").unwrap_or(console.style.menu),
                menu_armed: scheme_color(&root, "MainMenu.ArmedTextColor")
                    .unwrap_or(console.style.menu_armed),
                panel: scheme_color(&root, "Frame.BgColor").unwrap_or(console.style.panel),
                entry: scheme_color(&root, "RichText.BgColor").unwrap_or(console.style.entry),
                text: scheme_color(&root, "Console.TextColor").unwrap_or(console.style.text),
            };
            console.menu_layout.apply_source_scheme(&root);
        }
        if let Some(root) = vfs
            .read("resource/ClientScheme.res")
            .ok()
            .flatten()
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
        {
            console.menu_layout.apply_client_scheme(&root);
        }
        console
    }

    /// Replaces the GameUI resources and rebuilds both game menus from them.
    pub fn set_resources(&mut self, resources: GameUiResources) {
        let loc = &resources.localization;
        self.main_menu = GameMenu::new(
            resources.game_menu.clone(),
            resources.new_game_maps.clone(),
            loc,
        );
        self.pause_menu = GameMenu::new(
            resources.game_menu.clone(),
            resources.new_game_maps.clone(),
            loc,
        );
        self.pause_menu.set_in_game(true, loc);
        self.category_titles = resources.category_titles.clone();
        self.gameui = Arc::new(resources);
        self.options.set_resources(self.gameui.clone());
    }

    /// Checks whether the primary title font contains glyphs for all non-whitespace characters in `line`.
    pub(crate) fn title_font_can_render(&self, line: &str) -> bool {
        if let Some(face) = &self.title_face {
            !line.is_empty()
                && line
                    .chars()
                    .all(|c| c.is_whitespace() || face.lookup_glyph_index(c) != 0)
        } else {
            false
        }
    }

    /// Returns the main menu title for the given category.
    pub fn category_title(&self, category: &str) -> &str {
        let canonical = match category {
            "hl2ep1" => "episodic",
            "hl2ep2" => "ep2",
            "hl2lc" => "lostcoast",
            other => other,
        };
        self.category_titles
            .get(canonical)
            .or_else(|| self.category_titles.get(category))
            .map(String::as_str)
            .unwrap_or(&self.title)
    }

    /// The active menu title for the current mode and selected category.
    pub fn current_title(&self) -> &str {
        let shown = if self.mode == Mode::Options {
            self.options_return
        } else {
            self.mode
        };
        let category = if shown == Mode::Pause {
            self.pause_menu.category()
        } else {
            self.main_menu.category()
        };
        self.category_title(category)
    }

    /// The active menu category for the current mode.
    pub fn menu_category(&self) -> &str {
        let shown = if self.mode == Mode::Options {
            self.options_return
        } else {
            self.mode
        };
        if shown == Mode::Pause {
            self.pause_menu.category()
        } else {
            self.main_menu.category()
        }
    }

    /// Resolutions for the Video page, from the host's monitor query.
    pub fn set_display_modes(&mut self, modes: DisplayModes) {
        self.options.set_display_modes(modes);
    }

    pub fn resources(&self) -> &GameUiResources {
        &self.gameui
    }

    /// Whether the loaded map only serves as the main-menu background.
    pub fn background_level(&self) -> bool {
        self.background_level
    }

    /// The effective main-menu background map, prioritizing the persisted config override
    /// before falling back to the installed `scripts/chapterbackgrounds.txt`.
    pub fn background_map(&self) -> Option<&str> {
        self.config
            .gameplay
            .background_map
            .as_deref()
            .or(self.gameui.background_map.as_deref())
    }

    /// Overrides the persistent main-menu background map setting.
    pub fn set_background_map(&mut self, map: Option<String>) {
        self.config.gameplay.background_map = map;
    }

    /// Called by the host after a map finished loading. A background level shows the main menu;
    /// any other map starts gameplay.
    pub fn level_loaded(&mut self, background: bool) {
        if background && self.background_transition.is_active() {
            self.background_transition.finish();
        }
        self.loading_dialog = None;
        crate::gameui::loading::reset_progress();
        self.background_level = background;
        self.confirm_box = None;
        if !background {
            self.main_menu.back(&self.gameui.localization);
        }
        self.clear_menu_hover();
        if let Some(pending) = self.pending_map.take() {
            self.loaded_map = Some(pending);
        } else if self.loaded_map.is_none() && background {
            self.loaded_map = self.background_map().map(str::to_owned);
        }
        self.mode = if background {
            Mode::MainMenu
        } else {
            Mode::Gameplay
        };
        self.previous = self.mode;
    }

    /// Displays the modal loading dialog for the given map.
    pub fn start_loading(&mut self, map: &str) {
        self.pending_map = Some(map.to_owned());
        self.clear_menu_hover();
        self.loading_dialog = Some(crate::gameui::loading::LoadingDialog::new(map));
        crate::gameui::loading::reset_progress();
    }

    /// Displays the non-blocking corner plaque for background map loading.
    pub fn start_background_loading(&mut self, map: &str) {
        self.pending_map = Some(map.to_owned());
        self.background_transition.start(map);
        self.loading_dialog = Some(crate::gameui::loading::LoadingDialog::new_background(map));
        crate::gameui::loading::reset_progress();
    }

    /// Updates loading progress from the latest background loader report.
    pub fn update_loading(&mut self) {
        if self.background_transition.is_active() {
            self.background_transition.update(1.0 / 60.0);
        }
        if let Some(loading) = &mut self.loading_dialog {
            loading.update(crate::gameui::loading::current_progress());
        }
    }

    /// Ends the background transition due to a failure and dismisses the loading plaque.
    pub fn background_load_failed(&mut self, reason: &str) {
        self.background_transition.fail(reason);
        if let Some(loading) = &mut self.loading_dialog {
            loading.set_error(reason);
        }
        self.loading_dialog = None;
        self.pending_map = None;
        crate::gameui::loading::reset_progress();
    }

    /// Marks the active loading dialog as failed with the given error message.
    pub fn loading_failed(&mut self, error: &str) {
        self.pending_map = None;
        if let Some(loading) = &mut self.loading_dialog {
            loading.set_error(error);
            if loading.is_background() {
                self.background_transition.fail(error);
                self.loading_dialog = None;
                crate::gameui::loading::reset_progress();
            }
        } else if self.background_transition.is_active() {
            self.background_transition.fail(error);
        }
    }

    /// Clears the loading dialog once the level has completely loaded.
    pub fn loading_complete(&mut self) {
        if let Some(loading) = &mut self.loading_dialog {
            if loading.is_background() {
                self.background_transition.finish();
            }
            loading.set_complete();
        } else if self.background_transition.is_active() {
            self.background_transition.finish();
        }
        self.loading_dialog = None;
        self.pending_map = None;
        self.clear_menu_hover();
        crate::gameui::loading::reset_progress();
    }

    /// Whether a map load is currently in progress.
    pub fn is_loading(&self) -> bool {
        self.loading_dialog.as_ref().is_some_and(|l| !l.is_failed())
    }

    /// Whether background map loading is currently active.
    pub fn is_background_loading(&self) -> bool {
        self.background_transition.is_active()
            || self
                .loading_dialog
                .as_ref()
                .is_some_and(|l| l.is_background() && !l.is_failed() && !l.is_complete())
    }

    /// Returns the current background transition state.
    pub fn background_transition_state(
        &self,
    ) -> &crate::gameui::loading::BackgroundTransitionState {
        &self.background_transition
    }

    /// Whether any modal dialog or blocking widget is currently active over the menus.
    pub fn modal_dialog_open(&self) -> bool {
        self.mode == Mode::Options
            || self.confirm_box.is_some()
            || self
                .loading_dialog
                .as_ref()
                .is_some_and(|l| !l.is_background())
            || self.mode == Mode::Console
    }

    /// Bounding rectangle in canvas pixels of the currently active modal dialog, if any.
    pub fn modal_bounds(&self) -> Option<Rect> {
        if let Some(loading) = &self.loading_dialog {
            if !loading.is_background() {
                let scale = Painter::scale_for(self.canvas.height());
                let canvas = vec2(self.canvas.width(), self.canvas.height());
                let (origin, _) =
                    crate::gameui::loading::LoadingDialog::layout_geometry(canvas, scale);
                let size = crate::gameui::loading::LOADING_DIALOG_SIZE * scale;
                return Some(Rect::new(origin.x, origin.y, size.x, size.y));
            }
        }
        if let Some((confirm, _)) = &self.confirm_box {
            let (origin, scale, layout) = self.quit_geometry(confirm);
            return Some(Rect::new(
                origin.x,
                origin.y,
                layout.size.x * scale,
                layout.size.y * scale,
            ));
        }
        if self.mode == Mode::Options {
            let scale = Painter::scale_for(self.canvas.height());
            let canvas = vec2(self.canvas.width(), self.canvas.height());
            let size = crate::gameui::options::DIALOG_SIZE * scale;
            let origin = ((canvas - size) * 0.5).round();
            return Some(Rect::new(origin.x, origin.y, size.x, size.y));
        }
        if self.mode == Mode::Console {
            return Some(Rect::new(0., 0., self.canvas.width(), self.canvas.height()));
        }
        None
    }

    /// Whether a point in canvas coordinates is covered by an active modal dialog.
    pub fn is_occluded(&self, point: Vec2) -> bool {
        self.modal_bounds().is_some_and(|b| b.contains(point))
    }

    /// Clears hovered and selected state on both main and pause menus so underlying buttons
    /// are not armed while or immediately after a modal dialog is displayed.
    pub fn clear_menu_hover(&mut self) {
        self.main_menu.selection = None;
        self.pause_menu.selection = None;
        self.last_menu_hover = None;
        self.menu_tracker.reset();
    }

    /// Resets mouse tracker and hover state on the menus without wiping keyboard selection.
    pub fn reset_menu_interaction(&mut self) {
        self.last_menu_hover = None;
        self.menu_tracker.reset();
    }

    pub fn paused(&self) -> bool {
        self.mode != Mode::Gameplay
    }

    /// Escape closes the quit box first, then leaves a menu category, then toggles the menus.
    /// The main menu over a background level has nothing to return to.
    pub fn escape(&mut self) {
        if let Some(loading) = &self.loading_dialog {
            if loading.is_failed() {
                self.loading_dialog = None;
                self.mode = Mode::MainMenu;
                self.main_menu.back(&self.gameui.localization);
                self.clear_menu_hover();
            }
            return;
        }
        if self.confirm_box.take().is_some() {
            self.reset_menu_interaction();
            return;
        }
        if self.mode == Mode::Options {
            if !self.options.escape() {
                self.mode = self.options_return;
                self.reset_menu_interaction();
            }
            return;
        }
        let menu = if self.background_level {
            Mode::MainMenu
        } else {
            Mode::Pause
        };
        self.mode = match self.mode {
            Mode::Gameplay => menu,
            Mode::Pause => {
                if self.background_level {
                    Mode::MainMenu
                } else {
                    Mode::Gameplay
                }
            }
            Mode::Console | Mode::Options => self.previous,
            Mode::MainMenu => {
                if self.main_menu.back(&self.gameui.localization) || self.background_level {
                    Mode::MainMenu
                } else {
                    Mode::Gameplay
                }
            }
        };
    }
    pub fn toggle(&mut self) {
        if self.mode == Mode::Console {
            self.mode = self.previous;
            self.clear_menu_hover();
        } else {
            self.previous = self.mode;
            self.clear_menu_hover();
            self.mode = Mode::Console;
        }
    }
    pub fn log(&mut self, text: impl Into<String>) {
        self.output.push_back(text.into());
        if self.output.len() > 256 {
            self.output.pop_front();
        }
        self.scroll = 0;
    }
    pub fn submit(&mut self, line: &str) -> Vec<Effect> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        self.log(format!("] {line}"));
        if self.history.last().is_none_or(|last| last != line) {
            self.history.push(line.to_owned());
            if self.history.len() > 128 {
                self.history.remove(0);
            }
        }
        self.history_index = None;
        self.history_draft.clear();
        let commands = match tokenize(line) {
            Ok(commands) => commands,
            Err(error) => {
                self.log(error);
                return Vec::new();
            }
        };
        let mut effects = Vec::new();
        for tokens in commands {
            match self.interpret(&tokens) {
                Ok(Some(effect)) => effects.push(effect),
                Ok(None) => (),
                Err(error) => self.log(error),
            }
        }
        effects
    }
    fn interpret(&mut self, tokens: &[String]) -> Result<Option<Effect>, String> {
        let name = tokens[0].to_ascii_lowercase();
        let args = &tokens[1..];
        let usage = || {
            COMMANDS.iter().find(|(n, _)| *n == name).map_or_else(
                || format!("Unknown or unimplemented command: {}", tokens[0]),
                |(_, d)| d.to_string(),
            )
        };
        let cheat = || {
            if self.cheats {
                Ok(())
            } else {
                Err(format!("Can't use {name} without sv_cheats 1"))
            }
        };
        match name.as_str() {
            "sv_cheats" => {
                if args.is_empty() {
                    self.log(format!("\"sv_cheats\" = \"{}\"", u8::from(self.cheats)));
                } else if args.len() == 1 {
                    self.cheats = boolean(&args[0]).ok_or_else(usage)?;
                    self.log(format!("sv_cheats {}", u8::from(self.cheats)));
                } else {
                    return Err(usage());
                }
            }
            "impulse" => {
                if args != ["101"] {
                    return Err(usage());
                }
                cheat()?;
                return Ok(Some(Effect::Loadout));
            }
            "noclip" => {
                let value = match args {
                    [] => None,
                    [value] => Some(boolean(value).ok_or_else(usage)?),
                    _ => return Err(usage()),
                };
                cheat()?;
                return Ok(Some(Effect::Noclip(value)));
            }
            "getpos" => {
                if !args.is_empty() {
                    return Err(usage());
                }
                return Ok(Some(Effect::Getpos));
            }
            "setpos" | "setang" => {
                let value = vector(args).ok_or_else(usage)?;
                cheat()?;
                if name == "setang" && value.z != 0. {
                    return Err("setang: camera roll is not implemented; use zero roll".into());
                }
                return Ok(Some(if name == "setpos" {
                    Effect::Setpos {
                        x: value.x,
                        y: value.y,
                        z: (args.len() == 3).then_some(value.z),
                    }
                } else {
                    Effect::Setang(value)
                }));
            }
            "map" => {
                if args.len() != 1 {
                    return Err(usage());
                }
                let map = args[0].strip_suffix(".bsp").unwrap_or(&args[0]);
                if map.is_empty()
                    || map.len() > 128
                    || !map.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err("map: use a simple installed map name".into());
                }
                self.start_loading(map);
                return Ok(Some(Effect::Map(map.to_owned())));
            }
            "ent_fire" => {
                cheat()?;
                if args.is_empty() || args.len() > 4 || args[0].is_empty() {
                    return Err(usage());
                }
                let delay = args
                    .get(3)
                    .map_or(Ok(0.), |s| s.parse::<f64>())
                    .map_err(|_| usage())?;
                if !delay.is_finite() || delay < 0. {
                    return Err("ent_fire: delay must be finite and nonnegative".into());
                }
                let input = args.get(1).map_or("Use", String::as_str);
                if input.is_empty() {
                    return Err(usage());
                }
                return Ok(Some(Effect::Fire {
                    target: args[0].clone(),
                    input: input.into(),
                    parameter: args.get(2).cloned().unwrap_or_default(),
                    // Desktop ent_fire parses an integer delay; map I/O keeps fractions.
                    delay: delay.trunc(),
                }));
            }
            "help" | "find" => {
                if args.len() > 1 || name == "find" && args.is_empty() {
                    return Err(usage());
                }
                let query = args.first().map(|s| s.to_ascii_lowercase());
                let found: Vec<_> = COMMANDS
                    .iter()
                    .filter(|(n, description)| {
                        query.as_ref().is_none_or(|q| {
                            if name == "help" {
                                n == q
                            } else {
                                description.to_ascii_lowercase().contains(q)
                            }
                        })
                    })
                    .collect();
                if found.is_empty() {
                    self.log("No implemented command matches.");
                }
                for (_, description) in found {
                    self.log(*description);
                }
            }
            "clear" => {
                if !args.is_empty() {
                    return Err(usage());
                }
                self.output.clear();
            }
            "echo" => self.log(args.join(" ")),
            "toggleconsole" => {
                if !args.is_empty() {
                    return Err(usage());
                }
                self.toggle();
            }
            "mainmenu" => {
                if !args.is_empty() {
                    return Err(usage());
                }
                self.previous = self.mode;
                self.mode = Mode::MainMenu;
            }
            "quit" => {
                if !args.is_empty() {
                    return Err(usage());
                }
                return Ok(Some(Effect::Quit));
            }
            "volume" => {
                if args.is_empty() {
                    self.log(format!("\"volume\" = \"{:.2}\"", self.config.audio.volume));
                } else if args.len() == 1 {
                    let vol: f32 = args[0].parse().map_err(|_| usage())?;
                    let vol = vol.clamp(0.0, 1.0);
                    self.config.audio.volume = vol;
                    let _ = self.config.save();
                    self.log(format!("volume {:.2}", vol));
                    return Ok(Some(Effect::ApplyConfig(Box::new(self.config.clone()))));
                } else {
                    return Err(usage());
                }
            }
            "sensitivity" => {
                if args.is_empty() {
                    self.log(format!(
                        "\"sensitivity\" = \"{:.2}\"",
                        self.config.mouse.sensitivity
                    ));
                } else if args.len() == 1 {
                    let sens: f32 = args[0].parse().map_err(|_| usage())?;
                    let sens = sens.clamp(0.1, 20.0);
                    self.config.mouse.sensitivity = sens;
                    let _ = self.config.save();
                    self.log(format!("sensitivity {:.2}", sens));
                    return Ok(Some(Effect::ApplyConfig(Box::new(self.config.clone()))));
                } else {
                    return Err(usage());
                }
            }
            "fov" => {
                if args.is_empty() {
                    self.log(format!("\"fov\" = \"{:.0}\"", self.config.video.fov));
                } else if args.len() == 1 {
                    let fov: f32 = args[0].parse().map_err(|_| usage())?;
                    let fov = fov.clamp(50.0, 120.0);
                    self.config.video.fov = fov;
                    let _ = self.config.save();
                    self.log(format!("fov {:.0}", fov));
                    return Ok(Some(Effect::ApplyConfig(Box::new(self.config.clone()))));
                } else {
                    return Err(usage());
                }
            }
            "mat_hdr_level" => {
                if args.is_empty() {
                    let lvl = self.config.video.hdr.level();
                    self.log(format!("\"mat_hdr_level\" = \"{lvl}\""));
                } else if args.len() == 1 {
                    let lvl: u8 = args[0].parse().map_err(|_| usage())?;
                    self.config.video.hdr =
                        crate::config::HdrMode::from_level(lvl).ok_or_else(|| {
                            "mat_hdr_level must be 0 (None), 1 (Bloom) or 2 (Full)".to_owned()
                        })?;
                    let _ = self.config.save();
                    self.log(format!("mat_hdr_level {lvl}"));
                    return Ok(Some(Effect::ApplyConfig(Box::new(self.config.clone()))));
                } else {
                    return Err(usage());
                }
            }
            "mat_setvideomode" => {
                let [w, h, windowed] = args else {
                    return Err(usage());
                };
                let width: u32 = w.parse().map_err(|_| usage())?;
                let height: u32 = h.parse().map_err(|_| usage())?;
                if !(320..=8192).contains(&width) || !(240..=8192).contains(&height) {
                    return Err(usage());
                }
                let display_mode = match windowed.as_str() {
                    "1" => crate::config::DisplayMode::Windowed,
                    "0" => crate::config::DisplayMode::Fullscreen,
                    _ => return Err(usage()),
                };
                self.config.video.width = width;
                self.config.video.height = height;
                self.config.video.display_mode = display_mode;
                let _ = self.config.save();
                self.log(format!("mat_setvideomode {width} {height} {windowed}"));
                return Ok(Some(Effect::ApplyConfig(Box::new(self.config.clone()))));
            }
            "background_map" => {
                if args.is_empty() {
                    match self.background_map() {
                        Some(map) => self.log(format!("\"background_map\" = \"{map}\"")),
                        None => self.log("\"background_map\" is not set"),
                    }
                } else if args.len() == 1 {
                    let map = args[0].strip_suffix(".bsp").unwrap_or(&args[0]);
                    if map.eq_ignore_ascii_case("default")
                        || map == "0"
                        || map.eq_ignore_ascii_case("none")
                        || map.is_empty()
                    {
                        self.config.gameplay.background_map = None;
                        let _ = self.config.save();
                        self.log(format!(
                            "background_map reset to default ({})",
                            self.gameui.background_map.as_deref().unwrap_or("none")
                        ));
                    } else {
                        if map.len() > 128
                            || !map.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        {
                            return Err("background_map: use a simple installed map name".into());
                        }
                        self.config.gameplay.background_map = Some(map.to_owned());
                        let _ = self.config.save();
                        self.log(format!("background_map {map}"));
                    }
                    return Ok(Some(Effect::ApplyConfig(Box::new(self.config.clone()))));
                } else {
                    return Err(usage());
                }
            }
            "map_background" => {
                if args.len() != 1 {
                    return Err(usage());
                }
                let map = args[0].strip_suffix(".bsp").unwrap_or(&args[0]);
                if map.is_empty()
                    || map.len() > 128
                    || !map.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err("map_background: use a simple installed map name".into());
                }
                if self.is_loading() && !self.background_level {
                    self.config.gameplay.background_map = Some(map.to_owned());
                    let _ = self.config.save();
                    self.log("Gameplay map load in progress; background map persisted without interrupting load.");
                    return Ok(None);
                }
                self.config.gameplay.background_map = Some(map.to_owned());
                let _ = self.config.save();
                self.start_background_loading(map);
                return Ok(Some(Effect::MainMenuMap(map.to_owned())));
            }
            _ => return Err(usage()),
        }
        Ok(None)
    }
    fn history_move(&mut self, older: bool) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.history_draft = self.input.clone();
        }
        self.history_index = if older {
            Some(
                self.history_index
                    .map_or(self.history.len() - 1, |i| i.saturating_sub(1)),
            )
        } else {
            self.history_index
                .and_then(|i| (i + 1 < self.history.len()).then_some(i + 1))
        };
        self.input = self
            .history_index
            .map_or_else(|| self.history_draft.clone(), |i| self.history[i].clone());
        self.cursor = self.input.len();
    }
    /// Called before gameplay input collection. Opening characters are drained by the host.
    pub fn input(&mut self, input: &Input) -> Vec<Effect> {
        if let Some(loading) = &self.loading_dialog {
            if loading.is_failed() && (input.pressed(Key::Enter) || input.click) {
                self.loading_dialog = None;
                self.mode = Mode::MainMenu;
                self.main_menu.back(&self.gameui.localization);
                self.clear_menu_hover();
            }
            return Vec::new();
        }
        if self.confirm_box.is_some() {
            return self.confirm_input(input);
        }
        if self.mode == Mode::Options {
            return self.options_input(input);
        }
        if self.mode == Mode::Console {
            return self.console_input(input);
        }
        if self.mode == Mode::Pause || self.mode == Mode::MainMenu {
            return self.menu_input(input);
        }
        Vec::new()
    }

    fn options_input(&mut self, input: &Input) -> Vec<Effect> {
        let measure = Painter {
            canvas: &self.canvas,
            fonts: &self.widget_fonts,
            theme: &self.theme,
            origin: Vec2::ZERO,
            scale: 1.,
        };
        let outcome = self.options.input(input, &measure);
        for line in self.options.take_log() {
            self.log(line);
        }
        let mut effects: Vec<Effect> = self
            .options
            .take_sounds()
            .into_iter()
            .map(Effect::PlaySound)
            .collect();
        let (config, close) = match outcome {
            OptionsOutcome::None => return effects,
            OptionsOutcome::Apply(config) => (Some(config), false),
            OptionsOutcome::Close(config) => (config, true),
        };
        if close {
            self.mode = self.options_return;
            self.reset_menu_interaction();
        }
        let Some(config) = config else {
            return effects;
        };
        if let Err(error) = config.save() {
            self.log(format!("Could not save settings: {error}"));
        }
        self.config = config.clone();
        effects.push(Effect::ApplyConfig(Box::new(config)));
        effects
    }

    fn confirm_input(&mut self, input: &Input) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some((mut confirm, kind)) = self.confirm_box.take() {
            let (origin, scale, layout) = self.quit_geometry(&confirm);
            let pointer = input.pointer.map(|p| (p - origin) / scale);
            let down = input.click || input.held;
            let outcome = confirm.input(
                &layout,
                pointer,
                down,
                input.click,
                input.released,
                input.pressed(Key::Enter),
            );
            for sound in confirm.take_sounds() {
                effects.push(Effect::PlaySound(sound));
            }
            match outcome {
                Some(true) => match kind {
                    ConfirmKind::Quit => {
                        effects.push(Effect::Quit);
                        return effects;
                    }
                    ConfirmKind::ReturnToMainMenu => {
                        self.mode = Mode::MainMenu;
                        self.clear_menu_hover();
                        match self.background_map().map(str::to_owned) {
                            Some(map) => effects.push(Effect::MainMenuMap(map)),
                            None => {
                                self.log("No main-menu background map is installed; keeping the current map frozen behind the menu.");
                                self.level_loaded(true);
                            }
                        }
                        return effects;
                    }
                },
                Some(false) => {
                    self.reset_menu_interaction();
                }
                None => self.confirm_box = Some((confirm, kind)),
            }
        }
        effects
    }

    fn console_input(&mut self, input: &Input) -> Vec<Effect> {
        for c in input.text.chars() {
            if !c.is_control() && c != '`' && c != '~' && self.input.len() + c.len_utf8() <= 1024 {
                self.input.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
        }
        if input.pressed(Key::Backspace) && self.cursor > 0 {
            let previous = self.input[..self.cursor].char_indices().last().unwrap().0;
            self.input.replace_range(previous..self.cursor, "");
            self.cursor = previous;
        }
        if input.pressed(Key::Delete) && self.cursor < self.input.len() {
            let length = self.input[self.cursor..].chars().next().unwrap().len_utf8();
            self.input
                .replace_range(self.cursor..self.cursor + length, "");
        }
        if input.pressed(Key::Left) && self.cursor > 0 {
            self.cursor = self.input[..self.cursor].char_indices().last().unwrap().0;
        }
        if input.pressed(Key::Right) && self.cursor < self.input.len() {
            self.cursor += self.input[self.cursor..].chars().next().unwrap().len_utf8();
        }
        if input.pressed(Key::Home) {
            self.cursor = 0;
        }
        if input.pressed(Key::End) {
            self.cursor = self.input.len();
        }
        if input.pressed(Key::Up) {
            self.history_move(true);
        }
        if input.pressed(Key::Down) {
            self.history_move(false);
        }
        if input.pressed(Key::PageUp) {
            self.scroll = (self.scroll + 10).min(self.output.len().saturating_sub(1));
        }
        if input.pressed(Key::PageDown) {
            self.scroll = self.scroll.saturating_sub(10);
        }
        if input.pressed(Key::Tab) {
            let matches: Vec<_> = COMMANDS
                .iter()
                .filter(|(name, _)| name.starts_with(&self.input))
                .collect();
            if matches.len() == 1 {
                self.input = format!("{} ", matches[0].0);
                self.cursor = self.input.len();
            } else if matches.len() > 1 {
                self.log(
                    matches
                        .into_iter()
                        .map(|(n, _)| *n)
                        .collect::<Vec<_>>()
                        .join("  "),
                );
            }
        }
        if input.pressed(Key::Enter) {
            let line = std::mem::take(&mut self.input);
            self.cursor = 0;
            return self.submit(&line);
        }
        Vec::new()
    }
    /// The menu shown in the current mode, if any.
    fn game_menu(&self) -> Option<&GameMenu> {
        match self.mode {
            Mode::MainMenu => Some(&self.main_menu),
            Mode::Pause => Some(&self.pause_menu),
            _ => None,
        }
    }

    /// One hit rectangle per menu row (spacers included), in canvas pixels.
    fn menu_rows(&self, count: usize) -> Vec<Rect> {
        self.menu_layout
            .geometry(self.canvas.height())
            .row_rects(count, self.canvas.height())
    }

    fn painter(&self, origin: Vec2, scale: f32) -> Painter<'_> {
        Painter {
            canvas: &self.canvas,
            fonts: &self.widget_fonts,
            theme: &self.theme,
            origin,
            scale,
        }
    }

    /// Canvas origin, scale and layout of the open quit box (centered on the canvas).
    fn quit_geometry(&self, quit: &MessageBox) -> (Vec2, f32, MessageBoxLayout) {
        let scale = Painter::scale_for(self.canvas.height());
        let measure = self.painter(Vec2::ZERO, 1.);
        let layout = quit.layout(|text| measure.text_width(text, false));
        let canvas = vec2(self.canvas.width(), self.canvas.height());
        let origin = ((canvas - layout.size * scale) * 0.5).round();
        (origin, scale, layout)
    }

    fn menu_input(&mut self, input: &Input) -> Vec<Effect> {
        if self.modal_dialog_open() {
            return Vec::new();
        }
        let mut effects = Vec::new();
        let count = self.game_menu().map_or(0, |menu| menu.entries().len());
        let rows = self.menu_rows(count);
        let menu = match self.mode {
            Mode::MainMenu => &mut self.main_menu,
            _ => &mut self.pause_menu,
        };
        if input.pressed(Key::Up) {
            let prev = menu.selection;
            menu.step(-1);
            if menu.selection != prev
                && menu
                    .selection
                    .and_then(|i| menu.entries().get(i))
                    .is_some_and(|e| e.has_rollover_sound())
            {
                effects.push(Effect::PlaySound("ui/buttonrollover.wav".into()));
            }
        }
        if input.pressed(Key::Down) {
            let prev = menu.selection;
            menu.step(1);
            if menu.selection != prev
                && menu
                    .selection
                    .and_then(|i| menu.entries().get(i))
                    .is_some_and(|e| e.has_rollover_sound())
            {
                effects.push(Effect::PlaySound("ui/buttonrollover.wav".into()));
            }
        }
        let hovered = input
            .pointer
            .and_then(|p| rows.iter().position(|row| row.contains(p)))
            .filter(|&i| menu.entries().get(i).is_some_and(|e| e.enabled));
        if let Some(index) = hovered {
            menu.selection = Some(index);
        }
        if hovered.is_some()
            && hovered != self.last_menu_hover
            && hovered
                .and_then(|i| menu.entries().get(i))
                .is_some_and(|e| e.has_rollover_sound())
        {
            effects.push(Effect::PlaySound("ui/buttonrollover.wav".into()));
        }
        self.last_menu_hover = hovered;
        let down = input.click || input.held;
        let mut activate = self
            .menu_tracker
            .update(hovered, down, input.click, input.released);
        if input.pressed(Key::Enter) {
            activate = activate.or(menu.selection);
        }
        if let Some(index) = activate {
            effects.push(Effect::PlaySound("ui/buttonclickrelease.wav".into()));
            effects.extend(self.activate_menu(index));
        }
        effects
    }

    pub fn activate_menu(&mut self, index: usize) -> Vec<Effect> {
        let gameui = self.gameui.clone();
        let loc = &gameui.localization;
        let outcome = match self.mode {
            Mode::MainMenu => self.main_menu.activate(index, loc),
            Mode::Pause => self.pause_menu.activate(index, loc),
            _ => MenuOutcome::None,
        };
        match outcome {
            MenuOutcome::None => {}
            MenuOutcome::Resume => {
                if !self.background_level {
                    self.mode = Mode::Gameplay;
                }
            }
            MenuOutcome::OpenOptions => {
                self.options_return = self.mode;
                self.mode = Mode::Options;
                self.reset_menu_interaction();
                self.options.open(self.config.clone());
            }
            MenuOutcome::ConfirmQuit => {
                self.reset_menu_interaction();
                self.confirm_box = Some((MessageBox::quit(loc), ConfirmKind::Quit));
            }
            MenuOutcome::NewGame(map) => {
                self.clear_menu_hover();
                self.start_loading(&map);
                return vec![Effect::Map(map)];
            }
            MenuOutcome::ReturnToMainMenu => {
                self.reset_menu_interaction();
                self.confirm_box = Some((
                    MessageBox::return_to_main_menu(loc),
                    ConfirmKind::ReturnToMainMenu,
                ));
            }
            MenuOutcome::SwitchExpansion(game) => {
                if let Some(target_map) = self.gameui.background_map_for(&game).map(str::to_owned) {
                    self.config.gameplay.background_map = Some(target_map.clone());
                    let _ = self.config.save();
                    if !self.background_level || self.loaded_map.as_deref() != Some(&target_map) {
                        self.start_background_loading(&target_map);
                        return vec![Effect::MainMenuMap(target_map)];
                    }
                }
            }
            MenuOutcome::SwitchCategory(category) => {
                if category != "main" && category != "extras" {
                    if let Some(target_map) =
                        self.gameui.background_map_for(&category).map(str::to_owned)
                    {
                        self.config.gameplay.background_map = Some(target_map.clone());
                        let _ = self.config.save();
                        if !self.background_level || self.loaded_map.as_deref() != Some(&target_map)
                        {
                            self.start_background_loading(&target_map);
                            return vec![Effect::MainMenuMap(target_map)];
                        }
                    }
                }
            }
            MenuOutcome::Placeholder(label) => {
                self.log(format!("{label}: not implemented in HL2-RS yet."));
            }
        }
        Vec::new()
    }
    /// Four compact header rows and a footer, preserving the retained host's developer layout.
    /// Font data is bundled under its own MIT license, independent of installed game content.
    pub fn draw_debug(&self, lines: &[String]) {
        self.canvas.rectangle(
            0.,
            0.,
            self.canvas.width(),
            105.,
            Color::new(0.025, 0.045, 0.07, 0.88),
        );
        let rows = [
            (vec2(22., 30.), 27., WHITE),
            (vec2(22., 55.), 19., Color::new(0.784, 0.784, 0.784, 1.)),
            (vec2(22., 78.), 17., Color::new(0.784, 0.784, 0.784, 1.)),
            (vec2(22., 98.), 16., Color::from_rgba(240, 180, 90, 255)),
        ];
        let draw = |line: &str, origin: Vec2, size: f32, color| {
            if let Some(font) = &self.developer_font {
                font.draw_baseline(line, origin, size, 1., color);
            } else {
                text(
                    &self.canvas,
                    line,
                    origin,
                    size,
                    1.,
                    self.mono.as_ref().or(self.font.as_ref()),
                    color,
                );
            }
        };
        for (line, (origin, size, color)) in lines.iter().take(4).zip(rows) {
            draw(line, origin, size, color);
        }
        if let Some(footer) = lines.get(4) {
            self.canvas.rectangle(
                0.,
                self.canvas.height() - 38.,
                self.canvas.width(),
                38.,
                Color::new(0.025, 0.045, 0.07, 0.85),
            );
            draw(footer, vec2(20., self.canvas.height() - 13.), 18., WHITE);
        }
    }

    pub fn draw(&self, time: f64) {
        if self.mode == Mode::Gameplay && self.loading_dialog.is_none() {
            return;
        }
        if self.mode == Mode::Gameplay {
            if let Some(loading) = &self.loading_dialog {
                let scale = Painter::scale_for(self.canvas.height());
                loading.draw(&self.painter(Vec2::ZERO, scale), &self.gameui.localization);
            }
            return;
        }
        // The options dialog is drawn over the menu it was opened from.
        let shown = if self.mode == Mode::Options {
            self.options_return
        } else {
            self.mode
        };
        let menu_shown = shown == Mode::Pause || shown == Mode::MainMenu;
        let scale = self.canvas.height() / 720.;
        let text = |content: &str, x, y, size, color, mono: bool| {
            text(
                &self.canvas,
                content,
                vec2(x, y),
                (size * scale) as u16 as f32,
                1.,
                if mono {
                    self.mono.as_ref()
                } else if menu_shown {
                    self.menu_font.as_ref()
                } else {
                    self.font.as_ref()
                },
                color,
            );
        };
        if menu_shown {
            if let Some(loading) = &self.loading_dialog {
                if loading.is_background() {
                    loading.draw_background_overlay(&self.canvas);
                }
            } else if self.background_transition.is_active() {
                let alpha = self.background_transition.overlay_alpha();
                if alpha > 0.0 {
                    self.canvas.blur(
                        0.,
                        0.,
                        self.canvas.width(),
                        self.canvas.height(),
                        Color::new(0.0, 0.0, 0.0, alpha),
                    );
                }
            }
            let geometry = self.menu_layout.geometry(self.canvas.height());
            let title = self.current_title();
            if let Some(face) = &self.title_face {
                let em = geometry.title_tall * self.title_metrics.em_per_cell;
                for (line_idx, line) in title.lines().enumerate() {
                    let mut pen = match line_idx {
                        0 => geometry.title,
                        1 => geometry.title2,
                        _ => {
                            geometry.title2
                                + vec2(
                                    0.,
                                    (line_idx - 1) as f32
                                        * (geometry.title2.y - geometry.title.y).max(1.),
                                )
                        }
                    };
                    if line_idx == 0 {
                        let mut chars = Vec::with_capacity(line.len());
                        for character in line.chars() {
                            chars.push((character, pen));
                            // GDI supplies integer ABC advances. Fractional accumulation visibly widens this logo.
                            pen.x += face.metrics(character, em).advance_width.round();
                        }
                        for &(character, char_pen) in &chars {
                            draw_cell_text_outlined(
                                &character.to_string(),
                                char_pen,
                                geometry.title_tall,
                                self.title_font.as_ref(),
                                self.title_metrics,
                                Color::new(0., 0., 0., 0.),
                                TextOutline::DEFAULT,
                            );
                        }
                        for &(character, char_pen) in &chars {
                            draw_cell_text(
                                &self.canvas,
                                &character.to_string(),
                                char_pen,
                                geometry.title_tall,
                                self.title_font.as_ref(),
                                self.title_metrics,
                                WHITE,
                            );
                        }
                    } else {
                        // Subtitle line (e.g. episode or lost coast submenu subtitle).
                        let is_lostcoast = line.to_ascii_lowercase().contains("lost coast")
                            || line.contains("lost coasT");
                        if is_lostcoast && self.lostcoast_title_font.is_some() {
                            draw_cell_text_outlined(
                                line,
                                pen,
                                geometry.title_tall,
                                self.lostcoast_title_font.as_ref(),
                                self.lostcoast_title_metrics,
                                Color::from_rgba(255, 255, 255, 200),
                                TextOutline::DEFAULT,
                            );
                        } else if self.title_font_can_render(line) {
                            draw_cell_text_outlined(
                                line,
                                pen,
                                geometry.title_tall,
                                self.title_font.as_ref(),
                                self.title_metrics,
                                Color::from_rgba(255, 255, 255, 200),
                                TextOutline::DEFAULT,
                            );
                        } else {
                            let sub_tall = (geometry.title_tall * 0.55).round().max(14.);
                            draw_cell_text_outlined(
                                line,
                                pen,
                                sub_tall,
                                self.menu_font.as_ref(),
                                self.menu_metrics,
                                Color::from_rgba(255, 255, 255, 200),
                                TextOutline::DEFAULT,
                            );
                        }
                    }
                }
            } else {
                for (line_idx, line) in title.lines().enumerate() {
                    let pen = match line_idx {
                        0 => geometry.title,
                        1 => geometry.title2,
                        _ => {
                            geometry.title2
                                + vec2(
                                    0.,
                                    (line_idx - 1) as f32
                                        * (geometry.title2.y - geometry.title.y).max(1.),
                                )
                        }
                    };
                    let tall = if line_idx == 0 {
                        geometry.title_tall
                    } else {
                        (geometry.title_tall * 0.55).round().max(14.)
                    };
                    draw_cell_text_outlined(
                        line,
                        pen,
                        tall,
                        self.menu_font.as_ref(),
                        self.menu_metrics,
                        if line_idx == 0 {
                            WHITE
                        } else {
                            Color::from_rgba(255, 255, 255, 200)
                        },
                        TextOutline::DEFAULT,
                    );
                }
            }
            let menu = if shown == Mode::MainMenu {
                &self.main_menu
            } else {
                &self.pause_menu
            };
            let modal_open = self.modal_dialog_open();
            for (i, entry) in menu.entries().iter().enumerate() {
                if entry.spacer {
                    continue;
                }
                let is_pressed = !modal_open && self.menu_tracker.is_pressed(i);
                let armed = !modal_open && (is_pressed || menu.selection == Some(i));
                let color = if !entry.enabled {
                    self.theme.text_disabled
                } else if armed {
                    self.style.menu_armed
                } else {
                    self.style.menu
                };
                let border_color = if !entry.enabled {
                    Color::new(0.0, 0.0, 0.0, 0.25)
                } else if armed {
                    Color::new(0.0, 0.0, 0.0, 0.65)
                } else {
                    Color::new(0.0, 0.0, 0.0, 0.45)
                };
                let offset = if is_pressed { vec2(1., 1.) } else { Vec2::ZERO };
                draw_cell_text_outlined(
                    &entry.label,
                    geometry.row_text_pos(i) + offset,
                    geometry.font_tall,
                    self.menu_font.as_ref(),
                    self.menu_metrics,
                    color,
                    TextOutline::new(border_color, 1.0),
                );
            }
            if let Some((confirm, _)) = &self.confirm_box {
                let (origin, scale, layout) = self.quit_geometry(confirm);
                confirm.draw(&self.painter(origin, scale), &layout);
            }
            if self.mode == Mode::Options {
                self.options.draw(&self.painter(Vec2::ZERO, 1.));
            }
        } else {
            let left = 32. * scale;
            let top = 32. * scale;
            let width = (self.canvas.width() - 64. * scale).min(1000. * scale);
            let height = self.canvas.height() - 64. * scale;
            self.canvas.rectangle(
                0.,
                0.,
                self.canvas.width(),
                self.canvas.height(),
                Color::new(0., 0., 0., 0.3),
            );
            self.canvas
                .rectangle(left, top, width, height, self.style.panel);
            rectangle_lines(
                &self.canvas,
                left,
                top,
                width,
                height,
                1.,
                Color::from_rgba(150, 158, 147, 255),
            );
            let console_title = self.gameui.localization.text("#GameUI_Console");
            text(
                &console_title,
                left + 12. * scale,
                top + 24. * scale,
                18.,
                WHITE,
                false,
            );
            self.canvas.rectangle(
                left + 10. * scale,
                top + 36. * scale,
                width - 20. * scale,
                height - 88. * scale,
                self.style.entry,
            );
            let rows = ((height / scale - 108.) / 19.).max(1.) as usize;
            let end = self.output.len().saturating_sub(self.scroll);
            let start = end.saturating_sub(rows);
            for (i, line) in self.output.iter().skip(start).take(end - start).enumerate() {
                // Bound horizontal text to the panel. Full output remains in runtime reports.
                let columns = ((width / scale - 36.) / 9.).max(1.) as usize;
                let mut clipped = String::new();
                for c in line.chars().take(columns) {
                    let mut candidate = clipped.clone();
                    candidate.push(c);
                    if self.mono.as_ref().is_some_and(|font| {
                        font.width_em(&candidate, (16. * scale) as u16 as f32) > width - 36. * scale
                    }) {
                        break;
                    }
                    clipped = candidate;
                }
                text(
                    &clipped,
                    left + 18. * scale,
                    top + (57. + i as f32 * 19.) * scale,
                    16.,
                    self.style.text,
                    true,
                );
            }
            let bottom = top + height - 38. * scale;
            self.canvas.rectangle(
                left + 10. * scale,
                bottom,
                width - 20. * scale,
                28. * scale,
                self.style.entry,
            );
            let columns = ((width / scale - 40.) / 9.).max(1.) as usize;
            let preceding = self.input[..self.cursor].chars().count();
            let start = preceding.saturating_sub(columns.saturating_sub(1));
            let shown: String = self.input.chars().skip(start).take(columns).collect();
            text(
                &format!("] {shown}"),
                left + 15. * scale,
                bottom + 20. * scale,
                16.,
                WHITE,
                true,
            );
            if ((time * 2.) as u64).is_multiple_of(2) {
                let prefix = format!(
                    "] {}",
                    self.input
                        .chars()
                        .skip(start)
                        .take(preceding - start)
                        .collect::<String>()
                );
                let cursor_x = self.mono.as_ref().map_or(0., |font| {
                    font.width_em(&prefix, (16. * scale) as u16 as f32)
                });
                self.canvas.rectangle(
                    left + 15. * scale + cursor_x,
                    bottom + 5. * scale,
                    1.,
                    18. * scale,
                    WHITE,
                );
            }
        }
        if let Some(loading) = &self.loading_dialog {
            let scale = Painter::scale_for(self.canvas.height());
            loading.draw(&self.painter(Vec2::ZERO, scale), &self.gameui.localization);
        }
    }
}

fn boolean(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}
fn vector(args: &[String]) -> Option<glam::Vec3> {
    if !(2..=3).contains(&args.len()) {
        return None;
    }
    let values: Vec<f32> = args
        .iter()
        .map(|v| v.parse())
        .collect::<Result<_, _>>()
        .ok()?;
    values
        .iter()
        .all(|v| v.is_finite())
        .then(|| glam::Vec3::new(values[0], values[1], values.get(2).copied().unwrap_or(0.)))
}
/// Semicolons outside quotes separate commands; quoted arguments preserve spaces and separators.
fn tokenize(line: &str) -> Result<Vec<Vec<String>>, String> {
    let mut commands = Vec::new();
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            ';' if !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
                if !tokens.is_empty() {
                    commands.push(std::mem::take(&mut tokens));
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
            }
            _ => {
                token.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err("Unterminated quoted argument; no commands executed".into());
    }
    if started {
        tokens.push(token);
    }
    if !tokens.is_empty() {
        commands.push(tokens);
    }
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameui::localize::Localization;
    use crate::gameui::resources::{MenuItem, Visibility};
    #[test]
    fn cell_metrics_rejects_missing_font() {
        assert!(CellMetrics::from_bytes(b"missing font").is_none());
    }
    #[test]
    fn cheats_are_gated_in_order_and_queries_do_not_change_them() {
        let mut console = Console::default();
        assert!(console
            .submit("impulse 101; noclip; setpos 1 2 3; setang 0 0 0")
            .is_empty());
        assert_eq!(
            console.submit("sv_cheats 1; impulse 101; noclip 1"),
            vec![Effect::Loadout, Effect::Noclip(Some(true))]
        );
        assert!(console.submit("sv_cheats").is_empty());
        assert!(console.cheats);
        assert!(console.submit("sv_cheats 0; impulse 101").is_empty());
        assert!(!console.cheats);
    }
    #[test]
    fn quoted_separators_are_text_and_bad_quotes_execute_nothing() {
        let mut console = Console::default();
        console.submit("echo \"hello; world\"; echo done");
        assert!(console.output.iter().any(|line| line == "hello; world"));
        assert!(console.output.iter().any(|line| line == "done"));
        console.submit("sv_cheats 1; echo \"unfinished");
        assert!(!console.cheats);
    }
    #[test]
    fn invalid_numbers_paths_impulses_and_unknown_commands_do_not_succeed() {
        let mut console = Console::default();
        console.submit("sv_cheats 1");
        for line in [
            "setpos NaN 0 0",
            "setpos inf 0 0",
            "setpos 1",
            "setang 0 0 20",
            "map ../other",
            "map a/b",
            "map C:\\foo",
            "impulse 42",
            "god",
            "sv_cheats 2",
        ] {
            assert!(console.submit(line).is_empty(), "{line}");
        }
        assert_eq!(
            console.submit("map d1_trainstation_02.bsp"),
            vec![Effect::Map("d1_trainstation_02".into())]
        );
        assert_eq!(
            console.submit("setpos -10 20 30"),
            vec![Effect::Setpos {
                x: -10.,
                y: 20.,
                z: Some(30.)
            }]
        );
        assert_eq!(
            console.submit("setpos 1 2"),
            vec![Effect::Setpos {
                x: 1.,
                y: 2.,
                z: None
            }]
        );
    }

    #[test]
    fn entity_inputs_preserve_quoted_values_and_enforce_cheat_and_delay_rules() {
        let mut console = Console::default();
        assert!(console.submit("ent_fire security03 Start").is_empty());
        assert_eq!(
            console.submit("sv_cheats 1; ent_fire security_* Start \"value;kept\" 0.1"),
            vec![Effect::Fire {
                target: "security_*".into(),
                input: "Start".into(),
                parameter: "value;kept".into(),
                delay: 0.,
            }]
        );
        assert_eq!(
            console.submit("ent_fire panel"),
            vec![Effect::Fire {
                target: "panel".into(),
                input: "Use".into(),
                parameter: String::new(),
                delay: 0.,
            }]
        );
        for command in [
            "ent_fire",
            "ent_fire door Open x NaN",
            "ent_fire door Open x -1",
            "ent_fire door Open x inf",
        ] {
            assert!(console.submit(command).is_empty());
        }
    }
    #[test]
    fn console_returns_to_its_previous_pause_state_and_history_restores_draft() {
        let mut console = Console::default();
        console.escape();
        assert!(console.paused());
        console.toggle();
        assert_eq!(console.mode, Mode::Console);
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        console.escape();
        assert!(!console.paused());
        console.submit("echo first");
        console.submit("echo second");
        console.input = "draft".into();
        console.history_move(true);
        assert_eq!(console.input, "echo second");
        console.history_move(true);
        assert_eq!(console.input, "echo first");
        console.history_move(false);
        assert_eq!(console.input, "echo second");
        console.history_move(false);
        assert_eq!(console.input, "draft");
    }
    #[test]
    fn explicit_text_input_edits_unicode_at_boundaries_and_completes_commands() {
        let mut console = Console::default();
        console.toggle();
        console.input(&Input {
            text: "aé🦀".into(),
            ..Default::default()
        });
        console.input(&Input {
            keys: [Key::Left].into(),
            ..Default::default()
        });
        console.input(&Input {
            keys: [Key::Backspace].into(),
            ..Default::default()
        });
        assert_eq!(console.input, "a🦀");
        console.input(&Input {
            keys: [Key::Delete].into(),
            ..Default::default()
        });
        assert_eq!(console.input, "a");
        console.input.clear();
        console.cursor = 0;
        console.input(&Input {
            text: "noc".into(),
            keys: [Key::Tab].into(),
            ..Default::default()
        });
        assert_eq!(console.input, "noclip ");
        console.input(&Input {
            keys: [Key::Enter].into(),
            ..Default::default()
        });
        assert!(console.input.is_empty());
        assert!(console.output.iter().any(|s| s.contains("sv_cheats")));
    }
    /// Pointer position at the middle of menu row `index`.
    fn row(console: &Console, index: usize) -> Option<Vec2> {
        let geometry = console.menu_layout.geometry(console.canvas.height());
        Some(geometry.menu + vec2(5., geometry.item_tall * (index as f32 + 0.5)))
    }
    fn click_row(console: &mut Console, index: usize) -> Vec<Effect> {
        let pointer = row(console, index);
        let _ = console.input(&Input {
            pointer,
            click: true,
            held: true,
            ..Default::default()
        });
        console.input(&Input {
            pointer,
            released: true,
            ..Default::default()
        })
    }
    fn keys(console: &mut Console, keys: &[Key]) -> Vec<Effect> {
        console.input(&Input {
            keys: keys.iter().copied().collect(),
            ..Default::default()
        })
    }
    /// A console with the built-in menu structure, a known first chapter and a background map.
    fn installed_console() -> Console {
        let loc = Localization::default();
        let resources = GameUiResources {
            localization: loc,
            game_menu: fallback_game_menu(),
            background_map: Some("background01".into()),
            background_maps: crate::gameui::resources::fallback_background_maps(),
            new_game_maps: crate::gameui::resources::fallback_new_game_maps(),
            ..Default::default()
        };
        let mut console = Console::default();
        console.set_resources(resources);
        console
    }

    #[test]
    fn pause_menu_entries_activate_by_pointer_and_keyboard() {
        let mut console = installed_console();
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        // Resume, Load, Save, Achievements, Options, Return to main menu, Quit.
        assert_eq!(console.pause_menu.entries().len(), 7);
        click_row(&mut console, 4);
        assert_eq!(console.mode, Mode::Options);
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        assert!(
            click_row(&mut console, 1).is_empty(),
            "Load Game is a placeholder"
        );
        assert_eq!(console.mode, Mode::Pause);
        assert!(console.output.back().unwrap().contains("not implemented"));
        assert_eq!(
            click_row(&mut console, 5),
            vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
        );
        let (confirm, kind) = console
            .confirm_box
            .clone()
            .expect("return to main menu confirmation");
        assert_eq!(kind, ConfirmKind::ReturnToMainMenu);
        assert_eq!(confirm.title, "RETURN TO MAIN MENU");
        let (origin, scale, layout) = console.quit_geometry(&confirm);
        let click_btn = |console: &mut Console, button: Rect| {
            let pointer = Some(origin + vec2(button.x + 3., button.y + 3.) * scale);
            let _ = console.input(&Input {
                pointer,
                click: true,
                held: true,
                ..Default::default()
            });
            console.input(&Input {
                pointer,
                released: true,
                ..Default::default()
            })
        };
        // Cancel dismisses the dialog and remains in pause menu
        assert_eq!(
            click_btn(&mut console, layout.buttons[1]),
            vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
        );
        assert!(console.confirm_box.is_none());
        assert_eq!(console.mode, Mode::Pause);

        // The pointer left Return to main menu selected; Down moves to Quit, and Enter asks for
        // confirmation instead of quitting.
        assert_eq!(
            keys(&mut console, &[Key::Down]),
            vec![Effect::PlaySound("ui/buttonrollover.wav".into())]
        );
        assert_eq!(
            keys(&mut console, &[Key::Enter]),
            vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
        );
        assert!(console.confirm_box.is_some());
        console.escape();
        assert!(console.confirm_box.is_none(), "Escape closes only the box");
        assert_eq!(console.mode, Mode::Pause);
        keys(&mut console, &[Key::Enter]);
        assert_eq!(
            keys(&mut console, &[Key::Enter]),
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::Quit,
            ]
        );
        console.confirm_box = None;
        click_row(&mut console, 0);
        assert_eq!(console.mode, Mode::Gameplay, "Resume");
    }

    #[test]
    fn return_to_main_menu_confirmation_dialog_ok_unloads_to_background() {
        let mut console = installed_console();
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        // Click row 5: "Return to main menu"
        assert_eq!(
            click_row(&mut console, 5),
            vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
        );
        let (confirm, kind) = console
            .confirm_box
            .clone()
            .expect("return to main menu confirmation");
        assert_eq!(kind, ConfirmKind::ReturnToMainMenu);
        assert_eq!(confirm.title, "RETURN TO MAIN MENU");
        let (origin, scale, layout) = console.quit_geometry(&confirm);
        let pointer =
            Some(origin + vec2(layout.buttons[0].x + 3., layout.buttons[0].y + 3.) * scale);
        let _ = console.input(&Input {
            pointer,
            click: true,
            held: true,
            ..Default::default()
        });
        let effects = console.input(&Input {
            pointer,
            released: true,
            ..Default::default()
        });
        assert_eq!(
            effects,
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::MainMenuMap("background01".into()),
            ]
        );
        assert!(console.confirm_box.is_none());
        assert_eq!(console.mode, Mode::MainMenu);
    }

    #[test]
    fn background_level_main_menu_navigation_new_game_and_quit_box() {
        let mut console = installed_console();
        console.level_loaded(true);
        assert_eq!(console.mode, Mode::MainMenu);
        console.escape();
        assert_eq!(console.mode, Mode::MainMenu, "nothing to resume");
        click_row(&mut console, 0);
        assert_eq!(console.main_menu.category(), "hl2");
        console.escape();
        assert_eq!(console.main_menu.category(), "main");
        assert_eq!(console.mode, Mode::MainMenu);
        click_row(&mut console, 0);
        assert_eq!(
            click_row(&mut console, 0),
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::Map("d1_trainstation_01".into()),
            ]
        );
        console.level_loaded(false);
        assert_eq!(console.mode, Mode::Gameplay);
        assert_eq!(console.main_menu.category(), "main");
        console.escape();
        assert_eq!(console.mode, Mode::Pause);

        console.level_loaded(true);
        click_row(&mut console, 6);
        let (quit, kind) = console.confirm_box.clone().expect("quit confirmation");
        assert_eq!(kind, ConfirmKind::Quit);
        let (origin, scale, layout) = console.quit_geometry(&quit);
        let click_btn = |console: &mut Console, button: Rect| {
            let pointer = Some(origin + vec2(button.x + 3., button.y + 3.) * scale);
            let _ = console.input(&Input {
                pointer,
                click: true,
                held: true,
                ..Default::default()
            });
            console.input(&Input {
                pointer,
                released: true,
                ..Default::default()
            })
        };
        assert_eq!(
            click_btn(&mut console, layout.buttons[1]),
            vec![Effect::PlaySound("ui/buttonclickrelease.wav".into())]
        );
        assert!(console.confirm_box.is_none(), "Cancel closes the box");
        assert_eq!(console.mode, Mode::MainMenu);
        click_row(&mut console, 6);
        assert_eq!(
            click_btn(&mut console, layout.buttons[0]),
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::Quit,
            ]
        );
    }

    #[test]
    fn options_open_from_the_main_menu_and_console_commands_update_config() {
        let mut console = installed_console();
        console.level_loaded(true);
        click_row(&mut console, 5);
        assert_eq!(console.mode, Mode::Options);
        console.escape();
        assert_eq!(console.mode, Mode::MainMenu);
        click_row(&mut console, 5);
        console.toggle();
        console.toggle();
        assert_eq!(
            console.mode,
            Mode::Options,
            "console round trip keeps the dialog"
        );
        console.draw(0.);
        console.canvas.drain();
        console.escape();
        assert_eq!(
            console.mode,
            Mode::MainMenu,
            "Escape returns to the opening menu"
        );
        console.mode = Mode::Console;
        let effects = console.submit("volume 0.5; sensitivity 2.5; fov 90; mat_hdr_level 0");
        assert_eq!(console.config.audio.volume, 0.5);
        assert_eq!(console.config.mouse.sensitivity, 2.5);
        assert_eq!(console.config.video.fov, 90.0);
        assert_eq!(console.config.video.hdr, crate::config::HdrMode::None);
        assert_eq!(effects.len(), 4);
    }

    #[test]
    fn loading_dialog_state_transitions_and_dismissal() {
        let mut console = installed_console();
        console.level_loaded(true);
        assert!(!console.is_loading());

        // Start loading
        console.start_loading("d1_trainstation_01");
        assert!(console.is_loading());
        assert!(console.loading_dialog.is_some());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().map,
            "d1_trainstation_01"
        );

        // Update progress via global reporting
        crate::gameui::loading::report_progress(0.5, "LoadingProgress_SignonDataLocal");
        console.update_loading();
        assert_eq!(console.loading_dialog.as_ref().unwrap().fraction, 0.5);
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().stage,
            Some("LoadingProgress_SignonDataLocal")
        );

        // While actively loading, input is consumed without dismissing
        let input = Input {
            click: true,
            ..Default::default()
        };
        assert!(console.input(&input).is_empty());
        assert!(console.loading_dialog.is_some());

        // Completing loading clears the dialog
        console.loading_complete();
        assert!(!console.is_loading());
        assert!(console.loading_dialog.is_none());

        // Test failure path and dismissal via escape
        console.start_loading("invalid_map");
        console.loading_failed("Map invalid_map not found");
        assert!(!console.is_loading());
        assert!(console.loading_dialog.as_ref().unwrap().is_failed());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().error.as_deref(),
            Some("Map invalid_map not found")
        );

        console.escape();
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::MainMenu);

        // Test failure path and dismissal via Enter or click
        console.start_loading("invalid_map");
        console.loading_failed("Map invalid_map not found");
        let enter_input = Input {
            keys: [Key::Enter].into_iter().collect(),
            ..Default::default()
        };
        console.input(&enter_input);
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::MainMenu);

        // Test level_loaded also clears loading dialog
        console.start_loading("d1_trainstation_02");
        console.level_loaded(false);
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::Gameplay);
    }

    #[test]
    fn episode_one_and_two_new_game_navigation_and_missing_content_error_recovery() {
        let mut console = installed_console();
        console.level_loaded(true);
        assert_eq!(console.mode, Mode::MainMenu);

        // Row 1 on main menu: HL2: EPISODE ONE
        assert_eq!(
            click_row(&mut console, 1),
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::MainMenuMap("ep1_background01".into()),
            ]
        );
        assert_eq!(console.main_menu.category(), "episodic");
        console.level_loaded(true);

        // Row 0 in Episode One submenu: NEW GAME
        let effects = click_row(&mut console, 0);
        assert_eq!(
            effects,
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::Map("ep1_citadel_00".into()),
            ]
        );
        assert!(console.is_loading());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().map,
            "ep1_citadel_00"
        );

        // Graceful error handling for missing episode content:
        // Loader reports failure, error is latched onto loading dialog.
        console.loading_failed("owned map ep1_citadel_00.bsp not found");
        assert!(!console.is_loading());
        assert!(console.loading_dialog.as_ref().unwrap().is_failed());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().error.as_deref(),
            Some("owned map ep1_citadel_00.bsp not found")
        );

        // Player dismisses failed dialog via Escape and returns to main menu
        console.escape();
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::MainMenu);

        // Row 2 on main menu: HL2: EPISODE TWO
        assert_eq!(
            click_row(&mut console, 2),
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::MainMenuMap("ep2_background01".into()),
            ]
        );
        assert_eq!(console.main_menu.category(), "ep2");
        console.level_loaded(true);

        // Row 0 in Episode Two submenu: NEW GAME
        let effects = click_row(&mut console, 0);
        assert_eq!(
            effects,
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::Map("ep2_outland_01".into()),
            ]
        );
        assert!(console.is_loading());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().map,
            "ep2_outland_01"
        );

        // Missing episode content error is dismissed via Enter input
        console.loading_failed("owned map ep2_outland_01.bsp not found");
        let enter = Input {
            keys: [Key::Enter].into_iter().collect(),
            ..Default::default()
        };
        console.input(&enter);
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::MainMenu);
    }

    #[test]
    fn background_map_single_source_of_truth_and_return_to_main_menu() {
        let mut console = installed_console();
        // Default: falls back to GameUI resources background_map
        assert_eq!(console.background_map(), Some("background01"));
        assert_eq!(console.config.gameplay.background_map, None);

        // When overridden in config, background_map() prioritizes config
        console.set_background_map(Some("background02".into()));
        assert_eq!(console.background_map(), Some("background02"));

        // Trigger Return to Main Menu confirmation and click OK
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        click_row(&mut console, 5); // Return to main menu
        let (confirm, _) = console.confirm_box.clone().expect("confirmation box");
        let (origin, scale, layout) = console.quit_geometry(&confirm);
        let pointer =
            Some(origin + vec2(layout.buttons[0].x + 3., layout.buttons[0].y + 3.) * scale);
        let _ = console.input(&Input {
            pointer,
            click: true,
            held: true,
            ..Default::default()
        });
        let effects = console.input(&Input {
            pointer,
            released: true,
            ..Default::default()
        });
        // Verifies the overridden background map was emitted
        assert_eq!(
            effects,
            vec![
                Effect::PlaySound("ui/buttonclickrelease.wav".into()),
                Effect::MainMenuMap("background02".into()),
            ]
        );
        assert_eq!(console.mode, Mode::MainMenu);
    }

    #[test]
    fn background_map_and_map_background_console_commands() {
        let mut console = installed_console();
        // Query default background map
        let effects = console.submit("background_map");
        assert!(effects.is_empty());
        assert!(console
            .output
            .iter()
            .any(|line| line.contains("\"background_map\" = \"background01\"")));

        // Set persistent background map
        let effects = console.submit("background_map background03");
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::ApplyConfig(_)));
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("background03")
        );
        assert_eq!(console.background_map(), Some("background03"));

        // Reset to default
        let effects = console.submit("background_map default");
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::ApplyConfig(_)));
        assert_eq!(console.config.gameplay.background_map, None);
        assert_eq!(console.background_map(), Some("background01"));

        // map_background loads as background map and persists to config
        let effects = console.submit("map_background background04");
        assert_eq!(effects, vec![Effect::MainMenuMap("background04".into())]);
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("background04")
        );
        assert_eq!(console.background_map(), Some("background04"));
        assert!(console.is_loading());
        assert_eq!(console.loading_dialog.as_ref().unwrap().map, "background04");

        // Setting background map while gameplay map is loading persists without interrupting load
        console.start_loading("d1_trainstation_01");
        console.background_level = false;
        assert!(console.is_loading());
        let effects = console.submit("map_background background05");
        assert!(effects.is_empty());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().map,
            "d1_trainstation_01"
        );
        assert_eq!(console.background_map(), Some("background05"));
    }

    #[test]
    fn episode_submenus_show_two_line_titles() {
        let mut console = Console::default();
        let mut loc = Localization::default();
        loc.insert("GameUI_HalfLife2", "Half-Life 2");
        loc.insert("GameUI_EpisodeOne", "Episode One");
        loc.insert("GameUI_EpisodeTwo", "Episode Two");
        console.set_resources(GameUiResources {
            localization: loc.clone(),
            category_titles: resources::fallback_category_titles(&loc),
            ..Default::default()
        });
        console.mode = Mode::MainMenu;
        assert_eq!(console.current_title(), "Half-Life 2");

        // Menu with items to navigate between main, episodic, and ep2 categories
        console.main_menu = GameMenu::new(
            vec![
                MenuItem {
                    label: "#GameUI_HalfLife2EpisodeOne".into(),
                    command: "SwitchExpansion episodic".into(),
                    category: Some("main".into()),
                    order: 10,
                    visibility: Visibility::Always,
                    caps: true,
                },
                MenuItem {
                    label: "#GameUI_HalfLife2EpisodeTwo".into(),
                    command: "SwitchExpansion ep2".into(),
                    category: Some("main".into()),
                    order: 20,
                    visibility: Visibility::Always,
                    caps: true,
                },
                MenuItem {
                    label: "#GameUI_Back".into(),
                    command: "SwitchCategory main".into(),
                    category: Some("episodic".into()),
                    order: 30,
                    visibility: Visibility::Always,
                    caps: true,
                },
                MenuItem {
                    label: "#GameUI_Back".into(),
                    command: "SwitchCategory main".into(),
                    category: Some("ep2".into()),
                    order: 40,
                    visibility: Visibility::Always,
                    caps: true,
                },
            ],
            Default::default(),
            &loc,
        );

        // Activate Episode One entry
        console.main_menu.activate(0, &loc);
        assert_eq!(console.main_menu.category(), "episodic");
        assert_eq!(console.current_title(), "Half-Life 2\n== episode one==");
        assert_eq!(console.current_title().lines().count(), 2);

        // Activate Back button to return to main
        console.main_menu.activate(0, &loc);
        assert_eq!(console.main_menu.category(), "main");
        assert_eq!(console.current_title(), "Half-Life 2");
        assert_eq!(console.current_title().lines().count(), 1);

        // Activate Episode Two entry
        console.main_menu.activate(1, &loc);
        assert_eq!(console.main_menu.category(), "ep2");
        assert_eq!(console.current_title(), "Half-Life 2\n== episode two ==");
        assert_eq!(console.current_title().lines().count(), 2);
    }

    #[test]
    fn modal_dialog_blocks_underlying_menu_hover_sounds_and_clicks() {
        let mut console = installed_console();
        console.mode = Mode::MainMenu;
        console.canvas.resize(1280., 720.);

        // 1. When no modal dialog is open, hovering and clicking menu rows work normally
        assert!(!console.modal_dialog_open());
        assert_eq!(console.modal_bounds(), None);
        assert!(!console.is_occluded(vec2(100., 100.)));

        let hover_effects = console.input(&Input {
            pointer: row(&console, 0),
            ..Default::default()
        });
        assert!(
            hover_effects.contains(&Effect::PlaySound("ui/buttonrollover.wav".into())),
            "hovering menu row should play rollover sound when no dialog is open"
        );
        assert_eq!(console.main_menu.selection, Some(0));

        // 2. Open Options dialog: modal check is true, bounds are present, occlusion detects dialog
        console.mode = Mode::Options;
        assert!(console.modal_dialog_open());
        let opt_bounds = console.modal_bounds().expect("options bounds");
        assert!(
            opt_bounds.w > 0. && opt_bounds.h > 0.,
            "modal bounds must have positive area"
        );
        let center = vec2(
            opt_bounds.x + opt_bounds.w * 0.5,
            opt_bounds.y + opt_bounds.h * 0.5,
        );
        assert!(console.is_occluded(center));
        assert!(!console.is_occluded(vec2(0., 0.)));

        // Cursor over menu row 0 behind/beside Options does NOT play rollover sound or change menu selection
        let hover_effects = console.input(&Input {
            pointer: row(&console, 0),
            ..Default::default()
        });
        assert!(
            !hover_effects.contains(&Effect::PlaySound("ui/buttonrollover.wav".into())),
            "hover over underlying menu row must not produce rollover sound while Options is open"
        );

        // Click over menu row does not activate any menu command
        let click_effects = click_row(&mut console, 0);
        assert!(
            click_effects.is_empty(),
            "click over underlying menu row must not activate while Options is open"
        );

        // Closing options via escape preserves keyboard selection and returns to main menu
        console.escape();
        assert_eq!(console.mode, Mode::MainMenu);
        assert!(!console.modal_dialog_open());
        assert_eq!(console.main_menu.selection, Some(0));

        // 3. Open confirmation dialog: modal check is true, underlying rows inert
        let loc = console.gameui.localization.clone();
        console.confirm_box = Some((MessageBox::quit(&loc), ConfirmKind::Quit));
        assert!(console.modal_dialog_open());
        let confirm_bounds = console.modal_bounds().expect("confirm box bounds");
        assert!(console.is_occluded(vec2(
            confirm_bounds.x + confirm_bounds.w * 0.5,
            confirm_bounds.y + confirm_bounds.h * 0.5
        )));

        // Hover over menu row 0 produces no rollover sound
        let hover_effects = console.input(&Input {
            pointer: row(&console, 0),
            ..Default::default()
        });
        assert!(
            !hover_effects.contains(&Effect::PlaySound("ui/buttonrollover.wav".into())),
            "hover over underlying menu row must not produce rollover sound while ConfirmBox is open"
        );

        // Click over menu row 0 does not activate
        let click_effects = click_row(&mut console, 0);
        assert!(
            click_effects.is_empty(),
            "click over underlying menu row must not pass through while ConfirmBox is open"
        );
        assert!(console.confirm_box.is_some(), "confirm box remains open");

        // Escape dismisses confirm box and preserves menu selection
        console.escape();
        assert!(console.confirm_box.is_none());
        assert!(!console.modal_dialog_open());
        assert_eq!(console.main_menu.selection, Some(0));

        // 4. Loading dialog: modal check is true, underlying rows inert
        console.start_loading("d1_trainstation_01");
        assert!(console.modal_dialog_open());
        assert!(console.is_loading());
        let load_bounds = console.modal_bounds().expect("loading bounds");
        assert!(console.is_occluded(vec2(
            load_bounds.x + load_bounds.w * 0.5,
            load_bounds.y + load_bounds.h * 0.5
        )));

        let hover_effects = console.input(&Input {
            pointer: row(&console, 0),
            ..Default::default()
        });
        assert!(
            !hover_effects.contains(&Effect::PlaySound("ui/buttonrollover.wav".into())),
            "hover over menu row must not produce sound while LoadingDialog is open"
        );

        let click_effects = click_row(&mut console, 0);
        assert!(click_effects.is_empty());
        console.loading_complete();
        assert!(!console.modal_dialog_open());
        assert_eq!(console.main_menu.selection, None);

        // 5. Console: modal check is true
        console.toggle();
        assert_eq!(console.mode, Mode::Console);
        assert!(console.modal_dialog_open());
        let click_effects = click_row(&mut console, 0);
        assert!(click_effects.is_empty());
        console.toggle();
        assert_eq!(console.mode, Mode::MainMenu);
        assert!(!console.modal_dialog_open());
    }

    #[test]
    fn submenu_entry_switches_background_map_and_persists() {
        let mut console = installed_console();
        console.mode = Mode::MainMenu;
        console.background_level = true;
        console.loaded_map = Some("background01".into());

        // Initial background map state is background01
        assert_eq!(console.background_map(), Some("background01"));

        // 1. Enter Episode One submenu (row 1: "HL2: EPISODE ONE")
        let effects = click_row(&mut console, 1);
        assert_eq!(console.main_menu.category(), "episodic");
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("ep1_background01"),
            "single background-map state is updated to episode 1"
        );
        assert!(console.is_loading());
        assert!(console.loading_dialog.as_ref().unwrap().is_background());
        assert!(!console.modal_dialog_open());
        assert!(console.modal_bounds().is_none());
        assert!(
            effects.contains(&Effect::MainMenuMap("ep1_background01".into())),
            "dispatches Effect::MainMenuMap for ep1_background01"
        );

        // Map loads and level_loaded is called
        console.level_loaded(true);
        assert_eq!(
            console.main_menu.category(),
            "episodic",
            "category remains in episodic submenu after background map load"
        );
        assert_eq!(console.loaded_map.as_deref(), Some("ep1_background01"));
        assert!(!console.is_loading());

        // 2. Return to main category via Escape, then enter Episode Two submenu
        console.escape();
        assert_eq!(console.main_menu.category(), "main");
        // Background map remains ep1_background01 until a different expansion is selected
        assert_eq!(console.loaded_map.as_deref(), Some("ep1_background01"));

        // Click row 2: "HL2: EPISODE TWO"
        let effects = click_row(&mut console, 2);
        assert_eq!(console.main_menu.category(), "ep2");
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("ep2_background01"),
            "persisted background map updated to ep2"
        );
        assert!(console.is_loading());
        assert!(console.loading_dialog.as_ref().unwrap().is_background());
        assert!(!console.modal_dialog_open());
        assert!(effects.contains(&Effect::MainMenuMap("ep2_background01".into())));

        console.level_loaded(true);
        assert_eq!(console.main_menu.category(), "ep2");
        assert_eq!(console.loaded_map.as_deref(), Some("ep2_background01"));

        // 3. Return to main, then enter Half-Life 2 submenu (row 0)
        console.escape();
        assert_eq!(console.main_menu.category(), "main");

        let effects = click_row(&mut console, 0);
        assert_eq!(console.main_menu.category(), "hl2");
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("background01")
        );
        assert!(effects.contains(&Effect::MainMenuMap("background01".into())));

        console.level_loaded(true);
        assert_eq!(console.main_menu.category(), "hl2");
        assert_eq!(console.loaded_map.as_deref(), Some("background01"));

        // 4. Entering Half-Life 2 submenu when already on background01 does not reload
        console.escape();
        assert_eq!(console.main_menu.category(), "main");
        let effects = click_row(&mut console, 0);
        assert_eq!(console.main_menu.category(), "hl2");
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::MainMenuMap(_))),
            "no redundant reload when target background map is already active"
        );
        assert!(!console.is_loading());
    }

    #[test]
    fn submenu_back_and_escape_preserves_background_map() {
        let mut console = installed_console();
        console.mode = Mode::MainMenu;
        console.background_level = true;
        console.loaded_map = Some("background01".into());

        // Switch to Episode One
        let _ = click_row(&mut console, 1);
        console.level_loaded(true);
        assert_eq!(console.main_menu.category(), "episodic");
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("ep1_background01")
        );

        // In episodic submenu, row 5 is "BACK"
        let back_effects = click_row(&mut console, 5);
        assert_eq!(console.main_menu.category(), "main");
        assert!(
            !back_effects
                .iter()
                .any(|e| matches!(e, Effect::MainMenuMap(_))),
            "Back button does not trigger map reload"
        );
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("ep1_background01"),
            "background map persistence stays intact on Back"
        );
        assert_eq!(console.loaded_map.as_deref(), Some("ep1_background01"));
    }

    #[test]
    fn lostcoast_navigation_and_missing_content_error_recovery() {
        let mut console = installed_console();
        console.mode = Mode::MainMenu;
        console.background_level = true;
        console.loaded_map = Some("background01".into());

        // In main menu: row 4 is EXTRAS
        let effects = click_row(&mut console, 4);
        assert_eq!(console.main_menu.category(), "extras");
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::MainMenuMap(_))),
            "entering extras does not reload background map"
        );

        // In extras: row 1 is disabled Workshop placeholder
        assert!(!console.main_menu.entries()[1].enabled);
        assert_eq!(console.main_menu.entries()[1].label, "WORKSHOP");
        console.activate_menu(1);
        assert!(console
            .output
            .iter()
            .any(|line| line.contains("WORKSHOP: not implemented")));

        // In extras: row 0 is HALF-LIFE 2: LOST COAST
        let lc_effects = click_row(&mut console, 0);
        assert_eq!(console.main_menu.category(), "lostcoast");
        assert!(
            lc_effects
                .iter()
                .any(|e| matches!(e, Effect::MainMenuMap(map) if map == "lostcoast_background01")),
            "entering lostcoast submenu reloads background map to lostcoast_background01"
        );
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("lostcoast_background01")
        );
        console.level_loaded(true);

        // In lostcoast: row 0 is NEW GAME
        let new_game_effects = click_row(&mut console, 0);
        assert!(
            new_game_effects
                .iter()
                .any(|e| matches!(e, Effect::Map(map) if map == "d2_lostcoast")),
            "starts loading d2_lostcoast"
        );
        assert!(console.is_loading());
        assert_eq!(console.loading_dialog.as_ref().unwrap().map, "d2_lostcoast");

        // Simulate missing content error
        console.loading_failed("owned map d2_lostcoast.bsp not found");
        assert!(!console.is_loading());
        assert!(console.loading_dialog.as_ref().unwrap().is_failed());
        assert_eq!(
            console.loading_dialog.as_ref().unwrap().error.as_deref(),
            Some("owned map d2_lostcoast.bsp not found")
        );

        // Dismiss error with Escape -> returns to menu without crash
        console.escape();
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::MainMenu);
    }

    #[test]
    fn submenu_titles_resolve_for_expansion_aliases() {
        let mut console = Console::default();
        let mut res = GameUiResources::default();
        res.category_titles
            .insert("episodic".into(), "HALF-LIFE'\n== episode one==".into());
        res.category_titles
            .insert("ep2".into(), "HALF-LIFE'\n== episode two ==".into());
        res.category_titles
            .insert("lostcoast".into(), "HALF-LIFE'\nlost coasT".into());
        console.set_resources(res);

        assert_eq!(
            console.category_title("episodic"),
            "HALF-LIFE'\n== episode one=="
        );
        assert_eq!(
            console.category_title("hl2ep1"),
            "HALF-LIFE'\n== episode one=="
        );
        assert_eq!(
            console.category_title("ep2"),
            "HALF-LIFE'\n== episode two =="
        );
        assert_eq!(
            console.category_title("hl2ep2"),
            "HALF-LIFE'\n== episode two =="
        );
        assert_eq!(
            console.category_title("lostcoast"),
            "HALF-LIFE'\nlost coasT"
        );
        assert_eq!(console.category_title("hl2lc"), "HALF-LIFE'\nlost coasT");
    }

    #[test]
    fn background_loading_draw_emits_blur_overlay_behind_menu_and_corner_plaque() {
        let mut console = Console::default();
        console.canvas.resize(1920., 1080.);
        console.mode = Mode::MainMenu;

        console.start_background_loading("ep2_background01");
        assert!(console.is_loading());
        assert!(console.is_background_loading());
        assert!(!console.modal_dialog_open());

        console.draw(0.0);
        let quads = console.canvas.drain();

        // Must emit exactly one blur quad representing the full-canvas background blur overlay
        let blur_quads: Vec<_> = quads.iter().filter(|q| q.blur).collect();
        assert_eq!(blur_quads.len(), 1, "expected exactly 1 blur overlay quad");
        let overlay = blur_quads[0];
        assert_eq!(overlay.destination.w, 1920.);
        assert_eq!(overlay.destination.h, 1080.);
        assert_eq!(overlay.color, Color::new(0.0, 0.0, 0.0, 0.45));

        // The blur overlay must be drawn BEFORE the menu items so text is sharp over the blur
        let blur_index = quads.iter().position(|q| q.blur).unwrap();
        assert_eq!(
            blur_index, 0,
            "blur overlay must be the first quad drawn behind the menu"
        );

        let scale = Painter::scale_for(1080.);
        let expected_plaque_w = (crate::gameui::loading::BACKGROUND_LOADING_SIZE.x * scale).round();
        let expected_plaque_h = (crate::gameui::loading::BACKGROUND_LOADING_SIZE.y * scale).round();
        let plaque_quad = quads
            .iter()
            .find(|q| {
                (q.destination.w - expected_plaque_w).abs() < 0.1
                    && (q.destination.h - expected_plaque_h).abs() < 0.1
            })
            .expect("bottom-right corner plaque quad");
        assert!(!plaque_quad.blur);
    }

    #[test]
    fn gameplay_loading_draw_does_not_emit_blur_overlay() {
        let mut console = Console::default();
        console.canvas.resize(1920., 1080.);
        console.mode = Mode::MainMenu;

        console.start_loading("d1_trainstation_01");
        assert!(console.is_loading());
        assert!(!console.is_background_loading());
        assert!(console.modal_dialog_open());

        console.draw(0.0);
        let quads = console.canvas.drain();

        // Gameplay map loading must NEVER emit a blur quad
        let blur_quads: Vec<_> = quads.iter().filter(|q| q.blur).collect();
        assert!(
            blur_quads.is_empty(),
            "gameplay map loading must not emit any blur overlay quads"
        );

        // Frame and progress bar quads must be present
        let scale = Painter::scale_for(1080.);
        let expected_frame_w = (crate::gameui::loading::LOADING_DIALOG_SIZE.x * scale).round();
        let frame_quad = quads
            .iter()
            .find(|q| (q.destination.w - expected_frame_w).abs() < 0.1)
            .expect("centered gameplay loading dialog frame");
        assert!(!frame_quad.blur);
    }

    #[test]
    fn background_loading_completion_dismisses_blur_overlay() {
        let mut console = Console::default();
        console.canvas.resize(1920., 1080.);
        console.mode = Mode::MainMenu;

        console.start_background_loading("ep2_background01");
        assert!(console.is_background_loading());

        // Level loads
        console.level_loaded(true);
        assert!(!console.is_loading());
        assert!(!console.is_background_loading());

        console.draw(0.0);
        let quads = console.canvas.drain();
        let blur_quads: Vec<_> = quads.iter().filter(|q| q.blur).collect();
        assert!(
            blur_quads.is_empty(),
            "no blur overlay quads once background map loading is complete"
        );
    }

    #[test]
    fn background_loading_state_sequence_success() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.canvas.resize(1920., 1080.);

        assert_eq!(
            console.background_transition_state(),
            &crate::gameui::loading::BackgroundTransitionState::Idle
        );

        console.start_background_loading("ep2_background01");
        assert!(console.is_background_loading());
        assert!(matches!(
            console.background_transition_state(),
            crate::gameui::loading::BackgroundTransitionState::Starting { .. }
        ));
        assert_eq!(
            console.background_transition_state().target_map(),
            Some("ep2_background01")
        );

        console.update_loading();
        assert!(matches!(
            console.background_transition_state(),
            crate::gameui::loading::BackgroundTransitionState::Loading { .. }
        ));

        console.level_loaded(true);
        assert!(!console.is_background_loading());
        assert!(matches!(
            console.background_transition_state(),
            crate::gameui::loading::BackgroundTransitionState::Finished { .. }
        ));
        assert!(console.background_transition_state().is_terminal());

        console.draw(0.0);
        let quads = console.canvas.drain();
        assert!(quads.iter().all(|q| !q.blur));
    }

    #[test]
    fn background_load_failed_ends_transition_and_cleans_up() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.canvas.resize(1920., 1080.);

        console.start_background_loading("lostcoast_background01");
        assert!(console.is_background_loading());

        console.background_load_failed("Failed to open BSP file");
        assert!(!console.is_background_loading());
        assert_eq!(
            console.background_transition_state(),
            &crate::gameui::loading::BackgroundTransitionState::Failed {
                map: "lostcoast_background01".into(),
                reason: "Failed to open BSP file".into(),
            }
        );
        assert!(console.background_transition_state().is_terminal());
        assert!(console.background_transition_state().is_failed());

        console.draw(0.0);
        let quads = console.canvas.drain();
        assert!(
            quads.iter().all(|q| !q.blur),
            "no blur overlay quads after background load failure"
        );
    }

    #[test]
    fn generic_loading_failed_on_background_cleans_up() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.canvas.resize(1920., 1080.);

        console.start_background_loading("ep1_background01");
        assert!(console.is_background_loading());

        // Generic loading_failed on a background map must dismiss the dialog and mark transition failed
        console.loading_failed("Corrupt lump header");
        assert!(!console.is_background_loading());
        assert!(console.background_transition_state().is_failed());
        assert!(console.loading_dialog.is_none());

        console.draw(0.0);
        let quads = console.canvas.drain();
        assert!(quads.iter().all(|q| !q.blur));
    }

    #[test]
    fn menu_buttons_draw_letters_with_border_along_edge_path_and_no_box_fill() {
        let mut console = Console::default();
        console.canvas.resize(1280., 720.);
        console.mode = Mode::MainMenu;
        console.title = String::new();
        let font_bytes = include_bytes!("../assets/fonts/ProggyClean.ttf");
        console.menu_font = FontFace::load(font_bytes, console.canvas.clone()).ok();
        assert!(console.menu_font.is_some());

        // 1. Critical requirement: Verify that NO rectangular background boxes or fills are drawn
        // around the button rows (only textured glyph quads).
        console.draw(0.0);
        let quads = console.canvas.drain();

        let untextured_quads: Vec<_> = quads.iter().filter(|q| q.texture.is_none()).collect();
        assert!(
            untextured_quads.is_empty(),
            "menu button clickable area must not have rectangular box or fill quads: found {:?}",
            untextured_quads.len()
        );

        // 2. Verify outline quads are emitted along the letter paths with semi-transparent black
        let default_border = Color::new(0.0, 0.0, 0.0, 0.45);
        let outline_quads: Vec<_> = quads.iter().filter(|q| q.color == default_border).collect();
        assert!(
            !outline_quads.is_empty(),
            "menu buttons must emit outline quads along letter paths with default border color"
        );
        assert!(
            outline_quads.iter().all(|q| q.texture.is_some()),
            "outline quads must be textured glyphs tracing the letter contour"
        );

        // For each character drawn, there must be 8 outline quads (offsets) per 1 foreground quad
        let fg_quads: Vec<_> = quads
            .iter()
            .filter(|q| q.color == console.style.menu)
            .collect();
        assert!(
            !fg_quads.is_empty(),
            "menu buttons must emit foreground glyph quads"
        );
        assert_eq!(
            outline_quads.len(),
            fg_quads.len() * 8,
            "each letter must emit exactly 8 outline glyph quads (1px in 8 directions) for its contour"
        );

        // 3. Armed state (hovered/selected):
        // When selection is set, the selected item's letters use armed border color (alpha 0.65)
        // and armed foreground color.
        console.main_menu.selection = Some(0);
        console.draw(0.0);
        let quads = console.canvas.drain();

        let armed_border = Color::new(0.0, 0.0, 0.0, 0.65);
        let armed_outline_quads: Vec<_> =
            quads.iter().filter(|q| q.color == armed_border).collect();
        let armed_fg_quads: Vec<_> = quads
            .iter()
            .filter(|q| q.color == console.style.menu_armed)
            .collect();
        assert!(
            !armed_outline_quads.is_empty(),
            "armed menu button must use armed outline color (alpha 0.65)"
        );
        assert_eq!(
            armed_outline_quads.len(),
            armed_fg_quads.len() * 8,
            "armed item must have 8 outline quads per foreground letter quad"
        );

        // 4. Pressed state offset shift:
        // When pressed, all letter quads (outline and foreground) shift by (1.0, 1.0)
        let unpressed_first_fg_pos = armed_fg_quads[0].destination;
        console.menu_tracker.update(Some(0), true, true, false); // press button 0
        console.draw(0.0);
        let pressed_quads = console.canvas.drain();
        let pressed_fg_quads: Vec<_> = pressed_quads
            .iter()
            .filter(|q| q.color == console.style.menu_armed)
            .collect();
        assert_eq!(
            pressed_fg_quads[0].destination.x,
            unpressed_first_fg_pos.x + 1.0
        );
        assert_eq!(
            pressed_fg_quads[0].destination.y,
            unpressed_first_fg_pos.y + 1.0
        );
    }

    #[test]
    fn menu_titles_and_subtitles_draw_letters_with_border_along_edge_path() {
        let mut console = Console::default();
        console.canvas.resize(1280., 720.);
        console.mode = Mode::MainMenu;

        let font_bytes = include_bytes!("../assets/fonts/ProggyClean.ttf");
        console.menu_font = FontFace::load(font_bytes, console.canvas.clone()).ok();
        console.menu_metrics = CellMetrics::from_bytes(font_bytes).unwrap_or_default();
        assert!(console.menu_font.is_some());

        let mut loc = Localization::default();
        loc.insert("GameUI_HalfLife2", "Half-Life 2");
        loc.insert("GameUI_EpisodeOne", "Episode One");
        loc.insert("GameUI_EpisodeTwo", "Episode Two");
        loc.insert("GameUI_LostCoast", "Lost Coast");
        console.set_resources(GameUiResources {
            localization: loc.clone(),
            category_titles: resources::fallback_category_titles(&loc),
            ..Default::default()
        });

        let default_border = Color::new(0.0, 0.0, 0.0, 0.45);
        let geometry = console.menu_layout.geometry(console.canvas.height());

        for title_font_enabled in [false, true] {
            if title_font_enabled {
                console.title_metrics = CellMetrics::from_bytes(font_bytes).unwrap_or_default();
                console.title_font = FontFace::load(font_bytes, console.canvas.clone()).ok();
                console.title_face = console.title_font.as_ref().map(FontFace::face);
            } else {
                console.title_font = None;
                console.title_face = None;
            }

            // Test all expansion categories: hl2, hl2ep1, hl2ep2, hl2lc
            for (category, has_subtitle) in [
                ("hl2", false),
                ("hl2ep1", true),
                ("hl2ep2", true),
                ("hl2lc", true),
            ] {
                console.main_menu = GameMenu::new(
                    vec![
                        MenuItem {
                            label: category.into(),
                            command: format!("SwitchExpansion {category}"),
                            category: Some("main".into()),
                            order: 10,
                            visibility: Visibility::Always,
                            caps: true,
                        },
                        MenuItem {
                            label: "Back".into(),
                            command: "SwitchCategory main".into(),
                            category: Some(category.into()),
                            order: 20,
                            visibility: Visibility::Always,
                            caps: true,
                        },
                    ],
                    Default::default(),
                    &loc,
                );
                console.main_menu.activate(0, &loc);
                assert_eq!(console.main_menu.category(), category);
                console.draw(0.0);
                let quads = console.canvas.drain();

                let title_quads: Vec<_> = quads
                    .iter()
                    .filter(|q| q.destination.y < geometry.menu.y)
                    .collect();

                // 1. No untextured background box or fill quads in title/subtitle area
                let untextured: Vec<_> =
                    title_quads.iter().filter(|q| q.texture.is_none()).collect();
                assert!(
                    untextured.is_empty(),
                    "title/subtitle area for category {category} (title_font={title_font_enabled}) must not have box/fill quads: found {:?}",
                    untextured.len()
                );

                // 2. Outline quads emitted along letter paths with default border color
                let outline_quads: Vec<_> = title_quads
                    .iter()
                    .filter(|q| q.color == default_border)
                    .collect();
                assert!(
                    !outline_quads.is_empty(),
                    "title/subtitle for category {category} (title_font={title_font_enabled}) must emit outline quads"
                );
                assert!(
                    outline_quads.iter().all(|q| q.texture.is_some()),
                    "outline quads must be textured glyphs tracing the letter contour"
                );

                // 3. Foreground quads: line 0 is WHITE, subtitle (if present) is rgba(255, 255, 255, 200)
                let fg_line0: Vec<_> = title_quads.iter().filter(|q| q.color == WHITE).collect();
                assert!(
                    !fg_line0.is_empty(),
                    "title line 0 must emit white foreground glyph quads for category {category}"
                );

                let fg_sub: Vec<_> = title_quads
                    .iter()
                    .filter(|q| q.color == Color::from_rgba(255, 255, 255, 200))
                    .collect();
                if has_subtitle {
                    assert!(
                        !fg_sub.is_empty(),
                        "subtitle line must emit subtitle foreground glyph quads for category {category}"
                    );
                } else {
                    assert!(
                        fg_sub.is_empty(),
                        "category {category} should not emit subtitle foreground quads"
                    );
                }

                let total_fg = fg_line0.len() + fg_sub.len();
                assert_eq!(
                    outline_quads.len(),
                    total_fg * 8,
                    "each title/subtitle letter in category {category} (title_font={title_font_enabled}) must emit 8 outline quads"
                );

                // 4. In title_font mode, verify two-pass layering for line 0:
                // All line 0 outline quads must precede all line 0 foreground quads.
                if title_font_enabled {
                    let first_fg_line0_pos = title_quads
                        .iter()
                        .position(|q| q.color == WHITE)
                        .expect("foreground quad for line 0");
                    let outline_count_before_first_fg = title_quads[..first_fg_line0_pos]
                        .iter()
                        .filter(|q| q.color == default_border)
                        .count();
                    assert_eq!(
                        outline_count_before_first_fg,
                        fg_line0.len() * 8,
                        "all line 0 outline quads must be drawn before any line 0 foreground quads"
                    );
                }
            }
        }

        // 5. Verify Pause menu also renders title outline
        console.mode = Mode::Pause;
        console.pause_menu.set_in_game(true, &loc);
        console.draw(0.0);
        let pause_quads = console.canvas.drain();
        let pause_title_quads: Vec<_> = pause_quads
            .iter()
            .filter(|q| q.destination.y < geometry.menu.y)
            .collect();
        let pause_outline: Vec<_> = pause_title_quads
            .iter()
            .filter(|q| q.color == default_border)
            .collect();
        let pause_fg: Vec<_> = pause_title_quads
            .iter()
            .filter(|q| q.color == WHITE)
            .collect();
        assert!(
            !pause_outline.is_empty(),
            "pause menu must emit title outline quads"
        );
        assert_eq!(
            pause_outline.len(),
            pause_fg.len() * 8,
            "pause menu title letters must each have 8 outline quads"
        );
    }

    #[test]
    fn test_subtitle_font_selection_and_glyph_decoration() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.canvas.resize(1280., 720.);

        let font_bytes = include_bytes!("../assets/fonts/ProggyClean.ttf");
        console.menu_font = FontFace::load(font_bytes, console.canvas.clone()).ok();
        console.menu_metrics = CellMetrics::from_bytes(font_bytes).unwrap_or_default();
        console.title_font = FontFace::load(font_bytes, console.canvas.clone()).ok();
        console.title_face = console.title_font.as_ref().map(FontFace::face);
        console.title_metrics = CellMetrics::from_bytes(font_bytes).unwrap_or_default();

        // 1. Verify title_font_can_render correctly detects glyph coverage
        assert!(console.title_font_can_render("== episode one=="));
        assert!(console.title_font_can_render("== episode two =="));
        assert!(!console.title_font_can_render(""));
        // Character not in standard ASCII font
        assert!(!console.title_font_can_render("λ episode"));

        // 2. Set up category titles
        let mut loc = Localization::default();
        loc.insert("GameUI_HalfLife2", "Half-Life 2");
        let titles = resources::fallback_category_titles(&loc);
        console.set_resources(GameUiResources {
            localization: loc.clone(),
            category_titles: titles,
            ..Default::default()
        });

        // 3. Test episode subtitle drawing with title_font
        console.main_menu = GameMenu::new(
            vec![
                MenuItem {
                    label: "Episode One".into(),
                    command: "SwitchExpansion episodic".into(),
                    category: Some("main".into()),
                    order: 10,
                    visibility: Visibility::Always,
                    caps: true,
                },
                MenuItem {
                    label: "Back".into(),
                    command: "SwitchCategory main".into(),
                    category: Some("episodic".into()),
                    order: 20,
                    visibility: Visibility::Always,
                    caps: true,
                },
            ],
            Default::default(),
            &loc,
        );
        console.main_menu.activate(0, &loc);
        assert_eq!(console.main_menu.category(), "episodic");
        assert_eq!(console.current_title(), "Half-Life 2\n== episode one==");

        console.draw(0.0);
        let quads = console.canvas.drain();
        let geometry = console.menu_layout.geometry(console.canvas.height());
        let subtitle_quads: Vec<_> = quads
            .iter()
            .filter(|q| {
                q.destination.y >= geometry.title2.y - 5. && q.destination.y < geometry.menu.y
            })
            .collect();
        assert!(
            !subtitle_quads.is_empty(),
            "subtitle must emit quads for '== episode one=='"
        );
        let fg_subtitle: Vec<_> = subtitle_quads
            .iter()
            .filter(|q| q.color == Color::from_rgba(255, 255, 255, 200))
            .collect();
        assert!(
            !fg_subtitle.is_empty(),
            "subtitle must have foreground quads with rgba(255, 255, 255, 200)"
        );

        // 4. Test lostcoast subtitle drawing with dedicated lostcoast_title_font
        console.lostcoast_title_font = FontFace::load(font_bytes, console.canvas.clone()).ok();
        console.lostcoast_title_face = console.lostcoast_title_font.as_ref().map(FontFace::face);
        console.lostcoast_title_metrics = CellMetrics::from_bytes(font_bytes).unwrap_or_default();

        console.main_menu = GameMenu::new(
            vec![
                MenuItem {
                    label: "Lost Coast".into(),
                    command: "SwitchExpansion lostcoast".into(),
                    category: Some("main".into()),
                    order: 10,
                    visibility: Visibility::Always,
                    caps: true,
                },
                MenuItem {
                    label: "Back".into(),
                    command: "SwitchCategory main".into(),
                    category: Some("lostcoast".into()),
                    order: 20,
                    visibility: Visibility::Always,
                    caps: true,
                },
            ],
            Default::default(),
            &loc,
        );
        console.main_menu.activate(0, &loc);
        assert_eq!(console.main_menu.category(), "lostcoast");
        assert_eq!(console.current_title(), "Half-Life 2\nlost coasT");

        console.draw(0.0);
        let lc_quads = console.canvas.drain();
        let lc_subtitle_quads: Vec<_> = lc_quads
            .iter()
            .filter(|q| {
                q.destination.y >= geometry.title2.y - 5. && q.destination.y < geometry.menu.y
            })
            .collect();
        assert!(
            !lc_subtitle_quads.is_empty(),
            "lost coast subtitle must emit quads for 'lost coasT'"
        );
    }
}
