# Performance measurements - 2026-10-05

Performance is a current limitation, not a completed migration milestone. The user reported about22 FPS versus about60 in the retained build. A local packaged station02 spawn run reproduced about30 FPS at1920x1080 on the RTX3070/Vulkan adapter. The hosts use different texture mip limits (Bevy2048, retained512); neither a matched native benchmark nor144 FPS is established.

Optional `--profile` measures bounded CPU scopes after the first30 calls. It does not measure GPU execution or the entire render-thread cost. The same600-frame default-VSync scene before/after changed-collider query refresh and sampled reports measured:

| CPU scope | Before mean ms | After mean ms |
| --- | ---: | ---: |
| Fixed simulation, per15ms tick | 4.02 | 2.61 |
| Full diagnostics, per presentation | 1.39 | 0.022 |
| CPU mesh animation, per presentation | 5.79 | 5.75 |

Frame rate remained about30.2 versus30.6 FPS. A subsequent constant-pose upload fix plus `--uncapped` measured about35.1 FPS with animation4.63ms. This last run also changes presentation mode and is not an isolated FPS comparison for that fix. The remaining render/animation cost needs investigation. Do not infer a144 FPS ceiling or promise that this build reaches it.

Physics previously rebuilt the query tree twice every fixed step and marked unchanged poses as moved. It now incrementally refreshes actual changes;400 swept collision comparisons match full rebuilds across moves, rotations and visibility changes. Animated surfaces share rig samples, and constant poses no longer dirty/upload their meshes every frame. Full diagnostic snapshots are sampled every60 frames and refreshed for scripted/bounded capture completion and UI quit.

Reproduce with `launch-bevy.cmd --width 1920 --height 1080 --frames 600 --profile --report artifacts/performance.json`. Add `--uncapped` to request no-VSync presentation. Capture/JSON writes occur after shutdown. Current priorities are GPU upload/render-stage measurements, visible/PVS animation workload and a verified GPU skinning path. Preserve animation/eye/collision behavior when optimizing; do not lower fidelity silently to improve a benchmark.

## GPU skinning follow-up

The packaged binary uses Cargo's **release** profile. Development dependencies already have optimization overrides; changing those overrides does not change the release executable. The regression was reproduced in release mode.

Source still samples the same engine-independent animation clips. The Bevy adapter now sends joints and inverse bind poses to Bevy's GPU skinning pipeline instead of replacing each mesh's vertex positions every frame. Bone matrices include the Source-to-Bevy basis and model scale; weights preserve the CPU adapter's normalization and bind fallback. Surfaces of one actor share a skeleton. Models beyond Bevy's 256-joint budget retain CPU skinning. Eye UVs use the same sampled pose, and dynamically transformed joint bounds conservatively enclose animated vertices for frustum culling. Unchanged entity transforms are no longer marked dirty each frame.

`--cpu-skinning` selects the previous CPU path for controlled comparisons. `--profile` now records render-stage CPU wall times and Bevy's GPU/pass diagnostics; profiling adds timing fences and GPU queries, and per-pass GPU aggregates combine views with the same diagnostic name. They are not a complete GPU-frame measurement. The HUD report includes an average after 120 warm-up frames alongside its rolling one-second FPS.

The first controlled GPU run reached roughly 68 FPS uncapped at 1920x1080, versus roughly 35 with CPU skinning. CPU animation was about 0.25 ms and asset preparation about 0.56 ms, versus 4.77 and 11.60 ms. A subsequent matched-package 900-frame run averages 35.05 FPS with CPU skinning and 66.85 FPS with GPU skinning after 120 warm-up frames. See validation for scope and limitations. This is a substantial improvement, not verification of 144 FPS or Source performance parity.

Two controlled station01 close-ups compare Barney's monitor and player LOOKAT interests on both paths. Eye targets/projections agree exactly in reports, and the actor image region differs by less than 0.006 mean channel values on the 0-255 scale. Dynamic physics props elsewhere can differ between independent runs; whole-image identity is not claimed. This validates the migrated presentation against the previous Rust adapter, not the original engine's complete animation behavior.
