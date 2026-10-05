# Bevy migration preview

The `bevy-migration` branch adds a Bevy 0.19.1/wgpu host with custom Source materials and shared player movement/collision. It reads an owned, installed Half-Life 2 copy through `source-assets`; `modkit-core` provides the player controller and `hl2-simulation` provides shared collision support. Game assets and Valve DLLs are not included in the repository or package.

This Bevy executable defaults to **walking with collision**, using the retained `Player` and `Physics` code at 15 ms per step. Flight is available with `--fly` or F2. The retained entity I/O, choreography scheduler, conservative Barney locomotion, inventory, six implemented weapons and projectile simulation now run in the Bevy fixed step. Door/prop meshes follow collider poses; available skeletal clips and weapon viewmodels animate. The shared owned-resource HUD now presents health/ammunition, weapon buckets, quick-info and white/yellow crosshairs, including secondary-ammo positioning animations. Owned ambient, scene, weapon, animation-event and HUD cues now play through Bevy audio; existing sinks pause/resume with the host. Owned grenade models, energy-ball sprites, retained explosion sprites and clipped impact marks now have Bevy presentation adapters. The portable pause menu/console and asynchronous map host now run in Bevy. Authored triggers preserve landmark-relative position and inventory; console `map` resets them. Saved global/entity state and complete player transfer remain unfinished. The underlying incomplete AI and scene behavior is preserved, not upgraded to full Source parity. Visible geometry and working movement do not establish playable-map support or Source shader parity. The existing runtime remains available through `scripts/build.ps1` and `launch.cmd` for its broader implemented behavior.

## Build and launch

Use Windows 64-bit, Rust 1.95 or later with the MSVC toolchain, Visual Studio C++ build tools and a Windows SDK. Development used Rust 1.99.0. GPU compatibility and minimum memory requirements for the Bevy viewer have not been established. The tested owned installation is Steam app 220, build 19307283, patch 9912070; other builds are unverified.

From the checkout root:

```powershell
.\scripts\build-bevy.ps1
.\launch-bevy.cmd --map d1_trainstation_01
```

`-DebugBuild` selects a debug build. Cargo uses the checked-in lockfile and builds only `hl2-bevy`. Packaging writes `bin/hl2-bevy.exe`, copies the viewer's shader assets into `bin/bevy-assets`, and records their hashes in `bin/build-bevy-info.json`. Keep the executable and shader folder together. The launcher requires the packaged executable; rebuilding is necessary after changing source or shaders.

The runtime discovers the installed game. To select a particular installation or display size:

```powershell
.\launch-bevy.cmd --game "C:\Program Files (x86)\Steam\steamapps\common\Half-Life 2" --map d1_trainstation_02
.\launch-bevy.cmd --borderless
.\launch-bevy.cmd --width 1920 --height 1080
.\launch-bevy.cmd --fly
```

Camera and bounded capture options pass directly to the runtime:

```powershell
.\launch-bevy.cmd --map d1_trainstation_01 --fly --position -3400 -420 32 --yaw 180 --pitch 0 --frames 120 --capture artifacts/bevy-station.png --report artifacts/bevy-station.json
```

`--position` takes three Source-coordinate **eye-position** numbers. Yaw and pitch are in degrees. Captures and reports are local development artifacts. A render comparison does not validate NPC behavior, audio or campaign completeness.

Click the window to capture the mouse and resume movement. WASD moves; Space jumps, Ctrl crouches, Shift sprints and Alt walks slowly. F2 toggles flight, where Space/Ctrl rises/falls and Shift increases speed; flight has no collision. F3 grants the retained developer loadout. Slots 1-6 and mouse wheel select weapons, either attack button confirms selection and is consumed until released. Left/right mouse fires primary/implemented secondary attacks, R reloads, Q selects the previous weapon, E uses a ray-targeted door/button and G applies a test prop impulse. Escape cancels an open selection first. Escape or loss of focus opens the pause menu and releases the cursor. Use mouse/arrow keys and Enter on Resume, Console or Quit; clicks outside the menu keep it paused. Tilde toggles the developer console, which remembers whether it opened from gameplay or the pause menu. F10 exits. A held jump across an interactive pause must be released before it can trigger again.

