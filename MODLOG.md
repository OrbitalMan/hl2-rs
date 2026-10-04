# HL2 reconstruction journal

## 2026-10-05: shared HUD presentation in Bevy

Moved the retained owned-resource HUD into engine-independent hl2-ui with an explicit ordered CPU canvas. Both renderers use the same layout/font/crosshair/animation logic. Bevy now draws health/ammo, weapon buckets, quick-info and secondary ammo through a separate overlay camera with normal/additive materials. Asset loading stays outside systems; glyph textures and meshes are reused.

Packaged white-crosshair captures match the accepted retail five-pixel positions at 720/1080. Selection preserves active ammo, secondary-panel motion is captured at start/intermediate/end, and the door/weapon and bench fixtures remain passing. Native font rasterization and blend/gamma equivalence are still partial. Audio, pause/console, projectile/impact presentation and campaign host migration remain open.


## 2026-10-04: Bevy entity, weapon and animated presentation bridge

Moved the tested entities/gameplay/NPC/projectile/selection implementations into hl2-simulation and retained host re-exports. Shared actor/viewmodel preparation preserves clip loading. Bevy now runs scene, weapon, moving collider, NPC/projectile, rigid-body and player state in retained order. Local entity meshes follow authoritative poses/visibility; CPU skeletal animation updates bounds; an independent viewmodel pass preserves the existing projection. F3, weapon buckets/wheel, confirming/fire/reload/previous, E use and G test impulse are connected. Quick-click and pause/selection suppression regressions pass. Packaged bench and station-door/secondary-weapon fixtures pass; HUD/audio/effects/campaign host migration remains next.


## 2026-10-04: shared collision and Bevy player movement

Extracted the existing collision/Rapier adapter, convex sweeps and NPC probes into `hl2-simulation` unchanged, preserving their 25 tests and retained-runtime imports. Bevy now uses the same Source-coordinate player and collision code at 15 ms per step, with input/look before simulation and camera presentation afterward. Default walking, flight toggle and pause/resume are supported; input capture consumes transition-frame mouse movement and held jump until release.

The packaged bench fixture passes 26 movement/pause assertions, including the native collision height, one-time air crouch lift with retained eye/momentum, air uncrouching and jump rearming. Rendering remains static and rigid bodies are frozen until presentation is synchronized. Weapons/HUD, pause UI/console, entity I/O/scenes, NPC animation/AI and audio remain migration work. See `docs/validation.md` for package and test evidence.

## 2026-10-04: separate Bevy/wgpu host preview

Preserved the original host on `macroquad-prototype` and added `bevy-migration` for the long-term Bevy/wgpu work. Main keeps the retained runtime until verified replacements are available. README and contributor guidance identify the branch and executable each feature belongs to.

The new host reads owned maps through the shared format/core crates and renders static BSP, displacement, prop and entity geometry with custom base/lightmap materials and a fly camera. Shared CPU/GPU texture caching and visible-material filtering correct the first preview's loader-budget exhaustion. BSP render winding is normalized before appending already-normalized models, correcting culled model fronts without altering shared collision data.

Validation: 241 normal tests and nine owned-install checks pass, with formatting and strict Clippy. Final packaged station-map captures at 720p and borderless 1080p were inspected, with no texture-budget or capture failures. Dynamic monitor and eye materials remain unsupported. Gameplay, collision, animation, AI, choreography, HUD and audio have not migrated; Source rendering and performance parity are not established. See `docs/bevy-migration.md` and `docs/validation.md`.

## 2026-10-04: G-Man speech and authored locomotion data

Fixed the first-map G-Man omission with explicit `cycler_actor` model and scene-actor support, corroborated by retail factory/RTTI and actor lookup. The packaged debug-camera fixture resolves 17 authored intro events. Its initial run exposed compressed voice failures; standard Microsoft ADPCM now decodes in memory, preserves recorded frame counts and successfully requests playback of both opening lines. Facial animation, gestures, intro cameras and compositing remain unfinished.

