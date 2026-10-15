# AGENTS.md

## Project

HL2-RS is an experimental Rust rewrite of Half-Life 2 and relevant Source behavior, using content from an owned installation. It is not a passthrough mod and does not load Valve game/engine DLLs. Complete campaign and 1:1 fidelity have not been achieved.

## Hard rules, never break these

1. Keep game assets, native binaries, decompiled code, databases and research tools outside public source checkouts. Read installed content in place; binary analysis uses private copies. Generated screenshots/reports belong in ignored artifacts/.
2. Keep .gitignore as a source-only whitelist. Add narrow exceptions for reviewed source, documentation, fixtures or explicitly licensed project resources. Never force-add extracted game content or generated executables.
3. Commit/push only authorized work and stage only the task's files. The owner already authorized reviewed source commits/pushes to kvalls/hl2-rs, including milestones and checkpoints near 98% of the five-hour quota. Do not repeatedly ask. Work on main (Bevy-only since 2026-10-07); keep experimental or unverified work on separate branches until it is validated.
4. Keep the installed game read-only. Approved private working directories are workspace work/hl2-decompiled, work/references and work/publishing. Other unrelated files remain outside scope unless named by the owner.
5. Do not put credentials, tokens or passwords in project files, logs or commits.
6. No FAL, replacement assets, crossovers or unsolicited Discord messages. This phase reconstructs HL2. Mods and crossovers with other Rust game rewrites are the long-term goal after 1:1 fidelity (owner, 2026-10-07); keep `modkit-core::ModPlugin` and shared crates mod-friendly. There is no single-player/offline-only project restriction; multiplayer/netcode fidelity is not implemented or tested.
7. Fonts and UI typography must come strictly from the owned game installation VFS (in-place reading of installed `resource/*.ttf`, `platform/resource/linux_fonts/*.ttf`) or permissively licensed project assets (MIT `ProggyClean.ttf`). Never probe, read, bundle, or depend on proprietary host OS font files (`C:/Windows/Fonts`, `/System/Library/Fonts`, `/Library/Fonts`). Everything must be 100% legally clean and safe.
8. User-visible UI strings must come strictly from localized game resources (`resource/gameui_english.txt`, `resource/valve_english.txt`, `resource/vgui_english.txt`, `hl2_complete/resource/merged_loc_all.txt`, etc.). Never hardcode user-visible English literals in UI or HUD drawing routines. Authentic English fallback values are permitted only when game resources are missing or the key cannot be resolved, and must be defined as named constants in the centralized fallback table (`hl2-ui::gameui::localize::FALLBACK_STRINGS`), never as inline literals.
9. Third-Party Dependencies (Bevy, Winit, etc.): It is strictly forbidden to attempt fixing 3rd party dependencies like Bevy and Winit from their end (patching/modifying upstream crate sources). You can analyze their code, but only after failing to find any helpful information about said 3rd party dependency (from both documentation/web and our codebase). All adaptations, translations, and coordinate corrections must be implemented cleanly on the HL2RS side.

## How to work