Without a movement script, `--capture` defaults to 120 frames when `--frames` is omitted. A bounded capture waits for GPU readback, exits, then writes the PNG and report. The report records the selected adapter/backend, camera, submitted geometry, asset failures, player/collision state and capture size. It does not certify shader or gameplay fidelity; inspect the picture and log.

## Tick-based movement fixture

Run the owned station bench fixture through the normal shared player/collision adapter:

```powershell
.\launch-bevy.cmd --map d1_trainstation_02 --movement-script test-inputs/bevy-movement.json --capture artifacts/bevy-movement.png --report artifacts/bevy-movement.json
```

The fixture settles onto the bench, compares an ordinary jump with airborne crouching/unducking, pauses/resumes while holding jump, then releases and presses jump again. Its commands use strictly increasing `tick` numbers, starting at 1, on the 15 ms host clock. Optional `eye` coordinates reset the player from an eye position; `yaw` is in degrees. Optional `paused` and `fly` change those modes. Each command replaces all held `forward`, `side`, `jump`, `crouch`, `sprint` and `slow` values; omitted held fields become zero/false. A `label` records player state after that tick's step. Paused host ticks still process commands and labels while skipping player movement, so the script can resume itself.

The report stores labeled samples under `render.simulation.samples`. After the final command, player movement freezes; the executable captures the final frame when requested and exits. Omit `--frames` for this fixture: a frame bound can terminate it early and produces an error if the script has not finished. Scripted movement runs independently of window focus and replaces interactive movement controls. These fixtures exercise the migrated adapter. The separate campaign fixture below tests controlled trigger transitions, not an ordinary campaign playthrough.

## Architecture being migrated

Following iw4L's boundaries, Bevy owns the host lifecycle and scheduling, while custom material/render code retains Source-specific interpretation. Asset decoding remains outside GPU code. `modkit-core` keeps the Source-coordinate player controller; the engine-independent `hl2-simulation` crate shares the Rapier collision/rigid-body adapter and NPC probes between hosts. Its extraction preserves the retained code's limitations and does not establish Havok/VPhysics equivalence.

The Bevy adapter samples input/look before the fixed loop, advances walking at 15 ms per step and publishes the resulting eye position to the camera afterward. The fixed step preserves scene -> weapon/inventory -> entity collider poses/query refresh -> NPC/projectiles -> rigid bodies -> player ordering. Entity batches have ownership and local vertices; post-update publishes Source-coordinate scene/body poses and visibility before transform propagation. Existing CPU skinning updates animated mesh positions and bounds, skipping unchanged samples. A second camera uses the retained 54-degree reference viewmodel projection and an independent depth pass. Later entity/gameplay integration should publish presentation data for rendering rather than let draw systems mutate gameplay state. Fidelity and behavior still require direct comparisons with the owned original game.

The viewer currently uploads BSP/displacement triangles, baked LDR lightmaps, baked static props and selected entity meshes. A custom WGSL material handles base-texture UV transforms, tint, cutouts, translucency, additive output and two-sided flags through Bevy's depth-tested render phases. Static texture references share decoded and GPU images; immutable meshes/textures release their CPU copies after upload. Initial owned content is loaded before the app starts. Subsequent maps decode on a worker, with fixed simulation/script ticks frozen until completion; GPU upload and resource replacement occur in the host. Diagnostic files are written after it exits.

The decoded base-texture limit is **our loader's 512 MiB allocation budget**, not an engine limit. An early preview exceeded it by decoding material aliases separately and loading unused textures; shared caching and draw selection corrected that. The Bevy adapter currently selects existing VTF mips up to 2048 pixels; the retained viewer selects up to 512. Comparisons must account for this quality difference. These byte counts do not measure total RAM/VRAM or performance. Triangle winding is normalized only in BSP render copies before model appending: the model decoder already reverses VTX winding, and applying one native winding to both previously culled model fronts. Original collision copies remain intact.

