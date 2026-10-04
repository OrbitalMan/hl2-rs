# Bevy migration preview

The `bevy-migration` branch adds a Bevy 0.19.1/wgpu host with custom Source materials and shared player movement/collision. It reads an owned, installed Half-Life 2 copy through `source-assets`; `modkit-core` provides the player controller and `hl2-simulation` provides shared collision support. Game assets and Valve DLLs are not included in the repository or package.

This Bevy executable defaults to **walking with collision**, using the retained `Player` and `Physics` code at 15 ms per step. Flight is available with `--fly` or F2. The retained entity I/O, choreography scheduler, conservative Barney locomotion, inventory, six implemented weapons and projectile simulation now run in the Bevy fixed step. Door/prop meshes follow collider poses; available skeletal clips and weapon viewmodels animate. HUD, audio, impact decals, projectile visuals and campaign transitions still need Bevy presentation/host adapters. The underlying incomplete AI and scene behavior is preserved, not upgraded to full Source parity. Visible geometry and working movement do not establish playable-map support or Source shader parity. The existing runtime remains available through `scripts/build.ps1` and `launch.cmd` for its broader implemented behavior.

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

Click the window to capture the mouse and resume movement. WASD moves; Space jumps, Ctrl crouches, Shift sprints and Alt walks slowly. F2 toggles flight, where Space/Ctrl rises/falls and Shift increases speed; flight has no collision. F3 grants the retained developer loadout. Slots 1-6 and mouse wheel select weapons, either attack button confirms selection and is consumed until released. Left/right mouse fires primary/implemented secondary attacks, R reloads, Q selects the previous weapon, E uses a ray-targeted door/button and G applies a test prop impulse. Escape cancels an open selection first. Escape or loss of focus pauses movement and releases the cursor; a click resumes. F10 exits. A held jump across an interactive pause must be released before it can trigger again.

Without a movement script, `--capture` defaults to 120 frames when `--frames` is omitted. A bounded capture waits for GPU readback, exits, then writes the PNG and report. The report records the selected adapter/backend, camera, submitted geometry, asset failures, player/collision state and capture size. It does not certify shader or gameplay fidelity; inspect the picture and log.

## Tick-based movement fixture

Run the owned station bench fixture through the normal shared player/collision adapter:

```powershell
.\launch-bevy.cmd --map d1_trainstation_02 --movement-script test-inputs/bevy-movement.json --capture artifacts/bevy-movement.png --report artifacts/bevy-movement.json
```

The fixture settles onto the bench, compares an ordinary jump with airborne crouching/unducking, pauses/resumes while holding jump, then releases and presses jump again. Its commands use strictly increasing `tick` numbers, starting at 1, on the 15 ms host clock. Optional `eye` coordinates reset the player from an eye position; `yaw` is in degrees. Optional `paused` and `fly` change those modes. Each command replaces all held `forward`, `side`, `jump`, `crouch`, `sprint` and `slow` values; omitted held fields become zero/false. A `label` records player state after that tick's step. Paused host ticks still process commands and labels while skipping player movement, so the script can resume itself.

The report stores labeled samples under `render.simulation.samples`. After the final command, player movement freezes; the executable captures the final frame when requested and exits. Omit `--frames` for this fixture: a frame bound can terminate it early and produces an error if the script has not finished. Scripted movement runs independently of window focus and replaces interactive movement controls. These fixtures exercise the migrated adapter and do not demonstrate campaign progression.

## Architecture being migrated

Following iw4L's boundaries, Bevy owns the host lifecycle and scheduling, while custom material/render code retains Source-specific interpretation. Asset decoding remains outside GPU code. `modkit-core` keeps the Source-coordinate player controller; the engine-independent `hl2-simulation` crate shares the Rapier collision/rigid-body adapter and NPC probes between hosts. Its extraction preserves the retained code's limitations and does not establish Havok/VPhysics equivalence.

