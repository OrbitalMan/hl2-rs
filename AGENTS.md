# Project context

Read README.md and docs/research.md before extending the runtime. This project is an experimental partial Rust reconstruction, not a completed campaign rewrite.

Keep the installed game read-only. Work on private DLL copies for binary analysis. Keep decompiler output and original assets outside the source tree; generated exports and screenshots belong in ignored artifacts/. Do not call FAL; the user has no credits and wants to reuse installed assets.

Keep decompiler scripts, native binary indices and research-only asset inspection helpers outside this repository too, in ../../work/hl2-decompiled. The runtime is Rust code reading owned assets in place; its scripts/ folder contains build tooling only. Package with scripts/build.ps1 and test bin/hl2-rs.exe through launch.cmd, rather than leaving the shipped binary stale after testing target/debug.

Put Source-format code in source-assets, common data/player/mod contracts in modkit-core, shared physics, entities/choreography, weapons/projectiles, selection and NPC orchestration in hl2-simulation, and retained renderer/input/host behavior in hl2-runtime. Keep hl2-simulation independent of Macroquad and Bevy. Do not introduce engine-specific file paths into common mod code.

On bevy-migration, put the experimental Bevy host/render adapter in hl2-bevy. Preserve the existing runtime and shared validation. Package the new executable with scripts/build-bevy.ps1 and test bin/hl2-bevy.exe through launch-bevy.cmd. Keep macroquad-prototype as the preserved reference; merge replacements into main only after they are verified. Read docs/bevy-migration.md before extending this host.

The Bevy host now defaults to walking with the retained Player controller and shared Physics queries at 15 ms. Sample input before the fixed loop and publish camera state afterward, preserving held-jump/crouch semantics and Source feet/eye coordinates. --fly/F2 enables flight; Escape/focus loss pauses and releases the cursor, and clicking resumes. --movement-script uses tick-based commands and labeled state samples; test-inputs/bevy-movement.json is the owned bench fixture. Its final command freezes movement before an optional end capture. The shared Gameplay resource advances scene -> weapons -> entity collider poses/query refresh -> NPC/projectiles -> physics -> player. PostUpdate publishes owned local-mesh transforms/visibility before Bevy transform propagation and samples skeletal/viewmodel animation with updated bounds. Keep this order. F3/slots/wheel/fire/reload/E/G are bridged; HUD, audio, impacts/projectile visuals and campaign transitions are still pending. test-inputs/bevy-entities-weapons.json covers doors/use/weapon timing. Do not call this a complete playable campaign.

Use Cargo.lock. Run relevant unit tests and Clippy after code changes. Verify parser changes against installed maps when available; the local zero-byte d2_coast_02.bsp is a known rejection, not proof of a parser regression. Run the packaged executable for renderer changes. Record limitations and concrete validation in docs/validation.md.

The three upstream repos are research references under the chat's work/references. There is no integrated iw4L crossover yet. Do not describe placeholders or guessed native function names as verified behavior.