This is approximate legacy gamma multiplication, not Source's complete material pipeline. Missing/dynamic render textures use a magenta checker and are named in the report. Other DX fallback blocks, `$color2`, animated material proxies, normal/specular maps, shadows, fog, refraction and HDR/exposure remain unsupported. Eyes now use owned sclera/iris textures and studio projection; EyeRefract selects its authored Eyes_dx8 fallback, rather than implementing corneal refraction. Transparent surfaces inherited from the shared prebatched world can still contain disconnected faces, limiting depth sorting. Missing animation clips remain bind poses and are reported. Door/entity I/O and prop dynamics use the same incomplete retained simulation as the original host. Owned projectile/impact presentation and actual audio sinks are connected; native particle and audio fidelity remain incomplete. Unsupported scene/NPC inputs remain named diagnostics. Neither better performance nor 1:1 fidelity has been demonstrated.

## Contributor milestones before replacing main

1. Compare the custom renderer with matched original/retained-runtime cameras; restore sky visibility/masking, Source material stages and diagnostics without regressions.
2. Continue separating the existing gameplay host from Macroquad-specific types, preserving its behavior/tests and the migrated player/collision adapter's fixed-step scheduling.
3. Verify migrated pause/console, projectile/impact effects and campaign transitions against the retained host, then expand NPC/animation/choreography and saved campaign state. Preserve original-game evidence and packaged regressions for each subsystem.
4. Verify regressions for all retained runtime features and both trainstation levels through ordinary campaign state, document remaining gaps, and only then propose moving the replacement into `main`.

Target Bevy PRs at `bevy-migration`. Shared format/core fixes can target `main` and be brought across separately. The preserved `macroquad-prototype` branch is a reference snapshot, not the active migration target. Do not include game files, private native analysis or databases in a PR.

## Entity and weapon fixture

`launch-bevy.cmd --map d1_trainstation_02 --movement-script test-inputs/bevy-entities-weapons.json --capture artifacts/bevy-entities-weapons.png --report artifacts/bevy-entities-weapons.json` checks a closed/open/reclosed station doorway, E use, bucket confirmation, pistol primary fire, SMG grenade ammo and delayed AR2 launch. `actions` are one-shot commands; held `primary`/`secondary` replace the previous held state alongside movement. `send` targets named map entities for controlled setup. Deployment delays must elapse before firing. Labels include gameplay state and a 128-unit forward ray; the final report checks entity presentation against collider/scene poses. It does not prove an ordinary campaign playthrough.

## Shared HUD and capture fixture

`hl2-ui` keeps the retained scheme/resource parsing, Windows font-cell metrics, glyph blur/scanlines, numeric animation rules, weapon bucket state and default-sprite remapping. Its explicit CPU canvas records ordered texture/rectangle commands; asset reads occur before the Bevy app starts. Bevy uses a separate layer/camera and custom Material2d for normal/additive draws, retaining CPU glyph ownership and pooling meshes. The retained Macroquad host now consumes the same canvas. This is a host migration, not a redesign of the original interface.

```powershell
.\launch-bevy.cmd --map d1_trainstation_02 --movement-script test-inputs/bevy-hud.json --capture artifacts/bevy-hud.png --report artifacts/bevy-hud.json
```

The fixture starts unarmed, equips a pistol, opens selection while retaining pistol ammo, confirms SMG, waits for the secondary panel to move, fires a grenade and ends with shotgun selected in the menu while SMG ammo remains active. It pauses on its last tick for a stable capture. HUD draw count, pooled meshes, owned GPU texture count and viewport are reported under `render.presentation.hud`.

