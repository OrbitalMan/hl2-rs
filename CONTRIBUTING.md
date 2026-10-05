# Contributing

HL2-RS is a partial standalone Rust reconstruction. A map loading successfully does not establish that its campaign logic works. Please scope changes to a concrete behavior and describe what remains approximate.

## Build and check

Follow the fresh-checkout instructions in README.md. Use the checked-in Cargo.lock, then run the relevant tests and checks:

```powershell
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
.\scripts\build.ps1
.\launch.cmd
```

The commands above build/test the retained runtime. For a Bevy host/renderer change on `bevy-migration`, package with `scripts/build-bevy.ps1`, test `launch-bevy.cmd`, and record `bin/build-bevy-info.json` including shader hashes. Read [migration instructions](docs/bevy-migration.md); testing `launch.cmd` does not exercise the new Bevy renderer.

Test renderer and input changes using the appropriate packaged executable. Reports and captures go in ignored artifacts/. Record the tested map, camera or reproduction steps, installed Steam build, and executable fingerprint (`bin/build-info.json` for the retained runtime). An installed-game comparison is valuable; distinguish observed behavior from SDK references and inference.

GitHub Actions runs formatting, strict workspace Clippy and unit tests on Windows for `main`/`bevy-migration` pushes and pull requests. Those checks require no installed game files. They do not replace packaged play tests against an owned installation.

## Code and distribution boundaries

- Put file-format readers in `source-assets`, common data/player/mod contracts in `modkit-core`, engine-independent physics/entities/NPCs/weapons/choreography in `hl2-simulation`, portable owned-resource HUD/menu/console logic in `hl2-ui`, retained rendering/input/host logic in `hl2-runtime`, and Bevy presentation/input/host adapters in `hl2-bevy`.
- Read the owned game installation in place. Keep game files, generated world exports, native binary copies, decompiler output and research tools outside the repository.
- Ship only project source and properly attributed open-source dependencies. Do not submit ripped assets, leaked source, game executables, credentials or private local paths.
- Add meaningful regression coverage for simulation/parser changes. A screenshot should illustrate a tested behavior, not stand in for a reproducible check.

See docs/research.md for references and docs/validation.md for tested behavior and limitations. Useful contributions include Source collision and movement fidelity, material rendering, NPC schedules/navigation, choreography, remaining weapons, and campaign state. Open a focused issue or PR with reproduction steps and supporting evidence.
## Branches during the Bevy migration

Target `bevy-migration` for migration work, its Bevy/wgpu host and improvements to the extracted shared crates. Target `main` for maintenance of its current partial runtime; discuss fixes needed on both branches so they can be applied and checked deliberately. `macroquad-prototype` preserves the pre-migration implementation for reference. Describe which branch/runtime you tested. Migration changes will move to `main` after verified replacements are available.

Animation contributions should preserve owned sequence bone weights, automatic child layers, additive/post-delta transforms and Faceposer gesture timing. The inspected station gestures have empty parent tracks and require their child layers; playing only the parent clip does not implement them. Keep decoded metadata and pose composition shared between hosts, and keep installed-file reads out of frame systems. See the retail eye-origin and gesture evidence in [research notes](docs/research.md). Head controls, facial animation, NPC schedules and the ordinary first-level campaign remain open work.
