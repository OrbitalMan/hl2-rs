//! A bounded standalone console. Commands without a Rust implementation report an error.
pub mod commands;
pub mod complete;
pub mod dialog;
pub mod editor;

use crate::gameui::{
    display::DisplayModes,
    menu::{GameMenu, MenuLayout, MenuOutcome, MessageBox, MessageBoxLayout},
    options::{OptionsDialog, OptionsOutcome},
    placement::{self, CursorHint, Placement},
    resources::{self, fallback_game_menu, GameUiResources},
    widgets::{
        scheme_color, Align, ButtonState, ButtonTracker, Fonts, HoldRepeat, Painter,
        ScrollAccumulator, ScrollBarLayout, ScrollBarState, ScrollMode, Theme, SCROLLBAR_WIDTH,
    },
};
use crate::{
    canvas::*,
    hud::{FontFace, TextOutline},
};
use dialog::{wrap, DialogLayout, Selection};
use glam::{vec2, Vec2};
use serde::Serialize;
use source_assets::{keyvalues, vpk::Vfs};
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
    history_bar: ScrollBarState,
    history_wheel: ScrollAccumulator,
    key_repeat: KeyRepeat,
    /// The input line: editing, completion, history and reverse search.
    editor: editor::LineEditor,
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
    pub loading_dialog: Option<crate::gameui::loading::LoadingDialog>,
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
    /// The menu the options dialog was opened from (drawn behind it and restored on close).
    options_return: Mode,
    /// `(game, image)` -> pre-blurred background (see `GameUiResources::background_images`).
    pub background_textures: BTreeMap<(String, String), Texture2D>,
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
impl Key {
    /// Keys that repeat while held: moving through lists and text, deleting. Enter, Tab, Home
    /// and End act once per press.
    pub const fn repeats(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::Backspace
                | Self::Delete
                | Self::PageUp
                | Self::PageDown
        )
    }
}

/// Keyboard repeat for every UI layer at once, applied to [`Input::keys`] in [`Console::input`]:
/// like an operating system, the latest pressed key repeats while held ([`HoldRepeat`]), and any
/// other press takes over or (when it does not repeat) stops it. Typed text already arrives with
/// the system's own repeat and is left alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyRepeat {
    key: Option<Key>,
    timer: HoldRepeat,
}
impl KeyRepeat {
    /// Adds this frame's repeat of the held key to `input.keys`.
    pub fn apply(&mut self, input: &mut Input) {
        if let Some(&pressed) = input.keys.iter().next_back() {
            self.key = pressed.repeats().then_some(pressed);
            self.timer.press(input.time);
        } else if let Some(key) = self.key {
            if !input.held_keys.contains(&key) {
                self.cancel();
            } else if self.timer.held(input.time) {
                input.keys.insert(key);
            }
        }
    }