White-crosshair packaged raster measurements match the accepted retail five single-pixel offsets at 1280x720 and borderless 1920x1080. Resource-driven secondary panels were inspected at start/intermediate/end positions. Native GDI rasterization, complete HUD hide gates/commands, exact blend/gamma behavior and performance remain unverified; Bevy blends in its linear output pipeline. The pause/console uses the same portable canvas, with its supported subset documented below.

## Shared audio

`hl2-simulation::sounds` retains the sound manifest, gender/actor-aware variant selection, WAV/MS ADPCM and MP3 decoding, and viewmodel event cursor. Both hosts use it. Bevy preloads referenced waves before its app starts and sends requests to AudioPlayer entities after HUD presentation. The report distinguishes requests from actual AudioSink creation and records device failures, pending/paused sinks and selected paths. Preloading has a 512 MiB allocation budget and does not advance variant selection.

`test-inputs/bevy-audio-scenes.json` is a controlled first-map security-room speech and pause/resume fixture; it bypasses unsupported scene movement and does not establish campaign progression. Two-dimensional mixing keeps the retained volume policy; Source distance attenuation, DSP, soundscapes, voice channels, lipsync and native RNG are unfinished. Some unused installed scene references are absent and remain named preload diagnostics rather than silently omitted. Linux audio/build behavior is unverified; the small quad-alsa-sys forwarding shim lets both hosts resolve the same upstream ALSA binding package.

## Sky, eyes and door direction

The sky adapter restores the retained six-face LDR orientation/inset/UV transforms, center-only camera translation, miniature sky-camera origin/scale, and separate BSP leaf eligibility. Background cameras render before the main world with independent depth clearing. Closed walls/doors occlude the sky; native sky polygon masks, HDR and fog remain unfinished.

Eye metadata maps mesh materialtype/materialparam to the authored MDL eyeball, bone, origin, up/forward, iris scale and divergence. CPU iris UVs follow the current bone pose and feed a separate owned iris sample over the sclera. Default planar projection was checked against the owned native StudioRender database and published Eyes shaders. Glints, eyelid/facial flexes, nondefault eye-size/shift parameters and native model lighting remain unfinished. Compiled VCD LOOKAT now resolves player/self/named/target-slot references at event start and refreshes shared timed explicit interests. Both eyes consume one selected world target for the actor. Normalized-X Catmull-Rom event/scene ramps and the first 0.3-second importance limit are implemented. Scene pause refreshes interests at held scene time; UI pause freezes global simulation; cancellation leaves the existing 0.1-second tail to expire. Explicit targets use the published eye-direction gate, without the random-interest distance/visibility filters. Outside valid scripted interests, the bounded nearest-visible player/NPC fallback remains. Actor origin/head direction now use the bounded owned eyes attachment, or the model view offset when absent. Eye presentation samples each actor rig once and shares it between target and iris consumers, including cycler eye meshes. Native random/tactical/synthetic queues, previous-target retention, gaze smoothing, head poses, PVS-dependent attachment latching and facial animation remain unfinished.

Using linked prop doors passes the same opener origin to both leaves, so their opposite authored orientations produce opposite signed swing offsets and move both panels away from the player. Explicit opendir selects forward/backward; direct map Open retains the default forward direction. Locks and use-close flags remain active. Obstruction reversal/blockers, master/slave ownership, complete door animation/sound behavior and all spawn-position modes remain incomplete. `test-inputs/bevy-door-swing.json` checks use from both sides and returning to the authored closed pose; it ends on the outside opening for a stable capture.

The shared `OpenAwayFrom` prop-door input accepts a named opener or `!player`/`!activator`/`!caller`, resolving current origins at delivery. Run `test-inputs/bevy-door-inputs.json` with `--movement-script` for its entrance fixture. Repeated open inputs retain an existing swing; native linkage ownership and blocked-door reversal remain unfinished.

## Shared projectile and impact presentation

