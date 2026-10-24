//! A bounded standalone console. Commands without a Rust implementation report an error.
pub mod commands;
pub mod complete;
pub mod dialog;
mod focus;
mod loading;
mod menu_flow;
mod options_dialog;
mod panel;

use crate::gameui::{
    display::DisplayModes,
    menu::{GameMenu, MenuLayout, MenuOutcome},
    options::{OptionsDialog, OptionsOutcome},
    resources::{self, fallback_game_menu, GameUiResources},
    video_confirm::{VideoConfirm, VideoMode},
};
use dialog::{wrap, DialogLayout, Selection};
use glam::{vec2, Vec2};
use serde::Serialize;
use source_assets::{keyvalues, vpk::Vfs};
use source_ui::{
    canvas::*,
    console::editor::LineEditor,
    font::{FontFace, TextOutline},
    input::{Input, Key, KeyRepeat},
    message_box::{MessageBox, MessageBoxLayout},
    placement::{self, CursorHint, Placement},
    widgets::{
        scheme_color, Align, ButtonState, ButtonTracker, Fonts, FrameSpec, Painter,
        ScrollAccumulator, ScrollBarLayout, ScrollBarState, ScrollMode, Theme, SCROLLBAR_WIDTH,
    },
    windows::{Layer, Pointer, Window, Windows},
};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::Range;
use std::sync::{Arc, Mutex};

/// Output lines kept for scrollback.
const OUTPUT_LINES: usize = 1024;

/// How a console line was printed, which picks its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// The echo of a submitted line (`] command`).
    Input,
    /// Ordinary output (`Msg`): scheme `Console.TextColor`.
    Normal,
    /// Developer output (`DevMsg`, mirrored engine log): scheme `Console.DevTextColor`.
    Developer,
    /// Replaceable snapshot emitted by the live viewmodel tuning utility.
    ViewmodelTuning,
    Warning,
    Error,
}

/// A console output line. It reads and serializes as its text.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub level: Level,
    pub text: String,
    /// Seconds since start when printed (`con_timestamp`).
    pub time: f64,
}

impl std::ops::Deref for Line {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl PartialEq<str> for Line {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl Serialize for Line {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text)
    }
}

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
    /// ai_enable: None toggles.
    AiEnable(Option<bool>),
    /// ai_disable (CC_AI_Disable): toggle the AI, Source's messages.
    AiDisable,
    /// npc_create / npc_create_aimed / npc_create_equipment / prop_physics_create /
    /// ent_create, the whole line.
    NpcCommand(String),
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
    /// Load a game's main-menu background map and show the main menu over it.
    MainMenuMap(GameMap),
    /// Play a UI sound cue (e.g. button hover or release activation).
    PlaySound(String),
}

/// A map of one installed game, named by the game's installation folder (`hl2`, `episodic`,
/// `ep2`, `lostcoast`): Half-Life 2 and Lost Coast both ship a `background01`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameMap {
    pub game: String,
    pub map: String,
}

impl GameMap {
    pub fn new(game: impl Into<String>, map: impl Into<String>) -> Self {
        Self {
            game: game.into(),
            map: map.into(),
        }
    }
}

/// Action to perform upon confirming a message box dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmKind {
    Quit,
    ReturnToMainMenu,
    /// Keep an aspect ratio change, else revert it (counts down on [`Console::tick`]).
    KeepVideo(VideoConfirm),
}

