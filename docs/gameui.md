# GameUI: startup flow, main menu and options dialog

Status: **in progress** (started 2026-10-11). This file is the single architectural description of the
menu/options work; other documents link here instead of repeating it.

## 1. Design principles

- **Pure, testable models.** Menu and dialog state live in `hl2-ui::gameui`; keys and clicks go in, typed
  effects come out. Models are unit-tested without a window; the Bevy host only translates input and
  applies effects.
- **Installed resources drive layout and text.** Control rectangles, label tokens and visibility/enabled
  flags come from the installed VGUI `.res` files; menu entries from `GameMenu.res`; key actions from
  `kb_act.lst`. Nothing is copied into the repository.
- **Localization with a visible fallback.** Tokens resolve through a merged, case-insensitive table
  (first file wins, like Source's search path). An unresolved token renders as its key name, never as
  empty text.
- **Display modes are queried, not hard-coded.** Resolutions come from Bevy `Monitor::video_modes` and
  are filtered by aspect bucket (normal < 1.5, 16:10 ~ 1.6, otherwise 16:9); the current size is the
  fallback when no modes are reported.
- **Small widget helpers, canvas drawing.** Widget logic (combo list, slider fraction/value, focus) is
  separate from drawing; drawing uses the existing ordered CPU canvas (`hl2-ui::canvas`).
- **Typed configuration.** Choices edit a draft `Config`; OK/Apply commit and persist it, Cancel discards it.
- **Scope.** Single-player Half-Life 2 GameUI only; multiplayer dialogs are out of scope.

## 2. Baseline before this work (2026-10-11)

- `hl2-ui::console::Console` hosted Pause/Console/MainMenu/Options modes. The main menu was four
  hard-coded entries (NEW GAME, OPTIONS, DEVELOPER CONSOLE, QUIT) over a 95 % black scrim; New Game
  hard-coded `d1_trainstation_02`.
- `hl2-ui::options` drew four tabs with hand-placed rectangles; labels were partly hard-coded English
  defaults, the resolution "dropdown" cycled a static list, display mode toggled windowed/borderless,
  the keyboard list was a fixed 17-row table with no Use Defaults/Clear.
- Startup always loaded a map (`d1_trainstation_02` default); `--main-menu` only overlaid the menu.
- `Config` (JSON beside the executable) covered resolution, borderless, FOV, HDR, volume,
  sensitivity, invert and key names.

## 2a. Architecture review and adaptation decisions (milestone 2)

Deficiencies found in the baseline, each tied to a requirement:

| Area | Baseline defect | Decision |
|---|---|---|
| Main menu | Four hard-coded entries; no HL2/Episode One/Episode Two categories, spacers, Extras or Quit confirmation; New Game hard-coded `d1_trainstation_02` | Model built from `GameMenu.res` (file order out of game, `InGameOrder` in game); New Game map from `cfg/chapter1.cfg`; Quit opens a confirmation dialog |
| Pause menu | Separate hard-coded list (Resume/Options/Developer console/Quit) | Same `GameMenu.res` model with `OnlyInGame` entries; the console stays on tilde |
| Localization | Ad-hoc lookups of a few tokens in `gameui_english.txt` per module, hard-coded English defaults otherwise | One merged `Localization` table (all GameUI files, first file wins) with the key-name fallback |
| Options layout | Hand-placed rectangles in a 952-line module mixing state, input and drawing | Control rectangles/labels/enabled flags from `OptionsSub*.res`; dialog size 512x406 and title from retail `GameUI.dll`; widget state separated from drawing |
| Options content | No Gameplay/Gamepad tabs; static resolution list; windowed/borderless toggle only; FOV control not on the retail page | Retail tab set and controls; FOV stays a console/config setting |
| Keyboard page | Fixed 17-row table from a code constant; no Use Defaults, Clear or conflict handling | Rows from `scripts/kb_act.lst`, defaults from `cfg/config_default.cfg` (`bind_mac` on macOS); binding a key removes it from other actions |
| Configuration | No skill, mouse filter/raw/acceleration, aim assist, music volume, speaker/quality, language, captions, mute-on-focus-loss, display mode, aspect/HUD aspect, gamma; HDR lacks Bloom and defaults to Full (retail default None) | Extend `Config` with `#[serde(default)]` fields so existing `hl2-config.json` files keep loading; legacy `borderless` migrates to the display mode |
| Startup | A map was always loaded (default `d1_trainstation_02`); `--main-menu` only overlaid the menu; HUD drawn under the menu | No/unloadable `--map` -> main menu over the chapter background map with the HUD hidden; map-dependent arguments without a loadable map reject the launch |
| UI input | `Input` carried only navigation keys, pointer and click | Add held button, wheel and pressed binding names (slider drag, list scrolling, key capture); serde defaults keep scripted inputs valid |
| Display modes | Resolution not queried from the monitor; no exclusive full screen | Modes from Bevy `Monitor::video_modes`, filtered by aspect bucket; Full screen uses the matching video mode |

Kept as they are (working and covered by tests): the `Console` mode/`Effect` pipeline and host
effect application, the ordered CPU `Canvas`, font loading and cell metrics, `MenuLayout` geometry
from `ClientScheme.res`, the config file location, `input::binding` policy and the 15 ms system
order. (The earlier renderer-viewport resolution design, which letterboxed inside an unresized
window, was replaced by the game frame in section 10.)

Only `source-assets` change: `Vfs::game_root()` so the UI reader can read the `hl2_complete/`
overlay and list voice packs; the VFS search path itself is unchanged.

## 3. Installed resources (verified against the owned install)

| Need | File (search order: `hl2_complete/` loose, then the mounted `hl2`/`platform` VFS) |
|---|---|
| Main/category menus | `hl2_complete/resource/gamemenu.res` (`MenuCategory` main, hl2, episodic, ep2, extras, lostcoast; empty label = spacer) |
| Strings | `hl2_complete/resource/merged_loc_all.txt` (UTF-8, `lang/english/Tokens`), `resource/gameui_english.txt`, `resource/valve_english.txt`, `resource/vgui_english.txt` (UTF-16 LE) |
| Options pages | `resource/optionssub{difficulty,keyboard,mouse,gamepad,audio,video}.res`, `optionssubvideogammadlg.res`, `optionssub{audio,video}thirdpartydlg.res` |
| Key actions / defaults | `scripts/kb_act.lst`, `cfg/config_default.cfg` |
| Gamma test image | `materials/vgui/gamma.vtf` (ImagePanel `image "gamma"`) |
| Background map | Each game's own `scripts/chapterbackgrounds.txt` (chapter 1; `hl2` -> `background01`), read through that game's mount; the games are the `SwitchExpansion <folder>` commands of `gamemenu.res` (`resources::menu_games`, `read_game_text`) |
| New Game map | Each game's own `cfg/chapter1.cfg`, read the same way |
| Game of a map | Explicit where known: a submenu loads `(game, map)` (`Effect::MainMenuMap(GameMap)`), the saved background keeps `gameplay.background_game`, level transitions stay in the running game. A bare name (`map`, `map_background`, `--map`, `--background-map`, respawn, a background saved without its game) belongs to the game whose own folder ships `maps/<name>.bsp`, loose or in its own VPKs, searching `hl2` first, then the other `gameinfo.txt` folders (`source_assets::install::game_for_map`); names typed in a session (`map`, `map_background`, respawn) search the running game before `hl2`, command-line names do not. The map is then read through that game's mount (`Vfs::mount_game`), whose order is the `SearchPaths` block of the game's own `gameinfo.txt` (`vpk::search_paths`: custom folders, own VPKs, inherited VPKs, then loose folders). Loading a gameplay map saves the background of its chapter as the menu background (section 13); Return to main menu and the next launch show it. Pre-blurred `materials/console/<map>[_widescreen].vtf` images are keyed by `(game, map)`, so Half-Life 2's and Lost Coast's `background01` need no alias |
| Spoken languages | `hl2/hl2_sound_vo_<language>_dir.vpk` present in the install |

Retail slider ranges (installed `GameUI.dll` constants): `sensitivity` 0.1-6.0, `m_customaccel_exponent`
1.0-1.4, `m_autoaim_scale` 0.5-2.0, `mat_monitorgamma` 1.6-2.6, `volume`/`snd_musicvolume` 0-1; value
entries show two decimals.

Correction (2026-10-11): `#GameUI_NoWindowBorder` is defined in the UTF-16 `gameui_english.txt`
("Windowed (No Border)"); an earlier byte-level search missed it. Verified by the owned-install test
`gameui::resources::tests::owned_install_resources_load`.

## 4. Plan (milestones, each committed separately)

1. Localization + resource loading (`hl2-ui::gameui::{localize, layout, resources}`), tests.
2. Startup flow (valid `--map` -> gameplay, otherwise main menu over the chapter-1 background map),
   resource-driven main menu with categories and the Quit confirmation dialog. Arguments that only
   make sense with an immediately loaded map (`--position`, `--yaw`, `--pitch`, `--fly`,
   `--movement-script`, `--capture-live`, `--capture-burst`, `--capture-monitor`) without a loadable
   `--map` reject the launch with an error instead of opening the menu.
3. Options dialog framework (tabs Gameplay/Keyboard/Mouse/Gamepad/Audio/Video, OK/Cancel/Apply,
   draft vs applied `Config`, persistence), widgets.
4. Pages: Gameplay, Keyboard, Mouse, Gamepad (empty placeholder), Audio, Video + brightness dialog.
5. Documentation and validation records.

## 5. Implemented: startup flow and main menu (2026-10-11)

What exists:

- `hl2-ui::gameui::widgets`: scheme-derived `Theme` (`sourcescheme.res`/`clientscheme.res` colours),
  Tahoma/Tahoma Bold widget fonts, a `Painter` over the ordered CPU canvas, button states and text
  wrapping.
- `hl2-ui::gameui::menu`: `GameMenu` built from `gamemenu.res` categories (main, hl2, episodic,
  ep2), with spacers, keyboard/pointer navigation and no armed entry until hover or arrow keys;
  `MessageBox` for the "QUIT GAME" confirmation.
- Startup (`hl2-bevy::main::load_startup_map`):
  - a loadable `--map` starts gameplay (`--main-menu` turns it into a menu background instead);
  - no `--map`, or one that fails to load, loads the chapter-1 background map (`background01`)
    running behind the main menu;
  - `--position`, `--yaw`, `--pitch`, `--fly`, `--movement-script`, `--capture-live`,
    `--capture-burst` and `--capture-monitor` without a loadable `--map` exit with an error.
- New Game in the HALF-LIFE 2 category loads chapter 1 as gameplay. Escape on a background level
  closes the quit box or returns from a category, but never resumes the background into gameplay.

Placeholders (shown disabled; activating logs to the console): Load Game, Achievements, Workshop.
EXTRAS opens the Extras submenu (Lost Coast and Workshop placeholder). New Game supports Half-Life 2, Episode One, Episode Two, and Lost Coast.

Limitations:

- The background map runs live behind the menu like retail (entities, `ambient_generic` loops,
  soundscapes, effects; `Simulation::world_running`), with player input ignored as in the SDK
  (`in_main.cpp` skips input when `IsLevelMainMenuBackground()`). The view is the map's
  `point_viewcontrol` (`view_controller`, enabled by a `logic_auto`; SDK `CTriggerCamera`, in
  `hl2_simulation::view_control`): its origin, looking at its `target` or along its `angles`,
  with the player frozen (spawnflag 4). Half-Life 2 backgrounds lock the field of view to 75 with
  an `env_zoom`; the others use the player's FOV. The soundscape is still selected from the
  frozen player's ears (SDK `EarPosition()`); positional mixing listens at the camera. Not
  modelled: camera `moveto` paths, `interpolatepositiontoplayer`, eased (non-snapping) turns. Escape
  over gameplay, Options or the console over gameplay, and map loading still freeze the world.
- If a background map fails to load, the runtime logs a warning to the console, preserves the menu,
  and does not crash or interrupt menu navigation.
- Menu positions follow the retail layout, but the canvas scale is the existing HUD scale rather
  than a verified retail proportional scale.

Verified (2026-10-11, macOS Apple Silicon, release build from `scripts/build-bevy.sh`):

- `cargo test --locked -p hl2-ui -p hl2-bevy`: hl2-ui 69 passed / 2 ignored, hl2-bevy 54 passed /
  2 ignored (ignored tests need the owned install).
- `cargo clippy --locked -p hl2-ui -p hl2-bevy -p source-assets --all-targets -- -D warnings` and
  `cargo fmt --all --check`: clean.
- Packaged captures (ignored `artifacts/gameui/`): no `--map` shows background01 with the main menu
  (mode MainMenu); `--map no_such_map` reports the failure and falls back to the menu;
  `--map d1_trainstation_01` starts in gameplay; `--fly --position 0 0 0` and
  `--map no_such_map --fly` exit with status 1; scripted input with
  `--map background01 --main-menu --movement-script` captured the quit box and the HALF-LIFE 2
  category.

Not tested: Windows and Linux, mouse-only navigation in the packaged build (covered only by unit
tests), and the Episode One/Two categories beyond the model tests.

## 6. Implemented: Options dialog (2026-10-11)

Architecture:

- `hl2-ui::config`: typed settings (`HdrMode`, `DisplayMode`, `HudAspect`, `Speakers`,
  `SoundQuality`, `Captions`, `AimAssist`, `GameplaySettings`), `Config::migrate` (legacy
  `borderless` -> `display_mode`, clamps to the retail ranges). Persisted as `hl2-config.json` next
  to the executable.
- `hl2-ui::gameui::settings`: a pure `Draft` (pending `Config` plus video-page state such as the
  selected aspect bucket) and a binding table from each `.res` control name to a `Toggle`, `Value`,
  `Choice` or `Command`, with per-binding enable rules.
- `hl2-ui::gameui::options::OptionsDialog`: tabs, page layout from the installed
  `optionssub*.res` files (`hl2/resource/`), OK/Cancel/Apply, sub-dialogs (brightness, key-defaults
  confirmation, credits), key capture, and the placeholder log. Input is a plain `Input` snapshot,
  so the dialog is unit-testable without Bevy.
- `hl2-ui::gameui::widgets`: Painter widgets (check box, radio, slider, text entry, combo with drop
  list, menu list with scroll bar, divider, URL label, tab, image) using the scheme colours.
- Host (`hl2-bevy`): mouse wheel and held-button state, Source key names for capture
  (`MOUSE1`-`MOUSE5`, `MWHEELUP`/`MWHEELDOWN`), display modes from `Monitor::video_modes`
  (`video::display::modes`), and window mode/resolution applied on OK/Apply.

Semantics:

- OK applies and closes, Apply applies and stays open, Cancel/Escape discards the draft. Escape
  closes the innermost layer first (open combo, key capture, sub-dialog, then the dialog).
- Every label is resolved through the game localization; unresolved tokens show their key name.
- Video: resolutions come from the platform display modes, grouped by aspect bucket; changing the
  aspect ratio selects a valid resolution of that bucket. Nothing changes before OK/Apply.
- Display mode offers Fullscreen / Windowed / Windowed (No Border) on every platform (owner
  decision, 2026-10-17; the retail macOS build hides No Border). On macOS No Border is the native
  full-screen Space and Fullscreen is exclusive (section 10).
- Brightness ("Adjust brightness levels...") is enabled in every display mode (2026-10-15, owner
  decision; retail enables it in full screen only because its hardware gamma ramp needs full
  screen). The slider previews live, as in retail (`OptionsDialog::preview_gamma`,
  `Console::display_gamma`); Cancel restores the previous value, OK keeps it in the draft for
  OK/Apply. `mat_monitorgamma` is applied by the presentation shader
  (`video::PresentMaterial`, `assets/shaders/present.wgsl`) over the whole composed picture
  (3D view, HUD and menus all render into the game frame): each display-encoded value v becomes
  v^(gamma / 2.2) (`video::monitor_gamma_exponent`), so 2.2 is neutral and the slider's "Light"
  end (1.6) brightens. The exact retail ramp formula is not verified; the direction follows the
  installed dialog's `#GameUI_LIGHT`/`#GameUI_DARK` slider labels.
- Keyboard: rows and sections come from `hl2/scripts/kb_act.lst`, defaults from
  `hl2/cfg/config_default.cfg`. Edit Key, Enter on a selected row, or a double click on a row
  (SDK `VControlsListPanel::OnMouseDoublePressed` starts capture) captures the next key or mouse
  button; double clicks use `widgets::DoubleClick` with `DOUBLE_CLICK_SECONDS = 0.5` (a common
  desktop default, the retail interval is the OS setting) and the frame time in `Input::time`
  (scripted input without times never double-clicks). Escape cancels capture, assigning a key removes it from any other action,
  Clear Key unbinds, Use Defaults asks for confirmation and resets only the listed actions.
  Each section header row ends in a one-pixel `SectionedListPanel.DividerColor` (Black) line, two
  pixels above the row bottom, like SDK `SectionedListPanelHeader`. While a key is being captured
  only the key cell is filled with the selection color (retail edits the key column in place via
  its inline edit panel; the retail colors of that panel are not verified, orange is the owner's
  choice); the action cell keeps a dimmer selection
  (`SectionedListPanel.OutOfFocusSelectedBgColor` `255 155 0 128` with
  `OutOfFocusSelectedTextColor`) so the row stays identifiable (owner preference, 2026-10-15),
  faded further to half that alpha with the normal text color (`CAPTURE_ACTION_FADE`, owner
  tuning) so the key cell reads brighter.
- Mouse: on macOS Raw input and Acceleration are disabled, per the `[$OSX]` conditions in the
  installed `.res` file.
- High Dynamic Range is shown on macOS too, although the retail `.res` hides it there.
- Placeholders are shown disabled and log `"<label>: not implemented in HL2-RS yet."`: the Gamepad
  page's Open Steam Input Configuration (no Steam overlay).
- Gamepad (`optionssubgamepad.res`): Joystick Sensitivity (`sc_sensitivity_scale`), Aim-Assist
  (`sc_autoaim_mode`, Off/Basic/Enhanced), Aim-Assist Sensitivity (`sc_autoaim_scale`, enabled only
  while gamepad aim assist is on; both sliders 0.5-2 like the mouse aim-assist slider) and Weapon
  Selection Style (`hud_weaponui_gamepad`: Cross Layout = 2, Carousel Layout = 3; the retail page
  shows Cross for an unset value). Stored in `gamepad`, separate from the Mouse page's aim assist;
  no effect yet (no gamepad input). Labels follow the enabled state of their `associate` control
  (SDK `Label`), on every page.
- Video > Advanced (`optionssubvideoadvanceddlg.res`, title `#GameUI_VideoAdvanced_Title`, frame
  record 482 x 510, 358 on macOS): combos with the retail item lists (Model/Shadow detail
  Low/Medium/High; Texture detail adds Very High; Shader detail Low/High/Very High; Water detail
  Simple reflections/Reflect world/Reflect all; Antialiasing None/2x/4x/8x MSAA; Filtering
  Bilinear/Trilinear/Anisotropic 2X-16X; Color Correction, Wait for vertical sync, Motion Blur,
  Multicore Rendering, Windows Aero extensions Disabled/Enabled; Classic Effects Off/On) stored in
  `video.advanced`, plus the Field of view slider (`fov_desired`, range from the layout's
  `minvalue`/`maxvalue` 75-110) with its value label. Only V-sync and the field of view change
  rendering (`video::sync_present_mode`; `--uncapped` still wins); the detail levels are stored
  only. Retail lists only the antialiasing modes the hardware supports; here the list is fixed.
  The DirectX level rows are hidden on macOS/Linux by the layout (`[$POSIX]`); on Windows the
  disabled DirectX level combo stays empty.
  Its macOS (`[$OSX]`) footer positions predate the Classic
  Effects row and would put OK/Cancel on that combo, so a sub-dialog footer that overlaps content
  moves below it (`FOOTER_GAP` 8) and the frame grows by the same amount (`footer_reflow`; footer =
  OK/Close buttons plus an unbound caption at most two button heights above them). The FOV
  slider's "0.00"/"1.00" end captions are the layout's own `leftText`/`rightText`, kept as authored.
- Check box and radio button labels: `Theme::toggle_text` (scheme `Label.TextColor`, OffWhite 221,
  at rest; `Label.TextBrightColor` White while hovered or pressed; disabled color when disabled).
  Owner request (2026-10-16): the former rest color, `Label.TextDullColor`, was too dim on the
  opaque stacked dialogs.
- Stacked layers: every dialog drawn over the Options dialog (brightness, credits, Keyboard and
  Video Advanced, the confirmation box) uses `Theme::stacked` (opaque `frame_bg`), so the dialog
  below never shows through; the Options dialog itself, over the game scene, keeps the scheme's
  translucent `Frame.BgColor`.
- Tabs: the active tab adds no fill of its own and the page border is open under it
  (`Painter::raised_open_top`), so it shares the page's surface; inactive tabs keep their fill.
- Keyboard > Advanced (`optionssubkeyboardadvanceddlg.res`, title
  `#GameUI_KeyboardAdvanced_Title`): Fast weapon switch (`hud_fastswitch`, config
  `keyboard.fast_switch`, read by weapon selection every frame) and Enable developer console
  (`con_enable`, `keyboard.developer_console`, default on; while off the console key and
  `toggleconsole` do not open the console, an open console still closes). The layout has no frame
  record, so the frame is the controls' extent with the left margin repeated (284 x 144).
- Sub-dialogs (brightness, credits, advanced): edits go to the draft and Cancel, Escape or the
  title-bar close restore the draft as it was when the sub-dialog opened. OK of an advanced dialog
  applies that dialog's settings at once, as retail OK writes its cvars, and leaves other
  unapplied edits in the draft. Buttons without an explicit binding bind by their layout
  `Command` (`OK`, `Close`); `frame_*` controls (caption, close, grips) are frame chrome and are
  not drawn as widgets.

### 6a. Widget interaction states (2026-10-15)

Evidence: SDK `vgui_controls` (`Button.cpp`, `PropertySheet.cpp` `PageTab`) and the installed
`hl2_complete/resource/sourcescheme.res`.

- Push buttons resolve their look in one place, `Theme::button_style(ButtonState)`, used by
  `Painter::button` for every dialog and message box button. Precedence follows SDK `Button`:
  colors are depressed > armed (hover) > normal, the border is depressed > key focus (the default
  button) > raised. Disabled buttons are never armed or depressed. Colors come from
  `Button.{,Armed,Depressed}{Text,Bg}Color`.
- **Retail has no visible push-button hover on the desktop.** The scheme sets
  `Button.ArmedTextColor` = `Button.TextColor` = White and `Button.ArmedBgColor` = Blank
  `[$WIN32]` (`"190 115 0 255"` is `[$X360]` only), and SDK `Button::GetBorder` ignores the armed
  flag for bordered buttons.
- **Deliberate deviation (owner decision, 2026-10-15): a slight visible hover.** When the scheme's
  armed background equals the normal one (the desktop case), a hovered push button is filled with
  the scheme's `Menu.BgColor` (`160 160 160 64`), the translucent fill of menus and drop-down
  lists, which lightens it slightly. Text and border are unchanged; pressed and disabled buttons
  are unaffected; a scheme with its own `Button.ArmedBgColor` keeps it. The color is a scheme
  value, but using it for button hover is an HL2-RS choice, not retail behavior.
- **Tabs.** Retail tabs have no hover: SDK `PageTab` overrides cursor enter/exit without arming
  and takes its border and text color from the active flag only. Owner decision (2026-10-15):
  inactive, enabled tabs get the same `Menu.BgColor` hover fill as push buttons
  (`Theme::tab_style` -> `TabStyle::hover_bg`); the active tab never changes and pressing adds
  nothing.
- **Drop-down lists are fully opaque** (owner decision, 2026-10-15): `Painter::menu_list` lays the
  frame color down at full alpha before the translucent `Menu.BgColor` tint (retail
  `Frame.BgColor` is `132 132 132 212`).
- **Drag-off.** SDK buttons capture the mouse on press and do not disarm while selected, so a
  pressed control stays pressed while the pointer is dragged off and back, and the release
  activates it only while the pointer is over it. `ButtonTracker::capturing` implements this for
  the Options dialog and message boxes; while captured, no other control can arm
  (`ButtonTracker::armable`). Main and pause menu items keep `ButtonTracker::new` (press cancelled
  on drag-off), since SDK `MenuItem` disables mouse capture. Not observed in the retail binary.
- **Focus.** There is no keyboard focus state for dialog controls yet (no Tab-key focus
  traversal), so no focus border is drawn; the default button keeps its selection border.
- **Close button (owner decision, 2026-10-15):** the title bar × has no 3D border or fill. The
  glyph uses `FrameTitleButton.FgColor` (`200 200 200 196`), lightens to `FrameTitleBar.TextColor`
  (White) on hover and press, shifts 1px while pressed, and uses `FrameTitleButton.DisabledFgColor`
  when disabled (`Theme::close_style`). Retail draws a bordered title button; this is a deviation.
- Hover never plays the rollover sound for these controls (`WidgetKind::has_rollover_sound`).
- Retail `PageTab` selects on press; HL2-RS tabs still select on release (unchanged).
- Known gap: `hl2_complete/resource/SourceSchemeBase.res` is a 0-byte file in the
  20th-anniversary layout, and `Console::new` reads that name first, so the `Theme` currently runs
  on its defaults (which mirror the desktop scheme values). Reading `SourceScheme.res` instead also
  changes main menu metrics read from the same file, so it is left for a separate change.

### 6c. Platform-specific list scrolling (2026-10-15)

- The host turns mouse wheel events into `Input::wheel` (line units as notches, pixel units / 40).
  Lists (key list, popup lists) turn that into whole rows through `widgets::ScrollAccumulator`
  with `widgets::ScrollMode::platform()`:
  - `Notched` (Windows, Linux): each frame is rounded to whole notches; a notch scrolls 3 rows in
    the key list (`WHEEL_ROWS`) and 1 row in popup lists. This is the previous behavior.
  - `Smooth` (macOS, owner decision): trackpad deltas accumulate across frames, one row per whole
    step and the remainder kept (dropped on a direction change), so slow two-finger scrolling no
    longer gets lost. Lists still move row by row; pixel-smooth scrolling would need clipping
    support in the canvas and was not requested.
- Gameplay weapon selection and `MWHEELUP`/`MWHEELDOWN` key capture are unchanged.
- **Scroll bars (2026-10-15)** follow SDK `ScrollBar`/`ScrollBarSlider` and the installed scheme:
  width `ScrollBar.Wide` (17, `SCROLLBAR_WIDTH`); square arrow buttons at both ends
  (`ScrollBarButtonBorder`, sunken while pressed, `ScrollBarButton.FgColor` arrows) that step one
  row on press and repeat after 400 ms every 50 ms while held (needs `Input::time`); the nob
  (`ScrollBarSlider.FgColor`, Blank, so only its raised border shows; at least as tall as the bar
  is wide) follows the pointer live while dragged; a press on the track pages once by the
  visible rows. The bar owns the pointer from press to release (`widgets::ScrollBarState`,
  `ScrollBarLayout`), so a scroll gesture never selects a row, closes a popup or activates
  anything underneath. Used by the Options key list (`OptionsDialog::key_list_bar`) and every
  popup list (`PopupList::bar_layout`). Before, the bar had no arrows and the key list only
  jumped on release.
- **Press-and-hold repeat (2026-10-17)**: one timer, `widgets::HoldRepeat` (press, held,
  release; first repeat after `HOLD_REPEAT_DELAY` 400 ms, then every `HOLD_REPEAT_INTERVAL`
  50 ms, at most once a frame, never without `Input::time`), drives both the scroll bar arrows
  and the keyboard. Keyboard repeat is applied once, at the top of `Console::input`
  (`console::KeyRepeat` over `Input::held_keys`), so every layer that reads `Input::keys` repeats
  without code of its own: console editing, cursor, completion and history keys, main and pause
  menu Up/Down, the Options key list, open popup lists and Options text fields (Backspace). Like an operating system, the latest pressed key
  repeats; any other press takes over, and a key that acts once stops it. Up, Down, Left, Right,
  Backspace, Delete, PageUp and PageDown repeat; Enter, Tab, Home and End do not. The repeat is
  cancelled while the Keyboard page captures a key and whenever the UI mode changes. Typed
  characters keep the system's own repeat (Bevy `KeyboardInput`), and binding names
  (`Input::binding_keys`) never repeat. Deviation: retail keyboard repeat follows the operating
  system's delay and rate; HL2-RS uses the scroll bar timing for both. Scripted input has no
  held keys or times, so replays never repeat.

### 6b. Popup lists as the topmost input layer (2026-10-15)

- Root cause of clicks reaching controls behind an open combo list: the list closed (or selected)
  on the press and returned, and the dialog's `ButtonTracker`, which had not been updated while
  the list owned input, read the next held frame as a new press on whatever lay underneath, so
  the release activated it. The same path reopened a list closed by a click on its own header.
  Any modal layer that closes on a press had the same problem (for example the failed loading
  dialog over the main menu).
- Fix, in the existing layer routing rather than a second layer system: `OptionsDialog::input`
  already routes input innermost first (confirmation box, key capture, open combo list, then the
  page); the layers above the page now call `OptionsDialog::occlude`, and `Console::input` does
  the same for the menus while `modal_dialog_open()`. Occluding clears hover, press and tooltip
  and calls `ButtonTracker::occlude(down)`, which keeps the tracker on the physical button state.
  `ButtonTracker::reset` no longer forgets that state either.
- `widgets::PopupList` is the shared open-list state for any popup list: geometry, scrolling,
  keyboard stepping, hit testing (`PopupHit::{Item, List, Anchor, Outside}`) and the pointer
  gesture (`PopupEvent::{Select, Close}`). While a list is open nothing else in the dialog is
  hovered, armed, pressed, activated or given a tooltip, inside or outside the list area.
- Selection happens on release over an entry after a press in the list (SDK `MenuItem` activates
  on release). Clicking outside closes the list on release and activates nothing; this is the
  requested behavior. The SDK closes the menu when it loses focus on the press and does not
  capture the mouse, so retail may also deliver that click to the control underneath (not
  verified in the retail binary).
- Open/close state (`OptionsDialog::combo`: `None` closed, `Some(PopupList)` open): a click on a
  closed combo opens it on release over the combo; while open, a press and release on its own
  header (`PopupHit::Anchor`) closes it without changing the selection. Header, entry and outside
  closes all act on the release, and the page below stays occluded through the whole gesture, so
  one header click can never close the list and reopen it. Deviation: SDK `ComboBox` toggles on
  the press (`OnMousePressed` -> `DoClick` -> `HideMenu`); release-phase toggling is the
  requested behavior. Test: `dropdown_open_state_toggles_on_header_and_closes_in_the_release_phase`.

Limitations (as of milestone 5; section 8 supersedes the first four items):

- Stored but not yet used by the host: HDR mode, gamma, mouse filter/acceleration/invert, skill,
  music volume, speaker setup, sound quality, captions, spoken language and mute when unfocused.
- The host still reads attacks directly from the mouse buttons and only parses keyboard names for
  other actions; `MOUSE*`/`MWHEEL*` bindings are stored but not consumed yet.
- Display mode and resolution from the config are applied on OK/Apply; startup still uses the
  command line.
- Configs saved before `+attack`/`+attack2` were added to the defaults do not gain those entries
  (cleared bindings are stored as missing keys, so they are not refilled); Use Defaults restores
  them.
- Dialog scale is `max(1, height / 720)`, not a verified retail proportional scale.

Verified (2026-10-11, macOS Apple Silicon, release build from `scripts/build-bevy.sh`):

- `cargo test --locked -p hl2-ui -p hl2-bevy`: hl2-ui 83 passed / 3 ignored, hl2-bevy 56 passed /
  2 ignored.
- `cargo test --locked -p hl2-ui -- --ignored` with the owned install (discovered as at runtime): the
  resource test, the HUD crosshair test and
  `gameui::options::tests::owned_install_pages_bind_every_expected_control` pass (every expected
  control on each installed page is found and bound).
- `cargo clippy --locked -p hl2-ui -p hl2-bevy -p source-assets --all-targets -- -D warnings` and
  `cargo fmt --all --check`: clean.
- Packaged captures with scripted input (`--map background01 --main-menu --movement-script ...`,
  ignored `artifacts/gameui/options-*.png`): Gameplay, Keyboard (sections, MOUSE1/MOUSE2 attack
  rows), Mouse, Audio and Video (open display-mode list showing Full screen / Windowed on macOS).
  The brightness dialog was not captured because the windowed capture disables its button; it is
  covered by unit tests only.

Not tested: Windows and Linux (including the No Border choice and borderless switching), real
monitor mode switching to fullscreen, gamepads, and key capture in the packaged build (unit tests
only).

### 6c. Keyboard focus in dialogs (2026-10-17)

Every dialog control can be reached and used from the keyboard. The control behaviour follows the
SDK `vgui_controls` (`Button`, `RadioButton`, `Slider`, `ComboBox`, `PropertySheet`,
`MessageBox`); the order, the focus look and Enter follow owner decisions (2026-10-17):

- **Order.** Tab / Shift+Tab step through the layer and wrap. In the Options dialog: the active
  tab, the page's controls, then OK, Cancel and Apply (while enabled). Page and sub-dialog
  controls go in reading order, left to right in rows from top to bottom (controls whose tops are
  within 12 layout units share a row; `options::control_order`). The installed layouts'
  `tabPosition` values are parsed (`layout::Control::tab_position`) but not used: they jump
  around the Audio, Video and Gamepad pages, share positions and leave the Mouse sensitivity
  slider out. A radio group is one stop, its selected button (SDK `RadioButton`). Disabled
  controls are skipped. Installed pages (owned test `owned_install_pages_tab_in_reading_order`):
  Mouse = Reverse, Filter, sensitivity slider, its text field, Aim-Assist; Audio = SFX, Music,
  Speakers, Quality, Language, Captions, mute, credits; Video = Resolution, Display mode, Aspect
  Ratio, Brightness, HUD Aspect Ratio, Advanced, HDR, credits; Gamepad top to bottom.
- **Focus.** Nothing has focus when a dialog opens. A press on a control focuses it (SDK
  `OnMousePressed` → `RequestFocus`) without drawing the focus; the focus look appears after
  Tab / Shift+Tab and stays while keys move focus (arrows on tabs and radio groups) until the
  next click. A sub-dialog starts unfocused and gives the page's focus back when it closes; a
  finished or cancelled key capture gives focus to the key list. The message box focuses OK when
  it opens and draws it after Tab.
- **Keys.** Space or Enter presses the focused push button, check box, radio button or link
  (drawn depressed; activation on release, `widgets::KeyPress`; a one-frame scripted key activates
  at once). Arrows: a radio group moves its selection (wrapping) and its focus; a slider steps by
  one unit (Left/Down, Right/Up) or one tick (Page Down/Up); a closed combo box selects the
  previous / next item (Home / End: first / last) without opening, Alt+Up/Down opens its list and
  Enter is taken by it; the focused tab switches pages with Left / Right (no wrap); the key list
  moves its selected row and Enter edits the key, only while it has focus. A focused text entry
  edits at once; Tab commits it. Enter acts on the focused control only: there is no default
  button (retail VGUI presses the dialog's default button, OK, on Enter and draws it with the
  key-focus border; removed by owner request), so Enter on a tab, a slider or nothing does
  nothing. The retail slider classes (`CCvarSlider`) are not in the SDK: a unit is a hundredth of
  the range and a tick a tenth (unverified). The developer console keeps Tab for completion and
  is not part of this.
- **Look.** A focused push button, link or tab draws the SDK `Button::DrawFocusBorder` dashed
  1-pixel box (3, 3)-(wide - 4, tall - 2) in `Button.FocusBorderColor` (Black). Check boxes and
  radio buttons draw a dashed ring around the whole control, the box or circle and the label,
  `widgets::FOCUS_PAD` (2) away on every side, with the box's vertical margin also beside the box
  and after the label so the ring sits evenly around the box (`widgets::toggle_content`);
  sliders around what they draw, nob top to caption bottom (`widgets::slider_content`; the
  Video > Advanced FOV slider's layout rect starts above the bottom of its value label, so a ring
  on the rect crossed that text), text entries and the key list around their frame,
  with the same padding (`Painter::focus_ring`). A focused combo box shows its text selected
  (`TextEntry.SelectedBgColor` / `SelectedTextColor`); a text entry being edited draws
  `TextEntry.FocusEdgeColor` inside its border. The key list's selected row uses the scheme's
  out-of-focus selection (`SectionedListPanel.OutOfFocusSelectedBgColor`) while the list has no
  focus. No push button draws the default-button border any more.
- **Slider spacing.** The slider track ends a nob width short of the control's right edge, as in
  SDK `Slider::GetTrackRect` (`w = wide - nobSize`), so the Mouse sensitivity slider keeps clear
  of the text field right after it. Its end captions (Low / High) span the drawn slider, so the
  right caption ends where the slider does (it ran a nob width past it).
- **3rd party credits.** Close, the dialog's only control, holds the focus when the credits open
  (drawn after Tab); Space / Enter press it and the release closes the credits, through the shared
  `KeyPress`. It no longer draws the default-button border. Link text (the credits links on the
  Audio and Video pages) starts `widgets::LINK_TEXT_INSET` (5) into its control, inside the focus
  box (3 in) by `FOCUS_PAD`; at the control's edge the box crossed its first letter.
- **Escape.** When Escape would close a dialog with a close button (Options or one of its
  sub-dialogs, the developer console), the close button shows pressed while Escape is held and
  the dialog closes on release (`Console::escape_pressed` / `escape_released`; retail closes on
  the press). Escape that cancels something else (a drag, key capture, open list, text edit,
  completion list) or opens / leaves the pause menu acts at once.
- **Menus.** In the main and pause menus the pointer selects a row only when it enters it, so a
  pointer resting on a row no longer takes the selection back from the arrow keys.
- **Tests.** Unit tests for the reading order, a full Tab cycle driving every control kind, tab
  and radio arrows, a held Space, sub-dialog focus return, Enter without a default button, the
  key list's focus and dim selection, focus rings, the held Escape, the resting menu pointer and
  the message box; the owned-install order test. Packaged keyboard-only replays
  `test-inputs/bevy-options-capture-keyboard-*.json` (combo, button, check, radio, slider,
  entry), captured by `scripts/checks/replays.py` (`options`).

## 7. Plan: host use of the stored settings (milestone 6)

Evidence gathered before coding (the Source SDK, the installed files and the owned retail build):

- Music volume: the retail engine scales a channel's gain by `snd_musicvolume` only when the wave
  name's leading sound characters contain `#` (`CHAR_DRYMIX`, `public/soundchars.h`) and the
  channel has no distance attenuation (soundlevel 0). The installed `level_sounds_music.txt`
  scripts use `*#music/...` waves with `soundlevel 0`. The engine also applies a smoothed music
  factor (ducking) that is not modeled here.
- Mouse (SDK `game/client/in_mouse.cpp`): `m_filter` averages the current and previous raw delta;
  acceleration mode 3 multiplies by `(dx² + dy²)^((exponent - 1) / 2) * sensitivity`. The retail
  Options page writes `m_customaccel 3` when Mouse acceleration is checked and `0` otherwise, and
  the exponent slider writes `m_customaccel_exponent`.
- Skill (SDK `CGameRules::Think`): the skill level is re-read every frame, so a change takes effect
  immediately. In HL2-RS only the player damage take scale (`sk_dmg_take_scale<n>`) depends on it.
- Key bindings: retail Use Defaults is `unbindall` plus the default config, so actions without a
  default are unbound; HL2-RS stores an unbound action as an empty string so that a missing entry
  can still mean "use the default" (older config files gain new default actions on load).

Steps (one commit):

1. Bindings: `""` = unbound; Clear Key, key reassignment and Use Defaults write it; `migrate`
   fills missing default actions whose key is free. The host parses every stored name (keyboard,
   `MOUSE1`-`MOUSE5`, `MWHEELUP`/`MWHEELDOWN`) and drives `+attack`/`+attack2` and
   `invprev`/`invnext` through bindings.
2. Mouse filter and acceleration as pure, unit-tested functions; Bevy mouse motion is already raw,
   so Raw input has no separate effect.
3. Skill applied every gameplay tick.
4. Audio: per-frame sink volume = base x distance gain x master volume x (music volume for music
   channels) x (0 when unfocused and "mute when unfocused" is set).
5. Window: startup uses the stored display mode and resolution; `--windowed`, `--fullscreen` and
   `--borderless` override it; automated runs (capture, frames, movement script) default to
   Windowed unless a mode flag is given.
6. Gamma, HDR mode, speaker setup, sound quality, captions and spoken language stay stored only;
   documented as limitations.

## 8. Implemented: host use of the stored settings (2026-10-11)

- Bindings (`hl2-bevy::input`): every gameplay action is read through `Config::keys`. Stored names
  are Bevy `KeyCode` names (letters, digits, F1-F24, every name the Options page can store, the
  older `Ctrl`/`Shift`/`Alt`/arrow spellings), `MOUSE1`-`MOUSE5` and `MWHEELUP`/`MWHEELDOWN`.
  `+attack`/`+attack2` and `invprev`/`invnext` now go through bindings; `""` (Clear Key, or a key
  taken by another action) is unbound and an unknown name is treated as unbound. Without a UI
  configuration the defaults from `Config::default` apply. A wheel step is a single-frame press.
- Default bindings: `invprev` = MWHEELUP and `invnext` = MWHEELDOWN. On macOS Duck = Alt and
  Walk = Cmd (the retail `bind_mac` lines in `config_default.cfg`); elsewhere Ctrl and Alt.
  `Config::migrate` adds default actions missing from older files when their key is free; stored
  bindings, including unbound ones, are never rewritten.
- Mouse: `hl2_ui::input::MouseFilter` applies `m_filter` averaging, then sensitivity or the
  acceleration curve; the host rotates by 0.022 degrees per scaled count (`m_yaw`/`m_pitch`). This
  matches the previous fixed scale at the default settings. Invert is unchanged.
- Skill: `Gameplay::set_skill` is called from the host every frame and re-evaluates
  `sk_dmg_take_scale<n>` from the installed `cfg/skill.cfg` (ConVar defaults 0.5 / 1 / 1.5).
- Audio: `Library::resolve_wave` reports whether the selected wave has `#` among its leading sound
  characters; a channel is music when it has `#` and soundlevel 0 (its own soundlevel when
  positioned, the script soundlevel otherwise). Every frame each sink is set to base volume x
  distance gain x master volume x (music volume for music channels) x (0 when "mute when
  unfocused" is set and the window is unfocused). Master volume changes now also reach sounds that
  were already playing.
- Window: startup uses the stored display mode (`video::mode::startup_mode`, primary monitor) and
  resolution. `--windowed`, `--fullscreen` and `--borderless` override it; runs with `--frames`,
  `--capture`, `--capture-monitor` or `--movement-script` start windowed unless a mode flag is
  given.
  `launch-bevy-1080p.{cmd,sh}` pass `--windowed` so they still open a 1920x1080 window.

Limitations:

- Stored only, no effect yet: HDR mode, speaker setup, sound quality, spoken
  language, aim assist. (Gamma is applied since 2026-10-15, see section 6; closed captions
  since 2026-10-17: none = `closecaption 0`, subtitles and sound effects = `closecaption 1`,
  subtitles = `closecaption 1, cc_subtitles 1`, drawn by `hl2_ui::closecaption`, see
  `docs/DESIGN.md` "Closed captions".)
- Raw input has no separate effect: Bevy's accumulated mouse motion is already the raw device delta.
- The retail engine's smoothed music factor (ducking) is not modeled; only `snd_musicvolume`.
- The game pauses all sounds while it is paused or unfocused, so mute-when-unfocused is mostly
  inaudible today; it matters for sounds that keep playing (none at present).
- Full screen keeps the desktop video mode; the game frame is rendered at the configured
  resolution and scaled to it (section 10). Real monitor mode switching is not implemented.
- `--borderless` on macOS gives the native full-screen Space (Windowed (No Border)).

Verified (2026-10-11, macOS Apple Silicon):

- `cargo test --locked -p hl2-ui -p hl2-bevy -p hl2-simulation -p source-assets`: hl2-ui 85 passed /
  3 ignored, hl2-bevy 61 passed / 2 ignored, hl2-simulation 169 passed / 14 ignored, source-assets
  90 passed / 17 ignored. New tests: binding parsing for every storable name and every default,
  configured/mouse/wheel/unbound bindings, the mouse filter and acceleration, migration of missing
  defaults, skill rescaling, dry-mix music detection and the startup display mode.
- `cargo test --locked -p hl2-ui -- --ignored`: 3 owned-install tests pass.
- Strict Clippy for hl2-ui, hl2-bevy, source-assets and hl2-simulation; `cargo fmt --all --check`.
- Packaged release (`scripts/build-bevy.sh`): the Keyboard page capture with a config saved before
  the attack defaults now shows MOUSE1/MOUSE2 (filled by migration) while the stored Duck = CTRL is
  kept; `--map d1_trainstation_01 --frames 120 --capture ...` starts windowed and runs.

Not tested: live keyboard/mouse play in the packaged build (movement scripts bypass device input,
so the binding path is covered by unit tests only), hearing the music volume and focus mute,
startup in full screen or No Border, Windows and Linux.

## 9. Unfinished work (paused 2026-10-11)

The ordered list of remaining GameUI work (gamma, HDR modes, captions,
audio setup/language, gamepad, placeholders, untested platforms) is kept in
[STATUS.md](../STATUS.md) under "Current session: GameUI".

## 10. Resolution, display mode and scaling (2026-10-11)

Problem: the configured resolution only letterboxed a camera viewport. In windowed mode the window
kept its size, and in full screen the 3D view rendered at the display's native resolution.

Design (`crates/hl2-bevy/src/video.rs`):

- **Game frame.** The configured resolution is the size of one offscreen render target
  (`GameFrame`, `Rgba8UnormSrgb`, linear sampler). The sky background (layer 3 / order -2),
  miniature sky (layer 4 / order -1), world (layer 0), viewmodel and HUD/menu cameras all render
  into it. `present_frame` routes every camera except `MonitorCamera` (`_rt_Camera`) and
  `PresentCamera` into the frame each frame, so map-owned cameras spawned on a level change are
  covered too. It also clears camera viewports. The image is resized by `suspend_for_resize`
  while rendering is stopped (section 10a). Layer order, clears and per-camera post-processing are
  unchanged; only their target changed.
- **Presentation.** `PresentCamera` (a 2D camera, order 100, black clear, layer 31) draws one sprite
  showing the frame. The sprite is scaled to fit the window and centred (letterbox or pillarbox when
  the aspects differ), and the frame is sampled with linear filtering. Full screen at 1280x720 on a
  3456x2234 Retina panel renders 1280x720 pixels and upscales them. A window put in macOS full
  screen with the green title-bar button keeps rendering at the configured resolution and is scaled
  the same way.
- **HUD/menu and pointer.** The canvas is laid out in frame pixels (`ViewportBounds::resize_canvas`).
  `ViewportBounds::cursor_to_canvas` maps a logical window cursor position through the presented
  rect into frame pixels and rejects positions on the black bars.
- **Windowed resize.** Only an OK/Apply (or console command) that changes the resolution or
  display mode touches the window (2026-10-15): `Simulation::applied_window` holds what the window
  was last given (startup values after command-line overrides, then each queued apply) and an
  `ApplyConfig` is compared against it, not against the console's configuration, which already
  holds the new values when the effect arrives (that comparison broke every resolution and mode
  change in 5384402); applying
  mouse, audio, FOV or gamma settings never resizes, re-centres or re-modes it, so a manually
  resized window keeps its size. Such an OK/Apply in windowed mode stores the resolution in `GameFrame` and starts a
  bounded resize (`resize_window`, up to 120 frames). The window's client area is set to the
  resolution in physical pixels (`hl2_ui::viewport::windowed_size` against the monitor's pixel
  size), so the frame is shown 1:1 without scaling, also on high-DPI displays: on a 2x Retina panel
  a 2560x1600 window is 1280x800 points. (Updated 2026-10-12 at the owner's request; it was first
  sized in logical points, which upscaled the frame 2x and made every resolution above the
  desktop's point size unusable in a window.) A size that does not fit the monitor is shrunk
  uniformly to fit, with room left for the title, menu and task bars (scaled to pixels). The
  window is centred on the monitor (Windows, Linux). On macOS (2026-10-17) the room and the
  centre come from the screen's visible frame instead (`video::DesktopArea`, read by the
  macOS-only `read_desktop_area` from `NSScreen.visibleFrame` and the window's title bar height
  through the window's AppKit handle): the window fits between the menu bar and the Dock and is
  centred in that area, so a large window is never covered by the Dock. The position is set once
  the new size has been applied (`GameFrame::recenter`, also at a windowed startup): winit turns a
  position into AppKit coordinates with the window's current height and Bevy applies a position
  before a size in the same frame, so a combined request lands off by the height change. Verified
  in packaged runs on an M1 Pro (visible area 33..1054 pt, title bar 32 pt): 2560x1600 at
  (224, 128) pt with its bottom at 960; 1024x768 at (608, 336); 3456x2160 scaled to 1582x1021 pt
  filling 33..1054 exactly. The request is repeated while a mode transition (leaving full screen)
  finishes, and dropped once the size matches (within one point of OS rounding) or the window
  leaves windowed mode. While the window fills the display in OS full screen it is left alone, and
  the resize resumes when it returns to a normal window. Startup in windowed mode uses the same
  path, so a stored resolution larger than the monitor is clamped. Manual resizes are never
  fought: they just change the presented scale.
- **Full screen.** Full screen is exclusive on every platform (owner decision, 2026-10-17, macOS
  included): `WindowMode::Fullscreen(Current, VideoModeSelection::Current)`, the desktop mode, no
  mode switch. On macOS winit captures the display for it and runs the panel at scale factor 1
  while it lasts (3456x2169 on the M1 Pro panel; the cursor may look small); the menu bar and Dock
  are hidden, and the Retina scale returns on leaving it. A packaged replay went windowed ->
  exclusive 1280x800 -> exclusive 1024x768 (aspect change) -> windowed without a hang.
- **Windowed (No Border).** `BorderlessFullscreen`: on Windows and Linux a borderless window over
  the monitor, on macOS the native full-screen Space. The game window sets
  `Window::borderless_game: false`: Bevy's default (true, winit's `with_borderless_game`) sets
  `NSApplicationPresentationHideMenuBar | HideDock` for borderless full screen, so the menu bar and
  the title bar never appeared until the Space was changed. Without it macOS uses its standard
  full-screen behavior: they slide in when the pointer reaches the top edge.
- **macOS native full screen recognition (owner decision, 2026-10-15; since 2026-10-17 the Space is
  Windowed (No Border)).** On macOS the native full-screen Space is a game display mode, so the
  green title-bar button is treated as a mode
  switch: `video::sync_native_fullscreen` (macOS only, PreUpdate) reads winit's real full-screen
  state from the main-thread `bevy::winit::WINIT_WINDOWS` (read-only) and, only when that state
  changes, maps it with `native_fullscreen_sync`: entering native full screen from Windowed sets
  `Window::mode` to `BorderlessFullscreen` and the display mode to Windowed (No Border); leaving
  full screen (either kind) sets Windowed. The applied configuration is updated and saved, any pending windowed resize is
  dropped, and Options shows the real mode. Transitions the game starts itself reach winit later
  and then agree with `Window::mode`, so they are not reverted. Root causes fixed: the game never
  learned about green-button transitions, so choosing Full screen while already in native full
  screen did nothing (bevy_winit saw no difference) and leaving it left a stale Full screen state;
  every OK/Apply also re-requested the windowed size (fixed for all platforms in
  `Simulation::applied_window`, which the sync also updates). An open Options dialog follows the
  change at once (`OptionsDialog::sync_display_mode` via
  `Console::set_display_mode_external`), instead of only after reopening. Windows and Linux are
  unchanged.
- A launch `--width/--height` sets the initial frame and window size for that session; the stored
  setting takes over after the next OK/Apply.

Verified (2026-10-11, macOS M1 Pro, 3456x2234 at 2x):

- `cargo test --locked -p hl2-ui -p hl2-bevy`: all pass. New or changed tests cover cursor mapping
  in frame pixels (including high-DPI full screen), canvas size, `windowed_size`, windowed resize
  decisions (logical size on 2x displays, clamping the native resolution, OS full screen
  wait/cancel), the macOS full-screen mapping, and camera routing into the frame at the configured
  resolution with monitor and presentation cameras untouched.
- Strict Clippy for hl2-ui and hl2-bevy; `cargo fmt --all`.
- Packaged captures (`scripts/build-bevy.sh`, `--map background01 --main-menu`):
  - `--windowed --width 1024 --height 768`: a 2048x1536 capture with the 1024x768 frame filling it,
    upscaled 2x.
  - `--fullscreen --width 1280 --height 720`: a 3456x2168 capture with the 1280x720 frame upscaled
    to the full width and letterboxed. The Video page opened from scripted pointer input.
  - `--windowed --width 3456 --height 2234`: the startup clamp resized the window to 1579x1021
    points (a 3158x2042 capture), as computed.

Not tested: the green-button OS full screen by hand (the recognition is unit-tested only), the cursor size in macOS full screen, real mouse input
through the scaled frame, Windows and Linux. Resolution changes at runtime are covered in section 10a.

### 10a. Safe resolution change (2026-10-11)

Problem: changing the resolution at runtime crashed the game in windowed and full-screen modes. The
log ended with a wgpu validation error ("Attachments have differing sizes: the depth attachment's
texture view has extent (1024, 768, 1) but ... color attachment ... (1600, 900, 1)"), after which
Bevy's render error handler exits.

Cause: the frame image was resized in place while cameras were rendering into it. The GPU texture is
replaced in the same frame, but a camera's cached target size (which sizes its depth and other view
textures) is only refreshed when the image's `AssetEvent::Modified` is read, one frame later. For
that frame the old-size depth texture is paired with the new-size color target. Swapping in a new
image handle would not help: a `RenderTarget` change alone does not refresh the cached size either.

Fix (`crates/hl2-bevy/src/video.rs`): stop rendering, resize, then resume.

- `suspend_for_resize` (PostUpdate, after transform propagation so it overrides the sky/monitor
  presenters' per-frame camera activity) notices that the frame image no longer matches the
  configured resolution and suspends for `SUSPEND_FRAMES` (4) frames. Every camera that renders into
  the frame is deactivated (its previous `is_active` is kept in `SuspendedCamera`). The image is
  replaced only on a frame when those cameras are already inactive: the frame becomes a new image
  at the new resolution, the present material is pointed at it and the old image is released.
  Resizing the old image in place froze the picture (2026-10-17): the present material's bind
  group keeps the texture it was built with, so the window went on showing the last frame drawn
  before the resize while input, sound and the console kept running. Rendering resumes after the
  remaining frames, once the camera target sizes have caught up, and each camera gets its previous
  activity back.
- While suspended the presentation sprite is hidden, so the window shows black for those few
  frames instead of a stretched stale frame.
- The Options dialog's resolution list is refreshed whenever the dialog opens
  (`ui_system::refresh_display_modes`), also when a scripted replay opens it.
- `apply_video_settings` (Update, chained before `resize_window`) applies pending video settings.
  It used to run inside the menu update, which returns early while a movement script runs, so
  scripted and console changes never reached the window.
- Console command `mat_setvideomode <width> <height> <windowed 0|1>` (the retail command) sets the
  resolution and display mode, saves the settings and applies them. Movement scripts use it to
  change the resolution reliably (`{"action":"command","command":"mat_setvideomode 1600 900 1"}`).
- `debug!` logs trace each request, suspension, image resize, resume and window resize step
  (`RUST_LOG=hl2_bevy=debug`).

Verified (2026-10-11, macOS M1 Pro):

- Unit test: cameras are stopped before the image is resized, the image is resized only while they
  are stopped, the presentation is hidden meanwhile, and activity and visibility are restored after
  the suspension.
- Packaged runs (`--map background01 --main-menu --windowed --width 1280 --height 720` with a
  movement script of `mat_setvideomode` commands), exit code 0 and no validation errors:
  - windowed 1280x720 → 1024x768 → 1600x900 → 1280x720; the window resized each time. Before the
    fix this script crashed on the second change.
  - windowed → full screen 1280x720 → full screen 1024x768 → windowed 1280x720; the window resize
    waited for the full-screen exit and then completed. The final capture renders correctly.

Known limitation: on the first frame after resuming, sky cameras may use a stale activity value for
one frame (a one-frame visual glitch at most). Not tested: changes made with the Options dialog's
OK/Apply through real mouse input (it uses the same pending-settings path), Windows and Linux.

### 10b. Standard resolution lists and pixel-scale windowed mode (2026-10-12)

Problem:
1. On Retina displays (macOS), the panel only reported high-resolution 16:10-like modes (1920x1200
   and up), leaving the "Normal (4:3)" aspect ratio bucket empty and omitting common resolutions like
   720p.
2. In windowed mode, sizing the window in logical points caused high-DPI displays (2x scale) to double
   the window dimensions, making even modest resolutions fill the entire screen and making native
   resolutions (e.g. 2560x1600 or 3000+) impossible to use windowed.

Design and changes:
- **Standard resolution list (`STANDARD_MODES`):** Added 23 standard resolutions covering 4:3, 5:4,
  16:9, and 16:10 (from 640x480 up to 3840x2160) to `hl2_ui::gameui::display`. In `usable_modes`,
  these are merged with the monitor's reported modes, deduplicated, and sorted largest first. Since the
  game renders to an offscreen frame and samples it, display hardware support is not required to offer
  or render a mode.
- **DisplayMode-aware choices (`DisplayModes`):** `DisplayModes` tracks both full-screen modes and the
  desktop pixel bounds. Windowed choices filter out resolutions that exceed the desktop pixels, and
  switching display mode or aspect cleanly selects the largest fitting candidate if the current choice
  becomes invalid.
- **Physical pixel windowed sizing:** Windowed mode sizes the window client area in physical pixels
  (`set_physical_resolution`), giving 1:1 pixel parity with the game frame resolution on high-DPI
  displays. This allows choosing crisp resolutions up to the display's native pixel limits.
- **Input tracking:** Added `debug!` telemetry in `hl2-bevy::ui` to log cursor position, window
  dimensions, scale factor, viewport bounds, and canvas pointer coordinates on click.

Verified (2026-10-12, macOS M1 Pro Retina):
- Unit tests in `hl2-ui` and `hl2-bevy`: Retina mode merging, 4:3 choices availability, windowed
  desktop bounds clamping, and physical pixel window resize calculations.
- Live packaged run with real mouse interaction: navigated Options -> Video, selected and applied
  4:3 and wide resolutions (1600x1200, 2048x1536, 640x480, 1024x768, 1280x1024) in windowed mode,
  confirming reliable click responsiveness and window sizing.

### 10c. Every aspect choice up to the panel's size; ultrawide modes (2026-10-18)

Owner report: with Normal (4:3) selected, a Retina display offered nothing large (4:3 stopped at
2048x1536 on a 3456x2234 panel), and modern aspect ratios were missing.

- Retina panels report no 4:3 mode at all, so 4:3 came only from `STANDARD_MODES`. Now
  `display::native_fits` adds, for each aspect choice, the largest mode of exactly that ratio
  inside the monitor (sides multiples of 8): on 3456x2234, 2976x2232 (4:3), 3456x2160 (16:10),
  3456x1944 (16:9). The panel's own 3456x2234 (1.55) stays in the 16:9 bucket, as the retail
  grouping puts it.
- `STANDARD_MODES` gains 2560x1920, 2800x2100, 3200x2400 (4:3), 2560x2048 (5:4), 3840x2400
  (16:10), 5120x2880 (16:9) and ultrawide 2560x1080, 3440x1440, 3840x1600, 5120x2160 (21:9)
  and 3840x1080, 5120x1440 (32:9). As before, only modes that fit the monitor are offered.
- The installed GameUI has three aspect choices only (`#GameUI_AspectNormal`, `Wide16x9`,
  `Wide16x10`; `GameUI.dll` references no other), and retail groups every ratio wider than
  16:10 under Widescreen 16:9, so ultrawide resolutions are listed there. A separate ultrawide
  choice would need a label the game does not have.
- Data: the standard modes (grouped by ratio label), the exact ratio of each aspect choice, the
  minimum size (640x480) and the native-fit step (8) live in `crates/hl2-ui/assets/display/modes.json`,
  embedded with `include_str!` and parsed once; `display::standard_modes()` / `min_size()` read it.
  `mode_data_parses_and_every_entry_is_usable` checks that it parses, that every mode is at least
  the minimum, that no mode repeats and that each aspect ratio falls in its own bucket.
- Tests: `every_aspect_choice_reaches_the_panel_size`, `ultrawide_panels_get_their_modes_under_widescreen`.
  Packaged windowed capture on the 3456x2234 panel: the 4:3 list starts at 2976x2232. Ultrawide
  modes are not checked on an ultrawide display, and how the game frames ultrawide pictures (FOV)
  is not verified.

### 10d. Ultrawide aspect choices; temporary placeholder labels (2026-10-18)

Owner request: add the missing ultrawide aspect ratios, with temporary placeholder labels.

- The Aspect Ratio combo gains **Ultrawide 21:9** and **Super Ultrawide 32:9** after the three
  retail choices (`AspectRatio::Ultrawide21x9` / `Ultrawide32x9`). The 16:9 bucket now ends at
  2.1 (2:1 panels stay 16:9); 21:9 takes ratios below 3.0 (2560x1080, 3440x1440, 3840x1600,
  5120x2160) and 32:9 everything wider (3840x1080, 5120x1440). Their native fits use 64:27 and
  32:9. Buckets, exact ratios and bounds are all in `crates/hl2-ui/assets/display/modes.json`
  (`below` = upper bound, `tolerance` = window around the exact ratio, checked first, for 16:10).
- The installed GameUI has no token for these choices, so their labels are temporary
  placeholders in `gameui::localize::placeholders`: `HL2RS_`-prefixed token constants
  (`ASPECT_ULTRAWIDE_21X9`, `ASPECT_ULTRAWIDE_32X9`, and the console's
  `CONSOLE_REVERSE_SEARCH`, moved there from `FALLBACK_STRINGS`) and their English text in
  `PLACEHOLDER_STRINGS`. `Localization::lookup` checks loaded resources first, then the
  placeholders, then `FALLBACK_STRINGS`, so a resource defining the token already wins.
- Planned replacement: HL2RS-owned localization resources (`hl2rs_<language>.txt` in the game's
  `"lang" { "Tokens" { ... } }` format, loaded with `Localization::add_resource`), which plugins
  or users can add to or override per language. The placeholder entries then move into
  `hl2rs_english.txt`, the table is deleted and the token constants stay.
- Tests: `aspect_buckets_match_the_video_page_grouping`, `ultrawide_panels_get_their_own_aspect_choices`
  (renamed from `ultrawide_panels_get_their_modes_under_widescreen`; 16:9 no longer lists
  ultrawide modes), `mode_data_parses_and_every_entry_is_usable` (the widest bucket is
  unbounded), placeholder tests in `localize::placeholders`. No ultrawide display tested; the HUD
  Aspect Ratio choices (installed `GameUI_HUDAspectRatio_*`) are unchanged.

### 10e. Testing other displays: `--fake-monitor` (2026-10-18)

Owner question: what an 8K display would offer, and a way to test it here.

- `--fake-monitor WxH[,WxH...]` (hl2-bevy) makes the Video page list resolutions for that
  panel instead of the host monitor: the first entry is the panel size (and desktop size), all
  entries are reported modes (`display::MonitorSpec`, parsed and validated in hl2-ui: at least
  the minimum size, reported modes fit the panel). Only the list is faked: window placement and
  display mode changes still use the real monitor, so applying a mode larger than it is not a
  test of that display.
- The Options report lists the Resolution combo's current choices (`options.resolutions`), so
  replays assert on the list rather than on a capture alone.
- On a 7680x4320 panel each aspect choice reaches the panel: 5760x4320 (4:3), 7680x4320 (16:9),
  6912x4320 (16:10), 7680x3240 (21:9), 7680x2160 (32:9); every standard mode fits, and windowed
  lists equal the full-screen ones because the desktop is measured in pixels (display scaling
  does not shrink it).
- Tests: `an_8k_panel_offers_every_aspect_up_to_its_size`, `monitor_specs_parse_from_the_command_line`,
  `a_bad_fake_monitor_fails_the_launch`; packaged replay `video_8k` (docs/validation.md). No real
  8K display tested.
- Resolution changes must never stall or crash. Report samples now carry `frame`, the size of the
  frame image being rendered (`Simulation::frame_size`, kept by `video::record_frame_size`), so
  replays check that each change was carried out. `resolution_stress` (background, faked 8K):
  Options Apply of 7680 x 4320, 320 x 240 and 8192 x 8192 (the console's limits), four changes on
  consecutive ticks mixing full screen and windowed (the last wins). The `window` check adds the
  windowed-only sequence on a real window. Exclusive full screen on a real display is not part of
  the automated checks (it changes the desktop's display mode).

### 10f. Square and portrait aspect choices (2026-10-18)

Owner request: add square and vertical aspect ratios (10:16, 9:16, 3:4).

- The Aspect Ratio combo gains **Square 1:1**, **Portrait 3:4**, **Portrait 10:16** and
  **Portrait 9:16** after the ultrawide choices (`AspectRatio::Square1x1`, `Portrait3x4`,
  `Portrait10x16`, `Portrait9x16`; temporary placeholder labels, section 10d). Buckets in
  `modes.json`: 9:16 below 0.59, 10:16 below 0.69, 3:4 below 0.875 (4:5 joins it), 1:1 below
  1.125, then Normal (4:3, with 5:4) below 1.5 as before. Standard modes per ratio were added
  (720x720 .. 2160x2160, 768x1024 .. 1536x2048, 800x1280 .. 1600x2560, 720x1280 .. 2160x3840) and
  every monitor gets a native fit for each.
- `min_size` (640x480) now applies in either orientation (`Resolution::at_least`): a portrait
  mode is measured with its sides swapped, so a rotated 1080x1920 monitor can be faked and
  offers portrait and landscape modes.
- Tests: `aspect_buckets_match_the_video_page_grouping`, `a_rotated_monitor_offers_portrait_and_landscape_modes`;
  packaged replay `video_portrait` (faked 1080x1920 monitor at 1080 x 1920: by keyboard, the
  Aspect Ratio list opens with Portrait 9:16 selected, the Resolution choices are its 9:16 modes,
  the frame renders in portrait).
- The UI was too wide on portrait frames (the Options dialog cut off at the right); fixed in
  section 10g.

### 10g. UI that fits narrow screens; keep-or-revert after an aspect change (2026-10-18)

Owner decisions: the UI also fits the width on narrow screens; an aspect ratio change asks to be
kept and reverts on its own (Cancel does the same).

- **Scaling.** Source scales its 480-line design by the screen height, which fits every screen
  at least as wide as 5:4. `proportional::design_height(canvas)` is the screen height limited to
  `width / narrowest_design_aspect` (5:4, `crates/hl2-ui/assets/ui/tuning.json`); on 5:4 to
  ultrawide screens it is the height, so nothing changes there. Dialogs
  (`Painter::scale_for_canvas`), the main menu (`MenuLayout::geometry`), the console window,
  HUD fonts and panels and the caption panel scale by it. HUD panels and the caption panel keep
  their vertical place by the full height (`proportional::y_scale`), so bottom panels stay at the
  bottom while their size follows the width. Never scale UI by the raw canvas height.
- **Keep-or-revert.** `gameui::video_confirm`: applying an Options change whose aspect bucket
  differs (`VideoConfirm::needed`) applies at once and opens a modal box (`ConfirmKind::KeepVideo`)
  over everything, opaque over a window. Text: installed `#GameUI_Confirm` title, `#GameUI_OK` /
  `#GameUI_Cancel`, the installed `#vgui_TimeLeftSecondsRemaining` / `#vgui_TimeLeftSecondRemaining`
  countdown, and the temporary placeholder question `HL2RS_KeepVideoSettings` (no installed
  token). OK keeps; Cancel (focused by default), the close button, Escape and the timeout put
  back the previous width, height and display mode (saved and applied; an open Options dialog
  shows them). The timeout is `video_confirm_seconds` (15) in `tuning.json`: common practice is
  a 10 to 15 second countdown that reverts by default (Windows, GNOME, KDE; one Xfce report
  found 10 s too short while a display resyncs). Retail Source has no such box. The countdown
  runs on `Console::tick`, called every frame with real time, also while a replay drives the
  input; a background map load does not close the box. Console `mat_setvideomode` does not ask.
- Reports: `options.frame` (placed dialog), `console.confirm` (`kind`, `text`).
- Tests: `proportional`, `tuning`, `video_confirm` unit tests, `the_dialog_fits_square_and_portrait_screens`,
  console `an_aspect_change_applies_and_asks_to_keep_it`,
  `the_keep_video_box_counts_down_and_reverts_at_the_timeout`,
  `ok_keeps_the_change_and_cancel_escape_revert_it`; packaged `video_portrait` (dialog inside the
  1080 x 1920 frame) and `video_keep` (timeout, OK, Escape). Captures inspected: Options on
  1080 x 1920, the keep box over Options, the HUD of `bevy-hud.json` at 1080 x 1920.

### 10h. Portrait menu, console, background and view (2026-10-18)

Owner list after the narrow-screen scaling of 10g: the menu buttons became too small on 9:16, the
console ran to the bottom, the pre-blurred background stretched, and the first-person view was a
thin slice with the weapon cut off. Landscape from 5:4 up is unchanged by all four.

- **Main and sub menu.** `MenuLayout::geometry_for(canvas)`: the titles and the menu's position
  follow the UI design height (10g), but on a screen narrower than the design the rows' font and
  spacing follow the real screen height, as retail would on a screen that tall (1080x1920: 44 px
  text and 92 px rows instead of 16 and 38). The row hit width is the retail 250 units, limited to
  `canvas width - 2 * menu x` so it fits beside the margin (`MenuGeometry::row_wide`, 808 of 1080
  px on 9:16). Square screens get the same rule (their height-based font equals the design one at
  1080x1080).
- **Developer console.** `DialogLayout::default_frame`: on a narrow screen the frame spans the
  width between the 16-unit margins and ends where the design area ends (64 units above
  `design_height`), instead of half the width and down to the bottom; landscape keeps the retail
  placement (half the screen wide, `h - 2 * 64` tall). A saved placement still wins.
- **Pre-blurred background.** `loading::background_source` / `draw_background_texture` (both draw
  sites): on a screen narrower than the design the texture fills the screen without stretching,
  the centre of the 4:3 image cropped at the sides (iOS `scaleAspectFill`); no new texture is
  needed. 5:4 and wider keep the retail stretch. The overlay only shows while a background map
  loads, which here finishes within about 25 frames, so it was checked through the real draw
  function (`the_overlay_draws_the_cropped_texture_over_the_whole_screen`), not a capture.
- **View and viewmodel.** `video::vertical_fov_for(degrees, aspect)`: Source's FOV is horizontal
  at 4:3. A frame at least as wide as 4:3 keeps the vertical view of 4:3 (retail Hor+, bit for bit
  as before); a narrower frame keeps the 4:3 horizontal view and shows more above and below
  (Vert+). The world, sky (copies the world camera), menu background maps (env_zoom goes through
  the same call) and the viewmodel camera all use it, so on 1:1 and 9:16 the weapon is whole and
  sits where it does in a 4:3 frame instead of leaving the screen. The viewmodel's attachment
  size factor stays a ratio of the two FOVs.
- Tests: `narrow_screens_keep_row_text_by_height_and_fit_the_hit_width`,
  `landscape_frames_are_the_retail_ones_and_narrow_screens_get_a_bounded_frame`,
  `narrow_screens_fill_with_the_centre_of_the_background_and_others_stretch`,
  `narrow_frames_keep_the_four_by_three_horizontal_view`; packaged `narrow_layout` (1080x1920,
  1080x1080 and 1920x1080 through the report's `console.menu` and `console.console_frame`).
  Captures inspected at 1080x1920 (menu, Episode One submenu, console, gameplay with the SMG) and
  1080x1080.
- Not verified: how the viewmodel looks on a real portrait display; the world's wider vertical
  view shows more floor and ceiling than a designer may like (it is the 4:3 horizontal view).

### 10i. Vertical and viewmodel FOV sliders, wide-screen menu, portrait and ultrawide menu backgrounds (2026-10-18, updated 2026-10-19)

Owner list after 10h, extended 2026-10-19 with ultrawide menu background and pre-blurred overlay cropping.

- **Sliders.** Video > Advanced gains **Vertical FOV** (under Field of view) and **Viewmodel FOV**
  (next to it), so view model problems at portrait and ultrawide ratios can be tuned by hand.
  - `video.vertical_fov` (degrees, 0 = **Auto**, the slider's low end; explicit values
    `vertical_fov_range` 40..120). On `5:4..=16:9` frames Auto is the derived vertical field of
    view of 10h (`59.84°` at default `75°` 4:3 FOV). Outside `5:4..=16:9`, Auto smoothly applies
    `tuning::world_fov_offset(aspect)` (`assets/ui/tuning.json` `world_fov_offset`: `-17.0°` at
    `9:16` portrait -> `89.16°` vertical / `58.0°` horizontal instead of `107.51°`, avoiding
    extreme vertical fish-eye; `-11.0°` at `32:9` super-ultrawide -> `50.22°` vertical / `116.19°`
    horizontal instead of `127.69°`, taming peripheral horizontal stretching). An explicit slider
    value replaces Auto for the player's default FOV, and weapon zoom or `env_zoom` scale either
    one in proportion (`video::vertical_fov_with`), so the crossbow still zooms. A main-menu
    background ignores it. The vertical field of view is a real thing: games such as Source expose
    a horizontal one (`fov_desired`, horizontal at 4:3) and the vertical one follows from the
    aspect ratio; the slider overrides that.
  - `video.viewmodel_fov` (`viewmodel_fov`, 4:3 horizontal degrees, default 54, range
    `viewmodel_fov_range` 30..110) replaces the retail 54 in `viewmodel_fov(base, default, fov)`
    for the viewmodel camera and its attached effects; zoom still subtracts the FOV offset, so a
    base above 54 + (default - zoom FOV) keeps the viewmodel visible when zoomed (retail hides it).
  - `tuning::viewmodel_framing(weapon, aspect)` (`assets/ui/tuning.json` `viewmodel`) applies an
    additive `fov_offset` (degrees) and camera-space `offset` (`[forward, left, up]` in Source units,
    applied to `ViewmodelCamera` and viewmodel attachment effects) from a `standard`, `portrait`, or
    `ultrawide` profile in `default` or a per-weapon override (`weapons`). Standard values default
    to zero; below `5:4` and above `16:9` those values smoothly blend toward the portrait and
    ultrawide profiles respectively:
    - **Portrait (`< 5:4` down to `9:16`)**: `weapon_physcannon` shifts down (`offset: [0, 0, -11.5]`)
      so its severed left arm stump stays below the bottom bezel without narrowing horizontal FOV;
      `weapon_crowbar` uses `+8°` FOV and shifts down (`offset: [0, 0, -11.0]`) so its arm stump
      stays below the bottom bezel throughout swing animations; `weapon_rpg` uses `+16°` FOV and
      shifts left/down (`offset: [0, 4.0, -16.0]`); other weapons use `default.portrait` (`+8°`,
      `[0, 0, -9.0]`). The physcannon keeps zero FOV adjustment and its existing downward shift.
      Larger FOV values show more horizontal scene but make the weapon smaller. These portrait values
      match the `97fac81` baseline and are directly adjustable in the tuning asset.
      `fov_offset` changes the horizontal viewmodel FOV at the 4:3 reference aspect; `offset` moves
      the viewmodel camera in `[forward, left, up]` Source units. These settings preserve model
      proportions. The RPG's 9:16 projection test checks that its tube reaches into the frame; it
      does not assert that its unfinished rear is cropped in portrait. The JSON metadata documents
      both controls for future tuning.
    - **Ultrawide (`> 16:9` up to `32:9`)**: `default.ultrawide` (`+30°`, `[0, 0, -3.5]`) and
      `weapon_physcannon` (`+30°`, `[0, 0, -7.5]`) widen the viewmodel FOV to `84°` so the hands and
      forearms sit comfortably above the bottom edge while keeping arm stumps grounded at the bottom
      bezel; `weapon_rpg` widens FOV (`+28°` -> `82°`) and shifts back/right/down
      (`offset: [-2.0, -21.5, -1.5]`) so the hand and forearm remain visible above the bottom edge
      while the unmodeled rear cutoff of the launcher tube stays outside the right edge.
    - Standard `5:4`, `4:3`, `16:10`, and `16:9` frames use the zero-valued `standard` profile and
      are untouched unless a developer edits those values. Because framing is looked up from
      `game.inventory.active`, switching weapons applies the new weapon's framing immediately as its
      draw animation begins.
  - F5 toggles the runtime-only viewmodel tuning overlay during unpaused gameplay. It shows the
    active aspect profile, weapon classname, `fov_offset`, and `[x, y, z]` offset. `[`/`]` adjust FOV,
    `,`/`.` lower/raise Z, Up/Down adjust forward X, and Left/Right adjust lateral Y. Steps are
    `0.1`, or `0.01` while Shift is held. Adjust keys repeat while held (same timing as Options >
    Keyboard list arrows). F6 zeros the selected weapon's current profile `fov_offset` and `offset`.
    Adjustments apply immediately without disabling normal
    mouse look, movement, firing, reloading, or weapon selection. The tuner retains
    edits in memory by weapon and aspect profile for the current F5 session; number keys 1-6 use
    normal weapon selection/confirmation, and revisiting a weapon/profile restores its prior edit. The
    session survives Pause/Options so display resolution and aspect can be changed; while paused,
    keys go to the UI and tuning resumes on return to gameplay. Every adjustment updates one
    replaceable developer-console snapshot containing the complete `tuning.json` with all session
    edits merged. Turning F5 off ends the session and discards these in-memory edits. The utility
    does not write the asset file; copy the accumulated logged JSON into it to keep changes.
  - Both sliders are applied by the sub-dialog's OK (`Sub::apply_on_ok`) and saved in `hl2-config.json`
    (out-of-range values are clamped on load). Ranges and the other numbers are in
    `assets/ui/tuning.json`.
  - The installed dialog has no such controls, so they come from an HL2-RS layout addendum,
    `assets/ui/optionssubvideoadvanceddlg_extra.res` (`resources::LAYOUT_ADDENDA`, appended to the
    installed layout on load): controls in the free band under the Field of view slider (y 302..352,
    above the divider), the same on both platform layouts. Their labels `associate` their sliders so
    the footer reflow (which moves unbound labels near the buttons down with them) leaves them
    alone. Texts: the **Auto** value is the installed `#Spec_Auto`; the two slider names have no
    installed token and are temporary placeholders `HL2RS_VerticalFov` / `HL2RS_ViewmodelFov`.
    The keyboard focus order gained two stops after the FOV slider (the `keyboard-fov` replay
    now presses Shift+Tab seven times).
  - The report's samples carry `camera`, the vertical degrees of the world and viewmodel cameras.
- **Wide-screen menu.** Above 16:9 the menu grows with the extra width up to 1.35x
  (`proportional::widescreen_menu_scale`, `widescreen_menu_scale_max`), titles, rows and text
  alike, and grows upward: the bottom of the seven-row stack (`menu_stack_rows`) stays where it
  was, so it never runs off the screen. 21:9 grows 1.33x, 32:9 the cap. Up to 16:9, 16:10 and
  5:4 nothing changes; portrait keeps 10h. The console keeps the retail half-width placement.
- **Menu background in portrait and ultrawide.** A main-menu background level (`camera::world_fov`)
  keeps the 4:3 vertical view on frames up to 16:9 (cropping the left and right below 4:3 and
  expanding horizontally up to 16:9) and keeps the 16:9 horizontal view on ultrawide frames above
  `tuning::widest_design_aspect` (`16:9`), cutting the top and bottom of the 16:9 frame rather
  than adding left and right portions to it. The pre-blurred loading overlay
  (`loading::background_source`) applies the same vertical center-crop above 16:9 so the 16:9
  widescreen image is never stretched horizontally on 21:9 or 32:9 screens. Gameplay still uses the
  Vert+ view below 4:3 and Hor+ at 4:3 and wider.
- Tests: `Value` and label tests (settings), `the_vertical_and_viewmodel_fov_sliders_are_added_and_apply_on_ok`
  (also proves the labels stay out of the footer), `wide_screens_grow_the_menu_upward_within_the_screen`,
  `the_menu_grows_only_on_screens_wider_than_16_by_9`, `an_explicit_vertical_fov_replaces_the_derived_one_and_scales_with_zoom`,
  `viewmodel_framing_leaves_retail_aspects_untouched_and_adapts_portrait_and_ultrawide`,
  `viewmodel_framing_keeps_physcannon_and_rpg_in_frame_across_portrait_and_ultrawide`,
  `menu_backgrounds_crop_sides_when_narrow_and_top_and_bottom_when_ultrawide`,
  `narrow_and_ultrawide_screens_fill_with_the_centre_of_the_background`; packaged
  `fov_settings` (defaults, explicit values, portrait and super-ultrawide through the camera report) and
  `narrow_layout` (now with 21:9 and 32:9 and the cropped background FOV). Captures inspected: the
  Advanced dialog, the menu at 3440x1440, 5120x1440 and 1080x1920, and `weapon_physcannon` /
  `weapon_rpg` at 1080x1920 and 5120x1440.
- Not verified: physical portrait or ultrawide displays; the vertical slider's keyboard stepping at
  the Auto end.

### 10j. HUD Aspect Ratio (2026-10-19)

Options > Video > HUD Aspect Ratio (`video.hud_aspect`, `config::HudAspect`: Fit Display, Centered
4:3, Centered 16:9, Centered 16:10) lays the HUD out in a centered span of that aspect instead of the
whole screen. Evidence: the installed client registers the `hud_aspect` convar, help text "Force the
aspect ratio of the hud. 0 to disable."; the Source SDK has no such variable, so the exact retail
treatment of every element is **not verified**; this is the conservative reading of the setting.

- `proportional::hud_frame(screen, aspect) -> (x, width)`: a screen wider than `aspect` gets a
  span of `height * aspect`, centered, full height; a narrower screen and Fit Display use the whole
  width. The ratios are data (`assets/ui/tuning.json` `hud_aspects`, `tuning::hud_aspect_ratio`).
- `Canvas::set_frame` / `clear_frame`: while a frame is set, `Canvas::width()` is the frame's width and
  every quad and polygon moves right by the frame's x, so the existing panel layout code (right and
  center aligned `xpos`, animations, scale from `proportional::scale`) works unchanged inside the frame.
  `WeaponHud::framed()` sets it for one draw; with Fit Display, or a screen no wider than the chosen aspect,
  nothing is set and the output is bit for bit what it was.
- Inside the frame: health, suit, ammo and secondary ammo panels, the quick info, the weapon selection,
  the damage indicator sides and the closed captions. Outside it: the crosshair (view center), screen
  fades, the full-screen damage flash, the world view and every menu.
- The report (`hud.frame` = `[x, width]`, `hud.panels` = health, suit, ammo, secondary ammo rectangles in
  screen pixels at rest) shows the result; packaged check `hud_aspect`.

### 10k. Dynamic aspect ratio: Fit Display (2026-10-20)

Options > Video > Aspect Ratio adds **Fit Display** (`AspectRatio::Flexible`, persisted in `video.flexible_aspect` and labelled with the installed token `#GameUI_HUDAspectRatio_FitDisplay` "Fit Display"), allowing the rendering frame to follow the shape of the window, display, or workspace bounds dynamically.

- **Universal aspect ratio math (`tuning`):**
  - `min_supported_aspect()` (9:16, 0.5625) and `max_supported_aspect()` (32:9, 3.5556) define the supported rendering bounds derived from `viewmodel.portrait_aspect` and `viewmodel.ultrawide_aspect`.
  - `clamp_supported_aspect(aspect)` clamps any aspect ratio to `min_supported_aspect()..=max_supported_aspect()`.
  - `cubic_bezier_ease(t)` evaluates smooth 1D cubic Bézier ease with zero slope at endpoints ($3t^2 - 2t^3$), preventing slope kinks where tunings leave the untouched 5:4..=16:9 plateau or saturate at 9:16 / 32:9.
  - `aspect_factors(aspect) -> (portrait, ultrawide)` smoothly blends tuning values between known aspect ratios; drives both `world_fov_offset(aspect)` and `viewmodel_framing(weapon, aspect)`.
- **Viewport and resolution:**
  - `viewport::flexible_frame_resolution(configured, physical_window_size)`: the frame adopts the window's aspect ratio (clamped to 9:16..=32:9). The configured resolution sets detail (the short side: 1080 lines in landscape, 1080 columns in portrait), rendering 1:1 on smaller windows and upscaling cleanly on high-DPI/larger displays.
  - `ViewportBounds::compute_with_mode(..., flexible)` / `VideoViewport::compute_with_mode(..., flexible)`: fills 100% of the active window/display without letterbox or pillarbox bars when within 9:16..=32:9; letterboxes below 9:16 and pillarboxes above 32:9.
  - In windowed mode, `viewport::flexible_windowed_size(resolution, bounds)` limits each dimension independently to workspace bounds (`DesktopArea::client_room` on macOS, available screen on Windows/Linux) without forcing uniform aspect scaling, only narrowing/shortening if exceeding 9:16..=32:9. Live window resizes adapt `GameFrame::resolution` dynamically on presented frames.
  - While Fit Display is selected, the **Resolution** dropdown is disabled (`settings::enabled` returns false when `draft.aspect == AspectRatio::Flexible`, which also automatically disables its associated label `ResolutionLabel`). The combo displays the active content resolution in physical pixels without letterbox or pillarbox black stripes (`"{w} x {h}"`, e.g. `"1920 x 1080"`, via `current.without_stripes_for_flexible()` clamped to 9:16..=32:9). Selecting from it is a no-op; switching back to any fixed aspect ratio re-enables the control and restores the selectable resolution choices for that aspect ratio.
  - Applying Fit Display in Options prompts with `VideoConfirm` to keep or revert the mode.
  - **Ordering and resolution handoff (2026-10-22):** Fit Display is the first Aspect Ratio
    choice. Selecting it uses the active physical content area rather than the last fixed choice
    as the resolution/detail source. The host additionally raises a stale fixed configured size
    to the current physical content area while applying flexible mode, preventing a 640x480 4:3
    setting from producing a low-resolution frame on a Retina/high-DPI display. Unit coverage
    exercises the 640x480-to-Fit-Display transition; packaged high-DPI and portrait interaction is
    not tested.

## 11. Developer console (2026-10-17)

Evidence: SDK `vgui_controls/consoledialog.cpp`/`.h` (`CConsolePanel`), the installed
`hl2_complete/resource/sourcescheme.res` (`Console.TextColor` OffWhite, `Console.DevTextColor`
White, `RichText.*`) and Source's own console variables. Code:
`hl2-ui::console::{commands, complete, dialog, editor}` (pure, unit-tested), drawing in
`Console::draw_console`/`draw_completion`, host side in `hl2-bevy::console_host`.

- The dialog (`CConsoleDialog`): a `#Console_Title` frame drawn through the shared dialog frame
  (`Painter::frame_with_close`, rounded, close ×), with no screen dimming, over whatever it was
  opened from (main menu, pause menu, Options or the game). Default placement as in retail:
  proportional 16 and 64 (480-line basis) put it at `(w/2 - 16, 64)`, `w/2` wide and
  `h - 2 * 64` tall. Inside the frame client area (`Frame.ClientInsetX` 8, title 30,
  `Frame.ClientInsetY` 6) the `CConsolePanel::PerformLayout` insets apply as plain units (the
  panel is not proportional): history at inset 8 + top 4, entry and `#Console_Submit` button
  (64 wide, pulled 7 from the edge, Submit is the default button) 24 tall along the bottom,
  4 apart. Opened over Options it is opaque (`Theme::stacked`). The completion list is as wide
  as its widest row (at least the entry), up to the canvas edge. `console::dialog::DialogLayout` is the one geometry for drawing and hit tests.
- History (`RichText`, `DepressedButtonBorder` = sunken `Border.Dark`/`Border.Bright`): background
  `RichText.BgColor`, a vertical scroll bar (arrows, track paging, nob dragging) and the wheel
  (`widgets::ScrollMode`: 3 rows a notch on Windows/Linux, accumulated row steps on macOS). Scrolled back, the view stays on its rows while output arrives; Enter and
  Ctrl+End return to the newest, Ctrl+Home goes to the first row.
- Wrapping (SDK `RichText::RecalculateLineBreaks`, `console::dialog::wrap`): a line wider than the
  history breaks before the word that overflows (words are separated by whitespace only), or
  at the overflowing character when that word started the row; a newline in a line (mirrored
  engine warnings carry them) starts a row (`\r\n` counts once, one trailing newline adds no
  row); rows continuing a wrapped line skip their leading spaces, rows after a newline keep them;
  a tab is 8 spaces wide. Rows are measured in canvas pixels with the console font at the
  drawn size, re-wrapped when the history width or a line's shown text changes (each line's rows
  are cached by line number). Lines stay whole in the buffer: scrolling, the scroll bar and the
  wheel count rows, while filtering, coloring, selection and copy work on lines. The entry is a sunken `TextEntry` with no prompt (the `] line`
  echo stays in the history); it scrolls to keep the cursor in view.
- Selection: dragging over the history selects the lines under the rows it covers, every row
  of a wrapped line highlighted (`RichText.SelectedBgColor` Orange, `RichText.SelectedTextColor`
  Black), scrolling a row per frame past the top or bottom. Cmd+C on macOS, Ctrl+C elsewhere
  copies the selected lines as printed, one per line with their own newlines kept and no wrap
  breaks, with the current `con_filter_enable` filter and `con_timestamp` prefix, through Bevy
  `Clipboard`. Rows are numbered since start, so the selection stays on its
  lines while new output arrives and old lines leave the 1024-line buffer; Ctrl+L clears it.

- Commands and cvars are described once (`console::commands::COMMANDS`: name, help as
  `name args: description`, what each argument completes from, cvar or command). `help` and
  `find` print from it.
- Completion (retail `RebuildCompletionList`, extended): the line's last command (after the last
  `;` outside quotes) completes its first word from every command and cvar, later words from the
  argument's source: installed maps (`source_assets::install::installed_maps`, loose and in each
  game's own VPKs), the loaded map's targetnames and the inputs its own connections fire
  (`console_host::MapEntityNames`), command names, or fixed values. Ranking: exact name, prefix
  matches alphabetically (retail), then substring and letters-in-order matches (HL2-RS
  addition). An empty line lists the command history, like retail.
- The list shows under the input line, or above it when there is no room (the entry is at the
  bottom of the panel), at most 10 rows with the last one `...` when more (retail
  `MAX_MENU_ITEMS`), cvars with their current value. Above it, a help line shows the usage of the
  command being typed with the argument being typed highlighted (HL2-RS addition).
- Keys: Tab/Down step forward and Shift+Tab/Up back, putting the item in the line (retail
  `OnAutoComplete`); Tab on a single match takes it with a space; Up/Down on a line without
  matches step through the history; Escape ends a Ctrl+R search, then hides the list, then
  closes the console; Ctrl+R searches the history backwards (Enter runs the match, arrows/Tab take
  it); Ctrl+W/Backspace and Ctrl+Delete delete words, Ctrl+U/K to the start/end, Ctrl+A/E move
  there, Ctrl+arrows move by words, Ctrl+Home/End scroll the output, Ctrl+L clears it; paste is
  Cmd+V on macOS and Ctrl+V elsewhere (Bevy `Clipboard`); typed text is ignored while Control or
  Cmd is held.
- History: 100 entries (retail `MAX_HISTORY_ITEMS`), a repeated command moves to the end, saved
  in `hl2-console-history.txt` next to the settings file (never in the install). Lines submitted
  by scripts or `Console::submit` do not join it.
- Output: lines carry a level and the time they were printed; input echoes and messages use
  `Console.TextColor`, developer lines (mirrored HL2-RS info logs) `Console.DevTextColor`, read
  from the installed `SourceScheme.res`; warnings and errors (mirrored from the engine log, every
  target) use HL2-RS colors. Source's `con_filter_enable` (1 shows only lines containing
  `con_filter_text`, 2 shows them brighter) and `con_timestamp` apply to the shown output. The run
  report still records lines as text.
- Not ported: binds, aliases, `exec`, cvar registries for unimplemented engine cvars,
  game-specific commands from other titles, moving and resizing the console dialog (and the
  bounds retail remembers), and character-level selection inside a history row.

## 12. Moving and resizing dialogs (2026-10-17)

Evidence: SDK `vgui_controls/Frame.cpp` (`CaptionGripPanel`, `GripPanel`, `GetCaptionHeight` 23,
`GetCornerSize` 8, `GetBottomRightSize` 18, clip-to-parent snapping). Code:
`hl2-ui::gameui::placement` (pure, unit-tested), used by the Options dialog, its sub-dialogs and
confirmation box, the console dialog and the quit / main-menu box; cursor shapes set by
`hl2-bevy::console_host::apply_cursor_hint`.

- Dragging: a press on the caption (the top 23 units, the frame width minus 10, never the
  title-bar close button) moves the dialog while the button is held. The caption keeps the arrow
  cursor, as retail does. The press and the release belong to the drag: nothing under the pointer
  is activated or selected (the caption wins over the console's history row selection), and a
  press on the caption while a drop-down list is open only closes the list.
- Resizing (console only): the corner grips (8 x 8, the bottom-right one 18 x 18) resize the
  frame with the opposite corner fixed, showing the diagonal resize cursor. The smallest size is
  the SDK minimum raised so the entry keeps a Submit button's width and the history three rows
  (`console::dialog::MIN_SIZE`); a grip pulled past a canvas edge stops at that edge.
- Safety: the placement is an offset from the default position plus, for the console, a size,
  both in layout units, re-clamped to the current canvas and scale every frame: a dialog is
  snapped fully onto the canvas, or to its top-left when it is larger, and a stored size never
  exceeds the canvas, so window and display-mode changes never leave a dialog out of reach. A
  gesture ends whenever the button is up (also after a release outside the window), Escape
  during a gesture undoes it, and a double click on the caption restores the default position
  and size (HL2-RS additions). Sub-dialogs and message boxes open centered.
- Persistence: the Options dialog's position and the console's position and size are saved in
  the settings file (`dialogs`: offset from the default position and size, layout units) when a
  drag, resize or reset ends, and restored at startup. When a dialog opens and its saved frame
  no longer lies fully on the canvas (the resolution or display mode changed), it opens at its
  default place instead and that reset is saved; non-finite or non-positive saved values are
  dropped on load.
- Cost: resizing rewraps the history every frame; measured 2.6 ms per frame (release) with a
  full 1024-line buffer of 220-character lines and the console font, 0.2 ms without a resize.
  ASCII advances of the console font are cached per size.

## 13. Main-menu background after leaving a map (2026-10-17)

When a gameplay map finishes loading (a new game, `map`, a level transition), the background of
the chapter it belongs to becomes the saved menu background (`Console::remember_chapter_background`,
written only when it changes). Return to main menu shows it, and so does the next launch after
quitting mid-game. Module: `hl2-ui::gameui::chapters`.

- **Sources (installed files only, no map table):** a game's `scripts/chapterbackgrounds.txt`
  gives its chapter keys in play order (`1`..`9`, `9a`, `10`..`15` for Half-Life 2) and each
  chapter's background map; `cfg/chapter<key>.cfg` gives the chapter's start map (the word after
  `map` anywhere in the line, so Episode Two's `playvideo_exitcommand ep1_recap map
  ep2_outland_01` works); the `map` keyvalue of every `trigger_changelevel` in the entity lump of
  each loose `<game>/maps/*.bsp` gives the level-transition graph
  (`source_assets::bsp::read_entities` reads only the header and lump 0).
- **Assignment (`chapters::assign`):** the graph is undirected. Removing the chapter start maps
  splits it into stretches. A stretch bordered by one start belongs to that chapter. A stretch
  bordered by several starts (a map that skips ahead, such as `d3_c17_02_camera`) is measured by
  breadth-first distance from its earliest bordering start, within the stretch and its border
  starts: each map takes the farthest bordering start that is strictly closer than the map
  itself, so ties keep the earlier chapter (`d2_coast_08`, a detour off `d2_coast_07`, stays in
  chapter 7). Start maps the game does not ship are ignored. Maps no start reaches, test maps and
  background maps get no chapter and fall back to the game's chapter 1 background.
- **Results on the installed games:** Half-Life 2 chapters match the retail chapter list
  (`d2_coast_01` -> 7 -> `background04`, `d2_prison_07` -> 9a, `d3_c17_10a` -> 11); Episode
  Two's `ep2_outland_02` revisit loop stays in chapter 2; Lost Coast is one map, chapter 1.
  Episode One lists 8 chapters, but chapters 6-8 have no cfg of their own (they resolve to the
  inherited Half-Life 2 ones), so those starts are not Episode One maps and are ignored.
- **Cost:** reading every map's entity lump takes about 0.2 s for Half-Life 2 (release), less for
  the others, so `Console::index_chapters` builds the index on a `chapter-index` thread at
  startup; a lookup before it finishes works the running game out synchronously.
- **Tests:** unit `stretches_and_bypasses_follow_the_chapter_starts`,
  `changelevel_targets_come_from_trigger_changelevel_maps`,
  `loading_a_map_saves_its_chapter_background_for_the_menu`; owned
  `owned_maps_belong_to_their_installed_chapters`; packaged replay
  `test-inputs/bevy-menu-chapter-return.json` (`d2_coast_01` -> `hl2/background04`, `d3_c17_10a` -> `hl2/background06`) and `scripts/checks/replays.py` (`chapter_quit`) (`--map d2_coast_01`, exit
  mid-game: the saved background goes from `hl2/background01` to `hl2/background04`).

## 14. Non-modal windows: Options and the console (2026-10-18)

Owner report: the menu could not be used while the Options dialog was open, unlike the original
game; the developer console must not block the menu or Options either; dialogs show their focus
as the original does.

- **Model.** `gameui::windows::Windows` holds the open windows (Options, the console) bottom to
  top over a base (main menu, pause menu or gameplay) and which layer has the keyboard focus. It
  is a pure model (no drawing, no `Console`), unit-tested on its own.
- **Pointer.** Each frame `Windows::route` picks the layer that gets the pointer: the layer a held
  press belongs to (until release), else a modal layer inside Options (sub-dialog, its
  confirmation box, an open list, a key capture), else the topmost window whose placed frame
  contains the pointer, else the menu (gameplay under the console takes no pointer). Without a
  pointer position (scripted input) the focused layer gets the button and the wheel. Every layer
  sees every frame with only its share of it (`Input::share`: the pointer if it owns it, the
  keyboard if it has the focus); a layer without the pointer has its button tracker occluded
  (`ButtonTracker::occlude`), so a press that started elsewhere is never read as its own.
- **Focus.** A press on a window raises it to the top and focuses it (SDK `Frame::Activate`); a
  press on the menu focuses the menu and leaves the windows where they are. With a window
  focused the menu takes no keys, and its rows arm only under the pointer. `Console::mode()` is
  the focused layer (`Options`, `Console`, or the base); `Console::set_mode` opens a window on top
  or replaces the base (closing every window), the only way to change it.
- **Menu actions.** OPTIONS with the dialog open raises it as it is (its unapplied edits stay;
  retail is believed to activate the existing dialog, unverified: GameUI is not in the SDK). QUIT shows the confirmation box, modal over every
  window. Resume and starting a game hide the UI: the windows close (Options without applying).
  A submenu background loading behind the windows keeps them open.
- **Escape** closes the top window first, whichever layer has the focus; with no window open it
  acts on the base as before. Tilde closes an open console whether it has the focus or not.
- **Look** (SDK `Frame::OnFrameFocusChanged`, installed `hl2_complete/resource/sourcescheme.res`):
  the focused window's background is `Frame.BgColor` (132 132 132 212), an unfocused one's
  `Frame.OutOfFocusBgColor` (132 132 132 62); the background fades linearly over
  `Frame.FocusTransitionEffectTime` (0.3 s); the title switches at once between
  `FrameTitleBar.TextColor` and `FrameTitleBar.DisabledTextColor` (255 255 255 192)
  (`Theme::focus_faded`). A window over another window fades to opaque instead of
  `Frame.BgColor` (the stacked-dialog rule; the focused window is always the top one). Known
  case: with the menu focused, an unfocused console over an unfocused Options shows Options
  through it.
- **Tests.** `gameui::windows` (topmost hit, raise and focus, press ownership across layers,
  modal layer, gameplay base, focus after closing, a closed window's press, pointerless input,
  linear fade); `console` (menu beside Options works; a press on Options released over the menu,
  a menu press dragged onto Options, a caption drag across the menu at scale 1 and 2, a sub-dialog
  over the menu, the quit box over Options, OPTIONS again keeping the dialog, Resume closing it,
  the overlap of the console and Options and raising by a press, pointerless input, the focus
  fade, Escape with the menu focused, a submenu background load behind Options, the completion
  popup past the console's frame). Packaged: `test-inputs/bevy-menu-options-nonmodal.json` and
  `bevy-menu-options-list.json` (an open Resolution list: a click on the menu only closes it)
  (`replays.py` `menu_options`).
- **Not done:** retail probably hides open windows on Resume and shows them again with the pause
  menu (unverified); here they close. The console's completion list, which can reach past the
  console's frame, is part of the console for hit tests (`Console::window_contains`). Scripted
  replays carry no times, so packaged captures show finished fades; the 0.3 s timing is unit
  tested (the host feeds input and drawing from the same real-time clock).

## 15. Unified single-focus state for menus and dialogs (2026-10-21)

Unified focus model across the main/sub menu and all dialogs (Options, dev console, sub-dialogs,
and confirmation dialogs):
- **Single active focus invariant:** strictly one surface (`Layer::Base` or `Layer::Window(top)`)
  holds keyboard and primary interaction focus at any given moment (`Windows::focus()`).
- **Hierarchy and translucency:**
  - When a sub-dialog (e.g. Video Advanced, Keyboard Advanced) opens over Options, Options loses focus
    and transitions to translucent (`Theme::focus_faded` using `Frame.OutOfFocusBgColor`).
  - When a confirmation dialog (e.g. Video Keep/Revert) opens over a sub-dialog, the sub-dialog loses focus
    and transitions to translucent.
  - When any dialog is open over the main or submenu, the base layer loses focus: the Half-Life 2
    logo, chapter subtitle, and menu buttons transition to translucent using the 0.3 s fade time and
    unfocused alpha factor (0.3) declared in `assets/ui/tuning.json` (`ui_fade_duration`,
    `unfocused_alpha_factor`, matching `sourcescheme.res` `Frame.FocusTransitionEffectTime` and
    `Frame.OutOfFocusBgColor` ratio `62 / 212`).
  - Closing the top-most focused dialog restores focus step by step to the parent dialog, and finally
    back to the main menu when all dialogs are dismissed.
- **Shared layer management:** managed centrally via `gameui::windows::Windows` (`Window::SubDialog`,
  `Window::Confirmation`, `Windows::base_focus_amount`) and synchronized through `Console::sync_dialog_windows`,
  eliminating per-dialog focus checks.

## 16. Shared fade-in/out and focus-transition animations (2026-10-21)

All UI surfaces (main/sub menu, Options, sub-dialogs, developer console, confirmation boxes) share
a unified fade and focus-transition model:
- **Reference timings from game resources:** in `hl2_complete/resource/sourcescheme.res`, both
  `Frame.TransitionEffectTime` (dialog open/close fade) and `Frame.FocusTransitionEffectTime`
  (focus/out-of-focus transition) are explicitly `"0.3"` s. `Frame.BgColor` alpha is 212 and
  `Frame.OutOfFocusBgColor` is 62 (ratio ~0.292, rounded to 0.3).
- **Canvas alpha multiplier:** implemented `Canvas::set_alpha_multiplier` and `Canvas::alpha_multiplier`
  mirroring Source SDK `surface()->DrawSetAlphaMultiplier()`. Modulates all primitives (`quad`, `polygon`,
  `rectangle`, text glyphs, borders, buttons, drop-downs) without per-widget drawing modifications.
- **Window state and closing fade-out:** `gameui::windows::WindowRecord` tracks both `focus_fade` (0..1)
  and `alpha_fade` (0..1), with a `closing: bool` flag. When a dialog closes (`close`):
  - It is marked `closing = true` and fades visibility alpha from current down to 0.0 over 0.3 s.
  - Focus hands off immediately to the underlying active window or base menu; closing windows are
    ignored by `focus()`, `is_active()`, and `route()`, so they never intercept pointer or key input.
  - While fading out, the closing dialog is drawn on top with its fading composite alpha until completion,
    at which point `Windows::clean_closed()` purges it from the draw list.
- **Unified composite alpha:** each surface computes $A_{\text{comp}} = A_{\text{vis}} \times (u + (1 - u) \times F)$
  where $u = 0.3$, $A_{\text{vis}} \in [0, 1]$ is visibility alpha (opening/closing), and $F \in [0, 1]$
  is focus amount (focused vs translucent).
- **Verification:** unit-tested in `gameui::windows::tests::shared_fade_state_and_interpolation_logic_with_deterministic_timesteps`
  with deterministic timestep updates. Visual smoothness is documented as unverified, needs manual check.

## 17. Context-aware background dim overlay (menu background vs. mid-gameplay) (2026-10-21)

Dim overlay behavior distinguishes startup/menu background mode from active gameplay:
- **Rule 1: Menu background mode (startup / background map):**
  - When the main or submenu is in focus, no dim overlay is drawn (alpha 0.0); the 3D background map
    or 2D background texture remains clean and unobstructed.
  - When the menu loses focus (Options, dev console, or confirmation dialog opens), the dim overlay
    smoothly fades in over the background behind the menu buttons and dialogs up to the authentic
    `MainMenu.Backdrop` opacity (`156 / 255` ~ `0.611765` from `sourcescheme.res`) using the shared 0.3 s
    transition timing.
- **Rule 2: Mid-gameplay mode:**
  - When opening the pause menu during active gameplay, a slight dim overlay (`gameplay_dim_alpha: 0.3`
    in `assets/ui/tuning.json`, unverified fallback value) is applied over the paused 3D game world.
  - When a dialog (such as Options or dev console) opens on top of the pause menu, the overlay smoothly
    transitions from `gameplay_dim_alpha` (0.3) to the full `MainMenu.Backdrop` (0.611765) coordinated with
    the dialog focus fade. Dismissing the dialog smoothly returns the dim back to 0.3.
  - Active gameplay without the menu has no dim overlay (alpha 0.0).
- **Architecture and pure calculation:**
  - Implemented via `Console::dim_overlay_opacity(is_mid_gameplay, menu_open, base_focus)` and
    `Console::dim_overlay_alpha(time)`.
  - Drawn in `Console::draw` as a full-canvas quad behind the menu buttons and windows.
- **Verification:** unit-tested across all four scenarios and transition steps in
  `console::tests::context_aware_background_dim_overlay_across_all_four_cases`.
  Visual smoothness and gameplay dim contrast are unverified, needs manual check.

## 18. Mid-gameplay developer console and pause menu focus transition (2026-10-21)

Aligning mid-gameplay developer console activation with authentic Half-Life 2 / Source GameUI behavior:
- **Pause menu as base surface:** Invoking the developer console mid-gameplay (via the tilde / backquote
  key `toggle()`, `toggleconsole`, or `set_mode(Mode::Console)`) sets `base = Mode::Pause` if `base == Mode::Gameplay`
  and `background_level == false`. The pause menu (in-game main menu) becomes the active base surface,
  pausing and dimming gameplay, with the developer console opened and focused on top (`Mode::Console`,
  `windows.focus() == Layer::Window(Window::Console)`).
- **Focus restoration on console dismissal:** Closing the developer console mid-gameplay (pressing tilde
  again, clicking the close `[X]` button, or pressing Escape) closes the console dialog and returns focus
  to the underlying pause menu (`Mode::Pause`, `windows.focus() == Layer::Base`), in accordance with the
  single-focus model from section 15. The pause menu remains open and gameplay remains paused.
- **Resuming gameplay:** Gameplay is resumed from the pause menu either by activating the "Resume Game"
  menu row or by pressing Escape while the pause menu is focused.
- **Verification:** Unit-tested in `console::tests::mid_gameplay_tilde_dev_console_transition_and_pause_menu_focus`
  and `simulation::tests::ui_open_close_and_resume_consume_pointer_attack_and_held_jump`.

## 19. Sub-dialog interaction, keyboard defaults confirmation, and out-of-focus menu title translucency (2026-10-21)

Fixing input routing and visual defects across Options sub-dialogs and out-of-focus menu text:
- **Options sub-dialog mouse click activation:**
  - In `Console::windows_input`, previously `Window::Options`, `Window::SubDialog`, and `Window::Confirmation` were all in the window order list, and `self.options_input` was called for each. When a sub-dialog was open, `Window::Options` was passed an unowned share with no pointer coordinates, which reset the dialog's `ButtonTracker` and cleared button press state on release.
  - Now, `Console::windows_input` resolves `active_options_window` (giving precedence to `Window::Confirmation`, then `Window::SubDialog`, then `Window::Options`), and strictly routes input to `self.options_input(&share)` for the active layer.
  - In `gameui::settings::command_binding`, added `"apply"` alongside `"ok"` (mapping to `Binding::Command(Command::SubOk)`) to support `OptionsSubVideoAdvancedDlg.res` which authors the OK button with `"Command" "Apply"`.
- **Keyboard Use Defaults confirmation dialog persistence:**
  - In `OptionsDialog::input`, when `self.confirm` was handled, an unresolved outcome (`None`) previously did not restore `confirm` into `self.confirm`, causing the confirmation dialog to be discarded on the very first frame after opening. Now `confirm` is properly retained on `None`, and `closing_confirm` is set on resolution (`Some(true)` or `Some(false)`).
- **Out-of-focus main menu title and subtitle translucency:**
  - In `Console::draw`, `draw_cell_text_outlined` previously drew `TextOutline::DEFAULT` (8 dark outline glyph passes) unconditionally behind the Half-Life 2 title and expansion subtitle, even when `base_focus` was 0.0 (dialog open). This produced dark opaque silhouette blobs under translucent text.
  - Now, `title_outline` and `sub_outline` modulate their alpha with `base_focus`. When `base_focus <= 1e-4`, outline becomes `TextOutline::NONE`, emitting zero outline quads so text glyphs render at `alpha_mult` (0.3) directly over the background.
- **Focused subdialog close button highlighting on Escape:**
  - In `Console::escape_pressed`, `self.options.escape_held` now checks `matches!(self.windows.top(), Some(Window::Options | Window::SubDialog))` rather than strictly `Window::Options`. When a subdialog is focused, holding Escape down marks `escape_held = true`, rendering the subdialog's close button in the depressed, highlighted state (`ButtonState::pressed = true`) until Escape is released.
  - `Window::SubDialog` escape handling delegates to `options.escape_closes()` and `options.escape()`, cancelling active placement drags or open combo dropdowns first before dismissing the subdialog.
- **Confirmation dialog Cancel button arming on Escape:**
  - For confirmation dialogs (`MessageBox`) with a Cancel button and no title-bar close button (`!has_close && cancel.is_some()`, such as Quit, Return to Main Menu, Keep Video mode, and Options Use Defaults), pressing and holding Escape sets `escape_held = true` on the message box.
  - `MessageBox::draw` renders the Cancel button with `armed: true` and `pressed: true`, displaying the sunken `ButtonBorder::Depressed` border, 1px text offset, and depressed scheme colors until Escape is released.
  - On Escape release, `Console::escape_released` dismisses the confirmation dialog. Active placement drags on confirmation dialogs are cancelled immediately on Escape press without triggering held Escape.
- **Verification:**
  - Added unit tests:
    - `crates/hl2-ui/src/gameui/menu.rs`: `message_box_escape_held_arms_cancel_when_no_close_button` verifies `MessageBox::draw` renders depressed Cancel borders when `escape_held` is true on boxes without close button, and leaves Cancel un-depressed on boxes with close button.
    - `crates/hl2-ui/src/console.rs`:
      - `subdialog_buttons_can_be_activated_by_mouse_click`: verifies mouse click activation of OK and Cancel in `Sub::VideoAdvanced`, title bar close and bottom Close in `Sub::AudioCredits`, and OK in `Sub::Gamma`.
      - `keyboard_use_defaults_confirmation_works_with_click_and_keys`: verifies opening Use Defaults confirmation box, retaining it across frames, and activating OK restores default keybindings.
      - `out_of_focus_menu_title_and_subtitle_are_translucent_without_black_outline`: verifies no opaque text outline quads are emitted when the menu is out of focus.
      - `a_held_escape_presses_subdialog_close_button_and_release_closes_it`: verifies holding Escape highlights the subdialog close button, release closes the subdialog and restores focus to Options, and in-progress drags are cancelled immediately.
      - `a_held_escape_presses_confirmation_cancel_button_and_release_cancels_it`: verifies Escape press on `confirm_box` and `options.confirm` holds Cancel button pressed while open, Escape release cancels the dialog, and in-progress drags are cancelled immediately on press.
  - Ran `scripts/checks/validate.sh` and `scripts/checks/check_windows.sh`.

## 20. Stacked dialog translucency, content occlusion clipping, and console invocation gating (2026-10-22)

- **Stacked dialog opacity removal:**
  - Removed 100% opacity logic for stacked dialogs (`Theme::stacked()` no longer overrides `frame_bg.a = 1.0`). All dialogs out of focus or stacked over another dialog preserve their authentic translucent scheme background (`Frame.BgColor` / `Frame.OutOfFocusBgColor` from `sourcescheme.res`).
  - In `Console::window_theme`, `OptionsDialog::draw_with_occluders`, and `Console::draw`, dialogs use `Theme::focus_faded` directly without forced 100% alpha.
- **Underlying content occlusion clipping:**
  - In `crates/hl2-ui/src/canvas.rs`, added an occluder stack (`push_occluder`, `pop_occluder`, `clear_occluders`) to `Canvas`.
  - When occluders are active, `Canvas::quad` clips destination rectangles and proportionally interpolates source texture coordinates using 2D axis-aligned box subtraction (`clip_quad_against_occluder`). Portions of quads falling inside any active occluder rectangle are removed, while portions outside are preserved. Polygons with all vertices inside an active occluder are dropped. `Canvas::drain` clears active occluders between frames.
  - In `Painter` (`crates/hl2-ui/src/gameui/widgets.rs`), split `frame_spec` into `frame_background(&spec)` (which renders the rounded or rectangular frame fill and outer border) and `frame_foreground(&spec, close)` (which renders the title text and close button).
  - In `OptionsDialog::draw_with_occluders`, the Options frame background is drawn first unclipped via `p.frame_background(&spec)`. Active occluders for any overlying subdialog (`sub_bounds`), confirmation box (`confirm_bounds`), or external window are then pushed before drawing the foreground title, close button, tabs, page controls, and bottom buttons.
  - Subdialogs similarly draw their frame background unclipped via `sp.frame_background(&sub_spec)`, then push occluders for any overlying confirmation box (`confirm_bounds`) before drawing subdialog content.
  - In `Console::draw`, `draw_console_with_occluders` similarly draws the console frame background unclipped via `painter.frame_background(&spec)`, then pushes occluders for any window or confirmation box above it before drawing console history, entry, submit button, and completion popup.
  - Result: dialog frames behind a focused dialog retain their translucent background tint, but all controls, text, tabs, buttons, and borders behind the focused dialog are hidden/clipped, matching retail Source VGUI behavior.
- **Console invocation gating:**
  - In `Console::toggle()`, added a modal/subdialog guard: if any modal dialog or blocking subdialog is open (`self.windows.has_modal() || self.modal_dialog_open() || self.options.has_sub() || self.options.has_confirm()`), console toggle invocation is blocked early.
  - Pressing tilde (`KeyCode::Backquote`), activating `Action::ToggleConsole`, or executing `toggleconsole` while in Options > Video > Advanced, Gamma, Credits, Keyboard Advanced, or a confirmation dialog does not open or toggle the console, matching retail Half-Life 2 behavior.
- **Verification:**
  - Added unit tests:
    - `crates/hl2-ui/src/canvas.rs`: `occluder_clips_quads_and_proportional_texture_coordinates` tests quads outside, quads inside, partial quad splitting with proportional source UVs, and occluder clearing on drain.
    - `crates/hl2-ui/src/gameui/options.rs`: `dialogs_over_the_options_dialog_are_translucent_and_clip_underlying_content` verifies subdialogs and confirmation boxes have frame alpha < 1.0, options frame background is drawn across full bounds unclipped, and underlying options content quads behind the subdialog are clipped outside its bounds.
    - `crates/hl2-ui/src/console.rs`: `console_invocation_is_blocked_when_blocking_subdialog_or_modal_is_open` verifies that `Console::toggle()` is blocked when `Sub::VideoAdvanced`, `Command::KeyDefaults` confirmation, or Quit confirmation is open, and unblocked when they are closed.
  - Ran `scripts/checks/validate.sh` (343 hl2-ui unit tests, 93 hl2-bevy tests, strict clippy, rustfmt check) and `scripts/checks/check_windows.sh`.

## 21. Subdialog and confirmation focus styling and reversible theme transitions (2026-10-22)

- **Root cause:**
  - In `OptionsDialog::draw_with_occluders`, `base.theme` passed from `Console::draw` represented the parent Options dialog's theme, which had already faded to `Frame.OutOfFocusBgColor` (alpha 62) and `FrameTitleBar.DisabledTextColor`.
  - Calling `base.theme.focus_faded(1.0, true)` on that theme previously interpolated between `frame_bg_out_of_focus` (62) and `frame_bg` (which had already been overwritten to 62), permanently trapping subdialogs in the out-of-focus dimmed look even though they held keyboard and mouse focus.
  - Furthermore, `Console::sync_dialog_windows` computed `self.options.sub_alpha` and `confirm_alpha` but did not synchronize `sub_theme` or `confirm_theme`.
- **Reversible focus transitions on `Theme`:**
  - Added `frame_bg_focused: Color` and `title_text_focused: Color` to `Theme` (`crates/hl2-ui/src/gameui/widgets.rs`), initialized in `Theme::default()` and recorded from `sourcescheme.res` in `Theme::from_scheme()`.
  - In `Theme::focus_faded(fade, focused)`, interpolated between `frame_bg_out_of_focus` and `frame_bg_focused` (rather than mutating against current `frame_bg`), restoring `title_text_focused` when `focused` is true. Any theme can now transition to out-of-focus and back to focused idempotently without losing its authentic scheme colors.
- **Subdialog and confirmation dialog focus appearance:**
  - In `crates/hl2-ui/src/gameui/options.rs`, added `pub sub_theme: Option<Theme>` and `pub confirm_theme: Option<Theme>` to `OptionsDialog`.
  - When a subdialog is focused, its frame renders with `Frame.BgColor` (alpha 212) and `FrameTitleBar.TextColor` (white), while the underlying Options dialog renders with `Frame.OutOfFocusBgColor` (alpha 62) and `FrameTitleBar.DisabledTextColor`.
  - When an overlying confirmation dialog opens on top of a subdialog, the confirmation dialog renders with `Frame.BgColor` (alpha 212) and focused title text, and the subdialog transitions to `Frame.OutOfFocusBgColor` (alpha 62) and disabled title text.
  - In `crates/hl2-ui/src/console.rs`, synchronized `self.options.sub_theme` and `confirm_theme` in `Console::sync_dialog_windows` from `self.window_theme(Window::SubDialog, time)` and `self.window_theme(Window::Confirmation, time)`.
- **Verification:**
  - Added unit tests:
    - `widgets::tests::focus_faded_transitions_are_reversible_and_preserve_focused_colors`
    - `gameui::options::tests::dialogs_over_the_options_dialog_are_translucent_and_clip_underlying_content`
    - `console::tests::subdialog_in_focus_renders_focused_frame_while_parent_options_is_unfocused`
  - Ran `scripts/checks/validate.sh` (345 hl2-ui unit tests, 93 hl2-bevy tests, strict clippy, rustfmt check) and `scripts/checks/check_windows.sh`.

## 22. Text input with blinking cursor and parent-child focus hierarchy (2026-10-22)

- **Focus hierarchy and blinking caret models (`hl2-ui::gameui::widgets`):**
  - Added `InputFocus` (`crates/hl2-ui/src/gameui/widgets.rs`): models child-of-dialog focus relationships via `parent_focused: bool` and `child_focused: bool`. Under VGUI hierarchy rules, `is_active()` returns `true` if and only if both parent dialog and child control are focused. When a parent dialog loses focus (or has an overlying modal dialog), child inputs immediately become inactive.
  - Added `BlinkingCaret`: models the authentic Source SDK (`vgui_controls/TextEntry.cpp`) caret blink cycle (`rate: f64`, default 2.0 Hz -> 500 ms visible, 500 ms hidden). Evaluates phase toggling via `.is_multiple_of(2)`. Carets are only visible when `focus.is_active()` is `true` and the current periodic phase is on; when out of focus, carets are unconditionally hidden.
  - Added `TextInputModel`: unites `InputFocus`, `BlinkingCaret`, and optional interaction timestamp (`record_interaction` / SDK `ResetCursorBlink`). Distinguishes between `should_draw_focus_edge()` (which stays visible continuously while focused) and `should_draw_caret(time)` (which blinks over time).
  - Updated `Painter::text_entry` to accept `(focused: bool, caret_visible: bool)` and added `Painter::text_entry_model(rect, text, enabled, model, time)`.
- **Options dialog slider entry caret blinking and focus gating (`hl2-ui::gameui::options`):**
  - Added `pub parent_focused: bool` to `OptionsDialog`, synchronized by `Console::sync_dialog_windows` and `Console::draw`.
  - In `OptionsDialog::draw_with_occluders`, evaluated parent focus (`parent_focused && !modal`, false when SubDialog or Confirmation is open) and child focus (`editing.value == value`).
  - Slider text entries (e.g. Options > Mouse > Mouse sensitivity) now blink their insertion caret at 2.0 Hz when focused and editing, and completely hide their carets when the input loses focus, when Options loses focus to another window, or when an overlying subdialog or confirmation box is open.
- **Developer console text entry caret gating (`hl2-ui::console`):**
  - In `Console::draw_console_with_occluders`, determined `console_focused` (`self.windows.focus() == Layer::Window(Window::Console)`) and `entry_focused` (`!self.console_placement.active()`).
  - Gated console entry caret drawing and inner `entry_focus_edge` border with `TextInputModel`. The console caret now blinks when the developer console holds focus, and is strictly hidden whenever the console is out of focus (e.g. Options or Confirmation box is raised over it).
- **Verification:**
  - Unit tests added:
    - `widgets::tests::input_focus_parent_child_relations`: tests all focus permutations and active states.
    - `widgets::tests::blinking_caret_phase_timing_and_focus_gating`: tests 2.0 Hz 500 ms phase math and focus gating.
    - `widgets::tests::text_input_model_and_interaction_restart`: tests focus edge vs caret visibility and typing restart.
    - `widgets::tests::painter_text_entry_focus_edge_and_caret_drawing`: tests quad generation for focus edge and 1px caret.
    - `gameui::options::tests::options_slider_text_entry_cursor_blinks_when_focused_and_hides_when_unfocused`: tests mouse sensitivity slider entry blinking at 0.0s/0.5s/1.0s and hiding when unfocused, subdialog opened, or committed.
    - `console::tests::console_entry_cursor_blinks_when_focused_and_hides_when_unfocused`: tests console entry caret blinking when focused and hiding when Options or Quit confirmation is on top.
  - Ran `scripts/checks/validate.sh` (351 hl2-ui unit tests, 93 hl2-bevy tests, strict clippy, rustfmt check) and `scripts/checks/check_windows.sh`.



