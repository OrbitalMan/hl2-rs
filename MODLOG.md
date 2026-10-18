# MODLOG

## 2026-10-18 Video: resolution change stress checks

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** report samples carry `frame`, the size of the game frame image being rendered (`Simulation::frame_size`, kept by the new `video::record_frame_size` after `suspend_for_resize`). New packaged replays: `test-inputs/bevy-menu-resolution-stress.json` (check `resolution_stress`, background, faked 8K monitor) and `bevy-menu-resolution-stress-windowed.json` (part of the windowed `window` check).

**Why:** owner request: resolution changes must never get stuck or crash.

**Tested how:** `resolution_stress`: Options Apply of 7680 x 4320 from the 8K list, `mat_setvideomode` 320 x 240 and 8192 x 8192, four changes on consecutive ticks mixing full screen and windowed (last wins: 1280 x 720), back to 1024 x 768; every step reached the rendered frame, ticks kept advancing, no errors or warnings in the log. Windowed `window` check on a real window: the windowed-only sequence passed (one winit warning about a destroyed window id at shutdown). All UI replays, the validation suite and the Windows-target check.

**Result:** every resolution change in these sequences is carried out and the game keeps running.

**Still broken or not tested:** exclusive full screen on a real display is not automated (it changes the desktop's display mode); no real 8K display; the 8192 x 8192 frame was only rendered on this Mac's GPU.

**Next:** none.

## 2026-10-18 Video: --fake-monitor for testing other displays; 8K checks

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** new hl2-bevy launch option `--fake-monitor WxH[,WxH...]`: the Video page lists resolutions for that panel size and those reported modes instead of the host monitor's (`gameui::display::MonitorSpec`, `Resolution: FromStr`; `video::display::modes` takes the spec). The Options report gains `resolutions` (the Resolution combo's current choices; `settings::offered_resolutions` shared with the combo). New replay `test-inputs/bevy-menu-video-8k.json`, check `video_8k`.

**Why:** owner request: show and test what an 8K display would offer.

**Tested how:** unit tests (8K panel: every aspect up to the panel, windowed lists equal full screen; spec parsing and rejection; a bad `--fake-monitor` fails the launch); packaged replay `video_8k` (16:9 list from 7680 x 4320, list open in the capture, inspected); the validation suite and the Windows-target check.

**Result:** the Video page can be checked for any display size without that display.

**Still broken or not tested:** only the list is faked: window placement and display mode changes use the real monitor; no real 8K display tested.

**Next:** none.

## 2026-10-18 Video: ultrawide aspect choices with temporary placeholder labels

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** the Aspect Ratio combo gains Ultrawide 21:9 and Super Ultrawide 32:9 (`AspectRatio::Ultrawide21x9` / `Ultrawide32x9`); `AspectRatio::of` reads its bucket bounds from `assets/display/modes.json` (16:9 up to 2.1, 21:9 below 3.0, 32:9 beyond; 4:3 and 16:10 as before). New `gameui::localize::placeholders`: `HL2RS_` token constants with temporary English text (`PLACEHOLDER_STRINGS`) for UI text the installed game has no token for, consulted after loaded resources and before `FALLBACK_STRINGS`; the console's reverse-search prompt moved there. Documented as temporary until HL2RS-owned, plugin-extensible localization resources exist.

**Why:** owner request: ultrawide aspect ratios were missing (listed under 16:9); placeholder labels kept apart and ready for a resource/plugin localization system.

**Tested how:** unit tests (bucket bounds for 21:9 and 32:9 modes, 2:1 stays 16:9, ultrawide panels' lists, placeholder tokens and resource override); the validation suite and the Windows-target check.

**Result:** ultrawide displays get their own aspect choices and resolution lists.

**Still broken or not tested:** no ultrawide display tested; ultrawide FOV framing not verified; placeholder labels are English only.

**Next:** HL2RS localization resources replacing the placeholder table.

## 2026-10-18 Video: resolution data moved to a data file

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `gameui::display` no longer holds inline resolution numbers: the standard modes, each aspect choice's exact ratio, the minimum size and the native-fit step are read from `crates/hl2-ui/assets/display/modes.json` (embedded, parsed once). `STANDARD_MODES` / `MIN_SIZE` became `standard_modes()` / `min_size()`, and `AspectRatio::exact_ratio()` gives `native_fits` its ratios; the offered modes are unchanged. The file is whitelisted in `.gitignore`.

**Why:** owner rule: no hardcoded strings or numbers in code; data goes in resource files.

**Tested how:** new unit test `mode_data_parses_and_every_entry_is_usable` (parses, every mode at least the minimum, no duplicates, every aspect ratio in its bucket); existing display tests unchanged; the validation suite (tests, strict clippy, fmt) and the Windows-target check.

**Result:** same Video page resolutions, data editable without touching code.

**Still broken or not tested:** the aspect bucket thresholds in `AspectRatio::of` (1.5, 1.6 +- 0.05) stay in code as grouping logic.

**Next:** none.

## 2026-10-18 Video: every aspect choice up to the panel's size; ultrawide resolutions

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `gameui::display::native_fits` adds, for each aspect choice, the largest exact mode of that ratio inside the monitor (multiples of 8), merged in `usable_modes`; `STANDARD_MODES` gains larger 4:3 / 5:4 / 16:10 / 16:9 modes and 21:9 / 32:9 ultrawide modes (listed under Widescreen 16:9, as retail groups them; the installed GameUI has no other aspect choice or label).

**Why:** owner report: no large 4:3 resolutions on a Retina display; modern aspect ratios missing.

**Tested how:** unit tests (Retina 3456x2234: 4:3 to 2976x2232, 16:10 to 3456x2160; ultrawide 3440x1440 and 5120x1440 panels); packaged windowed capture of the 4:3 Resolution list on the 3456x2234 panel (starts at 2976x2232). Workspace tests, strict clippy, fmt, Windows-target clippy.

**Result:** every aspect choice reaches the display's size; ultrawide panels get their usual modes.

**Still broken or not tested:** no ultrawide display tested; ultrawide picture framing (FOV) not verified; a windowed 2976x2232 window does not fully fit under the macOS menu bar (the window is placed in the visible frame as before).

**Next:** owner check.

## 2026-10-18 GameUI: Options and the console are non-modal windows with a focus look

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** new `gameui::windows` (pure model: open windows in z-order over a base, keyboard focus, pointer routing with press ownership, modal inner layers, per-window focus fade). `Console` keeps `base` (menu or gameplay) plus `Windows` instead of `previous` / `options_return`; `mode` is private (`mode()` = the focused layer, `set_mode()` opens a window or replaces the base). `Console::input` routes each frame through `Windows::route` and gives every layer its share (`Input::share`); a layer without the pointer has its tracker occluded. A press raises and focuses its window or focuses the menu; Escape closes the top window first; OPTIONS raises an open dialog as it is; Resume and a new game close the windows; a submenu background load keeps them. `modal_dialog_open` now means the quit / return box or a game loading dialog only; `modal_bounds` uses placed frames (`OptionsDialog::frame_bounds`, `Console::window_bounds`). Focus look from SDK `Frame::OnFrameFocusChanged` and the installed scheme: `Frame.BgColor` / `Frame.OutOfFocusBgColor` faded over `Frame.FocusTransitionEffectTime`, `FrameTitleBar.TextColor` / `DisabledTextColor` (`Theme::focus_faded`; stacked windows fade to opaque). The run report lists the open windows. Host writes of the mode go through `set_mode`. Replay `test-inputs/bevy-menu-options-nonmodal.json` with check `menu_options`.

**Why:** owner report: the menu could not be used while Options was open, unlike the original; the console must not block the menu or Options; dialogs show their focus as the original does. Owner asked that hit testing not regress.

**Tested how:** unit tests: `gameui::windows` (9) and console window tests (14: menu beside Options, presses dragged between layers both ways, caption drag over the menu at scale 1 and 2, a sub-dialog over the menu, the quit box over Options, OPTIONS again, Resume, console / Options overlap and raising, pointerless input, the focus fade, Escape with the menu focused, a background load behind Options); updated modality tests. Packaged: `menu_options` (beside the dialog the menu runs, under the dragged dialog NEW GAME does not start a game; a control run without the dialog starts `ep1_citadel_00` from the same click); captures of the console over Options, Options raised over it, and both out of focus with the menu focused inspected. `replays.py` all 86 checks ok (630 s, `JOBS=4`), gameplay outcomes unchanged; workspace tests, strict clippy, fmt, Windows-target clippy.

**Result:** the menu, Options and the console work side by side as in the original, each layer receiving only its share of the input.

**Still broken or not tested:** retail probably hides open windows on Resume and shows them again with the pause menu (unverified; GameUI is not in the SDK, nor is OPTIONS activating the open dialog); here they close. With the menu focused, an unfocused console over an unfocused Options shows Options through it. The console's completion list counts as inside its frame. Windows not tested.

**Next:** owner check.

## 2026-10-17 GameUI: link text inside its focus box

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Painter::url` draws the link text and underline `widgets::LINK_TEXT_INSET` (5) into the control, so the 3rd party credits links on the Audio and Video pages start inside their focus box (3 in) by `FOCUS_PAD` instead of at the control's edge, where the box crossed the first letter.

**Why:** owner report.

**Tested how:** unit test (link quads start `FOCUS_PAD` inside the focus box); packaged background captures of both links focused by keyboard.

**Result:** the link text sits evenly inside its focus box.

**Still broken or not tested:** the link text now starts 5 units right of the controls above it; Windows not checked.

**Next:** owner check.

## 2026-10-17 GameUI: slider captions, even toggle focus rings, keyboard credits

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Painter::slider` lays its Low / High captions over the drawn slider (`widgets::slider_content`), so the right caption ends with the slider instead of a nob width past it (Mouse, Gamepad, Audio, Video > Advanced). `slider_content` spans nob top to caption bottom, so the focus ring of the Video > Advanced FOV slider no longer crosses the Field of view label and value above it. `toggle_content` adds the box's vertical margin beside the box and after the label, so check box and radio rings are even. The 3rd party credits' Close holds the focus when they open, shows it after Tab and is pressed / released by Space or Enter through `focus_keys` / `KeyPress` (it closed on the Enter press, bypassing the focus model); no default border. Replays `bevy-options-capture-keyboard-fov.json` and `-keyboard-credits.json` with assertions in `replays.py` (`options`).

**Why:** owner report.

**Tested how:** unit tests (even margins around a check box, the FOV ring below the value label text, credits focus with Enter closing on release); packaged background captures of the Mouse slider, a check box, the FOV slider and the focused credits Close inspected; `replays.py` `options` ok.

**Result:** captions inside their slider, even rings, a keyboard-reachable credits Close.

**Still broken or not tested:** the FOV slider's captions read 0.00 / 1.00 (unchanged here); not checked on Windows.

**Next:** owner check.

## 2026-10-17 Checks: every gameplay fixture replayed with expected outcomes

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `scripts/checks/replays.py` runs every `test-inputs` fixture: besides the 14 UI checks, one check per gameplay fixture (70) on its map at 1280x720 with generic checks (labels at their ticks, capture size, drawn poses, I/O budget, fixture actions found their entities, model / collision / sprite / sound loading with the known installation gaps listed), specific assertions for movement, entities-weapons, door-swing, door-inputs, the effect and particle fixtures and attention, and each label's outcome compared with `test-inputs/expected/<fixture>.json` (`UPDATE_EXPECTED=1` records them). `coverage` fails for a fixture no check runs. The console check also asserts that sounds pause and resume with the UI and repeats at 1920x1080. Checks run from a copy of the packaged game in `artifacts/replays/game/` with its own default settings, so `bin/hl2-config.json` is no longer set aside and restored; `GROUP=ui|gameplay` and `JOBS=N` (parallel gameplay replays) select and speed up a run.

**Why:** owner request: maximum coverage on macOS, including the gameplay fixtures, so that merging main shows regressions.

**Tested how:** macOS (Apple Silicon): 85 checks ok in 616 s (`JOBS=4`); the outcomes recorded with `JOBS=4` reproduce in a serial run (one forward-ray difference on a tipping pallet, left out since) and a second parallel run. Stale or regression was decided with a windowless build of main running the same fixtures: main also fails the exact sprite-draw counts (particles now count, so they are lower bounds) and shows the pallet sound gap; the security-room fixtures failed only here (fixed in the previous commit). Workspace tests, strict clippy, fmt, Windows-target clippy.

**Result:** a merge of main can be checked against the recorded outcomes of every fixture.

**Still broken or not tested:** outcomes recorded on macOS; on Windows physics results may differ slightly (not run there). Three pallet fixtures report a body impact sound variant that is not preloaded (main too; listed as a known gap). The console replay's `menu-resume` step still does not resume here (Enter with no pause-menu row selected), so its resume and end-state assertions are left out. Derived variants of a few fixtures (attention targets, eye close-ups, a decal baseline image, a console panel run) have no input files and are not run.

**Next:** owner check; a Windows run (`UPDATE_EXPECTED=1` there if only physics positions differ).

## 2026-10-17 Fixture entity setup applies under a frozen view

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `simulation::Simulation::step`: while a `point_viewcontrol` holds the view (or on a menu background level), a movement script's entity setup actions (`send`, `actor_pose`, `prop_pose`; `gameplay::Action::entity_setup`) still reach the world; player controls (movement, weapons, use) stay ignored. Unit test `a_frozen_view_ignores_player_actions_but_applies_entity_setup`.

**Why:** since menu backgrounds follow their camera (0cecafe), all script actions were dropped under a frozen view. `d1_trainstation_01` starts under the intro camera, so the security-room fixtures' setup and `security_02 Start` never arrived: Barney's scenes did not run in the attention, face, gesture, audio-scene and Kleiner-monitor fixtures.

**Tested how:** the unit test fails without the change; the 11 `d1_trainstation_01` fixtures then give the same scene, gesture and I/O outcomes as main (packaged, background).

**Result:** the security-room fixtures run their scenes again.

**Still broken or not tested:** nothing known.

**Next:** none.

## 2026-10-17 Checks: cross-platform UI replays with report assertions

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `scripts/checks/replays.py` (Python, standard library) replaces `runtime_replays.sh`, `runtime_check.sh`, `runtime_capture.sh`, `runtime_resolution_change.sh` and `runtime_window_place.sh`: it builds with `build-bevy.sh` / `build-bevy.ps1`, runs `bin/hl2-bevy[.exe]` with a per-run `--report` and timeout, starts every check from default settings (the user's settings set aside and restored), asserts on the report's labelled states, and sorts checks into shared (background), windowed, macOS-only and Windows-only (`WINDOWED=1`). It adds checks for the existing console, console-map, campaign, dev-overlay and HUD replays and `bevy-save-config.json`, keyboard-focus assertions for the Options replays, and a windowed keyboard Apply of a new resolution (`test-inputs/bevy-options-keyboard-apply-resolution.json`). The run report's console section now carries the Options dialog's state (`OptionsDialog::report`: tab, sub-dialog, focused control, focus drawn, pending changes, open list, editing, key capture). Removed the early pointer-based `bevy-options.json`, `bevy-options-keyboard.json`, `bevy-switch-resolution*.json`, `bevy-switch-to-windowed.json`, `bevy-apply-fullscreen.json` (run once: none reached the Options dialog any more). The Options capture replays end with a settle step.

**Why:** owner request: cross-platform UI tests, merged with the existing replays where they overlap, preferring the more reliable checks, in shared / Windows-only / macOS-only groups.

**Tested how:** macOS (Apple Silicon): all 14 shared checks ok (~5 min, no focus taken), `window` and `window_place` ok; unit test for the Options report; workspace tests, strict clippy, fmt, Windows check. Findings: with the owner's saved settings (fast weapon switch on) the console replay's weapon selection never opened, hence default settings per check; its `menu-resume` step presses Enter with no pause-menu row selected, which does not resume on this branch (Escape clears the menu selection), so those assertions are left out.

**Result:** one cross-platform UI check script with real assertions.

**Still broken or not tested:** not run on Windows or Linux (the `no_border` Windows check has never run); the gameplay replays (movement, weapons, physics, NPCs) are not part of it.

**Next:** owner check; a Windows run.

## 2026-10-17 Checks: one script for every packaged replay and capture

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `scripts/checks/runtime_replays.sh` replaces `runtime_submenus.sh`, `runtime_return_menu.sh`, `runtime_chapter_menu.sh`, `runtime_chapter_quit.sh`, `runtime_options_capture.sh`, `runtime_captions.sh`, `runtime_placement_persist.sh`, `runtime_bg_compare.sh` and the `runtime_all.sh` runner. It builds once, backs the settings up once and restores them before each check and at the end, runs each check in the background through one shared launcher with a watchdog, checks the results itself (loaded maps in order, the saved background, captures written) and prints one ok/FAIL line per check; `ONLY=` runs a subset. New helper `scripts/checks/show_menu_background.py`. The windowed checks of the window itself stay separate scripts.

**Why:** owner request: consolidate the near-identical check scripts instead of adding a runner on top of them.

**Tested how:** all 8 checks ok in 292 s with the frontmost app sampled every 2 s (the game never frontmost); a subset run with `ONLY`.

**Result:** one script instead of nine, with real pass/fail results.

**Still broken or not tested:** not run on Windows (bash; the executable name differs there).

**Next:** owner check.

## 2026-10-17 Host: background runs for replays and captures

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `--background` runs without an operating-system window (Bevy's `WinitPlugin` disabled, `ScheduleRunnerPlugin` at 60 Hz, the window state counts as focused, implies no setup window); `--capture` (and capture bursts) read the game frame image instead of the window, `--capture-window` keeps the window screenshot. The replay and capture checks (`runtime_submenus`, `runtime_return_menu`, `runtime_chapter_menu`, `runtime_chapter_quit`, `runtime_options_capture`, `runtime_captions`, `runtime_placement_persist`, `runtime_bg_compare`) use `--background`; `runtime_capture` and `runtime_resolution_change` capture the window.

**Why:** owner request: packaged captures and replays must not take focus or be disturbed by working in other apps. A windowed `--no-focus` run still became the frontmost app at launch (winit activates the app on launch; Bevy has no setting for it).

Also fixed: the check scripts' watchdog `sleep` outlived its subshell and kept the scripts' output open, so a script whose output was piped waited out its whole watchdog (180-300 s) after the game had finished; the scripts now kill it (`pkill -P`). `runtime_submenus.sh` pointed at a doubled input name (`bevy-menu-bevy-menu-submenus.json`) since the scripts moved into the repository.

**Tested how:** frontmost app sampled every 2 s during the whole background check suite (8 scripts, 297 s; the game was never frontmost); every replay finished with exit 0 and the expected maps; captures inspected; a background and a windowed run of the same replay take the same time (6.1 s / 6.6 s); workspace tests, strict clippy, fmt, Windows check.

**Result:** replays and captures run without touching the desktop.

**Still broken or not tested:** background runs present nothing, so the window path (presentation, resizing, placement) is covered only by the windowed checks; not checked on Windows.

**Next:** owner check.

## 2026-10-17 GameUI: keyboard focus, second pass

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** Tab order in reading order (`control_order` rows by top within 12 units, then left to right); focus drawn only after Tab / Shift+Tab (clicks focus silently; message box too); padded dashed `Painter::focus_ring` around the whole check box / radio (box or circle and label), sliders (track and nob), text entries and the key list; slider track leaves a nob width free at the right (SDK `GetTrackRect`); no default button: no default border on OK / sub-dialog OK / message box buttons, Enter acts only on the focused control; the key list takes arrows and Enter only while focused, gets focus back after a capture and shows its selection dim without focus; main and pause menu hover selects a row only on entering it.

**Why:** owner review of the first pass: uneven focus boxes, click flicker, slider and text field touching, random-looking orders on Gamepad / Audio / Video, a confusing default-button border and Enter pressing OK on a focused tab, the key list's selection fighting Tab focus, and menu hover overriding the arrow keys.

**Tested how:** unit tests (reading order, focus rings, click hides focus, Enter on a tab, key list focus and dim selection, resting menu pointer, slider track geometry); owned-install `owned_install_pages_tab_in_reading_order` (Mouse, Audio, Video exact, Gamepad rows); workspace tests, strict clippy, fmt, Windows check; packaged keyboard-only captures (check box, radio, slider, entry, combo, button), with a settle step so the capture follows a presented frame.

**Result:** keyboard focus reads like the pages and looks consistent; mouse users see no focus marks.

**Still broken or not tested:** not tried by hand; the console's Submit button keeps its default border (console not changed); not checked on Windows.

**Next:** owner check.

## 2026-10-17 GameUI: keyboard focus fixes (order, focus boxes, Escape)

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `options::control_order` puts a control without `tabPosition` (or repeating an earlier one) right before the next positioned control of the layout file; check box / radio focus boxes surround the label only; focused tabs and sliders draw the dashed box; `Console::escape_pressed` / `escape_released` (host: press and release of Escape) keep a dialog's close button pressed while Escape is held and close on release, with `OptionsDialog::escape_closes` and `LineEditor::can_dismiss` deciding when Escape would close a dialog.

**Why:** owner requests: the check box focus box overlapped the box; the Mouse sensitivity slider came after its text field and the Audio mute check box second; tabs and sliders had no focus box; Escape should show the close button pressed until released.

**Tested how:** unit tests (order with an unpositioned slider and a repeated position, focus box geometry for check box / tab / slider, held Escape on the Options dialog); owned-install `owned_install_pages_keep_a_readable_tab_order` (installed Mouse, Audio, Video orders); workspace tests, strict clippy, fmt, Windows check; packaged keyboard-only captures of the focused check box and slider.

**Result:** the focus order reads like the pages, every focused control is marked, Escape closes on release.

**Still broken or not tested:** not tried by hand; the console's held-Escape look not captured; not checked on Windows.

**Next:** owner check.

## 2026-10-17 GameUI: keyboard focus in every dialog

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `layout::Control` keeps `tabPosition` / `SubTabPosition`; `console::Key::Space` and `Input::alt` (host and replay); `widgets::focus_step`, `widgets::KeyPress`, `Painter::focus_box`, focused combo / entry looks and four scheme colors (`Button.FocusBorderColor`, `TextEntry.SelectedBgColor`/`SelectedTextColor`/`FocusEdgeColor`); the Options dialog (pages, footer, tab strip, sub-dialogs) gets a focus order (`control_order`), per-control keys (`focus_keys`), click-to-focus, focus return after sub-dialogs and key capture, and draws focus and the default button; `MessageBox::keys` for the Options confirmation and the quit / return-to-menu boxes; the credits dialog takes Space. Details in `docs/gameui.md` 6c.

**Why:** owner request: every dialog element reachable and usable from the keyboard (arrows, Tab, Enter) as in the original game.

**Tested how:** unit tests (order by tab position with disabled and unpositioned controls, a full Tab cycle driving check box / slider / entry / combo / footer, Alt+Down, tab and radio arrows, held Space, sub-dialog focus return, message box focus); owned-install `hl2-ui` tests; workspace tests, strict clippy, fmt, Windows check; packaged keyboard-only replays captured (focused Resolution combo shows selected text; focused "Advanced..." shows the key-focus border and the dashed box).

**Result:** the Options dialog, its sub-dialogs and the confirmation boxes work without a mouse.

**Still broken or not tested:** retail-exact details unverified where GameUI code is not public (slider step sizes, initial focus); controls retail leaves out of the Tab order are reachable here by owner request; not tried by hand; not checked on Windows.

**Next:** owner check.

## 2026-10-17 Video: macOS windowed mode fits and centres between the menu bar and the Dock

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `video::DesktopArea` (usable area and title bar height, with `client_room`/`centered`), `window_resize_in` (fits the client area into it), `resize_window` places the window with `WindowPosition::At` in a second step once the size is applied (`GameFrame::recenter`, also at a windowed startup) when the area is known; macOS-only `read_desktop_area` reads `NSScreen.visibleFrame` and the title bar height through the window's AppKit handle while a windowed resize is pending. macOS-only dependencies on `objc2` 0.6, `objc2-app-kit` 0.3, `objc2-foundation` 0.3 and `raw-window-handle` 0.6, all already in the build (Cargo.lock gains no packages). Windows and Linux are unchanged (monitor minus `WINDOW_MARGIN`, `Centered`). New check `scripts/checks/runtime_window_place.sh`.

**Why:** owner report: a large window was centred on the whole screen and covered by the Dock.

**Tested how:** unit `windowed_mode_fits_and_centres_in_the_desktop_area` (small, Dock-limited and over-tall sizes, a left-hand Dock); packaged runs on an M1 Pro with the real window frame read back from AppKit: 2560x1600 at (224, 128) pt, bottom 960 inside 33..1054; 1024x768 at (608, 336); 3456x2160 scaled to fill 33..1054; workspace tests, strict clippy, fmt, Windows check.

**Result:** windowed windows stay clear of the menu bar and the Dock.

**Still broken or not tested:** a Dock on the left or right and multiple monitors with different scale factors were only unit-tested; a window the user moved is moved again only by the next windowed resize.

**Next:** owner check.

## 2026-10-17 Video: exclusive full screen and No Border on macOS, menu bar in the full-screen Space

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `DisplayMode::choices` offers Full screen, Windowed and Windowed (No Border) on every platform; `video::mode` maps Full screen to exclusive `WindowMode::Fullscreen(Current, VideoModeSelection::Current)` everywhere (the macOS `BorderlessFullscreen` special case is gone) and No Border to `BorderlessFullscreen`, which on macOS is the native full-screen Space; the green button now maps to No Border. The primary window sets `borderless_game: false`.

**Why:** owner requests: exclusive full screen back in the Video options without the macOS check, and the menu bar reachable in the borderless full-screen mode. Bevy's `Window::borderless_game` defaults to true, which makes winit hide the menu bar and Dock outright in borderless full screen (they only came back after changing the Space); with false, macOS keeps its standard auto-hide (the menu bar and title bar slide in at the top edge). Plain Bevy window configuration, no dependency changes.

**Tested how:** unit tests (choices on every platform, mode mapping, green-button mapping); workspace tests, strict clippy, fmt, Windows check; packaged replay at the main menu: windowed 1024x768 -> exclusive 1280x800 -> exclusive 1024x768 -> windowed, exit 0, no hang, Retina scale back after exclusive.

**Result:** macOS offers all three modes; exclusive full screen works through an aspect change.

**Still broken or not tested:** the menu bar reveal in No Border, the green button and the cursor size in exclusive mode were not tried by hand; exclusive runs the panel at scale 1 while active; not checked on Windows (mapping unchanged there).

**Next:** owner check.

## 2026-10-17 Video: the picture froze after a resolution change

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `video::suspend_for_resize` gives the game frame a new image at the new resolution and points the present material at it (the old image is released), instead of resizing the image in place; `present_frame` routes the cameras to the new image. New `ui_system::refresh_display_modes` fills the Options dialog's resolution list whenever it opens, also when a scripted replay opens it (before, only pointer/keyboard input did, so replays offered a single resolution). New check `scripts/checks/runtime_resolution_change.sh` with `test-inputs/bevy-menu-resolution-change.json`.

**Why:** owner report: after changing the resolution or aspect ratio and pressing OK/Apply the game looked stuck with the button pressed while ambient sound went on. Root cause: since the presented picture is drawn by `PresentMaterial` (brightness, fffb172) its bind group kept the frame texture it was built with; the in-place resize (c91af10) left that material showing a texture nothing rendered into any more, so every frame after a resize showed the last picture before it. Input and the console kept working (a scripted replay reached the Options dialog while the window still showed the bare main menu).

**Tested how:** replay before the fix: console mode Options, capture showed the main menu without the dialog; touching the material alone did not help; after the fix the capture shows the dialog at 1280x720. Extended `game_cameras_render_into_the_frame_at_the_configured_resolution` (new image, old one released, material and cameras on the new image). Workspace tests, strict clippy, fmt, Windows check.

**Result:** resolution and aspect changes from Options or `mat_setvideomode` show the new picture.

**Still broken or not tested:** not tried by hand with the real mouse; not checked on Windows.

**Next:** owner check.

## 2026-10-17 Repository: local check scripts in the repository, no outside references

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** the validation script, the Windows-target check, the packaged replay scripts, their report and settings helpers (`scripts/checks/`) and their replay inputs (`test-inputs/bevy-menu-*.json`, `test-inputs/bevy-options-capture*.json`) are now part of the repository, run relative to it and pass `--no-setup`; `.gitignore` admits `scripts/checks/*.sh` and `*.py`. Docs cite them by these paths (`docs/validation.md` lists them); lines that named folders, notes or scripts outside the repository were reworded. New hard rule 10 in `AGENTS.md`: nothing outside the repository is referenced; the Source SDK may be cited by its SDK-relative paths.

**Why:** owner request: maintainers who only have this repository need the checks the docs describe, and the repository must not point outside itself.

**Tested how:** `scripts/checks/validate.sh`, `scripts/checks/check_windows.sh`, `scripts/checks/runtime_check.sh` and `scripts/checks/runtime_chapter_menu.sh` run from their new place.

**Result:** every check the docs cite ships with the repository.

**Still broken or not tested:** the scripts are bash and assume `bin/hl2-bevy`; not tried on Windows. The other packaged replays were moved unchanged apart from paths and `--no-setup` and were not all rerun.

**Next:** owner check.

## 2026-10-17 GameUI: save the chapter background when a gameplay map loads

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Console::level_loaded_for_map` saves the loaded gameplay map's chapter background as the menu background (`remember_chapter_background`, written only when it changes); `menu_background_after_game` uses the same lookup.

**Why:** owner report: quitting the game mid-gameplay relaunched over the previous background, because it was saved only on Return to main menu.

**Tested how:** unit `loading_a_map_saves_its_chapter_background_for_the_menu` (saved right after the load, then shown on return); workspace tests, strict clippy, fmt; packaged `scripts/checks/runtime_chapter_quit.sh` (`--map d2_coast_01`, exit after 600 frames: saved background `hl2/background01` -> `hl2/background04`) and `scripts/checks/runtime_chapter_menu.sh` (unchanged results).

**Result:** the next launch after a mid-game quit opens over the chapter's background.

**Still broken or not tested:** not tried by hand; not checked on Windows.

**Next:** owner check.

## 2026-10-17 GameUI: main-menu background follows the chapter of the map left

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** new `hl2-ui::gameui::chapters` (chapter list from `scripts/chapterbackgrounds.txt` and `cfg/chapter<key>.cfg`, level graph from `trigger_changelevel` targets, map -> chapter assignment, `ChapterIndex` built on a background thread); `source_assets::bsp::read_entities` reads only a map's entity lump; `GameUiResources::chapters`; `Console::index_chapters` (called at startup) and `menu_background_after_game` now pick the chapter's background, falling back to chapter 1. Details in `docs/gameui.md` section 13.

**Why:** owner request: after `map d2_coast_01` and Return to main menu the menu still showed chapter 1's background; the background must come from the chapter the map belongs to, derived from game resources.

**Tested how:** unit tests (stretch and bypass assignment, changelevel targets, console return to the chapter background); owned test over all four installed games (Half-Life 2 chapters 1-13 sample maps including 9a and the `d2_coast_08` detour, Episode One, Episode Two's revisit loop, Lost Coast); workspace tests, strict clippy, Windows check, fmt; packaged replays `scripts/checks/runtime_chapter_menu.sh` (`hl2/d2_coast_01` -> `hl2/background04`, `hl2/d3_c17_10a` -> `hl2/background06`) and `scripts/checks/runtime_return_menu.sh` (unchanged: `hl2/background01`).

**Result:** leaving a map returns to its chapter's menu background and saves it.

**Still broken or not tested:** not tried by hand; maps reached only through map-spawned commands (no `trigger_changelevel`) would fall back to chapter 1; not checked on Windows.

**Next:** owner check.

## 2026-10-17 GameUI: saved Options and console placements, reset when off screen

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `config::DialogPlacements` (`dialogs.options`, `dialogs.console`: offset and, for the console, size), sanitized in `migrate`; `placement::Placement::{from_saved, saved, take_changed, reset_if_off_screen}`; `Console::load` restores both, opening the Options dialog or the console resets a placement whose saved frame no longer lies fully on the canvas, and `Console::input` saves a changed placement at the end of the frame; settings coming back from the Options dialog keep the current placements. Details in `docs/gameui.md` section 12.

**Why:** owner request: persist the Options dialog position and the console position and size, resetting them when they would open off screen after a resolution change.

**Tested how:** unit tests (save/restore round trip, an undone drag saves nothing, off-screen reset for a moved and for an oversized frame, console restart restore and small-window reset saved, corrupt settings dropped); workspace tests, strict clippy, Windows check, fmt; packaged two-run check (`scripts/checks/runtime_placement_persist.sh`): run 1 drags the Options dialog and saves its offset, run 2 opens it at the saved place.

**Result:** both dialogs come back where they were left; a saved placement that no longer fits opens at the default.

**Still broken or not tested:** not tried by hand across a real resolution change; sub-dialogs and message boxes are not saved (they open centered by design).

**Next:** owner check.

## 2026-10-17 GameUI: drag dialogs by the caption, resize the console by its corners

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** new `hl2-ui::gameui::placement` (SDK `Frame` caption and grip geometry, clamping, gestures, cursor hints); the Options dialog, its sub-dialogs and confirmation box (37c7873), the console dialog and the quit / main-menu box use it; `console::dialog::DialogLayout::{default_frame, at}` and a `MIN_SIZE` derived from the panel insets; `Console::cursor_hint`; the console font's ASCII advances are cached per size; `hl2-bevy::console_host::apply_cursor_hint` sets the window's resize cursor. Details in `docs/gameui.md` section 12.

**Why:** owner request: drag dialogs by their title bar and resize the developer console by its corners, UX-safe.

**Tested how:** unit tests (caption vs close button vs body, clamping and window shrink, oversized pinning, Escape undo, missed release, double-click reset, corner resize limits and canvas edges, stored size vs smaller canvas; Options: OK follows the moved dialog, sub-dialog moves alone and reopens centered, a caption press with a list open only closes it; console: drag, Escape, caption beats row selection, resize to the minimum, cursor hints, smaller window; quit box moves and reopens centered); workspace tests, strict clippy, Windows check, fmt; packaged replays `test-inputs/bevy-options-capture-drag.json` and `test-inputs/bevy-options-capture-console-resize.json` captured and checked by eye; resize cost measured in release (2.6 ms per frame worst case).

**Result:** every interactive dialog moves by its caption and stays on screen; the console resizes by its corners.

**Still broken or not tested:** not tried by hand with a real mouse or on Windows; placements are not saved across runs; the gameplay loading dialog does not move (transient).

**Next:** owner check.

## 2026-10-17 Console: wrapped history rows

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `console::dialog::wrap` (pure, SDK `RichText::RecalculateLineBreaks` rules: break before the overflowing word, or inside a word that started its row; newlines start rows; continuation rows skip leading spaces; at least one character a row). `Console::history_rows` turns the shown lines into rows at the history width (console font at the drawn size, a nominal monospace advance without the font, tabs 8 spaces), caching each line's rows by line number. Scrolling, the wheel, the scroll bar, Page Up, Ctrl+Home, drag selection and drawing count rows; `print` adds a scrolled-back line's row count so the view stays put. Selection and copy stay per line, so copying any row of a wrapped line gives the whole line as printed. `FontFace::advance_em`. `docs/gameui.md` section 11.

**Why:** owner report: long console lines (mirrored engine warnings with embedded newlines) were cut off at the history's edge instead of wrapped.

**Tested how:** unit tests for `wrap` (word breaks, exact fit, long words, multi-byte characters, `\n`/`\r\n`/`\r`, empty and trailing newlines, advances) and for the console (rows fit and rejoin to the line, a warning's newline starts a row, row selection copies whole lines including their newlines, timestamps wrap with the text, the filter keeps whole lines, scrolled-back view stays on its rows while wrapped output arrives, Ctrl+Home and Page Up reach the first row); hl2-ui and hl2-bevy tests, strict clippy `--workspace --all-targets -D warnings`, fmt, Windows check; packaged replay `test-inputs/bevy-options-capture-console-wrap.json` (`scripts/checks/runtime_options_capture.sh`, 1024x768): `artifacts/options_capture_console_wrap.png` shows an echoed warning-like line wrapped at word boundaries, its embedded newline starting a row, and an over-long unknown command broken inside the word.

**Result:** long and multi-line output wraps inside the history instead of being cut off.

**Still broken or not tested:** drag-select and clipboard copy of wrapped rows not exercised by hand; not compared with a retail console capture; character-level selection inside a row is still not ported.

**Next:** owner check.

## 2026-10-17 Closed captions: caption radius for sound scripts, captions-off run

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2_simulation::captions::sound_caption_attenuated`: sound-script and sound patch captions are dropped when the sound plays beyond 1200 units of the player (SDK `EmitCloseCaption`, no view-entity retry); ambient_generic captions are never attenuated (`EmitAmbientSound` passes no origins). The host audio queue measures from the player's eye. Workspace wiki and DESIGN text corrected (the view-entity retry is scene-only).

**Why:** review of the first caption commit: distant positioned sounds (NPC gunfire) captioned in HL2-RS but not in retail.

**Tested how:** unit test of the radius; hl2-ui/hl2-bevy tests, source-assets/hl2-simulation suites, strict clippy, fmt, Windows check; `scripts/checks/runtime_captions.sh` now also runs with captions off: `artifacts/captions_off.png` has no panel and the report shows 0 processed captions, while the captions-on pass still shows Kleiner's line.

**Result:** sound-script captions follow the retail caption radius; the Options setting is checked both ways in a packaged run.

**Still broken or not tested:** requests that carry only an actor entity (no origin) are not attenuated; the player's eye stands in for its origin; no packaged run exercises sound-script captions (the menu background only plays scene speech with captions).

**Next:** owner check.

## 2026-10-17 Closed captions: installed dictionaries, scene and sound captions, HUD panel

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `source_assets::captions` (compiled `VCCD` dictionaries, CRC32 token lookup, every `resource/closecaption_<language>.dat` on the search path in order, `Vfs::read_all`). `hl2_simulation::captions` (SPEAK master/slave grouping per actor channel, caption duration, `scene_maxcaptionradius` check, env_microphone speaker origins); `Scene::captions`, `Scene::scene_caption`/`microphones`, `Diagnostics::attenuated_captions`; `SoundRequest::caption` (EmitSound_t `m_bEmitCloseCaption`, off for scene speech), `Library::caption_gender`, `sounds::raw_wave` public. `hl2_ui::closecaption` (pure SDK `CHudCloseCaption` model: tags `<clr>`, `<playerclr>`, `<B>`, `<I>`, `<cr>`, `<sfx>`, `<len>`, `<delay>`, `<norepeat>`, linger/predisplay, wrap, popping, panning, box growth, item fades; `Settings::from_config` for Options > Audio; `CaptionHud` with the `HudLayout.res` `HudCloseCaption` layout, `ClientScheme.res` `CloseCaption_*` fonts resolved among the scheme's registered installed files, `Panel.BgColor`/`BgAlpha`), `WeaponHud::captions`/`draw_captions`. `hl2-bevy`: the audio queue emits captions for sound-script emits, ambient_generic plays and sound patch starts (2 s) and forwards scene captions with the actor gender; `canvas_renderer::present` draws the panel also in background levels, before GameUI; the run report gains `hud.captions`. Plan in `docs/DESIGN.md`, findings in the workspace wiki.

**Why:** owner task: real subtitles / closed captions from the installed game data, honoring the Options audio setting.

**Tested how:** unit tests (dictionary parse/lookup/order/malformed input; tag splitting, delay/len/sfx/subtitles, norepeat and same-tick repeats, gendered tokens, wrap and colors, paint growth/fades/popping, quantized durations, Options mapping, font selection; scene master/slave captions, caption radius, microphone routing, a synthetic Kleiner scene with and without its microphone); owned-install tests (Episode One dictionaries in search order, all Kleiner broadcast lines resolve, the Episode One panel layout/scheme/fonts); hl2-ui and hl2-bevy tests, source-assets and hl2-simulation suites, strict clippy `--workspace --all-targets -D warnings`, fmt, Windows check; packaged run `scripts/checks/runtime_captions.sh` on `episodic/ep1_background02` with closed captions (1024x768, 420 frames): `artifacts/captions_capture.png` shows Kleiner's `ep_01.kl_fellowcitizens06` caption in his color and italics in the box behind the main menu; report: dictionaries [3153, 3970], 1 caption processed, no attenuated captions.

**Result:** captions and subtitles show for scene speech and sound scripts from the installed dictionaries, styled by the installed HUD layout and scheme.

**Still broken or not tested:** no side-by-side comparison with the retail panel; the installed schemes ask for Tahoma/Verdana, which are not installed, so the registered DejaVu Sans files are drawn (metrics differ); VOX sentence captions (HEV), `cc_lang`, `cc_emit` and the other `cc_*` commands are not implemented; partly visible lines are not clipped to the panel; "from player" for ordinary sounds is approximated; the speaker-routed microphone sound itself is not played at the speaker; Subtitles mode and captions in gameplay maps not checked in a packaged run.

**Next:** owner check; VOX sentence captions; a retail capture of the same Kleiner line for comparison.

## 2026-10-17 GameUI: press-and-hold repeat for keys and scroll bar arrows

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `widgets::HoldRepeat` (one press/held/release repeat timer, `HOLD_REPEAT_DELAY`/`HOLD_REPEAT_INTERVAL`, formerly `SCROLL_REPEAT_*`), now behind `ScrollBarState`'s arrow repeat. `console::KeyRepeat` + `Key::repeats` + `Input::held_keys`: `Console::input` adds the held key's repeats to `Input::keys` before any layer reads them (cancelled during Keyboard-page key capture, `OptionsDialog::capturing_key`, and on mode changes). `hl2-bevy::ui_host` builds pressed and held keys from one `UI_KEYS` table. Details in `docs/gameui.md` 6a; convention in `AGENTS.md`.

**Why:** owner request: holding arrows, Backspace and similar keys should repeat everywhere it makes sense, through one clean mechanism like the scroll bar arrows.

**Tested how:** unit tests (repeat timing, release, no time no repeat; latest press wins, release stops, once-only keys cancel; held Backspace in the console, held Down on the main menu equals three presses); existing scroll bar tests unchanged; hl2-ui and hl2-bevy tests, strict clippy, fmt, Windows check.

**Result:** every UI layer that reads keys repeats them while held, with the scroll bar's timing.

**Still broken or not tested:** not tried by hand in the packaged game (replays cannot hold keys); key capture cancellation has no automated test; repeat timing is fixed, not the operating system's.

**Next:** owner check.

## 2026-10-17 Console: retail VGUI console dialog, output row selection and copy

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::console::dialog` (`DialogLayout`: the SDK `CConsoleDialog`/`CConsolePanel::PerformLayout` geometry at the retail default placement; `Selection` by line number). `Console::draw_console` now draws a `#Console_Title` frame through `Painter::frame_with_close`, a sunken `RichText.BgColor` history with a scroll bar, a sunken entry without prompt and the default `#Console_Submit` button, over what the console was opened from and without dimming the screen. `clear` and Ctrl+L share one helper that keeps line numbers counting. The wheel uses the shared `widgets::ScrollAccumulator`/`ScrollMode`. Over Options the console uses `Theme::stacked`. The completion list is as wide as its widest row. Pointer input: close ×, Submit (both capturing, release-click sound), the history scroll bar, the wheel and dragging over rows to select them (`RichText.Selected*` colors). Scrolled-back output stays put while lines arrive. `Input::copy` (Cmd+C on macOS, Ctrl+C elsewhere, `ui_host::Modifiers::copy`), `Console::take_copied`, written to Bevy `Clipboard` in `ui_system`. `Console_Submit` fallback string. History rows use a smaller console font (12 on 14-unit rows). Details in `docs/gameui.md` section 11.

**Why:** owner request: keep the Half-Life 2 look of the console; the panel was still an earlier HL2-RS layout, and output row copy was missing.

**Tested how:** unit tests (layout at 1024x768 and the minimum size, selection spans, drag selection and copy, selection stable while lines arrive and leave the buffer, copy under `con_filter_enable`/`con_timestamp`, Ctrl+L clears it, scrolled-back view stays, Submit and close clicks); hl2-ui and hl2-bevy tests, strict clippy `--workspace --all-targets -D warnings`, fmt, Windows check; packaged capture `scripts/checks/runtime_options_capture.sh` (`options_capture_console.png`: the dialog over the main menu with `map d1` and its completion list).

**Result:** the console reads as the retail VGUI console dialog and output rows can be copied.

**Still broken or not tested:** copying to the system clipboard and the drag selection not exercised by hand in the packaged game; the dialog cannot be moved or resized; selection is by whole rows, not characters; the completion list grows to its widest row but stops at the canvas edge, so a long help line can still be clipped (one letter at 1024x768).

**Next:** owner check.

## 2026-10-17 Console: completion, suggestions, history, search, editing keys, log mirroring

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::console::commands` (one descriptor table: help, argument sources, cvars), `complete` (last-command completion of names and arguments, ranking, usage help line), `editor` (line editing, completion stepping, history, Ctrl+R search); `Console` uses them, prints `Line`s with a level and time (they still read and serialize as text), draws the completion list and help line, and gains `con_filter_enable`, `con_filter_text`, `con_timestamp`; console colors come from the installed `SourceScheme.res`. `Input` gains `ctrl`, `shift`, `paste` (serde defaults: replays keep loading). `source_assets::install::installed_maps`. `hl2-bevy::console_host`: completion sources, history file, `LogPlugin` layer mirroring warnings/errors and HL2-RS info into the console; Ctrl/Shift and paste (Bevy `Clipboard`) in `ui_system`. Details in `docs/gameui.md` section 11.

**Why:** owner request: bring proven console features (autocompletion, suggestions, history, editing, logging) to HL2-RS while keeping the Half-Life 2 console's behavior and look.

**Tested how:** unit tests (descriptors, ranking, argument completion after `;`, help line, editing keys, Tab/Up/Down stepping, history on an empty line and its 100-entry cap and dedupe, Escape layering, Ctrl+R, paste, `con_*` cvars, `Line` serialization, entity names from connections, installed map listing); workspace tests, strict clippy, Windows check, fmt; packaged runtime check, submenu replay and Options replays (old scripts still load); packaged capture of `map d1` showing the help line and the map list.

**Result:** the console completes commands, cvars, maps and `ent_fire` targets/inputs as you type, keeps its history across runs and shows engine warnings.

**Still broken or not tested:** paste and the log mirroring not exercised by hand; output row selection/copy not ported; the full Source cvar set is not registered (only implemented commands complete); the console panel itself is still the earlier HL2-RS layout, not the retail VGUI console frame.

**Next:** owner check.

## 2026-10-16 GameUI: brighter check box and radio button labels at rest

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `widgets::Theme::label_text` (scheme `Label.TextColor`, OffWhite 221 by default) and `Theme::toggle_text`, shared by `Painter::check_box` and `Painter::radio`: OffWhite at rest, White (`Label.TextBrightColor`) hovered or pressed, the disabled color when disabled. Before, labels at rest used `Label.TextDullColor` (190).

**Why:** owner report: enabled, idle check box and radio labels were far too dim; hover must stay brighter.

**Tested how:** unit test (rest between dull and hover, scheme mapping); hl2-ui tests, strict clippy, Windows check; packaged capture of Keyboard > Advanced.

**Result:** labels read clearly at rest and still brighten on hover.

**Still broken or not tested:** the theme still runs on defaults (the installed `SourceSchemeBase.res` it reads is empty); the default matches the installed `sourcescheme.res` value.

**Next:** owner check.

## 2026-10-16 GameUI: every dialog stacked on the Options dialog is opaque

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `widgets::Theme::stacked` (opaque `frame_bg`); `OptionsDialog::draw` uses it for every layer over the dialog (all sub-dialogs and the confirmation box). Replaces the per-dialog `Sub::opaque` list and the credits-only special case.

**Why:** owner request: any sub-dialog over another dialog should be 100% opaque, so the controls below never show through.

**Tested how:** unit test `dialogs_over_the_options_dialog_are_opaque` (options frame translucent; brightness, both credits, both Advanced dialogs and the Use Defaults confirmation opaque); hl2-ui tests, strict clippy, Windows check; packaged capture of Keyboard > Advanced.

**Result:** stacked dialogs are opaque; the Options dialog over the game keeps the scheme's translucency.

**Still broken or not tested:** check-box labels use the scheme's dull text color unless hovered (unchanged), which reads lower-contrast on the opaque grey than it did over the translucent key list.

**Next:** owner check.

## 2026-10-16 GameUI: opaque Video > Advanced, footer reflow, active tab joins its page

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::gameui::options`: `Sub::opaque` (credits and Video > Advanced draw on an opaque frame); `footer_reflow` moves a sub-dialog footer (OK/Close buttons and the unbound caption over them) below content it overlaps and grows the frame by the same amount, used by `frame_size` and `layout_widgets`; the page border opens under the active tab. `widgets`: `TabStyle::fill` (the active tab adds no fill), `Painter::raised_open_top`.

**Why:** owner requests: Video > Advanced at 100% opacity; its elements did not fit (on macOS the installed `[$OSX]` footer rows put OK/Cancel and the caption on the Classic Effects combo); the selected tab should share the fill of the page below.

**Tested how:** unit test (macOS rows shift by 38, Windows rows by 0, footer classification); owned test (no two interactive controls overlap in the gamma, Keyboard > Advanced and Video > Advanced installed layouts); workspace tests, strict clippy, Windows check, fmt; packaged captures at 1024x768 (`scripts/checks/runtime_options_capture.sh`: Keyboard > Advanced, Video > Advanced) checked by eye.

**Result:** Video > Advanced is opaque and 396 tall on macOS with the footer below Classic Effects; Windows keeps the authored 510 layout. The active tab and its page read as one surface.

**Still broken or not tested:** Windows not run; Keyboard > Advanced and the brightness dialog stay translucent (not requested).

**Next:** owner check.

## 2026-10-16 GameUI: Gamepad tab

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::config`: `gamepad` section (`sensitivity_scale` = `sc_sensitivity_scale`, `aim_assist` = `sc_autoaim_mode`, `aim_assist_scale` = `sc_autoaim_scale`, `weapon_selection` = `hud_weaponui_gamepad` Cross/Carousel), clamped like the mouse aim-assist scale. `settings`: `Panel::Gamepad` (`optionssubgamepad.res`), `Value::JoystickScale`/`GamepadAimAssistScale` (0.5-2), `Choice::GamepadAimAssist`/`WeaponSelection`, `Command::SteamInput` (disabled placeholder). `layout`: `Control::associate`; `options::layout_widgets` disables a label whose associated control is disabled (SDK `Label`). The Gamepad tab draws its page instead of logging a placeholder.

**Why:** owner request: implement the Gamepad tab.

**Tested how:** unit test (page edits its own settings, not the mouse ones; aim-assist sensitivity and its label follow the gamepad aim assist; Steam Input logs its localized label); owned test (all five installed controls bind); config clamp test; workspace tests, strict clippy (also Windows target), fmt; packaged runtime check.

**Result:** the Gamepad tab shows the installed page and stores its settings.

**Still broken or not tested:** no gamepad input exists, so the settings have no effect; Steam Input configuration cannot open; the gamepad aim-assist default (Off) is an HL2-RS choice, not a verified retail default; not checked visually in the packaged build.

**Next:** owner check.

## 2026-10-16 GameUI: Video > Advanced dialog

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::config`: `video.advanced` (`AdvancedVideoSettings`) with list enums built by `choice_enum!` and a `Listed` trait (values in combo order, item tokens). `settings`: `Panel::VideoAdvanced`, `Choice::Advanced(AdvancedChoice)` (one arm for all thirteen combos), `Value::Fov` with `slider_range` (a layout `minvalue`/`maxvalue` wins), `Binding::ValueLabel` for `FOVValueLabel`. `layout`: `Control::range` from `minvalue`/`maxvalue`. `options`: `Sub::VideoAdvanced` (OK applies `video.advanced` and the FOV at once); combo lists opened in a sub-dialog get input before it and draw over it. `hl2-bevy::video::sync_present_mode`: the window's present mode follows V-sync (`--uncapped` wins), written only on change, separate from the resolution/mode apply.

**Why:** owner request: implement the Video/Advanced dialog.

**Tested how:** unit tests (combo item lists and edits, FOV range from the layout, combo inside the sub-dialog, OK applies, present mode); owned test (every installed combo, the FOV slider and its label bind; interactive controls inside the frame); workspace tests, strict clippy (also Windows target), fmt; packaged 300-frame runtime check.

**Result:** Video > Advanced... opens the dialog with the installed layout; V-sync and field of view apply on OK.

**Still broken or not tested:** the detail levels have no renderer effect (stored only); the antialiasing list is fixed rather than queried from the hardware; the Windows-only DirectX level combo stays empty; not checked visually in the packaged build (scripted clicks cannot reach the Options dialog); the installed `Label10` box overhangs the frame by 2 units (clipped by the frame, as authored).

**Next:** Gamepad tab.

## 2026-10-16 GameUI: Keyboard > Advanced dialog

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::config`: new `keyboard` section (`fast_switch` = `hud_fastswitch`, `developer_console` = `con_enable`, default on). `settings`: `Panel::KeyboardAdvanced` (`optionssubkeyboardadvanceddlg.res`), `Toggle::FastSwitch`/`DeveloperConsole`, `command_binding` (layout `Command` `OK`/`Close` -> sub-dialog OK/Cancel). `options`: `Sub::KeyboardAdvanced`; sub-dialogs snapshot the draft when opened and Cancel/Escape/title close restore it (replaces the gamma-only restore); an advanced dialog's OK applies its own settings at once (`Sub::apply_on_ok`, retail OK writes the cvars) and leaves other unapplied edits in the draft; frames without a frame record take the controls' extent; `frame_*` chrome controls are skipped. `Console::toggle` opens only while `con_enable` is on. `hl2-bevy::input`: weapon selection follows `keyboard.fast_switch` every frame.

**Why:** owner request: implement the Keyboard/Advanced dialog.

**Tested how:** unit tests (Cancel restores, OK applies only the dialog's settings, frame size from the layout extent, `con_enable` gating, command bindings, legacy config keeps the console); owned test (installed layout binds both check boxes and OK/Cancel by `Command`, no frame chrome, every control inside the frame); workspace tests, strict clippy (also `x86_64-pc-windows-msvc`), fmt.

**Result:** Keyboard > Advanced... opens the dialog; Fast weapon switch selects weapons directly on slot keys; clearing Enable developer console stops the console key from opening the console.

**Still broken or not tested:** not checked visually or by hand in the packaged build (scripted clicks cannot reach the dialog: opening Options pauses the ticks that drive replay scripts); the pause menu still offers its console entry while `con_enable` is off.

**Next:** Video > Advanced, Gamepad tab.

## 2026-10-16 GameUI: main-menu labels in the scheme's GorDIN face

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** new `hl2-ui::gameui::scheme_fonts`: a scheme font (`Fonts/<name>/1` face and weight) is drawn from the file the scheme registers under `CustomFontFiles` whose family matches and whose weight is closest. `Console::load` draws menu items with the installed `SourceScheme.res` `MenuLarge` (GorDIN, weight 600 -> `resource/GorDIN-Semibold.ttf`), falling back to the former Liberation Sans/GorDIN Regular files. Only installed files are read; no host fonts.

**Why:** owner report: menu letters looked slightly too wide. The menu used Liberation Sans Regular, whose capitals average 0.98 cap heights wide against 0.86 for GorDIN SemiBold (owned files), about 14% wider in the all-caps menu.

**Tested how:** unit tests (scheme font and file list parsing, family/weight choice); owned test `owned_menu_font_is_the_schemes_semibold_gordin` (resolves to GorDIN 600, capitals narrower than Liberation); hl2-ui tests, strict clippy, fmt; packaged menu capture before/after (`menu_capture.png`), compared by eye.

**Result:** menu labels draw in GorDIN SemiBold, narrower and DIN-shaped. The cell size is unchanged, so with GorDIN's taller Windows cell (1.40 em) the letters are also slightly smaller, as a GDI cell font would be.

**Still broken or not tested:** not compared with a retail capture; menu font size still comes from defaults because `SourceSchemeBase.res` (read for the theme and menu metrics) is empty; dialog fonts still use Liberation Sans.

**Next:** owner check of the menu labels.

## 2026-10-16 source-assets owned tests locate the installation like the runtime

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** the 11 owned `source-assets` tests in `animation`, `bsp`, `flexes`, `model_lighting`, `overlays`, `sentence`, `visibility` and `vtf` mount `install::discover()` instead of requiring `HL2_ROOT`, like the other owned tests. `STATUS.md` note updated.

**Why:** owned tests must find the game the way the runtime does (`HL2_ROOT` first, then Steam libraries) and never require `HL2_ROOT`.

**Tested how:** `cargo test --locked --release -p source-assets -- --ignored` with `HL2_ROOT` unset (28 passed); `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo fmt --check`.

**Still broken or not tested:** not run on Windows.

## 2026-10-16 Merge main (installation setup, arsenal and physics sessions 11-17) into `orbitalman/main-menu-2`

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** merged `main` at `6226ae7` (installation setup window, crossbow/RPG/bug bait/gravity gun, VPhysics props, gibs, studio decals, explosions, sound patches, impact-table CI fix). Conflict resolutions:
- `source-assets::vpk`: kept the `gameinfo.txt` search-path mounting (VPKs before loose folders), which also covers the zero-byte loose placeholders (`hl2/models/weapons/v_crossbow.mdl`, `v_bugbait.mdl`) that main sidestepped; the owned search-order test now asserts both weapon models resolve to non-empty data.
- `source-assets::install`: `validate_content(path, map: Option<&str>)`; the map check uses `game_for_map` and `Vfs::mount_game`, so it works for every game and is skipped when no map is requested (the main menu). `hl2-bevy::setup` passes the optional map through (`--setup-window` sends an empty map for none).
- `hl2-simulation::sounds`: one `SoundRequest` base (`From<String>`) with struct-update construction; `SoundRequest::ui` alongside main's `stop`/`patch` emitter constructors.
- `hl2-simulation::entities`: both input sets kept (`point_viewcontrol`/`env_zoom` and `AddOutput`/`env_physexplosion`).
- `hl2-bevy`: main's `movement.rs` changes ported into the split modules: viewmodel FOV and punch angles in `camera.rs` (`punched`, `viewmodel_projection`, both unit-tested; no punch while a `point_viewcontrol` holds the view), `Input::max_speed`/`surface` set per fixed step in `simulation.rs`, frozen input under a scripted camera kept. `assets::load(root, game, map)` keeps the game, with main's break/frag physics loading. `audio.rs` keeps the UI/music channels and adds the sound-patch envelopes. Studio decals forget the old map's draws when `install_map` installs a new map.
- Startup: no `--map` still opens the main menu; `--setup`/`--no-setup`/`--setup-preview` from main. Campaign tests pass `--no-setup` with a valid mock installation.
- `README.md`: setup steps from main, adjusted for the main-menu startup.

**Why:** owner request: bring main into the menu branch, resolving towards isolated, testable code and installed game resources.

**Tested how:** see `docs/validation.md` (merge entry).

**Still broken or not tested:** the setup window was not exercised on macOS; an intermittent "Entity despawned" command warning during submenu background switches (also on the pre-merge build, not located); the 11 `source-assets` owned tests still need `HL2_ROOT` instead of `install::discover()` (unchanged from both sides).

**Next:** owner check of the menu and gameplay on the merged build.

## 2026-10-16 Menu backgrounds: point_viewcontrol camera and env_zoom field of view

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-simulation`: new `view_control` (`ViewControl`, `View`, `Zoom`); `Scene::view_control`; `point_viewcontrol` `Enable`/`Disable`, `env_zoom` `Zoom`/`UnZoom` inputs; camera `wait` expiry in `Scene::tick`. `hl2-bevy::simulation`: `view_control`, `zoom_fov`, `view_origin`/`view_angles`/`view_fov`, player frozen under `TAKECONTROL`; `camera.rs`, `rendering.rs`, `effects.rs`, `audio.rs`, `monitors.rs` use the view. Checks: `scripts/checks/runtime_bg_compare.sh`, `scripts/checks/set_bg_config.py`; `scripts/checks/validate.sh` runs clippy like CI (`--workspace --all-targets -D warnings`). Docs: `docs/gameui.md`, `AGENTS.md`, `STATUS.md`.

**Why:** Owner report: on the Episode Two background the view dropped to the ground like a character; asked to match retail position, angles and FOV.

**Tested how:** `cargo test -p hl2-simulation` (171), `-p hl2-ui` (193), `-p hl2-bevy` (75); owned `hl2-bevy` (7); strict workspace clippy `--all-targets -D warnings`, `x86_64-pc-windows-msvc` clippy; fmt; `scripts/checks/runtime_submenus.sh`, `scripts/checks/runtime_return_menu.sh`; `scripts/checks/runtime_bg_compare.sh` captures compared by eye with the retail pre-blurred menu images.

**Result:** All 15 chapter backgrounds view from their camera; HL2 ones at FOV 75; captures match the retail images' framing.

**Still broken or not tested:** camera `moveto` paths, `interpolatepositiontoplayer` and eased turns not modelled (no menu background uses them); episode/Lost Coast menus use the player's FOV setting as retail does (wider with a high FOV setting); missing materials (magenta) on `ep2_background01` and Lost Coast water; not run on Windows.

**Next:** Owner check; the missing-material areas.

## 2026-10-16 Host: menu background levels run live (ambience plays behind the menu)

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-bevy::simulation`: `Simulation::background_level` (from the console each step), `world_running()`; the world/player/soundscape step uses it and ignores scripted player input on background levels; `step` is `pub(crate)` for tests. `audio.rs`, `effects.rs`, `tonemap.rs` pause on `!world_running()` instead of `paused()`. Checks: `scripts/checks/show_audio.py`, `scripts/checks/show_audio_keys.py`. Docs: `docs/gameui.md` §4 limitations, `AGENTS.md` startup invariant, `STATUS.md`.

**Why:** Owner request: background maps should play their ambient sounds like retail. Retail keeps the background level running behind the menu and only ignores player input (SDK `in_main.cpp`, `IsLevelMainMenuBackground`); HL2RS paused it like the gameplay pause menu.

**Tested how:** `cargo test -p hl2-ui` (193), `-p hl2-bevy` (75); owned `hl2-bevy` (7, incl. the new 15-background live run); strict clippy for macOS and `x86_64-pc-windows-msvc`; fmt; packaged plain launch (`scripts/checks/runtime_check.sh`) and a `d1_trainstation_02` Escape-pause run; `scripts/checks/runtime_submenus.sh`, `scripts/checks/runtime_return_menu.sh`.

**Result:** Menu over Lost Coast: soundscape active, 4 sinks playing; gameplay pause: 4/4 sinks paused. Submenu and return-to-menu replays unchanged.

**Still broken or not tested:** View is still the player spawn, not `point_viewcontrol`; not listened to by me; not run on Windows.

**Next:** Owner listening check; optionally the `point_viewcontrol` camera for background levels.

## 2026-10-16 VFS: mount gameinfo.txt search paths; Return to main menu shows the running game's background

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `source-assets::vpk`: `search_paths(root, game)` reads `<game>/gameinfo.txt` `FileSystem/SearchPaths`; `Vfs` is an ordered list of loose folders and VPKs built by `Vfs::mount_game`; `Vfs::mount` mounts `hl2`; removed `mount_expansion` (hardcoded `ep2` -> `episodic`) and the separate custom-folder pass. `hl2-ui::console`: `menu_background_after_game` for Return to main menu. `scripts/build-bevy.sh`: replace the packaged executable instead of overwriting it. Checks: `scripts/checks/runtime_return_menu.sh` + `test-inputs/bevy-menu-return.json`. Docs: `docs/gameui.md` §3, `AGENTS.md`, `STATUS.md`.

**Why:** Owner: read inheritance from `gameinfo.txt` instead of hardcoding it; owner report: after loading `d1_trainstation_02` from the Episode Two menu, Return to main menu still showed Episode Two's background.

**Tested how:** `cargo test -p source-assets` (95), `-p hl2-ui` (193), `-p hl2-bevy` (74); owned-install: `source-assets` 20/20 (with `HL2_ROOT`), `hl2-simulation` + `modkit-core` `--include-ignored`, `hl2-ui` 5, `hl2-bevy` 6; strict clippy for macOS and `x86_64-pc-windows-msvc`; fmt; packaged replays `scripts/checks/runtime_return_menu.sh` and `scripts/checks/runtime_submenus.sh`.

**Result:** Each game mounts its declared search path; the reported case returns to `hl2/background01`.

**Still broken or not tested:** Retail order now prefers VPKs over loose folders (loose `hl2/models/weapons/*.mdl` duplicates are no longer used); 11 older owned `source-assets` tests still require `HL2_ROOT`; the menu returns to chapter 1's background, not the chapter of the map played; not run on Windows.

**Next:** Owner check; optionally move the remaining `HL2_ROOT` tests to `install::discover()`.

## 2026-10-16 Host/GameUI: carry each map's game instead of guessing it from the name

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `source-assets`: `install::BASE_GAME`, `install::games`, `install::game_for_map` (the game whose own folder ships `maps/<name>.bsp`; in-session names search the running game first, command-line names start at `hl2`), `Vfs::mount_game`; removed `vpk::expansion_for_map`. `hl2-ui`: `console::GameMap`; `Effect::MainMenuMap(GameMap)`; `Console::loaded_map` is a `GameMap`, `pending_map` removed; `start_background_loading`, `level_loaded_for_map`, `background_load_failed_for_map` take the game; `Console::background`, `set_background`, `show_category` (shared by `SwitchExpansion`/`SwitchCategory`); `BackgroundTransitionState::finish_map`/`fail_map` compare `(game, map)`; `GameUiResources` keeps `install_root`, `games`, `category_game`, `main_game`, and keys `background_images` by `(game, image)` (no category or alias copies); config `gameplay.background_game`; removed `Console::expansion_category_for_map` and `start_background_loading_expansion`. `hl2-bevy`: `assets::load`/`load_with_canvas` take the game, `LoadedMap::game`; campaign requests carry a game (menu: given; transitions: running game; bare names: `game_for_map`); `MapLoaded::game`, report `loaded_game`/`last_event_game`; console line `Loaded <game>/<map>`; startup resolves `--background-map` and an unrecorded saved game from assets. Checks: `scripts/checks/runtime_submenus.sh` + `test-inputs/bevy-menu-submenus.json` cover Extras -> Lost Coast, new `scripts/checks/show_report.py`. Docs: `docs/gameui.md` §3, `AGENTS.md`, `STATUS.md`.

**Why:** Owner asked for KISS, asset-driven code without map tables; the host still guessed games from map-name prefixes and renamed Lost Coast's `background01` to an invented key.

**Tested how:** `cargo test -p hl2-ui` (192), `cargo test -p hl2-bevy` (74), `cargo test -p source-assets`, owned-install `cargo test -- --ignored` for `source-assets` (game lookup), `hl2-ui` (5) and `hl2-bevy` (6, incl. headless Episode One/Two/Lost Coast background swaps and `background01_loads_from_the_named_game`), strict clippy for macOS and `x86_64-pc-windows-msvc` (`scripts/checks/check_windows.sh`), fmt; packaged replay `scripts/checks/runtime_submenus.sh`.

**Result:** Replay loads `hl2/background01`, `episodic/ep1_background01`, `hl2/background01`, `lostcoast/background01`; final report `loaded_game: lostcoast`. A saved `ep2_background01` without a game starts on `ep2/ep2_background01`.

**Still broken or not tested:** `Vfs::mount_expansion` still hardcodes the `ep2` -> `episodic` inheritance rather than reading `gameinfo.txt`; restart into a saved Lost Coast menu (`background_game: lostcoast`) covered by the config test only, not run; gameplay `changelevel` and the `map` console command from inside an episode not exercised by hand; not checked on Windows at runtime.

**Next:** Owner check; optionally derive mounts from each game's `gameinfo.txt` search paths.

## 2026-10-16 GameUI: every game reads its own chapter files (Half-Life 2 submenu background)

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::gameui::resources`: `menu_games` (games from `gamemenu.res` `SwitchExpansion` commands), `read_game_text` (reads through the game's own mount); `GameUiResources::load` takes each game's background from its own `scripts/chapterbackgrounds.txt` and New Game map from its own `cfg/chapter1.cfg`; removed `INITIAL_BACKGROUND_MAPS`, `INITIAL_EPISODE_MAPS`, `fallback_background_maps`, `fallback_new_game_maps`. Console tests use explicit test data. New check `scripts/checks/runtime_submenus.sh` (+ `test-inputs/bevy-menu-submenus.json`). Docs: `docs/gameui.md` §3, `AGENTS.md`, `STATUS.md`.

**Why:** Owner report: selecting the Half-Life 2 submenu did not update the background. The resources were read through the startup map's mount (the saved background's game), so HL2's chapter files were the episode's.

**Tested how:** `cargo test -p hl2-ui` (189), owned-install (5, incl. the new mount test, which fails when reading through the startup mount), `cargo test -p hl2-bevy` (74), strict clippy for macOS and `x86_64-pc-windows-msvc`, fmt; packaged menu replay (HL2 now loads `background01`; before, `ep2_background01`).

**Result:** Each submenu shows its own game's chapter 1 background regardless of the saved background.

**Still broken or not tested:** Host map mounting still guesses the game from the map name; not checked by hand.

**Next:** Owner check; optionally carry the game with background requests instead of guessing from names.

## 2026-10-15 GameUI/host: brightness (mat_monitorgamma) applied, dialog unlocked

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `video::PresentMaterial` + `assets/shaders/present.wgsl` replace the present sprite (unit quad scaled by `present_frame`); `video::monitor_gamma_exponent`, `video::update_present_gamma`; `install_frame` takes mesh/material assets. hl2-ui: `OptionsDialog::preview_gamma`, `Console::display_gamma`; `settings::enabled` always enables the brightness button. New check `scripts/checks/runtime_capture.sh`. Docs: `docs/gameui.md` §6/§8, `README.md`, `STATUS.md`.

**Why:** Owner request: unlock "Adjust brightness levels..." and finish it; gamma was stored but never applied.

**Tested how:** `cargo test -p hl2-ui` (189), `cargo test -p hl2-bevy` (75 incl. the ramp test), owned-install (4), strict clippy, fmt; windowed window capture (frame presented, unchanged at 2.2).

**Result:** Brightness works in every display mode with live preview.

**Still broken or not tested:** Visual ramp at other gamma values not checked by eye; exact retail ramp formula not verified.

**Next:** Owner manual checks.

## 2026-10-15 GameUI: fainter action cell while capturing a key

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `CAPTURE_ACTION_FADE` (0.5) on the action cell's dim selection, normal text color. Docs: `docs/gameui.md` §6, `STATUS.md`.

**Why:** Owner request: more transparency on the action portion so the key cell is relatively brighter.

**Tested how:** `cargo test -p hl2-ui` (189), `cargo test -p hl2-bevy` (74), clippy, fmt.

**Result:** Action cell at 64/255 orange, key cell fully orange.

**Still broken or not tested:** Not checked by eye.

**Next:** Brightness dialog.

## 2026-10-15 GameUI: scroll bars with arrow buttons and live dragging

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `widgets::{ScrollPart, ScrollBarLayout, ScrollBarState}`, `SCROLLBAR_WIDTH` 14 -> 17 (`ScrollBar.Wide`), `SCROLL_REPEAT_DELAY`/`INTERVAL`, `Painter::scroll_bar` (arrow buttons, scheme nob), `Painter::arrow_up`, theme `scrollbar_nob`/`scrollbar_arrow`; `PopupList` embeds the bar (`pointer` takes the frame time); `OptionsDialog::key_list_bar`. Docs: `docs/gameui.md` 6c, `AGENTS.md`, `STATUS.md`.

**Why:** Owner report: the scroll strip had no up/down arrows and the view only moved after releasing a press on it.

**Tested how:** `cargo test -p hl2-ui` (189), `cargo test -p hl2-bevy` (73), strict clippy, fmt; owned-install tests; packaged 300-frame run.

**Result:** SDK-like scroll bars in the key list and all popup lists.

**Still broken or not tested:** Not checked by hand; Marlett glyphs are approximated by drawn triangles.

**Next:** Owner manual checks (STATUS.md).

## 2026-10-15 GameUI: keep the action cell dimly selected while capturing

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Theme::selection_bg_dim`/`selection_text_dim` from `SectionedListPanel.OutOfFocusSelected{Bg,Text}Color`; `draw_key_list` fills the action cell with them during capture. Docs: `docs/gameui.md` §6, `STATUS.md`.

**Why:** Owner preferred the capture variant that keeps the action portion slightly highlighted.

**Tested how:** `cargo test -p hl2-ui` (184), `cargo test -p hl2-bevy` (73), clippy, fmt.

**Result:** Dim action cell plus orange key cell while listening for a key.

**Still broken or not tested:** Not checked by hand.

**Next:** Scroll bar arrows and live drag.

## 2026-10-15 Host: fix resolution/mode apply regression; live mode in an open Options dialog

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Simulation::applied_window` (`WindowSettings`), seeded in `main.rs` from the startup options; `ApplyConfig` compares against it. `OptionsDialog::sync_display_mode`, `Console::set_display_mode_external`; `video::sync_native_fullscreen` uses them and updates `applied_window`. Docs: `docs/gameui.md` §10, `AGENTS.md`, `STATUS.md`.

**Why:** Owner report: after 5384402 resolution and window mode changes never did anything (the console had already stored the new config, so the check compared it with itself). Owner report: the macOS full screen/window mode showed in Options only after reopening the dialog.

**Tested how:** `cargo test -p hl2-ui` (184), `cargo test -p hl2-bevy` (73), strict clippy, fmt. The apply test now uses the real order and fails on the previous code.

**Result:** Resolution/mode changes apply again; unrelated settings still leave the window alone; an open dialog shows external mode changes immediately.

**Still broken or not tested:** Not tested by hand.

**Next:** macOS display modes like Windows.

## 2026-10-15 Host: recognize macOS native full screen as the Full screen mode

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `video::native_fullscreen_sync` (pure) and macOS-only `video::sync_native_fullscreen` (PreUpdate, registered under `cfg(target_os = "macos")` in `main.rs`). Docs: `docs/gameui.md` section 10, `AGENTS.md`, `README.md`, `STATUS.md`.

**Why:** Owner reports: with a Windowed setting in green-button full screen, choosing Full screen did nothing, and after leaving with the green button the game still believed it was full screen. The game never learned about OS transitions; bevy_winit only compares against winit's cached state. Owner decision: the native full-screen Space is the macOS Full screen mode, recognized both ways, mac-only.

**Tested how:** `cargo test -p hl2-ui` (183), `cargo test -p hl2-bevy` (73 passed), strict clippy, fmt; packaged 300-frame windowed main-menu run (exit 0, no transition).

**Result:** Green-button transitions update the window mode and the saved display mode; game-initiated transitions are not reverted (edge-triggered on winit state changes).

**Still broken or not tested:** Not tested by hand on macOS; Windows/Linux unaffected but not re-checked.

**Next:** Owner manual check of the macOS scenarios in STATUS.md.

## 2026-10-15 Host: apply window settings only when they change

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Simulation::console_effects` queues `pending_video_config` only if `window_settings_changed` (width, height, display mode). Docs: `docs/gameui.md` section 10, `STATUS.md`.

**Why:** Every OK/Apply re-requested the windowed size even for unrelated settings: a manually resized window snapped back, and on macOS in native full screen the picture was drawn small in the top-left corner (no `Resized` follows the request there).

**Tested how:** `cargo test -p hl2-ui` (183), `cargo test -p hl2-bevy` (72 passed), strict clippy, fmt.

**Result:** Non-video settings never touch the window.

**Still broken or not tested:** Not checked by hand; the native full screen state itself is handled in the next change.

**Next:** macOS native full screen recognition.

## 2026-10-15 GameUI: platform-specific list scrolling

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `widgets::ScrollMode`, `widgets::ScrollAccumulator`, `PopupList::wheel`; the Options key list and popup lists scroll through them. Docs: `docs/gameui.md` 6c, `STATUS.md`.

**Why:** Owner request: wheel-based scrolling on Windows, smooth trackpad scrolling on macOS. Sub-notch trackpad deltas were rounded away every frame.

**Tested how:** `cargo test -p hl2-ui` (183 passed), `cargo test -p hl2-bevy` (71 passed), clippy, fmt.

**Result:** macOS accumulates trackpad motion (row steps, owner choice over pixel-smooth); Windows/Linux keep notched scrolling.

**Still broken or not tested:** Not checked by hand on either platform; pixel-smooth scrolling not implemented.

**Next:** Skip window re-apply on non-video changes; macOS native full screen.

## 2026-10-15 GameUI: double click on a key row edits the key

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `console::Input::time` (filled from `Time<Real>` in `hl2-bevy` `update_ui`; optional, scripts unaffected); `widgets::DoubleClick` and `DOUBLE_CLICK_SECONDS`; `OptionsDialog::key_list_click` starts capture on a double click. Docs: `docs/gameui.md` section 6, `STATUS.md`.

**Why:** Owner request; the SDK key list also starts capture on a double press.

**Tested how:** `cargo test -p hl2-ui` (181 passed), `cargo test -p hl2-bevy` (71 passed), clippy, fmt.

**Result:** Double click = Edit Key; slow clicks only select.

**Still broken or not tested:** Not checked by hand; interval is a fixed 0.5 s, not the OS setting.

**Next:** Platform-specific scrolling.

## 2026-10-15 GameUI: key list group separators and key-cell capture highlight

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `OptionsDialog::draw_key_list` draws a divider under each section header (new `Theme::list_divider` from `SectionedListPanel.DividerColor`) and, while capturing, fills only the key cell. Docs: `docs/gameui.md` section 6, `STATUS.md`.

**Why:** Owner requests matching the original: separators between action groups; only the key portion highlighted while listening for a key.

**Tested how:** `cargo test -p hl2-ui` (179 passed), `cargo test -p hl2-bevy`, clippy, fmt.

**Result:** Groups are separated by a black line; capture highlights the key cell only.

**Still broken or not tested:** Not checked by hand; retail inline edit panel colors not verified.

**Next:** Double-click to edit a key.

## 2026-10-15 GameUI: opaque drop-down lists and inactive tab hover

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Painter::menu_list` base fill at full alpha; `TabStyle::hover_bg` from `Theme::tab_style` for inactive enabled hovered tabs, drawn by `Painter::tab`. Docs: `docs/gameui.md` 6a, `AGENTS.md`, `STATUS.md`.

**Why:** Owner requests: 100% opaque drop-down lists; tabs highlight on hover like dialog buttons (inactive tabs only, owner choice).

**Tested how:** `cargo test -p hl2-ui` (178 passed), `cargo test -p hl2-bevy`, clippy, fmt.

**Result:** Lists hide what is behind them; hovered inactive tabs lighten slightly.

**Still broken or not tested:** Not checked by hand in the packaged build.

**Next:** Keyboard list separators.

## 2026-10-15 GameUI: plain close cross with hover

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Painter::close_button` draws only the × glyph; new `Theme::close_style` and theme fields `close_fg`/`close_disabled` from `FrameTitleButton.FgColor`/`DisabledFgColor`. Docs: `docs/gameui.md` 6a, `AGENTS.md`, `STATUS.md`.

**Why:** Owner request: no 3D border on the dialog close button, lighten the cross on hover.

**Tested how:** `cargo test -p hl2-ui` (177 passed), `cargo test -p hl2-bevy`, clippy, fmt.

**Result:** Close buttons in all frames (Options, sub-dialogs, message boxes) use the plain cross.

**Still broken or not tested:** Hover look not checked by hand.

**Next:** Opaque drop-down lists.

## 2026-10-15 GameUI: slight visible hover on dialog push buttons (owner-approved deviation)

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `Theme::button_style` uses the scheme's `Menu.BgColor` as the hover background when the armed background equals the normal one; test `desktop_scheme_push_buttons_hover_with_the_menu_fill` replaces the no-hover test. Docs: `docs/gameui.md` 6a, `AGENTS.md`, `README.md`, `STATUS.md`.

**Why:** The owner asked for a visible hover for better usability, accepting a slight difference from retail, which shows none on desktop. The fill reuses an existing scheme color for interactive lists instead of an invented one.

**Tested how:** `cargo test -p hl2-ui` (176 passed), `cargo test -p hl2-bevy` (71 passed), strict clippy, fmt; packaged 300-frame main-menu run exit 0 on the second launch (first was SIGKILLed at launch with an empty log).

**Result:** Hovered enabled push buttons lighten slightly; pressed, disabled and tab looks are unchanged.

**Still broken or not tested:** Hover look not checked by hand in the packaged build.

**Next:** Theme loading from the non-empty `SourceScheme.res` (proposed separately).

## 2026-10-15 GameUI: keep a captured press while crossing a slider

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `OptionsDialog::track` takes its slider shortcut only when no press is captured; test added to `options_dialog_buttons_follow_press_cancel_and_release_activation`. `README.md` Options paragraph, `STATUS.md`.

**Why:** Pressing OK and dragging over the sensitivity slider reset the captured press and armed the slider, so returning to OK and releasing did nothing, contrary to the capture behavior.

**Tested how:** `cargo test -p hl2-ui` (176 passed; the new assertions fail without the fix), owned-install `-- --ignored` (4 passed), `cargo test -p hl2-bevy` (71 passed), strict clippy, fmt.

**Result:** Captured presses survive crossing any control.

**Still broken or not tested:** Not checked by hand in the packaged build.

**Next:** As in the previous entry.

## 2026-10-15 GameUI: dropdown header toggle in the release phase

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:** `hl2-ui::gameui::options` test `dropdown_open_state_toggles_on_header_and_closes_in_the_release_phase`; `docs/gameui.md` 6b, `STATUS.md`. The behavior itself is the `PopupHit::Anchor` close path of `widgets::PopupList` (previous entry).

**Why:** Clicking the header of an open dropdown must close it without a selection change and without reopening it from the same click.

**Tested how:** `cargo test -p hl2-ui` (176 passed), `cargo test -p hl2-bevy` (71 passed), strict clippy, fmt; packaged 300-frame main-menu run (exit 0).

**Result:** Open/close state machine covered: open on click, close on header, entry and outside clicks, all on release.

**Still broken or not tested:** Not checked by hand in the packaged build. SDK `ComboBox` toggles on press; release-phase toggling is the requested behavior.

**Next:** Owner decision on a visible push button hover (deviation); reading `SourceScheme.res` instead of the empty `SourceSchemeBase.res`.

## 2026-10-15 GameUI: popup lists as the topmost input layer

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:**
- `hl2-ui::gameui::widgets`: `PopupList`, `PopupHit`, `PopupEvent`; `ButtonTracker::occlude`; `ButtonTracker::reset` keeps the button edge state.
- `hl2-ui::gameui::options`: the open combo is a `PopupList`; confirmation box, key capture and open list occlude the page (`OptionsDialog::occlude`). Test fixture gains a MouseFilter box under the Auto-aim list.
- `hl2-ui::console`: `Console::input` occludes the menu tracker while a modal dialog is open.
- Docs: `docs/gameui.md` 6b, `AGENTS.md`, `STATUS.md`.

**Why:** Selecting an entry in an open dropdown also clicked the control behind it. The list closed on the press while the page tracker, not updated meanwhile, took the next held frame for a new press.

**Tested how:** `cargo test -p hl2-ui` (175 passed; the four new tests fail without the fix), `cargo test -p hl2-bevy` (71 passed), strict clippy, fmt; packaged 300-frame main-menu run (exit 0).

**Result:** No click, hover, sound or tooltip reaches controls behind an open list; outside clicks close it without activation.

**Still broken or not tested:** Click-through and hover gating not checked by hand in the packaged build. Retail may deliver an outside click to the control underneath.

**Next:** Dropdown header toggle tests and docs.

## 2026-10-15 GameUI: shared push button style, tab hover decision and drag-off capture

**Agent/model:** Claude Code desktop; claude-opus-5-5.

**Changed:**
- `hl2-ui::gameui::widgets`: `Theme` reads `Button.{,Armed,Depressed}{Text,Bg}Color`; new `Theme::button_style(ButtonState) -> ButtonStyle` (SDK `Button` precedence) used by `Painter::button`; `Theme::tab_style(active, state)` used by `Painter::tab`, which now takes a `ButtonState`; `ButtonState::depressed/hovered`; `ButtonTracker::capturing()` and `ButtonTracker::armable()`.
- `hl2-ui::gameui::options` and `menu::MessageBox`: capturing trackers; tabs pass their state to the shared tab style.
- Docs: `docs/gameui.md` 6a, `AGENTS.md`, `STATUS.md`.

**Why:** Dialog buttons and tabs had no hover look. The installed desktop scheme and the SDK show that retail has none either: armed colors equal normal ones on `[$WIN32]`, bordered buttons ignore the armed border, and tabs never arm. The look is now resolved from scheme data in one shared place, so a scheme with armed colors would show them. Drag-off previously cancelled presses on dialog buttons; SDK buttons capture the mouse and activate on release over the button after returning.

**Tested how:** `cargo test -p hl2-ui` (171 passed), `cargo test -p hl2-bevy` (71 passed), strict clippy, `cargo fmt --check`; packaged release run of 300 main-menu frames (exit 0).

**Result:** Unit-tested style resolution for normal, hover, pressed, dragged-off, disabled, default and selected/unselected tab states; capturing tracker drag-off/return.

**Still broken or not tested:** Visual hover and drag-off in the packaged build are not checked by hand. `hl2_complete/resource/SourceSchemeBase.res` is empty, so the theme runs on its defaults. No keyboard focus state for dialog controls. Tabs select on release, retail selects on press.

**Next:** Dropdown input layering.

## 2026-10-15 Code cleanup: derive blur from map names and load backgrounds directly from game resources

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::resources`:
  - Removed static `BackgroundMapReference` struct, `BACKGROUND_MAP_REFERENCES` hardcoded array, and lookup functions `background_reference_for_map`, `background_reference_for_category`, and `background_reference_for_map_and_category`.
  - Added pure function `blur(map: &str) -> &str` returning `map.trim_end_matches(".bsp")`, matching the authentic Source engine convention where console blur textures in `materials/console/` share the exact map name.
  - Added `chapter_background_maps(text: &str) -> Vec<String>` to extract all unique chapter background maps directly from mounted `scripts/chapterbackgrounds.txt` resources.
  - Updated `GameUiResources::load` to populate background maps and blur VTF textures dynamically from the game's `chapterbackgrounds.txt` files for all expansions, with automatic fallback to initial background maps.
  - Simplified `GameUiResources::background_image` to look up via `blur(key)` and category aliases; removed `background_image_for_map_and_category`.
  - Updated unit tests to verify `blur` and `chapter_background_maps`.
- `hl2-ui::console`:
  - Simplified `Console::expansion_category_for_map` to derive category from `source_assets::vpk::expansion_for_map(blur(map))`.
  - Simplified `Console::background_texture` to look up textures directly via `blur(key)` and category aliases.
  - Removed `Console::background_texture_for_map_and_category` and updated `Console::draw` to call `self.background_texture(upcoming_map, is_widescreen)`.
  - Updated unit tests to verify `background_texture` map and category lookups.
- `AGENTS.md`, `STATUS.md`:
  - Updated documentation to reflect `blur(map)` resolution and dynamic resource-based background map loading.

**Why:**
- Eliminated redundant intermediate data structures and hardcoded map reference tables by reading chapter background maps directly from authentic game resources (`chapterbackgrounds.txt`) and deriving blur image names via `blur(map)`.

**Tested how:**
- `scripts/checks/validate.sh`: 166 `hl2-ui` tests pass, 70 `hl2-bevy` tests pass; clippy clean (0 warnings), fmt clean.
- `cargo test --locked -p hl2-ui -- --ignored`: 4 owned-install tests pass.
- Release binary built via `scripts/build-bevy.sh`.

**Result:** Clean, idiomatic, dynamic background map and blur image resolution directly from game resources with zero hardcoded reference arrays.

**Still broken or not tested:** Live rendering appearance during transition is unverified headless and requires manual check.

**Next:** Await owner review or testing.

## 2026-10-15 Background blur texture matching upcoming background map and expansion archive priority

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::resources`:
  - Defined `BackgroundMapReference` and `BACKGROUND_MAP_REFERENCES` mapping all 15 background maps across `hl2`, `episodic`, `ep2`, and `lostcoast` to their authentic console VTFs, menu categories, and source expansions.
  - Added helpers `background_reference_for_map`, `background_reference_for_category`, and `background_reference_for_map_and_category`.
  - Isolated expansion VTF loading in `load_vtf_file` so each expansion strictly queries its dedicated archive and loose hierarchy (`lostcoast` searches `lostcoast_pak_dir.vpk`, `episodic` searches `ep1_pak_dir.vpk`, `ep2` searches `ep2_pak_dir.vpk`, and `hl2`/`main` strictly search `hl2_complete` and `hl2` textures/misc archives). Base `hl2` never falls through to `vfs.read()` where it could be shadowed by `search_paths[0]` if an expansion like `lostcoast` is mounted.
  - Updated `GameUiResources::load` to iterate over `BACKGROUND_MAP_REFERENCES` and load both standard and widescreen background textures for every map, populating map keys and category aliases into `res.background_images`.
  - Updated `GameUiResources::background_image` and added `background_image_for_map_and_category` supporting map name lookups.
  - Added unit test `test_background_map_references` and owned-install regression test `test_lostcoast_vfs_does_not_shadow_hl2_background`.
- `hl2-ui::console`:
  - Added `Console::expansion_category_for_map` deriving category directly from map references.
  - Added `Console::background_texture_for_map_and_category` to disambiguate shared map names (such as `background01` in HL2 vs Lost Coast) using the target category context.
  - Updated `Console::start_background_loading` to infer expansion category via `expansion_category_for_map(map)` instead of defaulting to `main_menu.category()`.
  - Updated `Console::background_texture` to look up by map name (with or without `.bsp`) as well as category name.
  - Updated `Console::draw` to look up the background texture using `upcoming_map` (`target_map` -> `pending_map` -> `loaded_map` -> category) and `target_cat`, guaranteeing the pre-blurred image always matches the upcoming background.
  - Extended `test_console_background_texture_resolution` to verify map name lookups, `expansion_category_for_map`, and category-context disambiguation.
- `AGENTS.md`, `STATUS.md`:
  - Documented `BackgroundMapReference`, expansion archive priority, and upcoming map blur texture selection.

**Why:**
- When switching submenus or loading background maps, the blur texture occasionally defaulted to the base HL2 train station image because texture lookup used the menu category instead of the upcoming map name, and Lost Coast's unique monastery cliff image was shadowed by `hl2_complete`. Furthermore, when running under a mounted Lost Coast VFS, `vfs.read` prioritizing `search_paths[0]` caused base HL2 `background01.vtf` lookups to be overwritten by Lost Coast's cliff texture. Resolving these root causes ensures the blur overlay always accurately matches the upcoming background level.

**Tested how:**
- `cargo test -p hl2-ui --lib`: 170 tests passed.
- `cargo test -p hl2-bevy --bin hl2-bevy`: 70 tests passed.
- `cargo test -p hl2-ui -- --ignored`: 4 owned-install tests passed (verifying `test_background_map_references`, distinct Lost Coast monastery image, and `test_lostcoast_vfs_does_not_shadow_hl2_background`).
- `cargo clippy -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: clean (0 warnings).
- `cargo fmt --check`: clean.

**Result:** Background transition blur texture consistently and authentically matches the upcoming background map being loaded across all expansions.

**Still broken or not tested:** Live rendering appearance during transition is unverified headless and requires manual check.

**Next:** Await owner review or testing.

## 2026-10-15 Authored pre-blurred background texture display and MainMenu.Backdrop overlay
 
**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- Research notes:
  - Documented discovery of authored pre-blurred 2D background textures (`materials/console/%s.vtf` and `_widescreen.vtf`) across `hl2_textures_dir.vpk`, `episodic/ep1_pak_dir.vpk`, `ep2/ep2_pak_dir.vpk`, and `lostcoast/lostcoast_pak_dir.vpk`.
  - Documented retail Source GameUI backdrop rendering mechanics: binding the pre-blurred VTF, drawing it full-screen, tinting with `MainMenu.Backdrop` (`rgba(0, 0, 0, 156)` per `sourcescheme.res`), and displaying `CLoadingDiscPanel` without dynamic GPU blur shaders.
- `hl2-ui::canvas`:
  - Removed obsolete synthetic blur code (`Quad.blur`, `Canvas::blur`).
- `hl2-bevy::canvas_renderer`:
  - Removed `blur: bool` from `HudMaterial` and `HudMaterialKey`.
  - Removed 32x32 synthetic blur diffusion generation and obsolete blur unit tests; added unit test `backdrop_quad_mesh_geometry` verifying backdrop quad vertex and index generation.
- `hl2-ui::gameui::resources`:
  - Added `pub background_images: BTreeMap<String, Image>` to `GameUiResources`.
  - Added `load_vtf_file` helper loading standard and widescreen background textures for `hl2`, `episodic`, `ep2`, and `lostcoast`.
  - Added method `background_image(&self, category: &str, widescreen: bool) -> Option<&Image>`.
- `hl2-ui::gameui::loading`:
  - Defined `BackgroundTransitionState::MAIN_MENU_BACKDROP = Color::from_rgba(0, 0, 0, 156)` and `BACKDROP_ALPHA = 156.0 / 255.0`.
  - Updated `LoadingDialog::draw_background_overlay(&self, canvas: &Canvas, bg_texture: Option<&Texture2D>)` to queue the textured pre-blurred background quad followed by the authentic `MainMenu.Backdrop` rectangle.
  - Added unit test `test_background_overlay_drawing_with_and_without_texture`.
- `hl2-ui::console`:
  - Added `pub background_textures: BTreeMap<String, Texture2D>` to `Console`, initialized in `Default` and populated in `set_resources` from `GameUiResources::background_images`.
  - Added method `Console::background_texture(&self, category: &str, widescreen: bool) -> Option<&Texture2D>`.
  - Updated `Console::draw` during active background transitions to draw the pre-blurred background texture quad full-screen followed by the `MainMenu.Backdrop` rectangle.
  - Added unit test `test_console_background_texture_resolution` verifying canonical expansions, category aliases (`hl2ep1`, `hl2ep2`, `hl2lc`), and aspect ratio fallback.
- `AGENTS.md`:
  - Documented authored pre-blurred background texture loading, backdrop overlay tinting, and removal of synthetic blur code.

**Why:**
- Background map loading in retail Source does not run a live GPU blur shader on the 3D scene; Valve authored pre-blurred 2D background textures for each expansion in `materials/console/`. Implementing this mechanism achieves 1:1 visual match with retail Source during background transitions while cleanly eliminating unneeded synthetic blur code and rendering hazards.

**Tested how:**
- `cargo test -p hl2-ui --lib`: 165 tests passed.
- `cargo test -p hl2-bevy --bin hl2-bevy`: 70 tests passed.
- `cargo test -p hl2-ui -- --ignored`: 3 owned-install tests passed (verifying `owned_install_resources_load` loads all authored background textures).
- `cargo test -p hl2-bevy -- --ignored`: 5 owned-install tests passed (including headless Bevy app submenu switching tests).
- `cargo clippy -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: clean (0 warnings).
- `cargo fmt --check`: clean.

**Result:** Background transition visual overlay authentically displays authored pre-blurred background textures with `MainMenu.Backdrop` dimming and corner plaque, cleanly removing all synthetic blur code.

**Still broken or not tested:** Visual appearance in live windowed/fullscreen presentation is unverified headless and requires manual check.

**Next:** All 5 tasks in the visual bug-fix pass are complete and verified.

## 2026-10-15 Submenu subtitle application timed to background map loading completion

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::loading`:
  - Extended `BackgroundTransitionState` to track `target_category: String` and `displayed_category: String` across states (`Starting { map, target_category, displayed_category }`, `Loading { map, target_category, displayed_category, elapsed }`, `Finished { map, displayed_category }`, `Failed { map, displayed_category, reason }`).
  - Added methods `start(map, category)`, `finish_map(map) -> bool`, `fail_map(map, reason) -> bool`, `target_category() -> Option<&str>`, `displayed_category(menu_category) -> &str`, and `set_displayed_category(category)`.
  - Added unit test `test_background_transition_stale_completion_ignored_and_latest_wins` verifying state tracking during sequential and rapid switches.
- `hl2-ui::console`:
  - Derived `Console::current_title()` strictly from `self.background_transition.displayed_category(menu_cat)`, establishing a single source of truth for menu subtitle state.
  - Added `Console::start_background_loading_expansion(map, category)`.
  - Added `Console::level_loaded_for_map(map, background)` and `Console::background_load_failed_for_map(map, reason)` which validate map names against the active transition target, discarding stale background events.
  - Updated menu activation outcomes (`SwitchExpansion` and `SwitchCategory`) to initiate background loading with target categories or update displayed categories immediately if no background map transition is needed.
  - Added unit test `test_subtitle_applied_after_background_loading_finishes_or_fails` verifying:
    1. Select Episode One: during loading, old title is shown; upon completion, new subtitle is applied.
    2. Select Episode Two: upon load failure, new subtitle is still applied (preventing stuck titles).
    3. Rapid double switch (Episode One then Lost Coast): stale completion for Episode One is ignored; Lost Coast subtitle applies when finished.
- `hl2-bevy::campaign`:
  - Updated `step_map_load` to invoke `ui.source.level_loaded_for_map(&loaded.world.name, job.request.background)` and `ui.source.background_load_failed_for_map(&job.request.map, &error.to_string())`.
- `AGENTS.md`: documented transition-state subtitle derivation and timing rules.
- `HL2RS/STATUS.md`: updated status to record submenu subtitle timing resolution.

**Why:**
- Displayed menu subtitles were previously updated immediately when a submenu entry was clicked, before the corresponding background map finished loading. Deriving displayed subtitles from `BackgroundTransitionState` ensures the previous subtitle remains visible during map loading, transitions cleanly upon success or failure, and ignores stale load events from superseded transitions during rapid switching.

**Tested how:**
- `cargo test -p hl2-ui --lib`: 163 tests passed (including `test_subtitle_applied_after_background_loading_finishes_or_fails` and `test_background_transition_stale_completion_ignored_and_latest_wins`).
- `cargo test -p hl2-bevy --bin hl2-bevy`: 70 tests passed.
- `cargo clippy -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: clean (0 warnings).
- `cargo fmt --check`: clean.

**Result:** Submenu subtitles transition only after background map loading completes or fails, with complete resilience against rapid switching and stale completion events.

**Still broken or not tested:** Visual transition timing in interactive gameplay is unverified and needs manual check (launch game via `/build-run`, select Episode One, confirm old title stays while loading plaque is shown, confirm Episode One subtitle appears exactly when the map finishes loading).

**Next:** Task 5 (Blur during background change).

## 2026-10-15 Submenu subtitle placement and alignment with menu button column

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::menu`:
  - Investigated subtitle placement root cause: in the 2004 base game ClientScheme, `Main.Title2.X` was 291 and `Main.Title2.Y` was 207 (for the numeral '2' logo suffix next to 'HALF-LIFE'). In the 20th Anniversary combined launcher (`hl2_complete/resource/clientscheme.res`), Valve updated `Main.Title2.X` to 76 and `Main.Title2.Y` to 172 so the expansion subtitle aligns horizontally with `Main.Menu.X` (76) and `Main.Title1.X` (76).
  - Updated `MenuLayout::apply_client_scheme` to normalize legacy logo-suffix coordinates (`title2_x >= 200.0` or diverging from `menu_x`) to `self.menu_x` (and `title2_y` to `DEFAULT_TITLE2_Y = 172.0`) so subtitles always align with the menu button column regardless of which scheme is loaded.
  - Added unit test `test_menu_title_and_subtitle_layout_alignment_across_scales_and_resolutions` verifying horizontal alignment (`title2.x == menu.x`) across scale factor 1.0 (480p, x=76), scale factor 2.0 (960p, x=152), and 1280x800 resolution (x=126), and verifying normalization of legacy schemes.
- `hl2-ui::console`:
  - Updated `Console::new` to load `SourceSchemeBase.res` and `ClientScheme.res` via `Reader::new(vfs)` instead of direct `vfs.read` calls, ensuring the `hl2_complete` anniversary scheme overlay takes precedence over base `hl2`.
- `AGENTS.md`: documented subtitle layout alignment and legacy coordinate normalization.
- `HL2RS/STATUS.md`: updated status to record submenu subtitle placement resolution.

**Why:**
- Submenu subtitles (e.g. `== episode one ==`, `== episode two ==`, `lost coasT`) were shifted far to the right across the screen (~485px at 1280x800) because `console.rs` bypassed the `hl2_complete` overlay and read the legacy 2004 HL2 `ClientScheme.res` where `Title2.X` was 291. Loading schemes via `Reader` and normalizing legacy `Title2.X` in `MenuLayout::apply_client_scheme` ensures subtitles cleanly align with the menu button column across all display modes, resolutions, and scale factors.

**Tested how:**
- `cargo test -p hl2-ui --lib`: 161 tests passed (including `test_menu_title_and_subtitle_layout_alignment_across_scales_and_resolutions`).
- `cargo test -p hl2-bevy --bin hl2-bevy`: 70 tests passed.
- `cargo clippy -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.

**Result:** Subtitles for Episode One, Episode Two, and Lost Coast align horizontally with the menu button column across all scales and resolutions.

**Still broken or not tested:** Visual match of the rendered subtitle position on screen is unverified and needs manual check (launch game via `/build-run`, navigate to Episode One, Episode Two, and Lost Coast submenus, verify subtitle starts at the exact same horizontal position as the menu buttons).

**Next:** Task 4 (Apply the subtitle after background loading finishes).

## 2026-10-15 Background loading plaque styling routed through shared dialog frame

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`:
  - Added `bg_color: Option<Color>` field to `FrameSpec` with builder method `with_bg_color`.
  - Updated `Painter::frame_spec` to use `spec.bg_color.unwrap_or(self.theme.frame_bg)` and gate title drawing on `!spec.title.is_empty()`.
- `hl2-ui::gameui::loading`:
  - Routed `draw_background_plaque` through `Painter::frame_spec(FrameSpec::new(BACKGROUND_LOADING_SIZE, "").without_close().with_bg_color(Color::new(0.0, 0.0, 0.0, 0.65)), None)` instead of manual rectangular fill and `p.raised` 3D border.
  - Added unit test `test_background_loading_plaque_styled_with_shared_frame_rounded_corners_no_3d_border` verifying corner polygon quads are emitted (rounded corners), translucent dark fill color is preserved, and no outer 3D raised border rectangles are drawn.
  - Added unit test `test_gameplay_loading_dialog_uses_shared_frame_rounded_corners_no_3d_border` asserting gameplay loading dialog routes through shared frame with rounded corners and no outer 3D border.
  - Updated existing plaque drawing tests in `loading.rs` and `console.rs` to assert rounded frame structure instead of single square quads.
- Audited all dialog-like surfaces:
  - `MessageBox` (Quit confirmation, ReturnToMainMenu confirmation, KeyDefaults confirmation): uses `p.frame_with_close` -> shared `FrameSpec` with rounded corners and no outer 3D border.
  - `OptionsDialog` (main dialog, subdialogs Gamma and Credits): uses `p.frame_with_close` / `sp.frame_with_close` -> shared `FrameSpec` with rounded corners and no outer 3D border.
  - `LoadingDialog` (gameplay map loading): uses `p.frame_with_close` -> shared `FrameSpec` with rounded corners and no outer 3D border.
  - Developer console (`Console` in `console.rs`): developer terminal panel using custom retained border lines, confirmed as developer tool surface rather than GameUI dialog.
- `AGENTS.md`: documented shared frame routing for all dialog-like surfaces.
- `HL2RS/STATUS.md`: updated status to record background loading plaque styling.

**Why:**
- The bottom-right loading plaque shown during background loading bypassed `Painter::frame_spec`, drawing a hardcoded rectangular quad and calling `p.raised` (Windows 95 outer 3D raised border). Routing it through `FrameSpec` with `FrameStyle::DEFAULT` applies the authentic rounded corners (`FRAME_CORNER_RADIUS: 5.0`) and eliminates the 3D raised border, matching the other GameUI dialogs.

**Tested how:**
- `cargo test -p hl2-ui --lib`: 160 tests passed (including `test_background_loading_plaque_styled_with_shared_frame_rounded_corners_no_3d_border`).
- `cargo test -p hl2-bevy --bin hl2-bevy`: 70 tests passed.
- `cargo clippy -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.

**Result:** Background loading plaque styling matches other GameUI dialogs (rounded corners, no outer 3D border).

**Still broken or not tested:** Visual appearance of rounded corner arcs on the background loading plaque is unverified and needs manual check (launch game, click Episode One/Two/Lost Coast submenu, observe bottom-right "LOADING..." plaque has rounded corners and no 3D border).

**Next:** Task 3 (Submenu subtitle placement).

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::viewport`:
  - Added pure function `pointer_to_canvas(cursor_pos, viewport, window_logical_size, outer_to_inner_delta) -> Option<Vec2>`.
  - Documented coordinate origin invariants: `Window::cursor_position()` provides inner client coordinates (where `(0, 0)` is the top-left of the client view, below any native OS window title bar). When outer coordinates are supplied with an `outer_to_inner_delta` (e.g. macOS title bar height), the delta is subtracted to ensure hit testing strictly occurs in client coordinates.
  - Added unit test `pointer_to_canvas_handles_scale_factors_and_outer_inner_delta` verifying 1.0 (standard DPI) and 2.0 (Retina) scaling, inner coordinates directly, outer coordinates with title-bar delta subtraction, corner hit testing, and identical conversion across windowed, borderless, and fullscreen modes.
- `hl2-bevy::ui_system`:
  - Routed `cursor_to_canvas` through `hl2_ui::viewport::pointer_to_canvas`.
  - Added unit test `cursor_to_canvas_consistent_across_window_modes_and_scale` asserting consistency across window modes and scale factors.
- `AGENTS.md`: documented 3rd-party dependency rules and cursor coordinate conversion invariants.
- `HL2RS/STATUS.md`: updated status to record unified pointer coordinate conversion and window client alignment.

**Why:**
- In macOS windowed mode, investigated cursor coordinate origin and hit testing. Verified that `winit`'s `WindowEvent::CursorMoved` and Bevy's `Window::cursor_position()` provide logical coordinates relative to the inner client view. Extracted the pointer-to-canvas coordinate mapping into a pure shared function in `hl2-ui::viewport` that handles display scale factors (1.0 and 2.0) and guarantees that any outer-to-inner window origin delta (such as a macOS window title bar) is properly normalized so UI hit-testing operates identically across windowed, borderless, and fullscreen modes.

**Tested how:**
- `cargo test -p hl2-ui --lib`: 158 tests passed (including `pointer_to_canvas_handles_scale_factors_and_outer_inner_delta`).
- `cargo test -p hl2-bevy --bin hl2-bevy`: 70 tests passed (including `cursor_to_canvas_consistent_across_window_modes_and_scale`).
- `cargo fmt --all -- --check` & `cargo clippy`: clean.

**Result:** UI pointer hit-testing operates on a unified, pure coordinate conversion across window modes and scale factors.

**Still broken or not tested:** Manual hit-testing on macOS in windowed mode (hovering and clicking all 4 corners of menu buttons) is unverified and requires manual check.

**Next:** Task 2 (Loading plaque styling).

## 2026-10-15 Background map transition state sequence and dimming effect

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::loading`:
  - Added `BackgroundTransitionState` enum with explicit states: `Idle`, `Starting { map: String }`, `Loading { map: String, elapsed: f32 }`, `Finished { map: String }`, and `Failed { map: String, reason: String }`.
  - Added methods on `BackgroundTransitionState`: `start`, `update`, `finish`, `fail`, `reset`, `is_active`, `is_terminal`, `is_failed`, `is_finished`, `target_map`, and `overlay_alpha`.
  - Updated `LoadingDialog` to embed `transition: BackgroundTransitionState`. In `new_background`, initializes transition to `Starting { map }`. In `update`, advances transition elapsed time. In `set_error`, transitions to `Failed { reason }`. In `set_complete`, transitions to `Finished`.
  - Updated `draw_background_overlay` and `draw` (corner plaque) to only emit quads while `transition.is_active()` is true. Once failed or completed, no overlay or plaque quads are drawn.
  - Added unit tests: `test_background_transition_success_sequence`, `test_background_transition_failure_sequence`, and `test_background_loading_dialog_failure_ends_transition_and_suppresses_overlay`.
- `hl2-ui::console`:
  - Added `background_transition: BackgroundTransitionState` to `Console`.
  - Updated `Console::start_background_loading` to start the transition.
  - Updated `Console::update_loading` to advance transition time.
  - Added `Console::background_load_failed(&mut self, reason: &str)` and updated `Console::loading_failed(&mut self, error: &str)` to explicitly transition to `Failed { map, reason }`, dismiss the loading dialog and plaque, and reset progress.
  - Updated `Console::loading_complete` and `Console::level_loaded` to transition active background transitions to `Finished { map }`.
  - Added `Console::background_transition_state(&self) -> &BackgroundTransitionState`.
  - Updated `Console::draw` to condition background overlay drawing on active transition state.
  - Added unit tests: `background_loading_state_sequence_success`, `background_load_failed_ends_transition_and_cleans_up`, and `generic_loading_failed_on_background_cleans_up`.
- `hl2-bevy::campaign`:
  - In `step_map_load`, updated background load failure to invoke `ui.source.background_load_failed(&error.to_string())` instead of generic `loading_complete()`, ensuring background map load failures cleanly register as `Failed` in the transition state machine while keeping the menu intact.
- `AGENTS.md`: documented background transition state machine, dimming overlay, and rendering subsystem analysis for 3D scene blur.
- `HL2RS/STATUS.md`: updated status to record background transition state sequence and failure cleanup.

**Why:**
- Screen dimming without scene blur was investigated: in `hl2-bevy`, HUD material quads sample 2D canvas textures and do not sample the active 3D framebuffer (there is no intermediate render target or blur pass). True live scene blur in Source requires multi-pass offscreen render targets (`BlurFilterX/Y`), which is part of the 3D post-processing pipeline rather than 2D VGUI. Per guidance, rather than building a new post-processing subsystem, the contained effect retains the authentic dimming overlay and non-blocking plaque while modeling the transition as an explicit, robust state sequence (start, loading, finished/failed) that properly terminates and cleans up even upon load failure.

**Tested how:**
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked`: all 157 unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked -- --ignored`: all owned install tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml --locked`: all 320+ workspace unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-bevy --locked -- campaign::tests::headless_bevy_app_submenu_entry_switches_background_map_and_scene --ignored`: passes in 2.08s.
- `cargo clippy --manifest-path HL2RS/Cargo.toml --locked --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --manifest-path HL2RS/Cargo.toml --all -- --check`: verified clean formatting.

**Result:** Background map loading transitions follow an explicit state sequence (`Idle` -> `Starting` -> `Loading` -> `Finished` / `Failed`). Failures cleanly dismiss the transition overlay and plaque.

**Still broken or not tested:** True multi-pass Gaussian framebuffer blur is not implemented and requires a future post-processing render graph pass in Bevy (limitation documented). Visual presentation on display needs manual check.

**Next:** Verification and handoff.

## 2026-10-15 Authentic episode and lost coast submenu subtitle fonts and glyph decorations

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::resources`:
  - Added `normalize_subtitle_decoration` normalizing dashes, en-dashes (`\u{2013}` / `\x96`), em-dashes (`\u{2014}`), and hyphens to the authentic Source GameUI character representations (`== episode one==`, `== episode two ==`, `lost coasT`).
  - Updated `parse_gameinfo_title` to route `title2` through `normalize_subtitle_decoration`.
  - Updated `category_title_fallback` to return authentic two-line strings with `== episode one==`, `== episode two ==`, and `lost coasT`.
  - Added unit test `test_normalize_subtitle_decoration` and updated category title tests.
  - Added `lostcoast` title assertion in `game_ui_resources_loads_from_installed_game`.
- `hl2-ui::console`:
  - Updated `Console` to load `HL2EP2.ttf` (from `resource/HL2EP2.ttf` / `hl2/resource/hl2ep2.ttf` or fallback `HALFLIFE2.ttf`) as `title_font`, and load `lostcoast.ttf` (from `lostcoast/resource/lostcoast.ttf`) as `lostcoast_title_font`.
  - Added `Console::title_font_can_render(&self, line: &str) -> bool` checking whether the title font has glyph coverage for all non-whitespace characters in a string.
  - Updated `Console::draw` to render subtitle lines using `lostcoast_title_font` (for Lost Coast) or `title_font` (for Episode One / Episode Two) at `geometry.title_tall` with `ClientTitleFont` color `rgba(255, 255, 255, 200)` and `TextOutline::DEFAULT`, falling back to `menu_font` at `geometry.title_tall * 0.55` only if a title font lacks glyphs.
  - In `HL2EP2.ttf` (and `hl2ep1.ttf`), the `'='` character maps to GID 17: a wide horizontal bar decoration rule (not hyphens `---`).
  - Added regression test `test_subtitle_font_selection_and_glyph_decoration` and updated existing title tests.
- `AGENTS.md`: documented authentic subtitle fonts and glyph decorations.
- `HL2RS/STATUS.md`: updated status with verified subtitle fonts and glyph decorations.

**Why:**
- In retail Half-Life 2 / Source GameUI, the expansion subtitles (`title2`) are drawn using `ClientTitleFont` (`HL2EP2.ttf` / `lostcoast.ttf`) rather than sans-serif `menu_font`. In `HL2EP2.ttf`, the `'='` character maps to an authentic horizontal bar decoration rule flanking the lowercase episode text, and `lostcoast.ttf` contains the stylized letters for `"lost coasT"`. HL2RS previously fell back to drawing literal `---` hyphens with `menu_font`.

**Tested how:**
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked`: all 151 unit tests pass, including `test_normalize_subtitle_decoration` and `test_subtitle_font_selection_and_glyph_decoration`.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked -- --ignored`: all owned install tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml --locked`: all 314+ workspace unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-bevy --locked -- campaign::tests::headless_bevy_app_submenu_entry_switches_background_map_and_scene --ignored`: passes in 2.03s.
- `cargo clippy --manifest-path HL2RS/Cargo.toml --locked -- -D warnings`: 0 warnings.
- `cargo fmt --manifest-path HL2RS/Cargo.toml --all -- --check`: verified clean formatting.

**Result:** Episode One, Episode Two, and Lost Coast submenu subtitles render with their authentic title fonts and horizontal bar rule decorations.

**Still broken or not tested:** Visual appearance on live display is unverified and needs manual check (launch release build and open expansion submenus).

**Next:** Task 3: Blur during background change.

## 2026-10-15 Rounded dialog frame corners and overflow clipping

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`:
  - Added named constant `FRAME_CORNER_RADIUS: f32 = 5.0` derived from Source SDK `Panel::GetCornerTextureSize` and `sourcescheme.res` `FrameBorder` (`backgroundtype 2`).
  - Added `FrameStyle` (`corner_radius`, `rounded`, `clip_overflow`, `outer_3d_border`) with `FrameStyle::DEFAULT` enabling rounding, enabling overflow clipping, and disabling outer 3D borders. Added `contains_point` for geometric hit testing inside rounded boundaries and `clip_rect` for child layout clipping.
  - Updated `FrameSpec` to include `style: FrameStyle`, with `with_style` builder and overflow-clipped `close_rect()`.
  - Added `Painter::rounded_fill(&self, rect: Rect, color: Color, radius: f32)` decomposing the rounded frame into 3 rectangular quads (top, middle, bottom) and 4 corner fan quads via `Canvas::polygon` with zero seams and zero external texture requirements.
  - Added `Painter::frame_spec(&self, spec: FrameSpec, close: Option<ButtonState>)` and updated `Painter::frame` / `Painter::frame_with_close` to route through it. Removed outer 3D raised border from dialog frames while leaving all non-dialog widgets (push buttons, close button, sliders, tabs, text inputs, checkboxes) with their authentic 3D borders.
  - Added unit test `dialog_frame_style_rounded_corners_and_overflow_clipping` verifying rounding, overflow clipping, outer 3D border removal, hit testing, and non-dialog widget border preservation.
- `hl2-ui::gameui::options`: updated subdialog credits frame quad lookups in tests to match the rounded frame decomposition.
- `AGENTS.md`: documented dialog frame rounded corners and overflow clipping.
- `HL2RS/STATUS.md`: updated status to record rounded dialog frames and outer 3D border removal.

**Why:**
- In retail Half-Life 2 / Source GameUI, dialog frames (`FrameBorder` `backgroundtype 2`) use rounded corners and do not have Windows 95-style outer 3D raised borders. Non-dialog controls keep their 3D borders.

**Tested how:**
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked`: all 149 unit tests pass, including `dialog_frame_style_rounded_corners_and_overflow_clipping`.
- `cargo test --manifest-path HL2RS/Cargo.toml --locked`: all 312+ workspace unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked -- --ignored`: owned install tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-bevy --locked -- --ignored headless_bevy_app_submenu_entry_switches_background_map_and_scene`: passes in 2.06s.
- `cargo clippy --manifest-path HL2RS/Cargo.toml --locked -- -D warnings`: 0 warnings.
- `cargo fmt --manifest-path HL2RS/Cargo.toml --all -- --check`: verified clean formatting.

**Result:** Dialogs have rounded corners, no outer 3D border, and child elements clip cleanly, while buttons, tabs, sliders, and inputs retain their 3D borders.

**Still broken or not tested:** Visual appearance on live display requires manual check.

**Next:** Task 2: Episode and Lost Coast submenu subtitles investigation and fix.

## 2026-10-14 Menu title and subtitle letter contour outline

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::hud`: added `TextOutline::DEFAULT` (`Color::new(0.0, 0.0, 0.0, 0.45)`, thickness `1.0`). In `FontFace::draw_baseline_outlined`, guarded foreground quad emission with `color.a > 0.0` so outline-only passes emit no zero-alpha foreground quads.
- `hl2-ui::console`: in `Console::draw`, applied `TextOutline::DEFAULT` to menu titles (`HALF-LIFE 2` / `HALF-LIFE'`) and expansion subtitles (`title2`, e.g. `--- Episode One ---`, `--- Episode Two ---`, `--- Lost Coast ---`) across all categories (`hl2`, `hl2ep1` / `episodic`, `hl2ep2` / `ep2`, `hl2lc` / `lostcoast`), for both `title_face` font rendering and fallback menu font rendering. For character-by-character logo rendering (`title_face` line 0), implemented two-pass layering (Pass 1 emits all 8-directional outline quads for all glyphs; Pass 2 emits all foreground quads) to ensure no character outline quad ever overlaps adjacent foreground glyphs. Added unit test `menu_titles_and_subtitles_draw_letters_with_border_along_edge_path`.
- `HL2RS/AGENTS.md`: expanded the Menu Button & Title Letter Outline convention to document title and subtitle letter contour outlines, two-pass character logo layering, and category coverage.
- `HL2RS/STATUS.md`: recorded title and subtitle letter contour outlines across main and expansion submenus.

**Why:**
- The half-life 2 title and episode/expansion subtitles in the main menu and submenus were drawn with un-outlined white / translucent white text, making them difficult to read on bright or light 3D background maps. Applying the same thin semi-transparent black letter edge contour outline provides crisp readability while preserving the clean authentic look without box or rectangular fills.

**Tested how:**
- `cargo test --manifest-path HL2RS/Cargo.toml --workspace --locked`: all 217+ workspace unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked`: all 148 unit tests pass, including `menu_titles_and_subtitles_draw_letters_with_border_along_edge_path` across categories `hl2`, `hl2ep1`, `hl2ep2`, `hl2lc`, both with and without `title_face`.
- `cargo test --manifest-path HL2RS/Cargo.toml --locked -p hl2-ui -- --ignored`: owned install tests pass.
- `cargo clippy --manifest-path HL2RS/Cargo.toml --workspace --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo fmt --manifest-path HL2RS/Cargo.toml --all -- --check`: verified clean formatting.

**Result:** Half-Life 2 titles and expansion subtitles across main and expansion submenus (`hl2`, `hl2ep1`, `hl2ep2`, `hl2lc`) render with a crisp 1px semi-transparent black outline along the letter edge paths.

**Still broken or not tested:** Physical monitor display capture.

**Next:** Ready for user review / commit.

## 2026-10-14 Menu button letter outline along glyph contours

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::hud`: added `TextOutline` struct (`color: Color`, `thickness: f32`, `is_visible()`) and implemented `FontFace::draw_baseline_outlined` and `FontFace::draw_cell_outlined` to render an outline along glyph contours. Glyphs are rasterized and looked up from the cache once, then emitted as 8-directional offset textured quads (`[-1, 0, 1]` offsets scaled by `thickness`) with `outline.color` before emitting the foreground glyphs. Added unit test `font_draw_baseline_and_cell_outlined_emits_glyph_edge_contours`.
- `hl2-ui::console`: added `draw_cell_text_outlined` helper using `TextOutline`. In `Console::draw`, updated menu button rendering (`Mode::MainMenu` and `Mode::Pause`) to draw non-spacer entry labels with `draw_cell_text_outlined` using a thin semi-transparent black outline (`TextOutline::new(border_color, 1.0)`, where `border_color` is `Color::new(0.0, 0.0, 0.0, 0.45)` default / `0.65` armed / `0.25` disabled). No rectangular bounding boxes, fills, or panels are drawn on the button clickable area. When pressed, the letter text and its contour outline shift together by `(1.0, 1.0)`. Added unit test `menu_buttons_draw_letters_with_border_along_edge_path_and_no_box_fill`.
- `HL2RS/AGENTS.md`: documented the menu button letter contour outline conventions.

**Why:**
- To improve text legibility on white, light, or bright 3D background scenes without drawing unwanted bounding boxes or background fills around the button clickable area. Rendering an 8-direction thin semi-transparent black border along the letter edge paths keeps the authentic transparent button layout while providing contrast along the character paths.

**Tested how:**
- `cargo test --manifest-path HL2RS/Cargo.toml --workspace --locked`: all 216+ workspace unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml -p hl2-ui --locked`: all 147 unit tests pass.
- `cargo test --manifest-path HL2RS/Cargo.toml --locked -p hl2-ui -- --ignored`: owned install tests pass.
- `cargo clippy --manifest-path HL2RS/Cargo.toml --workspace --all-targets --locked -- -D warnings`: 0 warnings.
- `cargo fmt --manifest-path HL2RS/Cargo.toml --all -- --check`: clean formatting.

**Result:** Menu buttons draw their letter glyphs with a crisp 1px semi-transparent black outline along the character contours with no box or rectangular background fill.

**Still broken or not tested:** Physical monitor display capture.

**Next:** Ready for commit.

## 2026-10-14 Background loading blur overlay

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::canvas`: added `blur: bool` field to `Quad` and implemented `Canvas::blur(x, y, w, h, color)` to queue a blur effect quad with an optional color tint.
- `hl2-ui::gameui::loading`: implemented `LoadingDialog::draw_background_overlay(&self, canvas: &Canvas)` emitting a full-screen `Quad::blur` with `Color::new(0.0, 0.0, 0.0, 0.45)` when `is_background()` is true. For gameplay map loading (`LoadingKind::Gameplay`), `draw_background_overlay` is a strict no-op, preserving the isolated modal progress dialog without blur. Added unit tests `background_loading_overlay_emits_blur_quad` and `gameplay_loading_overlay_does_not_emit_blur_quad`.
- `hl2-ui::console`: exposed `Console::is_background_loading(&self) -> bool` reporting active background loads. In `Console::draw`, replaced the plain rectangle backdrop with `loading.draw_background_overlay(&self.canvas)` drawn behind the main menu title and buttons. Added unit tests `background_loading_draw_emits_blur_overlay_behind_menu_and_corner_plaque`, `gameplay_loading_draw_does_not_emit_blur_overlay`, and `background_loading_completion_dismisses_blur_overlay`.
- `hl2-bevy::canvas_renderer`: added `blur: bool` to `HudMaterial` and `HudMaterialKey`. When processing a `quad.blur` quad, `Hud::material` generates and caches a 32x32 Gaussian-blurred diffusion texture uploaded with linear filtering to soften and diffuse the underlying scene pixels. Added unit test `blur_quad_mesh_geometry`.
- `HL2RS/AGENTS.md`: updated documentation on background vs gameplay loading UI/UX to specify the full-canvas blur overlay.

**Why:**
- In retail Half-Life 2, background map loading diffuses and blurs the underlying 3D scene while keeping the main menu and bottom-right corner loading plaque sharp. Gameplay map loading uses an unblurred modal dialog with a progress bar. Adding explicit blur to the background loading overlay while leaving gameplay map loading untouched matches retail UI/UX fidelity.

**Tested how:**
- `cargo test --workspace --locked`: all 214+ unit tests pass.
- `cargo test -p hl2-ui`: 145 unit tests pass, including blur overlay emission, scale handling, and gameplay loading isolation.
- `cargo test -p hl2-bevy`: 69 unit tests pass, including `blur_quad_mesh_geometry`.
- `cargo test --locked -p hl2-ui -- --ignored`: owned install tests pass.
- `cargo test --locked -p hl2-bevy -- campaign::tests::headless_bevy_app_ep2_submenu_entry_switches_background_map --ignored`: passes.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.

**Result:** Background map loading displays a blurred and darkened backdrop overlay behind sharp menu elements and the corner plaque, and dismisses cleanly upon load completion. Gameplay map loading remains completely untouched.

**Still broken or not tested:** Physical monitor display capture.

**Next:** Ready for commit.

## 2026-10-14 Episode Two background map and background loading UI/UX

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `source-assets::keyvalues`: `parse_resource` gracefully breaks when encountering `}` where a value was expected, mirroring Source SDK `KeyValues.cpp:2490` (`RecursiveLoadFromBuffer: got } in key`). This resolves parser failure on retail `ep2/ep2_pak_dir.vpk`'s `scripts/HudLayout.res` (where an unquoted `HudPoisonDamageIndicator` in `HudHealth [$DECK]` offset key-value pairs by 1, leaving `}` as value), allowing Episode Two's HUD and `ep2_background01` background scene to load cleanly. Added unit test `parse_resource_tolerates_trailing_key_before_closing_brace` and owned test `parse_ep2_hudlayout`.
- `hl2-ui::gameui::loading`: added `LoadingKind` enum (`Gameplay` vs `Background`) and `BACKGROUND_LOADING_SIZE` (130 x 36) with `background_plaque_geometry` (24px scaled margin in the bottom-right corner). In `LoadingDialog::draw`, background map loads render a non-blocking corner plaque with translucent dark fill (`rgba(0, 0, 0, 0.65)`), raised bevel border, and centered localized `#GameUI_Loading` ("LOADING..."), without progress bars. Added unit test `background_loading_plaque_geometry_and_drawing`.
- `hl2-ui::console`: added `start_background_loading`, gated `modal_dialog_open()` and `modal_bounds()` on `!loading.is_background()` so menu navigation buttons remain interactive, armed on hover, and clickable while background maps load. Added a softened backdrop scrim (`rgba(0, 0, 0, 0.45)`) rendered across the canvas during background map loading. Updated `SwitchExpansion`, `SwitchCategory`, and `map_background` to dispatch `start_background_loading`. Updated `submenu_entry_switches_background_map_and_persists` unit test.
- `hl2-bevy::campaign`: updated `load_map` to dispatch `start_background_loading` when `request.background` is true. Added headless Bevy integration test `headless_bevy_app_ep2_submenu_entry_switches_background_map` verifying submenu entry into Episode Two, asynchronous loading, and active scene swap to `ep2_background01`.
- `hl2-bevy::main`: updated startup loading to dispatch `start_background_loading` when `options.main_menu` is true.
- `HL2RS/AGENTS.md`: documented the background vs gameplay loading UI/UX conventions and the KeyValues syntax tolerance rules.

**Why:**
- In retail Half-Life 2, background map loading does not display the modal centered progress dialog; it displays a small bottom-right corner frame with "Loading..." (`#GameUI_Loading`) and dims/softens the backdrop while allowing the user to continue interacting with menu buttons. Furthermore, switching to Episode Two failed to update the background map because retail `ep2/ep2_pak_dir.vpk` contains an authentic syntax error in `scripts/HudLayout.res`, which caused `ep2_background01` loading to fail. Graceful termination per Source SDK KeyValues rules fixes the parse failure.

**Tested how:**
- `cargo test --workspace --locked`: all workspace tests pass.
- `cargo test --locked -p hl2-bevy -- --ignored headless_bevy_app`: passes (both `headless_bevy_app_ep2_submenu_entry_switches_background_map` and `headless_bevy_app_submenu_entry_switches_background_map_and_scene` pass with live owned installation).
- `cargo test --locked -p hl2-ui -- --ignored`: passes (owned resources, options, HUD).
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.

**Result:** Switching to Episode Two submenu loads `ep2_background01` and swaps the 3D scene cleanly. Background map loading displays a non-blocking corner plaque with "LOADING..." and softened backdrop, preserving menu interaction.

**Still broken or not tested:** Physical display refresh check.

**Next:** Ready for review and commit.

## 2026-10-13 Background map on submenu entry

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `source-assets::vpk`: refactored `Vfs` from separate flat `roots` and `vpks` lists into structured `SearchPath { root: PathBuf, archives: Vec<VpkArchive> }` layers. `Vfs::read` and `resolve_material_name` now evaluate each search path's loose directory and its associated VPK archives before falling through to subsequent search paths, ensuring expansion mounts (`episodic`, `ep2`, `lostcoast`) accurately override base `hl2` loose files.
- `hl2-ui::gameui::resources`: canonicalized Lost Coast default/resolved background map to `"lostcoast_background01"` to prevent collision with base Half-Life 2's `hl2/maps/background01.bsp`.
- `source-assets::vpk`: updated `expansion_for_map` to map `"lostcoast_background01"` to `Some("lostcoast")`.
- `hl2-bevy::assets`: updated `assets::load_with_canvas` to map canonical `"lostcoast_background01"` to on-disk map `"background01"` while preserving world name as `"lostcoast_background01"`.
- `hl2-ui::console`: updated `Console::activate_menu` to handle `MenuOutcome::SwitchExpansion(game)` via `self.gameui.background_map_for(&game)`, emitting `Effect::MainMenuMap` and updating/persisting `config.gameplay.background_map`. Preserved current background map when navigating Back or pressing Escape. Made `Console::activate_menu` public and exposed `menu_category(&self) -> &str`.
- Tests: added headless Bevy end-to-end tests in `hl2-bevy::campaign`:
  - `campaign::tests::headless_bevy_app_submenu_entry_switches_background_map_and_scene` (`#[ignore = "requires owned HL2 installation"]`): verifies entering Episode One submenu triggers background state change and `MainMenuMap("ep1_background01")`, drives async loader until `campaign.loaded_map == Some("ep1_background01")` and asserts old scene entities despawned and new scene entities spawned with `MapOwned`. Then switches to Lost Coast submenu and verifies second asynchronous scene swap to `lostcoast_background01`.
  - `campaign::tests::headless_bevy_app_background_map_failure_logs_warning_and_does_not_crash`: headless Bevy app test with mock installation verifying that background map load failure logs a warning to `campaign.failures` and console output, clears loading state, and never interrupts the menu or crashes.

**Why:**
- In the original GameUI, entering an expansion's submenu switches the background map to that expansion's initial chapter background. Previously, two defects broke this chain: (1) `Vfs::read` evaluated all loose filesystem directories across all mounts before checking any VPK archives; because base `hl2/scripts/chapterbackgrounds.txt` exists as a loose file, it shadowed `ep1_pak_dir.vpk`'s chapterbackgrounds file and caused expansion background map lookups to fall back to `background01`. (2) In retail Lost Coast, the chapter 1 background map is `lostcoast/maps/background01.bsp`, which shares its filename with `hl2/maps/background01.bsp`, causing `expansion_for_map` to treat it as base `hl2` and the console to ignore the change because `background01` was already loaded. Refactoring search path layering and canonicalizing the Lost Coast background map resolves both root causes.

**Tested how:**
- `cargo test --workspace --locked`: all 300+ workspace tests pass.
- `cargo test --locked -p hl2-bevy -- --ignored headless_bevy_app_submenu_entry_switches_background_map_and_scene`: passed (headless Bevy app with live owned installation verifying submenu entry, background state change, async load, and scene swap for ep1 and lost coast).
- `cargo test --locked -p hl2-bevy campaign::tests::headless_bevy_app_background_map_failure_logs_warning_and_does_not_crash`: passed.
- `cargo test --locked -p hl2-ui -- --ignored owned_install_resources_load`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.

**Result:** Entering Half-Life 2, Episode One, Episode Two, or Lost Coast submenu updates the background map and swaps the active 3D background scene cleanly. Failures log a warning without crashing.

**Still broken or not tested:** Visual transition on physical display needs manual check.

**Next:** Bug-fix pass complete (all 5 tasks verified and committed).

## 2026-10-13 Hardcoded UI strings audit

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::localize`: added centralized `FALLBACK_STRINGS` table (`&[(&str, &str)]`) with authentic English fallback constants for GameUI dialogs (`GameUI_ConfirmMainMenu_Title`, `GameUI_MainMenuWarning`, `GameUI_QuitConfirmationTitle`, `GameUI_QuitConfirmationText`, `GameUI_Console_QuitWarning`, `GameUI_Quit`, `GameUI_OK`, `GameUI_Cancel`, `GameUI_Apply`, `GameUI_Close`), Console (`GameUI_Console`, `Console_Title`), Options (`GameUI_Options`, `GameUI_Gameplay`, `GameUI_Keyboard`, `GameUI_Mouse`, `GameUI_Gamepad`, `GameUI_Audio`, `GameUI_Video`, `GameUI_Action`, `GameUI_KeyButton`, `GameUI_AdjustGamma_Title`, `GameUI_ThirdPartyTechCredits`, `GameUI_KeyboardSettings`, `GameUI_KeyboardSettingsText`), Expansions (`GameUI_HalfLife2`, `GameUI_EpisodeOne`, `GameUI_EpisodeTwo`, `GameUI_LostCoast`, `GameUI_HalfLife2LostCoast`, `GameUI_Worskhop_MenuName`, `GameUI_Workshop_MenuName`), Credits (`GameUI_Miles_Voice`, `GameUI_Miles_Audio`, `GameUI_Bink`), Loading (`GameUI_Loading`, `GameUI_LoadingFilename`, `LoadingProgress_*`), and HUD (`Valve_Hud_HEALTH`, `Valve_Hud_SUIT`, `Valve_Hud_AMMO`, `Valve_Hud_AMMO_ALT`). Added `fallback_lookup(token: &str) -> Option<&'static str>` wired into `Localization::lookup` so `loc.text("#...")` automatically resolves standard fallbacks when game resources are missing.
- Verified localized game resource escape sequences: scanned all 29 retail localization files and verified that escape sequences (`\n`, `\t`, `\"`, `\\`) exist across 42 tokens in `gameui_english.txt`, 12 in `ep2_english.txt`, 10 in `valve_english.txt`, 9 in `episodic_english.txt`, 9 in `hl2_english.txt`, and 9 in `lostcoast_english.txt`. Preserved escape sequence handling strictly at the parser boundary (`Localization::add_resource` and `Localization::insert`) and removed redundant unescape operations from runtime non-token lookups.
- `hl2-ui::console`: replaced hardcoded `"Console"` title literal in `Console::draw` with `self.gameui.localization.text("#GameUI_Console")`. Simplified `installed_console()` test to use `Localization::default()`.
- `hl2-ui::gameui::loading`: replaced hardcoded loading stage match arms with `loc.text(stage)`, replaced loading template with `loc.text("#GameUI_LoadingFilename")` with `%s1` substitution, and localized loading dialog title with `loc.text("#GameUI_Loading")`.
- `hl2-ui::gameui::resources`: updated `category_title_fallback` to use `loc.text("#GameUI_HalfLife2")`, `loc.text("#GameUI_EpisodeOne")`, `loc.text("#GameUI_EpisodeTwo")`, `loc.text("#GameUI_LostCoast")`.
- `hl2-ui::hud`: added `health_label`, `suit_label`, `ammo_label`, `alt_label` fields to `WeaponHud`, populated during `WeaponHud::load` from mounted VFS localization resources (`resource/valve_english.txt`, `resource/gameui_english.txt`, `resource/hl2_english.txt`) via `Localization`, falling back to `FALLBACK_STRINGS`. Replaced literal `"HEALTH"`, `"SUIT"`, `"AMMO"`, `"ALT"` in `WeaponHud::draw_status` with `&self.health_label`, `&self.suit_label`, `&self.ammo_label`, `&self.alt_label`.
- Tests: added `central_fallback_table_resolves_standard_tokens_when_resources_absent`, `loaded_game_resources_take_precedence_over_central_fallbacks`, and audit test `audit_no_hardcoded_user_visible_strings_in_ui_code_outside_fallback_constants` ensuring non-test UI code contains no hardcoded user-visible literals outside the central table.
- Simplified existing tests in `console.rs`, `resources.rs`, and `menu.rs` to rely on `Localization::default()` rather than manual inline token insertions.

**Why:**
- In the original GameUI and HUD, user-visible strings are loaded from localized resource files (`gameui_english.txt`, `valve_english.txt`, etc.). Hardcoded inline English strings break localization and maintainability. Centralizing fallback constants in a single table ensures that when game resources are missing, authentic fallbacks are used consistently without scattering ad-hoc string literals throughout UI draw routines or test fixtures.

**Tested how:**
- `cargo test --workspace --locked`: all 300+ workspace tests pass.
- `cargo test --locked -p hl2-ui`: all 139 unit tests pass.
- `cargo test --locked -p hl2-ui -- --ignored`: 3 owned install tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.
- Headless runtime: `hl2-bevy --frames 30` ran 30 frames cleanly on Metal.

**Result:** All user-visible UI and HUD strings are resolved from game resources via `Localization`, with central authentic English fallbacks defined in one table. No hardcoded user-visible string literals remain in non-test UI code.

**Still broken or not tested:** Manual visual check of HUD and dialog labels across non-English language packs.

**Next:** Task 5 (Background map on submenu entry).

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `source-assets::keyvalues`: updated `decode_text` to robustly handle byte patterns for UTF-8 with BOM (`[0xef, 0xbb, 0xbf]`), UTF-16 with BOM (`[0xff, 0xfe]` / `[0xfe, 0xff]`), UTF-16 LE without BOM heuristic (`data.len() >= 4 && data.len() % 2 == 0 && data[1] == 0 && data[3] == 0`), and tolerant Windows-1252 / ISO-8859-1 fallback for 8-bit text (`\x80..=\xFF`, e.g. en-dash `\x96`, copyright `\xa9`). In `lexical_tokens`, stripped leading BOM and treated `\u{feff}` as whitespace/ignored so zero-width no-break spaces never pollute token names.
- `hl2-ui::gameui::resources`: updated `Reader::read_text` to decode via `keyvalues::decode_text` rather than raw `String::from_utf8_lossy`, preventing null bytes and replacement character corruption when reading UTF-16 resources.
- `hl2-ui::gameui::resources`: normalized expansion category aliases (`hl2ep1` -> `episodic`, `hl2ep2` -> `ep2`, `hl2lc` -> `lostcoast`) in `category_title_fallback`, populated both canonical and alias entries in `GameUiResources::load`, and added alias resolution in `GameUiResources::background_map_for` and `fallback_category_titles`.
- `hl2-ui::console`: normalized expansion category aliases in `Console::category_title`. In `Console::draw`, rendered line 0 (`HALF-LIFE'`) with `self.title_font` while rendering subtitle line 1 (`title2`) with `self.menu_font` and `self.menu_metrics`, ensuring Latin letters, numbers, and dashes render as authentic text instead of Half-Life symbol icons (since `HALFLIFE2.ttf` only maps logo glyphs for `HALF-LIFE'` and maps lowercase letters to symbol art icons). Fallback rendering when title font is absent now correctly uses `menu_font`.
- Tests: added regression tests in `source-assets` (`decode_text_and_parse_resource_handle_bom_utf16_and_windows1252_patterns`) and `hl2-ui` (`parse_gameinfo_title_handles_single_and_two_line_titles` testing UTF-8 BOM, UTF-16 LE with BOM, UTF-16 LE without BOM, and Windows-1252 en-dash `\x96`, and `submenu_titles_resolve_for_expansion_aliases` in `console.rs`).

**Why:**
- In the original GameUI, expansion submenus (`hl2ep1`, `hl2ep2`, `hl2lc`) map to `episodic`, `ep2`, and `lostcoast`. Previously, querying titles by expansion alias fell back to the base title `Half-Life 2`, and decoding resources via `Reader::read_text` corrupted UTF-16 text. Furthermore, `Console::draw` rendered subtitle line 2 using `self.title_font` (`HALFLIFE2.ttf`), which turned lowercase characters into symbol icons rather than Latin text. Fixing this at the shared decoding and resolution boundaries ensures all encodings (UTF-8 with BOM, UTF-16 LE with/without BOM, Windows-1252) and category aliases resolve cleanly across the entire UI.

**Tested how:**
- `cargo test --workspace --locked`: all 300+ workspace tests pass.
- `cargo test --locked -p hl2-ui -- --ignored`: 3 owned install tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.
- Headless runtime: `hl2-bevy --frames 30` ran 30 frames cleanly on Metal.

**Result:** Submenu titles resolve cleanly for `hl2ep1`, `hl2ep2`, `hl2lc` across all supported text encodings without decoding corruption or symbol glyph mismatch.

**Still broken or not tested:** Visual appearance of subtitle typography on physical display needs manual check.

**Next:** Task 4 (Hardcoded UI strings audit).

## 2026-10-13 Credits dialog sizing

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::options`: removed the credits scroll view entirely (eliminated `credits_scroll`, `needs_scroll`, `max_scroll`, scrollbar rendering, wheel scrolling, and scrollbar drag handling).
- `hl2-ui::gameui::options`: implemented dynamic, flexible height sizing for 3rd-party technology credits dialogs (`OptionsSubAudioThirdPartyDlg` and `OptionsSubVideoThirdPartyDlg`) via `sub_frame_size(&self, sub: Sub, measure_w) -> Vec2`. Sized dialog height strictly to its content: derived from the wrapped label layout plus standard title bar (`34.`) and bottom Close button margins (`46.`).
- Content and Close button layout: guaranteed that all text lines, images, and the bottom-right Close button remain strictly within dialog bounds without overlapping or exceeding frame borders.
- Window bounds reporting: added `credits_fits_window(&self, sub: Sub, canvas: Vec2, measure_w) -> bool` and window fit check in `OptionsDialog::input`. When dialog content cannot fit the window at small display resolutions (e.g. 400x200), logs a non-spamming report message to console history (`self.log`) instead of reintroducing scrolling.
- Tests: added `credits_subdialog_sizing_and_no_scrolling` verifying flexible content sizing between audio and video credits, absence of scrollbars, and strict containment within dialog bounds. Added `credits_dialog_reports_when_exceeding_window_at_small_resolutions` asserting window fit detection and console reporting at small resolutions. Updated `credits_dialogs_have_100_percent_opacity`.

**Why:**
- In the original GameUI, 3rd-party technology credits dialogs size their frames to fit their contained content rather than maintaining a scroll view. Previously, credits dialogs were constrained to a fixed 210px height and utilized a scrollbar. Sizing the dialog dynamically to its wrapped content eliminates unnecessary scrollbars and ensures all text and graphics are visible cleanly. Small resolutions that cannot contain the frame are diagnosed and reported via the console without degrading into scrolling views.

**Tested how:**
- `cargo test --workspace --locked`: all 300+ workspace tests pass.
- `cargo test --locked -p hl2-ui credits_subdialog_sizing_and_no_scrolling`: passed.
- `cargo test --locked -p hl2-ui credits_dialog_reports_when_exceeding_window_at_small_resolutions`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.
- Headless runtime: `hl2-bevy --frames 30` ran 30 frames cleanly on Metal.

**Result:** 3rd party credits subdialogs size height flexibly to wrapped content without scrolling, with all elements strictly inside dialog bounds and no overlaps. Small resolutions exceeding bounds are reported to the console log.

**Still broken or not tested:** Visual appearance on physical display across varying DPI scale factors needs manual check.

**Next:** Task 3 (Submenu title encoding).

## 2026-10-13 Rollover sound only for menu buttons

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`: added `WidgetKind` classification enum (`MenuButton`, `DialogButton`, `RadioButton`, `Dropdown`, `TextInput`, `CheckBox`, `Slider`, `Other`) with `has_rollover_sound(self) -> bool` returning `true` strictly for `MenuButton`.
- `hl2-ui::gameui::layout`: implemented `ControlKind::widget_kind(&self) -> WidgetKind` and `Control::widget_kind(&self)` & `Control::has_rollover_sound(&self) -> bool` (mapping `Button`/`UrlButton` to `DialogButton`, `RadioButton`, `ComboBox` to `Dropdown`, `TextEntry` to `TextInput`, etc.).
- `hl2-ui::gameui::menu`: added `kind: WidgetKind` and `rollover_sound: Option<&'static str>` to `MenuEntry`, with `has_rollover_sound(&self) -> bool` gated on `kind.has_rollover_sound() && rollover_sound.is_some() && enabled && !spacer`. Initialized menu items to `WidgetKind::MenuButton` and `Some("ui/buttonrollover.wav")` while spacers get `WidgetKind::Other` and `None`. Removed rollover sound from `MessageBox::input` on button hover.
- `hl2-ui::gameui::options`: implemented `Target::widget_kind(&self, widgets: &[Widget])` and `Target::has_rollover_sound(&self, widgets: &[Widget])`. In `OptionsDialog::track`, restricted rollover sound triggering strictly to targets that satisfy `has_rollover_sound(widgets)`. Removed rollover sound in `combo_input` on dropdown highlight change.
- `hl2-ui::console`: in `menu_input`, gated `Effect::PlaySound("ui/buttonrollover.wav")` on `menu.entries().get(index).is_some_and(|e| e.has_rollover_sound())` for hover and arrow key navigation.
- Tests: updated `options_dialog_sound_events_hover_and_activation`, `options_dialog_close_button_behaves_like_cancel`, and `message_box_sound_events_hover_and_release` to verify dialog buttons do not emit rollover sound on hover while release-activation sound (`ui/buttonclickrelease.wav`) remains intact. Added `rollover_sound_eligibility_only_for_menu_buttons` unit test asserting `MenuButton` is eligible (`true`), and dialog buttons, radio buttons, dropdowns, and text input fields are not (`false`).

**Why:**
- In authentic Source / Half-Life 2 GameUI, generic VGUI widgets (`vgui::Button`, `vgui::RadioButton`, `vgui::ComboBox`, `vgui::TextEntry`, etc.) do not have rollover sound (`sound_armed`) configured by default. Only top-level main menu and submenu navigation buttons play `ui/buttonrollover.wav`. Previously, generic widgets inside dialogs played the rollover sound on cursor hover. This fix restricts rollover sounds to menu buttons through data-driven eligibility on the widget definition rather than per-dialog exclusions.

**Tested how:**
- `cargo test --workspace --locked`: all 300+ tests passed.
- `cargo test --locked -p hl2-ui rollover_sound_eligibility_only_for_menu_buttons`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.
- Headless runtime: `hl2-bevy --frames 30` ran 30 frames cleanly.

**Result:** Only menu and submenu buttons emit rollover sound on hover and keyboard navigation. Dialog buttons, radio buttons, dropdown lists, and text inputs inside dialogs remain silent on hover, while preserving activation click sounds.

**Still broken or not tested:** Audible verification with physical sound playback needs manual check.

**Next:** Task 2 (Credits dialog sizing).

## 2026-10-13 Half-Life 2: Lost Coast and the Extras submenu

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::menu`: added `MenuCommand::Workshop` variant, parsed `"openworkshopdialog"` to `MenuCommand::Workshop`, and enabled `MenuCommand::SwitchCategory(category)` for all populated categories (including `extras`). Left `MenuCommand::Workshop` disabled (`false` in `implemented`), so it draws disabled and activating it emits `MenuOutcome::Placeholder("WORKSHOP")`. Updated tests to verify Extras and Lost Coast navigation and keyboard selection skipping disabled rows and spacers.
- `hl2-ui::gameui::resources`: updated `fallback_game_menu` to include `lostcoast` under `["hl2", "episodic", "ep2", "lostcoast"]` with standard 6-row game submenu structure (New Game, Load Game placeholder, Achievements placeholder, spacer, Options, Back) and `extras` category with Lost Coast (`SwitchExpansion lostcoast`), Workshop placeholder (`openworkshopdialog`), spacer, and Back (`SwitchCategory main`). Added `normalize_menu_categories` applied during `GameUiResources::load` to normalize `lostcoast` and `extras` entries across both owned `gamemenu.res` and fallback structures.
- `hl2-ui::console`: added `lostcoast_navigation_and_missing_content_error_recovery` unit test covering full navigation through main menu -> Extras -> Lost Coast -> New Game, error reporting via modal `LoadingDialog`, and graceful Escape dismissal.
- `hl2-bevy::main`: expanded owned-install test `episode_maps_load_successfully` to include `d2_lostcoast`, verifying full level load and gameplay-ready campaign state.
- Documentation: updated `README.md`, `docs/gameui.md`, `STATUS.md` so EXTRAS is no longer listed as a pure placeholder.

**Why:**
- Completes Half-Life 2: Lost Coast and Extras submenu support according to authentic retail GameUI specifications. The main menu EXTRAS button now opens the Extras submenu containing Lost Coast and a disabled Workshop placeholder. Lost Coast opens a standardized 6-row game submenu matching Half-Life 2, Episode One, and Episode Two, resolving its title from game resources (`lostcoast/gameinfo.txt`), background map (`background01`), and initial map (`d2_lostcoast`). Missing content is handled gracefully through the modal loading dialog without crashes.

**Tested how:**
- `cargo test --workspace --locked`: all 300+ workspace tests passed.
- `cargo test --locked -p hl2-ui`: 133 unit tests + 3 owned-install tests passed.
- `cargo test --locked -p hl2-bevy`: 67 unit tests passed.
- `cargo test --locked -p hl2-bevy episode_maps_load_successfully -- --ignored`: passed (loading `ep1_citadel_00`, `ep2_outland_01`, and `d2_lostcoast`).
- Headless runtime: `hl2-bevy --map d2_lostcoast --frames 30` ran 30 frames cleanly.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.

**Result:** EXTRAS button opens Extras submenu with Lost Coast and Workshop placeholder; Lost Coast opens authentic game submenu and starts `d2_lostcoast` with loading dialog; missing content recovers gracefully to main menu.

**Still broken or not tested:** Physical monitor visual inspection at 1280x800 and Retina needs manual check.

**Next:** All 9 tasks in the bug-fix and gap-filling pass are complete!

## 2026-10-13 Background map on submenu entry

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::resources`: added `INITIAL_BACKGROUND_MAPS` data-driven table mapping verified initial background maps (`hl2` -> `background01`, `episodic` -> `ep1_background01`, `ep2` -> `ep2_background01`, `lostcoast` -> `background01`, `main` -> `background01`). Added `fallback_background_maps()`, `GameUiResources::background_maps: BTreeMap<String, String>`, and `GameUiResources::background_map_for(&self, category: &str) -> Option<&str>`. Populated `background_maps` from mounted expansion search paths in `GameUiResources::load`.
- `hl2-ui::gameui::menu`: added `MenuOutcome::SwitchExpansion(String)` and `MenuOutcome::SwitchCategory(String)` variants to `MenuOutcome`. Emitted these outcomes from `GameMenu::activate` upon activating expansion and category buttons rather than returning `MenuOutcome::None`.
- `hl2-ui::console`: added `loaded_map: Option<String>` and `pending_map: Option<String>` to `Console`. Updated `level_loaded(background)` to retain current menu category when loading background levels (only returning to root category when `!background`), update `loaded_map` from `pending_map` (or fallback to `background_map()` on startup), and update `activate_menu` to switch background maps on submenu entry via `Effect::MainMenuMap`, updating and persisting `config.gameplay.background_map` without redundant reloads if the target background map is already active.
- `hl2-bevy::campaign` and `hl2-bevy::main`: tracked `ui.source.loaded_map` on level transition completions and startup map initialization.
- Added unit tests: `initial_background_maps_table_and_fallbacks`, `submenu_entry_switches_background_map_and_persists`, and `submenu_back_and_escape_preserves_background_map`.

**Why:**
- Entering the Half-Life 2, Episode One, or Episode Two submenus previously failed to transition the main menu background map because `MenuCommand::SwitchExpansion` returned `MenuOutcome::None` and `level_loaded(true)` unconditionally reset `main_menu.back(...)`, wiping the submenu category. Associating each expansion with its authentic background map, dispatching `Effect::MainMenuMap` on expansion entry, persisting the single background map state, avoiding redundant reloads of active background maps, and keeping the submenu category active upon background level load completes the authentic GameUI navigation flow.

**Tested how:**
- `cargo test --locked -p hl2-ui`: 130 unit tests + 3 owned-install tests passed.
- `cargo test --locked -p hl2-bevy`: 67 unit tests passed.
- `cargo test --locked -p hl2-bevy episode_maps_load_successfully -- --ignored`: passed.
- Headless runtime: `hl2-bevy --map ep1_background01 --frames 30` and `hl2-bevy --map ep2_background01 --frames 30` ran 30 frames cleanly.
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.

**Result:** Entering Half-Life 2, Episode One, or Episode Two submenu transitions the background map, preserves submenu category, and persists the single background map state across sessions.

**Still broken or not tested:** Physical monitor visual inspection at 1280x800 and Retina needs manual check.

**Next:** Task 9: Half-Life 2: Lost Coast and the Extras submenu.

## 2026-10-13 Episode map loading failures (UTF-8 tolerant decoding)

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `source-assets::scenes`: replaced strict `std::str::from_utf8` decoding with tolerant `String::from_utf8_lossy(&tail[..length]).into_owned()` in `scenes::Cache::parse` when parsing string table entries in Valve Scene Image Files (`scenes/scenes.image`). Added unit regression test `scene_cache_tolerates_non_utf8_strings` covering non-UTF-8 bytes at index 10.
- `hl2-bevy::campaign`: added `MapLoaded` completion marker struct, `Campaign::loaded_map: Option<String>`, and `Campaign::is_gameplay_ready(&self) -> bool` providing an observable gameplay-ready state distinguishing successful full loads from merely passing parser stages. Added background map load failure tolerance: when a background map fails to load, logs a warning to the console, records the failure, clears the loading dialog, and continues the session rather than presenting a fatal error modal or aborting. Guarded ongoing gameplay map loads against being interrupted by background map requests while a gameplay load is active.
- `hl2-bevy::simulation`: guarded `Effect::MainMenuMap` in `Simulation::console_effects` from overwriting active gameplay map load requests. Added unit test `background_map_setting_does_not_interrupt_gameplay_map_load`.
- `hl2-ui::console`: guarded `map_background` console command against interrupting active gameplay map loading (saves config and logs notice without dispatching effect). Added unit test assertion in `background_map_and_map_background_console_commands`.
- `hl2-bevy::main`: made `load_startup_map` resilient against custom background map failures with automatic fallback to the default startup background map. Added owned-install test `episode_maps_load_successfully` asserting that both Episode One (`ep1_citadel_00`) and Episode Two (`ep2_outland_01`) load completely into an active gameplay-ready state (`is_gameplay_ready`, `sim.loading == false`, `!sim.player.dead`, and console `Mode::Gameplay`).

**Why:**
- Loading initial maps for Episode One (`ep1_citadel_00`) and Episode Two (`ep2_outland_01`) failed with `map transition failed: invalid utf-8 sequence of 1 bytes from index 10`. The retail VSIF scene cache (`scenes.image`) string pool contains byte-oriented C strings with non-UTF-8 characters. Strict UTF-8 decoding rejected the entire choreography cache during world load. Tolerant decoding at the C string parser boundary resolves the issue without compromising UTF-8 validation elsewhere. Additionally, background map requests or failures previously risked disrupting ongoing or startup map loads.

**Tested how:**
- `cargo test --locked -p source-assets`: all 92 tests passed (including `scene_cache_tolerates_non_utf8_strings`).
- `cargo test --locked -p hl2-ui`: all 127 tests passed + 3 owned-install tests passed.
- `cargo test --locked -p hl2-bevy`: all 67 tests passed (including `background_map_setting_does_not_interrupt_gameplay_map_load`).
- `cargo test --locked -p hl2-bevy episode_maps_load_successfully -- --ignored`: both `ep1_citadel_00` and `ep2_outland_01` verified loading completely into active gameplay state.
- Runtime headless executions: `hl2-bevy --map ep1_citadel_00 --frames 30` and `hl2-bevy --map ep2_outland_01 --frames 30` both verified loading and executing 30 frames in active gameplay mode.
- `cargo clippy --locked -p hl2-ui -p hl2-bevy -p source-assets -- -D warnings`: 0 warnings.
- `cargo fmt --all --check`: clean formatting.

**Result:** `ep1_citadel_00` and `ep2_outland_01` load completely into active gameplay; background map failures log warnings without aborting; gameplay map loading is protected from background map interruption.

**Still broken or not tested:** None.

**Next:** Task 8: Background map on submenu entry.

## 2026-10-13 Input blocking behind dialogs

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::console`: implemented shared modal dialog occlusion and input blocking. Added `modal_dialog_open(&self) -> bool` checking whether any modal dialog or overlay is active (`Mode::Options`, `confirm_box.is_some()`, `loading_dialog.is_some()`, `Mode::Console`). Added `modal_bounds(&self) -> Option<Rect>` and `is_occluded(&self, point: Vec2) -> bool` returning exact canvas bounding rectangles for active modal dialogs. Added `reset_menu_interaction(&mut self)` to clear pointer hover and button tracker state while preserving keyboard selection. Added `clear_menu_hover(&mut self)` to reset all menu selection on level loads and transitions. Extracted modular input handlers `confirm_input`, `options_input`, and `console_input`.
- Gated `Console::menu_input`: early-returns empty action and sound event vectors when `modal_dialog_open()` is true, completely preventing rollover sounds, button presses, clicks, and activations from passing through to underlying buttons.
- Gated `Console::draw`: underlying menu buttons are drawn unarmed (`armed = !modal_open && (is_pressed || menu.selection == Some(i))`, `is_pressed = !modal_open && self.menu_tracker.is_pressed(i)`), eliminating hover highlights on buttons underneath open dialogs.
- Preserved keyboard navigation: closing dialogs via Cancel/Escape calls `reset_menu_interaction()` rather than wiping keyboard selection, keeping focus on the initiating menu button for subsequent keyboard navigation.
- Added unit test `modal_dialog_blocks_underlying_menu_hover_sounds_and_clicks` verifying input and rollover sound suppression while options or confirmation dialogs are open, and verifying restoration upon closure.

**Why:**
- When an options dialog, confirmation message box, map loading dialog, or console overlay was open, moving the cursor over the underlying main menu or submenu buttons triggered hover effects, rollover sounds, and clicks passing through. A shared modal check and input gating mechanism guarantees that underneath buttons are completely inert and non-interactive until dialogs are dismissed.

**Tested how:**
- `cargo test --locked -p hl2-ui`: all 127 unit tests passed (including `modal_dialog_blocks_underlying_menu_hover_sounds_and_clicks`).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked -p hl2-ui -- -D warnings`: strict clippy passed with 0 warnings.
- `cargo fmt --all --check`: clean formatting verified across all workspace crates.
- `cargo test --locked -p hl2-bevy`: all 66 unit tests passed.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30` passed (30 frames rendered, exit code 0).

**Result:** Underlying menu buttons are visually unarmed and completely inert while dialogs or overlays are open; no clicks, rollover sounds, or activations pass through.

**Still broken or not tested:** None.

**Next:** Task 7: Episode map loading failures (UTF-8 tolerant decoding).

## 2026-10-13 Wrapped multi-line labels, dialog layouts

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`: enhanced greedy word wrapping `wrap` to split words longer than `width` character-by-character (handling Unicode scalar values safely), guaranteeing lines never exceed parent bounds. Added `Painter::multiline_label` with alignment support and parent height bounding. Updated `Painter::wrapped` to delegate to `multiline_label`. Expanded unit test `wrap_breaks_on_measured_width` covering very long words, multi-byte UTF-8 localization, explicit `\n`, and zero-width edge cases.
- `hl2-ui::gameui::loading`: added `LoadingDialog::layout_geometry_sized` and updated `LoadingDialog::draw` to wrap status/warning labels to 340px width. Dynamically expands dialog frame height and shifts the progress bar downward when warning or error text wraps beyond 2 lines, keeping all content strictly inside the dialog frame without overlap. Added unit test `loading_dialog_multiline_status_wraps_and_expands_bounds`.
- `hl2-ui::gameui::options`: resolved label overlap in 3rd-party technology credits dialogs (`OptionsSubAudioThirdPartyDlg` / `OptionsSubVideoThirdPartyDlg`). Replaced static overlapping `ypos` layout with sequential vertical measurement (`measure_credits_height`, `draw_credits_subdialog`, `CreditItem`). Wrapped text and images strictly stay within `view_rect` (between title and fixed Close button at bottom right). When content height exceeds available view height (e.g. Audio credits), renders a vertical scrollbar and enables mouse wheel scrolling via `credits_scroll` and `credits_input`. Added unit test `credits_subdialog_layout_prevents_overlap_and_scrolls`.

**Why:**
- Long status, warning, and error messages in the map loading dialog previously overflowed horizontally outside the frame bounds and overlapped the progress bar. In the 3rd-party technology credits dialogs, retail `.res` coordinates placed multi-line labels (`Label1`, `Label3`, `Label2`, `BinkImage`) at fixed vertical offsets that caused wrapped lines to print directly over subsequent labels and overlap the Close button. Word-based wrapping with character breaking, dynamic dialog height expansion, sequential vertical layout, and vertical scrolling eliminate clipping and overlaps.

**Tested how:**
- `cargo test --locked -p hl2-ui`: all 126 unit tests passed (including `wrap_breaks_on_measured_width`, `loading_dialog_multiline_status_wraps_and_expands_bounds`, and `credits_subdialog_layout_prevents_overlap_and_scrolls`).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked -p hl2-ui -- -D warnings`: strict clippy passed with 0 warnings.
- `cargo fmt --all --check`: clean formatting verified across all workspace crates.
- `cargo test --locked`: all 91 simulation/assets tests and 66 Bevy host tests passed.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed (30 frames rendered, exit code 0).

**Result:** Multi-line labels wrap within parent bounds, map loading dialog warnings expand without overflow, and 3rd-party technology credits display with sequential non-overlapping layout and scrollbar support.

**Still broken or not tested:** Visual check on physical display (unverified, needs manual check at 1280x800 and native Retina for loading dialog warning and credits scrollbar).

**Next:** Task 6: Input blocking behind dialogs (shared modal/occlusion check).

## 2026-10-13 High-DPI / scale-factor consistency

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::menu`: replaced static unscaled button vertical spacing (`MENU_BUTTON_SPACING = 20.`) with proportional spacing derived from the 480p scale basis (`BASE_MENU_BUTTON_SPACING = 12.`, `button_spacing(scale) = (BASE_MENU_BUTTON_SPACING * scale).round()`). At 1280x800 reference resolution (scale 5/3), spacing remains exactly 20.0 px, preserving existing layout invariants while scaling button spacing proportionally on High-DPI/Retina screens (e.g. 40.0 px at 1600p height). Added `MenuLayout::geometry_scaled(height, scale_factor)` supporting explicit window scale factors. Updated unit test `native_menu_uses_480_coordinate_scaling_with_proportional_spacing` asserting exact 1280x800 stability and Retina 2.0 scaling.
- `hl2-ui::gameui::widgets`: added `Painter::scale_for_window(logical_height, scale_factor) -> f32` computing the shared dialog scale `(effective_height / 720.).max(1.)`. Added unit test `scale_computation_for_scale_factors_and_window_sizes` verifying scale computation for scale factors 1.0 and 2.0 across several standard resolutions (480p, 720p, 800p, 1080p, 1440p, 2160p) with regression assertions for 1280x800 layout scaling.
- `hl2-ui::gameui::loading`: added `LoadingDialog::layout_geometry(canvas_size, scale) -> (Vec2, f32)` centering `LOADING_DIALOG_SIZE * scale` on canvas. Updated `LoadingDialog::draw` to scale dialog size and origin with `painter.scale`. Added unit test `loading_dialog_scaling_and_layout_geometry` verifying centered placement and proportional sizing at 720p (1.0x), 1280x800 (1.11x), and Retina 2.0x (2.22x).
- `hl2-ui::console`: updated `Console::draw` in both gameplay mode and menu mode to pass `scale = Painter::scale_for(self.canvas.height())` to `loading.draw` instead of hardcoded 1.0.

**Why:**
- On native Retina and High-DPI displays, menu button vertical spacing was locked to a static 20px constant, causing buttons to appear cramped and squished relative to the scaled font and title text. Furthermore, the map loading dialog was drawn with an unscaled 1.0 scale factor and unscaled centering, causing it to render as a tiny box at high resolutions. Deriving button spacing from the 480p scale basis and scaling the loading dialog via the shared dialog scale policy ensures full resolution and DPI agnosticism.

**Tested how:**
- `cargo test --locked -p hl2-ui`: all 124 unit tests passed (including `scale_computation_for_scale_factors_and_window_sizes`, `native_menu_uses_480_coordinate_scaling_with_proportional_spacing`, and `loading_dialog_scaling_and_layout_geometry`).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked -p hl2-ui -- -D warnings`: strict clippy passed with 0 warnings.
- `cargo fmt --all --check`: clean formatting verified across all workspace crates.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed (30 frames rendered, exit code 0).

**Result:** Menu button vertical spacing and map loading dialog sizing are resolution and DPI agnostic across all scale factors, while exactly preserving the 1280x800 layout values.

**Still broken or not tested:** Physical monitor visual inspection at native Retina and 1280x800 (unverified, needs manual check).

**Next:** Task 5: Wrapped multi-line labels, dialog layouts (shared word wrapping, loading dialog warning, 3rd party credits scrolling).

## 2026-10-13 Check box checkmark

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`: replaced the filled square placeholder in `Painter::check_box` with `Painter::check_mark`, which draws an authentic 2-pixel-thick checkmark using simple UI shapes (7 vertical column rectangles forming a descending short left stroke from dx=3, dy=5 to apex dx=5, dy=7..8, and an ascending long right stroke from dx=5, dy=7..8 to dx=9, dy=3). The checkmark shifts by +1px on press when enabled, scales seamlessly across all DPI scale factors, and uses `theme.check` (white) when enabled or `theme.text_disabled` when disabled.
- `hl2-ui::gameui::widgets`: added unit test `check_box_checked_draws_white_checkmark_not_plain_square` verifying unchecked vs checked quad counts, white checkmark color, exact shape coordinates, and +1px pressed offset.

**Why:**
- In Source VGUI, checked check boxes render an authentic white checkmark (tick glyph) rather than a filled solid square placeholder.

**Tested how:**
- `cargo test --locked -p hl2-ui`: all 122 unit tests passed (including `check_box_checked_draws_white_checkmark_not_plain_square`).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked -p hl2-ui -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed (30 frames rendered, exit code 0).

**Result:** Checked check boxes display an authentic white checkmark matching the Source VGUI look.

**Still broken or not tested:** Visual appearance on a physical display (unverified, needs manual check at 1280x800 and native Retina).

**Next:** Task 4: High-DPI / scale-factor consistency (DPI-agnostic button spacing & loading dialog scaling).

## 2026-10-13 Episode submenu titles

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::menu`: added `title2_x` and `title2_y` layout fields to `MenuLayout` (with Source-authentic defaults `DEFAULT_TITLE2_X = 76.` and `DEFAULT_TITLE2_Y = 172.` from `Main.Title2` in `ClientScheme.res`), added `title2` coordinates to `MenuGeometry`, and parsed `Main.Title2.X` and `Main.Title2.Y` in `apply_client_scheme`. Updated unit test `native_menu_uses_480_coordinate_scaling_with_static_spacing`.
- `hl2-ui::gameui::resources`: added `category_titles` to `GameUiResources` resolving titles for each category (`"main"`, `"hl2"`, `"episodic"`, `"ep2"`, `"lostcoast"`). Added `parse_gameinfo_title` extracting `title` and optional `title2` (joined with `\n`), `category_title_fallback` generating localized two-line titles (`"Half-Life 2\n--- Episode One ---"` / `"Half-Life 2\n--- Episode Two ---"`), and `fallback_category_titles`. Populated `category_titles` in `GameUiResources::load` from expansion `gameinfo.txt` files with clean fallbacks. Added unit tests `parse_gameinfo_title_handles_single_and_two_line_titles` and `category_titles_fallback_provides_two_line_episode_titles`.
- `hl2-ui::console`: added `category_titles` to `Console` and methods `category_title(&self, category)` and `current_title(&self)` returning the active category title (falling back to `self.title`). Updated `Console::draw` to split multi-line titles across lines, positioning line 0 at `geometry.title` and line 1 at `geometry.title2` with GDI-aligned ABC pen advances for `title_face` and baseline cell rendering for text fallbacks. Added unit test `episode_submenus_show_two_line_titles`.

**Why:**
- In the original GameUI and 20th Anniversary launcher, Episode One and Episode Two submenus display authentic two-line titles ("Half-Life 2" on line 1, "--- Episode One ---" / "--- Episode Two ---" on line 2), resolved directly from game resources (`gameinfo.txt` `title` and `title2` or localized tokens) with clean fallbacks.

**Tested how:**
- `cargo test --locked`: all 121 unit tests in `hl2-ui`, 66 in `hl2-bevy`, 91 in `source-assets`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed (verifying `res.category_titles` parses `== episode one==` and `== episode two ==`).
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- `cargo fmt --all --check`: clean formatting verified across all workspace crates.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed (30 frames rendered, exit code 0).

**Result:** Episode One and Episode Two submenus now resolve and render authentic two-line titles with scheme-defined `Main.Title1` and `Main.Title2` offsets and clean localization fallbacks.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 3: Check box checkmark (authentic white checkmark shape in checked state instead of plain square).

## 2026-10-13 Literal \n in localized text

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::localize`: added `pub fn unescape(s: &str) -> String` unescaping standard Source escape sequences (`\n`, `\r`, `\t`, `\"`, `\\`) while preserving non-escape literal backslashes intact. Integrated `unescape` into `Localization::add_resource`, `Localization::insert`, and `Localization::localize`. Added unit tests `unescape_handles_linebreaks_tabs_quotes_and_preserves_literal_backslashes` and `localized_tokens_unescape_line_breaks`.
- `hl2-ui::gameui::widgets`: updated `Painter::text_width` and `Painter::text` to split multi-line strings across newline boundaries, computing max line width and rendering lines vertically incremented by `FONT_TALL * self.scale`.
- `hl2-ui::gameui::menu`: updated `MessageBox::layout` to compute text dimensions across multiple lines and expand box height by `FONT_TALL * line_count + 100.`. Added unit test `return_to_main_menu_multiline_layout`.

**Why:**
- Resolves literal `\n` in localized dialogs (e.g. Return to main menu confirmation `#GameUI_MainMenuWarning`) at the shared localization resolver level so every localized text consumer automatically receives clean unescaped line breaks.

**Tested how:**
- `cargo test --locked -p hl2-ui`: all 118 unit tests passed (0 failures).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- `cargo fmt --all --check`: clean formatting verified.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed (30 frames rendered, exit code 0).

**Result:** Localized strings with escape sequences render line breaks cleanly; message boxes dynamically adjust bounds for multi-line text.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 2: Episode submenu titles (two-line titles for Episode One and Episode Two).

## 2026-10-13 Background map handling

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::config`: added `pub background_map: Option<String>` to `GameplaySettings` with `#[serde(skip_serializing_if = "Option::is_none")]` and default `None`. In `Config::migrate`, sanitized map name (stripping `.bsp` extensions, validating alphanumeric and underscore characters up to 128 chars, reverting blank/invalid values to `None`). Added unit test `background_map_persistence_roundtrip_and_migration` verifying default `None` JSON omission, persistence roundtrip, `.bsp` stripping, and invalid name rejection.
- `hl2-ui::console`: added `Console::background_map(&self) -> Option<&str>` establishing the single source of truth (prioritizing `config.gameplay.background_map` before falling back to `gameui.background_map`). Added `Console::set_background_map(&mut self, Option<String>)`. Added console commands `background_map [name|default]` (querying or setting persistent background map) and `map_background <name>` (setting config, starting modal loading dialog, and emitting `Effect::MainMenuMap`). Updated `ConfirmKind::ReturnToMainMenu` to route through `self.background_map()`. Added unit tests `background_map_single_source_of_truth_and_return_to_main_menu` and `background_map_and_map_background_console_commands`.
- `hl2-bevy::main`: added `pub background_map: Option<String>` to `Options` and `--background-map NAME` CLI option. In `load_startup_map`, resolved startup background map via `options.background_map` -> `config.gameplay.background_map` -> `startup_background_map(&vfs)` fallback chain. Added unit test `startup_parses_background_map_override`.

**Why:**
- Provides a unified single source of truth for the main-menu background map, persisted cleanly in user settings without touching read-only game installation files.
- Supports runtime customization via console commands (`background_map`, `map_background`) and launch arguments (`--background-map`) matching Source engine conventions.

**Tested how:**
- `cargo test --locked`: all 115 unit tests in `hl2-ui`, 66 in `hl2-bevy`, 91 in `source-assets`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- `cargo fmt --all --check`: clean formatting verified across all workspace crates.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and loaded default background map `background01`.
- Headless Bevy runtime background override: `cargo run --locked -p hl2-bevy -- --background-map background02 --frames 30 --no-focus` passed and loaded overridden background map `background02`.

**Result:** Single source of truth for background map handling across persistent config, console commands, Return to Main Menu confirmation, CLI arguments, and runtime map loader. All 10 tasks in the GameUI follow-up pass complete.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Follow-up milestones in owner's order (arsenal step 13b, NPC AI step 14, detail sprites, etc.).

## 2026-10-13 Episode One & Two New Game maps

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `source-assets::vpk`: added `expansion_for_map(map)` detecting `ep2`, `episodic`, and `lostcoast` from map name prefixes (`ep2_*`, `ep1_*`, `*lostcoast*`). Added `Vfs::mount_expansion(game, expansion)` implementing Source engine search path inheritance (`ep2` -> `ep2` + `episodic` + `hl2` + `platform`; `episodic` -> `episodic` + `hl2` + `platform`; `lostcoast` -> `lostcoast` + `hl2` + `platform`). Kept `Vfs::mount(game)` as default `hl2` + `platform` mount. Added unit test `expansion_for_map_detects_episodes_and_lostcoast`.
- `hl2-ui::gameui::resources`: added `INITIAL_EPISODE_MAPS` data-driven table mapping `"hl2"` -> `"d1_trainstation_01"`, `"episodic"` -> `"ep1_citadel_00"`, `"ep2"` -> `"ep2_outland_01"`, and `"lostcoast"` -> `"d2_lostcoast"`. Added `fallback_new_game_maps()`. Updated `chapter_map` to extract map names from composite commands (such as `playvideo_exitcommand ep1_recap map ep2_outland_01` in Episode Two's `chapter1.cfg`). Updated `GameUiResources::load` to populate `new_game_maps` from each expansion's installed `chapter1.cfg` with fallbacks. Added unit tests `initial_episode_maps_table_and_fallbacks` and extended `chapter_files_name_their_maps`.
- `hl2-ui::console`: updated `installed_console` to populate all episode categories via `fallback_new_game_maps()`. Updated failed loading dialog dismissal in `Console::escape` and `Console::input` to reset `main_menu` category to root (`main`). Added unit test `episode_one_and_two_new_game_navigation_and_missing_content_error_recovery` verifying New Game actions across Episode One and Episode Two, failure state latching on missing content, and modal return to main menu on Escape / Enter.
- `hl2-bevy::assets`: updated `load_with_canvas` to mount expansion content via `Vfs::mount_expansion(game, expansion_for_map(map))` ensuring episode BSPs and assets in `episodic/` and `ep2/` are discovered.

**Why:**
- In the retail 20th Anniversary combined launcher, the main menu supports starting New Game directly for Half-Life 2, Episode One, and Episode Two.
- When episode content or maps are missing from an installation, the engine provides graceful error handling via the modal loading dialog without panicking or crashing.

**Tested how:**
- `cargo test --locked`: all 112 unit tests in `hl2-ui`, 64 in `hl2-bevy`, 91 in `source-assets`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed (verifying `hl2`, `episodic`, `ep2`, and `lostcoast` map discoveries).
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.
- Headless Bevy runtime Episode One: `cargo run --locked -p hl2-bevy -- --frames 30 --map ep1_citadel_00 --no-focus` passed and exited code 0.
- Headless Bevy runtime Episode Two: `cargo run --locked -p hl2-bevy -- --frames 30 --map ep2_outland_01 --no-focus` passed and exited code 0.

**Result:** Data-driven episode map table verified against retail game files, authentic search path inheritance, and graceful loading dialog failure recovery for missing content.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 10 (Background map handling).

## 2026-10-13 Map loading dialog

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::loading`: created module with `LoadingDialog`, `LoadProgress`, thread-safe `report_progress`, `current_progress`, and `reset_progress`. Modeled after retail Source `LoadingDialogNoBanner.res` (380x112 centered frame, `#GameUI_Loading` title, info status label at (20, 34, 340, 24), progress bar at (20, 64, 340, 24)). Supports localized stage status strings (`LoadingProgress_SpawningServer`, `LoadingProgress_LoadMap`, `LoadingProgress_PrecacheWorld`, `LoadingProgress_SignonDataLocal`, `LoadingProgress_LoadResources`, `LoadingProgress_SignonLocal`), unlocalized English fallbacks, `status_override`, and failure error messages. Added unit tests for initial state, progress reporting, localized fallbacks, failure latches, and completion.
- `hl2-ui::gameui::widgets`: added `Painter::progress_bar` implementing Source SDK `ProgressBar.cpp` geometry (segmented chunks: width 8px, gap 4px, inset 4px, depressed sunken border, dark cavity background, white chunk fills). Added unit test `progress_bar_drawing_and_segments`.
- `hl2-ui::console`: added `pub loading_dialog: Option<LoadingDialog>` to `Console`. Added helpers `start_loading`, `update_loading`, `loading_failed`, `loading_complete`, and `is_loading`. Updated `level_loaded` to dismiss loading dialog and reset progress. Updated `Console::draw` to render loading dialog centered over menus or blank screen during gameplay. Made `Console::input` modal while loading is active, consuming inputs and allowing Enter / mouse click / Escape to dismiss failed dialogs and return to main menu. Added unit test `loading_dialog_state_transitions_and_dismissal`.
- `hl2-bevy::assets`: wired authentic stage progress reports into `assets::load_with_canvas`: 0.05 (`LoadingProgress_SpawningServer`), 0.15 (`LoadingProgress_LoadMap`), 0.35 (`LoadingProgress_PrecacheWorld`), 0.50 (`LoadingProgress_SignonDataLocal`), 0.55..0.85 (`LoadingProgress_LoadResources` fine-grained across texture decoding), 0.90..1.0 (`LoadingProgress_SignonLocal`).
- `hl2-bevy::campaign`: wired `start_loading`, `update_loading`, and `loading_failed` around asynchronous map loading tasks, preserving spawn positions/angles across transitions.
- `hl2-bevy::main`: on startup with `--map`, queues `options.map` into `console_ui.map_request` with `start_loading`, loads background map, and pauses `--frames` budget consumption while loading is active.

**Why:**
- Half-Life 2 displays an authentic modal VGUI loading dialog with stage progress when loading a map from the main menu, in-game console `map` command, or startup arguments.
- Segmented chunk rendering and real stage tokens match retail Source engine behavior.

**Tested how:**
- `cargo test --locked`: all 109 unit tests in `hl2-ui`, 64 in `hl2-bevy`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo test --locked -p hl2-ui -- --ignored`: all 3 owned-install tests passed.
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.
- Headless Bevy runtime map load: `cargo run --locked -p hl2-bevy -- --frames 30 --map d1_trainstation_01 --no-focus` passed and exited code 0.

**Result:** Authentic modal loading dialog matching Source SDK `ProgressBar` chunk rendering and retail `LoadingDialogNoBanner.res` layout, with real loader stages and modal failure recovery.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 9 ("New Game" for HL2: Episode One and Episode Two).

## 2026-10-13 Options tooltips

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::layout`: added `tooltip: String` field to `Control` struct and parsed `tooltiptext` property in KeyValues control definitions. Added unit test verifying `reverse.tooltip` parses `#GameUI_ReverseMouseLabel`.
- `hl2-ui::gameui::widgets`: added `tooltip_bg`, `tooltip_text`, and `tooltip_border` to `Theme` with defaults matching Source VGUI scheme values (`Tooltip.BgColor` orange `(255, 155, 0, 240)`, `Tooltip.TextColor` black, dark 1px border `(40, 40, 40, 255)`). Added `tooltip_rect(pointer, tip_size, bounds)` implementing Source SDK positioning rules (hangs +20px down and right, flips upwards or leftwards on screen overflow, strictly clamped within screen bounds). Added `Painter::tooltip` rendering wrapped multiline tips, background box, and border. Added unit test `tooltip_geometry_and_screen_clamping`.
- `hl2-ui::gameui::options`: added `ActiveTooltip { pub text: String, pub pointer: Vec2 }` and `pub(crate) active_tooltip: Option<ActiveTooltip>` to `OptionsDialog`. Added `tip_for_control(&self, panel, control_name) -> Option<String>` looking up `control.tooltip` in layout and resolving via `loc.localize` (returning `None` if missing or empty). Added `tooltip_at(&self, canvas, pointer) -> Option<String>` looking up hovered controls on the active page. In `page_input`, updates `active_tooltip` on pointer motion, and clears it on clicking, dragging, blank page hovering, or hovering controls with no authored tip (missing-tip case). In `input`, clears tooltip when confirmation dialog, subdialog, combo dropdown, or key-bind capture is active. In `draw`, renders `active_tooltip` on top of all dialog layers within screen bounds. Added unit test `options_tooltips_lookup_and_missing_case`.

**Why:**
- Half-Life 2 GameUI options controls (such as Reverse Mouse, Mouse Filter, Raw Input, Mouse Acceleration) specify explanatory hints in their `.res` resource files via `tooltiptext` pointing to `gameui_english.txt` tokens.
- When hovering an options control with an authored tooltip, a tooltip box displays near the cursor, flipped and clamped to stay within screen boundaries per Source SDK `vgui_controls/Tooltip.cpp` rules. Controls without tooltips display nothing.

**Tested how:**
- `cargo test --locked`: all 103 unit tests in `hl2-ui`, 64 in `hl2-bevy`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.

**Result:** Authentic options tooltips styled per VGUI scheme with SDK positioning, multi-line wrapping, bounds clamping, and clean dismissal on clicks/subdialogs.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 8 (Map loading dialog).

## 2026-10-13 "Return to main menu" confirmation dialog

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::menu`: added `MessageBox::return_to_main_menu(loc)` resolving retail localized tokens `#GameUI_ConfirmMainMenu_Title` («RETURN TO MAIN MENU»), `#GameUI_MainMenuWarning` («Are you sure?\nAll unsaved progress will be lost.»), `#GameUI_OK` («OK»), and `#GameUI_Cancel` («Cancel») with key-name fallback.
- `hl2-ui::console`: generalized message box tracking from `quit_box: Option<MessageBox>` to `confirm_box: Option<(MessageBox, ConfirmKind)>`, supporting `ConfirmKind::Quit` and `ConfirmKind::ReturnToMainMenu`. Replaced direct menu-item map unloading with the shared confirmation dialog when selecting "Return to main menu" (row 5 in pause menu, command `OpenMainMenuDialog`). Confirming OK switches mode to `Mode::MainMenu` and triggers map unload via `Effect::MainMenuMap` (or keeps map frozen if no background map is installed). Confirming Cancel closes the confirmation dialog and stays in the pause menu. Escape closes the confirmation dialog before leaving the menu. Added unit tests `return_to_main_menu_confirmation_dialog_ok_unloads_to_background` and updated `pause_menu_entries_activate_by_pointer_and_keyboard`.

**Why:**
- Half-Life 2 GameUI presents an explicit confirmation dialog when selecting "Return to main menu" from the in-game pause menu, warning the player that unsaved progress will be lost.
- Reusing the shared `MessageBox` component preserves Source SDK `vgui::MessageBox` layout rules, button styling, hover/click audio, and keyboard Enter activation.

**Tested how:**
- `cargo test --locked`: all 101 unit tests in `hl2-ui`, 64 in `hl2-bevy`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.

**Result:** Authentic Return to Main Menu confirmation dialog resolving retail tokens, with Cancel keeping gameplay paused and OK unloading to the main menu background.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 7 (Options tooltips).

## 2026-10-13 Close button on dialogs and frames

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`: added standard Source VGUI title bar close button geometry `CLOSE_BUTTON_SIZE = vec2(18., 16.)`, `CLOSE_BUTTON_INSET = vec2(24., 6.)`, and `close_button_rect(frame_size)`. Added declarative `FrameSpec` struct with `has_close: bool` (defaulting to true) and `close_rect()`. Added `Painter::frame_with_close` and `Painter::close_button` rendering bevel frames with centered title button glyph and +1px pressed state. Added unit test `frame_spec_and_close_button_drawing`.
- `hl2-ui::gameui::options`: added `Target::Close` to `Target` enum. Wired close button hit testing and hover/release tracking into `page_input` (`DIALOG_SIZE`) and `sub_input` (subdialog size). Activating close on the main options dialog emits `OptionsOutcome::Close(None)` (discarding unapplied draft modifications, identically to Cancel). Activating close on subdialogs (Gamma, Tech Credits) triggers `Command::SubCancel`, restoring draft values to their pre-opened state (e.g. restoring saved gamma). Updated `draw` to render close buttons with active ButtonState on both main dialog and subdialogs. Added unit tests `options_dialog_close_button_behaves_like_cancel` and `options_subdialog_close_button_cancels_and_restores`.
- `hl2-ui::gameui::menu`: added `pub has_close: bool` to `MessageBox` and `MessageBoxLayout`, defaulting to `false` per original Source SDK `vgui::MessageBox` architecture (`SetCloseButtonVisible(false)`). Added `with_close(bool)` builder method, allowing data-driven per-dialog enablement. Wired close button hit testing to return `Some(false)` (Cancel). Added unit test `message_box_close_button_data_driven_cancel`.

**Why:**
- Source VGUI `Frame` and `PropertyDialog` display a close (X) button in the top-right corner of their title bar, functioning identically to Cancel.
- Confirmation message boxes (`vgui::MessageBox`) explicitly disable the close button, relying solely on explicit action buttons. The shared frame component supports data-driven enablement across dialog types.

**Tested how:**
- `cargo test --locked`: all 100 unit tests in `hl2-ui`, 64 in `hl2-bevy`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.

**Result:** Authentic VGUI close (X) buttons on Options dialog and subdialogs, data-driven configuration on message boxes, with exact Cancel semantics and hover/click audio.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 6 ("Return to main menu" confirmation dialog).

## 2026-10-13 UI sounds: hover rollover, click release and GameUI audio routing

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-simulation::sounds`: added `pub ui: bool` field to `SoundRequest` and helper `SoundRequest::ui(name)` to distinguish UI-triggered audio cues from spatial in-world simulation audio.
- `hl2-bevy::audio`: added `ui: bool` to `SoundPlayer`. Preloaded `"ui/buttonrollover.wav"`, `"ui/buttonclick.wav"`, and `"ui/buttonclickrelease.wav"` cues on initialization. In `Audio::emit` and `Audio::queue`, UI sounds bypass the simulation pause check so menu/dialog sounds play normally when the game or background level is paused. Routed volume scaling through master volume (`global.volume.to_linear()`) and `snd_mute_losefocus`. Added `warn!` logging for missing sound requests and missing cues.
- `hl2-bevy::simulation`: routed `Effect::PlaySound(sound)` from UI console effects to `game.sound_requests.push(SoundRequest::ui(sound))`. Added unit test `console_effects_routes_play_sound_to_ui_sound_requests`.
- `hl2-ui::console`: added `Effect::PlaySound(String)`. Added `last_menu_hover: Option<usize>` tracking to emit `"ui/buttonrollover.wav"` only on boundary entry from outside, on keyboard Up/Down navigation, and `"ui/buttonclickrelease.wav"` on menu item activation. Forwarded drained sounds from `OptionsDialog` and `MessageBox`.
- `hl2-ui::gameui::options`: added `last_armed: Option<Target>` tracking and sound queue (`take_sounds()`). Emits `"ui/buttonrollover.wav"` on control hover entry and combo navigation, and `"ui/buttonclickrelease.wav"` on button/tab/item activation (excluding slider drags). Added unit test `options_dialog_sound_events_hover_and_activation`.
- `hl2-ui::gameui::menu`: added `last_armed: Option<usize>` tracking and sound queue to `MessageBox`. Emits `"ui/buttonrollover.wav"` on button hover entry and `"ui/buttonclickrelease.wav"` on button activation. Added unit test `message_box_sound_events_hover_and_release`.

**Why:**
- Half-Life 2 GameUI plays distinct audio cues on button rollover (`ui/buttonrollover.wav`) and activation (`ui/buttonclickrelease.wav`).
- UI audio must trigger only on new hover transitions (not repeated every frame while hovering), respect button release-activation rules, and play even while the simulation clock is paused.

**Tested how:**
- `cargo test --locked`: all 96 unit tests in `hl2-ui`, 64 in `hl2-bevy`, and 90 in `hl2-simulation` passed (0 failures).
- `cargo clippy --locked --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.

**Result:** Authentic GameUI sound feedback on hover and release across main menu, options dialogs, message boxes, and combo boxes, playing while paused and respecting master volume.

**Still broken or not tested:** Physical monitor audio output not directly listened to in headless environment.

**Next:** Task 5 (Close button on dialogs/menus).

## 2026-10-13 Button interaction model: pressed state, pointer release and cancellation semantics

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::widgets`: implemented reusable `ButtonTracker<T>` state machine managing press-down, hold, move-away cancellation, and release activation. Added `ButtonState { armed, pressed, enabled, default }`.
- `hl2-ui::gameui::widgets` drawing: styled pressed states for all interactive controls matching the Source scheme:
  - Push buttons (`Painter::button`): sunken bevel border and +1px,+1px text shift while pressed.
  - Checkboxes (`Painter::check_box`): `border_dark` cavity fill, +1px,+1px check offset, and armed/pressed highlight text color.
  - Radio buttons (`Painter::radio`): `border_dark` cavity fill, dot offset, and highlight text color.
  - URL link buttons (`Painter::url`): text highlight and +1px,+1px text/underline offset.
  - Unit tests: added `button_tracker_press_cancel_activate_state_machine` and `widget_pressed_states_drawing`.
- `hl2-ui::gameui::options`: wired `ButtonTracker<Target>` into `OptionsDialog`. Buttons, checkboxes, radio buttons, link buttons, and tabs activate on mouse release only when the cursor remains over the control; moving away while held cancels the pressed state and avoids triggering actions. Sliders retain immediate click-and-drag behavior. Added unit test `options_dialog_buttons_follow_press_cancel_and_release_activation`.
- `hl2-ui::gameui::menu` & `hl2-ui::console`: integrated `ButtonTracker` into `MessageBox` (Quit / Key Defaults confirmation) and console menus, routing mouse down, hold, and release transitions.
- `hl2-bevy::ui_system`: routed `buttons.just_released(MouseButton::Left)` through `hl2_ui::console::Input.released`.

**Why:**
- Source/VGUI buttons, checkboxes, radio buttons, and tabs activate on release rather than press-down, allowing users to cancel an accidental press by dragging away before releasing.
- Push buttons and interactive controls were missing the authentic sunken/depressed visual states during active mouse clicks.

**Tested how:**
- `cargo test --locked -p hl2-ui -p hl2-bevy`: all 94 unit tests in `hl2-ui` and 64 unit tests in `hl2-bevy` passed (0 failures).
- `cargo clippy --locked -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` ran cleanly and exited code 0.

**Result:** Authentic Source GameUI button interaction model across all widgets and dialogs, with full press/hold/cancel/release lifecycle and sunken/offset visual states.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 4 (UI sounds: hover, click, and GameUI audio events wired through existing audio infrastructure).

## 2026-10-13 Quick visual fixes: credits opacity, button typography and vertical spacing

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::options`: 3rd party technology credits dialogs (`Sub::AudioCredits` and `Sub::VideoCredits`) now render with 100% opacity (`frame_bg.a = 1.0`). Added unit test `credits_dialogs_have_100_percent_opacity` verifying that the credits frame background quad alpha is 1.0.
- `hl2-ui::gameui::menu`: doubled vertical spacing between menu buttons (`MENU_BUTTON_SPACING = 20.`) and replaced layout magic numbers with named constants (`BASE_SCREEN_HEIGHT`, `DEFAULT_TITLE_X`, `DEFAULT_TITLE_Y`, `DEFAULT_MENU_X`, `DEFAULT_MENU_Y`, `DEFAULT_TITLE_TALL`, `DEFAULT_ITEM_TALL`, `DEFAULT_FONT_TALL`, `MENU_ROW_BASE_WIDTH`). Updated `native_menu_uses_480_coordinate_scaling_with_static_spacing` unit test.
- `hl2-ui::console` & `hl2-ui::hud`: completely removed host OS font probing (`C:/Windows/Fonts`, `/System/Library/Fonts`, `/Library/Fonts`). All fonts now load strictly and exclusively from the owned game installation VFS (`resource/linux_fonts/liberationsans-regular.ttf`, `resource/linux_fonts/liberationsans-bold.ttf`, `resource/linux_fonts/DejaVuSans-Bold.ttf`, `resource/gordin-regular.ttf`, `resource/HALFLIFE2.ttf`), falling back only to the bundled MIT developer font (`ProggyClean.ttf`).
- `AGENTS.md` (root & HL2RS): installed hard legal safety rule mandating that all UI font loading must strictly read from the owned game installation VFS or permissively licensed project assets, never probing or depending on proprietary host OS font files.

**Why:**
- 3rd party technology credits dialogs were partially transparent (~83% alpha) when opened over the options dialog.
- Main menu and category submenu buttons used heavy bold typography and cramped vertical spacing.
- Probing proprietary host OS font files introduced legal ambiguity and platform inconsistencies; the owned Half-Life 2 installation already ships open (SIL OFL) and authored Source fonts directly in `platform/resource/linux_fonts/` and `resource/`.

**Tested how:**
- `cargo test --locked -p hl2-ui -p hl2-bevy`: all 91 tests in `hl2-ui` and 64 tests in `hl2-bevy` passed (0 failures).
- `cargo clippy --locked -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` passed and exited code 0.

**Result:** Credits dialogs render fully opaque, menu buttons render with lighter authentic font weight, and button rows have doubled spacing with named constants.

**Still broken or not tested:** Physical monitor visual inspection not performed in headless environment.

**Next:** Task 3 (Button interaction model: pressed state, pointer release/cancel semantics).

## 2026-10-13 Crate restructuring and module renames

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-bevy`: renamed 4 misnamed modules to describe their actual content:
  - `movement.rs` -> `simulation.rs`: represents the full Bevy host simulation runner (`Simulation` resource, tick loop, character physics, command script playback, cheats/effects, respawn).
  - `hud.rs` -> `canvas_renderer.rs`: represents the WGPU 2D rendering pipeline (`HudMaterial`, quad mesh pool) that renders the shared UI canvas (HUD, menus, options, console, dialogs) to the GPU.
  - `console.rs` -> `ui_host.rs`: represents the Bevy host UI resource (`Console`, alias `UiHost`) and input event translation (`keys`, `wheel_steps`, `binding_keys`).
  - `ui.rs` -> `ui_system.rs`: represents the Bevy ECS system plugin (`UiPlugin`, `update_ui`) for UI input dispatch, canvas resize synchronization, and window cursor management.
- Verified crate boundaries between `hl2-ui` and `hl2-bevy`: all portable UI domain logic, layout calculation, models, and viewport coordinate math reside in `hl2-ui`; `hl2-bevy` contains only Bevy host adapter, ECS systems, and GPU rendering pipelines with no circular dependencies.

**Why:**
- Previous module names in `hl2-bevy` were historical artifacts from the early prototype, obscuring the actual roles of the simulation runner, 2D canvas renderer, UI host adapter, and UI ECS systems.

**Tested how:**
- `cargo test --locked -p hl2-ui -p hl2-bevy`: all 90 tests in `hl2-ui` and 64 tests in `hl2-bevy` passed (0 failures).
- `cargo clippy --locked -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`: strict clippy passed with 0 warnings.
- Headless Bevy runtime verification: `cargo run --locked -p hl2-bevy -- --frames 30 --no-focus` ran cleanly and exited code 0.

**Result:** Crate ownership boundaries and module naming cleanly separated without functional or behavioral regressions.

**Still broken or not tested:** Non-macOS physical display checks not tested.

**Next:** Task 2 (Quick visual fixes: credits dialog opacity and button typography/spacing).

## 2026-10-12 Standard resolutions, 4:3 aspect ratio and pixel-scale windowed mode

**Agent/model:** Antigravity agentic assistant; Gemini 3.8 Flash model family.

**Changed:**
- `hl2-ui::gameui::display`: added `STANDARD_MODES` (23 standard resolutions covering 4:3, 5:4, 16:9, 16:10 from 640x480 up to 4K) merged with monitor-reported modes in `usable_modes`; introduced `DisplayModes` carrying full-screen modes and desktop pixel bounds.
- `hl2-ui::gameui::settings`: resolution choices now filter by the draft display mode (`DisplayModes::choices`); switching display mode or aspect updates the available list and automatically falls back to the largest fitting candidate if the current resolution exceeds desktop bounds.
- `hl2-bevy::video`: windowed mode resize now sizes the window client area in physical pixels (`set_physical_resolution`), giving 1:1 pixel rendering and allowing resolutions up to Retina native without 2x point ballooning; monitor query supplies desktop pixel bounds.
- `hl2-bevy::ui`: added `debug!` telemetry logging cursor position, window dimensions, scale factor, viewport bounds, and canvas pointer coordinates on click.

**Why:**
- High-DPI/Retina monitors only reported a few high-res 16:10 modes, leaving the Normal (4:3) aspect bucket empty and omitting common smaller resolutions like 720p.
- Windowed mode previously sized windows in logical points, which doubled window dimensions on 2x Retina displays and made resolutions above half the screen size fill or exceed the display.
- Verified click responsiveness and coordinate mapping across windowed resolution changes.

**Tested how:** `cargo test --locked -p hl2-ui -p hl2-bevy` (all pass: 64 + 89 passed); strict Clippy (`-D warnings`); `cargo fmt --all`. Live packaged run on macOS M1 Pro Retina (`./launch-bevy.sh --map background01 --main-menu --windowed --width 1280 --height 720`): verified live interactive navigation in Options -> Video, changing resolutions across 1600x1200, 2048x1536, 640x480, 1024x768, and 1280x1024 with click dispatch and window resizing confirmed operational. See docs/gameui.md section 10b.

**Result:** 4:3 aspect ratio and standard resolutions are available on all displays; windowed mode runs at native 1:1 pixel scale up to display native resolution; click response verified.

**Still broken or not tested:** Windows and Linux display mode queries not tested on physical hardware. Fullscreen monitor mode switching not tested.

**Next:** Windows verification; owner check.

## 2026-10-11 Safe runtime resolution change

**Agent/model:** Antigravity agentic assistant; Claude Opus model family (exact identifier not exposed).

**Changed:**
- `hl2-bevy::video`: new `suspend_for_resize` system. When the resolution changes, it deactivates every camera that renders into the game frame, resizes the frame image only while they are inactive, and resumes rendering `SUSPEND_FRAMES` (4) frames later with each camera's previous activity restored. The presentation sprite is hidden (black) meanwhile. `present_frame` no longer resizes the image.
- `apply_video_settings` now applies pending video settings in `Update`, before `resize_window`. It used to run inside the menu update, which skips while a movement script runs.
- `hl2-ui::console`: added `mat_setvideomode <width> <height> <windowed 0|1>`.
- `debug!` tracing of resize requests, suspension, image resize, resume and window resize steps.

**Why:** Changing the resolution crashed the game in windowed and full-screen modes. A wgpu validation error reported differing attachment sizes: the camera's cached target size, and so its depth texture, is refreshed one frame after the frame image is resized in place.

**Tested how:** `cargo test --locked -p hl2-ui -p hl2-bevy` (all pass, including the updated stop/resize/resume test); strict Clippy; `cargo fmt --all`. Packaged macOS runs driven by `mat_setvideomode`, both exit 0 with no validation errors: windowed 1280x720 → 1024x768 → 1600x900 → 1280x720 (crashed before the fix), and windowed → full screen 1280x720 → full screen 1024x768 → windowed. See docs/gameui.md section 10a.

**Result:** Runtime resolution changes no longer crash, and the window follows them.

**Still broken or not tested:** sky cameras may show a stale activity for one frame after resuming. Not tested: green-button full screen, macOS full-screen cursor size, Windows and Linux.

**Next:** owner check on macOS; Windows verification.

## 2026-10-11 Resolution, display mode and scaling

**Agent/model:** Antigravity agentic assistant; Claude Opus model family (exact identifier not exposed).

**Changed:**
- `hl2-bevy::video`: added the `GameFrame` offscreen target at the configured resolution. All scene, viewmodel and HUD cameras except `_rt_Camera` render into it, and a presentation camera draws it into the window with linear filtering, aspect fit and black bars.
- Windowed OK/Apply and windowed startup now size the window to the resolution in logical points. The size is clamped to the monitor and re-requested through mode transitions. It is not applied while the window is in OS (green-button) full screen.
- macOS full screen is now a full-screen window over the desktop mode, so the panel is never switched to an unscaled mode (tiny cursor).
- `hl2-ui::viewport`: cursor mapping and canvas size now use frame pixels; added `windowed_size`.

**Why:** Changing the resolution did not resize the window, and full screen rendered at the display's native resolution instead of the setting.

**Tested how:** `cargo test --locked -p hl2-ui -p hl2-bevy` (all pass); strict Clippy; `cargo fmt --all`. Packaged macOS captures: windowed 1024x768 (2048x1536, 2x upscale), full screen 1280x720 (3456x2168, upscaled and letterboxed), windowed 3456x2234 startup clamp (1579x1021 points). See docs/gameui.md section 10.

**Result:** The game frame resolution follows the setting in every mode; the window follows it in windowed mode.

**Still broken or not tested:** not tested: menu-driven windowed resize in a packaged run, green-button full screen, macOS full-screen cursor size, live mouse input through the scaled frame, Windows and Linux. No real monitor mode switching.

**Next:** owner check on macOS; Windows verification.

## 2026-10-11 GameUI settings used by the game

**Agent/model:** Antigravity agentic assistant; Claude Opus model family (exact identifier not exposed).

**Changed:**
- `hl2-bevy::input`: all gameplay actions read through the stored bindings, including mouse buttons and wheel (`+attack`/`+attack2`, `invprev`/`invnext`); empty or unknown bindings are unbound; Source mouse filter/acceleration at 0.022 degrees per count; skill applied every frame.
- `hl2-ui`: `""` marks an unbound action (Clear Key, reassignment, Use Defaults); `migrate` adds missing default actions when their key is free; `invprev`/`invnext` wheel defaults; macOS Duck/Walk defaults from the retail `bind_mac` lines; `MouseFilter`.
- `hl2-simulation`: `Library::resolve_wave` reports the `#` (dry-mix) sound character; `is_music`; `PlayerDamage::take_scale_for`.
- `hl2-bevy::audio`: per-frame sink volume includes the master volume, the music volume for music channels and mute when unfocused.
- `hl2-bevy` startup: stored display mode, `--windowed`/`--fullscreen`/`--borderless`, unattended runs start windowed.

**Why:** The Options dialog stored these settings but the game ignored most of them.

**Tested how:** `cargo test --locked -p hl2-ui -p hl2-bevy -p hl2-simulation -p source-assets` (85 + 61 + 169 + 90 passed); `cargo test --locked -p hl2-ui -- --ignored` (3 passed); strict Clippy; `cargo fmt --all --check`; packaged macOS Keyboard-page capture and a direct-map run (docs/validation.md).

**Result:** Bindings, mouse filter/acceleration/invert, skill, music volume, mute when unfocused and the stored display mode now affect the game.

**Still broken or not tested:** gamma, HDR, speakers, sound quality, captions, language and aim assist are stored only; music ducking not modeled; live packaged keyboard/mouse play, audible checks, full-screen/No Border startup and Windows/Linux not tested.

**Next:** owner review of the GameUI branch.

## 2026-10-11 GameUI Options dialog

**Agent/model:** Antigravity agentic assistant; Claude Opus model family (exact identifier not exposed).

**Changed:**
- `hl2-ui::config`: typed video/audio/mouse/gameplay settings, `DisplayMode` with platform choices (Windowed (No Border) hidden only on macOS, stored values kept), legacy `borderless` migration, clamps; retail `+attack`/`+attack2` default bindings.
- `hl2-ui::gameui::{settings, options}`: draft settings model, control bindings, and `OptionsDialog` (tabs, pages from installed `optionssub*.res`, OK/Cancel/Apply, brightness/defaults/credits sub-dialogs, key capture, placeholder logging). Replaces the previous hand-laid `hl2-ui::options`.
- `hl2-ui::gameui::widgets`: check box, radio, slider, text entry, combo, list with scroll bar, divider, URL, tab and image widgets.
- `hl2-bevy`: mouse wheel and held-button input, Source key names for capture, display modes from `Monitor::video_modes`, window mode/resolution applied on OK/Apply.

**Why:** Owner-requested resource-driven Options dialog.

**Tested how:** `cargo test --locked -p hl2-ui -p hl2-bevy` (83 + 56 passed, 3 + 2 ignored); owned-install ignored tests pass; strict Clippy; `cargo fmt --all --check`; packaged macOS captures of every page and the open display-mode list (docs/validation.md).

**Result:** All six tabs render from installed resources and localization; settings round-trip through `hl2-config.json`.

**Still broken or not tested:** many stored settings are not consumed by the host yet (docs/gameui.md section 6); brightness dialog only unit-tested; Windows/Linux, fullscreen mode switching and packaged key capture not tested.

**Next:** host wiring of the stored settings.

## 2026-10-11 GameUI resources, startup flow and main menu

**Agent/model:** Antigravity agentic assistant; Claude Opus model family (exact identifier not exposed). One read-only research subagent surveyed UI patterns before planning.

**Changed:**
- `hl2-ui::gameui::{localize, layout, keys, display, resources}`: install-driven localization (UTF-8 and UTF-16 LE files, key-name fallback), VGUI `.res` layouts, `gamemenu.res` categories, key actions/default binds and display-mode helpers.
- `hl2-ui::gameui::{widgets, menu}`: scheme theme, widget fonts and painter; `GameMenu` (categories, spacers, disabled placeholders, keyboard/pointer navigation) and the QUIT GAME `MessageBox`. The console host uses them for the main and pause menus.
- `hl2-bevy`: startup without a loadable `--map` opens the main menu over the paused `background01`; map-dependent arguments without a loadable `--map` fail the launch; background levels keep the HUD hidden and the cursor free.

**Why:** Owner-requested resource-driven GameUI (startup, main menu, options).

**Tested how:** `cargo test --locked -p hl2-ui -p hl2-bevy` (69 + 54 passed, 2 + 2 owned-install ignored); strict Clippy; `cargo fmt --all --check`; packaged macOS captures (see docs/validation.md).

**Result:** Main menu, categories and quit box render from installed resources; startup rules behave as specified.

**Still broken or not tested:** background map frozen and seen from the spawn; Load Game, Achievements, Extras and Episode One/Two New Game are placeholders; Windows/Linux not tested.

**Next:** options dialog framework and pages.

## 2026-10-09 UI/input and player-movement boundary

**Agent/model:** Codex Desktop; exact model identifier not exposed in this session.

**Changed:**
- Added portable `hl2-ui::input` binding lookup, including a unit test for configured/default resolution. It deliberately remains native-key-type agnostic.
- Kept viewport fitting, pointer translation, and canvas resize in `hl2-ui::viewport`; `hl2-bevy::video` now converts explicitly between its Bevy math types and the portable crate's `glam` types, so the two dependency versions do not leak across the boundary.
- Moved Bevy keyboard/mouse-to-gameplay adaptation into `crates/hl2-bevy/src/input.rs`; `ui.rs` handles UI focus/text/pointer/cursor state, while `input.rs` handles captured gameplay actions and character intent.
- Moved player-view camera publication into `crates/hl2-bevy/src/camera.rs`; `movement.rs` schedules fixed character movement and no longer contains native event mapping or camera presentation.
- Recorded the bounded architecture and fixed-step ordering in `docs/DESIGN.md`.

**Why:** Continue separating portable UI/render/input policy from Bevy platform adaptation and stop UI/render concerns accumulating in the character-movement module.

**Tested how:** `cargo fmt --all --check`; `cargo test --locked -p hl2-ui -p hl2-bevy` (91 passed, 2 owned-install tests ignored); `cargo clippy --locked -p hl2-ui -p hl2-bevy --all-targets -- -D warnings`.

**Result:** Existing menu capture, held-jump, remapped key, mouse sensitivity, viewport, and camera scheduling tests stay green. Packaged runtime capture and workspace-wide validation are not tested in this session.

## 2026-10-09 Decouple UI & Canvas from Movement into Dedicated ui.rs & Fix Letterbox Hit-Testing

**Agent/model:** Antigravity agentic assistant (OrbitalMan pair-programming session, Gemini 3.8 Flash).

**Changed:**
- `crates/hl2-bevy/src/ui.rs`:
  - Created dedicated UI module and Bevy plugin (`UiPlugin`) for UI canvas management, text/keyboard input forwarding, cursor grab/visibility toggles, settings propagation, and pointer hit-testing.
  - Implemented `cursor_to_canvas`: maps window coordinates to UI canvas coordinates offset by `video_viewport.logical_offset`, returning `None` if the cursor is outside the active viewport (e.g. over letterbox/pillarbox black stripes).
  - Implemented `resize_canvas`: guarantees UI canvas dimensions match `video_viewport.logical_size` (or window logical size) prior to processing UI input, resolving the 1:1 hit-test mismatch between visual centering and click coordinates.
  - Added unit tests: `cursor_to_canvas_maps_inside_letterbox_viewport`, `resize_canvas_resizes_to_viewport_logical_size`, and `update_ui_system_resizes_canvas_and_syncs_mode`.
- `crates/hl2-bevy/src/movement.rs`:
  - Stripped all UI, text event forwarding, pointer translation, canvas resizing, and video configuration application logic out of `movement::controls`.
  - Restored `movement.rs` to solely handle player simulation and motion: walking, jumping, ducking, sprinting, noclip flight, weapon selection, and mouse look.
  - Updated movement unit tests to step input via `update_ui` then `controls`.
- `crates/hl2-bevy/src/main.rs`:
  - Declared `mod ui;` and registered `ui::UiPlugin` in `RunFixedMainLoopSystems::BeforeFixedMainLoop`, ordered before `movement::MovementControlsSet`.

**Why:** The user requested to stop bloating `movement.rs` with canvas and UI-related code, move UI and canvas logic to a dedicated file, and fix button hit-test offsets in letterboxed/pillarboxed viewports where visual button centers did not match clickable regions.

**Tested how:**
- `cargo test --workspace`: all 171 tests passed (50 hl2-bevy, 33 hl2-ui, 88 source-assets).
- `cargo clippy --workspace --all-targets -- -D warnings`: passed with 0 warnings.
- `cargo fmt --check`: passed cleanly.

**Result:** `movement.rs` is solely focused on player movement. UI and canvas logic is isolated in `ui.rs`. Button hit-testing matches visual positions 1:1 in letterbox/pillarbox viewports.
## 2026-10-09 Viewport Renderer Output Resolution and Letterboxing/Pillarboxing

**Agent/model:** Antigravity agentic assistant (OrbitalMan pair-programming session, Gemini 3.8 Flash).

**Changed:**
- `crates/hl2-bevy/src/video.rs`:
  - Decoupled video resolution from OS window frame size: configured video resolution now acts strictly as the renderer output and camera viewport resolution.
  - Implemented `VideoViewport` resource with automatic aspect-ratio calculation:
    - Pillarbox (vertical black stripes on left & right) when window is wider than target aspect ratio (e.g. 4:3 target on 16:9 display).
    - Letterbox (horizontal black stripes on top & bottom) when window is taller than target aspect ratio (e.g. 16:9 widescreen cinema target on 4:3 display).
    - Full-frame fill when aspect ratios match.
  - Added `update_viewports` system synchronizing camera viewports across all primary window rendering cameras (`FlyCamera`, viewmodel, skybox, HUD) on both `PreUpdate` and `PostUpdate`.
  - Added comprehensive unit tests for pillarboxing, letterboxing, exact aspect filling, High-DPI scale factors, window size preservation, and camera synchronization.
  - Kept default `ClearColor` as `Color::BLACK`. Swapchain clear pass in wgpu clears the full texture to black, while viewport and scissor constrain scene and 2D cameras to the letterbox bounds. (Fixed crash on launch: an initial separate `Camera2d` spawned for letterboxing defaulted to Bevy's MSAA sample count of 4, causing wgpu pass parameter validation error with the 1-sample swapchain/depth texture; removed the redundant camera).
  - Initialized `VideoViewport` resource and registered `update_viewports` systems.
- `crates/hl2-bevy/src/hud.rs`:
  - Sized HUD canvas to `video_viewport.logical_size` instead of the full unconstrained window size, ensuring UI, health, ammo, crosshair, and menus render within the letterboxed viewport.
- `crates/hl2-bevy/src/movement.rs`:
  - Offset mouse pointer coordinates for `ui.source.input` by `video_viewport.logical_offset`, ensuring clicking in menus and options dialog aligns 1:1 with visual elements in letterboxed frames.
- `crates/hl2-ui/src/options.rs`:
  - Expanded `common_resolutions` in Video Options to include classic 4:3 (640x480, 800x600, 1024x768, 1280x960, 1600x1200), 16:10 (1280x800, 1440x900, 1680x1050, 1920x1200), 5:4 (1280x1024), and 16:9 (1280x720, 1600x900, 1920x1080, 2560x1440, 3840x2160) resolutions.
  - Added aspect ratio formatting (`1024 x 768 (4:3)`, `1920 x 1080 (16:9)`, etc.) in the options dialog.

**Why:** The user specified that video resolution in settings represents renderer output / viewport resolution, not OS window size, and must fit inside the window or fullscreen frame with letterboxing/pillarboxing black stripes on mismatched aspect ratios without distortion.

**Tested how:**
- `cargo test --workspace`: 88 source-assets + 33 hl2-ui + 46 hl2-bevy unit tests passing (167 passing tests).
- `cargo clippy --workspace --all-targets -- -D warnings`: passed with 0 warnings.
- `cargo fmt --check`: passed cleanly.
- `target/debug/hl2-bevy --map d1_trainstation_02 --frames 30 --capture artifacts/test_res.png --no-focus`: completed 30 frames and capture with 0 errors.

**Result:** Clean letterboxed and pillarboxed viewport rendering with solid black stripes preserving aspect ratio without distortion.
## 2026-10-09 Video Resolution Scaling and Display Metrics Architecture

**Agent/model:** Antigravity agentic assistant (OrbitalMan pair-programming session).

**Changed:**
- `crates/hl2-bevy/src/video.rs`:
  - Structured display and resolution management into modular submodules: `display` (`DisplayMetrics`), `resolution` (`ResolutionPlan`), and `mode` (`desired_mode`).
  - Added physical-to-logical resolution scaling calculations ($\text{Logical Size} = \text{Physical Resolution} / \text{Scale Factor}$) preventing oversized window backing textures and GPU depth attachment extent mismatch errors (`wgpu` validation error: `Attachments have differing sizes`) on High-DPI / Retina displays.
  - Implemented macOS workspace fullscreen detection (`is_workspace_fullscreen`) to protect against window resizing while in AppKit native workspace fullscreen.
  - Added unit tests: `changes_resolution_with_scale_factor` and `workspace_fullscreen_protected_from_resolution_desync`.

**Why:** Changing resolution in windowed mode on High-DPI/Retina screens caused physical textures to exceed monitor bounds, leading to OS window clamping, desynchronization between color and depth attachments, and wgpu crash with blinking magenta screen.

**Tested how:**
- `cargo test --bin hl2-bevy`: 44 unit tests passing (including new scale factor and workspace fullscreen tests).
- `cargo clippy -p hl2-bevy --all-targets`: passed with 0 warnings.
- `cargo fmt --check`: passed cleanly.
- `target/debug/hl2-bevy --frames 60 --no-focus`: verified 60 frames on Apple M1 Pro (Metal backend) loading `d1_trainstation_02` with 0 validation errors.

**Result:** Clean resolution scaling and crash-free display mode and resolution switching.

## 2026-10-15: main CI fix (impact table address identity)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** `impact_damage::NPC_TABLE` and `GLASS_TABLE` are `static` instead of `const`. No behavior change: both were only used by reference.

**Why:** GitHub "Rust checks" on main failed since 675c9df (runs 38523057505, 38659681714): `glass_table_breaks_bottles_on_light_drops_and_keeps_its_row_order` asserts `std::ptr::eq(table_named(Some("glass")), &GLASS_TABLE)`. Each `&CONST` is a separately promoted value with no guaranteed shared address; release builds merged them (local `--release` passed), CI's debug `cargo test` did not. A `static` has exactly one address.

**Tested how:** in a clean origin/main worktree, the CI commands: `cargo +stable fmt --all --check`, `cargo +stable test --workspace --locked` (all pass; hl2-simulation 233 passed / 18 ignored, including the glass test), `cargo +stable clippy --workspace --all-targets --locked -- -D warnings` (clean).

**Result:** the CI failure is fixed locally; the GitHub run on this push is the confirmation.

**Still broken or not tested:** owned-file (`--ignored`) tests and packaged runs not rerun (no runtime change).

**Next:** none for CI.

## 2026-10-15: friendly game installation setup on main

**Agent/model:** Codex Desktop, GPT-6 family (exact model identifier not exposed). No subagents.

**Changed:** independent bootstrap window with editable/pasteable folder, native Browse, Retry detection, Continue/Cancel; remembered per-user installation, automatic recovery/discovery, `--setup`, `--no-setup` and a setup-preview diagnostic. Root/hl2-subfolder normalization and bounded map/core-content checks reuse source-assets/VFS. A helper owns the setup event loop before gameplay starts. Added beginner **Setup steps** near the top of README. Discovery now returns its validated root.

**Why:** owner requested friendly first-launch setup and explicitly requested publication based on main without NPC changes.

**Tested how:** based on main 675c9df: five setup/control tests; workspace tests 459 passed / 48 ignored / one unchanged physics pointer-identity test excluded; strict Clippy, formatting and diff/whitelist checks. Main release package rebuilt, setup screenshot visually inspected, station02 startup captured. Earlier owned readiness, missing-content and preference checks are recorded in docs/validation.md. Evidence: artifacts/friendly-setup-20261015/main.

**Result:** only setup source, its dependencies, README and setup documentation are applied to main. NPC logic, animation, gameplay wiring and NPC branch references are untouched. The test-only physics adjustment from the earlier feature branch is also excluded. Initial rejected captures are preserved; preview now waits for renderer readiness.

**Still broken or not tested:** the unchanged glass-table address assertion fails locally; it is documented, not modified. Native picker/clipboard interaction, title-bar cancellation and the complete interactive Continue-to-game transition could not be exercised because Windows computer-use timed out through recovery; control/validation/response paths have programmatic tests. macOS/Linux setup is **not tested**. Content checks are not a full integrity/compatibility audit.

**Next:** exercise native picker/clipboard on supported desktops when computer use is available. Existing campaign work remains separate.

## 2026-10-14 session 17: explosive props (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** prop_data `explosive_damage`/`explosive_radius` (PropData, over the entity's ExplodeDamage/ExplodeRadius keyvalues, not on *_override props). CBreakableProp::Break of such a prop queues ExplosionCreate at its center (magnitude and radius scaled by modelscale, truncated to integers; radius 0 = magnitude x 2.5, env_explosion); the host detonates them after the tick's damage and before gibs spawn (native order: the explosion precedes PropBreakableCreateAll; a one-tick deferral let the barrel's own gibs block its blast), repeating so chains resolve in the tick (bounded at 32). `Projectiles::explode_props`; `grenade_explosion` takes position/magnitude/radius.

**Why:** owner question (session 17): explosive barrels did not explode.

**Tested how:** unit test `breaking_an_explosive_prop_queues_its_explosion`, hl2-simulation 233 passed, owned tests (HL2_ROOT, `--ignored`, workspace: 47 passed), strict workspace Clippy, fmt. Packaged d1_canals_01 (oildrum001_explosive: health 20, explosive 120/256), walking, eye (406,-2790,452) yaw 90 pitch -14, pistol at barrel #635: it breaks on the 4th shot, explodes (magnitude 120, radius 256) and chain-detonates #634 in the same tick; 17 gibs; the player (128-160 units away) takes about 60 + 45 and dies (SDK falloff). In fly mode the player takes no damage at all (pre-existing: player damage runs only in the walking branch). The packaged asset cache (bin/bevy-assets) must be rebuilt for new PropData fields: target/release runs used the old cache and saw no explosion.

**Result:** explosive barrels explode and chain.

**Still broken or not tested:** native comparison not done; fire is not modeled: native barrels ("ignite" "halfhealth", "explosive_resist") catch fire at half health from bullets and explode after burning, and blasts make them burn 0.1-5 s before exploding; ours explode at 0 health, chained ones instantly; PROPINTER_PHYSGUN_BREAK_EXPLODE (propane tanks) variant; explosion effect/decal flags (SURFACEONLY etc.) not distinguished; barrel gibs/flames.

**Next:** record the physics/arsenal leftovers, regression batch, merge to main, then step 14 (owner, session 17).
## 2026-10-14 session 17: explosions break prop_data props; SDK blast blocking (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** `Projectiles::grenade_explosion` (frag, SMG grenade, RPG) now sends DMG_BLAST damage to every prop_physics (the entity damage dispatch resolves prop_data health and skips unbreakable props); before, only props with an explicit `health` keyvalue were damaged, so prop_data props (crates, pallets, bottles, boxes) were only pushed. Blast line of sight follows CGameRules::RadiusDamage: the world or a brush entity blocks; an interposing studio-model entity never shields fully (the CONTENTS_SOLID re-trace's StandardFilterRules skips non-brush models): a physics object absorbs mass/350 of the damage (all from 350 kg), others 25% (`blast_blocked_fraction`); before, any prop shielded completely. `Physics::filtered_ray`, `Physics::entity_mass`.

**Why:** owner question (session 17): do explosives break props? They did not (owner saw bottle #702 untouched next to an RPG blast).

**Tested how:** unit test `interposing_props_absorb_blast_by_mass_instead_of_shielding`, hl2-simulation tests 232 passed, strict Clippy, fmt. Packaged d1_trainstation_02, eye (-3490,-2110,128) fly, yaw 236: RPG (pitch -30, blast at (-3565,-2210,64)): before the fix bottle #701 broke only by impact, bottle #702 survived because box #700 blocked the ray 1.5 units before it; after: box #700 and bottles #701/#702 break (6 glass chunks). SMG grenade (pitch -20) and frag lob (pitch -60, blast (-3516,-2165)): the same three break. Native (original-oracle T060459Z, same eye, pitch 30, weapon_rpg): box and both bottles gone after the blast.

**Result:** frag, SMG grenade and RPG blasts break prop_data props like native in the tested spot.

**Still broken or not tested:** the cardboard box's gib is a ragdoll gib (skipped until ragdolls); our glass chunks flew ~300 units while native shards stayed near the spot (not investigated); no scorch decal; explosive props (prop_data explosive_damage/radius: barrels) do not explode; non-lethal blast damage does not ignite props (CBreakableProp::OnTakeDamage Ignite); robust NPC radius damage (edge spill) and BodyTarget points not modeled.

**Next:** explosive props, then the session 16 open items.
## 2026-10-14 session 16 (continued): env_physexplosion, prop OnPlayerUse, TraceBleed, world-collision-disabled constraints (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** `hl2_simulation::phys_explosion` (env_physexplosion Explode input: targets by name, else class within the radius, else everything in the radius; SDK falloff and explosive damage force applied at the explosion origin; DMG_BLAST damage unless NODAMAGE; report `gameplay.phys_explosions`); prop_physics with spawnflag 256 fire OnPlayerUse on +USE (pickup path and ray-use path); world constraints with SF_CONSTRAINT_DISABLE_COLLISION make their body ignore static colliders without an entity (BSP, displacements, static props); TraceBleed (CBaseEntity::TraceBleed: 1/2/4 noisy traces 172 units along the shot behind a hit NPC, Blood/YellowBlood decal groups by BloodColor, machines none; `Inventory::blood`, `Impacts::add_blood`); `Physics::remove_body` updates the query pipeline (a faded gib left a stale entry: one intermittent Rapier 'No element at index' panic in batch s16b, d1_trainstation_01, where debris bottles #869/#870 authored ~47 units above the displacement now shatter on landing under the glass table); movement-report sample field `awake_bodies`. Fixtures bevy-tictactoe-use-spins-block, bevy-trace-bleed-cop-wall. Oracle (private): `!use`/`!unuse` and `!npc` (npc_create, ORACLE_NPC) view suffixes (backup run_view_oracle.py.bak-20261014).

**Why:** owner items 0c (env_physexplosion, the playground tic-tac-toe blocks and ghost swings) and 0b (TraceBleed world blood behind NPCs, native blood comparison).

**Tested how:** unit tests (explosion impulse band and blast damage; TraceBleed trace counts, wall hits, mechanical NPCs), owned tests (decal preload count 56), strict Clippy, fmt, workspace tests; packaged: +USE on block 5 fires one explosion and the block turns (before: it sat 4.7 units inside the frame rod and could not move), native T001349Z at eye (-4540,-3599,64) pitch 15 yaw 0; three pistol shots at terminal_side_cop posed at (-3330,-2560,64) in front of the hall wall: 3 blood decals on the wall behind him (bleed-side.png), native T002821Z (npc_create cop at the same spot): splats on the same wall/pillar and on the chest; native's are larger and the native cop died after 3 shots (ours survives: NPC health is item 3). Regression batch artifacts/s16b-regression: weapons 17/17, attention 26/26, movement 25/26 (doll01 only), no panics; finals vs s16-regression mean <= 0.51; Breen screen feed differs by 24 pixels at the mouth (one lip-sync frame, to recheck), slate feed identical.

**Result:** the tic-tac-toe blocks react to +USE; shot NPCs leave blood on the wall behind them like native.

**Still broken or not tested:** native turns the block about a quarter turn and it settles within 0.3 s; ours spins at the angular speed cap for over 6 s (open, research needed: IVP hinge friction units / impulse transfer / angular drag); swing pushes need ropes (off); PUSH_PLAYER; carousel thruster.

**Next:** STATUS session 16.
## 2026-10-14 session 16: prop data, chunk gibs, displacement and clip collision, world hinges (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** (1) DESIGN 13e steps 4-5: the whole prop_data block resolves over its propdata.txt base chain (`World::model_prop_data`: dmg.bullets/club/explosive, damage_table, breakable_model/count/skin) and the BreakableModels chunk sets (`World::breakable_chunks`); entity damage carries DMG_* bits (`projectiles::Damage::kind`); breakables take GetBreakableDamage (`gameplay::breakable_damage`, CBreakable func_breakdmg defaults for func_breakable/func_physbox); player bullets and melee on breakables go through the entity damage path (props with prop_data health now break under fire: before, only props with an explicit health keyvalue did, without gibs or break sound); glass impact table for damage_table "glass" (`impact_damage::GLASS_TABLE`, retail registry = SDK); gibs take the parent's skin and the break block's collision group (fadetime -> debris); breakable props without .phy gibs break into their chunk set (`Gibs::spawn_chunks`: count, max size, mid-plane points, 100 u/s burst, AlignBoxes, breakable_skin, 5-10 s fade); the host preloads gib skins and used chunk models. (2) Physics fixes found on the way: displacement colliders reversed (BSP clockwise winding made Rapier's oriented trimesh inside-out: props fell through displacements) and built without internal-edge correction; player/NPC clip-only brushes have no contacts with physics objects and shot/use traces skip them (`Physics::ground_ray` keeps them for the player's ground traces); logic_collision_pair startdisabled; SF_CONSTRAINT_ASSUME_WORLD_GEOMETRY world hinges as revolute joints after the SDK mass-center snap onto the axis; bullet/melee push = ApplyForceOffset at the hit point with the ammo's BULLET_IMPULSE / melee damage x 300 kg in/s (`Physics::impulse_at`, `gameplay::bullet_impulse`). Fixtures bevy-pistol-breaks-bottle-skin4, bevy-pistol-breaks-teeter-seat-chunks.

**Why:** owner item 0a (propdata damage tables/modifiers, chunk sets, parent skin on gibs) and 0c (assume-world-geometry hinges). The pot falling through the playground and the teeter seat flying off surfaced while testing them.

**Tested how:** unit tests (modifiers, glass table order, chunk placement/alignment/skin, gib skin/render key, clip brushes vs bodies and traces), owned tests (prop_data modifiers/tables/chunk sets; 18 + 24 owned pass), workspace tests (the two known release-only Bevy should_panic tests run in debug), strict Clippy, fmt; packaged fixtures and native oracle views (docs/validation.md session 16); regression batch artifacts/s16-regression (same as s15b).

**Result:** bottles and chunk-set wood props break under fire like native, in the right skin; props rest on displacements; the teeter seat sits on its hinge.

**Still broken or not tested:** the teeter seat rocks far less than native under pistol fire and native broke it in 5 shots (ours 8); ragdoll gibs; explosive props; gib limits; carousel (SF 9 + phys_thruster) not checked; other displacement maps not checked visually.

**Next:** env_physexplosion + OnPlayerUse (tic-tac-toe blocks, ghost swings), then the remaining item-0 work (STATUS session 16).
## 2026-10-13 session 15 (end): debris props (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** prop_physics with SF_PHYSPROP_DEBRIS (4) use a DEBRIS collision group (SDK CGameRules::ShouldCollide: COLLISION_GROUP_DEBRIS collides only with COLLISION_GROUP_NONE): world and ordinary props yes; player movement (incl. the exact convex path), the player shadow, NPC hulls/models, thrown frags and other debris no; traces (bullets, +USE, gravity gun) still hit them. 18 of 66 prop_physics on d1_trainstation_02 carry the flag (e.g. bottle #701, box #700). Fixture bevy-carry-bottle-shove now starts at (-3356,-2520,128) (the old spot put the walking hull inside world geometry: stuck, so no crouch).

**Why:** owner item 0d (carried bottle shoved into the floor, crouch while carrying): crouch works while carrying; the carried debris bottle overlapping the crouched hull froze the player via CheckStuck, which native avoids because debris never collides with the player.

**Tested how:** unit test (a debris prop lets the player hull through and still stops a pick ray; a non-debris prop blocks), workspace tests, strict Clippy, fmt; packaged: crouched carry looking down: no stuck samples, no hull contacts (before: stuck from d186 with a 0.6-unit overlap), bottle minimum z 66.5 (floor rest 66.6). Regression batch artifacts/s15b-regression: weapons 17/17, attention 26/26, movement 25/26 (doll01 only); finals vs s15-regression mean <= 0.59, feeds identical.

**Result:** debris props behave like native toward the player; the carried-bottle freeze is gone.

**Still broken or not tested:** interactive-debris return to debris at rest (irrelevant until non-debris props are converted); native comparison of walking through debris; owned tests not rerun for this change (no asset path changed).

**Next:** see STATUS session 15 and work/publishing/next-session-prompt-20261013c.txt.
## 2026-10-13 session 15 (continued): studio decals along the shot (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** weapon hits keep their shot direction (`Inventory::impacts` entries gain it; `Impacts::add_shot`), and NPC studio decals are shot along it instead of the reversed trace-shape normal.

**Why:** FX_ImpactTrace shoots decals along the ray; the trace shape's normal missed the skin on grazing hits.

**Tested how:** workspace tests, strict Clippy, fmt; packaged `bevy-studio-decal-cop.json`: 3/3 shots build blood decals at the hit points (was 2/3), sdecal4-zoom.png. Owned tests not rerun for this change (no owned-data path changed).

**Result:** blood lands where the bullet hits.

**Still broken or not tested:** native comparison of NPC blood.

**Next:** handoff.
## 2026-10-13 session 15 (continued): VPhysics constraints (hinges), spawn-embedded CCD (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** (1) `Physics::add_constraints` (SDK physconstraint.cpp ActivateConstraint/CreateConstraint): phys_hinge (revolute about hingeaxis - origin; hingefriction x 1000 kg in^2/s^2 as a capped zero-velocity motor), phys_ballsocket, phys_ragdollconstraint (spherical with x/y/z limits in the constraint angles), phys_constraint (fixed) as Rapier impulse joints between the attach1/attach2 bodies (missing or non-physics = a fixed world body); SF_CONSTRAINT_DISABLE_COLLISION. Reported, not built (`constraint_errors`): phys_lengthconstraint (rope joints rejected: the swing seats first dropped ~19 units then were flung to z 245, also with CCD off for jointed bodies), constraints with SF_CONSTRAINT_ASSUME_WORLD_GEOMETRY (native BecomeHinged; the impulse teeter-totter hinge inside its static base drifted 115 units; multibody joints panicked in Rapier: "Matrix slicing out of bounds"), unknown attachments. (2) CCD gating fix: props authored interpenetrating a static entity (found once after the first step, contact deeper than 1 unit; world geometry and NPC hulls excluded) keep permanent CCD: with this morning's gating the teeter-totter seat was ejected from its base (to -4756 / -4406). A per-tick variant was rejected (kept CCD on for props pressed into hulls/terrain: plaza tip mean 7.7 ms). (3) The owned impacts test now expects the 11 studio-decal textures (32; it failed after the studio decal commit, which was pushed without rerunning owned tests).

**Why:** owner item 0c: the playground tic-tac-toe blocks fall and jitter; native holds them on hinges.

**Tested how:** unit test (hinged prop pushed off-axis keeps its pivot within 0.5 and does not fall; ropes reported; unknown attachment reported), workspace tests, owned tests (all pass; frag replay bests unchanged 27.6/16.4/3.8/12.8/44.1), strict Clippy, fmt. Packaged (`bevy-playground-constraints.json`, artifacts/s15/play*.json/png): the 8 tic-tac-toe blocks stay at their hinge heights z 63/46/29 (play1.png); teeter-totter seat stays at (-4848,-3488,19) (authored -4851,-3492,20); swing seats fall to z -10 as before; carousel rests at z -10 as before; plaza cop tip-off worst physics step 6.5 ms, mean 3.0 (before the CCD work 20-40 ms).

**Result:** the tic-tac-toe board works as in native; nothing else on the playground regressed.

**Still broken or not tested:** native comparison of the spinning blocks (only SDK + visual); swings (ropes), teeter-totter and carousel hinges (assume-world-geometry), constraint systems, breaking, TurnOn/TurnOff, phys_thruster, env_physexplosion (the playground ghost-swing pushes), func_physbox as dynamic bodies; props authored inside static entities keep permanent CCD (cost as before today).

**Next:** regression batch, then the owner's carried-bottle crouch question.
## 2026-10-13 session 15 (continued): studio decals on NPCs (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** DESIGN 13f. (1) `modkit_core::studio_decals`: `project` clips the posed triangles facing the shot to the decal square (depth = radius), carrying barycentric bind positions, blended top-4 bone weights and UVs; `ray_distance` finds the front face along a ray. (2) Simulation: bullet hits on NPCs queue `Impacts::studio_requests` with Impact.Flesh (TranslationData F -> decals/flesh/blood1-5_subrect, drawn with their `$modelmaterial` decals/flesh/bloodN at the subrect's size), and bug bait splats on NPCs queue BeerSplash (drawn with decals/beersplashmodel, 64 units); `$modelmaterial` read from the VMTs, model decal textures preloaded. (3) Host `studio_decals.rs`: builds each request from the NPC's GPU-skinned draws (main-world mesh + joint GlobalTransforms x inverse binds), re-shoots along the shot onto the skin, spawns a decal mesh sharing the draw's `SkinnedMesh` joints (follows animation), tinted by the light state at the hit, hidden with the entity, 50 per model. Report `presentation.studio_decals`, `impacts.studio_created`. Fixtures bevy-studio-decal-cop, -bugbait-cop, -bugbait-citizen.

**Why:** owner item 0b: NPCs must take decals like native studio decals (bug bait stains, blood, paint).

**Tested how:** unit tests (clipped area and bind-space positions, UVs, weights across a bone seam, back faces/out of depth/degenerate rejected, ray front-face hit), workspace and owned tests, strict Clippy, fmt. Packaged (artifacts/s15): three pistol shots at posed terminal_side_cop: 2 blood decals on the belt/abdomen (sdecal3-zoom.png), one grazing hit builds nothing; before the skin re-shoot only 1/3 built (the hit lies on the convex trace shape). Bug bait at the posed cop from 200 units (80 units is inside the bait's non-solid grace period) builds a 1788-triangle stain over the torso; a tint-1 diagnostic showed the geometry and z-fighting (lift raised 0.05 -> 0.25). Native (oracle T160710Z: terminal_side_cop is behind a fence natively, bait hit the fence; T160814Z: bait at the queue citizen's back): no clearly visible stain on the dark clothing natively, nor in ours at the same views (ours hit a different citizen: the queue had progressed differently). Inconclusive; native blood on NPCs not captured.

**Result:** bullets leave blood on NPCs that moves with them; bug bait stains NPCs.

**Still broken or not tested:** native comparison of blood/bait decals on NPCs (needs an open, deterministic native NPC); shot direction is the reversed trace normal; TraceBleed blood on world surfaces behind NPCs; other game materials (antlion/alien/metal NPCs use flesh); per-vertex lighting of translucent model decals; ragdolls; thin seams at creases from the flat lift.

**Next:** VPhysics constraints (owner item 0c; also needed for ragdoll gibs).
## 2026-10-13 session 15 (continued): breakable prop gibs (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** DESIGN 13e steps 2-3. `hl2_simulation::gibs`: a broken prop (CBreakableProp::Break) records its pose and motion in `Scene::prop_breaks`; `Gibs::spawn` (PropBreakableCreateAll / BreakModelCreateSingle / BreakModelCreate_Prop) creates each non-ragdoll .phy break block as a runtime prop body (`Physics::add_prop_body`, new; the frag keeps `add_projectile_body`) at the parent's pose and offset with the parent's velocity at that point, its angular velocity and the default 100 u/s outward burst (`burst` key parsed); gibs get projectile-range entity ids (holdable). `Gibs::tick` follows SUB_StartFadeOut/SUB_FadeOut/SUB_AllowedToFade: after `fadetime`, 3 alpha per 15 ms tick while outside the player's 2D view cone (0.766) and not held, removed at 0; PerformanceMode 1 spawns none. Host: gib meshes preloaded with the projectile models (`PreparedEffects::load` takes the world), `effects::present_gibs` draws them with a per-gib material lit from the map light state and alpha-blended while fading; report `gameplay.gibs`, `gib_draws`. Fixture bevy-pallet-gibs-fade.

**Why:** owner item 0a: breakable props must break into pieces like native. Owner question during the work: should gibs be newly spawned entities? Native: yes (BreakModelCreate_Prop CreateNoSpawn("prop_physics") at the break; models precached by PrecacheGibsForModel). Ours are created at the break too, as runtime bodies; full entities wait for DESIGN 14r.

**Tested how:** unit tests (offset in parent model space, inherited velocity + burst, ragdoll gibs skipped and reported, entity binding; fade postponed while seen or held, 85-tick fade, removal, no-fadetime gib stays), workspace and owned tests, strict Clippy, fmt. Packaged (artifacts/s15): pallet #327 launched into the hall wall breaks into its 7 chunks (gibs1/gibcap.png), lit like the intact pallet at that spot (intact.png); after looking away all 7 fade and are removed by 20 s (two at 2 s, the rest at their 10 s fadetime). Owned check (scratch test, removed): cardboard_box001a/002a break into `ragdoll` gibs (6 solids); pallet chunk .phy hulls match their render bounds in model space.

**Result:** wooden props break into physical pieces that fade like native.

**Still broken or not tested:** native comparison of the pieces (counts/spread/fade timing) not done; cardboard boxes produce no pieces (ragdoll gibs need constraints, owner item c); parent skin, placementOrigin attachments, propdata chunk sets, gib health/DAMAGE_EVENTS_ONLY, gib visibility culling; gibs are runtime bodies, not map entities (DESIGN 14r).

**Next:** NPC studio decals (owner item 0b), then constraints (0c, also unlocks ragdoll gibs).
## 2026-10-13 session 15: stacked-prop margin, prop CCD frame drop (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** (1) Prop-on-prop contacts keep one VPhysics margin: Rapier adds both colliders' contact skins (narrow_phase: `contact.dist - skin1 - skin2`), so two props rested 1.0 apart while a prop rests 0.5 above the floor; dynamic prop colliders now run the solver-contact hook, which adds the shared skin back for pairs where both sides carry one. (2) Prop CCD gating: map props and runtime bodies (frag) no longer keep Rapier's hard CCD on permanently; each tick an awake body enables it only while its linear travel per step exceeds the body's own thickness (smallest extent of its pieces' union in the body frame). (3) Model-shaped hitboxes of hull-box NPCs are excluded in the contact-pair filter (Rapier enables a pair when either collider's active collision types allow it, so clearing the hitboxes' types alone did not stop props touching them: a pallet balanced on a plaza cop was pushed off by the animated hitboxes). (4) Movement-report sample field `step_ms` (whole fixed host step). Fixtures bevy-crate-on-box-stack, bevy-pallet-tips-off-plaza-cop, bevy-pallet-tips-off-npc-hull.

**Why:** owner playtest: props stacked on each other rest visibly higher than native; dropping the pallet off-center onto a plaza cop so it tips over still drops 46 -> 17 fps.

**Tested how:** unit tests (two stacked boxes: floor and stack gaps both 0.5 +- 0.1; hull-box test extended with a box across the hitbox face), workspace tests, owned tests with HL2_ROOT (all pass; frag replay bests lob 27.6, roll 15.6 (was 16.4), high 3.8, plaza 12.8, wall 44.1), strict Clippy, fmt. Packaged d1_trainstation_02 (artifacts/s15): Rapier profiler (temporary build, reverted) showed the cost was the CCD time-of-impact search (17-26 ms per step while the pallet rocked, 2-4.5 ms at idle; collision+solver ~2 ms); plaza cop #427 tip-off (`bevy-pallet-tips-off-plaza-cop.json`): worst physics step 2.5 ms over the run (before 20-40 ms for ~0.7 s while falling, 3-7 ms idle); crate #715 dropped on box #710 rests exactly 0.500 lower than with the old skin sum (crate-box 20.404 -> 19.904; box unchanged). Pallet street launch vs native T021822Z: identical to session 14 through the first bounces (y -4235/-4012/-3849 at f305/f320/f335), then diverges while tumbling: rest -3479 (session 14 -3495, native -3572). Rejected: Rapier soft CCD (speculative 8-unit prediction) for slow props: the launched pallet bounced to z 75 on its first ground hit and rested at -3467.

**Result:** stacked props sit one margin apart; the pallet-on-cop frame drop is gone in the measured case.

**Still broken or not tested:** native stack gap not measured numerically (pairwise 0.5 follows the floor measurement and the owner's visual comparison); tunneling of thin props moving less than their own thickness per tick through zero-thickness geometry is not tested; why ~7-38 props never sleep at idle (tic-tac-toe blocks without constraints and props near z -4) is open; owner's live gravity-gun drop not re-run by the owner.

**Next:** owner item 0 (gibs, NPC decals, constraints, carried-bottle crouch).
## 2026-10-13 session 14 (late): VPhysics contact margin, prop impact damage (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** (1) Dynamic prop colliders get a 0.5-unit Rapier contact skin (`VPHYSICS_CONTACT_MARGIN`): the maps' authored rest poses put .phy hull bottoms 0.497 (PlasticCrate01a) and 0.500 (a lying glass bottle) units above the floor, i.e. IVP's collision margin; without it props sat half a unit into floors (owner screenshot). The player shadow is now the hull at its sides/top with the bottom raised by the margin (replaces the +0.25 oversize). (2) Prop impact damage (DESIGN 13e step 0): CPhysicsProp::VPhysicsCollision -> CalculatePhysicsImpactDamage with allowStaticDamage (both bodies' lost speed, the other weighted by its mass, per kg of the prop), prop m_impactEnergyScale (0/absent -> 0.1, AddOutput override), x10 on the first collision after a gravity-gun launch (`Physics::launched`, set by the launch, cleared by any collision); prop_physics without health stay unbreakable (events only). (3) `prop_pose` gains `launched`. (4) Break sound: CBreakableProp::Break -> PhysBreakSound plays the surfaceprop break cue at the prop (all surface break cues preloaded); packaged: pallet #327 into the wall plays Wood.Break at the break. (5) Gib lists: `phy::break_models` (CBreakParser keys/defaults), `World::model_breaks` and gib physics preloaded at map load (owned test: wood_crate001a lists 7 chunk gibs, all present); spawning/rendering gibs not done. (6) SF_PHYSPROP_PREVENT_PICKUP (0x200) blocks +USE and gravity-gun pickup (SDK FVPHYSICS_NO_PLAYER_PICKUP / CanBePickedUpByPhyscannon; unit test). Found: the d1_trainstation_02 playground tic-tac-toe blocks (playgroundTick-tack-toe_block01a, authored z 46-62) fall to z ~-4 and jitter forever: native holds them with physics constraints (phys_hinge); constraints are not implemented. Fixtures bevy-pallet-launched-into-wall, bevy-crate-drop-rest.

**Why:** owner: red plastic crate and bottles clip into the floor; crates/pallets must break like native (native, owner-verified: they break when launched into a close wall, not when launched far).

**Tested how:** unit tests (launch bonus/energy scale/small-mass cap; a prop launched into a wall queues >= 50), owned tests (all pass, frag replay unchanged), packaged: crate #715 dropped on the hall floor rests at z 71.921 vs the native authored 71.957 (71.421 before); box push rest (-3359.4,-2607.4) vs native (-3358.3,-2607.4), stuck only 2/16 samples while pushing the pinned box; standing on a box and walking into boxes 0 stuck; pallet #327 (physdamagescale 0.1) fired at 1500 u/s into the hall wall with the launch flag breaks on impact, without it survives (3 hits); the native-compared street launch of pallet #882 does not break (its map physdamagescale is 0.0001), rest y -3495 vs native -3572 (-3521 before the margin). Regression batch artifacts/s14b-regression: weapons 17/17, attention 26/26, movement 25/26 (doll01 only); finals vs s14-regression mean 0.00-0.48, Breen feeds identical.

**Result:** props rest on floors like native; launched props break against close walls.

**Still broken or not tested:** native comparison of the wall break (which pallet the owner broke: #882 has physdamagescale 0.0001 and cannot break by impacts per the SDK; #327/#349 have 0.1); a clean far-launch test of a breakable pallet (the hall run hit an obstacle); gibs, break sounds and propdata damage tables/modifiers (13e steps 1-4); bottle shoved into the floor with +USE not re-measured with a bottle; props near z -4 keep jittering (never sleep; possibly water) - not investigated; the margin is not applied to the thrown frag or prop-prop pairs are doubled (0.5 + 0.5, not verified vs native).

**Next:** 13e gibs/break sounds, then the remaining owner items and 13b, then the merge.
## 2026-10-13 session 14 (continued): NPC hull boxes, prop drag fractions, AddOutput keyvalues (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents. Research input: Codex REA pass physics-research-20261013b/drag-axes (read-only).

**Changed:** (1) NPC hull boxes (DESIGN 13d): SOLID_BBOX NPCs of HULL_HUMAN classes (metropolice, combine soldiers, player companions, Kleiner/Eli/Breen/Mossman/Magnusson/G-Man/Monk, zombies, stalkers, assassins) get an axis-aligned (-13,-13,0)-(13,13,72) box collider at the entity origin (follows set_entity, never rotates, disabled with the entity); player movement and physics props collide with it, traces (bullets, use, projectiles) and NPC navigation do not; the model-shaped NPC colliders stay as trace hitboxes with no contacts. (2) Drag: the VPHY header's stored orthographic fractions (retail copies them) are read (`Phy::drag_fractions`, `PhysicsSolid::drag_fractions`) and used for map prop bodies instead of the convex-silhouette estimate; the thrown frag keeps the estimate. (3) `AddOutput "key value"` (SDK keyvalue form) sets a per-entity keyvalue override; impact damage reads `physdamagescale` from it first. (4) Test fixture action `prop_pose` (teleport a map prop_physics body with a velocity; stands in for prop_physics_create until DESIGN 14r) and report fields `physics_ms`. Fixtures: bevy-player-on-box, bevy-player-walk-into-boxes, bevy-player-on-npc-hull, bevy-pallet-on-npc-hull, bevy-pallet-kills-cop-physdamagescale1, bevy-frag-use-pickup.

**Why:** owner: in native you stand on a citizen's box-shaped collider above the shoulders; a pallet resting on a cop dropped the frame rate; the pallet launch drag was ~1.7x native (queued item 0); can we spawn a pallet like native; did the cop test set physdamagescale 1 (it had not: terminal_side_cop has map physdamagescale 0).

**Tested how:** unit tests (player lands on the hull past the model, traces hit only the model, hull follows/disables, a thrown box strikes the hull with an NPC contact event; AddOutput keyvalue form), owned tests (pallet .phy fractions 0.9417/0.4026/0.9936; frag replay unchanged: lob 27.6, roll 16.4, high 3.8, plaza 12.8, wall 44.1), packaged d1_trainstation_02 (artifacts/s14): standing over terminal_side_cop's shoulder holds feet at 137 (hull top) while shuffling (before: slid off); pallet resting on a cop: physics step 4.7 ms mean / 9.4 max (model colliders: 25.5 / 62 ms and the pallet slid off); pallet launch at the owner's street spot vs native T021822Z: y -4235/-3899/-3673/rest -3521 at 0.08/0.45/0.9 s (native -4258/-3904 at 0.47/-3711 at 0.91/rest -3572; before -4246/-3999/-3843/-3769); pallet launched at the cop at 1500 u/s: dies with AddOutput physdamagescale 1, unhurt with the map's 0. Workspace tests, strict Clippy, fmt. Regression batch artifacts/s14-regression: weapons 17/17, attention 26/26, movement 25/26 (only the known doll01 PHY root-rotation warning); finals vs artifacts/s13-regression mean differences 0.00-1.07, Breen feeds identical, Kleiner feed 6 pixels.

**Result:** standing on NPCs works like native boxes; the pallet-on-cop frame drop is gone; the pallet launch now tracks native within ~40 units (was ~200 short).

**Still broken or not tested:** other NPC hull types (scanners, antlions, striders, vortigaunts...) keep model colliders; crouch/small hulls; NPC-vs-NPC navigation still uses model shapes; native comparison of standing on an NPC not captured (SDK only). Drag: the retail IVP-frame axis pairing (Source (vx, vz, vy)) was tried and rejected for now (pallet barely changed, frag roll replay 16 -> 65); stored fractions broke the frag's plaza replay (12.8 -> 40.1), so the frag keeps its estimate: an unmodelled object/core frame difference remains (drag-axes CONTINUATION). AddOutput output-connection form unsupported. No real prop spawn yet (14r).

**Next:** regression review, then the remaining owner items (breakables/gibs, NPC studio decals), 13b leftovers and the merge.
## 2026-10-13 session 14: thrown frag +USE pickup; player stuck on/against props (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** (1) Thrown frags get a projectile entity id (`physics::projectile_entity`) bound to their VPhysics body, so the +USE carry and the gravity gun can hold them (SDK CBaseGrenade: FCAP_IMPULSE_USE | FCAP_USE_IN_RADIUS, Use -> PickupObject; HL2 OnPhysGunPickup leaves the fuse running). FindUseEntity and the gravity gun's FindObject/punt traces use COLLISION_GROUP_NONE (`pick_ray`/`pick_hull` also hit weapon-group bodies); FindUseEntity now follows the SDK order (eye line returns at once, the last usable hull trace wins) and adds the PLAYER_USE_RADIUS sphere search (0.8 cone, nearest to the view line, occlusion trace). (2) Player stuck on props: `Player` ports CGameMovement::CheckStuck (stuck table, 0.2 s single-player interval, one offset per 0.05 s, move skipped while stuck) and exports `step_height` (m_outStepHeight). The VPhysics shadow: the explicit 85 kg weight impulse on movable ground is removed (not SDK/retail: the retail shadow has no gravity and light supports are not rideable; it sank a 5 kg box ~1.5 units into the floor and its rebound filled the player's hull); a step teleports the shadow (SetPosition/StepUp) instead of driving it through the prop; retail contact-plane limits (50 u/s into a pushable object, 0 into a heavy or blocked one), also applied ahead of contact by a sweep; the collider extends 0.25 past the hull's sides/top with its bottom raised 0.25. Report gains `hull_contacts`, `shadow` and `carrying`.

**Why:** owner playtest: a thrown grenade cannot be picked up with E (native can); the player often gets stuck walking into or standing on physics props. Owner corrections this session: test by standing on top of a prop; a box pinned at a wall should not move.

**Tested how:** unit tests (frag pickup until detonation; CheckStuck nudge; shadow pushes light not heavy, held object, no sinking of the prop stood on), owned tests with HL2_ROOT (44 pass), workspace tests (430), strict Clippy, fmt. Packaged d1_trainstation_02 runs (artifacts/s14): teleport onto cardboard box #710 and walk around (before: stuck on landing; after: 0/28 samples stuck, box unmoved); diagonal walk into boxes #710/#711 (before: stuck 230 ticks on #711; after 0/42); native-compared push into the wall (box rests (-3360.0,-2607.9), native (-3358.3,-2607.4), stays at the wall; momentary <0.01-unit contacts while pushing a pinned box); frag thrown at the feet then +USE: carried in front of the view until the fuse (capture fraguse-held.png).

**Result:** frag pickup works with +USE (and the gravity gun path is enabled, not separately tested); the stuck-on-top and walk-into-box cases no longer stick.

**Still broken or not tested:** native comparison of frag pickup (hold distance, timing) not done; gravity gun frag grab not tested; light props are no longer weighed down by the player (retail lead: controller support contacts, normal Z < -0.7); retail two/multi-normal controller constraints, bounded per-axis corrections and VPhysicsShadowUpdate (shadow pushing the player) not ported; props sinking into floors under load or when shoved with a held object (owner report: red crate, bottles) is Rapier contact softness, not fixed; NPC movement colliders are model-shaped where the SDK uses a HULL_HUMAN box (owner report, next).

**Next:** NPC hull boxes (SOLID_BBOX HULL_HUMAN for player movement and physics), then the pallet-on-cop frame drop, prop floor penetration, then the queued owner items.
## 2026-10-13 session 13 (continued): bug bait splat, player shadow, physdamagescale (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** bug bait BeerSplash decal (retail subrect of decals/decals_lit, lightmap-lit translucent decals through `Impacts::add_decal`) and the SporeExplosion emitter; the player's 85 kg VPhysics shadow (pushes movable props <= 350 kg, weighs down movable ground, skips the held object; `Player::wish_velocity`); NPC impact damage scaled by the entity's `physdamagescale`. Fixtures `test-inputs/bevy-bugbait-wall.json`, `test-inputs/bevy-player-push.json`. Private oracle gained `!keep` (no setpos), `!forward/!unforward`, `!name` (targetname the prop under the crosshair), `!spawn` (prop_physics_create, ORACLE_SPAWN) and `!second` (ent_text on ORACLE_TEXT2).

**Why:** owner playtest reports (bug bait leaves no stain or particles; walking into props does not push them; a launched pallet barely hurt a cop).

**Tested how:** unit tests (spore schedule/direction/decal request, shadow push light vs heavy, held object, weight; physdamagescale), owned tests (decal textures incl. the translucent subrect), packaged captures vs native oracle: bug bait at the spawn-room wall (T012846Z), cardboard box push (T012535Z), crate/oil drum/pallet vs plaza cop (T015556Z, T015900Z, T020048Z). Workspace tests, strict Clippy, fmt.

**Result:** stain and push match native closely; the plaza cop's immunity is the map's physdamagescale 0, now honoured.

**Still broken or not tested:** spores dimmer/sparser than native (11f); our pallet launch flight vs native not compared (needs a spawn command, 14r); NPC studio decals; breakables; push speed limit and the shadow pushing the player.

**Next:** breakable props (propdata), NPC decals, a debug prop spawn for open-space launch tests, then the rest of 13b and the merge.

## 2026-10-13 session 13: frag as a real w_grenade.phy rigid body (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:** the thrown frag is a Rapier rigid body from w_grenade.phy by default (`--frag-swept-box` keeps the old fitted box): SDK spawn orientation (vec3_angle for throw/lob, (0, yaw, -90) for roll; ProjectileSpawn gained `angles`); the 12-sided .phy prism collides as a round cylinder; mass center and rotation inertia from the .phy IVP compact surface (source-assets phy.rs -> PhysicsSolid::compact_inertia); VPhysics performance-default speed limits (2000 u/s, 3600 deg/s) on every dynamic body. Owned test `owned_frag_throws_follow_native_samples` replays five native throws; unit test for the rolling prism; owned test for the frag .phy inertia; fixture `test-inputs/bevy-frag-wall.json`.

**Why:** owner report: the frag floated above surfaces (4-unit swept box vs the 1.8-unit hull), rolled artificially, flopped at walls instead of bouncing; the session-12 rigid body stopped dead because it spawned aligned with the throw (SDK spawns it upright) and the faceted hull lost energy on every edge.

**Tested how:** headless replay on d1_trainstation_02 collision vs native ent_text samples (best of seven spins, RMS units): lob 27.6, roll 16.4, overhand 3.8, plaza 12.8, wall 44.1 (old swept box 92.1/22.3/13.1/44.7/105.8). Three new native wall throws (oracle T000533Z, T002157Z, T002243Z). Packaged captures (artifacts/frag-s13): ground-level views of landed frags on the hall floor and on top of cardboard box #710 (rests at hull radius, no gap). Workspace tests, strict Clippy, fmt.

**Result:** frag lies on its side on surfaces and rolls with real rotation; wall impacts pop it up and deflect it.

**Still broken or not tested:** wall rebound for some spins is larger than the three native throws (in game, spin -0.5: lands ~90 units out vs native 18-60); IVP impact friction is not reproduced (retail friction product kept); packaged audio of the rigid frag vs native not rerun; two Bevy image `should_panic` tests fail in release builds only (debug assertions; pre-existing, unrelated).

**Next:** bug bait splat (BeerSplash decal + SporeExplosion), player physics shadow (push props), pallet launch flop, breakables, rest of 13b.

## 2026-10-12 session 12: local validation of the cloud arsenal + physics branches (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents. Validates work written by the cloud sessions (Claude Code cloud, `claude-opus-5-5`, SDK-only: crossbow, RPG, bug bait, gravity gun, viewmodel effects, sound stop/patch path, view punch, authored VPhysics props, surfaceprops, impact sounds, frag rigid body, +USE carry, ground surfaces, per-side world materials).

**Changed (this session, local):** VFS searches VPKs before loose folders (gameinfo order); Weapon_Equip clip rule; retail facts in owned tests (no RPG reload sound, no crossbow unloaded activities); projectile models posed by sequence 0; viewmodel-attached sprites at world-FOV size; viewmodel FOV follows the zoom; RPG laser dot in glow mode (no depth test); crossbow skin-1 glowing bolt and CrossbowLoad sparks; per-object retail VPhysics drag (drag.rs) with native air density 2.00; held-object mass reduction and drag toggle; frag rigid body opt-in (`--frag-rigid-body`); fixtures bevy-physcannon-props.json and bevy-use-carry.json.

**Why:** owner: validate the cloud branches locally and fix what the installation, private research and native HL2 show; owner reports during the session (zoom past the viewmodel, laser dot clipping, plain vs incandescent bolt).

**Tested how:** Windows build, workspace unit tests, strict Clippy, fmt; owned tests with HL2_ROOT; packaged fixtures (frag x4, crossbow, RPG, bug bait, gravity gun at a prop spot, +USE carry; captures in artifacts/cloud-validate); native oracle (gravity gun hold/launch and air_density T191304Z, crossbow T192857Z); frag positions vs session-11 native samples; regression batch artifacts/cloud-regression(2).

**Result:** owned tests pass; gravity gun glow matches native; rigid-body frag worse than the swept box on lob/trainstation landings (kept opt-in); batch weapons 17/17, attention 26/26, movement 25/26 (new doll01 collision warning, explained in STATUS).

**Later in the session:** crossbow skin-1 glowing bolt + CrossbowLoad sparks, viewmodel FOV follows the zoom, RPG laser dot in glow mode, gravity gun DoEffectLaunch, physics impact damage to NPCs (SDK/retail NPC table; owner reports and requests). Native RPG missile path matches within ~3 units (T194927Z).

**Still broken or not tested:** native trajectories of bolt/bait/launched props (two rejected attempts each for bolt and launch); weapon audio vs native; DoEffectLaunch beam, bolt motion-blur trail, ragdoll pinning; 11f gamma sprites; doll01 root-rotated .phy; the AI branch is not validated yet.

**Next:** remaining 13b native checks, then fold into wip/arsenal-20261009 and the single 13b merge; then validate `wip/cloud-npc-ai-20261010` (step 14).

## 2026-10-12 cloud session: ground surface movement and per-side world materials (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code cloud session (Linux sandbox), `claude-opus-5-5` per the session environment. No subagents.

**Changed:** player ground friction/acceleration/max speed/jump from the ground surfaceprop (CategorizeGroundSurface); surfaceprops parses jumpfactor/maxspeedfactor/thickness/dampening; restitution clamped to 0..1; BSP brush-side materials -> $surfaceprop -> per-contact friction/restitution for world brush colliders through a Rapier contact-modification hook.

**Why:** owner: physics should use Source's materials (friction, bounciness) everywhere; the owner supplied the installed surfaceproperties scripts.

**Tested how:** unit tests with synthetic data (surface factors on a floor, icy side vs default slide distance, parser quirks); temporary uncommitted parser check against the owner's uploaded scripts (91 entries); strict Clippy, fmt, workspace tests.

**Result:** passes on Linux. Not run in the game (cloud: no owned files/native).

**Still broken or not tested:** native stopping distances and prop slides; brush entities/displacements keep "default"; slipperyslime jump question (STATUS).

**Next:** STATUS physics checklist items 7-9.

## 2026-10-10 second cloud session: VPhysics props, frag rigid body, +USE carry (`wip/cloud-physics-20261010`)

**Agent/model:** Claude Code cloud session (Linux sandbox), `claude-opus-5-5` per the session environment. No subagents.

**Changed:** new branch from the arsenal branch (owner request mid-session to separate the physics work). .phy solid parameters and surfaceproperties (source-assets) into `World`; Rapier prop bodies with authored pieces, mass, inertia, damping and surfaceprop friction/elasticity; air drag and collision events in `Physics::tick`; PhysCollisionSound/AddImpactSound/PlayImpactSounds for props and frags; the thrown frag as a Rapier body from w_grenade.phy with VPhysicsUpdate's NPC reflection; `grab.rs` shared CGrabController; `player_pickup.rs` +USE carry; host wiring (frag .phy load, collision sounds, E/use routing, holstered viewmodel); merge of the arsenal branch's 13b 3-5.

**Why:** owner, 2026-10-10: physics must follow Source/Havok (weights, bounciness, materials), the frag lacked a real physics object, and E should pick up small props.

**Tested how:** unit tests on synthetic data (solid parsing defaults, surfaceprop inheritance and hard/soft choice, authored mass/inertia/damping/material on bodies, drag per tick and contact events, frag body landing/sound/release, carry pickup/hold/throw/deny/drop, alignment); hl2-bevy compiled/tested on Linux; strict Clippy (exit code, color off), fmt.

**Result:** hl2-simulation 202 / 18 ignored, source-assets 92 / 19, modkit-core 53, hl2-ui 32 / 1, hl2-bevy 37 / 2.

**Still broken or not tested:** nothing compared with native or run packaged; the frag body is not fitted (the swept box was); player does not push props; see STATUS physics handoff.

**Next:** local validation per the physics checklist, decide frag body vs swept box, then merge after the arsenal branch.

## 2026-10-10 second cloud session: 13b items 3-5 (`wip/cloud-arsenal-20261010`)

**Agent/model:** Claude Code cloud session (Linux sandbox), `claude-opus-5-5` per the session environment. No subagents.

**Changed:** sound stop path and CSoundEnvelopeController patches for game cues (Missile.Ignite stop, gravity gun HoldSound); RPG lowered idle without rockets; gravity gun launch spin and carry max speed (new modkit-core `Input::max_speed`); ViewPunch/DecayPunchAngle for crossbow and gravity gun with the host camera; host drawing for stuck bolts, FX_ElectricSpark sparks and the SDK viewmodel sprites/beams (crossbow, RPG, gravity gun) on the viewmodel layer; viewmodel pose parameters ("active").

**Why:** owner priority 0, DESIGN 13b items (3)-(5) of the cloud prompt.

**Tested how:** unit tests (patch envelopes, missile stop request, motor patch sequence, lowered RPG, launch spin, carry speed, max-speed override, punch spring, sparks counts/expiry, viewmodel effect states and quads); hl2-bevy compiled and tested on Linux; strict Clippy (exit code, color off) and fmt.

**Result:** hl2-simulation 196 passed / 18 ignored, hl2-bevy 37/2, modkit-core 53, source-assets 90/18, hl2-ui 32/1. Commit cc82bb8 alone failed strict Clippy (doc lint), fixed in 615d630.

**Still broken or not tested:** nothing was run or compared with native (visual, audio, timing); viewmodel skin swap, CrossbowLoad effect and the launch beam are not done; punch does not affect aim/viewmodel; Windows build.

**Next:** local validation per the STATUS checklist items 7-12; merge order with `wip/cloud-physics-20261010` (stacked on this branch).

## 2026-10-10 cloud session: bug bait and gravity gun (13b d/e, `wip/cloud-arsenal-20261010`)

**Agent/model:** Claude Code cloud session (Linux sandbox), `claude-opus-5-5` per the session environment. No subagents.

**Changed:** weapon_bugbait / npc_grenade_bugbait / point_bugbait (`weapon_bugbait.rs`, BugBaitEvent hook for step 14) and weapon_physcannon (`weapon_physcannon.rs`: punt, pull, pickup, hold controller, drop, launch, deny) on the Rapier props; host hooks; fixtures test-inputs/bevy-bugbait.json, bevy-physcannon.json. Clippy fixes for the RPG commit.

**Why:** owner priority 0, DESIGN 13b (d) and (e).

**Tested how:** unit tests (bug bait haul/throw/squeeze and splat + sensors; launch spline, punt impulse, pickup/hold/launch, deny, pull on synthetic Rapier bodies); hl2-bevy compiled/tested on Linux; strict Clippy/fmt (verified with color off).

**Result:** builds; 190 hl2-simulation tests and 37 hl2-bevy tests pass on Linux.

**Still broken or not tested:** prop masses are not the authored ones; hold controller approximates VPhysics; everything vs native; Windows build. An earlier claim in this session that Clippy was clean (crossbow/RPG commit) was wrong; fixed in ded5dcc.

**Next:** local validation per the STATUS checklist; then step 14 (separate branch).

## 2026-10-10 cloud session: crossbow and RPG (13b b/c, `wip/cloud-arsenal-20261010`)

**Agent/model:** Claude Code cloud session (Linux sandbox), `claude-opus-5-5` per the session environment. No subagents.

**Changed:** weapon_crossbow + crossbow_bolt (`hl2-simulation/src/weapon_crossbow.rs`) and weapon_rpg + rpg_missile + env_laserdot (`weapon_rpg.rs`) from SDK 2013 weapon_crossbow.cpp / weapon_rpg.cpp; player FOV ramp (SetFOV/GetFOV); thin hooks in gameplay.rs/projectiles.rs; host wiring in hl2-bevy (models, cue preloads, FOV, impacts, suit updates, laser dot, missile events); fixtures test-inputs/bevy-crossbow.json, bevy-rpg.json. Details and SDK-vs-prompt differences in STATUS "Cloud session handoff".

**Why:** owner priority 0, DESIGN 13b (b) and (c), prepared in the cloud for local validation.

**Tested how:** unit tests only (state machines in 15 ms ticks, FOV spline, bolt flight/stick/reflect/NPC hit, missile ignition/homing/grace/blast, laser-dot line following); hl2-bevy compiled and its tests run on Linux; strict Clippy/fmt. Owned tests written but ignored (no game files).

**Result:** builds and unit tests pass on Linux.

**Still broken or not tested:** everything visual/audio/timing vs native; owned scripts/viewmodels; Windows build; stuck bolts/sparks/charger/beam not drawn; see STATUS.

**Next:** bug bait, gravity gun, then local validation per the checklist.

## 2026-10-10 session 11 (late): frag impact sounds, native fuse check

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed (branch `wip/arsenal-20261009`, not merged):** frag impacts play the grenade surfaceprop's impact sound per SDK PhysCollisionSound/PlayImpactSounds: normal speed >= 70 u/s, >= 0.05 s since the previous collision, volume min(1, (speed/320)^2), `Grenade.ImpactHard` (installed surfaceproperties.txt: grenade impacthard/impactsoft, hardness 1.0 and no hard thresholds, so hard is always chosen), positioned at the grenade; the cue is preloaded.

**Why:** native physics objects sound on impact; ours were silent.

**Tested how:** unit test (floor drop: one sound, SDK volume, friction impulse, no bounce); 385 workspace tests, strict Clippy/fmt; packaged lob `--audio-trace`: request 0.24 s after spawn at the sidewalk landing, started physics/metal/metal_grenade_impact_hard2.wav. Native fuse (timescale 0.1, T043539Z): detonation 2.94-3.15 s after spawn vs ours 3.045 s. Rejected: oracle timescale 0.05 (user command buffer overflow drops +attack, two runs).

Hits on characters (npc_*) now use the SDK's VPhysicsUpdate ray reflection (0.2 restitution, spin x -0.5, no physics sound), since COLLISION_GROUP_WEAPON passes through characters in VPhysics; unit test added (386 tests).

Per-process recordings: native blip onsets match ours (0, 1.057, 2.102, 2.415, 2.740 s); blip peaks match within ~1 dB; the impact is masked by the plaza ambient (two attempts; tentatively ours ~3 dB quieter).

**Still broken or not tested:** impact sound level vs native (repeat at a quiet place); the 0.1 DMG_CRUSH "bonk" to NPCs (step 14); the player as a reflecting character; rotational dynamics; NPC reflection not compared with native.

**Next:** see the 13b list (crossbow, RPG, bug bait, gravity gun; optional frag rigid body; DESIGN 11f).

## 2026-10-10 session 11 (later): frag world contact, lob and roll vs native

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed (branch `wip/arsenal-20261009`, not merged):** frag world contacts follow VPhysics instead of the entity-only 0.2 reflection (SDK CGrenadeFrag::VPhysicsUpdate reflects only off entities its collision group skips): surfaceprops grenade (friction 0.9, elasticity 0.01) x default/concrete/tile (0.8, 0.2) give practically no bounce and a Coulomb friction impulse 0.72 x |normal speed| on impact; resting contacts keep their tangential motion (gravity carries the body off edges instead of snagging), floors add a constant 190 u/s^2 rolling resistance fitted to native. Fixtures `test-inputs/bevy-frag-lob.json` (secondary, standing) and `bevy-frag-roll.json` (secondary while crouched); both look 25 degrees down at release like the native captures.

**Why:** owner priority 13b (lob/roll comparison; owner reminder: secondary fire standing and crouched).

**Tested how:** native HDR lob and roll (oracle '!attack2'/'!release2', roll with '!duck'; sessions view-d1_trainstation_02-20261010T040222Z lob, T040313Z roll; 14 ent_text samples each at timescale 0.1). Native spawn ~0.09 s after the release command (fit from the first sample; ours spawns at the owned event, 0.105 s). Packaged comparison by time since spawn: roll RMS 22.4 units over 2.2 s (end 21.2; curb drop impulse ~530 -> 335 u/s as native); lob flight/landing 4.7 and 0.5 units; overhand monument throw in flight 10.9 and 8.3 units. 384 workspace tests, strict Clippy/fmt.

**Still broken or not tested:** no rotational dynamics: native turns spin into rolling on contact and rolls along the grenade's own axis, so after landing the lob (ends 151 units off, native rolls straight to the curb at ~155 u/s) and the overhand throw (ends 70 units off; native rolls ~45 u/s off the monument ledge) differ. Swept 4-unit box instead of the w_grenade convex hull. Sprite gamma blending (DESIGN 11f). Regression batch (artifacts/arsenal-regression): 26/17/26, final captures unchanged vs damage-regression except known slate exposure and book-edge noise.

**Next:** rigid-body frag (Rapier body from w_grenade.phy with the fitted drag and surface friction) if the owner wants post-landing fidelity; otherwise crossbow, RPG, bug bait, gravity gun.

## 2026-10-10 session 11: frag flight drag, spin range, sprite-trail strip (arsenal WIP)

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed (branch `wip/arsenal-20261009`, not merged):** npc_grenade_frag flight gets quadratic air drag dv/dt = -c|v|v, c = 6.4e-4 per unit, fitted to native (VPhysics drag lives in vphysics.dll, not the SDK; w_grenade.phy sets damping 0). The frag spin used a [-3, 1] random range (random() is already [-1, 1] and was remapped again); now RandomInt(-1200, 1200) as the SDK. The frag trail is one C_SpriteTrail/CBeamSegDraw strip (shared edge vertices, normal from neighbouring points, alpha and width 8->1 by each point's remaining life, u across the width, v = 0 because grenade_frag never sets a texture resolution) instead of separate per-segment quads with full alpha and a full texture per segment (owner report: wonky, less smooth than native). Fixtures `test-inputs/bevy-frag-native.json` (plaza, yaw 12, Bevy pitch 10) and `bevy-frag-high.json` (yaw -20, Bevy pitch 30). The private oracle gained per-view weapon input suffixes (`!attack`, `!release`, `!attack2`, `!release2`, `!duck`, `!unduck`, `!text` = ent_text on ORACLE_TEXT; positions need `developer 1`).

**Why:** owner priority 13b (native frag comparison) and owner report on the trail.

**Tested how:** native HDR captures (oracle sessions view-d1_trainstation_02-20261010T013831Z, T014922Z, T015224Z; ent_text positions at host_timescale 0.25/0.1). Drag fit: 8 in-flight samples, RMS 3.7 units (linear drag 10.3, none 195.6); the independent pitch -10/yaw 12 throw is predicted within 3 units. Packaged run after the change: 7-20 units from native over 1.3 s (before: up to ~370). Rest position after the monument bounce: ours (-2102.6,-1747.6,20.0) vs native (-2098.9,-1747.5,18.1). New unit tests (native drag samples, trail strip); 384 workspace tests, strict Clippy/fmt.

**Result:** frag flight matches native closely; the trail is smooth and fades like native.

**Still broken or not tested:** native draws Sprite-shader materials with `$nosrgb` 1 (SDK sprite_dx9.cpp default: no sRGB read/write, TONEMAP_SCALE_GAMMA), i.e. additive in gamma space into the integer-HDR framebuffer; ours adds in linear space, so the trail core is about half as bright (red excess ~50 vs ~100) and looks thinner. CBeamSegDraw geometry was checked against the SDK's compiled tier2.lib (half width per side, vertex colour = rgb + alpha). Gamma-space sprite blending is a renderer item for all sprites (planned, not started). Explosion timing vs native not measured (not in view); lob/roll not compared; regression batch not run; the remaining four weapons.

**Next:** lob/roll native comparison, then crossbow, RPG, bug bait, gravity gun; gamma-space sprite blending as a separate renderer step (DESIGN 11f).

## 2026-10-09 session 10 (later): arsenal WIP (frag grenade), suit logon

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed (branch `wip/arsenal-20261009`, not merged):** DESIGN 13b plan (all five weapons, one merge). weapon_frag per SDK CWeaponFrag with the owned v_grenade.mdl events (owned test asserts the sequences/events); npc_grenade_frag fuse/blips/explosion on Source think ticks; projectile models render per model (SMG grenade, w_grenade); frag glow + trail at the fuse attachment (additive by entity render mode); clipless weapons give default ammo as reserve (GiveDefaultAmmo); item_suit plays !HEV_AAx (CItemSuit::MyTouch).

**Why:** owner priority: the rest of the arsenal as one feature after player damage.

**Tested how:** unit tests (launch math, fuse schedule), owned test (v_grenade events), workspace tests, strict Clippy/fmt; packaged trainstation_02 throw (pullback, release, throw at the event, AMMO 5 -> 4, blips, explosion at +3.045 s, redraw) and trainstation_06 suit pickup (!HEV_AAx). No native comparison yet.

**Still broken or not tested:** native frag comparison; lob/roll packaged runs; the frag as a real physics body (friction/rolling approximated); grenade punt/pickup (gravity gun); crossbow, RPG, bug bait, gravity gun.

**Next:** finish 13b on the same branch (native frag check, then crossbow, RPG, bug bait, gravity gun), regression batch, single merge.

## 2026-10-09 session 10: player damage path, gordon_invulnerable, fades, damage indicator, HEV voice

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed:**
- One SDK-ordered player damage intake, `hl2_simulation::player_damage` (CHL2_Player::OnTakeDamage skill scale with the DROWN/CRUSH/FALL/POISON/SNIPER exemption, CBasePlayer::OnTakeDamage armor with integer m_ArmorValue, the fractional damage accumulator, DamageEffect, suit diagnosis, CHL2_Player::UpdateClientData "Damage" message). trigger_hurt, the player's grenade blasts and falls all use it; `Inventory::damage_player` is removed.
- env_global and the global table (`hl2_simulation::globals`; SF_GLOBAL_SET only when absent, TurnOn/Off/Remove/Toggle/counters), carried on changelevel. While `gordon_invulnerable` is on (owned d1_trainstation_01-03 and the 04 knockout) the player takes no damage, as the owner reported for native.
- trigger_hurt hurts every toucher that passes its filters (spawnflags), fires OnHurt for NPCs and OnHurtPlayer for the player, counts both for doubling/forgiveness; NPC damage goes through the shared entity damage path; dead players are not hurt.
- Screen fades (`hl2_ui::fades`, CViewEffects): DamageEffect fades, env_fade inputs and the fatal-landing black fade (PlayerFallingDamage), drawn under the HUD on the game clock.
- HUD damage indicator: the HudDamageIndicator panel with the owned HudTakeDamage*/HudPlayerDeath sequences, SDK MsgFunc_Damage selection (angle bins, DAMAGE_HIGH, damage bits), white_additive trapezoids and fullscreen flash. HudPlayerDeath now comes from the Damage message (CHudHealth never started it).
- HEV suit voice: `hl2_simulation::suit` (SetSuitUpdate/CheckSuitUpdate), `source_assets::vox` (sentences.txt and retail engine.dll VOX word rules, 100bb370/100bc9d0/100bb4b0), sentence synthesis from preloaded words in audio.rs; suitvolume 0.25; HEV_DEAD at death; the queue clears at death (Event_Killed). Nothing without the suit; DeathSound picks FallGib from the accumulated DMG_FALL bit.
- Fixtures: `test-inputs/bevy-player-death.json` now drops on d1_trainstation_04 (run with `--map d1_trainstation_04`; trainstation_02 no longer allows damage); new `test-inputs/bevy-player-hurt.json` (suit, trigger_hurt *32).
- DESIGN: 13a-2 plan, owner corrections, 13b as one feature, step 14 NPC AI with an F4 developer NPC spawn overlay (native F5/F6/F9 binds avoided).

**Why:** DESIGN 13a remainder (owner priority: player features first), and owner corrections: no damage in d1_trainstation_01-03 (gordon_invulnerable), HEV sounds only with the suit.

**Tested how:**
- Unit tests: damage order/armor/accumulator/skill exemptions, Damage message, DamageEffect and diagnosis, globals (trainstation sequence, carry, counters), trigger_hurt NPC touchers and filters, fade math (CViewEffects), indicator angle bins and sequences, suit queue spacing/no-repeat/no-suit, VOX tokenizer and parameter scope. Owned: 60+ HEV sentences render within 5 ms of their authored {Len} (HEV_MED0's Len is stale). 380 workspace tests, strict Clippy, fmt.
- Packaged: trainstation_02 drops leave health 100 (globals on); d1_trainstation_04 trigger_hurt *32 kills at four street spots; suit run plays !HEV_DMG5 at 0.15 s, Player.FallGib and HEV_DEAD1 at death (audio trace); no-suit fatal fall plays Player.FallDamage + FallGib only and respawns with 100 health.
- Native HDR comparison (oracle, `ORACLE_SETUP="noclip;cl_drawhud 1"`): trigger_hurt death at (-5608,-4783) after 6 s: native mean (255,64,50) / ours (255,60,47), red saturated on 100% of pixels in both; fatal landing at (-4584,-4163): native uniform (253,0,0) / ours (255,0,0); both engines land on the same player clip at z 640 at (-5608,-4783).
- Batch artifacts/damage-regression: 26/17/26; Breen feeds pixel-identical; the weapons view's HEALTH now reads 100 (trainstation_02 refuses the own-grenade damage that used to leave 16.4); attention book-edge differences match a rerun's physics noise.

**Still broken or not tested:** drowning (needs water level/swimming), knockback/punch angles, time-based damage, explosion ear ringing, NPC health from sk_*_health, parented triggers (canals_01 trains), modulate fade blend (inferred), VOX word pitch vs channel pitch and time compression (retail mixer unread), the friendlies-talking suit volume cut; timing of the indicator/death ramps vs native; native HEV audio levels; the 2-level (253 vs 255) death red.

**Next:** merge, then DESIGN 13b (the rest of the arsenal in one feature).

## 2026-10-08 Video and Window Display Decoupling

**Agent/model:** Antigravity agentic assistant (OrbitalMan pair-programming session).

**Changed:**
- `crates/hl2-bevy/src/video.rs`: Created dedicated module for window display mode, resolution changes, wgpu depth attachment synchronization (`apply_video_config_to_window`), FOV conversion (`vertical_fov`), and associated safety unit tests (`applies_video_config_safely`).
- `crates/hl2-bevy/src/main.rs`: Registered `mod video;`.
- `crates/hl2-bevy/src/movement.rs`: Decoupled video and window management logic from input/movement simulation. Factored display query parameters into `DisplayDevices` tuple alias, satisfying clippy complexity limits.

**Why:** Separate concerns by decoupling display/video management from the player movement and physics simulation module.

**Tested how:**
- `cargo test --workspace --locked`: all unit tests passing (including `video::tests::applies_video_config_safely`).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed with 0 warnings.
- `cargo fmt --all --check`: passed cleanly.

**Result:** Clean modular architectural boundaries between display/window configuration and player simulation.

## 2026-10-08 Main Menu and Native Source Settings Suite

**Agent/model:** Antigravity agentic assistant (OrbitalMan pair-programming session).

**Changed:**
- `crates/hl2-ui/src/config.rs`: Implemented persistent JSON configuration system (`Config`, `VideoSettings`, `AudioSettings`, `MouseSettings`, `HdrMode`), saved alongside the binary (`bin/hl2-config.json` via `std::env::current_exe()`) without modifying the read-only game installation. Includes serialization and robust default key bindings.
- `crates/hl2-ui/src/options.rs`: Implemented native Source VGUI-styled 4-tab Options dialog (`Keyboard`, `Mouse`, `Audio`, `Video`).
  - Extracted UTF-16/UCS-2 localized strings from installed game resources (`resource/gameui_english.txt` and `resource/valve_english.txt`).
  - Tab 1 (Keyboard): Interactive key rebinding with scrolling list, mapping actions (`+forward`, `+jump`, `+duck`, `+use`, `+reload`, etc.) to pressed keys.
  - Tab 2 (Mouse): Reverse mouse toggle and sensitivity slider (0.5..10.0 scale).
  - Tab 3 (Audio): Master volume slider (0.0..1.0).
  - Tab 4 (Video): Resolution selection cycling (720p, 1080p, 1440p, 4K), Windowed/Borderless toggle, Field of View slider (60..120°), and HDR mode toggle (None / Full) planned for the future LDR mode.
  - Action buttons: OK, Cancel, Apply.
- `crates/hl2-ui/src/console.rs`:
  - Added `Mode::MainMenu` and `Mode::Options` to console UI state machine.
  - Extended pause menu and main menu layouts to 4 items with localized tokens:
    - Pause menu: `RESUME GAME`, `OPTIONS`, `DEVELOPER CONSOLE`, `QUIT`.
    - Main menu: `NEW GAME`, `OPTIONS`, `DEVELOPER CONSOLE`, `QUIT`.
  - Added runtime console commands: `volume`, `sensitivity`, `fov`, `mat_hdr_level`, and `mainmenu`.
  - Emits `Effect::ApplyConfig(Box<Config>)` on Apply/OK or convar changes to synchronize with host.
- `crates/hl2-bevy/src/main.rs`:
  - `Options::parse()` reads saved configuration as default values for width, height, borderless, volume, and camera FOV, overridden by CLI arguments when supplied.
  - Added `--main-menu` CLI argument to launch directly into the native Main Menu.
- `crates/hl2-bevy/src/movement.rs`:
  - Added `parse_keycode` helper and action binding checkers (`action_pressed`, `action_just_pressed`).
  - Applied configured mouse sensitivity and invert pitch to view simulation angles.
  - Wired gameplay action keys and movement axes to read from user bindings in `Config`.
  - Updated `console_effects` to handle `Effect::ApplyConfig`, updating camera FOV, audio volume, and window mode/resolution at runtime.
  - Fixed wgpu attachment size validation crash (`differing sizes: depth (1280, 720, 1) vs color (3456, 2168, 1)`) by refactoring video configuration updates into `apply_video_config_to_window`: prevents stomping `window.resolution` with windowed dimensions when transitioning to or running in `BorderlessFullscreen`.
- `AGENTS.md`: Documented owner guidance rules, render scaling architecture, and the rule that original in-game implementation behavior strictly overrides current/temporary implementations.
- Research notes: documented Source engine `mat_setvideomode`, display modes, and resolution scaling.

**Why:** Implement the native Half-Life 2 Main Menu, Options dialog, and settings persistence suite as requested, and fix the wgpu validation crash when switching to fullscreen and applying options.

**Tested how:**
- `cargo test --workspace --locked`: all 166 non-ignored unit tests passed across `hl2-ui`, `hl2-bevy`, `hl2-simulation`, `source-assets`, and `modkit-core`.
- Added unit tests:
  - `hl2_ui::config::tests::config_defaults_and_serialization`: checks default config generation, JSON roundtrip, and path calculation.
  - `hl2_ui::console::tests::main_menu_and_options_configuration_interaction`: verifies menu pointer interactions, Options dialog navigation, drafting, Apply/OK effect emission, and cancelation.
  - `hl2_bevy::movement::tests::config_key_bindings_and_mouse_sensitivity_are_respected`: verifies custom key bindings (`KeyI`, `KeyJ`), mouse sensitivity scaling (6.0), and inverted pitch.
  - `hl2_bevy::movement::tests::config_effect_updates_simulation_fov_and_console_config`: verifies `Effect::ApplyConfig` updates simulation FOV and console config.
  - `hl2_bevy::movement::tests::video_config_does_not_override_resolution_in_borderless_fullscreen`: verifies that applying video settings when transitioning to or running in borderless fullscreen preserves native display resolution and does not stomp `window.resolution`, keeping wgpu depth and swapchain color attachments synchronized.
- Live execution and input script verification:
  - Ran `./launch-bevy.sh --game "../Half-Life 2" --frames 35 --no-focus --movement-script "test-inputs/bevy-apply-fullscreen.json" --capture "artifacts/test-apply-fullscreen.png" --report "artifacts/test-apply-fullscreen.json"`.
  - Executed runtime transition into Options -> Video tab -> toggle Borderless Fullscreen -> click OK/Apply and close dialog: successfully transitioned, captured 3456x2168 full frame, and exited cleanly with exit code 0.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed with 0 warnings.
- `cargo fmt --all --check`: passed with 0 formatting differences.

**Result:** Main Menu and Options dialog fully operational and integrated with Bevy host. Settings persist cleanly to `hl2-config.json` next to the executable.

## 2026-10-08 session 9 (end): player fall damage, death and respawn; detail sprite WIP

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed:**
- Owner priority change: player features first, then the rest of the arsenal (DESIGN 13).
- Player (DESIGN 13a, SDK CSingleplayRules::FlPlayerFallDamage, CBasePlayer::Event_Killed/DeathSound/PlayerDeathThink, respawn): HL2 fall damage from 526.5 to 922.5 u/s (armor ignored) with Player.FallDamage; death holsters the weapon, sets the dead view height (14), freezes input, plays Player.Death or Player.FallGib; after every button is released, one press restarts the map (singleplayer reload without saves). `Inventory::damage_player`, shared `Player::dead`/`landed`. Death cues are preloaded. Fixture `test-inputs/bevy-player-death.json`.
- trigger_hurt (SDK CTriggerHurt, player only): immediate hit on touch, damage x 0.5 every 0.5 s, damagemodel 1 doubling to damagecap with 3 s forgiveness, leave damage when the last think missed, negative damage heals, OnHurtPlayer; DMG_FALL/DROWN/POISON/RADIATION bypass armor. Unit-tested; batch artifacts/hurt-regression 26/17/26. Maps: trainstation_01 turret_hurt_1 (start disabled), trainstation_04 one, canals_01 two; not exercised in a packaged run.
- Detail sprites (DESIGN 11c, branch `wip/detail-sprites-20261008`, not merged): `source_assets::details` decoder and a per-view billboard renderer (vertex shader facing and squared-distance fade).

**Why:** Owner request to finish player features (death etc.) before more renderer work; detail sprites were the queued item 2.

**Tested how:**
- Unit tests: fall-damage curve and armor bypass, landing speed and frozen dead input, detail decoder (synthetic and owned: 7 sprites, 3,622 records, 66 leaves), fade/corner/lighting math. Workspace tests, strict Clippy/fmt.
- Packaged death run: fatal drop -> pl_fallpain1 + body_medium_break2 (FallGib), eye 14, weapon holstered, respawn at the map start with 100 health after a press.
- Packaged park-grass capture: 3,622 sprites in 66 leaf meshes render bottom-anchored and facing the camera (not compared with native yet).
- Batch artifacts/death-regression: movement 26, weapons 17, attention 26; Breen feed pixel-identical to the footsteps baseline.

**Still broken or not tested:** native death-view comparison (oracle `hurtme` had no effect twice; rejected); trigger_hurt, skill scaling, damage fades/indicator, pain/HEV sentences, drowning; the death view's exact native interpolation/tint; detail sprites vs native, flicker bursts and per-leaf sort order.

**Next:** DESIGN 13a remainder, then 13b arsenal (frag, crossbow, RPG, gravity gun, bug bait).

## 2026-10-08 session 9 (later): crowbar surface sounds, footsteps, script cue levels

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed:**
- Crowbar (SDK basebludgeonweapon/weapon_crowbar/fx_hl2_impacts): a player melee hit plays the hit surface's `bulletimpact` sound like bullets; the player crowbar no longer plays melee_hit/melee_hit_world (NPC-operator cues). NPC hits from any weapon play the flesh impact sound. Fixture `test-inputs/bevy-crowbar-surfaces.json`.
- Footsteps (`hl2_simulation::footsteps`, SDK UpdateStepSound/PlayStepSound, HL2 jump/landing constants): step timer 400/300 ms (+100 crouched), walk/run thresholds 90/220 (60/80 crouched), material volumes (0.2/0.5, dirt 0.25/0.55, vent 0.4/0.7, x0.65 crouched), alternating stepleft/stepright, jump step 1.0, landing 0.85/1.0 above 303 u/s. Ground surface from `Impacts::ground_property` (shared receiver lookup). The shared Player reports `jumped`/`landed`. Fixture `test-inputs/bevy-footsteps.json`.
- Game cues use the sound script's volume and pitch drawn per emission (separate random stream) instead of a fixed 0.4/100; impact sounds are positioned at the hit with the script soundlevel. `SoundRequest` gained `volume` and `origin`.

**Why:** Owner reports: no player footsteps; the crowbar sounded the same on every hit. Script levels are the DESIGN 12b follow-up.

**Tested how:**
- 365 normal tests, owned tests, strict Clippy/fmt.
- Packaged crowbar run: Concrete/Tile/Concrete BulletImpact by floor with rotating variants; open-air swing plays only the swing.
- Packaged footstep run: tile walk 400 ms at 0.2, concrete sprint 300 ms at 0.5, crouch 500 ms at 0.13, jump 1.0; normal jump landing silent.
- Batches artifacts/melee-regression, footsteps-regression, cues-regression: 26/17/26. The first cue build changed Breen's monitor lip sync (0.16% of feed pixels) because parameter draws shared the wave-selection generator; with a separate stream two reruns are pixel-identical to the previous batch.

**Result:** Crowbar hits sound by surface, the player has footsteps, and cue levels follow the scripts.

**Still broken or not tested:** native comparison of footstep and weapon levels is inconclusive (low correlation, likely native DSP/pitch; private footsteps-melee-20261008, cue-levels-20261008); how native spatializes the local player's own sounds; NPC speech stays unpositioned; ladders/water steps; model-level surfaceprops for props.

**Next:** detail sprites (DESIGN 11c, branch wip/detail-sprites-20261008).

## 2026-10-08 session 9: barrier close-hum loop, env_soundscape backgrounds

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed:**
- ambient_generic PlaySound/StopSound/ToggleSound follow SDK CAmbientGeneric m_fActive and restart/stop that entity's own sound with its parameters (`AmbientControl`); `--audio-trace` request lines name the control. Fixture `test-inputs/bevy-barrier-hum.json`.
- New `hl2_simulation::soundscapes`: soundscape manifest/map scripts, SDK server selection for env_soundscape/proxy (Scene::update_soundscape after player movement; Enable/Disable/ToggleEnabled; OnPlay), SDK client playback commands. The Bevy host preloads reachable waves, plays ambient and positional loops and random one-shots, fades by scene time (pause freezes it), honours `--mute-ambient`, traces commands and reports `audio.soundscape`. Fixture `test-inputs/bevy-soundscapes.json`.
- docs/research.md: LuckerParty specs as unverified pointers; parallel REA workflow. DESIGN 12d/12e plans.

**Why:** Owner reports: the barrier's close hum played once instead of looping; the background should change between the spawn room, the station hall and outside.

**Tested how:**
- `cargo test --locked --workspace` 360 passed; owned soundscape test (trainstation scripts and waves) passed; strict all-target Clippy and fmt.
- Packaged barrier walk-in/out with `--audio-trace` and per-process recording: loop continuous while inside (7.8 s and 6.7 s), stops on leaving, twice.
- Packaged soundscape tour (fly and walking): Interrogation -> Turnstyle -> TerminalSquare -> Turnstyle, with crossfades and random city sounds.
- Native per-process recordings at the same places (oracle, HDR, `snd_mute_losefocus 0`): background medians within 3 dB at spawn and hall; outside ours is 6.6 dB louder.
- Regression batches artifacts/ambient-inputs-regression and artifacts/soundscapes-regression: movement 26, weapons 17, attention 26; images match the previous batch (Breen slate exposure varies run to run, 0.64-0.68).

**Result:** The close hum loops while touching the barrier. Each area has its own background, switching with the original crossfade.

**Still broken or not tested:** outside background level (+6.6 dB vs native, cause unverified); DSP room effects, stereo panning, the close hum's spin-up/fade presets, trigger_soundscape, soundscape carry-over across level transitions; owner listening check. Rejected: console soundscape debug through the oracle hung native twice.

**Next:** crowbar surface sounds (branch wip/melee-footsteps-20261008) and footsteps (DESIGN 12e).

## 2026-10-08 session 8 (later): positional ambients, security fixtures, monitor brightness investigation, PR #2 review

**Agent/model:** Claude Code desktop, `claude-opus-5-5`. No subagents.

**Changed:**
- `ambient_generic` playback per SDK CAmbientGeneric. Distance gain from the retail engine's `GetDistGainFromSoundLevel` (engine.dll 10216bd0 -> 10216a20, disassembled privately; stock convars snd_refdb 60, refdist 36, gain_min 0.01, foliage loss 4).
- Sound script volume/pitch/soundlevel parsing in `Library::params`.
- Report `audio.spatial`.
- Seeded security fixtures (`bevy-attention*`, `bevy-monitors-kleiner-scene`) now enable `overlay_kleinertv` (security_01 OnTrigger2) and kill `scene1_start`, `scene2_start` and `intro_music`. The G-man intro had started six seconds in.
- Resampler test WAV header uses escapes, not raw control bytes.
- Monitor feeds: kept-exposure and fixed-2 experiments were reverted; scale 1 remains.

**Why:** Owner reports: loud hum everywhere, shield hum should fade with distance, the blue HUD was missing in the Barney batch cases, G-man speech played during those cases, and the Kleiner screen looked too dim.

**Tested how:**
- 349 normal tests, strict Clippy/fmt.
- Packaged reports at five plaza distances (gate hum 1.0/0.85/0.30/0.14/0.08 at 14/132/332/632/988 units).
- Audio trace of the attention fixture: the G-man cues are gone.
- Attention verifier 26/26 with the overlay visible.
- Native HDR security scene at 10.5 s from the Kleiner camera, plus native lab views after the intro.
- Regression batch artifacts/session8-audio.

**Result:** The shield hum is positional. The HUD overlay renders in the Barney cases, and those runs no longer play the G-man intro.

**Still broken or not tested:**
- The Kleiner screen and plaza slate are ~2x dim (linear) vs native. A feed-scale fix regressed hall exposure (wall 109 vs 129) and was reverted.
- Bumped lightmaps (rocky brick).
- env_soundscape, panning/DSP, positional PlaySound inputs, script cue volume for game requests.
- Native audio recording of the hum.
- Rejected this session: native lab captures blocked by the G-man intro camera (wait out the intro with ORACLE_INTRO_SECONDS=55 plus intro deactivation), and the `kept`/`fixed 2` monitor scales.

**Next:** env_soundscape (DESIGN 12d), then detail sprites (11c).

## 2026-10-08 session 8: weapon-selection tick, godray regression, pause freeze, shield mipmaps

**Agent/model:** Claude Code desktop, `claude-opus-5-5` (session system context). No subagents.

**Changed:**
- Audio (fixes the owner's weapon-switch report): every preloaded wave is resampled once to 48 kHz 16-bit (Catmull-Rom) in `hl2-bevy/src/audio.rs`. Stereo one-shots get one 512-sample silent span first. rodio 0.22 starts each player with a mono 48 kHz, 512-sample filler span, so a sound in another format had its first 512 samples played as 48 kHz mono. The 22 kHz `wpn_moveselect.wav` tick lost its attack: about 12 ms missing, then 2.2x too fast. New diagnostics: `--audio-trace JSONL` (per-sound request/start/removal with last sink position) and `--mute-ambient` (skip map ambient loops for isolated recordings). Shared `hl2_simulation::sounds::wav_samples`.
- Godrays: UnlitTwoTexture tint is linear($color x entity rendercolor) again (SDK `SetModulationPixelShaderDynamicState_LinearColorSpace`). Session 7's barrier path had overwritten it with $color alone, which whited out the trainstation vol_light shafts (rendercolor ~49 45 34).
- Pause: auto exposure and bloom adaptation freeze while paused (owner: native freezes everything). `--capture-live` keeps simulating during a capture burst, to verify motion over time.
- Plaza shields: `source_assets::vtf::decode_frame_mips` and mip-chain upload for UnlitTwoTexture, so far shields show the authored smudged-plasma mips instead of sharp cracks.

**Why:** Owner reports 2026-10-08 (tick cutoff, godrays, pause, far shield look).

**Tested how:**
- Tick: process-tree loopback recordings (private Microsoft ApplicationLoopback build) of the packaged game with ambient muted, plus a one-off local recording of the historical Macroquad host. Burst energies relative to the attack:
  - file: -10.4/-10.4/-7.4/-10.4/-10.4 dB
  - Macroquad: -10.3/-10.2/-7.4/-9.9/-10.3 dB
  - Bevy before: -7.4/-7.3/-4.4/-7.3/-7.4 dB
  - Bevy after: -10.5/-10.4/-7.5/-10.4/-10.5 dB
  Attack windows after the fix: correlation 0.993-0.998, gain 0.99-1.01. The owner confirmed by ear.
- Barrier scroll: matched native HDR frame series by normalised cross-correlation (0.73-0.83 vs 0.43 median), and the times advance like native's (~2 s steps, 14.1 s period).
- Pause: Escape pause menu, 150 live frames pixel-identical.
- Godrays: native HDR hall views vs before/after.
- Shields: native HDR at 600-1000 units.
- `cargo test --locked --workspace`: 347 passed; owned ignored tests: 33 passed; strict all-target Clippy and fmt pass.
- Packaged batch artifacts/session8-final: verifiers movement 26, weapons 17, attention 26. The Breen hall view's mean luma is 91 (session 6: 90.9; session 7: 170, the godray regression).

**Result:** The selection tick plays like the original file. Godrays match native again. Pause freezes exposure. Shield scrolling was already native-equivalent; its far look now matches.

**Still broken or not tested:**
- Map ambients are still non-positional and play everywhere: no Source distance attenuation, no `env_soundscape`. This is the owner's loud-hum report and the barrier hum not fading.
- Native HL2 audio was not recorded for the tick; Macroquad and the file are the references.
- Stereo one-shots start 5.3 ms late.
- Combine wall panels are olive vs native blue-steel (likely envmap/bump). The green light sprite and the plaza screen's blue glow are missing.
- Rejected this session: the ambient-masking hypothesis (Macroquad had the same ambients), a biased 10 ms high-pass envelope comparison, and a `setpause` native capture whose screenshots were taken outside the pause.

**Next:** Source ambient_generic attenuation (retail engine `GetDistGainFromSoundLevel` 10216bd0/10216a20, recorded privately) and soundscapes. Then detail sprites (DESIGN 11c) and the remaining owner-order items.

## 2026-10-08 plaza barrier materials and monitor proxy motion

**Agent/model:** Codex Desktop, `gpt-6.1-sol` (verified session metadata); materially involved research/implementation subagents inherited this model.

**Changed:** Bounded 2D VTF frame decoding, ordered PlayerProximity/Subtract/Clamp frame proxies, independent brush material instances and UnlitTwoTexture texture multiplication/scrolling. Monitor proxies now evaluate literal LinearRamp and translation-only TextureTransform in authored order. The seeded Kleiner fixture enables its separate blue Combine HUD brush. AGENTS requires model attribution; historical entries without individual evidence say "not recorded".

**Why:** Native plaza shields select darker authored frames with distance from the local player's center; they are not permanently bright panes. Kleiner's scanline material uses a negative Y ramp to scroll top to bottom. The blue graphics come from `overlay_kleinertv`, enabled by security_01, not the camera shader.

**Tested how:** SDK b8cfb12 first, exact read-only pass14 retail retrievals and owned VMT/VTF/BSP checks (private barriers-20261008, monitor-scroll-20261008 and weapon-switch-audio-20261008). `cargo test --locked --workspace`: 344 passed; owned `cargo test --locked --workspace -- --ignored`: 33 passed; strict all-target Clippy and fmt passed. `scripts/build-bevy.ps1`: SHA256 `6E6C7FE4ED4C5988553739F37F596F33CB632F8765937AEAFC3B0B4D0330A3B2`, built 2026-10-08T01:39:14.6464456Z. Packaged regression batch + movement26, weapons17, attention26 assertions passed. Weapons burst: 0/121 missing with flicker_check.py. Barrier near/mid/far two phases and 61-frame bursts; monitor four clock samples and 31-frame bursts; corrected HUD capture at 1280x720. Native HDR2 shield and monitor references; owner's native screenshot independently identifies the HUD texture.

**Result:** Authored distance-frame fading is fixed, confirmed by the owner. Correction after the owner's 2026-10-08 follow-up: barrier scrolling animation is still broken; the earlier scrolling-success claim is withdrawn. Changing uploaded UV values did not establish visible animation. Kleiner's monitor scanline motion was verified separately. HUD graphics render over the camera. No playback fix: per-process audio recordings reproduce full sound tails; the owner's halfway cutoff remains unresolved. Owned decoder regression added.

**Still broken or not tested:** Plaza barrier scrolling animation remains broken; proximity fading alone is fixed. Ordinary campaign security/HUD timing, focused manual weapon selection and native audible comparison, complete proxy vocabulary, UnlitTwoTexture envmaps, full lighting/color parity, detail sprites, displacement overlays and overlay fade distances. Barrier native capture lacked fresh loaded-module hash proof (failed helper); later monitor capture verified hashes. Initial paused barrier and black intro monitor captures were rejected. Two system-loopback attempts were contaminated; per-process capture replaced that approach. The regression helper's older image baselines differ substantially, so assertion success does not establish identical pixels. No new FPS benchmark.

**Next:** Diagnose weapon cutoff with a new reproduction and fix remaining barrier scrolling animation; detail sprites and leaf decals per DESIGN11c, then displacement overlays/fades, Metrocop movement/idle + campaign security comparison, renderer LDR/brush-model cubemaps/directional lightmaps. OrbitalMan retains menu/settings ownership.

Newest entries first. Historical entries retain their original wording/test scope; current state is in STATUS.md. This is a Rust rewrite. New entries follow the [requested MODLOG template](https://github.com/trevaintdead/ai-game-modding-guides/blob/main/templates/MODLOG-template.md).

Each entry records **Agent/model**. Historical entries without per-entry provenance are marked "not recorded"; general session environment notes do not establish an individual entry's author or model.

## 2026-10-08 info_overlay fragments on brush faces

**Agent/model:** not recorded.

**Changed:** source-assets::overlays builds fragments per the retail COverlayMgr brush path (private overlay-20261008/README.md: engine.dll 10149db0, 10147b70, 1014c970, 1014d230, 1014c700, 1014cbd0, 1014a6c0):
- V = normalize(N x U), negated by the VBSP flip bit. Quad corners are origin + uv.x U + uv.y V, with texcoords (U0,V0), (U0,V1), (U1,V1), (U1,V0).
- Each listed face is fanned (triangles of area <= 1 are skipped); each triangle is flattened onto the overlay plane and clips the quad.
- Clipped vertices get bilinear texcoords from their inverse-bilinear position in the quad (SDK PointInQuadToBarycentric convention), are projected back onto the face along N and pushed 0.1 units off it (0x1031d910 = 0.1f).

The BSP world fills the new `World::overlays` (lightmapped by the face's lightmap; not decal/impact receivers), and the Bevy host draws them like world surfaces.

**Why:** DESIGN 11b. The plaza "leaves" attribution in the session 5 plan was wrong: d1_trainstation_02's 27 overlays are ivy, wall posters/graffiti, floor stains, blood and two trash decals; none are leaves.

**Tested how:**
- 338 normal tests (new: one-face quad, face clipping, flipped V and bilinear corners) and 30 owned tests (new: trainstation_02 gives 26 overlay surfaces / 474 triangles with valid lightmap pages; the 27th overlay's two trash decals are on displacements). Strict Clippy and fmt.
- Packaged captures vs native HDR: the ivy on the station's outer wall (view T005456Z) matches in placement and shape; the floor around a hall stain (T005326Z) is within 1-2 levels.
- Regression batch artifacts/overlays-regression: all exits 0, verifiers 26/17/26, no viewmodel drops (burst 120). Image changes are confined to the Breen views (the wall stain behind the translucent jumbotron).

**Result:** Overlay decals appear on brush faces.

**Still broken or not tested:** Overlays on displacement faces (the retail displacement-space path, 10148b90), overlay fade distances (HDR fade lump), render-order sorting, and overlays on brush-entity faces.

**Next:** Owner reports: weapon-switch sound cut halfway, plaza Combine barrier fade/animation, Kleiner monitor scanlines; then the plaza leaves/park grass (detail props, DESIGN 11c).

## 2026-10-08 $selfillum for LightmappedGeneric and VertexLitGeneric

**Agent/model:** not recorded.

**Changed:** Materials with `$selfillum` (LightmappedGeneric, VertexLitGeneric) blend toward `$selfillumtint` x albedo by the base alpha before the envmap is added, as in SDK lightmappedgeneric_ps2_3_x.h and vertexlit_and_unlit_generic_ps2x.fxc. The flag is cleared when the base texture has no alpha (SDK dx9 helpers). `$selfillummask`, `$selfillumfresnel` and `$selfillum_envmapmask_alpha` are not supported.

**Why:** The trainstation hall windows (window002e and the left arch windows) were far darker than native.

**Tested how:** 335 normal tests (new self-illum rule test), strict Clippy and fmt. Packaged hall capture vs native HDR view-d1_trainstation_02-20261007T164900Z: upper windows (147,154,164)/(135,140,150) vs native (146,153,163)/(131,137,147), up from (99,102,94)/(59,57,49); left windows (197,187,141)/(192,182,142) vs native (200,189,142)/(196,186,145), up from (72,61,37)/(82,69,43); wall unchanged (140,123,86) vs (140,124,87). Regression batch artifacts/selfillum-regression: all cases exit 0, no pose/visibility mismatches, campaign's 25 known texture errors only. Changes against artifacts/envmap-regression are self-illuminated surfaces: hall windows in the Breen views, the AR2's red rings, the pistol's sight dots and a book in the attention room. Verifiers: movement 26/26, weapons 17/17, attention 26/26. Viewmodel burst: 0 of 121 frames lost.

**Result:** The hall's upper and left windows match native within about 4 levels.

**Still broken or not tested:** The self-illum mask/fresnel modes. Native viewmodel glows were not compared side by side.

**Next:** info_overlay projection (DESIGN 11b).

## 2026-10-08 LightmappedGeneric $envmap cubemap reflections with masks and bump normals

**Agent/model:** not recorded.

**Changed:**
- source-assets::vtf decodes cubemap faces (`decode_cube`: the six faces of frame 0; the 7.0-7.4 spheremap is skipped; RGBA16161616F HDR cubemaps are kept as half floats) and reports translucency (one/eight-bit alpha flags).
- BSP world vertices carry the face plane normal, oriented to the side the winding faces.
- hl2-bevy loads `$envmap` for LightmappedGeneric (HDR mode prefers the map's .hdr.vtf), `$envmaptint`, float3 `$envmapcontrast`/`$envmapsaturation` and `$fresnelreflection`.
- Masks follow SDK lightmappedgeneric_dx9_helper.cpp: a `$bumpmap` undefines `$envmapmask`, `$normalmapalphaenvmapmask` masks by the bump map's alpha, `$envmapmask` by its RGB, and `$basealphaenvmapmask` by 1 - base alpha (cleared when the base texture is opaque).
- Bump-mapped materials (except `$ssbump`) reflect off the normal map: rgb * 2 - 1 in a tangent frame along the texture's s/t axes, as in SDK GetBumpNormals and `mul(vNormal, tangentSpaceTranspose)`. The frame is built from screen-space derivatives.
- The shader adds the SDK specular term: cube(reflect) capped at 16, then mask, tint, contrast, saturation and Fresnel.
- Envmap parameter or mask errors drop only the envmap, never the material.

**Why:** DESIGN 11a. The owner compared the trainstation_02 windows and floors with native. Retail ENV_MAP_SCALE is 16 under integer HDR (shaderapidx9 1004b7b0; private material-20261007 notes), so HDR cube texels are capped at 16. The first unmasked build gave the main hall floor a mirror glare; the owner noted that native's sheen looks like real tile, which comes from the bump normals.

**Tested how:** 334 normal tests and 29 owned tests, strict Clippy and fmt. New tests cover cube decoding (synthetic and HDR), owned window cubemaps, owned brush-normal orientation, and mask selection/vector parameters. Packaged captures were compared against native HDR:
- Outside the window002c wall (view-d1_trainstation_02-20261007T231545Z): bright panes (116,106,76) vs native (121,110,79), right panes (121,114,84) vs (126,118,87); with the envmap off (84,73,43)/(82,72,40).
- Main hall tile floor pitched down (T002014Z halltiles): left/mid/right (54,43,29)/(50,35,17)/(54,39,24) vs native (53,42,29)/(50,36,17)/(53,39,23). The bumped sheen breaks up along the tiles as native's does; flat normals gave a mirror streak.
- Regression batch artifacts/envmap-regression: all cases exit 0, no pose/visibility mismatches, only campaign's 25 known station03 texture errors. Differences against artifacts/bloomfix-regression are confined to envmapped surfaces (plaza windows, a metal piece under Breen's screen, a metal surface in the attention room). Verifiers: movement 26/26, weapons 17/17, attention 26/26. Viewmodel burst: 0 of 121 frames lost.

**Result:** Window and floor reflections appear where native has them and at similar brightness.

**Still broken or not tested:**
- env_cubemap on brush entities and models (runtime nearest-cubemap lookup); VertexLitGeneric envmaps; `$ssbump`.
- Bumped diffuse lighting: the three directional bump lightmaps are not used.
- Mask and bump UVs reuse the base texture transform. Displacement normals use the base face.
- Textures have no mip chain, so distant normal-mapped sheen can alias.
- LDR-mode envmaps (an sRGB path is coded) are **not tested**.
- The hall's left and upper windows needed `$selfillum` (next entry); an intermediate attribution to the trainstation_arch001 prop's lighting was wrong.

**Next:** `$selfillum`, then info_overlay projection (DESIGN 11b).

## 2026-10-08 Fix the blinking weapon viewmodel (bloom pass ordering)

**Agent/model:** not recorded.

**Changed:** The Source bloom pass (hl2-bevy bloom.rs) now runs in Bevy's `Core3dSystems::PostProcess` set, after the camera's main pass, instead of being ordered only `.before(tonemapping)`. New test option `--capture-burst N` saves the N frames after `--capture` as `<capture>-1.png` ... `<capture>-N.png`, so flicker can be measured. Private helpers: work/publishing/flicker_check.py (counts frames without viewmodel pixels) and flicker_bisect_step.sh.

**Why:** Owner report: the weapon viewmodel blinked in and out. The bloom system was not placed in a Core3d set, so the parallel render schedule could run it before the viewmodel camera's main pass. On those frames its post-process swap discarded the weapon drawn afterwards. Bisected with burst captures (the burst option patched onto each tested commit): 0dc8291, 1028388 and 6a7455d clean (0 of 61 frames lost); 8ee25d2 (bloom, session 5) and later lose 1-9 of 61 frames, including 1640df0 (before this session and the macOS merge) and 994df99. SDK viewrender.cpp RenderView draws the viewmodel (DrawViewModels), then the fade/overlays, then DoEnginePostProcessing (bloom), so bloom on the viewmodel camera after its main pass matches Source.

**Tested how:** Weapons fixture with `--capture-burst 120`: two runs, 0 of 121 frames without the viewmodel (before: 1-9 of 61 per run). 331 normal tests, strict Clippy and fmt. Regression batch artifacts/bloomfix-regression: all cases exit 0, captures within 9 levels of the accepted bloom-regression baselines, no pose/visibility mismatches, campaign's 25 known station03 texture errors only; verifiers movement 26/26, weapons 17/17, attention 26/26.

**Result:** The viewmodel draws every frame in the tested fixture. Earlier single-frame captures could not show the bug; the accepted bloom-regression weapons capture happened to land on a good frame.

**Still broken or not tested:** Other Core3d ordering assumptions were not audited beyond this pass. Ordinary play was **not tested** by an automated check; the owner's live report is the reference.

**Next:** Envmap masks (`$normalmapalphaenvmapmask`, `$envmapmask`) on wip/envmap before it merges.

## 2026-10-07 README platforms table; feature list moved to docs/features.md

**Agent/model:** not recorded.

**Changed:** README.md gains a "Platforms" table near the top: Windows (primary tested), macOS Apple Silicon (tested by a contributor, PR #1), Linux and other systems (**not tested**). The long "Implemented so far" list moved verbatim to docs/features.md; the README keeps a one-paragraph summary with a link.

**Why:** Owner request: show confirmed platforms at a glance and shorten the README. A versioned docs page was chosen over a GitHub wiki so the list stays reviewable in PRs alongside the code.

**Tested how:** Documentation only. Checked that docs/*.md is whitelisted, that no document links to the old README anchor, and that relative links in docs/features.md resolve.

**Result:** README is shorter; the feature list content is unchanged.

**Still broken or not tested:** Linux builds and runs are **not tested**; CI runs on Windows only.

**Next:** Envmap cubemaps (DESIGN 11a) on wip/envmap.

## 2026-10-06 Apple Silicon (macOS aarch64) support for hl2-bevy

**Agent/model:** not recorded.

**Changed:** Added first-class macOS / Apple Silicon support to `main`:
- `source-assets::install`: Auto-discovers installed Steam content in `~/Library/Application Support/Steam`.
- `hl2-ui::console`: Multi-target test assertions for non-Windows platforms, eliminating unused variable warnings under strict Clippy.
- `scripts/build-bevy.sh` & `launch-bevy.sh`: Added POSIX build and launch scripts (with 1080p and borderless variants) generating `bin/build-bevy-info.json` and packaging `bin/bevy-assets`.
- `.gitignore`: Whitelisted new POSIX scripts.

**Why:** Enable running the Bevy/wgpu host natively on macOS Apple Silicon without manual path flags or Windows-only script dependencies.

**Tested how:**
- `cargo test --workspace --locked` (109 shared crate tests, 25 Bevy tests passed).
- `cargo clippy --workspace --all-targets --locked -- -D warnings` (zero warnings).
- `cargo fmt --all --check`.
- Packaged release build with `./scripts/build-bevy.sh` (`bin/hl2-bevy`, `bin/bevy-assets`, SHA256 metadata verified).
- Launched `./launch-bevy.sh`: Auto-discovered owned Steam installation on macOS, initialized wgpu/Metal clustering and preprocessing, created native AppKit window, and loaded `d1_trainstation_01`.

**Result:** Native macOS execution on Apple Silicon via Metal and CoreAudio works out of the box with zero runtime errors.

**Still broken or not tested:** Retained Macroquad/OpenGL host on macOS (deprecated by Apple; main is primary target). Native Windows GDI font parity (macOS uses portable fontdue). Retained scripts remain Windows-focused.

## 2026-10-07 Source bloom, pre-bloom exposure histogram, monitors at scale 1

**Agent/model:** not recorded.

**Changed:** Source 8-bit bloom as a Bevy Core3d post pass on the viewmodel camera (the last 3D camera, so it covers sky, world and viewmodel and runs before the HUD): gamma-space Shape (pow 2.2 times luminance with r_bloomtint 0.3/0.59/0.11) over a quarter-size 4-tap (4x4) downsample, 13-tap Gaussian blur in X and Y (SDK offsets/weights; Y times the bloom amount), additive composite onto the gamma frame (Engine_Post BloomFactor 1). env_tonemap_controller SetBloomScale sets the scale; the amount eases by 0.05 per frame from 1 (GetBloomAmount). The exposure histogram now reads a 320x180 pre-bloom presample by GPU readback (the SDK histogram runs before bloom) instead of window screenshots. Monitor (camera feed) views render at tonemap scale 1. Dev profile: no debug info for dependencies, line tables for our crates (target/debug 36 GB -> 3 GB).

**Why:** Native HDR shows bloom around bright windows and screens, and our world stayed 10-20% darker. With bloom in the screenshot the histogram dropped our exposure (plaza 1.45), which is why it must measure before bloom. SDK viewrender.cpp draws monitors before TurnOnToneMapping after the previous main view reset the integer-HDR scale to 1 (lines 2080, 2090, 2214), so applying the scale to the feed and the screen doubled it.

**Tested how:** 328 normal tests, strict Clippy/fmt. Packaged captures vs settled native HDR: hall wall (162,142,101) vs (159,139,100), screen (170,140,106) vs (181,153,121), plaza street (152,137,97) vs (162,144,100), sky (103,102,104) vs (102,101,102), exposure plaza 1.95 vs 2.00, hall 2.00 vs 1.88, slate 0.651 vs 0.65. Regression batch artifacts/bloom-regression: complete, 0 mismatches, movement 26/26, weapons 17/17, attention 26/26; contact sheet inspected (bloom on bright monitors/explosion, HUD unaffected).

**Still broken or not tested:** Window panes need `$envmap` cubemaps (native left windows 159 vs ours ~70). Plaza buildings remain ~15% darker. Bloom is not applied to monitor feeds (matches Source). No LDR mode. Batch images depend on frame timing (auto exposure).

**Next:** Environment maps, then the plaza items (info_overlay leaves, detail props, metrocop).

## 2026-10-07 Source HDR path: HDR lightmaps, auto exposure, HDR sky

**Agent/model:** not recorded.

**Changed:** Rendering now follows Source's HDR path (mat_hdr_level 2). Maps with HDR lighting use the HDR lightmap, ambient and world-light lumps; the lightmap atlas is linear RGBA16F capped at 16 (retail integer HDR range). Every Source material and sprite multiplies its output by the tonemap scale (Bevy camera exposure on the world, viewmodel, monitor and sky cameras). Auto exposure follows SDK viewpostprocess.cpp (17-bin histogram of the presented frame's linear luminance over the central 90% x 85%, 2% bright pixels at 60%, minimum 3% median, V-weighted 10-sample goal) and retail materialsystem 10062660 adaptation (rate x 2, accelerated darkening, step capped at 1/64), with env_tonemap_controller SetAutoExposureMin/Max, SetTonemapRate and UseDefaultAutoExposure. Sky faces with `$hdrcompressedTexture` decode RGBS (rgb x alpha x 8). VMT conditionals evaluate `hdr?` true and `ldr?` false. `--tonemap-scale S` forces a scale (mat_force_tonemap_scale). Histogram readbacks never share a frame with the final capture.

**Why:** Owner screenshots showed their game uses "High Dynamic Range: Full" (mat_hdr_level 2, tonemap scale 1.35 in the console) and a much brighter Combine slate. The native oracle's private config (mirrored by Steam Cloud) had mat_hdr_level 0, so all earlier native captures were LDR; the oracle now passes +mat_hdr_level 2 +skill 2 and can echo cvars per view (ORACLE_QUERY) after a dwell (ORACLE_DWELL). Evidence: SDK stdshaders (FinalOutput TONEMAP_SCALE_LINEAR in LightmappedGeneric, VertexLitGeneric, UnlitTwoTexture, Sky_HDR_DX9, sprites), SDK viewpostprocess.cpp; retail materialsystem 10062660 (adaptation), 10062ae0 (integer HDR lightmap encoding), 10057340/10062b20 (LDR lightmap table, for a future LDR mode), client 101d8490 (histogram query). Private notes: work/hl2-decompiled/material-20261007/README.md.

**Tested how:** Unit tests for histogram bins/target, goal averaging, retail adaptation, Bevy exposure mapping, HDR atlas cap, conditionals; 328 normal tests, strict Clippy/fmt. Native HDR captures with settled exposure (view-d1_trainstation_02-20261007T164900Z: plaza 2.00, hall 1.88; T165556Z slate 0.65). Bevy: plaza 1.80-1.98, hall during the broadcast 1.92 with the screen at (184,158,123) vs native (181,153,121), slate 0.69 with native's cyan/white-glyph look. Regression batch artifacts/hdr-regression: all cases complete, 0 pose mismatches, movement 26/26, weapons 17/17, attention 26/26; every image changes (exposure). Readback cost about +0.3 ms CPU and +0.2 ms GPU at the 60 fps cap. Rejected: gamma-space histogram (hall pinned at 0.5) and unclamped float lightmaps (white slate).

**Still broken or not tested:** The world is still about 10-20% darker than native (no bloom; SetBloomScale .4 is visible in native around windows). Window panes need `$envmap` cubemaps. No LDR mode or options menu yet (owner idea: Video options with HDR None/Full). Auto exposure depends on frame timing, so batch images are not bit-reproducible. HDR on maps without HDR lumps falls back to LDR data with the tonemap scale.

**Next:** Bloom (SDK Bloom/Downsample shaders), environment maps, then the LDR mode for the options menu.

## 2026-10-07 Eyes keep a latched view target (Breen looks into the broadcast camera)

**Agent/model:** not recorded.

**Changed:** Actor eyes now follow SDK CBaseFlex/CAI_BaseActor view-target semantics: the last chosen eye target persists per actor while ValidEyeTarget holds (at least 1 unit away, within 75 degrees of the head). With no scene interest or visible candidate, the eyes look at a point 128 units ahead of the head with the SDK's right +-32 / up +-16 jitter (deterministic per actor and scene clock), so both eyes converge instead of staying parallel.

**Why:** Owner review: native Breen's eyes focus on the broadcast camera while ours looked into infinity. instinct.vcd's `LookAt camera_tv_breen` event is authored inactive, so the native focus comes from the default view target (SDK ai_baseactor.cpp MaintainLookTargets random view and ValidEyeTarget). The camera is near the 128-unit point in front of Breen.

**Tested how:** New unit test; 324 normal tests, strict Clippy/fmt; packaged Breen broadcast report: eye target was none, now "view" at (919, 7517, -237) with both eyes sharing it; attention fixtures 26/26. Feed close-up shows slightly converged irises (256 px feed; not compared side by side with native at that resolution).

**Still broken or not tested:** Native random look-target selection among nearby entities, blink on target change and head-direction decay are approximated by the existing nearest-visible fallback. No native close-up comparison yet.

**Next:** HDR lighting path (owner report: native broadcast screen is much brighter).

## 2026-10-07 Monitor screen colour: VMT conditionals and linear UnlitTwoTexture modulation

**Agent/model:** not recorded.

**Changed:** The VMT reader applies retail MaterialSystem key conditionals (`test?$var`, optional `!`): passing keys replace the plain value, failing keys are skipped. Tests are evaluated for a DX9 sRGB-capable renderer without HDR (`srgb`, `ldr` true; `hdr`, `lowfill`, `360` and unknown tests false). Camera monitor materials (UnlitTwoTexture) now read $texture2 through an sRGB view and convert the ($color x $color2) modulation to linear like SDK SetModulationPixelShaderDynamicState_LinearColorSpace (channels above 1 unchanged; mathlib GammaToLinear table, 1.0 from 0.95).

**Why:** The jumbotron screen (dev/dev_combinemonitor_3) sets `srgb?$color2 "[2.5 2.5 2.5]"`, which we ignored, so the 0.4 $color proxy dimmed the feed, and the scanline texture was multiplied as gamma bytes. Evidence: SDK 2013 unlittwotexture_dx9.cpp/_ps2x.fxc (both samplers sRGB-read, result = base x texture2 x modulation), BaseVSShader.cpp:652, BaseShader.h ApplyColor2Factor, mathlib color_conversion.cpp; retail materialsystem.dll 1003c230 (conditional tests, return value = skip) and 1003b300 (a passing conditional replaces the plain var). Private notes: work/hl2-decompiled/material-20261007/README.md.

**Tested how:** New unit tests for conditionals and modulation; 323 normal tests, strict Clippy/fmt. Packaged slate fixture vs native slate.png: screen mean RGB (78,122,114) vs native (76,156,154) (was (80,118,100)); white glyphs (165,199,199) vs (175,214,218); scanline grid and black frame match. World regions unchanged. Rejected experiment: a 16x lightmap cap gave (105,229,230), far brighter than native; no constant was fitted.

**Still broken or not tested:** Native runs the HDR path: the slate luxels are linear 128 in both lumps, and native's HDR lightmap range and TONEMAP_SCALE_LINEAR saturate the poster further (native G and B equal, ours G > B). HDR lightmaps/tonemapping are not implemented. Fallback block selection (">=DX90", "hdr_dx9", ...) is not implemented. The conditional change affects every material that uses `ldr?`/`srgb?` keys; the regression batch is the next check.

**Next:** Rerun the regression batch and accept baselines; HDR lighting remains a separate, larger plan.

## 2026-10-07 Combine slate frame and lightmap overbright headroom

**Agent/model:** not recorded.

**Changed:** World and brush-entity faces using tools/toolsblack* now render (an owned UnlitGeneric black texture); every other tools/ material stays skipped. The lightmap atlas stores gamma(linear / 2) and lightmapped materials restore the factor 2 before the gamma decode, so baked light between 1.0 and 2.0 is no longer clipped. New fixture test-inputs/bevy-monitors-breen-slate.json views the jumbotron before the broadcast starts.

**Why:** Native trainstation_02 shows only the bright Combine slate on the jumbotron at map start; ours showed Breen around a dark slate. The slate func_brush (*87) is framed by four toolsblack faces we skipped. Its `_minlight 255` is not a runtime value: SDK 2013 VRAD (radial.cpp:676/813) bakes a per-luxel floor of `_minlight*128` into the lightmap. Source LDR lightmaps keep 2x headroom (imaterialsystem.h OVERBRIGHT 2.0), which our clamp at 1.0 removed.

**Tested how:** Updated lightmap unit test (linear 2.0 saturates, 1.0 encodes gamma(0.5)=186), 321 normal tests, strict Clippy/fmt. Packaged slate fixture against native view-d1_trainstation_02-20261007T054643Z/slate.png (artifacts/slate): black frame now matches native; screen mean RGB before/after/native (76,97,82)/(80,118,100)/(76,156,154); wall, rail and window regions unchanged within 2 levels.

**Still broken or not tested:** The screen is still greener and darker than native's cyan (UnlitTwoTexture colour path, next step). Native runs the HDR path (HDR lightmaps and auto-exposure tonemapping), which is not implemented. Regression batch not yet rerun.

**Next:** UnlitTwoTexture modulation/sRGB and the pixel grid; rerun the batch and accept baselines.

## 2026-10-07 Static map decals (Breen's studio backdrop)

**Agent/model:** not recorded.

**Changed:** infodecal entities without a targetname are now projected at map load onto lightmapped world brush faces (modkit-core::decals::static_decal; hl2-bevy assets add_static_decals). Each decal is sized from its base texture times $decalscale, centered on the plane point within 5 units, oriented by the receiving face's texture axes, clipped to its rectangle and lit by the face's lightmap.

**Why:** Owner review of the native jumbotron: the yellow Combine logo behind Breen is his studio backdrop (decals/decal_posterbreentv), not see-through screen areas as I had first recorded. SDK 2013 world.cpp CDecal::StaticDecal places static decals on world brushes; props are excluded by its trace filter.

**Tested how:** New decal unit test, 321 normal tests, strict Clippy/fmt. Packaged Breen fixture: the feed now shows the backdrop with the logo, matching native session view-d1_trainstation_02-20261007T053505Z. **Not run:** the full regression batch with decals (baselines will change wherever static decals exist).

**Still broken or not tested:** Named (triggered) infodecals, decals on brush entities, engine-exact decal projection/orientation (inferred from Source behavior, engine code not reviewed), decal shaders (DecalModulate). Combine slate: native shows only a bright slate while the rest of the screen is off. The slate func_brush (*87) is surrounded by four tools/toolsblack faces (SURF_NOLIGHT), which our BSP builder skips as tools/ materials, so Breen shows around the slate. Native slate brightness (_minlight 255) and the screen's "pixel" look (UnlitTwoTexture: sRGB-read base x texture2, HDR tonemap scale) are not matched yet.

**Next:** Render tools/toolsblack as opaque black, apply func_brush _minlight, compare the screen shader and pixel grid with native, rerun the batch and accept baselines.

## 2026-10-07 Scene gestures for template-spawned actors (Barney's head-down at 21-22.5 s)

**Agent/model:** not recorded.

**Changed:** Choreography clip preparation now resolves every entity a scene actor name can match, including point_template children that are still pending (Barney, Kleiner), so their scene gesture and posture clips load with the map. Head control now measures its correction in the full 3D frame of the animated "forward" attachment, which is parented to the head bone (SDK UpdateHeadControl), instead of a level yaw-only frame. The Bevy report lists per-actor gesture composition errors (`gesture_compose_errors`). The regression batch drops the superseded edge-on Breen and empty-lab cases, whose fixtures are deleted and baselines retired.

**Why:** Owner report: Bevy Barney was stiff (no arm/body motion) compared with native. Investigation of the unexplained native head-down at security_02 21-22.5 s: native plays rubNeck (Gesture08) and thinking (posture01). The shared composition raised Barney's right hand from z 40.8 to 64.7 at 21.6 s, but the Bevy report showed MissingSequence for posture01/gesture08 (Barney) and kposture01/kgesture04 (Kleiner). scene_actor skips killed entities, and template children are killed until ForceSpawn, so their clips were never prepared at load. Every scene gesture for these actors was dropped in Bevy.

**Tested how:** 320 normal and 26 owned tests (new owned test: template-pending Barney/Kleiner require gesture08/posture01/kposture01 before ForceSpawn; new head-frame unit test), strict Clippy/fmt. Packaged close-ups at 21.00/21.30/22.45 s now match native (head bowed, hand at neck); wide 21.30 s view matches the native pose. Batch artifacts/trimmed-regression: all complete, 0 pose/visibility mismatches, attention 26/26. Kleiner-scene changes (Kleiner now gestures); weapons varies only in the HUD health box between runs. Baselines accepted.

**Result:** Barney's and Kleiner's authored gestures and postures play in Bevy, which explains the native head pitch (it is the gesture, not head control).

**Still broken or not tested:** Native comparison of the full timeline after this fix; other template-spawned NPCs are covered by the same fix but untested.

**Next:** Native trainstation_02 jumbotron/godray comparison; campaign-order reveal chain.

## 2026-10-07 Kleiner scene movement and new regression views

**Agent/model:** not recorded.

**Changed:** npc_kleiner is registered with the shared human ground-movement controller, like Barney, so scene MOVETO events move him. New regression cases: test-inputs/bevy-monitors-breen-screen.json (face-on jumbotron during the broadcast) and test-inputs/bevy-monitors-kleiner-scene.json (Kleiner on Barney's monitor in campaign state). The new baselines are accepted, with the previous ones archived locally.

**Why:** Owner report: the old Kleiner case showed an empty lab and the old Breen case an edge-on screen. Kleiner is a kleiner_template child, and security02 opens with `MoveTo marks_kleiner_catwalk_1` (0.01 s); because only Barney was registered, Kleiner stayed at his spawn, 110 units outside the lab camera. SDK 2013 npc_kleiner.cpp and the retail CNPC_Kleiner::Spawn (server.dll 10391cd0, reviewed privately) agree on HULL_HUMAN, SOLID_BBOX, MOVETYPE_STEP and capabilities 0x801801 (ground move, open doors, turn head, animated face) plus friendly-damage immunity.

**Tested how:** hl2-simulation tests (139), strict Clippy/fmt. Regression batch artifacts/kleiner-regression: all 11 cases complete with 0 pose/visibility mismatches. Diff counts against the old baselines are unchanged from the lighting batch, except weapons (view-model lighting basis). Attention verifier 26/26. Kleiner reaches (-850, 2334) and speaks on the monitor at 10.5 s; Barney's faceplate is off and the cameras are folded in the attention fixtures.

**Result:** The lab feed shows Kleiner talking; the jumbotron shows Breen face-on. New accepted baselines cover the lighting change, the godray fix and the new views.

Follow-up (owner report: Barney was talking to the default blue screen): all eight security_02 fixtures now also force-spawn kleiner_template, enable barney_security_monitor_1 and switch on kleiner_security_camera_1, as the campaign's triggers do. Rerun batch artifacts/kleiner-monitor-regression: pixel-identical to the new baselines, attention 26/26, and Kleiner is on Barney's monitor in the attention view. The private native security oracle setup gets the same three inputs (not yet run).

**Still broken or not tested:** Kleiner's walk/turn timing against native is not compared. Other NPC classes (metrocops, citizens) are still not registered for movement. No native comparison of the lab feed or jumbotron yet.

**Next:** Native head-pitch analysis; native jumbotron/lab feed comparison once the oracle cursor fix is approved.

## 2026-10-07 Bevy becomes main; Macroquad host removed

**Agent/model:** not recorded.

**Changed:** At the owner's request, bevy-migration was merged into main (main's docs-only commit b4b1731 was merged in first) and main now fast-forwards to it. The former Macroquad/OpenGL host was removed: crates/hl2-runtime, its launchers (launch.cmd, launch-borderless.cmd, launch-1080p.cmd), scripts/build.ps1, the vendored third_party/miniquad and quad-alsa-sys shim, its 13 input-script fixtures and the mods/*.json sandbox files. Cargo.lock only drops the 20 packages of that stack. AGENTS, README, CONTRIBUTING, STATUS, docs and CI now describe a Bevy-only rewrite on main. macroquad-prototype, bevy-migration (frozen at the merge) and wip/scripted-scenes are preserved.

**Why:** Owner decision: Bevy is far ahead of the old host, so checking both hosts after shared changes was overhead. Removing the old host keeps the repository clean.

**Tested how:** After removal: 319 normal tests (9 removed with the old host) and 25 owned tests, strict Clippy/fmt, packaged Bevy build, and the movement (26/26) and entities/weapons (17/17) packaged fixtures with their verifiers.

**Result:** One host. The shared crates (source-assets, modkit-core, hl2-simulation, hl2-ui) are unchanged.

**Still broken or not tested:** History and the macroquad-prototype branch still hold the old host; its features that Bevy lacks (JSON sandbox, inspect/verify/export subcommands) are gone from main.

**Next:** Scene MOVETO for Kleiner (lab feed), Breen face-on regression case, native head-pitch analysis.

## 2026-10-07 Source model lighting (leaf ambient cubes and world lights)

**Agent/model:** not recorded.

**Changed:** Models are now lit the way the retail engine's light cache does it, instead of with a constant gray. New `modkit-core::lighting`: leaf ambient samples weighted by 1/(d^2+1); world lights with Source falloff, styles and a world-only visibility trace (8-unit slack); up to four local lights kept by luminance and the rest folded into the ambient cube; skipping lights flagged as already baked into the cube; the ambient boost for flagged models; SDK vertex shader terms. New `source-assets::model_lighting` reads the leaf tree, the LDR/HDR leaf ambient lumps (matching the lightmap choice), world lights and sky faces. `Vertex.normal` carries VVD normals, and the studio illumposition/flags are read per model. Static props are baked once per vertex at load, which applies to both hosts. Bevy model draws get a per-entity lighting uniform that is recomputed when the illumination origin moves. The shader evaluates ambient cube + diffuse (+ $halflambert) on the linearized base texture, with skinned normals on both GPU and CPU paths. The view model maps its camera space onto the player's view for lighting.

**Why:** Owner report that entities lack the map's lighting (DESIGN step 8). Retail engine.dll was reviewed privately (lighting-20261007/README.md): Mod_LoadLeafs, Mod_LeafAmbientColorAtPos, light-cache selection, the draw-time ambient boost and the light-to-shader conversion. SDK 2013 corroborated ColorRGBExp32ToVector (its factor of 255) and the light-descriptor cone math.

**Tested how:** 328 normal tests (7 new lighting/decoder tests) and 25 owned tests (d1_trainstation_01: 535 world lights, 13,479 ambient samples), strict Clippy/fmt. Packaged Barney close-ups against native: at 18.05 s, mean skin RGB is 95/66/45 (Bevy) vs 93/64/42 (native), with luminance percentiles 48/67/93 vs 45/65/90; at 22.45 s the helmet and hair tones match (confirmed by the owner). A new private generic native view oracle (run_view_oracle.py) captured the d1_trainstation_03 corridor: the door is equally dark and the pistol view model lit similarly. Regression batch artifacts/lighting-regression: every case completes with 0 pose/visibility mismatches; images change wherever models appear (expected relighting). Retained smoke capture renders with baked static props.

**Result:** Barney, props and the view model now pick up the map's ambient color and nearby lights. Face shading follows the native key light.

**Still broken or not tested:** Shadows; bump/phong/specular (native helmet sheen); flex normal deltas; styled lights are treated as on; the skylight uses sky-face polygons instead of a surface-flag trace; HDR tonemapping; the retail 162-sample static-prop path. The retained host does not light dynamic models. The first native corridor capture was rejected because the game took focus and trapped the owner's cursor. The oracle now records the user's foreground window before launch and passes +cl_mouseenable 0.

**Next:** Explain the native head-down pitch at 21-22.5 s; trainstation_02 jumbotron/godray native comparison; campaign-order reveal chain.

## 2026-10-06 Entity render color in Bevy (godray brightness)

**Agent/model:** not recorded.

**Changed:** Bevy entity materials now apply Source color modulation. rendercolor tints the model, and renderamt sets its alpha outside kRenderNormal. Materials are keyed per (material, lightmap, modulation), so untinted entities still share handles. New fixture test-inputs/bevy-monitors-breen-screen.json frames the trainstation_02 jumbotron face-on at about 25 s, during the broadcast.

**Why:** Owner report: the trainstation godrays were overexposed in Bevy (the retained host already applied rendercolor). The shafts are additive prop_dynamic vol_light models tinted to about 19% (rendercolor ~49 45 34), which Bevy drew at full white. The owner also noted the old Breen fixture looks at a wall. jumbotron1's screen layers face +-Y, so the old yaw-180 view is edge-on, and its 7 s capture precedes the broadcast (the Combine slate brush is correct then; the scene starts at about 11 s and its OnTrigger1 hides the slate).

**Tested how:** Packaged captures (artifacts/godrays). Regression batch artifacts/godray-regression: only the Breen main view changes (whole frame, dimmer shafts); weapons/Kleiner/campaign identical; movement unchanged from the previous batch (735 px against the old baseline); attention 26/26.

**Result:** Godrays are subtle instead of whiting out the hall. The new fixture shows Breen speaking on the jumbotron, with lip sync.

**Still broken or not tested:** No native trainstation_02 comparison yet for shaft brightness or the jumbotron, whose dark feed areas render see-through. The accepted partition-breen baseline image predates this change.

**Next:** Model lighting (docs/DESIGN.md step 8).

## 2026-10-06 Scripted-sequence script events (faceplate removal)

**Agent/model:** not recorded.

**Changed:** While a scripted_sequence plays, SCRIPT_EVENT_FIREEVENT (1003) animation events crossed by the actor's clip fire the script's OnScriptEventNN output, with NN from the event options and the actor as activator.

**Why:** ss_Helmet_Reveal (helmet_reveal, events "1" at cycle 0.417 and "2" at 0.762) removes Barney's head faceplate and toggles the hand copy through these outputs.

**Tested how:** Packaged Bevy captures before, between and after the events (artifacts/reveal): Barney lifts the faceplate, then his face is revealed with helmetBack kept. hl2-simulation tests and strict Clippy/fmt pass.

**Still broken or not tested:** The native side-by-side of security_01 and the reveal was not run. Other studio script events (1000-1008) are not dispatched.

**Next:** Model lighting; Breen fixture framing; godray brightness.

## 2026-10-06 Head/body flexes, Combine camera activities and movement turning

**Agent/model:** not recorded.

**Changed:** Actor pose parameters now add the server-side flex controllers, following SDK CAI_BaseActor: head_rightleft/updown/tilt on top of the head look correction (UpdateHeadControl), body_yaw/spine_yaw/neck_trans from body_rightleft/chest_rightleft/head_forwardback (UpdateBodyControl), and gesture_height/width from gesture_updown/rightleft (MaintainLookTargets). The values are included in the pose signature. Head control takes its eye position from the animated "eyes" attachment and the self-look direction and head frame from "forward" (shared `Scene::attachment_frames`). npc_combine_camera deploys at spawn (open idle) unless StartInactive. Enable/Disable/Toggle play the retract transition into closed idle, or return to open idle. Walking NPCs turn toward the path at MaxYawSpeed (45 x 10 deg/s) instead of snapping, and the move_yaw pose parameter keeps the legs on the path.

**Why:** Owner steps 5 and 7. Step 6 check: Barney's state origin at his desk equals mark_barneyroom_monitor_3 exactly, and the 18.05 s native/Bevy silhouettes line up within about one unit, so the session 2 ~5-unit offset no longer reproduces.

**Tested how:** 321 normal tests (new synthetic head/body flex and camera activity tests), strict Clippy/fmt. Packaged close-ups at 18.05/21.00/22.45 s against native sessions security-scene-20261006T204141Z and T214010Z. Camera open/retracting/closed captures (artifacts/camera-acts). Walk samples along Barney's route (artifacts/facing).

**Result:** Scene head flexes move Barney's head by their authored degrees. Cameras fold when logic_disable_cameras fires. Path corners turn smoothly.

**Still broken or not tested:** At security_02 21-22.5 s native Barney's head pitches down toward the consoles while Bevy's stays level. Head flexes and gesture pose flexes are ruled out (both near zero there), and the cause is unexplained. The reversed camera-open transition, camera aim pose and eye sprites are not modeled. Entity lighting and shadows: plan recorded in docs/DESIGN.md step 8.

**Next:** Source model lighting (ambient cubes, world lights, normals).

## 2026-10-06 Lip sync from speech phonemes

**Agent/model:** not recorded.

**Changed:** New `source-assets::sentence` reads the text VDAT chunk of owned WAVs (SDK CSentence 1.0: word phonemes and emphasis samples, Catmull-Rom emphasis intensity) and flex settings files (`expressions/phonemes*.vfe`). New `hl2-simulation::lipsync` implements the client viseme path: box filter (0.08 s), neighbor crossfade extension, and weak/normal/strong emphasis blending. The pinned SDK was statically compared with retail client.dll AddVisemesForSentence 100bb720, AddViseme 100bb630 and ComputeBlendedSetting 100bbbd0 (private review). Both hosts parse VDAT when loading waves, register the actor's voice with the chosen wave when a scene line plays, and render `Scene::actor_flex_values` (scene controllers plus visemes) through the shared FaceModel path. Scenes flagged ignorePhonemes play without lip sync.

**Why:** Owner step 4: mouth movement timed to speech.

**Tested how:** Unit tests (VDAT parse/intensity, viseme box filter/cleanup). Owned tests: phoneme VFE files (48 normal, 1 weak, 1 strong settings; aa opens jaw_drop); all 23 Barney waves in d1_trainstation_01 scenes carry phonemes and move the mouth. Strict Clippy/fmt. Packaged Bevy close-ups during ba_thinking01 (21.00/21.30 s) show the lips parting and closing.

**Result:** Barney's mouth follows his lines in Bevy, and the retained host shares the path.

**Still broken or not tested:** Voice time is scene time since the request, not mixer position. phonemedelay/phonemesnap/LOD and streaming delays are fixed at defaults. Native frame-by-frame mouth comparison is limited by about 0.05 s oracle timing jitter. Retained lip sync was not visually checked.

**Next:** Head flexes into head control; Barney's mark offset.

## 2026-10-06 Retained-host facial flexes and volume option

**Agent/model:** not recorded.

**Changed:** The retained host now applies the shared `FaceModel` flex path: per-vertex studio flex keys in its draw batches, NPC face models loaded at setup, and scene controller values plus rest-gaze FACS eyelids added to bind positions before CPU skinning. It also accepts `--volume 0..1` as a master scale for all sounds, so unattended tests run at 1%.

**Why:** Shared changes must work in both hosts. Quiet testing had no option on the retained host.

**Tested how:** Strict workspace Clippy/fmt, hl2-runtime tests. Packaged retained G-Man intro close-ups (artifacts/retained-flex/gman-closeup.png, launched without activation at volume 0.01): faces deform sanely and the expression changes over time.

**Result:** Retained actors show scene facial expressions.

**Still broken or not tested:** The retained host has no eye-target presentation (eyelids use the rest gaze) and no iris shader. In a retained security_02 smoke run Barney did not spawn from his template (helmetBack stayed at its map position), a retained-specific gap not investigated. The retained input script needs an explicit `quit` before it writes its report.

**Next:** Lip sync from phoneme data.

## 2026-10-06 Entity parenting and Barney's helmet props

**Agent/model:** not recorded.

**Changed:** The simulation now supports movement parenting. Map-spawn `parentname` keeps the spawn offset, and the `SetParent`, `SetParentAttachment`, `SetParentAttachmentMaintainOffset` and `ClearParent` inputs work. Each tick, children follow the parent's pose or animated attachment, using the same composed pose both hosts render. `prop_dynamic` StartDisabled hides the prop. Gear attached to an animated attachment has no collider, so a helmet no longer blocks its wearer. The security fixtures now include the campaign state by security_02: faceplates off (ss_Helmet_Reveal runs in security_01) and logic_disable_cameras triggered.

**Why:** Owner priority: in security_02 Barney wears the separate helmetBack prop, parented by logic_barney_init to helmet_attachment after the template spawn. Only the faceplate comes off, during ss_Helmet_Reveal.

**Tested how:** 317 normal + 22 owned tests (synthetic parenting test), strict Clippy/fmt. Packaged security captures compared with native session security-scene-20261006T204141Z. Regression batch artifacts/helmet-regression: weapons/Kleiner/campaign identical; movement 751 px and Breen 3,383/6,654 px, the same known autoplay differences as the accepted 1bf087f batch. Attention 26/26.

**Result:** helmetBack renders on Barney's head through the whole scene, matching the native close-ups. With the faceplates off, Barney walks to his desk again. (An intermediate build where the solid helmet blocked his route was rejected and fixed.)

**Still broken or not tested:** The faceplate removal in security_01/ss_Helmet_Reveal has not been played or compared yet. Moving brush/physics hierarchies are not parented. Nested hierarchies lag one tick. Retained host not visually checked (shared simulation only).

**Next:** Retained-host flexes, lip sync, head flexes, Barney's mark offset.

## 2026-10-06 FACS eyelids and native face close-ups

**Agent/model:** not recorded.

**Changed:** Eyeball records now carry their FACS eyelid fields, and actor flexes apply the retail eyelid step (private review: StudioRender 1001bd80). It converts lid raiser/neutral/lowerer weights and the eye's look direction into lid descriptor values before vertex deltas. Flex math moved into shared `source-assets` (`FaceModel`, `descriptor_weights`, `vertex_deltas`) so the retained host can reuse it. Rigs now keep every model attachment. Movement scripts accept `pitch` and `fov` (Source horizontal 4:3 degrees) for zoomed comparison shots, and the private native oracle takes a matching `ORACLE_CLOSEUP`.

**Why:** Native face close-ups at 18.10 s showed Barney's eyes fully open. Bevy left the lid descriptors at zero, which half-closed the upper lids and raised the lower ones.

**Tested how:** 316 normal + 22 owned tests (synthetic eyelid test; owned Barney/Kleiner neutral face has no lid deformation), strict Clippy/fmt. Packaged close-ups `test-inputs/bevy-face-security-1805/2245.json` compared with native session security-scene-20261006T204141Z (scene times 18.10/22.50 s).

**Result:** Bevy lids now match the native open eyes at both times.

**Still broken or not tested:** Lids use the previous frame's eye direction. Head pose (native head turns and tilts further toward the player), brow intensity, Source model lighting (ambient cube/local lights), helmet props, delayed flex weights, lip sync and the retained-host flex path. Full regression batch not rerun.

**Next:** Helmet props parented to Barney's attachments, then retained flexes, lip sync and head flexes.

## 2026-10-06 Facial flexes render in Bevy

**Agent/model:** not recorded.

**Changed:** Scene-driven flex controllers now deform actor faces in the Bevy renderer (setup-loaded flex data, per-vertex flex references, retail weighting, GPU and CPU skinning paths). Earlier the same day: the flex track evaluator and Surface flex sources.

**Why:** Scripted scenes need facial animation; Barney's security_02 expressions were missing.

**Tested how:** 315 normal tests, strict Clippy/fmt, owned flex/surface mapping test, packaged security captures, attention 26/26.

**Result:** Barney's expression changes during security_02.

**Still broken or not tested:** Retained host flexes, native facial comparison, delayed weights, lip sync/phonemes, eyelid-eye interaction, normals.

**Next:** Native face comparison at matching times, then lip sync (phoneme tracks to flex settings).

## 2026-10-06 Facial flex data reader

**Agent/model:** not recorded.

**Changed:** New `source-assets::flexes`: MDL flex descriptors, controllers, rules and mesh vertex deltas; SDK RunFlexRules and the retail vertex weighting.

**Why:** First step toward facial animation in scenes (FlexAnimation tracks, expressions, later lip sync).

**Tested how:** Synthetic rule/ramp tests; owned Barney/Kleiner parse and jaw_drop evaluation; strict Clippy.

**Result:** Real flex data parses and evaluates.

**Still broken or not tested:** Not driven by scenes and not rendered yet; delayed weights and wrinkles are not applied.

**Next:** Scene flex tracks to controller values, then CPU vertex deltas in both renderers.

## 2026-10-06 Pose-parameter blends, autoplay head sequences and head control

**Agent/model:** not recorded.

**Changed:** Rigs keep pose parameters, full blend grids and autoplay sequences. Composition samples blends by pose parameter and adds autoplay after layers. NPC head control drives head_yaw/pitch from look interests with the SDK think rates.

**Why:** FACE turned Barney's body away from the player; the original turns his head on pose parameters. Blend grids and autoplay are also how the models encode head, body and gesture variation.

**Tested how:/** 312 normal and 21 owned-install tests, strict Clippy/fmt; synthetic blend/head tests; owned Kleiner autoplay/blend checks; attention 26/26; regression batch.

**Result:** Barney looks over his shoulder at the player while facing the monitor.

**Still broken or not tested:** Chest-bias limits, roll, random/synthetic looks, eye-position attachments in the simulation, sequence transitions, IK, bone controllers, the posekey 2D path and locomotion move_yaw blending for base clips.

**Next:** Rerun the native security comparison; then facial flexes or locomotion move_yaw blending.

## 2026-10-06 Scene FACE events, arrival distance, Combine camera model

**Agent/model:** not recorded.

**Changed:** FACE events turn standing NPCs toward targets at SDK yaw speed. MOVETO walks until within the event arrival distance (2D). The npc_combine_camera model is set. Unattended-test options: Bevy `--volume`/`--no-focus`, and a quiet, unfocused native oracle.

**Why:** In the native comparison, Barney faced the wrong way, stopped 9 units short of his mark, and the wall camera was missing.

**Tested how:** 310 normal tests, strict Clippy/fmt; FACE unit test; packaged security fixtures; movement and attention verifiers.

**Result:** Barney stands on his mark and faces the monitor; the camera renders.

**Still broken or not tested:** Attention 24/26: without head pose control Barney cannot look at the player while his body faces the monitor. Facing while moving, camera open/close activities and facial flexes are not implemented.

**Next:** Pose-parameter sequence blending and CAI_BaseActor head control (head_yaw/head_pitch) driven by attention targets.

## 2026-10-06 NPC door lookahead: Barney reaches his desk

**Agent/model:** not recorded.

**Changed:** NPC routes remember door segments and request the door ahead of contact (within 96 units).

**Why:** The cop-door trigger unlocks barney_door_2 for only one second, before Barney touches the door.

**Tested how:** 308 normal tests, strict Clippy/fmt; packaged security fixtures; movement 26 and attention 26 assertions; native/Bevy side-by-side.

**Result:** Barney walks through both doors and gestures at the desk in Bevy, as in the original.

**Still broken or not tested:** FACE events, exact arrival position, the wall Combine camera, desk material, and a native rerun with the mask fix. The full regression batch was not rerun.

**Next:** FACE events, then rerun the native comparison (mask off) and compare poses.

## 2026-10-06 Trigger toucher filters

**Agent/model:** not recorded.

**Changed:** trigger_once/trigger_multiple honor client/NPC/everything toucher flags, and NPCs can activate NPC triggers. Security fixtures enable trigger_cop_close_door_1 as the campaign does earlier.

**Why:** NPC-only triggers fired for the player, and NPCs could not trigger anything, which blocks Barney's door sequence.

**Tested how:** 308 normal tests, strict Clippy/fmt; packaged movement assertions and movement/campaign pixel comparisons.

**Result:** No regression in the checked fixtures. Barney still stops at barney_door_2.

**Still broken or not tested:** Why the cop-door trigger does not let Barney through; no NPC-touch unit test; the full regression batch was not rerun.

**Next:** Instrument trigger_cop_close_door_1 bounds and door lock timing against Barney's feet, then rerun the native comparison.

## 2026-10-06 Native scene oracle, point_template spawning, prop flags and NPC doors

**Agent/model:** not recorded.

**Changed:** Private native oracle for the security scene. point_template children wait for ForceSpawn. prop_physics motion-disabled/start-asleep flags are honored. NPC routes pass through doors and open them on contact. Barney fixtures force-spawn him like the campaign trigger.

**Why:** The first native comparison showed Barney never reaching his desk in Bevy. The causes: closed doors, a desk that physics wrongly moved, and templated actors existing too early, which is also wasted simulation.

**Tested how:** 308 normal and 21 owned tests, strict Clippy/fmt; synthetic template test; owned security tests with ForceSpawn; native oracle captures; packaged security fixtures and the regression batch.

**Result:** The desk stays at its authored pose. Barney spawns on demand, opens the interrogation-room door and walks to the next door.

**Still broken or not tested:** NPC-touch triggers (trigger_cop_close_door_1 unlocks barney_door_2), so the desk walk and a side-by-side gesture comparison are unfinished. The Combine wall camera is not rendered. Repeat ForceSpawn copies, EnableMotion and door-blocked replanning edge cases are untested.

**Next:** NPC trigger touch (spawnflags 2), then rerun the native/Bevy security comparison at 18.05/22.45 s.

## 2026-10-06 Scene gesture execution

**Agent/model:** not recorded.

**Changed:** Scene GESTURE events create per-actor layers: SDK tag retiming, intensity weights, posture suppression and RemoveLayer fades. Both hosts compose base clip and layers through `Scene::actor_matrices`. Faceposer keyvalues are parsed, and the clip budget accounts for gesture children.

**Why:** Authored scene gestures were ignored, so actors only played base clips.

**Tested how:** 307 normal and 21 owned tests, strict Clippy/fmt; synthetic retiming/layer tests; owned security_02 playback with Barney's rig; packaged 1,600-tick security timeline and the regression batch (pixel comparisons, movement/weapons/attention verifiers, retained smoke).

**Result:** Barney, Kleiner and the G-Man actor now layer their authored gestures in Bevy. Existing fixtures without gestures are unchanged.

**Still broken or not tested:** Comparison against the original game, cross-scene layer priority, IK, head/facial animation and lip sync. Empty-name gestures produce no layer, and native handling of them is unverified. Posture motion uses scene movement as IsMoving.

**Next:** Capture the same security scene moments in the original game and compare poses/timing; then head pose and facial flexes.

## 2026-10-06 Shared Source animation layer composition

**Agent/model:** not recorded.

**Changed:** modkit-core composes Source sequences: delta/post layers, per-bone weights and autolayer ramps through `Rig::accumulate_pose`. source-assets keeps raw delta frames, sequence flags, bone weights, fades and named autolayers, and loads autolayer children with their parents.

**Why:** Real NPC gestures are masked parents whose children are delta layers. The old absolute delta conversion and base-clip-only path could not reproduce them.

**Tested how:** 302 normal and 20 owned tests, strict Clippy/fmt; synthetic composition tests; owned Barney g_pointRight load/compose check; retail client.dll AccumulatePose/AddSequenceLayers/SlerpBones static comparison; five packaged fixtures pixel-identical to 28c0bb3 captures, plus movement 26, weapons 17 and attention 26 assertions and a retained smoke capture.

**Result:** Composition matches the SDK and the reviewed retail code paths. Existing scenes are unchanged.

**Still broken or not tested:** Scene gestures are not executed yet. IK, local-context, world-space and pose-parameter layers, fixed-alignment slerp, 3-way blends and native runtime pose comparisons are missing.

**Next:** Scene gesture layers: faceposer tag retiming, intensity and end fades, then compose them in both hosts.

## 2026-10-06 Priority change: resume Rust rewrite, on-demand decompilation

**Agent/model:** not recorded.

**Changed:** Owner decision: pause whole-corpus decompilation coverage after pass14 and resume the Rust rewrite with authored NPC gestures. AGENTS.md, STATUS.md and docs/DESIGN.md now make decompilation feature-driven: retrieve and review the relevant retail functions from the private index before implementing each step. A drafted pass15 dispatcher-trace plan was dropped before any run.

**Why:** About 94% of baseline executable bytes already sit inside identified functions with pseudocode (133,473 addresses), but zero have reviewed names/types. Further coverage passes have diminishing returns for the campaign; reviewed semantics of specific systems are what the rewrite needs.

**Tested how:** Documentation-only change: whitelist, local-link and public snapshot checks. No runtime build/test rerun.

**Result:** Handoff documents agree on the new priority. Private research databases are unchanged.

**Still broken or not tested:** Everything listed in STATUS.md; full decompilation, names/ABIs and native parity remain unfinished.

**Next:** Identify retail animation-layer/gesture functions in the index, record reviewed findings privately, then implement raw delta/post/mask/layer readers.

## 2026-10-05 Callback discovery, ABI and switch evidence; quota handoff

**Agent/model:** not recorded.

**Changed:** Continued private research through pass14. The exact-byte index now combines13 source datasets,133,473 observed addresses with some pseudocode and133,556 export variants across42 selected modules. Saved entries and10,752 temporary discovery entries remain separate. Public changes document research; runtime code is unchanged.

**Why:** Export success, listing ownership and decompiler switch recovery do not establish complete discovery or trustworthy ABIs. Preserve failures, variants and module/hash/address identity before using native evidence in Rust.

**Tested how:** Exact database/source-row/hash audits, original project bytes, callback/disassembly rollback, private PE data/relocations,16 x87 context/recovery checks,44 existing-function switch/owner controls, ambiguity/overwrite refusal and C-only gamedb range audits. Detailed commands are in docs/validation.md.

**Result:** All research audits pass. Callback discovery adds8,992 bodies and bounded disassembly62. All2,462 disassembly addresses restore original entry/listing absence. All2,333 data records match PE bytes. Original project/catalogues remain unchanged; type/implementation/native-verification flags remain empty.

**Still broken or not tested:** Full decompilation, scope/ABI/names, native gameplay and campaign parity. The two-double-width pilot fails;45/46 orphan dispatch controls remain unmatched. Navigator omissions/range errors and rejected mixed/incomplete indexes remain recorded. Runtime tests were not rerun for documentation-only changes.

**Next:** Raw branch/table-bound/shared-tail evidence for unresolved dispatchers, x87 helper storage and runtime scope. Preserve the final private checkpoint near the owner's requested quota margin; gestures remain queued.

## 2026-10-05 Continued discovery and optional-module databases

**Agent/model:** not recorded.

**Changed:** Resumed authorized private research after the owner corrected an early stop. A private baseline clone adds 9,850 candidate exports. Seventeen optional modules expand saved coverage to 42 modules/122,721 entries with some pseudocode. Separate raw-flow/disassembly passes produce 34 and 11 temporary bodies. Public changes document evidence only.

**Why:** Whole-module export counts omitted callable entries and optional scope. Checkpoints are validation boundaries; preserve baseline provenance while continuing discovery, rather than treating a checkpoint as task completion.

**Tested how:** Original project/catalogue hashes; relocated pointer/RTTI records and bounded instruction flow; exact SQLite artifact/body audits; saved optional entry sets; five new failure recoveries/signature checks; independent rollback of temporary function/listing state; exact body retrieval and scoped gamedb control. Commands/counts are in docs/validation.md. Installed content stayed read-only.

**Result:** Discovery ledger:20,842 artifacts/9,850 bodies/zero discrepancies. Optional ledger:393 artifacts/23,987 successful body rows plus five preserved original errors/zero discrepancies. All16 known original failures have separate recovery bodies. Temporary 34/11 body ledgers and rollback checks pass. Discovery navigator has one range mismatch; optional navigator omits2,544 bodies and has779 mismatches, so exact path/hash/address retrieval remains authoritative.

**Still broken / not tested:** Complete discovery/runtime scope, original names/ABIs, exception coverage, optional activation, middleware/tool/variant exclusions and native behavior remain unresolved. Two analysis-adjusted flow attempts, initial writable-RTTI rejection, missing project-directory launch and Vulkan exception-handler analyzer error remain recorded. No Rust change, rebuild, gameplay capture, performance or multiplayer test.

**Next:** Continue optional orphan-target discovery and relocated callback/instruction-operand references across all42 snapshots, retaining versioned databases and rollback proofs. Gestures remain queued behind the documented research priority.

## 2026-10-05 Private database coverage and failed-export recovery

**Agent/model:** not recorded.

**Changed:** Continued private decompilation with a 126-path hashed PE inventory, discovery audits, three new loader-referenced module exports, eleven supplemental failed-body recoveries and a path/hash/address SQLite evidence ledger. Public changes are evidence documentation only.

**Why:** The 22-module export denominator omitted runtime modules and did not establish function discovery. Preserve original errors and uncertain signatures while improving database retrieval and recording unresolved coverage.

**Tested how:** Fresh baseline catalogue audits before/after; all 22 saved/installed/private hashes; read-only Ghidra instruction/storage/discovery inspection; eleven rollback signature checks; supplemental 301-artifact/body audit; exact VPhysics/Speex retrieval and a scoped gamedb CreateInterface control. See docs/validation.md for commands. Installed files were read-only.

**Result:** Across 25 selected modules: 88,884 identified addresses, 88,873 original bodies plus 11 supplemental recoveries. Original failure rows remain. Baseline 22-module audit finds 1,459,199 executable bytes outside functions and 13,830 unresolved pointer-slot observations. Supplement audit has zero discrepancies; fresh gamedb extension indexes 827 bodies, misses 263 and has 36 range mismatches. Native research remains outside Rust source checkouts.

**Still broken / not tested:** Complete runtime scope, discovery, recovered ABIs and native semantics remain unverified. No Rust behavior, game capture, campaign, performance or multiplayer test in this pass. Failed return-type recovery, initial debug setup, old extension-builder path rejection and rejected newline audit remain recorded privately.

**Next:** Review bounded unlabelled RTTI/vtable/orphan-code candidates and remaining optional loader branches on disposable projects, then extend a new audited database version. NPC gestures remain queued.

## 2026-10-05 Decompilation priority for the new chat

**Agent/model:** not recorded.

**Changed:** Updated AGENTS.md, STATUS.md and docs/DESIGN.md with the owner's new decompilation priority, plus a private inventory/failure checkpoint.

**Why:** Continue toward complete original-game coverage in a fresh chat while retaining the verified Rust build and precise limits of the database method.

**Tested how:** Read the existing 22-module coverage, export scripts, catalogue and gamedb audit documentation; enumerated installed DLL/EXE files read-only. Checked document consistency, Git diff and the public snapshot.

**Result:** Handoff records 87,783 successful exports/ten failures, exact-byte catalogue coverage, gamedb omissions/read-range errors and 126 installed DLL/EXE paths awaiting scope/dependency classification. The plan separates inventory, failure recovery, discovery, type/behavior recovery and verified Rust translation.

**Still broken / not tested:** No new Ghidra run, recovery/export, database rebuild or runtime modification occurred. Existing audit reports were inspected, not rerun. Complete decompilation, function discovery and semantic parity remain unverified; the failed exports and parser discrepancies remain open.

**Next:** The owner starts a fresh chat in outputs/hl2-rs-bevy. Read AGENTS.md, STATUS.md, docs/DESIGN.md and the newest private checkpoint, then execute the bounded decompilation plan before queued NPC gesture work.

## 2026-10-05 Fresh-chat documentation handoff

**Agent/model:** not recorded.

**Changed:** Reworked AGENTS.md, added STATUS.md and docs/DESIGN.md, updated this log and changed .gitignore to a source-only whitelist.

**Why:** Preserve verified state, existing authorization and failed approaches without rereading the entire chat.

**Tested how:** Reviewed the three templates against repository instructions, Git revisions, build metadata, recorded logs and the private checkpoint. Checked links, whitelist coverage/exclusions and the staged public snapshot.

**Result:** Handoff identifies active branches, both release packages, tested capabilities, unresolved fidelity work and the next bounded gesture plan. Historical entries remain available.

**Still broken / not tested:** Runtime code is unchanged; gameplay/build checks belong to28c0bb3 and are not rerun claims for this documentation edit. Full campaign/1:1 parity remains unfinished.

**Next:** Start a fresh chat in the Bevy checkout; read AGENTS.md, STATUS.md and docs/DESIGN.md before continuing.

## 2026-10-05 Native crosshairs and optional spatial batches —28c0bb3

**Agent/model:** not recorded.

**Changed:** Shared owned Windows glyph rasterization/placement, Bevy1080p/borderless launchers, optional spatial batches/render-candidate diagnostics and targeted retained build tooling.

**Why:** Correct blur/fractional reticles and investigate broad world batches without changing physics or forcing equal native sizes.

**Tested how:** Ran original HL2 at1080p,294 unit tests/19 owned checks, strict Clippy/fmt, packaged captures/replays and source audit.

**Result:** Seven Bevy captures and retained1080 captures match dot offsets: white22x22, pistol/SMG21x17. Movement26, weapons17, monitor/campaign18 and accepted images9 pass. Plaza/spawn partition images are identical; audit212 files/0 failures.

**Still broken / not tested:** Splitting stays opt-in; hall camera was rejected, frozen benchmark script was incomplete and25 known station03 material errors remain. Other fonts/colors/platforms and full campaign parity are not established.

**Next:** Preserve verified reticles and reconstruct authored NPC gestures.

## 2026-10-05 Shared materials and model activity —c2b5220 /322afaa

**Agent/model:** not recorded.

**Changed:** Shared authored material/lightmap handles and avoided unchanged GPU uploads; added owned label/activity/weight resolution and corrected unsupported Kleiner idle.

**Why:** Reduce preparation overhead and use model-authored animations instead of assumed labels.

**Tested how:** Unit/owned checks, controlled Kleiner capture, movement/weapon replays, matched actor/eye/monitor comparisons and local1080 profiles.

**Result:** Station02 material handles921→743. Kleiner resolves idle_subtle; valid metrocop idle_baton stays. Material package measured174.72FPS at the tested spawn, with limited scope.

**Still broken / not tested:** Native RNG/modifiers/full virtual IDs, gestures/facial animation and natural staging remain unfinished. Global eye reports differ after the intervening idle fix.

**Next:** Preserve raw delta/post transforms, masks and child dependencies before gestures.

## 2026-10-05 GPU skinning and cached BSP PVS —df6af1b /b888144

**Agent/model:** not recorded.

**Changed:** GPU skinning/current bounds/shared iris poses; conservative PVS across active player/monitor views with script visibility/fail-open behavior.

**Why:** Profiling found CPU skinning/uploads and preparation of out-of-view geometry.

**Tested how:** Synthetic/owned bounds/visibility checks, packaged actor/eye/monitor/campaign/gameplay fixtures and900-frame1080 profiles.

**Result:** Selected actor/eye comparisons preserve tested scope. CPU→GPU35.05→66.85FPS and PVS-off→on66.69→156.94 are separate local comparisons; simulation remains enabled.

**Still broken / not tested:** Area portals/occluders/LODs, complete animation and whole-campaign/native144FPS parity remain unverified.

**Next:** Preserve current bounds/view-union ordering and expand fidelity with the same regressions.

## 2026-10-05 - animated attention attachments

**Agent/model:** not recorded.

Actor gaze origin/forward now come from the owned animated eyes attachment, with model view-offset fallback. Eye presentation caches one skin pose per visible actor and shares it with iris projection, preserving cycler meshes. Malformed/singular/overflowing attachment records are rejected; PVS latching, native head-pose controls and full layered animation remain unfinished.


## 2026-10-05 - authored scene attention

**Agent/model:** not recorded.

Shared simulation now executes compiled LOOKAT events with normalized-time scene/event ramps, timed deduplicated interests, target aliases, pause refresh and cancellation expiry. Bevy projects both eyes toward one selected actor target; monitor/player/self behavior is capture-verified in a deliberately seeded security02 fixture. Retained simulation/reporting consumes the same queue. Head/facial controls and native random/tactical attention remain unfinished; this is not campaign or retail AI parity.


## 2026-10-05: shared pause/console and Bevy campaign host

**Agent/model:** not recorded.

Moved the retained console parser/history/cheat gates and resource-driven pause layout into hl2-ui. Both hosts now provide explicit input and consume the same canvas. Bevy opens Resume/Console/Quit, supports the existing command subset and preserves selection/capture/held-input ordering. Console rendering clips output to its panel and handles Unicode character boundaries.

Added asynchronous owned-map decoding with simulation/script clocks frozen while loading. Successful transitions replace map-owned draws/audio and carry landmark-relative eye position/inventory with rebased weapon deadlines; direct map starts fresh. Missing maps preserve the current level. Testing exposed repeated same-map loads from input-only changelevel brushes; shared touch handling now honors their 0x2 flag while retaining explicit ChangeLevel input. Full saved entity/global/player state and ordinary campaign completion remain unfinished.

267 normal tests, thirteen owned-install tests and strict Clippy pass. Packaged console/campaign captures pass 45 checks, including 720p/borderless1080p UI, actual paused audio sinks, two real station exit transitions, failure recovery and camera cleanup. Retained-host and weapon/movement regressions are recorded in validation.md. No game files or native research are published.


## 2026-10-05: shared effects presented in Bevy

**Agent/model:** not recorded.

Extracted retained projectile billboard/RNG/blur and impact selection/clipping into hl2-simulation, leaving rendering adapters in each host. Bevy now presents owned SMG grenades, energy balls, impact/explosion sprites and bullet marks with depth testing. Decals retain destination-color blending and doubled modulation, and follow moving receiver transforms. Testing found a retained door-mark failure: projection used raw MDL vertices instead of the animated idle pose and rejected the one-unit collision/visual gap. Both hosts now project onto the current posed mesh with a bounded same-receiver correction. Native studio deformation and complete particles remain unfinished.

Packaged effect fixtures pass 40 checks, including real observed draws, frozen simulation, secondary reserves, explosion completion and marked-door transform agreement. The full combined weapon fixture and retained packaged smoke are checked separately. Source assets stay installed and private capture evidence stays ignored.

## 2026-10-05: shared HUD presentation in Bevy

**Agent/model:** not recorded.

Moved the retained owned-resource HUD into engine-independent hl2-ui with an explicit ordered CPU canvas. Both renderers use the same layout/font/crosshair/animation logic. Bevy now draws health/ammo, weapon buckets, quick-info and secondary ammo through a separate overlay camera with normal/additive materials. Asset loading stays outside systems; glyph textures and meshes are reused.

Packaged white-crosshair captures match the accepted retail five-pixel positions at 720/1080. Selection preserves active ammo, secondary-panel motion is captured at start/intermediate/end, and the door/weapon and bench fixtures remain passing. Native font rasterization and blend/gamma equivalence are still partial. Audio, pause/console, projectile/impact presentation and campaign host migration remain open.


## 2026-10-04: Bevy entity, weapon and animated presentation bridge

**Agent/model:** not recorded.

Moved the tested entities/gameplay/NPC/projectile/selection implementations into hl2-simulation and retained host re-exports. Shared actor/viewmodel preparation preserves clip loading. Bevy now runs scene, weapon, moving collider, NPC/projectile, rigid-body and player state in retained order. Local entity meshes follow authoritative poses/visibility; CPU skeletal animation updates bounds; an independent viewmodel pass preserves the existing projection. F3, weapon buckets/wheel, confirming/fire/reload/previous, E use and G test impulse are connected. Quick-click and pause/selection suppression regressions pass. Packaged bench and station-door/secondary-weapon fixtures pass; HUD/audio/effects/campaign host migration remains next.


## 2026-10-04: shared collision and Bevy player movement

**Agent/model:** not recorded.

Extracted the existing collision/Rapier adapter, convex sweeps and NPC probes into `hl2-simulation` unchanged, preserving their 25 tests and retained-runtime imports. Bevy now uses the same Source-coordinate player and collision code at 15 ms per step, with input/look before simulation and camera presentation afterward. Default walking, flight toggle and pause/resume are supported; input capture consumes transition-frame mouse movement and held jump until release.

The packaged bench fixture passes 26 movement/pause assertions, including the native collision height, one-time air crouch lift with retained eye/momentum, air uncrouching and jump rearming. Rendering remains static and rigid bodies are frozen until presentation is synchronized. Weapons/HUD, pause UI/console, entity I/O/scenes, NPC animation/AI and audio remain migration work. See `docs/validation.md` for package and test evidence.

## 2026-10-04: separate Bevy/wgpu host preview

**Agent/model:** not recorded.

Preserved the original host on `macroquad-prototype` and added `bevy-migration` for the long-term Bevy/wgpu work. Main keeps the retained runtime until verified replacements are available. README and contributor guidance identify the branch and executable each feature belongs to.

The new host reads owned maps through the shared format/core crates and renders static BSP, displacement, prop and entity geometry with custom base/lightmap materials and a fly camera. Shared CPU/GPU texture caching and visible-material filtering correct the first preview's loader-budget exhaustion. BSP render winding is normalized before appending already-normalized models, correcting culled model fronts without altering shared collision data.

Validation: 241 normal tests and nine owned-install checks pass, with formatting and strict Clippy. Final packaged station-map captures at 720p and borderless 1080p were inspected, with no texture-budget or capture failures. Dynamic monitor and eye materials remain unsupported. Gameplay, collision, animation, AI, choreography, HUD and audio have not migrated; Source rendering and performance parity are not established. See `docs/bevy-migration.md` and `docs/validation.md`.

## 2026-10-04: G-Man speech and authored locomotion data

**Agent/model:** not recorded.

Fixed the first-map G-Man omission with explicit `cycler_actor` model and scene-actor support, corroborated by retail factory/RTTI and actor lookup. The packaged debug-camera fixture resolves 17 authored intro events. Its initial run exposed compressed voice failures; standard Microsoft ADPCM now decodes in memory, preserves recorded frame counts and successfully requests playback of both opening lines. Facial animation, gestures, intro cameras and compositing remain unfinished.

Added bounded PC AIN37 decoding and optional inspect/runtime reports, preserving hull offsets, raw masks/metadata and Hammer IDs. Added v48 authored movement records and activity/all-blend metadata, plus piecewise motion sampling, turning and positive/negative loops. Owned walk/run tracks match 80 units/1s and 125.87412 units/0.6s. These readers do not yet implement NPC pathfinding, weighted blending, motor planning or movement readiness.

Validation: 210 workspace tests, six owned-install checks, formatting and strict Clippy pass. The rebuilt launcher package passes 12 G-Man, 33 scene-controller/sequence and 20 secondary-projectile assertions. A graph census decodes 72 and rejects six mismatched revisions across 78 loadable installed maps; the known empty coast map remains rejected. Private evidence and game files remain outside public source.

## 2026-10-04: crosshair, secondary projectiles and authored scenes

**Agent/model:** not recorded.

Corrected the oversized unarmed crosshair by identifying the native height accessor and reproducing both texture-coordinate insets. Packaged720/1080 captures now match the original five pixel positions; odd viewport rounding also passes. Font rasterization and tone mapping remain separate work.

Added real SMG contact grenades and AR2 charging balls, separate cooldowns/reserves, reload interruption/veto, continuous collision queries, bounded blast obstruction/damage, bounce/expiry and owned effects. Native controls corroborate selected weapon-state behavior. Rapier physics, damage policy and effects still do not reproduce the full Source implementation.

Added a bounded Rust reader for compiled choreography and authored scene control/triggers/completion. Selected installed SEQUENCE clips follow the paused scene clock and restore their baseline. Unsupported movement readiness holds SECTION rather than inventing success. The first level still lacks NPC schedules/movement, gesture/facial layers and intro systems needed for complete playback.

MP3 cues decode into an in-memory PCM cache, resolving the trainstation music gap. Added bounded cheat-gated ent_fire through normal entity I/O and more detailed snapshots.

Validation:192 workspace tests, both owned-install checks, strict Clippy and formatting pass. The latest package passes27 existing weapon checks,20 projectile checks and33 first-map scene-controller/sample checks. Captures and limitations are recorded in docs/validation.md; private research and installed assets remain excluded from Git.

## 2026-10-03: air crouch, stable contacts and pause/console

**Agent/model:** not recorded.

Air crouching now tucks the feet once while preserving head height and momentum; standing clearance gates air unducking. Reviewed retail sliding rules and analytical world-brush/convex sweeps correct reproduced wall-contact jump interruptions. Native PHY compound integration and full VPhysics parity remain unfinished.

Added a working resource-based pause menu and bounded developer console with cheat gates, history/editing/completion, position/angle commands and owned map loading. Focus freezes simulation and suppresses attack/jump leakage. The menu uses the owned title font and calibrated 720p metrics. Full console, save/options and paused audio remain open.

139 tests and strict Clippy pass; packaged weapon, movement and console fixtures pass their meaningful assertions. Latest double-door collision and secondary-ammo HUD reports are queued for the next iteration. Private bench PHY research matches the measured original standing height but is not integrated into Rust yet.

## 2026-10-03: publication and automatic empty fire

**Agent/model:** not recorded.

Published the reviewed Rust source to kvalls/hl2-rs with fresh-checkout instructions, contribution boundaries and a labeled runtime screenshot. The public main branch starts from a clean source snapshot; older local research history stays private. Windows formatting, Clippy and unit checks run for main pushes and contributor pull requests.

SMG1/AR2 now use a reviewed empty-fire latch and per-weapon half-second sound throttle. Empty clicks retain the primary deadline and animation; the next eligible attempt reloads, and idle reload uses a strictly elapsed primary deadline. Four regression tests cover held/released input, throttle boundaries, independent weapon state and reload completion. Other secondary attacks, autoswitch ranking and custom reload flags remain incomplete.

## 2026-10-03: depth, secondary fire and retail input/movement

**Agent/model:** not recorded.

Separated OpenGL depth testing and writes in the vendored backend, preserving depth clears between sky/world/viewmodel passes. Matching trainstation views reproduce then remove hidden light shafts, entrance columns and barrier effects; a front-side capture retains the visible columns.

Added shotgun alternate fire with twelve pellets/two shells, one-shell primary fallback, native/script-derived pump deadlines and retained secondary reload interruption. Accepted health/battery pickups now use their installed item sounds. The original engine confirms a six-to-four-shell alternate discharge.

Selection now consumes already-held attack buttons when Slot/Wheel opens the menu, rearms each button independently, and observes ordered script release/repress transitions. The F1 overlay identifies the Rust runtime. Movement now preserves standing/duck jump gravity ordering, categorizes after the sweep, crops diagonal boost, permits negative speed-cap additions and retains the native next-tick rising-air friction factor.

Fresh-clone build instructions and contribution boundaries were added for public source sharing. Game assets, native analysis, captures and research tools remain separate. Full campaign, NPC AI and Source rendering/collision parity remain unfinished; docs/validation.md records the checks and package fingerprint.

## 2026-10-02

**Agent/model:** not recorded.

The original executable remains read-only, with the earlier save/config snapshots preserved. Work is focused on HL2 fidelity; no FAL assets or crossovers were added.

The first prototype supported map/prop display. This iteration adds brush submodels, displacement/prop collision, fixed-step movement, timed entity I/O and doors, primary lightmap atlases, material transparency/tint/two-texture scrolling, compressed skeletal animation, PCM WAV audio, crowbar/pistol gameplay and limited scripted sequences/map transitions. These are partial implementations; the README lists the missing systems.

Mouse-look uses macroquad's previous-minus-current delta: upward movement increases pitch, rightward movement decreases Source yaw. The Windows miniquad reader additionally converts absolute RAWINPUT positions into deltas; treating those positions as motion caused extreme jumps during automation. Interactive verification is recorded in docs/validation.md.

Regression tests cover ordered/delayed duplicate outputs and fire limits, door locking/completion, shuffle batches, post-idle completion timing, ammunition transfer, stationary hull overlap at a floor, and disabling killed dynamic colliders. Movement and animation tests cover deterministic replay, acceleration/friction, jump apex and hierarchy/interpolation.

Research tools and private decompiler output belong in ../../work/hl2-decompiled. They are development references only. The runtime builds from Rust and reads owned assets in place. The packaged executable, rather than target/debug, is the final launch.cmd validation target.


### 2026-10-02: weapon selector, impacts and separate sky world

**Agent/model:** not recorded.

- Preserve nested sound-script wave alternatives; use distinct crowbar flesh/world sounds and hit animation.
- Add receiver-clipped installed impact textures and inherited surface-property bullet sounds. Marks follow rigid props and brush entities; keep a bounded local pool.
- Implement six-bucket UI with installed weapon icons, wheel cycling, confirmation, cancellation and previous-weapon switching; cancel reload on switch. Only crowbar and pistol combat are implemented.
- Separate background geometry/props using BSP leaf/PVS data and sky-camera scale. No native game code or decompiler tooling entered this project.
- Validation: 36/36 unit tests and workspace Clippy pass; packaged interactive run recorded three hit decals and multiple metal-impact sound variants with zero audio errors. Wheel UI rendered correctly; live confirmation needs a faster-input retest because its timeout expired between automation calls. Installed maps 78/79 parse with only the known empty coast map failure.

### 2026-10-02: retail HUD, six primary weapons and sky visibility

**Agent/model:** not recorded.

The earlier selector was a provisional design. This pass replaces it with installed resource dimensions, fonts/cell metrics, corners, labels and input behavior, checked against selected retail methods and a localized original-engine capture. Numeric HUD animation rules, bounded low-health loops and QuickInfo progress/fades/warning audio now run. Native GDI rasterization, damage messages and the remaining HUD panels/gates are still incomplete.

Added .357, SMG1, AR2 and shotgun primaries, separate ammo reserves, native/script-derived cadence/spread, shotgun shell/pump/interruption behavior and installed model sound events. Crowbar collision now tries the full ray, shortened +/-16 hull, facing test and closest corner refinement over approximate Rapier geometry. The background renderer now checks the camera BSP leaf's 3D-sky flag.

Validation: 79/79 workspace tests, strict Clippy, formatting and diff checks pass. Packaged input regression records 64 attacks, 82 impacts/marks, 6 model sound events, 199 audio requests, zero audio errors, two crowbar impact variants and one low-ammo warning. Actual Computer Use wheel/left/right/Q/Escape input verified selection with zero shots. A 60-frame hidden-sky test recorded zero sky frames. Installed maps remain 78/79, with the existing empty coast map rejected. See docs/validation.md for artifacts, current package fingerprint and limits.

Original-game research writes, localization copy, inspection tools and native pseudocode stay outside the Rust project. The original test process was closed, stock cfg manifest unchanged, no native DLL patched and no FAL/crossover used. Full NPC/campaign/shader fidelity remains substantial work; parser coverage is not campaign completion.

### LDR sky asset groundwork

**Agent/model:** not recorded.

Added a bounded six-face LDR sky loader resolving VMT base textures and transforms, with eight synthetic tests. HDR rendering, cube drawing and leaf gating integration remain unfinished. All 109 workspace tests and strict Clippy passed; the final package was rebuilt and smoke-tested.

### 2026-10-03: database audit, sky rendering and movement collision

**Agent/model:** not recorded.

Integrated the six-face LDR sky background with verified retail orientation, installed material transforms, clamped sampling and separate BSP 2D/3D eligibility. It draws before scenery/world without depth writes. Matched original-engine views corroborate cloud orientation; HDR, fog and sky polygon masks remain unfinished.

Player collision now excludes retained NPC-only world clip brushes, correcting the oversized invisible station-bench blockers. Grounded crouch commands and maximum speed are separated for jump boost; airborne crouch retains full air acceleration. Regression checks cover clip masks, duck boost caps, backward overspeed, air strafing and chained released jumps. Native PHY geometry and full movement equivalence remain open.

The supplied Database Method was tested privately across all exports. Unchanged gamedb misses functions and some boundaries/call targets, so an exact-byte catalogue keyed by module/address preserves the complete export inventory and known failures alongside the supplemental navigator. This improves traceability without marking semantic parity automatically. The original game also confirmed three-shell shotgun secondary behavior: double discharge leaves one shell, and the next secondary action fires it as a single shot. See research and validation for measured coverage and packaged checks.

### 2026-10-03: air crouch, console, static PHY and secondary HUD

**Agent/model:** not recorded.

Air crouching preserves head height with a one-time foot lift, and stable world/convex hull sweeps correct reproduced wall-jump contacts. Added a resource-driven pause menu and bounded console with cheat gates, history, completion and map loading. Original ground-duck timers, prediction and full Source command/UI coverage remain unfinished.

Added a bounded original Rust PHY reader and separate installed convex pieces for supported solid static props. Bench standing height now agrees with the owned original measurement. Rotating-door collision applies the visible model's initial idle pose, fixing the station entrance's closed-door walkthrough and invisible open-shaped blocker.

Primary ammo stays visible during weapon selection. Unarmed crosshairs use the installed white default sprite; armed crosshairs retain installed glyphs. SMG/AR2 secondary reserves, carry limits, pickups and ALT counters now work. Their actual grenade/energy-ball attacks and the remaining weapons are still missing. See validation for package fingerprints, tests and the known trainstation MP3 music gap.

### 2026-10-04: Barney scene movement and actor-aware speech

**Agent/model:** not recorded.

Added Barney's native default model and owned MDL eye metadata, dedicated NPC collision masks/probes, bounded human ground routes and authored central walk/run locomotion. Scene MOVETO requests now persist while SECTION waits for actual arrival; the controller continues while only the scene clock is paused. UI pause freezes both, blocked routes hold the gate, and cancel/script ownership prevents stale pose changes. The controlled security03 fixture verifies player blocking, arrival-driven door output and cancellation without a manual scene Resume. It explicitly seeds a preceding authored target and is not a complete campaign replay.

Symbolic speech carries the resolved actor model through the installed gender registry and retains native tagged wave/fallback behavior. The first-map owned census resolves all 72 actors and decodes all 75 registered alternatives. Native RNG/mixing, localized combined lines and lip sync remain open. See validation for tests and packaged checks; native motor timing, weighted blends, general NPC AI, facial/eye rendering and the playable first level remain unfinished.

### Bevy audio adapter

**Agent/model:** not recorded.

Shared owned sound-script selection, actor gender, WAV/MS ADPCM/MP3 decoding and viewmodel event cursor now serve both hosts. Bevy preloads references outside systems and plays ambient/scene/weapon/HUD requests through monitored audio sinks, including host pause/resume. Packaged door/weapon and controlled scene fixtures, full tests and strict Clippy passed. Source spatial audio/DSP, soundscapes and lipsync remain unfinished; this does not complete the campaign.

### Bevy sky, iris projection and prop door swing

**Agent/model:** not recorded.

Restored owned LDR cube/miniature sky passes before the playable world, with current-leaf visibility and depth occlusion. Eyes now use authored studio metadata and separate iris textures; gaze uses a bounded visible player/NPC approximation. EyeRefract uses its owned Eyes_dx8 fallback, with refraction/flex/glints still missing. Prop door use carries the opener position to linked leaves and respects explicit swing direction, fixing the opposite inward/outward entrance behavior. Native door blockers and full gaze/choreography logic remain unfinished.

## Deferred prop-door opener inputs

**Agent/model:** not recorded.

Added shared OpenAwayFrom target resolution, preserved locks/fixed directions and stopped repeated opening inputs from resetting the swing. Two regression tests and an owned packaged entrance fixture cover named/current origins, player/caller/activator lookup and both sides. Full native door linkage/blocking remains unfinished.