    /// Stops the repeat until the next press.
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Input {
    pub keys: std::collections::BTreeSet<Key>,
    /// Keys held down this frame (pressed earlier or now); [`KeyRepeat`] repeats them.
    pub held_keys: std::collections::BTreeSet<Key>,
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
    /// Monotonic real time of this frame in seconds, used for double clicks. `None` (scripted
    /// input without times) disables double-click detection.
    pub time: Option<f64>,
    /// Control is held (console editing shortcuts); typed text is ignored meanwhile.
    pub ctrl: bool,
    /// Shift is held (Shift+Tab steps the completion list back).
    pub shift: bool,
    /// Text pasted this frame (the host reads the clipboard on its paste shortcut).
    pub paste: Option<String>,
    /// The copy shortcut (Cmd+C on macOS, Ctrl+C elsewhere) was pressed this frame.
    pub copy: bool,
}
impl Input {
    pub(crate) fn pressed(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }
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
            previous: Mode::Gameplay,
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
            history_bar: ScrollBarState::default(),
            history_wheel: ScrollAccumulator::default(),
            key_repeat: KeyRepeat::default(),
            editor: editor::LineEditor::default(),
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
            loading_dialog: None,
            background_transition: crate::gameui::loading::BackgroundTransitionState::Idle,
            background_level: false,
            loaded_map: None,
            theme: Theme::default(),
            widget_fonts: Fonts::default(),
            style: Style::default(),
            menu_tracker: ButtonTracker::new(),
            last_menu_hover: None,
            options: OptionsDialog::default(),
            options_return: Mode::MainMenu,
            background_textures: BTreeMap::new(),
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
        // Menu items draw the source scheme's MenuLarge face (GorDIN 600 in the installed
        // scheme) from the font files the scheme registers.
        let reader = crate::gameui::resources::Reader::new(vfs);
        let menu_bytes = reader
            .read("resource/SourceScheme.res")
            .and_then(|bytes| keyvalues::parse_resource(&bytes).ok())
            .and_then(|roots| roots.into_iter().next())
            .and_then(|root| {
                crate::gameui::scheme_fonts::resolve(&root, "MenuLarge", |path| reader.read(path))
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

    /// The pre-blurred image of `game`'s background map `map`, if loaded.
    pub fn background_texture(
        &self,
        game: &str,
        map: &str,
        widescreen: bool,
    ) -> Option<&Texture2D> {
        let key = |name: String| (game.to_owned(), name);
        let map = crate::gameui::resources::blur(map);
        widescreen
            .then(|| {
                self.background_textures
                    .get(&key(format!("{map}_widescreen")))
            })
            .flatten()
            .or_else(|| self.background_textures.get(&key(map.to_owned())))
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
        let menu_cat = if shown == Mode::Pause {
            self.pause_menu.category()
        } else {
            self.main_menu.category()
        };
        let category = self.background_transition.displayed_category(menu_cat);
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

    /// The effective main-menu background: the persisted override (a map saved without its
    /// game belongs to the game that ships it), else chapter 1 of the first game's
    /// `scripts/chapterbackgrounds.txt`.
    pub fn background(&self) -> Option<GameMap> {
        let gameplay = &self.config.gameplay;
        match (&gameplay.background_game, &gameplay.background_map) {
            (Some(game), Some(map)) => Some(GameMap::new(game, map)),
            (None, Some(map)) => Some(self.game_map(map, None)),
            (_, None) => self
                .gameui
                .background_map
                .as_ref()
                .map(|map| GameMap::new(self.gameui.main_game(), map)),
        }
    }

    /// Overrides the persistent main-menu background (`None` restores the installed default).
    pub fn set_background(&mut self, background: Option<GameMap>) {
        let (game, map) = background.map(|b| (b.game, b.map)).unzip();
        self.config.gameplay.background_game = game;
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
        if self.loaded_map.is_none() && background {
            self.loaded_map = self.background();
        }
        self.mode = if background {
            Mode::MainMenu
        } else {
            Mode::Gameplay
        };
        self.previous = self.mode;
    }

    /// Completes loading `game`'s `map`. Ignores stale background completions.
    pub fn level_loaded_for_map(&mut self, game: &str, map: &str, background: bool) {
        if background
            && self.background_transition.is_active()
            && !self.background_transition.finish_map(game, map)
        {
            return;
        }
        self.loaded_map = Some(GameMap::new(game, map));
        self.level_loaded(background);
    }

    /// `map` of the game that ships it: `current` first, then Half-Life 2, then the other
    /// installed games (see `source_assets::install::game_for_map`).
    fn game_map(&self, map: &str, current: Option<&str>) -> GameMap {
        let game = self
            .gameui
            .install_root
            .as_deref()
            .and_then(|root| source_assets::install::game_for_map(root, map, current))
            .unwrap_or_else(|| current.unwrap_or(self.gameui.main_game()).to_owned());
        GameMap::new(game, map)
    }

    /// The background to show when leaving a map for the main menu: the running game's own
    /// chapter 1 background, which becomes the saved menu background (a Half-Life 2 map played
    /// from the Episode Two menu returns to Half-Life 2's background), else the saved one.
    fn menu_background_after_game(&mut self) -> Option<GameMap> {
        let running = self.loaded_map.as_ref().map(|m| m.game.clone());
        let Some((game, map)) = running.and_then(|game| {
            let map = self.gameui.background_maps.get(&game)?.clone();
            Some((game, map))
        }) else {
            return self.background();
        };
        let background = GameMap::new(game, map);
        self.set_background(Some(background.clone()));
        let _ = self.config.save();
        Some(background)
    }

    /// Shows a game's menu category over that game's own chapter 1 background, loading it
    /// unless it is already the loaded background level.
    fn show_category(&mut self, category: &str) -> Vec<Effect> {
        let game = self.gameui.category_game(category).to_owned();
        let Some(map) = self.gameui.background_maps.get(&game).cloned() else {
            self.background_transition.set_displayed_category(category);
            return Vec::new();
        };
        let background = GameMap::new(game, map);
        self.set_background(Some(background.clone()));
        let _ = self.config.save();
        if self.background_level && self.loaded_map.as_ref() == Some(&background) {
            self.background_transition
                .set_displayed_category(&background.game);
            return Vec::new();
        }
        self.start_background_loading(&background.game, &background.map);
        vec![Effect::MainMenuMap(background)]
    }

    /// Displays the modal loading dialog for the given map.
    pub fn start_loading(&mut self, map: &str) {
        self.clear_menu_hover();
        self.loading_dialog = Some(crate::gameui::loading::LoadingDialog::new(map));
        crate::gameui::loading::reset_progress();
    }

    /// Starts the background transition to `game`'s `map` and shows the corner loading plaque.
    /// The transition's category is the game, whose subtitle shows once the map has loaded.
    pub fn start_background_loading(&mut self, game: &str, map: &str) {
        self.background_transition.start(map, game);
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
        crate::gameui::loading::reset_progress();
    }

    /// Ends the background transition if `game`'s `map` is the target, ignoring stale failures.
    pub fn background_load_failed_for_map(&mut self, game: &str, map: &str, reason: &str) {
        if self.background_transition.is_active()
            && !self.background_transition.fail_map(game, map, reason)
        {
            return;
        }
        self.loading_dialog = None;
        crate::gameui::loading::reset_progress();
    }

    /// Marks the active loading dialog as failed with the given error message.
    pub fn loading_failed(&mut self, error: &str) {
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

    /// The `mat_monitorgamma` to display: the brightness dialog's live value while it is open,
    /// otherwise the applied setting.
    pub fn display_gamma(&self) -> f32 {
        let preview = (self.mode == Mode::Options)
            .then(|| self.options.preview_gamma())
            .flatten();
        preview.unwrap_or(self.config.video.gamma)
    }

    /// The window's display mode changed outside the game's settings (macOS native full screen):
    /// the applied configuration and an open Options dialog follow it.
    pub fn set_display_mode_external(&mut self, mode: crate::config::DisplayMode) {
        self.config.video.display_mode = mode;
        self.options.sync_display_mode(mode);
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
        // A drag or resize in progress is undone first.
        if self.quit_placement.cancel()
            || (self.mode == Mode::Console && self.console_placement.cancel())
        {
            return;
        }
        // In the console Escape first ends a history search, then hides the completion list.
        if self.mode == Mode::Console && self.editor.dismiss() {
            return;
        }
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
    /// Opens or closes the console; it opens only while `con_enable` is on.
    pub fn toggle(&mut self) {
        if self.mode == Mode::Console {
            self.console_placement.end();
            self.mode = self.previous;
            self.clear_menu_hover();
        } else if self.config.keyboard.developer_console {
            self.previous = self.mode;
            self.clear_menu_hover();
            self.mode = Mode::Console;
            let frame = self.console_frame();
            self.console_placement.reset_if_off_screen(&frame);
        }
    }
    pub fn log(&mut self, text: impl Into<String>) {
        self.print(Level::Normal, text);
    }
    /// Prints a line at `level`, stamped with the current time.
    pub fn print(&mut self, level: Level, text: impl Into<String>) {
        self.output.push_back(Line {
            level,
            text: text.into(),
            time: self.clock,
        });
        if self.output.len() > OUTPUT_LINES {
            self.output.pop_front();
            self.output_start += 1;
        }
        // Scrolled back, the view stays on its rows while new ones arrive below.
        let number = self.output_start + self.output.len() as u64 - 1;
        if let Some(line) = self
            .output
            .back()
            .filter(|line| self.scroll > 0 && (self.con_filter != 1 || self.filter_match(line)))
        {
            self.scroll += self.line_rows(number, line, self.wrap_metrics()).len();
        }
    }
    pub fn submit(&mut self, line: &str) -> Vec<Effect> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        self.print(Level::Input, format!("] {line}"));
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
            commands::find(&name).map_or_else(
                || format!("Unknown or unimplemented command: {}", tokens[0]),
                |spec| spec.help.to_string(),
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
                let found: Vec<_> = commands::COMMANDS
                    .iter()
                    .filter(|spec| {
                        query.as_ref().is_none_or(|q| {
                            if name == "help" {
                                spec.name == q
                            } else {
                                spec.help.to_ascii_lowercase().contains(q)
                            }
                        })
                    })
                    .collect();
                if found.is_empty() {
                    self.log("No implemented command matches.");
                }
                for spec in found {
                    self.log(spec.help);
                }
            }
            "clear" => {
                if !args.is_empty() {
                    return Err(usage());
                }
                self.clear_output();
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
            "con_filter_enable" | "con_filter_text" | "con_timestamp" => match args {
                [] => {
                    let value = self.cvar_value(&name).unwrap_or_default();
                    self.log(format!("\"{name}\" = \"{value}\""));
                }
                [value] => {
                    match name.as_str() {
                        "con_filter_enable" => {
                            self.con_filter = value
                                .parse::<u8>()
                                .ok()
                                .filter(|v| *v <= 2)
                                .ok_or_else(usage)?;
                        }
                        "con_filter_text" => self.con_filter_text = value.clone(),
                        _ => self.con_timestamp = boolean(value).ok_or_else(usage)?,
                    }
                    self.scroll = 0;
                }
                _ => return Err(usage()),
            },
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
                        self.set_background(None);
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
                        let current = self.loaded_map.as_ref().map(|m| m.game.as_str());
                        let background = self.game_map(map, current);
                        self.set_background(Some(background));
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
                let current = self.loaded_map.as_ref().map(|m| m.game.as_str());
                let background = self.game_map(map, current);
                self.set_background(Some(background.clone()));
                let _ = self.config.save();
                if self.is_loading() && !self.background_level {
                    self.log("Gameplay map load in progress; background map persisted without interrupting load.");
                    return Ok(None);
                }
                self.start_background_loading(&background.game, &background.map);
                return Ok(Some(Effect::MainMenuMap(background)));
            }
            _ => return Err(usage()),
        }
        Ok(None)
    }
    /// The current value of an implemented cvar, as the console prints it.
    pub fn cvar_value(&self, name: &str) -> Option<String> {
        let c = &self.config;
        Some(match name.to_ascii_lowercase().as_str() {
            "sv_cheats" => u8::from(self.cheats).to_string(),
            "volume" => format!("{:.2}", c.audio.volume),
            "sensitivity" => format!("{:.2}", c.mouse.sensitivity),
            "fov" => format!("{:.0}", c.video.fov),
            "mat_hdr_level" => c.video.hdr.level().to_string(),
            "background_map" => self.background_map()?.to_owned(),
            "con_filter_enable" => self.con_filter.to_string(),
            "con_filter_text" => self.con_filter_text.clone(),
            "con_timestamp" => u8::from(self.con_timestamp).to_string(),
            _ => return None,
        })
    }

    /// The text being typed on the console line.
    pub fn input_line(&self) -> &str {
        self.editor.text()
    }

    /// Names that command arguments complete from (installed maps, the loaded map's entities).
    pub fn set_completion_sources(&mut self, sources: complete::Sources) {
        self.completion = sources;
        self.editor.refresh(&self.completion);
    }

    /// Restores the command history the host saved.
    pub fn set_history(&mut self, history: Vec<String>) {
        self.editor.set_history(history);
    }

    /// The command history, once after it changed (the host saves it then).
    pub fn take_changed_history(&mut self) -> Option<Vec<String>> {
        self.editor
            .take_history_changed()
            .then(|| self.editor.history().to_vec())
    }
    /// Called before gameplay input collection. Opening characters are drained by the host.
    pub fn input(&mut self, input: &Input) -> Vec<Effect> {
        let effects = self.input_frame(input);
        // A drag, resize or reset that ended this frame is saved at once.
        self.save_placements();
        effects
    }

    fn input_frame(&mut self, input: &Input) -> Vec<Effect> {
        // Held keys repeat for every layer below, except into a key binding being captured.
        let mut repeated = input.clone();
        if self.mode == Mode::Options && self.options.capturing_key() {
            self.key_repeat.cancel();
        } else {
            self.key_repeat.apply(&mut repeated);
        }
        let mode = self.mode;
        let effects = self.layer_input(&repeated);
        if self.mode != mode {
            self.key_repeat.cancel();
        }
        effects
    }

    fn layer_input(&mut self, input: &Input) -> Vec<Effect> {
        if self.modal_dialog_open() {
            // The menus are occluded: keep their tracker on the physical button so the rest of
            // a click that closes the modal layer is not read as a new menu press.
            self.menu_tracker.occlude(input.click || input.held);
        }
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
        // The dialog's copy of the settings predates any drag made while it was open.
        let mut config = config;
        config.dialogs = self.config.dialogs;
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
            let frame = self.quit_frame(&confirm);
            if self
                .quit_placement
                .input(&frame, input.pointer, input.click, input.held, input.time)
            {
                self.confirm_box = Some((confirm, kind));
                return effects;
            }
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
                        match self.menu_background_after_game() {
                            Some(background) => effects.push(Effect::MainMenuMap(background)),
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
        if let Some(time) = input.time {
            self.clock = time;
        }
        // The caption and corner grips own the pointer before the history's row selection.
        self.pointer = input.pointer;
        let frame = self.console_frame();
        let placing = self.console_placement.input(
            &frame,
            input.pointer,
            input.click,
            input.held,
            input.time,
        );
        if !placing {
            if let Some(effects) = self.console_pointer(input) {
                return effects;
            }
        }
        let ctrl_key = |name: &str| input.ctrl && input.binding_keys.iter().any(|k| k == name);
        let editor = &mut self.editor;
        if !input.ctrl {
            let typed: String = input
                .text
                .chars()
                .filter(|c| *c != '`' && *c != '~')
                .collect();
            editor.insert(&typed);
        }
        if let Some(paste) = &input.paste {
            editor.paste(paste);
        }
        if ctrl_key("KeyR") {
            editor.search_back();
        }
        if ctrl_key("KeyU") {
            editor.kill_to_start();
        }
        if ctrl_key("KeyK") {
            editor.kill_to_end();
        }
        if ctrl_key("KeyW") {
            editor.delete_word_back();
        }
        if ctrl_key("KeyA") {
            editor.home();
        }
        if ctrl_key("KeyE") {
            editor.end();
        }
        if input.pressed(Key::Backspace) {
            if input.ctrl {
                editor.delete_word_back();
            } else {
                editor.backspace();
            }
        }
        if input.pressed(Key::Delete) {
            if input.ctrl {
                editor.delete_word_forward();
            } else {
                editor.delete();
            }
        }
        if input.pressed(Key::Left) {
            if input.ctrl {
                editor.word_left();
            } else {
                editor.left();
            }
        }
        if input.pressed(Key::Right) {
            if input.ctrl {
                editor.word_right();
            } else {
                editor.right();
            }
        }
        if input.pressed(Key::Home) {
            if input.ctrl {
                self.scroll = self.history_rows().len();
            } else {
                self.editor.home();
            }
        }
        if input.pressed(Key::End) {
            if input.ctrl {
                self.scroll = 0;
            } else {
                self.editor.end();
            }
        }
        if input.pressed(Key::PageUp) {
            self.scroll = (self.scroll + 10).min(self.history_rows().len().saturating_sub(1));
        }
        if input.pressed(Key::PageDown) {
            self.scroll = self.scroll.saturating_sub(10);
        }
        // Completion follows the edited line before Tab/Up/Down step through it.
        self.editor.refresh(&self.completion);
        if input.pressed(Key::Up) {
            self.editor.step(true, false);
        }
        if input.pressed(Key::Down) {
            self.editor.step(false, false);
        }
        if input.pressed(Key::Tab) {
            self.editor.step(input.shift, true);
            self.editor.refresh(&self.completion);
        }
        if ctrl_key("KeyL") {
            self.clear_output();
        }
        if input.copy {
            self.copied = self.selected_text();
        }
        if input.pressed(Key::Enter) {
            let line = self.editor.submit();
            self.scroll = 0;
            return self.submit(&line);
        }
        Vec::new()
    }
    /// Pointer input of the console dialog: the close and Submit buttons, the history scroll
    /// bar, the wheel, and dragging over history rows to select them. `Some` when a button
    /// ended this frame's input (the console closed, or the line was submitted).
    fn console_pointer(&mut self, input: &Input) -> Option<Vec<Effect>> {
        let layout = self.dialog_layout();
        let view = HistoryView::new(&layout);
        let point = input.pointer.map(|p| layout.local(p));
        let down = input.click || input.held;
        let rows: Vec<u64> = self.history_rows().iter().map(|row| row.number).collect();
        let total = rows.len();
        let max_scroll = total.saturating_sub(view.rows);
        let mode = ScrollMode::platform();
        let steps = self.history_wheel.steps(input.wheel, mode) as i64
            * mode.rows_per_step(WHEEL_ROWS) as i64;
        self.scroll = (self.scroll as i64 + steps).clamp(0, max_scroll as i64) as usize;
        self.scroll = self.scroll.min(max_scroll);
        let first = total - view.rows.min(total) - self.scroll;
        let bar = ScrollBarLayout::new(view.bar, first, view.rows, total);
        if !self.selecting && self.history_bar.wants(&bar, point, input.click) {
            if let Some(first) = self.history_bar.pointer(
                &bar,
                point,
                down,
                input.click,
                input.time,
                first,
                view.rows,
            ) {
                self.scroll = max_scroll.saturating_sub(first);
            }
            self.console_buttons.occlude(down);
            self.console_hover = None;
            return None;
        }
        let hovered = point.and_then(|p| {
            [
                (ConsoleButton::Close, layout.close),
                (ConsoleButton::Submit, layout.submit),
            ]
            .into_iter()
            .find(|(_, rect)| rect.contains(p))
            .map(|(button, _)| button)
        });
        if self.selecting {
            self.console_buttons.occlude(down);
            self.console_hover = None;
        } else {
            self.console_hover = hovered;
            match self
                .console_buttons
                .update(hovered, down, input.click, input.released)
            {
                Some(ConsoleButton::Close) => {
                    self.console_hover = None;
                    self.toggle();
                    return Some(vec![Effect::PlaySound(RELEASE_SOUND.into())]);
                }
                Some(ConsoleButton::Submit) => {
                    let line = self.editor.submit();
                    self.scroll = 0;
                    let mut effects = vec![Effect::PlaySound(RELEASE_SOUND.into())];
                    effects.extend(self.submit(&line));
                    return Some(effects);
                }
                None => {}
            }
        }
        if let Some(p) = point {
            if input.click && hovered.is_none() && view.text.contains(p) {
                self.selecting = true;
                self.selection = view
                    .row_at(p.y, false)
                    .and_then(|row| rows.get(first + row))
                    .map(|number| Selection::at(*number));
            } else if self.selecting && down {
                // Dragging past the top or bottom scrolls the history a row per frame.
                if p.y < view.text.y {
                    self.scroll = (self.scroll + 1).min(max_scroll);
                } else if p.y > view.text.y + view.text.h {
                    self.scroll = self.scroll.saturating_sub(1);
                }
                let first = total - view.rows.min(total) - self.scroll;
                let row = view.row_at(p.y, true).unwrap_or(0);
                let line = rows.get(first + row).or(rows.last()).copied();
                if let (Some(selection), Some(line)) = (self.selection.as_mut(), line) {
                    selection.head = line;
                }
            }
        }
        if !down {
            self.selecting = false;
        }
        None
    }

    /// Empties the history (`clear`, Ctrl+L). Line numbers keep counting, so nothing printed
    /// later can take a selected line's number.
    fn clear_output(&mut self) {
        self.output_start += self.output.len() as u64;
        self.output.clear();
        self.scroll = 0;
        self.selection = None;
        self.selecting = false;
    }

    /// The console dialog's frame: retail placement and minimum size, sizeable by its corners.
    fn console_frame(&self) -> placement::Frame {
        let canvas = vec2(self.canvas.width(), self.canvas.height());
        let (origin, size) = DialogLayout::default_frame(canvas);
        placement::Frame {
            origin,
            size,
            scale: Painter::scale_for(canvas.y),
            canvas,
            min_size: Some(dialog::MIN_SIZE),
            close: crate::gameui::widgets::close_button_rect,
        }
    }

    fn dialog_layout(&self) -> DialogLayout {
        let frame = self.console_frame();
        let placed = self.console_placement.place(&frame);
        DialogLayout::at(frame.scale, placed.origin, placed.size)
    }

    /// Saves the Options dialog's and the console's placement once a drag, resize or reset changed
    /// it (the previous input frame's).
    fn save_placements(&mut self) {
        let mut changed = false;
        if let Some(options) = self.options.take_placement_changed() {
            self.config.dialogs.options = options;
            changed = true;
        }
        if self.console_placement.take_changed() {
            self.config.dialogs.console = self.console_placement.saved();
            changed = true;
        }
        if changed {
            if let Err(error) = self.config.save() {
                self.log(format!("Could not save settings: {error}"));
            }
        }
    }

    /// The quit / main-menu box's frame (centered, moveable, no close button).
    fn quit_frame(&self, quit: &MessageBox) -> placement::Frame {
        let scale = Painter::scale_for(self.canvas.height());
        let measure = self.painter(Vec2::ZERO, 1.);
        let size = quit.layout(|text| measure.text_width(text, false)).size;
        let canvas = vec2(self.canvas.width(), self.canvas.height());
        placement::Frame {
            origin: ((canvas - size * scale) * 0.5).round(),
            size,
            scale,
            canvas,
            min_size: None,
            close: |_| Rect::new(0., 0., 0., 0.),
        }
    }

    /// The pointer shape the window should show: a resize cursor over or while dragging one of the
    /// console's corner grips, otherwise the arrow.
    pub fn cursor_hint(&self) -> CursorHint {
        if self.mode != Mode::Console || self.confirm_box.is_some() {
            return CursorHint::Arrow;
        }
        self.console_placement
            .hint(&self.console_frame(), self.pointer)
    }

    /// The output the history shows (`con_filter_enable` 1 hides lines without
    /// `con_filter_text`), with each line's number.
    fn shown_lines(&self) -> Vec<(u64, &Line)> {
        (self.output_start..)
            .zip(self.output.iter())
            .filter(|(_, line)| self.con_filter != 1 || self.filter_match(line))
            .collect()
    }

    /// A line as the history shows it, with its time under `con_timestamp`.
    fn shown_text(&self, line: &Line) -> String {
        if self.con_timestamp {
            format!("[{:7.2}] {}", line.time, line.text)
        } else {
            line.text.clone()
        }
    }

    /// The history's text width and font size in canvas pixels, which rows wrap to.
    fn wrap_metrics(&self) -> WrapMetrics {
        let layout = self.dialog_layout();
        WrapMetrics {
            width: layout.canvas(HistoryView::new(&layout).text).w,
            em: (CONSOLE_TEXT * layout.scale) as u16 as f32,
            font: self.mono.is_some(),
        }
    }

    /// Advance of `c` in the console font. A tab is eight spaces wide (SDK `RichText`); without
    /// the font every character takes a nominal monospace advance.
    fn console_advance(&self, c: char, em: f32) -> f32 {
        if c == '\t' {
            return TAB_SPACES as f32 * self.console_advance(' ', em);
        }
        let Some(font) = self.mono.as_ref() else {
            return em * NOMINAL_ADVANCE;
        };
        match self.ascii_table(em) {
            Some(table) if c.is_ascii() => table[c as usize],
            _ => font.advance_em(c, em),
        }
    }

    /// The console font's ASCII advances at `em` (tab already eight spaces), built once per size.
    fn ascii_table(&self, em: f32) -> Option<[f32; 128]> {
        let font = self.mono.as_ref()?;
        let mut table = self
            .ascii_advances
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if table.is_none_or(|(size, _)| size != em.to_bits()) {
            let mut advances = [0.; 128];
            for (code, advance) in advances.iter_mut().enumerate() {
                *advance = font.advance_em(char::from(code as u8), em);
            }
            advances[usize::from(b'\t')] = TAB_SPACES as f32 * advances[usize::from(b' ')];
            *table = Some((em.to_bits(), advances));
        }
        table.map(|(_, advances)| advances)
    }

    /// The rows line `number` wraps to, as byte ranges into its [`Self::shown_text`].
    fn line_rows(&self, number: u64, line: &Line, metrics: WrapMetrics) -> Vec<Range<usize>> {
        let stamp = self.con_timestamp.then_some(line.time);
        let mut cache = self.wrapped.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = cache.get(&number).filter(|entry| {
            entry.metrics == metrics && entry.stamp == stamp && entry.text == line.text
        }) {
            return entry.rows.clone();
        }
        let table = self.ascii_table(metrics.em);
        let rows = wrap(&self.shown_text(line), metrics.width, |c| match table {
            Some(table) if c.is_ascii() => table[c as usize],
            _ => self.console_advance(c, metrics.em),
        });
        if cache.len() >= 2 * OUTPUT_LINES {
            cache.retain(|n, _| *n >= self.output_start);
        }
        cache.insert(
            number,
            Wrapped {
                metrics,
                stamp,
                text: line.text.clone(),
                rows: rows.clone(),
            },
        );
        rows
    }

    /// The history's rows: the shown lines wrapped to its width at word boundaries, a row for
    /// each line of text that holds newlines.
    fn history_rows(&self) -> Vec<HistoryRow<'_>> {
        let metrics = self.wrap_metrics();
        self.shown_lines()
            .into_iter()
            .flat_map(|(number, line)| {
                self.line_rows(number, line, metrics)
                    .into_iter()
                    .map(move |text| HistoryRow { number, line, text })
            })
            .collect()
    }

    /// The selected history lines as text, one line each and whole however they wrap.
    fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let lines: Vec<String> = self
            .shown_lines()
            .into_iter()
            .filter(|(number, _)| selection.contains(*number))
            .map(|(_, line)| self.shown_text(line))
            .collect();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// Text the copy shortcut copied from the history, for the host to put on the clipboard.
    pub fn take_copied(&mut self) -> Option<String> {
        self.copied.take()
    }

    fn console_button_state(&self, button: ConsoleButton) -> ButtonState {
        let hovered = self.console_hover == Some(button);
        ButtonState {
            armed: self.console_buttons.armable(self.console_hover) == Some(button),
            pressed: hovered && self.console_buttons.is_pressed(button),
            enabled: true,
            // SDK `CConsolePanel` makes Submit the default button.
            default: button == ConsoleButton::Submit,
        }
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
        let origin = self.quit_placement.place(&self.quit_frame(quit)).origin;
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
                self.options
                    .reset_placement_if_off_screen(vec2(self.canvas.width(), self.canvas.height()));
            }
            MenuOutcome::ConfirmQuit => {
                self.reset_menu_interaction();
                self.quit_placement.reset();
                self.confirm_box = Some((MessageBox::quit(loc), ConfirmKind::Quit));
            }
            MenuOutcome::NewGame(map) => {
                self.clear_menu_hover();
                self.start_loading(&map);
                return vec![Effect::Map(map)];
            }
            MenuOutcome::ReturnToMainMenu => {
                self.reset_menu_interaction();
                self.quit_placement.reset();
                self.confirm_box = Some((
                    MessageBox::return_to_main_menu(loc),
                    ConfirmKind::ReturnToMainMenu,
                ));
            }
            MenuOutcome::SwitchExpansion(category) => return self.show_category(&category),
            MenuOutcome::SwitchCategory(category) => {
                if category != "main" && category != "extras" {
                    return self.show_category(&category);
                } else if category == "main" {
                    self.background_transition.set_displayed_category("main");
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
        // The console and the options dialog are drawn over what they were opened from.
        let base = if self.mode == Mode::Console {
            self.previous
        } else {
            self.mode
        };
        let shown = if base == Mode::Options {
            self.options_return
        } else {
            base
        };
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
                    draw_texture_ex(
                        &self.canvas,
                        tex,
                        0.,
                        0.,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(w, h)),
                            ..Default::default()
                        },
                    );
                }
                self.canvas.rectangle(
                    0.,
                    0.,
                    w,
                    h,
                    crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP,
                );
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
            if base == Mode::Options {
                self.options.draw(&self.painter(Vec2::ZERO, 1.));
            }
        }
        if self.mode == Mode::Console {
            self.draw_console(time);
        }
        if let Some(loading) = &self.loading_dialog {
            let scale = Painter::scale_for(self.canvas.height());
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

impl Console {
    /// Whether `line` passes `con_filter_enable` 1 (and is drawn brighter under 2).
    fn filter_match(&self, line: &Line) -> bool {
        self.con_filter_text.is_empty()
            || line
                .text
                .to_lowercase()
                .contains(&self.con_filter_text.to_lowercase())
    }

    fn line_color(&self, line: &Line) -> Color {
        if self.con_filter == 2 && !self.con_filter_text.is_empty() && self.filter_match(line) {
            return self.style.dev_text;
        }
        match line.level {
            Level::Input | Level::Normal => self.style.text,
            Level::Developer => self.style.dev_text,
            Level::Warning => WARNING_TEXT,
            Level::Error => ERROR_TEXT,
        }
    }

    /// The console dialog (SDK `CConsoleDialog`): a titled frame with the history, the entry
    /// and the Submit button, then the completion popup over everything.
    fn draw_console(&self, time: f64) {
        let layout = self.dialog_layout();
        let scale = layout.scale;
        // Over the Options dialog the console is opaque, like every stacked dialog.
        let stacked = self.theme.stacked();
        let painter = Painter {
            theme: if self.previous == Mode::Options {
                &stacked
            } else {
                &self.theme
            },
            ..self.painter(layout.origin, scale)
        };
        let em = (CONSOLE_TEXT * scale) as u16 as f32;
        let text = |content: &str, x: f32, y: f32, color: Color| {
            text(
                &self.canvas,
                content,
                vec2(x, y),
                em,
                1.,
                self.mono.as_ref(),
                color,
            );
        };
        let localization = &self.gameui.localization;
        painter.frame_with_close(
            layout.size,
            &localization.text("#Console_Title"),
            Some(self.console_button_state(ConsoleButton::Close)),
        );

        // History (`RichText`, `DepressedButtonBorder`): filtered, wrapped, scrolled, one color
        // per level, the rows of selected lines highlighted.
        painter.fill(layout.history, self.style.entry);
        painter.sunken(layout.history);
        let view = HistoryView::new(&layout);
        let history = self.history_rows();
        let rows = view.rows.min(history.len());
        let scroll = self.scroll.min(history.len() - rows);
        let first = history.len() - rows - scroll;
        let area = layout.canvas(view.text);
        let row = CONSOLE_ROW * scale;
        for (i, shown) in history[first..first + rows].iter().enumerate() {
            let top = area.y + i as f32 * row;
            let selected = self.selection.is_some_and(|s| s.contains(shown.number));
            let color = if selected {
                self.canvas
                    .rectangle(area.x, top, area.w, row, self.style.selected_bg);
                self.style.selected_text
            } else {
                self.line_color(shown.line)
            };
            let content = self.shown_text(shown.line)[shown.text.clone()]
                .replace('\t', &" ".repeat(TAB_SPACES));
            text(&content, area.x, top + row - CONSOLE_DESCENT * scale, color);
        }
        painter.scroll_bar(
            view.bar,
            first,
            view.rows,
            history.len(),
            self.history_bar.pressed(),
        );

        // The entry (`TextEntry`), or the reverse history search in its place.
        painter.fill(layout.entry, painter.theme.entry_bg);
        painter.sunken(layout.entry);
        let (line, cursor) = match self.editor.search() {
            Some((query, found)) => {
                let label = localization.text("#HL2RS_Console_ReverseSearch");
                let line = format!("({label})`{query}': {}", found.unwrap_or_default());
                let cursor = line.chars().count();
                (line, cursor)
            }
            None => (self.editor.text().to_owned(), self.editor.cursor()),
        };
        let inner = Rect::new(
            layout.entry.x + 3.,
            layout.entry.y,
            layout.entry.w - 6.,
            layout.entry.h,
        );
        // Scroll the line so the cursor stays inside the entry.
        let chars: Vec<char> = line.chars().collect();
        let width = |from: usize, to: usize| {
            painter.text_width(&chars[from..to].iter().collect::<String>(), false)
        };
        let mut start = 0;
        while start < cursor && width(start, cursor) > inner.w - 2. {
            start += 1;
        }
        let mut end = cursor;
        while end < chars.len() && width(start, end + 1) <= inner.w {
            end += 1;
        }
        let shown: String = chars[start..end].iter().collect();
        painter.text_in(&shown, inner, Align::Left, painter.theme.text, false);
        if ((time * 2.) as u64).is_multiple_of(2) {
            let x = inner.x + width(start, cursor) + 1.;
            painter.fill(
                Rect::new(x, inner.y + 4., 1., inner.h - 8.),
                painter.theme.text,
            );
        }
        painter.button(
            layout.submit,
            &localization.text("#Console_Submit"),
            self.console_button_state(ConsoleButton::Submit),
        );
        let entry = layout.canvas(layout.entry);
        self.draw_completion(scale, (entry.x, entry.y, entry.w, entry.h), &text);
    }

    /// The completion popup (SDK `CConsolePanel` completion list, VGUI menu colors): a help
    /// line for the command being typed, then up to `COMPLETION_ROWS` items, cvars with their
    /// value. It opens below the entry when it fits, else above it.
    fn draw_completion(
        &self,
        scale: f32,
        (x, entry_y, w, entry_h): (f32, f32, f32, f32),
        text: &dyn Fn(&str, f32, f32, Color),
    ) {
        let theme = &self.theme;
        let mut rows: Vec<Vec<(String, Color)>> = Vec::new();
        let mut highlighted = None;
        if self.editor.search().is_none() {
            if let Some(help) = complete::arg_help(self.editor.text()) {
                let mut runs = vec![(help.name.to_owned(), theme.text)];
                for (i, arg) in help.args.iter().enumerate() {
                    let color = if help.current == Some(i) {
                        theme.text
                    } else {
                        theme.text_dull
                    };
                    runs.push((format!(" {arg}"), color));
                }
                if help.cvar {
                    if let Some(value) = self.cvar_value(help.name) {
                        runs.push((format!(" = \"{value}\""), self.style.dev_text));
                    }
                }
                runs.push((format!("   {}", help.description), theme.label_text));
                rows.push(runs);
            }
        }
        let help_rows = rows.len();
        if let Some((items, index)) = self.editor.popup() {
            let more = items.len() > COMPLETION_ROWS;
            let shown = if more {
                COMPLETION_ROWS - 1
            } else {
                items.len()
            };
            // Keep the item stepped to in view.
            let first = index.map_or(0, |i| (i + 1).saturating_sub(shown));
            for (i, item) in items.iter().enumerate().skip(first).take(shown) {
                let mut runs = vec![(item.text.clone(), theme.text)];
                if let Some(value) = commands::find(&item.text)
                    .filter(|spec| spec.cvar)
                    .and_then(|spec| self.cvar_value(spec.name))
                {
                    runs.push((format!(" {value}"), self.style.dev_text));
                }
                if !item.detail.is_empty() {
                    runs.push((format!("   {}", item.detail), theme.label_text));
                }
                if index == Some(i) {
                    highlighted = Some(rows.len());
                }
                rows.push(runs);
            }
            if more {
                rows.push(vec![("...".to_owned(), theme.text_dull)]);
            }
        }
        if rows.is_empty() {
            return;
        }
        // Like a VGUI menu, the list is as wide as its widest row (at least the entry), up to
        // the canvas edge.
        let widest = rows
            .iter()
            .map(|runs| {
                runs.iter()
                    .map(|(content, _)| self.console_text_width(content, scale))
                    .sum::<f32>()
            })
            .fold(0., f32::max);
        let w = w
            .max(widest + 12. * scale)
            .min(self.canvas.width() - x)
            .max(0.);
        let row = CONSOLE_ROW * scale;
        let h = rows.len() as f32 * row + 4. * scale;
        let below = entry_y + entry_h;
        let y = if below + h <= self.canvas.height() {
            below
        } else {
            (entry_y - h).max(0.)
        };
        self.canvas.rectangle(
            x,
            y,
            w,
            h,
            Color::new(theme.menu_bg.r, theme.menu_bg.g, theme.menu_bg.b, 1.),
        );
        rectangle_lines(&self.canvas, x, y, w, h, 1., theme.border_dark);
        let max_width = w - 12. * scale;
        for (r, runs) in rows.iter().enumerate() {
            let ry = y + 2. * scale + r as f32 * row;
            if highlighted == Some(r) {
                self.canvas
                    .rectangle(x + 1., ry, w - 2., row, theme.selection_bg);
            }
            if r == help_rows && help_rows > 0 {
                self.canvas
                    .rectangle(x + 1., ry, w - 2., 1., theme.border_dark);
            }
            let mut cx = x + 6. * scale;
            for (content, color) in runs {
                let color = if highlighted == Some(r) {
                    theme.selection_text
                } else {
                    *color
                };
                let mut clipped = String::new();
                for c in content.chars() {
                    clipped.push(c);
                    if cx - x + self.console_text_width(&clipped, scale) > max_width {
                        clipped.pop();
                        break;
                    }
                }
                text(
                    &clipped,
                    cx,
                    ry + (CONSOLE_ROW - CONSOLE_DESCENT) * scale,
                    color,
                );
                cx += self.console_text_width(&clipped, scale);
            }
        }
    }

    fn console_text_width(&self, content: &str, scale: f32) -> f32 {
        self.mono.as_ref().map_or(0., |font| {
            font.width_em(content, (CONSOLE_TEXT * scale) as u16 as f32)
        })
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
    fn console_returns_to_its_previous_pause_state_and_steps_through_history() {
        let mut console = Console::default();
        console.escape();
        assert!(console.paused());
        console.toggle();
        assert_eq!(console.mode, Mode::Console);
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        console.toggle();
        for line in ["echo first", "echo second"] {
            console.input(&Input {
                text: line.into(),
                keys: [Key::Enter].into(),
                ..Default::default()
            });
        }
        // An empty line: Up and Down step through the history, newest first.
        keys(&mut console, &[Key::Up]);
        assert_eq!(console.input_line(), "echo second");
        keys(&mut console, &[Key::Up]);
        assert_eq!(console.input_line(), "echo first");
        keys(&mut console, &[Key::Down]);
        assert_eq!(console.input_line(), "echo second");
        // Escape hides the list, then closes the console.
        console.escape();
        assert_eq!(console.mode, Mode::Console);
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        assert_eq!(
            console.take_changed_history(),
            Some(vec!["echo first".to_owned(), "echo second".to_owned()])
        );
        assert_eq!(console.take_changed_history(), None);
    }
    #[test]
    fn explicit_text_input_edits_unicode_at_boundaries_and_completes_commands() {
        let mut console = Console::default();
        console.toggle();
        console.input(&Input {
            text: "aé🦀".into(),
            ..Default::default()
        });
        keys(&mut console, &[Key::Left]);
        keys(&mut console, &[Key::Backspace]);
        assert_eq!(console.input_line(), "a🦀");
        keys(&mut console, &[Key::Delete]);
        assert_eq!(console.input_line(), "a");
        keys(&mut console, &[Key::Backspace]);
        console.input(&Input {
            text: "noc".into(),
            keys: [Key::Tab].into(),
            ..Default::default()
        });
        assert_eq!(
            console.input_line(),
            "noclip ",
            "one match: taken with a space"
        );
        keys(&mut console, &[Key::Enter]);
        assert!(console.input_line().is_empty());
        assert!(console.output.iter().any(|s| s.contains("sv_cheats")));
    }
    #[test]
    fn console_shortcuts_search_paste_filter_and_timestamps() {
        let mut console = Console::default();
        console.toggle();
        console.set_history(vec!["fov 90".into(), "volume 0.5".into()]);
        console.input(&Input {
            text: "x".into(),
            ctrl: true,
            binding_keys: vec!["KeyR".into()],
            ..Default::default()
        });
        console.input(&Input {
            text: "fov".into(),
            ..Default::default()
        });
        assert_eq!(console.editor.search(), Some(("fov", Some("fov 90"))));
        console.escape();
        assert_eq!(console.mode, Mode::Console, "Escape ends the search first");
        console.input(&Input {
            paste: Some("echo pasted\n".into()),
            ..Default::default()
        });
        assert_eq!(console.input_line(), "echo pasted");
        console.input(&Input {
            ctrl: true,
            binding_keys: vec!["KeyU".into()],
            ..Default::default()
        });
        assert_eq!(console.input_line(), "");
        console.submit("con_filter_enable 1; con_filter_text Loaded; con_timestamp 1");
        assert_eq!(
            console.cvar_value("con_filter_enable").as_deref(),
            Some("1")
        );
        assert_eq!(
            console.cvar_value("con_filter_text").as_deref(),
            Some("Loaded")
        );
        assert!(console.con_timestamp);
        assert!(console.submit("con_filter_enable 3").is_empty());
        assert_eq!(
            console.cvar_value("con_filter_enable").as_deref(),
            Some("1")
        );
        let loaded = Line {
            level: Level::Normal,
            text: "Loaded hl2/background01".into(),
            time: 0.,
        };
        assert!(console.filter_match(&loaded));
        assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            "\"Loaded hl2/background01\"",
            "lines serialize as their text in the run report"
        );
        console.input(&Input {
            ctrl: true,
            binding_keys: vec!["KeyL".into()],
            ..Default::default()
        });
        assert!(console.output.is_empty(), "Ctrl+L clears the output");
    }
    /// Canvas point at the middle of a frame-relative layout rectangle of the console dialog.
    fn dialog_point(console: &Console, rect: Rect) -> Vec2 {
        let r = console.dialog_layout().canvas(rect);
        vec2(r.x + r.w * 0.5, r.y + r.h * 0.5)
    }
    /// Canvas point on the history row `row` (0 is the top row shown).
    fn history_point(console: &Console, row: usize) -> Vec2 {
        let layout = console.dialog_layout();
        let text = layout.canvas(HistoryView::new(&layout).text);
        vec2(
            text.x + 10.,
            text.y + (row as f32 + 0.5) * CONSOLE_ROW * layout.scale,
        )
    }
    fn press(console: &mut Console, at: Vec2) -> Vec<Effect> {
        console.input(&Input {
            pointer: Some(at),
            click: true,
            held: true,
            ..Default::default()
        })
    }
    fn release(console: &mut Console, at: Vec2) -> Vec<Effect> {
        console.input(&Input {
            pointer: Some(at),
            released: true,
            ..Default::default()
        })
    }
    /// Drags over the history from row `from` to row `to`.
    fn drag(console: &mut Console, from: usize, to: usize) {
        press(console, history_point(console, from));
        let to = history_point(console, to);
        console.input(&Input {
            pointer: Some(to),
            held: true,
            ..Default::default()
        });
        release(console, to);
    }
    fn copy(console: &mut Console) -> Option<String> {
        console.input(&Input {
            copy: true,
            ..Default::default()
        });
        console.take_copied()
    }

    /// One frame at time `t` with `pressed` going down now and `held` keys down.
    fn key_frame(t: f64, pressed: &[Key], held: &[Key]) -> Input {
        Input {
            keys: pressed.iter().copied().collect(),
            held_keys: held.iter().chain(pressed).copied().collect(),
            time: Some(t),
            ..Default::default()
        }
    }

    #[test]
    fn key_repeat_follows_the_latest_press_while_it_is_held() {
        let mut repeat = KeyRepeat::default();
        let mut frame = |t: f64, pressed: &[Key], held: &[Key]| {
            let mut input = key_frame(t, pressed, held);
            repeat.apply(&mut input);
            input.keys.into_iter().collect::<Vec<_>>()
        };
        assert_eq!(frame(0., &[Key::Down], &[]), [Key::Down]);
        assert_eq!(frame(0.2, &[], &[Key::Down]), []);
        assert_eq!(frame(0.4, &[], &[Key::Down]), [Key::Down]);
        // A newer press takes over; the earlier key, still held, no longer repeats.
        assert_eq!(frame(0.5, &[Key::Left], &[Key::Down]), [Key::Left]);
        assert_eq!(frame(0.9, &[], &[Key::Down, Key::Left]), [Key::Left]);
        // Releasing stops it, and it stays stopped when the key goes down again unpressed.
        assert_eq!(frame(1.0, &[], &[Key::Down]), []);
        assert_eq!(frame(2.0, &[], &[Key::Down, Key::Left]), []);
        // Keys that act once never repeat, and their press stops the repeating key.
        assert_eq!(frame(3.0, &[Key::Up], &[]), [Key::Up]);
        assert_eq!(frame(3.1, &[Key::Enter], &[Key::Up]), [Key::Enter]);
        assert_eq!(frame(4.0, &[], &[Key::Up, Key::Enter]), []);
        // Scripted input carries no time: nothing repeats.
        let mut input = key_frame(0., &[Key::Down], &[]);
        input.time = None;
        repeat.apply(&mut input);
        let mut held = Input {
            held_keys: [Key::Down].into(),
            ..Default::default()
        };
        repeat.apply(&mut held);
        assert!(held.keys.is_empty());
    }

    #[test]
    fn held_keys_repeat_in_the_console_and_the_main_menu() {
        let mut console = Console::default();
        console.toggle();
        console.input(&Input {
            text: "abcdefghij".into(),
            ..Default::default()
        });
        console.input(&key_frame(0., &[Key::Backspace], &[]));
        for i in 1..=10 {
            console.input(&key_frame(i as f64 * 0.1, &[], &[Key::Backspace]));
        }
        // One press, then repeats at 0.4, 0.5, ... 1.0.
        assert_eq!(console.input_line(), "ab");
        console.input(&key_frame(1.1, &[], &[]));
        console.input(&key_frame(2.0, &[], &[Key::Backspace]));
        assert_eq!(
            console.input_line(),
            "ab",
            "released: no repeat until pressed again"
        );

        // Holding Down on the main menu walks it like three separate presses.
        let mut stepped = installed_console();
        stepped.level_loaded(true);
        for _ in 0..3 {
            keys(&mut stepped, &[Key::Down]);
        }
        let mut held = installed_console();
        held.level_loaded(true);
        held.input(&key_frame(0., &[Key::Down], &[]));
        for t in [0.2, 0.4, 0.45] {
            held.input(&key_frame(t, &[], &[Key::Down]));
        }
        assert!(stepped.main_menu.selection.is_some());
        assert_eq!(held.main_menu.selection, stepped.main_menu.selection);
    }

    /// Press at `from`, move held to `to`, release there.
    fn pointer_drag(console: &mut Console, from: Vec2, to: Vec2) {
        press(console, from);
        console.input(&Input {
            pointer: Some(to),
            held: true,
            ..Default::default()
        });
        release(console, to);
    }

    #[test]
    fn the_console_moves_by_its_caption_and_escape_undoes_a_drag() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.toggle();
        let start = console.dialog_layout();
        let caption = start.origin + vec2(100., 10.) * start.scale;
        pointer_drag(&mut console, caption, caption - vec2(200., 50.));
        let moved = console.dialog_layout();
        assert_eq!(moved.origin, start.origin - vec2(200., 50.));
        assert_eq!(moved.size, start.size);
        // The caption press is the placement's: no history row selection started.
        assert_eq!(copy(&mut console), None);
        // Escape during a drag restores the frame and keeps the console open.
        let caption = moved.origin + vec2(100., 10.) * moved.scale;
        press(&mut console, caption);
        console.input(&Input {
            pointer: Some(caption + vec2(300., 300.)),
            held: true,
            ..Default::default()
        });
        console.escape();
        assert_eq!(console.mode, Mode::Console);
        assert_eq!(console.dialog_layout().origin, moved.origin);
        // Closing and reopening keeps the place.
        console.toggle();
        console.toggle();
        assert_eq!(console.dialog_layout().origin, moved.origin);
    }

    #[test]
    fn the_console_resizes_by_its_corners_within_limits() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.toggle();
        let start = console.dialog_layout();
        let corner = start.origin + start.size * start.scale - Vec2::splat(4.);
        console.pointer = Some(corner);
        assert_eq!(console.cursor_hint(), CursorHint::ResizeNwSe);
        pointer_drag(&mut console, corner, corner + vec2(-100., -60.));
        let resized = console.dialog_layout();
        assert_eq!(resized.origin, start.origin, "the top-left stays");
        assert!((resized.size.x - (start.size.x - 100. / start.scale)).abs() <= 1.);
        assert!(resized.entry.w > 0. && resized.history.h > 0.);
        // Way past the minimum: the layout keeps every control.
        let corner = resized.origin + resized.size * resized.scale - Vec2::splat(4.);
        pointer_drag(&mut console, corner, start.origin);
        assert_eq!(console.dialog_layout().size, dialog::MIN_SIZE);
        // The top-left grip shows the same diagonal; the caption keeps the arrow.
        console.pointer = Some(console.dialog_layout().origin + Vec2::splat(2.));
        assert_eq!(console.cursor_hint(), CursorHint::ResizeNwSe);
        console.pointer = Some(console.dialog_layout().origin + vec2(60., 10.) * start.scale);
        assert_eq!(console.cursor_hint(), CursorHint::Arrow);
        // A smaller window never leaves the console larger than it or off it.
        console.canvas.resize(640., 480.);
        let small = console.dialog_layout();
        assert!(small.origin.x >= 0. && small.origin.y >= 0.);
        assert!(small.origin.x + small.size.x * small.scale <= 640.);
        assert!(small.origin.y + small.size.y * small.scale <= 480.);
    }

    #[test]
    fn console_placement_survives_a_restart_and_resets_off_screen() {
        let mut console = Console::default();
        console.canvas.resize(1920., 1080.);
        console.toggle();
        let start = console.dialog_layout();
        let caption = start.origin + vec2(100., 10.) * start.scale;
        pointer_drag(&mut console, caption, caption - vec2(600., 0.));
        let corner = console.dialog_layout().origin + console.dialog_layout().size * start.scale
            - Vec2::splat(4.);
        pointer_drag(&mut console, corner, corner - vec2(100., 100.));
        console.input(&Input::default());
        let saved = console
            .config
            .dialogs
            .console
            .expect("saved after the gesture");
        let placed = console.dialog_layout();
        // The next run restores it.
        let mut next = Console::default();
        next.canvas.resize(1920., 1080.);
        next.console_placement = Placement::from_saved(Some(saved));
        next.toggle();
        assert_eq!(next.dialog_layout(), placed);
        // Opened on a smaller window where it would not fit as saved: the default frame.
        let mut small = Console::default();
        small.canvas.resize(800., 600.);
        small.console_placement = Placement::from_saved(Some(saved));
        small.toggle();
        assert_eq!(small.dialog_layout(), DialogLayout::new(vec2(800., 600.)));
        small.input(&Input::default());
        assert_eq!(small.config.dialogs.console, None, "the reset is saved");
    }

    #[test]
    fn the_quit_box_moves_alone_and_opens_centered() {
        let mut console = Console::default();
        console.canvas.resize(1280., 720.);
        console.mode = Mode::MainMenu;
        let loc = console.gameui.localization.clone();
        console.quit_placement.reset();
        console.confirm_box = Some((MessageBox::quit(&loc), ConfirmKind::Quit));
        let quit = console.confirm_box.as_ref().unwrap().0.clone();
        let (origin, _, _) = console.quit_geometry(&quit);
        pointer_drag(
            &mut console,
            origin + vec2(40., 8.),
            origin + vec2(140., 108.),
        );
        assert!(
            console.confirm_box.is_some(),
            "dragging does not answer the box"
        );
        assert_eq!(console.quit_geometry(&quit).0, origin + vec2(100., 100.));
        console.escape();
        assert!(console.confirm_box.is_none());
        console.quit_placement.reset();
        assert_eq!(console.quit_geometry(&quit).0, origin);
    }

    #[test]
    fn dragging_over_history_rows_selects_them_for_copy() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.toggle();
        console.output.clear();
        for i in 0..5 {
            console.log(format!("line {i}"));
        }
        // Rows 1..=3 by dragging upwards; the selection holds whole lines.
        drag(&mut console, 3, 1);
        assert_eq!(
            copy(&mut console).as_deref(),
            Some("line 1\nline 2\nline 3")
        );
        assert_eq!(console.take_copied(), None, "taken once");

        // New output and lines dropped from the front leave the selection on its lines.
        for i in 5..OUTPUT_LINES + 5 {
            console.log(format!("line {i}"));
        }
        assert_eq!(copy(&mut console), None, "the selected lines scrolled out");
        console.output.clear();
        console.output_start = 0;
        for i in 0..5 {
            console.log(format!("line {i}"));
        }
        drag(&mut console, 2, 2);
        console.log("printed after");
        console.output.pop_front();
        console.output_start += 1;
        assert_eq!(copy(&mut console).as_deref(), Some("line 2"));

        // Copying follows what the history shows: the filter and timestamps.
        drag(&mut console, 0, 3);
        console.submit("con_filter_enable 1; con_filter_text 3; con_timestamp 1");
        let copied = copy(&mut console).unwrap();
        assert!(copied.ends_with("] line 3"), "{copied}");
        assert!(!copied.contains('\n'), "{copied}");

        console.input(&Input {
            ctrl: true,
            binding_keys: vec!["KeyL".into()],
            ..Default::default()
        });
        assert_eq!(copy(&mut console), None, "Ctrl+L clears the selection");
        console.submit("con_filter_enable 0");
        drag(&mut console, 0, 0);
        assert!(copy(&mut console).is_some());
        console.submit("clear");
        console.log("after clear");
        assert_eq!(copy(&mut console), None, "clear drops the selection too");
    }