pub struct Console {
    pub canvas: Canvas,
    /// The focused layer: a window ([`Mode::Options`], [`Mode::Console`]) or the base. Read it
    /// with [`Console::mode`], change it with [`Console::set_mode`].
    mode: Mode,
    /// What the windows are over: the main menu, the pause menu or the game.
    base: Mode,
    /// The Options dialog and the console: non-modal windows over the base (retail VGUI
    /// frames), in z-order with the keyboard focus (`source_ui::windows`).
    windows: Windows,
    pub cheats: bool,
    pub output: VecDeque<Line>,
    /// Line number (since start) of `output[0]`: lines dropped from the front count on.
    output_start: u64,
    /// Output rows selected by dragging over the history, copied by the copy shortcut.
    selection: Option<Selection>,
    /// A drag that started in the history is extending the selection.
    selecting: bool,
    /// Where the console dialog was dragged or resized (kept while the game runs).
    console_placement: Placement,
    /// Where the quit / main-menu box was dragged (centered on opening).
    quit_placement: Placement,
    /// The pointer of the latest input frame (cursor hints).
    pointer: Option<Vec2>,
    /// Text the copy shortcut put on the clipboard, until the host takes it.
    copied: Option<String>,
    /// The dialog's close (×) and Submit buttons.
    console_buttons: ButtonTracker<ConsoleButton>,
    console_hover: Option<ConsoleButton>,
    /// Escape is held and closes the top dialog on release ([`Console::escape_pressed`]).
    escape_held: bool,
    history_bar: ScrollBarState,
    history_wheel: ScrollAccumulator,
    key_repeat: KeyRepeat,
    /// The input line: editing, completion, history and reverse search.
    editor: LineEditor,
    /// Names arguments complete from (installed maps, the loaded map's entities), set by the host.
    completion: complete::Sources,
    /// Seconds since start of the latest input frame, stamped on printed lines.
    clock: f64,
    /// `con_filter_enable`: 0 off, 1 only lines containing `con_filter_text`, 2 those brighter.
    con_filter: u8,
    con_filter_text: String,
    /// `con_timestamp`: prefix lines with the time they were printed.
    con_timestamp: bool,
    /// History rows scrolled back from the newest.
    scroll: usize,
    /// Each output line's wrapped rows by line number, rebuilt when its shown text or the
    /// history width changes.
    wrapped: Mutex<HashMap<u64, Wrapped>>,
    /// ASCII advances of the console font at one size (rewrapping on resize measures every
    /// character of the history).
    ascii_advances: Mutex<Option<(u32, [f32; 128])>>,
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
    pub(crate) closing_confirm_box: Option<(MessageBox, ConfirmKind)>,
    /// UI clock in seconds (real time), set every frame by [`Console::tick`].
    ui_clock: f64,
    /// Effects of calls that return none ([`Console::escape`]), handed out by the next
    /// [`Console::tick`] or [`Console::input`].
    deferred: Vec<Effect>,
    pub loading_dialog: Option<crate::gameui::loading::LoadingDialog>,
    /// The progress the map loader reports for [`Console::loading_dialog`].
    loading_progress: crate::gameui::loading::ProgressReport,
    pub background_transition: crate::gameui::loading::BackgroundTransitionState,
    /// The loaded map is a menu background: no HUD, and the main menu cannot be dismissed.
    background_level: bool,
    /// Currently loaded map, if any.
    pub loaded_map: Option<GameMap>,
    theme: Theme,
    widget_fonts: Fonts,
    style: Style,
    pub menu_tracker: ButtonTracker<usize>,
    last_menu_hover: Option<usize>,
    pub options: OptionsDialog,

    /// `(game, image)` -> pre-blurred background (see `GameUiResources::background_images`).
    pub background_textures: BTreeMap<(String, String), Texture2D>,
    pub config: crate::config::Config,
    /// Map -> chapter of every game (menu backgrounds after leaving a map).
    chapter_index: crate::gameui::chapters::ChapterIndex,
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

/// The console's completion popup: its rows (colored runs), the highlighted row, how many help
/// rows lead, and its rectangle in canvas pixels.
struct Completion {
    rows: Vec<Vec<(String, Color)>>,
    highlighted: Option<usize>,
    help_rows: usize,
    rect: Rect,
}

/// The console dialog's push buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsoleButton {
    Close,
    Submit,
}

