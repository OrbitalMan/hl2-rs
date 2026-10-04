# Bevy migration preview

The `bevy-migration` branch adds a Bevy 0.19.1/wgpu host with a custom Source-material map viewer. It reads an owned, installed Half-Life 2 copy through the existing `source-assets` and `modkit-core` crates. Game assets and Valve DLLs are not included in the repository or package.

This Bevy executable is a **static renderer with a fly camera**. It does not yet run player collision/movement, weapons, NPC AI, audio, animated entity state, scripted scenes, entity I/O or campaign progression. Visible geometry does not establish playable-map support or Source shader parity. The existing runtime remains available through `scripts/build.ps1` and `launch.cmd`; its implemented behavior has not yet been migrated into Bevy.

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
```

Camera and bounded capture options pass directly to the runtime:

```powershell
.\launch-bevy.cmd --map d1_trainstation_01 --position -3400 -420 32 --yaw 180 --pitch 0 --frames 120 --capture artifacts/bevy-station.png --report artifacts/bevy-station.json
```

`--position` takes three Source-coordinate numbers. Yaw and pitch are in degrees. Captures and reports are local development artifacts. A static render comparison does not validate simulation, NPC behavior, audio or campaign completeness.

Click the window to capture the mouse. WASD flies; Space/Ctrl rises/falls; Shift increases speed. Escape releases the cursor and F10 exits. Fly movement has no collision. `--capture` defaults to 120 frames when `--frames` is omitted. A bounded capture waits for GPU readback, exits, then writes the PNG and report. The report records the selected adapter/backend, camera, submitted geometry, asset failures and capture size. It does not certify shader or gameplay fidelity; inspect the picture and log.

## Architecture being migrated

Following iw4L's boundaries, Bevy owns the host lifecycle and scheduling, while custom material/render code retains Source-specific interpretation. Asset decoding remains outside GPU code, and the common data/simulation contracts remain independent of Bevy. Future simulation integration should publish presentation data for rendering rather than let draw systems mutate gameplay state. The preview is the first host/rendering milestone; fidelity and behavior still require direct comparisons with the owned original game.

The viewer currently uploads BSP/displacement triangles, baked LDR lightmaps, baked static props and selected entity meshes. A custom WGSL material handles base-texture UV transforms, tint, cutouts, translucency, additive output and two-sided flags through Bevy's depth-tested render phases. Static texture references share decoded and GPU images; immutable meshes/textures release their CPU copies after upload. Owned content is loaded before the app starts, and diagnostic files are written after it exits.

The decoded base-texture limit is **our loader's 512 MiB allocation budget**, not an engine limit. An early preview exceeded it by decoding material aliases separately and loading unused textures; shared caching and draw selection corrected that. The Bevy adapter currently selects existing VTF mips up to 2048 pixels; the retained viewer selects up to 512. Comparisons must account for this quality difference. These byte counts do not measure total RAM/VRAM or performance. Triangle winding is normalized only in BSP render copies before model appending: the model decoder already reverses VTX winding, and applying one native winding to both previously culled model fronts. Original collision copies remain intact.

This is approximate legacy gamma multiplication, not Source's complete material pipeline. Missing/dynamic render textures use a magenta checker and are named in the report. DX fallback blocks, `$color2`, animated material proxies, normal/specular maps, eyes, shadows, fog, refraction, HDR/exposure and both sky passes remain unsupported. Transparent surfaces inherited from the shared prebatched world can still contain disconnected faces, limiting depth sorting. Animated doors/models use authored initial transforms/bind poses; runtime entity state and collision are not migrated. Neither better performance nor 1:1 fidelity has been demonstrated.

## Contributor milestones before replacing main

1. Compare the custom renderer with matched original/retained-runtime cameras; restore sky visibility/masking, Source material stages and diagnostics without regressions.
2. Separate the existing gameplay host from Macroquad-specific types, preserve its behavior/tests, and integrate fixed-step simulation with Bevy resources/components and explicit scheduling.
3. Migrate player/input, weapons/HUD, collision/props, entity I/O, animation/choreography and audio. Preserve original-game evidence and packaged regressions for each subsystem.
4. Verify regressions for all retained runtime features and both trainstation levels through ordinary campaign state, document remaining gaps, and only then propose moving the replacement into `main`.

Target Bevy PRs at `bevy-migration`. Shared format/core fixes can target `main` and be brought across separately. The preserved `macroquad-prototype` branch is a reference snapshot, not the active migration target. Do not include game files, private native analysis or databases in a PR.