    #[test]
    fn scrolled_back_history_stays_put_while_output_arrives() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.toggle();
        for i in 0..200 {
            console.log(format!("line {i}"));
        }
        console.input(&Input {
            wheel: 2.,
            ..Default::default()
        });
        let rows = 2 * ScrollMode::platform().rows_per_step(WHEEL_ROWS);
        assert_eq!(console.scroll, rows);
        console.log("more");
        assert_eq!(console.scroll, rows + 1);
        keys(&mut console, &[Key::Enter]);
        assert_eq!(console.scroll, 0, "submitting returns to the newest output");
    }

    /// Characters that fit a history row with the nominal advance of a console without fonts.
    fn row_chars(console: &Console) -> usize {
        let metrics = console.wrap_metrics();
        (metrics.width / (metrics.em * NOMINAL_ADVANCE)) as usize
    }
    /// The top history row shown: its line number and text.
    fn top_row(console: &Console) -> (u64, String) {
        let rows = HistoryView::new(&console.dialog_layout()).rows;
        let history = console.history_rows();
        let shown = rows.min(history.len());
        let first = history.len() - shown - console.scroll.min(history.len() - shown);
        let row = &history[first];
        (
            row.number,
            console.shown_text(row.line)[row.text.clone()].to_owned(),
        )
    }
    /// Words long enough to wrap to at least `rows` history rows.
    fn long_line(console: &Console, rows: usize) -> String {
        let words = rows * row_chars(console) / 5 + 1;
        (0..words)
            .map(|i| format!("w{i:03}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn long_lines_wrap_to_rows_and_newlines_start_rows() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.toggle();
        console.output.clear();
        console.output_start = 0;
        let long = long_line(&console, 3);
        let warning =
            "Encountered an error in command `<Enable the debug feature to see the name>`:\n\
                       Entity despawned: The entity with ID 2224v1 is invalid";
        console.log("first");
        console.log(long.clone());
        console.print(Level::Warning, warning);
        console.log("last");
        let metrics = console.wrap_metrics();
        let history = console.history_rows();
        let rows: Vec<(u64, String)> = history
            .iter()
            .map(|row| {
                (
                    row.number,
                    console.shown_text(row.line)[row.text.clone()].to_owned(),
                )
            })
            .collect();
        let long_rows = rows.iter().filter(|(n, _)| *n == 1).count();
        assert!(long_rows >= 3, "{rows:?}");
        // Rows fit the history and break between words; joined they give the line back.
        for (_, text) in &rows {
            let width: f32 = text
                .chars()
                .map(|c| console.console_advance(c, metrics.em))
                .sum();
            assert!(width <= metrics.width, "{text}");
            assert!(!text.starts_with(' ') && !text.ends_with(' '), "{text:?}");
        }
        let joined: Vec<&str> = rows[1..=long_rows]
            .iter()
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(joined.join(" "), long);
        // The warning wraps, and its newline starts a row.
        let warning_rows: Vec<&str> = rows
            .iter()
            .filter(|(n, _)| *n == 2)
            .map(|(_, t)| t.as_str())
            .collect();
        let count = warning_rows.len();
        assert!(count >= 3, "{warning_rows:?}");
        assert!(warning_rows[count - 2].ends_with("name>`:"));
        assert_eq!(
            warning_rows[count - 1],
            "Entity despawned: The entity with ID 2224v1 is invalid"
        );
        assert_eq!(rows.last().unwrap(), &(3, "last".to_owned()));

        // Selecting any row of a wrapped line copies the whole line as printed.
        drag(&mut console, 2, 2);
        assert_eq!(copy(&mut console).as_deref(), Some(long.as_str()));
        let warning_row = long_rows + count;
        drag(&mut console, warning_row, warning_row);
        assert_eq!(copy(&mut console).as_deref(), Some(warning));
        drag(&mut console, rows.len() - 1, 0);
        assert_eq!(
            copy(&mut console),
            Some(format!("first\n{long}\n{warning}\nlast"))
        );

        // Timestamps wrap with the text and copy with it.
        console.submit("con_timestamp 1");
        assert_eq!(top_row(&console), (0, "[   0.00] first".to_owned()));
        drag(&mut console, 2, 2);
        let copied = copy(&mut console).unwrap();
        assert!(
            copied.starts_with("[   0.00] w000") && copied.ends_with(&long),
            "{copied}"
        );
        // The filter keeps or hides whole lines, all their rows.
        // (Set directly: the echo of a `con_filter_text` command contains its text.)
        console.submit("con_timestamp 0; con_filter_enable 1");
        console.con_filter_text = "despawned".into();
        let shown: Vec<u64> = console.history_rows().iter().map(|r| r.number).collect();
        assert_eq!(shown, vec![2; count]);
    }

    #[test]
    fn scrolled_back_wrapped_history_stays_put_and_reaches_the_top() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.toggle();
        console.output.clear();
        console.output_start = 0;
        let long = long_line(&console, 2);
        for i in 0..100 {
            console.log(if i % 2 == 0 {
                long.clone()
            } else {
                format!("line {i}")
            });
        }
        console.input(&Input {
            wheel: 2.,
            ..Default::default()
        });
        let top = top_row(&console);
        let scroll = console.scroll;
        assert!(scroll > 0);
        // New output below adds its rows to the scroll, so the shown rows stay.
        console.log(long.clone());
        let added = console
            .history_rows()
            .iter()
            .filter(|r| r.number == 100)
            .count();
        assert!(added >= 2);
        assert_eq!(console.scroll, scroll + added);
        console.log("one\ntwo");
        assert_eq!(console.scroll, scroll + added + 2);
        assert_eq!(top_row(&console), top);
        console.input(&Input::default());
        assert_eq!(top_row(&console), top, "the next frame keeps the view");

        // Ctrl+Home goes to the first row of the first line, Page Up stops there too.
        console.input(&Input {
            keys: [Key::Home].into(),
            ctrl: true,
            ..Default::default()
        });
        console.input(&Input::default());
        assert_eq!(top_row(&console).0, 0);
        assert!(long.starts_with(&top_row(&console).1));
        keys(&mut console, &[Key::PageDown]);
        keys(&mut console, &[Key::PageUp]);
        keys(&mut console, &[Key::PageUp]);
        assert_eq!(top_row(&console).0, 0);
        assert!(long.starts_with(&top_row(&console).1));
        console.input(&Input {
            keys: [Key::End].into(),
            ctrl: true,
            ..Default::default()
        });
        assert_eq!(console.scroll, 0);
    }

    #[test]
    fn submit_button_runs_the_line_and_close_button_closes_the_console() {
        let mut console = Console::default();
        console.canvas.resize(1024., 768.);
        console.escape();
        assert_eq!(console.mode, Mode::Pause);
        console.toggle();
        console.input(&Input {
            text: "echo clicked".into(),
            ..Default::default()
        });
        let submit = dialog_point(&console, console.dialog_layout().submit);
        press(&mut console, submit);
        assert_eq!(console.input_line(), "echo clicked", "runs on release");
        assert_eq!(
            release(&mut console, submit),
            vec![Effect::PlaySound(RELEASE_SOUND.into())]
        );
        assert_eq!(console.input_line(), "");
        assert_eq!(
            console.output.back().map(|l| l.text.as_str()),
            Some("clicked")
        );

        let close = dialog_point(&console, console.dialog_layout().close);
        press(&mut console, close);
        assert_eq!(console.mode, Mode::Console);
        release(&mut console, close);
        assert_eq!(console.mode, Mode::Pause, "back to what it was opened from");
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
    /// Game -> map test data, standing in for the installed chapter files.
    fn test_maps(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(game, map)| ((*game).to_owned(), (*map).to_owned()))
            .collect()
    }

    /// A console with the built-in menu structure, a known first chapter and a background map.
    fn installed_console() -> Console {
        let loc = Localization::default();
        let resources = GameUiResources {
            localization: loc,
            game_menu: fallback_game_menu(),
            games: ["hl2", "episodic", "ep2", "lostcoast"]
                .map(String::from)
                .to_vec(),
            background_map: Some("background01".into()),
            background_maps: test_maps(&[
                ("hl2", "background01"),
                ("episodic", "ep1_background01"),
                ("ep2", "ep2_background01"),
                ("lostcoast", "background01"),
            ]),
            new_game_maps: test_maps(&[
                ("hl2", "d1_trainstation_01"),
                ("episodic", "ep1_citadel_00"),
                ("ep2", "ep2_outland_01"),
                ("lostcoast", "d2_lostcoast"),
            ]),
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
                Effect::MainMenuMap(GameMap::new("hl2", "background01")),
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
                Effect::MainMenuMap(GameMap::new("episodic", "ep1_background01")),
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
                Effect::MainMenuMap(GameMap::new("ep2", "ep2_background01")),
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
        console.set_background(Some(GameMap::new("hl2", "background02")));
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
                Effect::MainMenuMap(GameMap::new("hl2", "background02")),
            ]
        );
        assert_eq!(console.mode, Mode::MainMenu);
    }

