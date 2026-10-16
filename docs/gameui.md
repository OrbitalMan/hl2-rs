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
| Game of a map | Explicit where known: a submenu loads `(game, map)` (`Effect::MainMenuMap(GameMap)`), the saved background keeps `gameplay.background_game`, level transitions stay in the running game. A bare name (`map`, `map_background`, `--map`, `--background-map`, respawn, a background saved without its game) belongs to the game whose own folder ships `maps/<name>.bsp`, loose or in its own VPKs, searching `hl2` first, then the other `gameinfo.txt` folders (`source_assets::install::game_for_map`); names typed in a session (`map`, `map_background`, respawn) search the running game before `hl2`, command-line names do not. The map is then read through that game's mount (`Vfs::mount_game`), whose order is the `SearchPaths` block of the game's own `gameinfo.txt` (`vpk::search_paths`: custom folders, own VPKs, inherited VPKs, then loose folders). Return to main menu shows the running game's chapter 1 background and saves it as the menu background. Pre-blurred `materials/console/<map>[_widescreen].vtf` images are keyed by `(game, map)`, so Half-Life 2's and Lost Coast's `background01` need no alias |
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
- Display mode offers Fullscreen / Windowed / Windowed (No Border). No Border is hidden only on
  macOS (as in the retail macOS build); a stored No Border value is kept in the file and treated as
  Fullscreen there (`DisplayMode::effective`). Windows and Linux keep borderless.
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
- Placeholders are shown disabled and log `"<label>: not implemented in HL2-RS yet."`: Gamepad tab
  (empty page).
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
  given. On macOS a stored or requested No Border resolves to full screen, as on the Video page.
  `launch-bevy-1080p.{cmd,sh}` pass `--windowed` so they still open a 1920x1080 window.

Limitations:

- Stored only, no effect yet: HDR mode, speaker setup, sound quality, captions, spoken
  language, aim assist. (Gamma is applied since 2026-10-15, see section 6.)
- Raw input has no separate effect: Bevy's accumulated mouse motion is already the raw device delta.
- The retail engine's smoothed music factor (ducking) is not modeled; only `snd_musicvolume`.
- The game pauses all sounds while it is paused or unfocused, so mute-when-unfocused is mostly
  inaudible today; it matters for sounds that keep playing (none at present).
- Full screen keeps the desktop video mode; the game frame is rendered at the configured
  resolution and scaled to it (section 10). Real monitor mode switching is not implemented.
- `--borderless` on macOS gives full screen (a full-screen window over the desktop mode).

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
  window is centred. The request is repeated while a mode transition (leaving full screen)
  finishes, and dropped once the size matches (within one point of OS rounding) or the window
  leaves windowed mode. While the window fills the display in OS full screen it is left alone, and
  the resize resumes when it returns to a normal window. Startup in windowed mode uses the same
  path, so a stored resolution larger than the monitor is clamped. Manual resizes are never
  fought: they just change the presented scale.
- **Full screen.** On Windows and Linux full screen stays `WindowMode::Fullscreen(Current,
  VideoModeSelection::Current)`: the desktop mode, no mode switch. On macOS full screen is a
  full-screen window over the desktop mode (`BorderlessFullscreen`). An exclusive mode set there can
  select an unscaled (1x) panel mode, which shrinks the desktop scaling, and so the mouse cursor,
  for the session. Stored settings are not rewritten.
- **macOS native full screen recognition (owner decision, 2026-10-15).** On macOS the game's Full
  screen mode is the native full-screen Space, so the green title-bar button is treated as a mode
  switch: `video::sync_native_fullscreen` (macOS only, PreUpdate) reads winit's real full-screen
  state from the main-thread `bevy::winit::WINIT_WINDOWS` (read-only) and, only when that state
  changes, maps it with `native_fullscreen_sync`: entering native full screen from Windowed sets
  `Window::mode` to `BorderlessFullscreen` and the display mode to Full screen; leaving it sets
  Windowed. The applied configuration is updated and saved, any pending windowed resize is
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
  replaced only on a frame when those cameras are already inactive. Rendering resumes after the
  remaining frames, once the camera target sizes have caught up, and each camera gets its previous
  activity back.
- While suspended the presentation sprite is hidden, so the window shows black for those few
  frames instead of a stretched stale frame.
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