The Bevy adapter samples input/look before the fixed loop, advances walking at 15 ms per step and publishes the resulting eye position to the camera afterward. The fixed step preserves scene -> weapon/inventory -> entity collider poses/query refresh -> NPC/projectiles -> rigid bodies -> player ordering. Entity batches have ownership and local vertices; post-update publishes Source-coordinate scene/body poses and visibility before transform propagation. Existing CPU skinning updates animated mesh positions and bounds, skipping unchanged samples. A second camera uses the retained 54-degree reference viewmodel projection and an independent depth pass. Later entity/gameplay integration should publish presentation data for rendering rather than let draw systems mutate gameplay state. Fidelity and behavior still require direct comparisons with the owned original game.

The viewer currently uploads BSP/displacement triangles, baked LDR lightmaps, baked static props and selected entity meshes. A custom WGSL material handles base-texture UV transforms, tint, cutouts, translucency, additive output and two-sided flags through Bevy's depth-tested render phases. Static texture references share decoded and GPU images; immutable meshes/textures release their CPU copies after upload. Owned content is loaded before the app starts, and diagnostic files are written after it exits.

The decoded base-texture limit is **our loader's 512 MiB allocation budget**, not an engine limit. An early preview exceeded it by decoding material aliases separately and loading unused textures; shared caching and draw selection corrected that. The Bevy adapter currently selects existing VTF mips up to 2048 pixels; the retained viewer selects up to 512. Comparisons must account for this quality difference. These byte counts do not measure total RAM/VRAM or performance. Triangle winding is normalized only in BSP render copies before model appending: the model decoder already reverses VTX winding, and applying one native winding to both previously culled model fronts. Original collision copies remain intact.

This is approximate legacy gamma multiplication, not Source's complete material pipeline. Missing/dynamic render textures use a magenta checker and are named in the report. DX fallback blocks, `$color2`, animated material proxies, normal/specular maps, eyes, shadows, fog, refraction, HDR/exposure and both sky passes remain unsupported. Transparent surfaces inherited from the shared prebatched world can still contain disconnected faces, limiting depth sorting. Missing animation clips remain bind poses and are reported. Door/entity I/O and prop dynamics use the same incomplete retained simulation as the original host. Projectile simulation has no visible grenade/ball/effect adapter yet; audio requests are counted explicitly as unplayed. Unsupported scene/NPC inputs remain named diagnostics. Neither better performance nor 1:1 fidelity has been demonstrated.

## Contributor milestones before replacing main

1. Compare the custom renderer with matched original/retained-runtime cameras; restore sky visibility/masking, Source material stages and diagnostics without regressions.
2. Continue separating the existing gameplay host from Macroquad-specific types, preserving its behavior/tests and the migrated player/collision adapter's fixed-step scheduling.
3. Finish migrating HUD, audio, projectile/impact effects and campaign host transitions, then expand verified NPC/animation/choreography behavior. Preserve original-game evidence and packaged regressions for each subsystem.
4. Verify regressions for all retained runtime features and both trainstation levels through ordinary campaign state, document remaining gaps, and only then propose moving the replacement into `main`.

Target Bevy PRs at `bevy-migration`. Shared format/core fixes can target `main` and be brought across separately. The preserved `macroquad-prototype` branch is a reference snapshot, not the active migration target. Do not include game files, private native analysis or databases in a PR.

## Entity and weapon fixture

`launch-bevy.cmd --map d1_trainstation_02 --movement-script test-inputs/bevy-entities-weapons.json --capture artifacts/bevy-entities-weapons.png --report artifacts/bevy-entities-weapons.json` checks a closed/open/reclosed station doorway, E use, bucket confirmation, pistol primary fire, SMG grenade ammo and delayed AR2 launch. `actions` are one-shot commands; held `primary`/`secondary` replace the previous held state alongside movement. `send` targets named map entities for controlled setup. Deployment delays must elapse before firing. Labels include gameplay state and a 128-unit forward ray; the final report checks entity presentation against collider/scene poses. It does not prove an ordinary campaign playthrough.