    #[test]
    fn return_to_main_menu_shows_the_running_games_background() {
        let mut console = installed_console();
        // The Episode Two menu is showing, then a Half-Life 2 map is loaded from the console.
        console.set_background(Some(GameMap::new("ep2", "ep2_background01")));
        console.level_loaded_for_map("ep2", "ep2_background01", true);
        assert_eq!(
            console.submit("map d1_trainstation_02"),
            vec![Effect::Map("d1_trainstation_02".into())]
        );
        console.level_loaded_for_map("hl2", "d1_trainstation_02", false);
        assert_eq!(console.mode, Mode::Gameplay);

        console.escape();
        click_row(&mut console, 5); // Return to main menu
        let (confirm, _) = console.confirm_box.clone().expect("confirmation box");
        let (origin, scale, layout) = console.quit_geometry(&confirm);
        let pointer =
            Some(origin + vec2(layout.buttons[0].x + 3., layout.buttons[0].y + 3.) * scale);
        let press = Input {
            pointer,
            click: true,
            held: true,
            ..Default::default()
        };
        let _ = console.input(&press);
        let effects = console.input(&Input {
            pointer,
            released: true,
            ..Default::default()
        });
        assert!(
            effects.contains(&Effect::MainMenuMap(GameMap::new("hl2", "background01"))),
            "{effects:?}"
        );
        assert_eq!(
            console.background(),
            Some(GameMap::new("hl2", "background01"))
        );
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
        assert_eq!(
            effects,
            vec![Effect::MainMenuMap(GameMap::new("hl2", "background04"))]
        );
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
    fn click_that_dismisses_a_modal_layer_does_not_reach_the_menu_below() {
        let mut console = installed_console();
        console.mode = Mode::MainMenu;
        console.canvas.resize(1280., 720.);
        console.start_loading("invalid_map");
        console.loading_failed("Map invalid_map not found");
        let at = row(&console, 0);
        // The press dismisses the failed loading dialog; the held and release frames of the
        // same click land on a menu row and must not activate it.
        let mut effects = Vec::new();
        for (click, held, released) in [
            (true, true, false),
            (false, true, false),
            (false, false, true),
        ] {
            effects.extend(console.input(&Input {
                pointer: at,
                click,
                held,
                released,
                ..Default::default()
            }));
        }
        assert!(console.loading_dialog.is_none());
        assert_eq!(console.mode, Mode::MainMenu);
        assert!(
            !effects.contains(&Effect::PlaySound("ui/buttonclickrelease.wav".into())),
            "no menu activation from the dismissing click: {effects:?}"
        );
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
        console.loaded_map = Some(GameMap::new("hl2", "background01"));

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
            effects.contains(&Effect::MainMenuMap(GameMap::new(
                "episodic",
                "ep1_background01"
            ))),
            "dispatches Effect::MainMenuMap for ep1_background01"
        );

        // Map loads and level_loaded is called
        console.level_loaded_for_map("episodic", "ep1_background01", true);
        assert_eq!(
            console.main_menu.category(),
            "episodic",
            "category remains in episodic submenu after background map load"
        );
        assert_eq!(
            console.loaded_map,
            Some(GameMap::new("episodic", "ep1_background01"))
        );
        assert!(!console.is_loading());

        // 2. Return to main category via Escape, then enter Episode Two submenu
        console.escape();
        assert_eq!(console.main_menu.category(), "main");
        // Background map remains ep1_background01 until a different expansion is selected
        assert_eq!(
            console.loaded_map,
            Some(GameMap::new("episodic", "ep1_background01"))
        );

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
        assert!(effects.contains(&Effect::MainMenuMap(GameMap::new(
            "ep2",
            "ep2_background01"
        ))));

        console.level_loaded_for_map("ep2", "ep2_background01", true);
        assert_eq!(console.main_menu.category(), "ep2");
        assert_eq!(
            console.loaded_map,
            Some(GameMap::new("ep2", "ep2_background01"))
        );

        // 3. Return to main, then enter Half-Life 2 submenu (row 0)
        console.escape();
        assert_eq!(console.main_menu.category(), "main");

        let effects = click_row(&mut console, 0);
        assert_eq!(console.main_menu.category(), "hl2");
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("background01")
        );
        assert!(effects.contains(&Effect::MainMenuMap(GameMap::new("hl2", "background01"))));