- Read STATUS.md, README.md, docs/DESIGN.md, docs/research.md and docs/bevy-migration.md first. Machine-local detail is in workspace work/publishing/continuation.md; its newest checkpoint supersedes history.
- Current priority (owner decision 2026-10-06): the Rust rewrite resumes with authored NPC gestures. Whole-corpus coverage/discovery passes are paused after pass14. Decompile on demand: for each feature, retrieve the relevant native functions from the private pass14 index by path/hash/address, review them privately and record findings before implementing. Preserve the baseline catalogue; indexing or relaxed-metadata exports do not establish semantic parity.
- Shared work: STATUS.md "Claims" lists items owned by other contributors (GitHub issue assignments and open PRs). Before starting an item, check open issues/PRs and that list; skip claimed items and avoid editing the files they touch. Ask the owner before claiming new items publicly.
- For substantial changes, record a bounded design/validation plan in docs/DESIGN.md before coding. Keep steps independently reviewable and reversible.
- Use instrumentation and visual evidence together. The owner explicitly authorizes testing, screenshots and computer use. Record positions, timings, state transitions and image comparisons. Test packaged executables. Do not close user-launched games or send input to unrelated apps.
- Reuse shared implementations: source-assets reads formats; modkit-core owns contracts/player/pose math; hl2-simulation owns shared gameplay/physics; hl2-ui owns portable HUD/menu/console; hl2-bevy is the host adapter. No asset reads/font rasterization in frame systems.
- HL2-RS is a Bevy Rust rewrite. Owner decision 2026-10-07: bevy-migration merged into main and the former Macroquad host (hl2-runtime) was removed; continue on main. Ask before materially different large refactors. Preserve the macroquad-prototype (historical host), bevy-migration (frozen at the merge) and unverified wip/scripted-scenes branches.
- Preserve 15 ms order: sample input first; scene -> weapons -> entity collider poses/query refresh -> NPC/projectiles -> rigid physics -> player. Publish presentation before transform propagation and current animation bounds before visibility. Preserve held input, pause and selection semantics.
- Build with scripts/build-bevy.ps1 and run launch-bevy.cmd (or the 1080p/borderless variants). Keep generated executable/shader metadata current after runtime changes.
- Use Cargo.lock. Run relevant tests, strict Clippy and formatting; verify parsers against owned files and rendering through packaged captures. Give exact test commands and expected results. Documentation-only edits require document/whitelist checks, not gameplay rebuilds.
- Owned-install tests (`#[ignore]`, run with `cargo test -- --ignored`) locate the game with `source_assets::install::discover()` exactly like the runtime (HL2_ROOT first, then Steam libraries); never require HL2_ROOT in a test.
- Multi-platform: Windows is the primary tested platform, macOS a contributor platform. Platform-specific retail restrictions (e.g. macOS has no "Windowed (No Border)", `[$OSX]` `.res` conditionals) are platform-gated, never applied globally; borderless stays supported on Windows/Linux, and stored settings are not rewritten just because the current platform lacks them.
- Research database/native exports are evidence, not automatic parity. Distinguish inferred names/signatures, SDK behavior and verified retail behavior. Seeded developer actors do not establish ordinary campaign completion.
- Explain changes plainly: actual behavior, verification, limitations and next bounded step.
- When hitting sandbox or offline environment barriers or fundamental architectural choices, ask the owner for guidance promptly rather than looping on inaccessible files or retrying blocked commands.
- Resolution scaling: distinguish window presentation dimensions from internal 3D render target resolution (rendering at a configured lower resolution such as 720p and upscaling/filtering to a higher-resolution fullscreen display for performance).
- Feature fidelity: prioritize verified behavior of the owned retail game unless the owner explicitly requests a deviation. Consult official SDK references within their license terms and, where legally permitted, targeted private binary analysis. Implement behavior independently in Rust. Never use leaked source or copy or mechanically translate Valve implementation code into the project. Keep native analysis and game assets outside public source checkouts.

## Honesty

- Write **not tested** when applicable. Distinguish unit tests, owned-file checks, controlled replays, native comparisons and ordinary playthroughs.
- After two unsuccessful real attempts, stop repeating that approach and record evidence in STATUS.md. Choose a materially different evidence-based approach; continue independent work rather than retrying random variations.
- Record failures/rejected captures alongside successes. Never claim complete decompilation, 1:1 fidelity or FPS guarantees without verification.

## Keep these files updated