struct Style {
    menu: Color,
    menu_armed: Color,
    panel: Color,
    entry: Color,
    /// `Console.TextColor`.
    text: Color,
    /// `Console.DevTextColor`.
    dev_text: Color,
    /// Selected history rows (`RichText.SelectedBgColor` / `RichText.SelectedTextColor`).
    selected_bg: Color,
    selected_text: Color,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            menu: WHITE,
            menu_armed: Color::from_rgba(200, 200, 200, 255),
            panel: Color::from_rgba(132, 132, 132, 212),
            entry: Color::from_rgba(0, 0, 0, 128),
            text: Color::from_rgba(221, 221, 221, 255),
            dev_text: WHITE,
            selected_bg: Color::from_rgba(255, 155, 0, 255),
            selected_text: Color::from_rgba(0, 0, 0, 255),
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
            base: Mode::Gameplay,
            // `Frame.FocusTransitionEffectTime` of the installed desktop scheme.
            windows: Windows::new(0.3, crate::tuning::unfocused_alpha_factor()),
            cheats: false,
            output: VecDeque::new(),
            output_start: 0,
            selection: None,
            selecting: false,
            console_placement: Placement::default(),
            quit_placement: Placement::default(),
            pointer: None,
            copied: None,
            console_buttons: ButtonTracker::capturing(),
            console_hover: None,
            escape_held: false,
            history_bar: ScrollBarState::default(),
            history_wheel: ScrollAccumulator::default(),
            key_repeat: KeyRepeat::default(),
            editor: LineEditor::default(),
            completion: complete::Sources::default(),
            clock: 0.,
            con_filter: 0,
            con_filter_text: String::new(),
            con_timestamp: false,
            scroll: 0,
            wrapped: Mutex::default(),
            ascii_advances: Mutex::default(),
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
            closing_confirm_box: None,
            ui_clock: 0.,
            deferred: Vec::new(),
            loading_dialog: None,
            loading_progress: Default::default(),
            background_transition: crate::gameui::loading::BackgroundTransitionState::Idle,
            background_level: false,
            loaded_map: None,
            theme: Theme::default(),
            widget_fonts: Fonts::default(),
            style: Style::default(),
            menu_tracker: ButtonTracker::new(),
            last_menu_hover: None,
            options: OptionsDialog::default(),
            background_textures: BTreeMap::new(),
            config,
            chapter_index: Default::default(),
        };
        console.set_resources(gameui);
        console.log("HL2-RS developer console. Type help for implemented commands.");
        console.log("Other Source commands are not implemented in this runtime.");
        console
    }
}

/// Height of the F1 developer overlay's header, and of the F5 viewmodel overlay panel.
const DEBUG_HEADER_HEIGHT: f32 = 105.;
const VIEWMODEL_DEBUG_HEIGHT: f32 = 130.;

impl Console {
    pub fn load(vfs: &Vfs, canvas: Canvas) -> Self {
        let font = |path: &str| {
            vfs.read(path)
                .ok()
                .flatten()
                .and_then(|bytes| FontFace::load(&bytes, canvas.clone()).ok())
        };
        let developer_font = FontFace::load(source_ui::PROGGY_CLEAN, canvas.clone()).ok();
        // Menu items draw the source scheme's MenuLarge face (GorDIN 600 in the installed
        // scheme) from the font files the scheme registers.
        let reader = crate::gameui::resources::Reader::new(vfs);
        let menu_bytes = reader
            .read("resource/SourceScheme.res")
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
            .and_then(|root| {
                source_ui::scheme_fonts::resolve(&root, "MenuLarge", |path| reader.read(path))
            })
            .or_else(|| {
                vfs.read("resource/linux_fonts/liberationsans-regular.ttf")
                    .ok()
                    .flatten()
            })
            .or_else(|| vfs.read("resource/gordin-regular.ttf").ok().flatten());
        let menu_font = menu_bytes
            .as_deref()
            .and_then(|bytes| FontFace::load(bytes, canvas.clone()).ok())
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
        // Saved dialog placements (checked against the canvas when each dialog opens).
        console.console_placement = Placement::from_saved(console.config.dialogs.console);
        console
            .options
            .set_placement(Placement::from_saved(console.config.dialogs.options));
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
        let scheme_reader = crate::gameui::resources::Reader::new(vfs);
        if let Some(root) = scheme_reader
            .read("resource/SourceSchemeBase.res")
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
        {
            console.theme = Theme::from_scheme(&root);
            if let Some(time) = root
                .get("BaseSettings")
                .and_then(|e| e.get("Frame.FocusTransitionEffectTime"))
                .and_then(|e| e.text())
                .and_then(|t| t.trim().parse::<f32>().ok())
            {
                console.windows.fade_time = time;
            }
            console.style = Style {
                menu: scheme_color(&root, "MainMenu.TextColor").unwrap_or(console.style.menu),
                menu_armed: scheme_color(&root, "MainMenu.ArmedTextColor")
                    .unwrap_or(console.style.menu_armed),
                panel: scheme_color(&root, "Frame.BgColor").unwrap_or(console.style.panel),
                entry: scheme_color(&root, "RichText.BgColor").unwrap_or(console.style.entry),
                text: scheme_color(&root, "Console.TextColor").unwrap_or(console.style.text),
                dev_text: scheme_color(&root, "Console.DevTextColor")
                    .unwrap_or(console.style.dev_text),
                ..console.style
            };
            console.menu_layout.apply_source_scheme(&root);
        }
        // The console and history colors from the installed source scheme (the base scheme it
        // reads above is empty in the installed game).
        if let Some(root) = scheme_reader
            .read("resource/SourceScheme.res")
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
        {
            for (name, color) in [
                ("Console.TextColor", &mut console.style.text),
                ("Console.DevTextColor", &mut console.style.dev_text),
                ("RichText.BgColor", &mut console.style.entry),
                ("RichText.SelectedBgColor", &mut console.style.selected_bg),
                (
                    "RichText.SelectedTextColor",
                    &mut console.style.selected_text,
                ),
            ] {
                if let Some(value) = scheme_color(&root, name) {
                    *color = value;
                }
            }
        }
        if let Some(root) = scheme_reader
            .read("resource/ClientScheme.res")
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
        self.background_textures.clear();
        for (key, img) in &resources.background_images {
            self.background_textures.insert(
                key.clone(),
                Texture2D::from_rgba8(img.width, img.height, &img.rgba),
            );
        }
        self.gameui = Arc::new(resources);
        self.options.set_resources(self.gameui.clone());
    }