        console.level_loaded_for_map("hl2", "background01", true);
        assert_eq!(console.main_menu.category(), "hl2");
        assert_eq!(
            console.loaded_map,
            Some(GameMap::new("hl2", "background01"))
        );

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
        console.loaded_map = Some(GameMap::new("hl2", "background01"));

        // Switch to Episode One
        let _ = click_row(&mut console, 1);
        console.level_loaded_for_map("episodic", "ep1_background01", true);
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
        assert_eq!(
            console.loaded_map,
            Some(GameMap::new("episodic", "ep1_background01"))
        );
    }

    #[test]
    fn lostcoast_navigation_and_missing_content_error_recovery() {
        let mut console = installed_console();
        console.mode = Mode::MainMenu;
        console.background_level = true;
        console.loaded_map = Some(GameMap::new("hl2", "background01"));

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
                .any(|e| e == &Effect::MainMenuMap(GameMap::new("lostcoast", "background01"))),
            "entering lostcoast submenu loads Lost Coast's own background01"
        );
        assert_eq!(
            console.config.gameplay.background_map.as_deref(),
            Some("background01")
        );
        assert_eq!(
            console.config.gameplay.background_game.as_deref(),
            Some("lostcoast")
        );
        console.level_loaded_for_map("lostcoast", "background01", true);

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

        console.start_background_loading("ep2", "ep2_background01");
        assert!(console.is_loading());
        assert!(console.is_background_loading());
        assert!(!console.modal_dialog_open());

        console.draw(0.0);
        let quads = console.canvas.drain();

        // Must emit the full-canvas background backdrop overlay quad
        let backdrop_quads: Vec<_> = quads
            .iter()
            .filter(|q| {
                q.destination.w == 1920.
                    && q.destination.h == 1080.
                    && q.color
                        == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP
            })
            .collect();
        assert_eq!(
            backdrop_quads.len(),
            1,
            "expected exactly 1 backdrop overlay quad"
        );
        let overlay = backdrop_quads[0];
        assert_eq!(overlay.destination.w, 1920.);
        assert_eq!(overlay.destination.h, 1080.);

        // The backdrop overlay must be drawn BEFORE the menu items so text is sharp over the backdrop
        let backdrop_index = quads
            .iter()
            .position(|q| {
                q.destination.w == 1920.
                    && q.destination.h == 1080.
                    && q.color
                        == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP
            })
            .unwrap();
        assert_eq!(
            backdrop_index, 0,
            "backdrop overlay must be the first quad drawn behind the menu"
        );

        let scale = Painter::scale_for(1080.);
        let expected_plaque_w = (crate::gameui::loading::BACKGROUND_LOADING_SIZE.x * scale).round();
        let margin = (24.0 * scale).round();
        let expected_x = (1920.0 - expected_plaque_w - margin).max(0.0).round();

        // Plaque is drawn via shared rounded dialog frame (no outer 3D border)
        let middle_bar = quads
            .iter()
            .find(|q| {
                (q.destination.w - expected_plaque_w).abs() < 1.0
                    && (q.destination.x - expected_x).abs() < 1.0
                    && q.vertices.is_none()
            })
            .expect("bottom-right corner plaque middle bar");
        assert_eq!(middle_bar.color, Color::new(0.0, 0.0, 0.0, 0.65));

        let corner_polygons = quads
            .iter()
            .filter(|q| q.vertices.is_some() && q.destination.x >= expected_x - 1.0)
            .count();
        assert_eq!(corner_polygons, 4);
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

        // Gameplay map loading must NEVER emit a backdrop overlay quad
        let backdrop_quads: Vec<_> = quads
            .iter()
            .filter(|q| {
                q.destination.w == 1920.
                    && q.destination.h == 1080.
                    && q.color
                        == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP
            })
            .collect();
        assert!(
            backdrop_quads.is_empty(),
            "gameplay map loading must not emit any backdrop overlay quads"
        );

        // Frame and progress bar quads must be present
        let scale = Painter::scale_for(1080.);
        let expected_frame_w = (crate::gameui::loading::LOADING_DIALOG_SIZE.x * scale).round();
        let _frame_quad = quads
            .iter()
            .find(|q| (q.destination.w - expected_frame_w).abs() < 0.1)
            .expect("centered gameplay loading dialog frame");
    }

    #[test]
    fn background_loading_completion_dismisses_blur_overlay() {
        let mut console = Console::default();
        console.canvas.resize(1920., 1080.);
        console.mode = Mode::MainMenu;

        console.start_background_loading("ep2", "ep2_background01");
        assert!(console.is_background_loading());

        // Level loads
        console.level_loaded(true);
        assert!(!console.is_loading());
        assert!(!console.is_background_loading());

        console.draw(0.0);
        let quads = console.canvas.drain();
        let backdrop_quads: Vec<_> = quads
            .iter()
            .filter(|q| {
                q.destination.w == 1920.
                    && q.destination.h == 1080.
                    && q.color
                        == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP
            })
            .collect();
        assert!(
            backdrop_quads.is_empty(),
            "no backdrop overlay quads once background map loading is complete"
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

        console.start_background_loading("ep2", "ep2_background01");
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
        assert!(quads.iter().all(|q| !(q.destination.w == 1920.
            && q.destination.h == 1080.
            && q.color == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP)));
    }

    #[test]
    fn background_load_failed_ends_transition_and_cleans_up() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.canvas.resize(1920., 1080.);

        console.start_background_loading("lostcoast", "background01");
        assert!(console.is_background_loading());

        console.background_load_failed("Failed to open BSP file");
        assert!(!console.is_background_loading());
        assert_eq!(
            console.background_transition_state(),
            &crate::gameui::loading::BackgroundTransitionState::Failed {
                map: "background01".into(),
                displayed_category: "lostcoast".into(),
                reason: "Failed to open BSP file".into(),
            }
        );
        assert!(console.background_transition_state().is_terminal());
        assert!(console.background_transition_state().is_failed());

        console.draw(0.0);
        let quads = console.canvas.drain();
        assert!(
            quads.iter().all(|q| !(q.destination.w == 1920.
                && q.destination.h == 1080.
                && q.color
                    == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP)),
            "no backdrop overlay quads after background load failure"
        );
    }

    #[test]
    fn generic_loading_failed_on_background_cleans_up() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.canvas.resize(1920., 1080.);

        console.start_background_loading("episodic", "ep1_background01");
        assert!(console.is_background_loading());

        // Generic loading_failed on a background map must dismiss the dialog and mark transition failed
        console.loading_failed("Corrupt lump header");
        assert!(!console.is_background_loading());
        assert!(console.background_transition_state().is_failed());
        assert!(console.loading_dialog.is_none());

        console.draw(0.0);
        let quads = console.canvas.drain();
        assert!(quads.iter().all(|q| !(q.destination.w == 1920.
            && q.destination.h == 1080.
            && q.color == crate::gameui::loading::BackgroundTransitionState::MAIN_MENU_BACKDROP)));
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

    #[test]
    fn test_subtitle_applied_after_background_loading_finishes_or_fails() {
        let mut console = installed_console();
        let mut loc = Localization::default();
        loc.insert("GameUI_HalfLife2", "Half-Life 2");
        loc.insert("GameUI_EpisodeOne", "Episode One");
        loc.insert("GameUI_EpisodeTwo", "Episode Two");
        loc.insert("GameUI_LostCoast", "Lost Coast");
        console.category_titles = resources::fallback_category_titles(&loc);
        console.mode = Mode::MainMenu;
        console.canvas.resize(1920., 1080.);

        // Initial default title state
        assert_eq!(console.current_title(), "Half-Life 2");
        assert_eq!(
            console.background_transition_state(),
            &crate::gameui::loading::BackgroundTransitionState::Idle
        );

        // --- Sequence 1: Select Episode One -> loading (old title shown) -> finished (new title shown) ---
        // Row 1 in fallback_game_menu() is Episode One ("SwitchExpansion episodic")
        click_row(&mut console, 1);
        assert_eq!(console.main_menu.category(), "episodic");
        assert!(console.is_background_loading());
        assert!(console.background_transition_state().is_active());
        assert_eq!(
            console.background_transition_state().target_map(),
            Some("ep1_background01")
        );
        // While in Starting / Loading, the old title MUST still be shown
        assert_eq!(
            console.current_title(),
            "Half-Life 2",
            "old title must remain visible while background map is starting"
        );

        console.update_loading();
        assert!(console.is_background_loading());
        assert_eq!(
            console.current_title(),
            "Half-Life 2",
            "old title must remain visible while background map is loading"
        );

        // Finish background loading for Episode One
        console.level_loaded_for_map("episodic", "ep1_background01", true);
        assert!(!console.is_background_loading());
        assert!(console.background_transition_state().is_finished());
        // Now new title is applied!
        assert_eq!(
            console.current_title(),
            "Half-Life 2\n== episode one==",
            "new subtitle must be applied after background loading finishes"
        );

        // --- Sequence 2: Select Episode Two -> load failure (new title still applied) ---
        // Enter Episode Two ("SwitchExpansion ep2")
        console.start_background_loading("ep2", "ep2_background01");
        assert!(console.is_background_loading());
        assert!(console.background_transition_state().is_active());
        // While loading Episode Two, previously displayed title (Episode One) remains visible
        assert_eq!(
            console.current_title(),
            "Half-Life 2\n== episode one==",
            "previous subtitle must remain visible while new background map is loading"
        );

        // Fail Episode Two load
        console.background_load_failed("BSP corrupted or missing");
        assert!(!console.is_background_loading());
        assert!(console.background_transition_state().is_failed());
        // Failure must still apply the new title so it does not remain stuck on the old title
        assert_eq!(
            console.current_title(),
            "Half-Life 2\n== episode two ==",
            "new title must still be applied when background load fails"
        );

        // --- Sequence 3: Fast double switch (latest wins, stale completion ignored) ---
        // Switch to Episode One first
        console.start_background_loading("episodic", "ep1_background01");
        assert_eq!(
            console.background_transition_state().target_map(),
            Some("ep1_background01")
        );
        assert_eq!(
            console.current_title(),
            "Half-Life 2\n== episode two ==",
            "Episode Two title remains visible while Episode One starts loading"
        );

        // User fast double switches to Lost Coast before Episode One finishes
        console.start_background_loading("lostcoast", "background01");
        assert_eq!(
            console.background_transition_state().target_map(),
            Some("background01")
        );
        assert_eq!(
            console.current_title(),
            "Half-Life 2\n== episode two ==",
            "Episode Two title still visible while Lost Coast is loading"
        );

        // Stale completion arrives for Episode One
        console.level_loaded_for_map("episodic", "ep1_background01", true);
        // Stale completion must be ignored; transition must still be active for Lost Coast
        assert!(
            console.is_background_loading(),
            "stale Episode One completion must not end active Lost Coast transition"
        );
        assert_eq!(
            console.background_transition_state().target_category(),
            Some("lostcoast")
        );
        // A stale Half-Life 2 `background01` completion is not Lost Coast's `background01`.
        console.level_loaded_for_map("hl2", "background01", true);
        assert!(console.is_background_loading());
        assert_eq!(
            console.current_title(),
            "Half-Life 2\n== episode two ==",
            "stale completion must not apply Episode One title"
        );

        // Lost Coast finishes
        console.level_loaded_for_map("lostcoast", "background01", true);
        assert!(!console.is_background_loading());
        assert!(console.background_transition_state().is_finished());
        assert_eq!(
            console.current_title(),
            "Half-Life 2\nlost coasT",
            "Lost Coast subtitle must be applied (latest wins)"
        );
    }

    #[test]
    fn test_console_background_texture_resolution() {
        let mut console = Console::default();
        let texture =
            |width: u32| Texture2D::from_rgba8(width as u16, 2, &vec![255; width as usize * 2 * 4]);
        for (game, image, width) in [
            ("hl2", "background01", 2),
            ("hl2", "background01_widescreen", 4),
            ("lostcoast", "background01", 20),
            ("ep2", "ep2_background01", 2),
        ] {
            console
                .background_textures
                .insert((game.into(), image.into()), texture(width));
        }
        let width = |game, map, wide| {
            console
                .background_texture(game, map, wide)
                .map(|t| t.width())
        };
        assert_eq!(width("hl2", "background01", false), Some(2.0));
        assert_eq!(width("hl2", "background01.bsp", true), Some(4.0));
        // Lost Coast's `background01` is its own image, with no widescreen variant.
        assert_eq!(width("lostcoast", "background01", false), Some(20.0));
        assert_eq!(width("lostcoast", "background01", true), Some(20.0));
        assert_eq!(width("ep2", "ep2_background01", true), Some(2.0));
        assert_eq!(width("episodic", "ep2_background01", false), None);
    }

    #[test]
    fn con_enable_off_keeps_the_console_closed_but_lets_it_close() {
        let mut console = Console {
            mode: Mode::MainMenu,
            ..Default::default()
        };
        console.config.keyboard.developer_console = false;
        console.toggle();
        assert_eq!(console.mode, Mode::MainMenu, "con_enable 0");
        console.config.keyboard.developer_console = true;
        console.toggle();
        assert_eq!(console.mode, Mode::Console);
        console.config.keyboard.developer_console = false;
        console.toggle();
        assert_eq!(console.mode, Mode::MainMenu, "an open console still closes");
    }
}
