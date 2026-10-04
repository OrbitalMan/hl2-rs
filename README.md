# HL2-RS

A standalone, partial Rust reconstruction of Half-Life 2 that reads maps, models, textures, animations and sounds from an installed copy. It does not load Valve's game or engine DLLs. The full campaign is not playable yet.

## Runtime migration and contributing

Development is moving toward a Bevy/wgpu host, following the separation of owned asset readers, simulation and rendering demonstrated by [iw4L](https://github.com/vladtrc/iw4L). The original Macroquad/OpenGL runtime was built as an early prototype; its rendering and gameplay work remains available.

| Branch | Purpose | Pull requests |
| --- | --- | --- |
| `main` | Current working partial runtime. It still uses Macroquad while the replacement is being verified. | Shared Source-format/core fixes and fixes to the current runtime. |
| [macroquad-prototype](https://github.com/kvalls/hl2-rs/tree/macroquad-prototype) | Preserved snapshot of the original prototype before migration. | Reference branch; discuss continuing prototype-specific work first. |
| [bevy-migration](https://github.com/kvalls/hl2-rs/tree/bevy-migration) | Bevy/wgpu host with shared movement, entity/weapon simulation and animated presentation alongside the existing runtime. | Bevy host, renderer and migration changes. Target this branch rather than `main`. |

`source-assets` and `modkit-core` stay engine-independent. This branch also extracts shared collision, rigid-body support, NPC probes/controllers, entity I/O/choreography, weapons/projectiles and selection into `hl2-simulation`, reused by both hosts. Existing asset decoders, simulation behavior and validation are retained as the host is migrated. Bevy's default PBR materials do not recreate Source shaders or physics; those still require their own implementations and comparisons. No speed or fidelity improvement is assumed solely from changing engines.

Bevy work will enter `main` after its rendering and gameplay replacements are verified. Until then, use the current launcher below for the existing runtime and the migration branch's own build/run instructions for its experimental renderer. A Bevy map preview is not a playable campaign. See [CONTRIBUTING](CONTRIBUTING.md) before opening a PR.

On this branch, build with `scripts/build-bevy.ps1` and run `launch-bevy.cmd`. The Bevy executable defaults to walking with the shared 15 ms `Player` controller and `Physics` collision queries; `--fly` or F2 enables flight. Escape pauses and releases the cursor; click resumes. Doors and physics props now move with their colliders, available entity clips and weapon viewmodels animate, and retained scene/weapon/projectile logic runs in the fixed step. F3 gives the developer loadout; E uses doors, slots/mouse wheel select weapons and mouse buttons fire/confirm. HUD, audio, impact/projectile visuals and campaign transitions are still pending. Tick-based `--movement-script` fixtures record player/gameplay state and capture the final frame. See [Bevy migration instructions and milestones](docs/bevy-migration.md). The detailed gameplay/weapon/HUD lists below describe the retained `hl2-runtime` executable launched by `launch.cmd`.

![HL2-RS Rust runtime rendering the trainstation with pistol, HUD and development diagnostics](docs/images/hl2-rs-trainstation.png)

Capture from the packaged Rust build on `d1_trainstation_02`, with F1 diagnostics and a developer weapon loadout. This is a rendering preview, not evidence of completed campaign gameplay. The screenshot depicts owned HL2 content; distributable game assets are not included.

## Build a fresh checkout

You need an owned, installed Steam PC copy of Half-Life 2, Windows 64-bit, Rust stable with the MSVC toolchain, and Visual Studio C++ build tools with a Windows SDK. The renderer uses OpenGL; GPU and RAM minimums have not been measured. Other operating systems are unverified.

The tested installation is Steam app 220, build `19307283`, patch `9912070`. Other game builds are unverified. Rust `1.99.0` was used for the recorded Windows checks. Cargo resolves the library versions in `Cargo.lock`; no external mod, loader or Source engine runtime is needed.

```powershell
git clone https://github.com/kvalls/hl2-rs.git
cd hl2-rs
.\scripts\build.ps1
.\launch.cmd
```

The repository contains source, not a prebuilt executable. Building creates `bin/hl2-rs.exe`. If PowerShell blocks the local build script, invoke `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build.ps1` for that process. To uninstall, remove the checkout; the game installation is read-only.

## Play the current build

Double-click `launch.cmd` in this folder. It launches `bin/hl2-rs.exe`, the packaged release build. Rust is needed only for rebuilding. The default map is `d1_trainstation_02`, and the default mod configuration adds no sandbox blocks.

```powershell
.\launch.cmd --map d1_trainstation_01
.\launch.cmd --map d2_coast_03
.\launch.cmd --game 'C:\Program Files (x86)\Steam\steamapps\common\Half-Life 2'
.\launch.cmd --mods mods/sandbox.json
.\launch.cmd --borderless
.\launch.cmd --width 1920 --height 1080
```

Double-click `launch-borderless.cmd` to fill the primary desktop without changing its display resolution, or `launch-1080p.cmd` to request a 1920x1080 window. Borderless is verified at 1920x1080 on the tested desktop. Windows shortened the titled 1080p window to 1920x1061 to fit; use borderless for exact 16:9 comparisons. Explicit pixel-size modes enable DPI-aware rendering. Other monitors, DPI settings and native font rasterization remain unverified.

Steam libraries are discovered automatically; `HL2_ROOT` can override discovery. Original assets are read in place. There are no bundled game assets, decompiler tools or native game DLLs in the runtime. No FAL account or credits are needed.

| Control | Action |
| --- | --- |
| WASD / mouse | Move / look; click to capture mouse |
| Escape / tilde | Cancel weapon selection first; otherwise pause / open developer console |
| Space / Ctrl | Jump / crouch |
| Shift / Alt | Sprint / walk slowly |
| E | Use a door or button within reach |
| Left click / R | Attack / reload |
| Right click with shotgun | Double shot; one remaining shell falls back to a single shot |
| 1–6 / mouse wheel | Open/cycle owned weapons by stock slot and position |
| Left/right click while selecting / Escape | Confirm without firing / cancel |
| Q while walking | Switch to the previous usable weapon |
| F3 | Developer loadout: crowbar, pistol, .357, SMG1, AR2, shotgun, reserves and suit |
| F1 | Toggle the development overlay; hidden by default |
| Middle mouse | Apply a test impulse to a nearby prop |
| F2 / Q / E | Toggle flight / rise / descend while flying |
| F4 | Reset player to the map spawn |
| F5 / Tab | Reload JSON mod / toggle entity markers |
| B / Backspace | Place / remove a sandbox block |
| F12 / F10 | Save screenshot and camera / quit and write runtime report |

The starting map normally has no player weapon. F3 is a test aid. It does not demonstrate campaign weapon acquisition. Some campaign scripts still block progression.

The pause menu has working Resume Game, Developer Console and Quit controls. The console supports history, editing and command-name completion with Tab. `help` lists the implemented commands. Enter `sv_cheats 1; impulse 101` to get the six implemented weapons, ammunition and suit. Other supported commands include `noclip`, `getpos`, `setpos`, `setang`, `map`, `ent_fire`, `find`, `echo`, `clear`, `toggleconsole` and `quit`. `ent_fire <target> [input] [parameter] [delay]` sends normal entity I/O to named targets; its console delay uses whole seconds. Classname fallback and the full Source command registry are unfinished. Unsupported commands report an error. Save/load, options and bindings are unfinished. Player, weapon, entity and prop simulation pauses while the menu or console is open; ambient audio does not yet pause. The UI uses installed GameUI labels/colors and system fonts, but its layout is not an exact VGUI reconstruction.

## Implemented so far

- Read-only VPK v1/v2 and loose content mounting, map ZIP content, custom content directories/VPKs, CRC checks, bounded readers.
- BSP 19/20 and compressed lumps, world geometry, brush submodels, displacement geometry and collision, duplicate entity outputs.
- VMT patch includes, VTF decoding, primary baked lightmap atlases, opaque/cutout/translucent/additive materials, basic two-texture scrolling and entity tint. Six-face LDR sky backgrounds use installed textures/transforms and separate 2D/3D leaf visibility. Lighting remains approximate.
- MDL/VVD/VTX meshes, skins, static props and selected entity models, including the first map's `cycler_actor` G-Man. Bone hierarchies, skinning, selected compressed MDL/ANI clips and bounded sequence-event decoding; viewmodel sound events play at their recorded cycles.
- Bounded PC AIN37 navigation-graph decoding, preserving hull offsets, link movement masks and editor IDs. A first Barney controller supports collision-validated direct/ground-node routes, authored central walk/run motion and actual-arrival scene gates. Inspection/runtime reports expose rejected routes and missing support. Full NPC schedules and combat AI remain unfinished.
- Authored v48 model locomotion records, activity names and all blend tracks, with piecewise root-motion sampling, turning and loop accumulation. Barney uses the forward central tracks; weighted pose blends, native motor timing and turning remain unfinished.
- Fixed 15 ms movement with acceleration, friction, gravity, air movement, jump, airborne crouch/uncrouch, sliding and stepping. World brush planes and convex collider SAT provide stable player hull sweeps; meshes and other shapes retain fallback queries. Collision is not Source prediction or exact collision equivalence.
- Bounded VPHY/IVP convex collision from matching installed PHY/MDL files for supported static props, with separate convex pieces. Rotating-door collision follows the same initial idle pose as the visible model. Rapier rigid props, interaction and impulses retain approximate mass/material properties and explicit render-mesh fallbacks.
- Timed entity I/O, target lookup, relays, counters, branches, cases/shuffling, timers, player triggers, doors and a subset of scripted sequences. Installed VSIF/BVCD choreography provides authored Start/Pause/Resume/Cancel, actor-aware speech, triggers, STOPPOINT completion, selected full-body SEQUENCE playback and bounded Barney MOVETO. Unresolved actor readiness holds SECTION events; unsupported scene control is diagnosed. Complete campaign scenes remain unfinished.
- Crowbar, pistol, .357, SMG1, AR2 and shotgun primary attacks, shotgun secondary attack, installed scripts/viewmodels, separate reserves, magazine reloads and shell-by-shell shotgun reload/pump/interruption. Shotgun secondary fire consumes two shells and fires twelve pellets; reload interruption retains a delayed shot after release. Crowbar traces use the Source ray/hull/corner sequence with approximate Rapier geometry. NPC models can animate and take damage, but do not implement combat AI.
- SMG contact grenades with gravity, swept box collision, occluded blast falloff and reserve consumption. AR2 charge/release, current-aim launch, reload/holster veto, swept energy balls, reflection, bounded NPC damage/dissolve handling and timed expiry without blast damage. Free balls use owned effect textures for body, flicker and motion blur; effects and collision remain approximate.
- Resource-driven PC weapon selection, health/suit/ammunition animation rules, SMG/AR2 secondary-ammo counters, low-health pulse loops, QuickInfo brackets/progress/fades/warning sounds, installed fonts and rounded corners. Primary ammo remains visible during selection. The unarmed white crosshair uses the installed default sprite; weapon crosshairs use installed glyphs. Layout, font-cell scaling and input behavior are compared with selected retail HUD methods and original-engine captures; exact native font rasterization is not reproduced.
- PCM WAV, standard Microsoft ADPCM WAV and bounded MP3 decoding for installed ambient/event/weapon audio, including trainstation music and opening G-Man speech. Map changes through `trigger_changelevel` preserve landmark-relative player position and inventory.
- Common world/mod interfaces in `modkit-core`, with a reloadable JSON sandbox. No crossovers are integrated.

## Fidelity work remaining

This is not a completed rewrite of Source. Major gaps include general NPC navigation/schedules/combat, the remaining arsenal, recoil/muzzle flashes/tracers/shell ejection, exact prediction spread and shotgun pellet hull traces, the full player damage/death pipeline, vehicles, save/load, campaign state transfer, complete choreography movement/loops/subscenes/dialogue/lip sync, non-sound animation events/blends/layered gestures/IK/flexes, ragdolls, dynamic/animated PHY collision, moving-platform behavior and blocked-door handling. Scripted movement and parenting/attachments are incomplete. HUD damage-message dispatch, history/aux-power panels, zoom/convar gates and additional VGUI animation commands remain unfinished. Projectile collision/material response, relationship and damage filters, water rejection, guided targeting, dissolve visuals and explosion particles do not reproduce the full native behavior.

Water/ladders, surface movement modifiers, exact crouch transitions and prediction need further work. The renderer lacks Source's full lighting/material system: HDR/exposure, bump/normal/specular shading, light probes, shadows, cubemaps, sky polygon masking, sky fog, fog, water/refraction, dynamic lightstyles and many material proxies. Symbolic speech now uses the owned actor-model gender registry and registered wave alternatives. Audio still lacks streaming, spatial mixing, soundscapes, DSP, HEV sentence scheduling and other compressed formats. An MP3 is decoded once into a bounded in-memory PCM cache; its first request can stall while decoding. Content mounting is not a general `gameinfo.txt` implementation. Supported map loading does not prove that a map's gameplay works.

## Rebuild and inspect

With Rust stable, rustfmt, Clippy and Visual Studio C++ tooling installed, rebuild using:

```powershell
.\scripts\build.ps1
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
.\bin\hl2-rs.exe inspect --map d1_trainstation_02 --output artifacts/inspect.json
.\bin\hl2-rs.exe verify --all --output artifacts/verification.json
.\bin\hl2-rs.exe view --frames 90 --smoke --capture artifacts/smoke.png
```

`build.ps1` replaces the executable used by `launch.cmd`; changing source or committing it does not rebuild that executable. Use `-DebugBuild` only when deliberately packaging a debug build. `bin/build-info.json` records the packaged executable's fingerprint when built with the script.

`export` writes a local game-derived world; keep those exports local. Runtime captures/reports are written in ignored `artifacts/`. The two licenses in `third_party/miniquad` cover the vendored Rust window/input dependency, including its Windows absolute-mouse fix.

## Research boundary

The separate native research folder for this checkout is `../../work/hl2-decompiled`. Its private binary copies, Ghidra databases, C pseudocode, indices and research scripts are outside this project. It is a reference for implementing behavior; none of its native code is loaded or compiled into the Rust runtime. Native exports are approximate pseudocode, not recovered original source.

Development used the universal-modder Codex plugin and ten skills, version 0.2.0. They are not runtime or build dependencies. Its FAL capabilities are unused. The requested three reference repositories were read and are recorded in [research](docs/research.md). See [validation](docs/validation.md) for concrete checks and [MODLOG](MODLOG.md) for implementation notes.

This is an AI-assisted project developed with OpenAI Codex and GPT-6, with original-game testing and manual review of selected native behavior. It remains an experimental reconstruction. Contributions are welcome; read [CONTRIBUTING](CONTRIBUTING.md) for the code boundaries, checks and useful evidence to include in a PR.

Original project code is MIT licensed. Installed assets and private analysis retain their own rights and are excluded from Git. Dependencies retain their own licenses.


## Current weapon and sky iteration

The PC selector follows the installed resources and reviewed `CHudWeaponSelection` behavior: five small slot boxes and one expanded column, only selected-item text, stock colors/corners, wheel cycling, either mouse button to confirm, Escape to cancel and Q for the previous weapon in walk mode. Empty slots retain their box without a number. It starts its inactivity fade after 0.5 seconds and closes after 1.25 seconds. Confirming consumes attack input. The six implemented primary weapons use separate ammunition and reload state; switching cancels an incomplete reload.

SMG1 and AR2 primary empty fire follows the reviewed base-weapon latch: the first empty attempt clicks without changing the animation or attack deadline, and the next attempt tries reload. Empty sounds have separate per-weapon 0.5-second throttles; idle automatic reload waits until the primary deadline has elapsed. Their secondary attacks have independent cooldowns and reserves. Next-best-weapon autoswitch and custom no-auto-reload flags remain unfinished.

The world camera converts HL2's 75-degree horizontal field of view at 4:3 into vertical field of view; the viewmodel keeps its separate 54-degree reference. Development banners are hidden by default and can be toggled with F1.

`--input-script <JSON>` drives timed regression actions through the normal runtime handlers and records them in the runtime report. Captures use simple filename stems under `artifacts/`. `--time-scale 0.1` slows simulation for manual HUD/input checks; normal play defaults to 1. These test options do not establish full original-game equivalence.

Run `.\launch.cmd --input-script test-inputs/weapons-hud.json` for the approximately 41-second weapon/HUD regression after the map loads. It supplies a test loadout, selects/fires/reloads the six supported weapons, checks selector confirmation/cancellation/timeout, hits the floor with the crowbar and empties/reloads the SMG. It exits automatically and writes captures and `artifacts/runtime-report.json`.

Run `.\launch.cmd --input-script test-inputs/shotgun-secondary.json` for a nine-second secondary-fire regression. It checks held right-click confirmation, double fire and cooldown, one-shell fallback, and reload interruption with release before the delayed shot. Both fixtures overwrite the same runtime report; save it before another run if needed.

Run `.\launch.cmd --input-script test-inputs/secondary-projectiles.json` for SMG grenade consumption/contact explosions and AR2 charge, reload/holster veto, flight, bounce, cooldown and expiry. Runtime reports record active projectiles, effects, diagnostics, health and secondary reserves. This exercises the Rust implementation; native physics equivalence still needs differential testing.

Run `.\launch.cmd --input-script test-inputs/selection-held.json` for held-button selection, independent button rearming and an ordered release/repress in one input poll. These captures and reports also stay in ignored `artifacts/`.

Run `.\launch.cmd --map d1_trainstation_01 --input-script test-inputs/compiled-scene-security.json` to check authored security-scene timing, the door output and an installed customs-officer sequence. This older control fixture manually resumes the movement gate and does not demonstrate a playable first level.

Run `.\launch.cmd --map d1_trainstation_01 --input-script test-inputs/barney-scene-movement.json` for the 25-second collision/scene regression. It explicitly seeds Barney at the authored security02 monitor target, blocks his security03 route with the player, pauses/resumes the UI, then moves the player away. Actual arrival releases the scene gate and the map's output opens the door; a second run checks cancellation. `actor_pose` is fixture setup, not a replay of the preceding campaign. Full-body walk/run clips use the controller clock while SECTION is frozen. Only ordinary Barney is registered for this first movement scope; native motor ETA/acceleration/turning, weighted blends, dynamic links/hints, give-way and general scripted movement remain unfinished.

Run `.\launch.cmd --map d1_trainstation_01 --input-script test-inputs/gman-intro-actor.json` to preview the owned G-Man model and the first 18 seconds of authored intro events from a debug camera. Facial animation, gestures and the original intro camera/compositing remain unfinished.

Run `.\launch.cmd --input-script test-inputs/pause-console.json` to exercise the pause menu, console commands, cheat gates and input focus. `.\launch.cmd --input-script test-inputs/movement-crouch.json` compares ordinary and airborne crouched jumps on the station bench. Scripted movement and rendered-frame snapshots are recorded in the runtime report. These fixtures exit automatically; captures remain local.

Crowbar flesh/world impacts use their distinct script sounds, and symbolic sound playback retains all `rndwave` alternatives instead of discarding all but the first. Surface impact marks use installed concrete/metal/wood/glass textures, are clipped to receiver triangles and follow moving props/doors. Their rendering approximates DecalModulate; NPC blood decals, layered fading and exact Source surface response remain unfinished.

Health kits, health vials and batteries play their own installed pickup sounds when accepted. Full health/armor leaves the item in place, and a battery requires a suit. Suit logon sentences are still missing.

BSP leaves and compressed PVS identify the miniature 3D sky area. Its scenery uses the map's sky-camera origin/scale and renders before the playable world with a separate depth clear, only when the camera leaf has the `LEAF_FLAGS_SKY` visibility flag. The six-face LDR background draws first, without depth writes, when either the 2D or 3D sky bit is present. Opaque map geometry still occludes both passes. The patched OpenGL backend tests opaque depth for translucent/additive surfaces independently of their depth-write mask. Matched trainstation captures verify that hidden light shafts, entrance pillars and barrier effects no longer draw over the intervening wall; visible entrance pillars remain. General PVS/areaportal culling, native sky polygon masks, HDR, sky fog and multiple sky cameras remain unfinished.

Player hull queries exclude NPC-only clip brushes. Supported solid static props now use their installed PHY convex pieces, correcting the bench height and preserving gaps between pieces. Other model formats, static solid modes, animated and dynamic collision retain explicit fallbacks; Havok/VPhysics solver equivalence remains unfinished.

Grounded crouch now crops movement commands independently of maximum speed, preserving the reviewed jump-boost cap. Air crouching retains full air acceleration. Released jumps can chain without an added ground-friction tick in controlled tests. Exact crouch/suit transitions and original-game movement over campaign geometry still need comparison.

Ordinary airborne crouching now tucks the feet upward by 36 units while preserving head height; holding crouch does not repeat the lift. Air uncrouching lowers the origin only after a clear standing-hull sweep. Sliding follows reviewed retail plane-reset, clipping and overlap-exit rules. Stable world-brush and convex-prop player sweeps correct reproduced zero-time contacts that interrupted wall jumps. Ground crouch timers, special duck-jump eye states, mesh query fidelity, broader PHY formats and Havok/VPhysics solver equivalence remain unfinished.
