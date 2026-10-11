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
from `ClientScheme.res`, the renderer-viewport resolution design (letterbox, window frame not
resized), the config file location, `input::binding` policy and the 15 ms system order.

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
| Background map | `scripts/chapterbackgrounds.txt` (chapter 1 -> `background01`) |
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
    paused behind the main menu;
  - `--position`, `--yaw`, `--pitch`, `--fly`, `--movement-script`, `--capture-live`,
    `--capture-burst` and `--capture-monitor` without a loadable `--map` exit with an error.
- New Game in the HALF-LIFE 2 category loads chapter 1 as gameplay. Escape on a background level
  closes the quit box or returns from a category, but never resumes the background into gameplay.

Placeholders (shown disabled; activating logs to the console): Load Game, Achievements, Extras.
Episode One/Two New Game are disabled because only the `hl2` chapter-1 map is mapped so far.

Limitations:

- The background map is frozen and viewed from the player spawn, not from its
  `point_viewcontrol` camera.
- There is no map-less host mode: if the background map itself fails to load, startup fails.
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
- Brightness ("Adjust brightness levels...") is enabled only in Fullscreen, following retail. It
  edits the draft and Cancel restores the previous value (retail applies immediately).
- Keyboard: rows and sections come from `hl2/scripts/kb_act.lst`, defaults from
  `hl2/cfg/config_default.cfg`. Edit Key (or Enter on a selected row) captures the next
  key or mouse button (double-clicking a row is not implemented), Escape cancels capture, assigning a key removes it from any other action,
  Clear Key unbinds, Use Defaults asks for confirmation and resets only the listed actions.
- Mouse: on macOS Raw input and Acceleration are disabled, per the `[$OSX]` conditions in the
  installed `.res` file.
- High Dynamic Range is shown on macOS too, although the retail `.res` hides it there.
- Placeholders are shown disabled and log `"<label>: not implemented in HL2-RS yet."`: Gamepad tab
  (empty page), Keyboard Advanced..., Video Advanced....

Limitations:

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