    pub fn resources(&self) -> &GameUiResources {
        &self.gameui
    }

    /// A modal dialog is up: the quit / return confirmation box or a game loading dialog. It
    /// takes every click; the Options dialog and the console are not modal (see [`Windows`]).
    /// The canvas size in pixels.
    fn canvas_size(&self) -> Vec2 {
        vec2(self.canvas.width(), self.canvas.height())
    }
    /// Called before gameplay input collection. Opening characters are drained by the host.
    /// Advances the UI clock to `now` (real seconds), once per frame whatever drives the input
    /// (scripted replays too): a keep-video confirmation counts down and reverts at its timeout.
    pub fn tick(&mut self, now: f64) -> Vec<Effect> {
        self.ui_clock = now;
        self.windows.set_time(now);
        if let Some((confirm, ConfirmKind::KeepVideo(keep))) = &mut self.confirm_box {
            if keep.expired(now) {
                let previous = keep.previous;
                self.closing_confirm_box = self.confirm_box.take();
                self.reset_menu_interaction();
                let effect = self.revert_video(previous);
                self.deferred.push(effect);
            } else {
                confirm.set_text(VideoConfirm::text(
                    &self.gameui.localization,
                    keep.remaining(now),
                ));
            }
        }
        self.sync_dialog_windows();
        std::mem::take(&mut self.deferred)
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
    /// Four compact header rows and a footer, preserving the retained host's developer layout.
    /// Font data is bundled under its own MIT license, independent of installed game content.
    /// Returns the height the overlay occupies at the top of the canvas.
    pub fn draw_debug(&self, lines: &[String]) -> f32 {
        self.canvas.rectangle(
            0.,
            0.,
            self.canvas.width(),
            DEBUG_HEADER_HEIGHT,
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
        DEBUG_HEADER_HEIGHT
    }

    /// Draws the live viewmodel tuning values with their top edge at `top`; returns the bottom
    /// edge.
    pub fn draw_viewmodel_debug(&self, lines: &[String], top: f32) -> f32 {
        self.canvas.rectangle(
            0.,
            top,
            self.canvas.width(),
            VIEWMODEL_DEBUG_HEIGHT,
            Color::new(0.025, 0.045, 0.07, 0.9),
        );
        let draw = |line: &str, origin: Vec2, color| {
            if let Some(font) = &self.developer_font {
                font.draw_baseline(line, origin, 16., 1., color);
            } else {
                text(
                    &self.canvas,
                    line,
                    origin,
                    16.,
                    1.,
                    self.mono.as_ref().or(self.font.as_ref()),
                    color,
                );
            }
        };
        let normal = Color::new(0.784, 0.784, 0.784, 1.);
        for (i, line) in lines.iter().take(5).enumerate() {
            let y = top + 20. + i as f32 * 22.;
            let color = if i == 4 {
                Color::from_rgba(240, 180, 90, 255)
            } else if i == 0 {
                WHITE
            } else {
                normal
            };
            draw(line, vec2(22., y), color);
        }
        top + VIEWMODEL_DEBUG_HEIGHT
    }

    pub fn draw(&self, time: f64) {
        if self.mode == Mode::Gameplay && self.loading_dialog.is_none() {
            return;
        }
        if self.mode == Mode::Gameplay {
            if let Some(loading) = &self.loading_dialog {
                let scale = crate::proportional::dialog_scale(self.canvas_size());
                loading.draw(&self.painter(Vec2::ZERO, scale), &self.gameui.localization);
            }
            return;
        }
        // The windows (Options, the console) are drawn over the base, bottom to top.
        let shown = self.base;
        let menu_shown = shown == Mode::Pause || shown == Mode::MainMenu;
        if menu_shown {
            let menu_cat = if shown == Mode::Pause {
                self.pause_menu.category()
            } else {
                self.main_menu.category()
            };
            let is_widescreen = self.canvas.width() / self.canvas.height().max(1.0) >= 1.6;
            // The pre-blurred image of the upcoming background: the transition's target, else
            // the loaded map, else the menu category's own background.
            let transition = &self.background_transition;
            let upcoming = transition
                .target_category()
                .zip(transition.target_map())
                .or_else(|| {
                    self.loaded_map
                        .as_ref()
                        .map(|m| (m.game.as_str(), m.map.as_str()))
                })
                .or_else(|| {
                    let game = self.gameui.category_game(menu_cat);
                    self.gameui
                        .background_maps
                        .get(game)
                        .map(|map| (game, map.as_str()))
                });
            let bg_tex =
                upcoming.and_then(|(game, map)| self.background_texture(game, map, is_widescreen));
            if let Some(loading) = &self.loading_dialog {
                if loading.is_background() {
                    loading.draw_background_overlay(&self.canvas, bg_tex);
                }
            } else if self.background_transition.is_active() {
                let (w, h) = (self.canvas.width(), self.canvas.height());
                if let Some(tex) = bg_tex {
                    crate::gameui::loading::draw_background_texture(&self.canvas, tex);
                }
                self.canvas.rectangle(
                    0.,
                    0.,
                    w,
                    h,
                    crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP,
                );
            } else {
                let dim_alpha = self.dim_overlay_alpha(time);
                if dim_alpha > 0.001 {
                    let (w, h) = (self.canvas.width(), self.canvas.height());
                    self.canvas
                        .rectangle(0., 0., w, h, Color::new(0., 0., 0., dim_alpha));
                }
            }
            let geometry = self.menu_layout.geometry_for(self.canvas_size());
            let title = self.current_title();
            let base_focus = self.windows.base_focus_amount(time);
            let unfocused_alpha = crate::tuning::unfocused_alpha_factor();
            let alpha_mult = unfocused_alpha + (1.0 - unfocused_alpha) * base_focus;

            let title_color = Color::new(1., 1., 1., alpha_mult);
            let sub_color = if alpha_mult >= 1.0 - 1e-4 {
                Color::from_rgba(255, 255, 255, 200)
            } else {
                Color::new(1., 1., 1., (200.0 / 255.0) * alpha_mult)
            };
            let title_outline = if base_focus >= 1.0 - 1e-4 {
                TextOutline::DEFAULT
            } else if base_focus > 1e-4 {
                TextOutline::new(
                    Color::new(
                        TextOutline::DEFAULT.color.r,
                        TextOutline::DEFAULT.color.g,
                        TextOutline::DEFAULT.color.b,
                        TextOutline::DEFAULT.color.a * base_focus,
                    ),
                    1.0,
                )
            } else {
                TextOutline::NONE
            };
            let sub_outline = if base_focus >= 1.0 - 1e-4 {
                TextOutline::DEFAULT
            } else if base_focus > 1e-4 {
                TextOutline::new(
                    Color::new(
                        TextOutline::DEFAULT.color.r,
                        TextOutline::DEFAULT.color.g,
                        TextOutline::DEFAULT.color.b,
                        TextOutline::DEFAULT.color.a * base_focus,
                    ),
                    1.0,
                )
            } else {
                TextOutline::NONE
            };

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
                        if title_outline.is_visible() {
                            for &(character, char_pen) in &chars {
                                draw_cell_text_outlined(
                                    &character.to_string(),
                                    char_pen,
                                    geometry.title_tall,
                                    self.title_font.as_ref(),
                                    self.title_metrics,
                                    Color::new(0., 0., 0., 0.),
                                    title_outline,
                                );
                            }
                        }
                        for &(character, char_pen) in &chars {
                            draw_cell_text(
                                &self.canvas,
                                &character.to_string(),
                                char_pen,
                                geometry.title_tall,
                                self.title_font.as_ref(),
                                self.title_metrics,
                                title_color,
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
                                sub_color,
                                sub_outline,
                            );
                        } else if self.title_font_can_render(line) {
                            draw_cell_text_outlined(
                                line,
                                pen,
                                geometry.title_tall,
                                self.title_font.as_ref(),
                                self.title_metrics,
                                sub_color,
                                sub_outline,
                            );
                        } else {
                            let sub_tall = (geometry.title_tall * 0.55).round().max(14.);
                            draw_cell_text_outlined(
                                line,
                                pen,
                                sub_tall,
                                self.menu_font.as_ref(),
                                self.menu_metrics,
                                sub_color,
                                sub_outline,
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
                            title_color
                        } else {
                            sub_color
                        },
                        if line_idx == 0 {
                            title_outline
                        } else {
                            sub_outline
                        },
                    );
                }
            }
            let menu = if shown == Mode::MainMenu {
                &self.main_menu
            } else {
                &self.pause_menu
            };
            // With the keyboard elsewhere (a window has the focus), a row is armed only under
            // the pointer or while pressed.
            let menu_keyboard = self.windows.focus() == Layer::Base;
            let modal_open = self.modal_dialog_open();
            for (i, entry) in menu.entries().iter().enumerate() {
                if entry.spacer {
                    continue;
                }
                let is_pressed = !modal_open && self.menu_tracker.is_pressed(i);
                let selected = if menu_keyboard {
                    menu.selection == Some(i)
                } else {
                    self.last_menu_hover == Some(i)
                };
                let armed = !modal_open && (is_pressed || selected);
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
                let color = if alpha_mult >= 1.0 - 1e-4 {
                    color
                } else {
                    Color::new(color.r, color.g, color.b, color.a * alpha_mult)
                };
                let border_alpha = if base_focus > 1e-4 {
                    border_color.a * base_focus
                } else {
                    0.0
                };
                let button_outline = if border_alpha > 1e-4 {
                    TextOutline::new(Color::new(0.0, 0.0, 0.0, border_alpha), 1.0)
                } else {
                    TextOutline::NONE
                };
                let offset = if is_pressed { vec2(1., 1.) } else { Vec2::ZERO };
                draw_cell_text_outlined(
                    &entry.label,
                    geometry.row_text_pos(i) + offset,
                    geometry.font_tall,
                    self.menu_font.as_ref(),
                    self.menu_metrics,
                    color,
                    button_outline,
                );
            }
        }
        let confirm_box_to_draw = self
            .confirm_box
            .as_ref()
            .or(self.closing_confirm_box.as_ref());
        let quit_bounds = confirm_box_to_draw.map(|(confirm, _)| {
            let (origin, scale, layout) = self.quit_geometry(confirm);
            Rect::new(
                origin.x,
                origin.y,
                layout.size.x * scale,
                layout.size.y * scale,
            )
        });

        let active_windows: Vec<Window> = self.windows.order().collect();
        for (idx, &window) in active_windows.iter().enumerate() {
            let mut occluders = Vec::new();
            if let Some(qb) = quit_bounds {
                occluders.push(qb);
            }
            for &above in &active_windows[idx + 1..] {
                match above {
                    Window::Console => occluders.push(self.window_bounds(Window::Console)),
                    Window::Options => occluders.push(self.window_bounds(Window::Options)),
                    _ => {}
                }
            }

            match window {
                Window::Options => {
                    let theme = self.window_theme(window, time);
                    let alpha = self.windows.window_composite_alpha(Window::Options, time);
                    let old_alpha = self.canvas.set_alpha_multiplier(alpha);
                    let options_focused = self.windows.focus() == Layer::Window(Window::Options);
                    self.options.draw_with_occluders(
                        &Painter {
                            theme: &theme,
                            ..self.painter(Vec2::ZERO, 1.)
                        },
                        &occluders,
                        Some(time),
                        Some(options_focused),
                    );
                    self.canvas.set_alpha_multiplier(old_alpha);
                }
                Window::Console => {
                    let alpha = self.windows.window_composite_alpha(Window::Console, time);
                    let old_alpha = self.canvas.set_alpha_multiplier(alpha);
                    self.draw_console_with_occluders(time, &occluders);
                    self.canvas.set_alpha_multiplier(old_alpha);
                }
                Window::SubDialog | Window::Confirmation => {}
            }
        }
        // The confirmation box is modal over every window.
        if let Some((confirm, _)) = confirm_box_to_draw {
            let (origin, scale, layout) = self.quit_geometry(confirm);
            let painter = self.painter(origin, scale);
            let theme = self.window_theme(Window::Confirmation, time);
            let alpha = self
                .windows
                .window_composite_alpha(Window::Confirmation, time);
            let old_alpha = self.canvas.set_alpha_multiplier(alpha);
            confirm.draw(
                &Painter {
                    theme: &theme,
                    ..painter
                },
                &layout,
            );
            self.canvas.set_alpha_multiplier(old_alpha);
        }
        if let Some(loading) = &self.loading_dialog {
            let scale = crate::proportional::dialog_scale(self.canvas_size());
            loading.draw(&self.painter(Vec2::ZERO, scale), &self.gameui.localization);
        }
    }
}