Added bounded PC AIN37 decoding and optional inspect/runtime reports, preserving hull offsets, raw masks/metadata and Hammer IDs. Added v48 authored movement records and activity/all-blend metadata, plus piecewise motion sampling, turning and positive/negative loops. Owned walk/run tracks match 80 units/1s and 125.87412 units/0.6s. These readers do not yet implement NPC pathfinding, weighted blending, motor planning or movement readiness.

Validation: 210 workspace tests, six owned-install checks, formatting and strict Clippy pass. The rebuilt launcher package passes 12 G-Man, 33 scene-controller/sequence and 20 secondary-projectile assertions. A graph census decodes 72 and rejects six mismatched revisions across 78 loadable installed maps; the known empty coast map remains rejected. Private evidence and game files remain outside public source.

## 2026-10-04: crosshair, secondary projectiles and authored scenes

Corrected the oversized unarmed crosshair by identifying the native height accessor and reproducing both texture-coordinate insets. Packaged720/1080 captures now match the original five pixel positions; odd viewport rounding also passes. Font rasterization and tone mapping remain separate work.

Added real SMG contact grenades and AR2 charging balls, separate cooldowns/reserves, reload interruption/veto, continuous collision queries, bounded blast obstruction/damage, bounce/expiry and owned effects. Native controls corroborate selected weapon-state behavior. Rapier physics, damage policy and effects still do not reproduce the full Source implementation.

Added a bounded Rust reader for compiled choreography and authored scene control/triggers/completion. Selected installed SEQUENCE clips follow the paused scene clock and restore their baseline. Unsupported movement readiness holds SECTION rather than inventing success. The first level still lacks NPC schedules/movement, gesture/facial layers and intro systems needed for complete playback.

MP3 cues decode into an in-memory PCM cache, resolving the trainstation music gap. Added bounded cheat-gated ent_fire through normal entity I/O and more detailed snapshots.

Validation:192 workspace tests, both owned-install checks, strict Clippy and formatting pass. The latest package passes27 existing weapon checks,20 projectile checks and33 first-map scene-controller/sample checks. Captures and limitations are recorded in docs/validation.md; private research and installed assets remain excluded from Git.

## 2026-10-03: air crouch, stable contacts and pause/console

Air crouching now tucks the feet once while preserving head height and momentum; standing clearance gates air unducking. Reviewed retail sliding rules and analytical world-brush/convex sweeps correct reproduced wall-contact jump interruptions. Native PHY compound integration and full VPhysics parity remain unfinished.

Added a working resource-based pause menu and bounded developer console with cheat gates, history/editing/completion, position/angle commands and owned map loading. Focus freezes simulation and suppresses attack/jump leakage. The menu uses the owned title font and calibrated 720p metrics. Full console, save/options and paused audio remain open.

139 tests and strict Clippy pass; packaged weapon, movement and console fixtures pass their meaningful assertions. Latest double-door collision and secondary-ammo HUD reports are queued for the next iteration. Private bench PHY research matches the measured original standing height but is not integrated into Rust yet.

## 2026-10-03: publication and automatic empty fire

Published the reviewed Rust source to kvalls/hl2-rs with fresh-checkout instructions, contribution boundaries and a labeled runtime screenshot. The public main branch starts from a clean source snapshot; older local research history stays private. Windows formatting, Clippy and unit checks run for main pushes and contributor pull requests.

SMG1/AR2 now use a reviewed empty-fire latch and per-weapon half-second sound throttle. Empty clicks retain the primary deadline and animation; the next eligible attempt reloads, and idle reload uses a strictly elapsed primary deadline. Four regression tests cover held/released input, throttle boundaries, independent weapon state and reload completion. Other secondary attacks, autoswitch ranking and custom reload flags remain incomplete.

## 2026-10-03: depth, secondary fire and retail input/movement

Separated OpenGL depth testing and writes in the vendored backend, preserving depth clears between sky/world/viewmodel passes. Matching trainstation views reproduce then remove hidden light shafts, entrance columns and barrier effects; a front-side capture retains the visible columns.

Added shotgun alternate fire with twelve pellets/two shells, one-shell primary fallback, native/script-derived pump deadlines and retained secondary reload interruption. Accepted health/battery pickups now use their installed item sounds. The original engine confirms a six-to-four-shell alternate discharge.

