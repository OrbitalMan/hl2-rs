//! A bounded standalone console. Commands without a Rust implementation report an error.
mod chapter_dialog;
pub mod commands;
pub mod complete;
mod draw_menu;
mod draw_windows;
mod effect;
mod focus;
mod loading;
mod menu_flow;
mod options_dialog;
mod panel;

use crate::gameui::{
    menu::{GameMenu, MenuLayout},
    options::OptionsDialog,
    resources::{self, fallback_game_menu, GameUiResources},
    video_confirm::VideoConfirm,
};
pub use effect::{Effect, GameMap};
use glam::{vec2, Vec2};
use serde::Serialize;
use source_assets::{keyvalues, vpk::Vfs};
use source_ui::{
    canvas::{Canvas, Color, Texture2D, WHITE},
    console::panel::Style as ConsoleStyle,
    font::{FontFace, TextOutline},
    message_box::MessageBox,
    placement::Placement,
    shell::Shell,
    widgets::{scheme_color, ButtonTracker, Fonts, Painter, Theme},
    windows::WindowId,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A dialog window over the base layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Window {
    NewGame,
    Options,
    Console,
    /// A sub-dialog over a parent dialog (e.g. Video Advanced, Keyboard Advanced, Gamma, Credits).
    SubDialog,
    /// A modal confirmation / message box dialog (Quit, Return to menu, Video keep, Reset defaults).
    Confirmation,
}

impl WindowId for Window {
    const CONSOLE: Self = Window::Console;

    /// Whether this window is modal (captures focus until closed).
    fn is_modal(&self) -> bool {
        matches!(self, Window::SubDialog | Window::Confirmation)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Mode {
    #[default]
    Gameplay,
    Pause,
    Console,
    MainMenu,
    NewGame,
    Options,
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
    /// The window stack and focus (the Options dialog and the console), the console dialog,
    /// Escape held over a dialog, key repeat and the latest pointer.
    pub shell: Shell<Window>,
    pub cheats: bool,
    /// Where the quit / main-menu box was dragged (centered on opening).
    quit_placement: Placement,
    /// Names arguments complete from (installed maps, the loaded map's entities), set by the host.
    completion: complete::Sources,
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
    chapter_dialogs: BTreeMap<String, crate::gameui::new_game::Dialog>,
    chapter_game: Option<String>,

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

/// The menu colors from the scheme (the console's are the panel's).
struct Style {
    menu: Color,
    menu_armed: Color,
    panel: Color,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            menu: WHITE,
            menu_armed: Color::from_rgba(200, 200, 200, 255),
            panel: Color::from_rgba(132, 132, 132, 212),
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
            shell: Shell::new(
                0.3,
                crate::tuning::unfocused_alpha_factor(),
                crate::proportional::SCALING,
            ),
            cheats: false,
            quit_placement: Placement::default(),
            completion: complete::Sources::default(),
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
            chapter_dialogs: BTreeMap::new(),
            chapter_game: None,
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

/// Height of the F1 developer overlay's header, and of the viewmodel tweak overlay panel.
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
        console.shell.panel.font = console.mono.clone();
        console.shell.panel.placement = Placement::from_saved(console.config.dialogs.console);
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
                console.shell.windows.fade_time = time;
            }
            console.style = Style {
                menu: scheme_color(&root, "MainMenu.TextColor").unwrap_or(console.style.menu),
                menu_armed: scheme_color(&root, "MainMenu.ArmedTextColor")
                    .unwrap_or(console.style.menu_armed),
                panel: scheme_color(&root, "Frame.BgColor").unwrap_or(console.style.panel),
            };
            let style = &mut console.shell.panel.style;
            *style = ConsoleStyle {
                entry: scheme_color(&root, "RichText.BgColor").unwrap_or(style.entry),
                text: scheme_color(&root, "Console.TextColor").unwrap_or(style.text),
                dev_text: scheme_color(&root, "Console.DevTextColor").unwrap_or(style.dev_text),
                ..*style
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
                ("Console.TextColor", &mut console.shell.panel.style.text),
                (
                    "Console.DevTextColor",
                    &mut console.shell.panel.style.dev_text,
                ),
                ("RichText.BgColor", &mut console.shell.panel.style.entry),
                (
                    "RichText.SelectedBgColor",
                    &mut console.shell.panel.style.selected_bg,
                ),
                (
                    "RichText.SelectedTextColor",
                    &mut console.shell.panel.style.selected_text,
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
        self.prepare_chapters(&resources);
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
        self.shell.windows.set_time(now);
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
            if self.is_background_loading() {
                self.draw_windows(time);
                return;
            }
            let geometry = self.menu_layout.geometry_for(self.canvas_size());
            let title = self.current_title();
            let base_focus = self.shell.windows.base_focus_amount(time);
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
            self.draw_menu_buttons(shown, geometry, alpha_mult, base_focus);
        }
        self.draw_windows(time);
    }
}

/// Dialog push buttons play this when they activate.
const RELEASE_SOUND: &str = "ui/buttonclickrelease.wav";

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
#[cfg(test)]
mod viewmodel_tests;