/// Warning and error lines (not in the scheme; HL2-RS colors).
const WARNING_TEXT: Color = Color::new(1.0, 0.8, 0.3, 1.0);
const ERROR_TEXT: Color = Color::new(1.0, 0.4, 0.35, 1.0);
/// SDK `CConsolePanel` shows at most this many completion rows, the last one `...` when more.
const COMPLETION_ROWS: usize = 10;
/// History rows scrolled per mouse wheel notch.
const WHEEL_ROWS: usize = 3;
/// Dialog push buttons play this when they activate.
const RELEASE_SOUND: &str = "ui/buttonclickrelease.wav";
/// Console history and completion row height, text size and baseline distance from the row
/// bottom, in layout units.
const CONSOLE_ROW: f32 = 14.;
const CONSOLE_TEXT: f32 = 12.;
const CONSOLE_DESCENT: f32 = 3.;
/// Space between the history's border and its rows.
const HISTORY_PAD: f32 = 3.;

/// A tab is this many spaces wide in the history.
const TAB_SPACES: usize = 8;
/// Advance per EM of every character when the console font is missing.
const NOMINAL_ADVANCE: f32 = 0.6;

/// What history rows wrap to: the text width and font size in canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WrapMetrics {
    width: f32,
    em: f32,
    /// Whether the console font measures characters (else they take a nominal advance).
    font: bool,
}