Selection now consumes already-held attack buttons when Slot/Wheel opens the menu, rearms each button independently, and observes ordered script release/repress transitions. The F1 overlay identifies the Rust runtime. Movement now preserves standing/duck jump gravity ordering, categorizes after the sweep, crops diagonal boost, permits negative speed-cap additions and retains the native next-tick rising-air friction factor.

Fresh-clone build instructions and contribution boundaries were added for public source sharing. Game assets, native analysis, captures and research tools remain separate. Full campaign, NPC AI and Source rendering/collision parity remain unfinished; docs/validation.md records the checks and package fingerprint.

## 2026-10-02

The original executable remains read-only, with the earlier save/config snapshots preserved. Work is focused on HL2 fidelity; no FAL assets or crossovers were added.

The first prototype supported map/prop display. This iteration adds brush submodels, displacement/prop collision, fixed-step movement, timed entity I/O and doors, primary lightmap atlases, material transparency/tint/two-texture scrolling, compressed skeletal animation, PCM WAV audio, crowbar/pistol gameplay and limited scripted sequences/map transitions. These are partial implementations; the README lists the missing systems.

Mouse-look uses macroquad's previous-minus-current delta: upward movement increases pitch, rightward movement decreases Source yaw. The Windows miniquad reader additionally converts absolute RAWINPUT positions into deltas; treating those positions as motion caused extreme jumps during automation. Interactive verification is recorded in docs/validation.md.

Regression tests cover ordered/delayed duplicate outputs and fire limits, door locking/completion, shuffle batches, post-idle completion timing, ammunition transfer, stationary hull overlap at a floor, and disabling killed dynamic colliders. Movement and animation tests cover deterministic replay, acceleration/friction, jump apex and hierarchy/interpolation.

Research tools and private decompiler output belong in ../../work/hl2-decompiled. They are development references only. The runtime builds from Rust and reads owned assets in place. The packaged executable, rather than target/debug, is the final launch.cmd validation target.


### 2026-10-02: weapon selector, impacts and separate sky world

- Preserve nested sound-script wave alternatives; use distinct crowbar flesh/world sounds and hit animation.
- Add receiver-clipped installed impact textures and inherited surface-property bullet sounds. Marks follow rigid props and brush entities; keep a bounded local pool.
- Implement six-bucket UI with installed weapon icons, wheel cycling, confirmation, cancellation and previous-weapon switching; cancel reload on switch. Only crowbar and pistol combat are implemented.
- Separate background geometry/props using BSP leaf/PVS data and sky-camera scale. No native game code or decompiler tooling entered this project.
- Validation: 36/36 unit tests and workspace Clippy pass; packaged interactive run recorded three hit decals and multiple metal-impact sound variants with zero audio errors. Wheel UI rendered correctly; live confirmation needs a faster-input retest because its timeout expired between automation calls. Installed maps 78/79 parse with only the known empty coast map failure.

### 2026-10-02: retail HUD, six primary weapons and sky visibility

The earlier selector was a provisional design. This pass replaces it with installed resource dimensions, fonts/cell metrics, corners, labels and input behavior, checked against selected retail methods and a localized original-engine capture. Numeric HUD animation rules, bounded low-health loops and QuickInfo progress/fades/warning audio now run. Native GDI rasterization, damage messages and the remaining HUD panels/gates are still incomplete.

Added .357, SMG1, AR2 and shotgun primaries, separate ammo reserves, native/script-derived cadence/spread, shotgun shell/pump/interruption behavior and installed model sound events. Crowbar collision now tries the full ray, shortened +/-16 hull, facing test and closest corner refinement over approximate Rapier geometry. The background renderer now checks the camera BSP leaf's 3D-sky flag.

Validation: 79/79 workspace tests, strict Clippy, formatting and diff checks pass. Packaged input regression records 64 attacks, 82 impacts/marks, 6 model sound events, 199 audio requests, zero audio errors, two crowbar impact variants and one low-ammo warning. Actual Computer Use wheel/left/right/Q/Escape input verified selection with zero shots. A 60-frame hidden-sky test recorded zero sky frames. Installed maps remain 78/79, with the existing empty coast map rejected. See docs/validation.md for artifacts, current package fingerprint and limits.