`hl2-simulation::projectile_visuals` retains the sprite geometry, flicker and rendered-displacement blur state. Bevy pools depth-tested billboard meshes and instances the owned SMG grenade model. Shared impact selection preloads surface metadata, textures and sound alternatives; firing performs no asset reads. Marks are clipped to current posed triangles, kept in receiver-local coordinates and presented using its current rigid/entity transform. A bounded 1.5 Source-unit same-receiver projection handles small collision-hull gaps. Marks do not deform with later skeletal animation; full Source studio decals remain unfinished.

Run the `test-inputs/bevy-effect-{grenade,ball,decal,explosion,decal-door}.json` fixtures through `--movement-script` for stable captures. Each ends paused. The report includes observed grenade/decal meshes and pose disagreement counts, so zero mismatches alone cannot hide absent draws. The owned installation lacks base textures for five unused sand/shot material names; these remain explicit preload errors. Native particles, dynamic lights, beam rings/sparks, exact blend/gamma behavior and projectile/model shading are still incomplete.

## Shared pause menu, console and map host

`hl2-ui::console` preserves the retained parser, bounded history, Unicode editing, completion, cheat gates and resource-based menu/title layout. Both hosts supply explicit key/text/pointer input and draw through the CPU canvas. Resume, Console and Quit are implemented; save/load/options and the complete Source command registry are unfinished. `help` lists supported commands, including sv_cheats, impulse 101, noclip, getpos, setpos, setang, ent_fire and map. Unknown commands report that they are unsupported. Opening/closing the UI consumes transition clicks/attacks and requires held jump to be released; existing audio sinks pause with simulation.

```powershell
.\launch-bevy.cmd --movement-script test-inputs/bevy-console.json --capture artifacts/bevy-console.png --report artifacts/bevy-console.json
.\launch-bevy.cmd --map d1_trainstation_01 --movement-script test-inputs/bevy-campaign.json --capture artifacts/bevy-campaign.png --report artifacts/bevy-campaign.json
.\launch-bevy.cmd --movement-script test-inputs/bevy-console-map.json --capture artifacts/bevy-console-map.png --report artifacts/bevy-console-map.json
```

The console fixture checks cheating, editing, selection cancellation and pause/resume. Optional `ui` contains one-shot `escape`, `toggle_console`, `resume`, `command` (with `command` text) or `input` (with nested explicit `input` data). The campaign fixture teleports into the real station01 and station02 exits, verifying station03 arrival and carried ammunition; it deliberately bypasses unfinished campaign scenes. Input-only changelevel brushes (spawn flag 0x2) accept ChangeLevel but do not load on touch. The console-map fixture loads two owned maps and requests a missing map between them, preserving the current world on failure and retaining console history/cheat state on success. Direct `map` uses new-game scene initialization and a fresh inventory; a trigger transition suppresses new-game outputs and rebases weapon deadlines to the new scene clock.

Map-owned meshes, sky cameras, HUD/effect pools and audio entities are removed on success before new resources/draws are installed. The report includes live asset/camera counts and current-map metadata. This is bounded host integration, not complete Source transition/save behavior: entity/global state, full player motion/crouch transfer, save restoration and complete campaign progression remain unfinished. Interactive font rasterization and all native UI behavior are not claimed identical.

## Authored NPC attention

Run test-inputs/bevy-attention.json with --map d1_trainstation_01 --movement-script, a --capture path and a --report path. The retained equivalent is test-inputs/attention-lookat.json, used with --input-script and --capture.

These fixtures isolate the installed security02 scene, disable unrelated entry triggers before entering the room, and deliberately seed Barney's position/orientation with the developer actor_pose action. This bypasses unfinished campaign actor placement; it is not an ordinary playthrough. Bevy samples expose interest sources, importance, timing and selected eye targets. The retained host executes the shared queue and reports it but does not have the Bevy iris adapter. LOOKAT does not implement FACE, gestures, eyelid/facial flexes or lipsync. SDK reference checks do not prove equivalence to retail AI scheduling. Text VCD custom curve/edge overrides are outside the compiled BVCD reader.