/// A line's wrapped rows and what they were wrapped from.
struct Wrapped {
    metrics: WrapMetrics,
    /// The time shown before the text (`con_timestamp`).
    stamp: Option<f64>,
    text: String,
    rows: Vec<Range<usize>>,
}

/// A history row: a byte range of a shown line's [`Console::shown_text`].
struct HistoryRow<'a> {
    number: u64,
    line: &'a Line,
    text: Range<usize>,
}

/// The history's text rows and scroll bar inside its border, in frame layout units.
struct HistoryView {
    text: Rect,
    bar: Rect,
    /// Whole rows that fit.
    rows: usize,
}

impl HistoryView {
    fn new(layout: &DialogLayout) -> Self {
        let h = layout.history;
        let bar = Rect::new(
            h.x + h.w - 1. - SCROLLBAR_WIDTH,
            h.y + 1.,
            SCROLLBAR_WIDTH,
            (h.h - 2.).max(0.),
        );
        let text = Rect::new(
            h.x + HISTORY_PAD,
            h.y + HISTORY_PAD,
            (bar.x - h.x - 2. * HISTORY_PAD).max(0.),
            (h.h - 2. * HISTORY_PAD).max(0.),
        );
        let rows = ((text.h / CONSOLE_ROW) as usize).max(1);
        Self { text, bar, rows }
    }

    /// The row at layout height `y`; `clamp` puts points above or below on the first or last.
    fn row_at(&self, y: f32, clamp: bool) -> Option<usize> {
        let row = ((y - self.text.y) / CONSOLE_ROW).floor();
        if clamp {
            Some(row.clamp(0., (self.rows - 1) as f32) as usize)
        } else {
            (row >= 0. && (row as usize) < self.rows).then_some(row as usize)
        }
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
#[cfg(test)]
mod tests;
