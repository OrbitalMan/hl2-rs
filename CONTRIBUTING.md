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

Test renderer and input changes using the packaged executable launched by launch.cmd. Reports and captures go in ignored artifacts/. Record the tested map, camera or reproduction steps, installed Steam build, and executable fingerprint from bin/build-info.json. An installed-game comparison is valuable; distinguish observed behavior from SDK references and inference.

## Code and distribution boundaries

- Put file-format readers in source-assets, shared simulation and mod contracts in modkit-core, and rendering/input/host logic in hl2-runtime.
- Read the owned game installation in place. Keep game files, generated world exports, native binary copies, decompiler output and research tools outside the repository.
- Ship only project source and properly attributed open-source dependencies. Do not submit ripped assets, leaked source, game executables, credentials or private local paths.
- Add meaningful regression coverage for simulation/parser changes. A screenshot should illustrate a tested behavior, not stand in for a reproducible check.

See docs/research.md for references and docs/validation.md for tested behavior and limitations. Useful contributions include Source collision and movement fidelity, material rendering, NPC schedules/navigation, choreography, remaining weapons, and campaign state. Open a focused issue or PR with reproduction steps and supporting evidence.