Original-game research writes, localization copy, inspection tools and native pseudocode stay outside the Rust project. The original test process was closed, stock cfg manifest unchanged, no native DLL patched and no FAL/crossover used. Full NPC/campaign/shader fidelity remains substantial work; parser coverage is not campaign completion.

### LDR sky asset groundwork

Added a bounded six-face LDR sky loader resolving VMT base textures and transforms, with eight synthetic tests. HDR rendering, cube drawing and leaf gating integration remain unfinished. All 109 workspace tests and strict Clippy passed; the final package was rebuilt and smoke-tested.

### 2026-10-03: database audit, sky rendering and movement collision

Integrated the six-face LDR sky background with verified retail orientation, installed material transforms, clamped sampling and separate BSP 2D/3D eligibility. It draws before scenery/world without depth writes. Matched original-engine views corroborate cloud orientation; HDR, fog and sky polygon masks remain unfinished.

Player collision now excludes retained NPC-only world clip brushes, correcting the oversized invisible station-bench blockers. Grounded crouch commands and maximum speed are separated for jump boost; airborne crouch retains full air acceleration. Regression checks cover clip masks, duck boost caps, backward overspeed, air strafing and chained released jumps. Native PHY geometry and full movement equivalence remain open.

The supplied Database Method was tested privately across all exports. Unchanged gamedb misses functions and some boundaries/call targets, so an exact-byte catalogue keyed by module/address preserves the complete export inventory and known failures alongside the supplemental navigator. This improves traceability without marking semantic parity automatically. The original game also confirmed three-shell shotgun secondary behavior: double discharge leaves one shell, and the next secondary action fires it as a single shot. See research and validation for measured coverage and packaged checks.

### 2026-10-03: air crouch, console, static PHY and secondary HUD

Air crouching preserves head height with a one-time foot lift, and stable world/convex hull sweeps correct reproduced wall-jump contacts. Added a resource-driven pause menu and bounded console with cheat gates, history, completion and map loading. Original ground-duck timers, prediction and full Source command/UI coverage remain unfinished.

Added a bounded original Rust PHY reader and separate installed convex pieces for supported solid static props. Bench standing height now agrees with the owned original measurement. Rotating-door collision applies the visible model's initial idle pose, fixing the station entrance's closed-door walkthrough and invisible open-shaped blocker.

Primary ammo stays visible during weapon selection. Unarmed crosshairs use the installed white default sprite; armed crosshairs retain installed glyphs. SMG/AR2 secondary reserves, carry limits, pickups and ALT counters now work. Their actual grenade/energy-ball attacks and the remaining weapons are still missing. See validation for package fingerprints, tests and the known trainstation MP3 music gap.

### 2026-10-04: Barney scene movement and actor-aware speech

Added Barney's native default model and owned MDL eye metadata, dedicated NPC collision masks/probes, bounded human ground routes and authored central walk/run locomotion. Scene MOVETO requests now persist while SECTION waits for actual arrival; the controller continues while only the scene clock is paused. UI pause freezes both, blocked routes hold the gate, and cancel/script ownership prevents stale pose changes. The controlled security03 fixture verifies player blocking, arrival-driven door output and cancellation without a manual scene Resume. It explicitly seeds a preceding authored target and is not a complete campaign replay.

Symbolic speech carries the resolved actor model through the installed gender registry and retains native tagged wave/fallback behavior. The first-map owned census resolves all 72 actors and decodes all 75 registered alternatives. Native RNG/mixing, localized combined lines and lip sync remain open. See validation for tests and packaged checks; native motor timing, weighted blends, general NPC AI, facial/eye rendering and the playable first level remain unfinished.

### Bevy audio adapter

Shared owned sound-script selection, actor gender, WAV/MS ADPCM/MP3 decoding and viewmodel event cursor now serve both hosts. Bevy preloads references outside systems and plays ambient/scene/weapon/HUD requests through monitored audio sinks, including host pause/resume. Packaged door/weapon and controlled scene fixtures, full tests and strict Clippy passed. Source spatial audio/DSP, soundscapes and lipsync remain unfinished; this does not complete the campaign.