- MODLOG.md: newest first; Agent/model / Changed / Why / Tested how / Result / Still broken or not tested / Next. Record the agent application and exact model identifier when available; distinguish contributor work and materially involved subagents. Never guess a model from the date or the current chat. Use "not recorded" for historical entries without evidence, and state when only a model family is known.
- STATUS.md: verified revision/builds, current problems, evidence, rejected approaches, reproduction commands and next work.
- docs/DESIGN.md: architecture and substantial plans. README.md: tested capabilities/limitations. docs/validation.md and docs/performance.md: detailed evidence.
- Private work/publishing/continuation.md: precise local checkpoint before quota exhaustion or handoff. When the owner requests a fresh chat, finish the handoff and await further instructions here instead of continuing implementation in this chat.

## Environment

- Windows 11 Pro 64-bit, observed version 10.0.26200, is the primary tested platform. macOS Apple Silicon (aarch64, Metal) was tested by a contributor (@OrbitalMan, PR #1: build, tests and the packaged build running d1_trainstation_02 with its scripted dispenser, Metrocop and queue sequences on an M1 Pro); other platforms **not tested**.
- Owned Steam Half-Life 2 app220, tested build19307283/patch9912070; other versions **not tested**. Default installation: C:/Program Files (x86)/Steam/steamapps/common/Half-Life 2; discover another library or use HL2_ROOT.
- Rust1.99.0 stable MSVC, Visual Studio C++ tools/Windows SDK. Bevy0.19.1/wgpu and direct Rapier0.26.1. Switching to Avian was not requested; comparative physics performance is not tested.
- Loader: none. Game owning the player: not applicable; Rust simulation owns state and the installation supplies content.
- Agent: Claude Code desktop (2026-10-06/07 sessions, Claude Opus 5.5); earlier runs used Codex Desktop with GPT-6.1 Extra High/Normal Speed. Select new-chat settings explicitly.
- Active checkout: workspace outputs/hl2-rs-bevy on main. Sibling outputs/hl2-rs is a detached historical checkout of the former Macroquad main (b4b1731).

## GameUI, Localization & Resource Conventions

- **UI Strings & Localization Fallback**:
  - All user-visible strings across UI, HUD, and console MUST resolve via `Localization::text("#Token")` from mounted game resources (`resource/gameui_english.txt`, `resource/valve_english.txt`, `resource/vgui_english.txt`, `hl2_complete/resource/merged_loc_all.txt`, etc.).
  - Never introduce hardcoded English literals in UI or HUD drawing code. If game resources are missing or a key is unresolvable, fallback strings MUST be defined as named constants in `hl2-ui::gameui::localize::FALLBACK_STRINGS`, never as inline literals.
  - Localization files in retail contain authentic escape sequences (`\n`, `\t`, `\"`, `\\`) across multiple tokens. Escape sequences MUST be handled strictly at the resource parser/insertion boundary (`Localization::add_resource` and `Localization::insert`). Never duplicate unescape logic at runtime lookup sites, and never dynamically unescape arbitrary non-token text.
  - Verify compliance with the audit test: `cargo test -p hl2-ui audit_no_hardcoded_user_visible_strings_in_ui_code_outside_fallback_constants`.

- **Text Encodings & BOM Handling**:
  - Retail resource files use diverse text encodings: UTF-8 with BOM (`[0xef, 0xbb, 0xbf]`), UTF-16 LE with BOM (`[0xff, 0xfe]`), UTF-16 LE without BOM heuristic, and Windows-1252 / ISO-8859-1 for 8-bit text (e.g. en-dash `\x96` in `gameinfo.txt`, copyright `\xa9`).
  - Never call `String::from_utf8_lossy` directly on VFS text files; always decode via `source_assets::keyvalues::decode_text`.
  - Strip leading BOM bytes during tokenization so zero-width no-break spaces (`\u{feff}`) never pollute token keys or values.
  - Submenu subtitle lines (`title2`, e.g. `== episode one==`, `== episode two ==`, `lost coasT` across `episodic`, `ep2`, `lostcoast`) render using the authentic Source title fonts (`HL2EP2.ttf` for Episode One/Two, `lostcoast.ttf` for Lost Coast). In `HL2EP2.ttf` (and `hl2ep1.ttf`), the `'='` character maps to GID 17: a wide horizontal bar decoration rule, NOT literal hyphens `---`. Subtitle decorations (`–`, `—`, `---`, `--`) normalize to `==` at the parser boundary so they cleanly map to the horizontal rule glyph. Subtitles render at `geometry.title_tall` with `ClientTitleFont` color `rgba(255, 255, 255, 200)` and letter edge contour outline (`TextOutline::DEFAULT`), falling back to `menu_font` at `geometry.title_tall * 0.55` only if a title font lacks glyphs.
  - Subtitle layout coordinates (`Main.Title2.X`, `Main.Title2.Y`) derive from `hl2_complete/resource/clientscheme.res` (`Title2.X == Menu.X == 76`, `Title2.Y == 172`), aligning horizontally with the menu button column across all scales (e.g. 76px at 1.0x, 152px at 2.0x, 126px at 1280x800). If a legacy 2004 `ClientScheme.res` is encountered with the old logo-suffix coordinate (`Main.Title2.X = 291` for the numeral '2'), `MenuLayout::apply_client_scheme` normalizes `title2_x` to `menu_x` (and `title2_y` to `DEFAULT_TITLE2_Y = 172.0`) so subtitles are never shifted across the screen. Scheme loading in `Console` prioritizes `hl2_complete` via `Reader::new(vfs)`.

- **VFS Search Path Layering**:
  - Source search paths are strictly layered: an expansion mount (`episodic`, `ep2`, `lostcoast`) must check its loose directory AND its VPK archives together before falling through to base `hl2` (`SearchPath { root, archives }`).
  - Never separate loose directory checks from VPK checks globally; doing so causes loose base `hl2` files (like `hl2/scripts/chapterbackgrounds.txt`) to shadow expansion VPK archives (`episodic/ep1_pak_dir.vpk`).
  - When an expansion map shares a filename with a base game map (e.g. Lost Coast's `lostcoast/maps/background01.bsp` vs `hl2/maps/background01.bsp`), canonicalize internal resource keys (`"lostcoast_background01"`) while translating back to on-disk map names during asset loading.

- **Widget Interaction & Audio**:
  - Only main menu and submenu navigation buttons play the button rollover sound (`ui/buttonrollover.wav`).
  - Dialog push buttons, radio buttons, dropdowns, check boxes, and text input fields do NOT play rollover sounds on hover. Restrict rollover audio at the widget model level via `WidgetKind::has_rollover_sound()`, never through per-dialog ad-hoc exclusions.
  - Dialog push buttons DO play the release-click sound (`ui/buttonclickrelease.wav`) when activated.

- **Dialog Sizing & Content Containment**:
  - Sizing for fixed-content dialogs (such as 3rd-party technology credits) must be flexible and derived from wrapped content layout plus title/button margins, rather than using an arbitrary scroll view.
  - All labels, images, and buttons must stay strictly within dialog bounds without overlapping.
  - Display resolutions too small to fit dialogs must log a report to the console rather than arbitrarily clipping or reintroducing unwanted scrolling.

- **Expansion Background Maps**:
  - Entering an expansion's submenu (Half-Life 2, Episode One, Episode Two, Lost Coast) updates the background map to that expansion's initial chapter background and triggers an asynchronous 3D scene swap.
  - Background map load failures only log a warning to console and `campaign.failures`; they must never crash the game, terminate the process, or dismiss the menu.
  - Navigating Back or pressing Escape preserves the current expansion background map; it must not revert to base Half-Life 2 until another expansion is explicitly chosen.
  - Verification of background map transitions must use end-to-end headless Bevy app tests that assert asynchronous loader completion and active scene entity replacement; unit-testing state transitions alone is insufficient.

- **Background vs Gameplay Loading UI/UX & Transition State Sequence**:
  - Gameplay map loading uses the full modal centered `LoadingDialog` (380 x 112) with a segmented progress bar and localized stage reports (`#GameUI_LoadingFilename`, `#LoadingProgress_*`), blocking clicks to underlying menus.
  - Background map loading uses an explicit state sequence (`BackgroundTransitionState`: `Idle` -> `Starting` -> `Loading` -> `Finished` / `Failed`) and renders a non-blocking bottom-right corner plaque (`LoadingKind::Background`, 130 x 36) displaying localized `#GameUI_Loading` ("LOADING...") over a translucent dark panel routed through the shared dialog frame (`FrameSpec` with rounded corners and no outer 3D border), without progress bars.
  - Menu navigation buttons remain interactive, armed on hover, and clickable while a background map is loading.
  - A full-canvas transition backdrop overlay (`Quad::blur` with `rgba(0, 0, 0, 0.45)`) is drawn behind menu text and buttons during background map loading to soften and dim the underlying 3D scene.
  - When background map loading finishes or fails, the transition cleanly transitions to `Finished` or `Failed`, and the corner plaque and blur overlay disappear immediately.
  - Displayed menu subtitles are derived strictly from `BackgroundTransitionState`: while in `Starting` or `Loading`, the previously displayed subtitle remains visible; when reaching `Finished` or `Failed`, the target subtitle is applied (so a failed load never leaves the subtitle stuck). Rapid consecutive switches update the target map and category, and stale load events for older maps are safely ignored.
  - Technical note on scene blur: in retail Source, GameUI uses a 2D VGUI dimming overlay and corner loading plaque; full live 3D post-processing blur requires offscreen framebuffer render targets (`BlurFilterX/Y`), which is part of the 3D pipeline rather than 2D VGUI. In Bevy, 2D HUD material quads do not read the active 3D framebuffer, so the transition combines the authentic dimming overlay and non-blocking plaque.

- **KeyValues Syntax Quirks & Parser Tolerance**:
  - Retail Valve resource files (e.g. `ep2/scripts/HudLayout.res` in the 20th Anniversary update) contain authentic authoring errors, such as unquoted identifiers merged into quoted literals (`"ypos" "426"HudPoisonDamageIndicator`).
  - Following Source SDK `KeyValues.cpp` (`RecursiveLoadFromBuffer: got } in key`), when `}` is encountered where a value is expected, the parser breaks out of the current block cleanly instead of aborting the parse. This allows retail expansion HUD and chapter background maps to load without error.

- **Menu Button & Title Letter Outline**:
  - Main menu and pause menu button labels (`Mode::MainMenu` and `Mode::Pause`), as well as game titles (`HALF-LIFE 2` / `HALF-LIFE'`) and expansion subtitles (`title2`, e.g. `--- Episode One ---`, `--- Episode Two ---`, `--- Lost Coast ---` across `hl2`, `hl2ep1`, `hl2ep2`, `hl2lc`), are drawn with a thin semi-transparent black border along the letter edge paths/contours of the characters (`TextOutline::DEFAULT`, `Color::new(0.0, 0.0, 0.0, 0.45)`, 1.0px thickness) rather than drawing rectangular bounding boxes or background fills.
  - The letter border is rendered via `FontFace::draw_baseline_outlined` / `draw_cell_text_outlined` emitting 8-directional 1px offset glyph quads with semi-transparent black (`TextOutline::new(Color::new(0.0, 0.0, 0.0, 0.45), 1.0)` default / `0.65` armed / `0.25` disabled) before emitting the foreground glyphs.
  - When rendering character-by-character logos with per-glyph advances (such as `title_face` line 0), rendering uses two passes (Pass 1 emits all 8-directional outline quads for all characters, Pass 2 emits all foreground quads) so outline quads never overlap adjacent foreground characters.
  - When pressed, the button letter text and its contour outline shift together by `(1.0, 1.0)`.
  - Spacers and disabled rows never emit active buttons, and the letter outline ensures crisp text contrast on bright, light, or white backgrounds without modifying menu geometry or drawing button boxes.

- **Dialog Frames, Rounded Corners & Overflow Clipping**:
  - All dialog windows and dialog-like surfaces (options dialog, confirmation boxes for quit/main menu, gameplay map loading dialog, background loading plaque, third-party technology credits, brightness/gamma, and all frames rendered via `Painter::frame` / `Painter::frame_with_close` / `Painter::frame_spec`) route through `Painter::frame_spec` with rounded corners (`FRAME_CORNER_RADIUS: f32 = 5.0`) and omit Windows 95-style outer 3D raised borders (`FrameStyle::DEFAULT.outer_3d_border == false`), matching Source SDK `sourcescheme.res` `FrameBorder` (`backgroundtype 2`). Background fills default to `theme.frame_bg` and support custom color overrides via `FrameSpec::with_bg_color`.
  - Non-dialog widgets (push buttons, close button, sliders, tabs, scrollbars, text entries, checkboxes) strictly retain their 3D raised/sunken borders.
  - Dialog frames enforce overflow clipping and corner containment (`FrameStyle::clip_overflow` and `FrameStyle::clip_rect`), ensuring child controls, title text, and the title bar close button never poke outside the rounded frame boundaries.

- **Cursor Coordinate Conversion & Window Client Alignment**:
  - Pointer/cursor coordinates for UI hit-testing are converted via `hl2_ui::viewport::pointer_to_canvas`.
  - Window cursor positions come from `Window::cursor_position()` in logical inner client coordinates (where `(0, 0)` is the top-left of the client content area, below any OS window title bar).
  - Any outer-vs-inner window origin delta (such as a macOS window title bar height offset) is strictly accounted for so UI hit-testing operates identically across windowed, borderless windowed, and fullscreen modes, at both 1.0 and 2.0 (Retina) display scale factors.

## Runtime invariants to preserve

- Walking uses shared Player/Physics; F2/--fly enables flight. Escape cancels selection before pause; tilde opens console. Consume transition input and pause simulation/audio. Map workers freeze fixed/script clocks; replace map-owned resources only after success. Missing maps preserve the world; saved/global state transfer is unfinished.
- Startup: a loadable `--map` starts gameplay; no `--map` or an unloadable one opens the main menu over the paused chapter-1 background map, except that map-dependent arguments (position/angles/fly/movement script/live, burst or monitor captures) without a loadable `--map` fail the launch with an error. Escape never turns a background level into gameplay.
- Submenu background maps: entering an expansion submenu updates the background map and scene asynchronously; failures log a warning and leave the menu intact. Back or Escape preserves the expansion background map.
- Modal dialog occlusion: when a modal dialog (message box, options dialog, loading dialog) is open, underlying menu buttons must not be hovered, armed, or clicked.
- HUD uses owned resources and an ordered CPU canvas. GDI font rasterization is setup-only.
- Sky background layer3/order-2, miniature sky layer4/order-1, world layer0: independent depth/color clears and BSP 2D/3D eligibility. Current animation bounds and all active player/monitor views drive visibility; missing data fails open, script-hidden/killed entities remain hidden. Spatial splitting is opt-in.
- Eyes use owned MDL records, animated eyes attachment origin/forward and iris projection before entity transform/scale. Preserve authored LOOKAT ramps, cancellation tails, pause refresh, cycler meshes and one target for both eyes. EyeRefract uses explicit Eyes_dx8 fallback; native head/facial/random/tactical attention is unfinished.
- Door use carries opener origin, opens linked leaves away and honors explicit direction. Blocked reversal/arbitrary spawn/native linkage remain incomplete.
- Keep the known zero-byte d2_coast_02.bsp rejection and station03's known materials distinct from new regressions. Research references are not integrated iw4L crossovers.

Template basis: [AGENTS starter](https://github.com/trevaintdead/ai-game-modding-guides/blob/main/templates/AGENTS-starter.md), adapted to existing owner authorization and this rewrite.
