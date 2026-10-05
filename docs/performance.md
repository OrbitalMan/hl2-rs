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

## BSP PVS follow-up

Debug compilation was not the cause of this regression: the packaged executable is release-built, and development dependencies already have optimization overrides. Profiling instead showed substantial work preparing and binding geometry outside the current room. BSP PVS rejection now supplements Bevy's frustum culling. It uses a validated cached tree, padded current bounds, cached cluster rows and the union of active player/monitor cameras. It never suspends gameplay simulation or bases animation decisions on the previous frame's visibility.

Matched-package 900-frame uncapped/profiled station02 spawn runs at1920x1080, with120 warm-up frames, give:

| Measurement | PVS disabled | PVS enabled |
| --- | ---: | ---: |
| Average FPS | 66.69 | 156.94 |
| Average frame, ms | 14.995 | 6.372 |
| Render preparation CPU, ms | 5.857 | 1.518 |
| Render graph CPU wall interval, ms | 7.392 | 3.318 |
| Source visibility CPU, ms | 0.009 | 0.032 |

PVS rejects824 of886 tagged draws at this position. An inspected spawn comparison is pixel-identical.798 bounds checks were reusable at the final sample; the initial implementation retraversed every bound and cost about0.96ms per frame. The loaded mesh/material counts stay fixed across900 frames; they are not visible draw counts. Finer render preparation timers identify phase-buffer writes and bind-group preparation as major costs reduced by rejection.

Reproduce with `launch-bevy.cmd --width 1920 --height 1080 --frames 900 --uncapped --profile --report artifacts/pvs-on.json`; add `--no-pvs` for the reference run. Do not compile or run another host concurrently with a benchmark. The camera is stationary but autonomous scene time advances: faster runs cover fewer simulation seconds. These are room-specific measurements with profiling overhead, not a matched native campaign benchmark or proof of sustained144 FPS. Area portals/occluders, Source LODs and spatial partitioning of broad material batches still need work.

## Shared material handles

The map adapter now reuses a material handle for the same authored material and lightmap pair. Owner transforms, skeletons and eye UVs remain independent. Current implemented monitor proxies depend on the authored definition and shared scene clock; each shared handle is evaluated once, and unchanged proxy values no longer dirty the GPU asset. Future entity-specific material inputs will require separate handles or copy-on-write.

Station02 material assets fall from921 to743 with all921 draw meshes retained. Station01 uses695 material assets for1109 meshes. The reported material count now counts unique map material handles; it is not a visible draw or texture count. The station02 spawn image and Breen feed are pixel-identical to preceding captures. A matching Barney player-interest image crop is also exact, with unchanged per-actor eye origins/targets/projections. Global eye reports include the intervening Kleiner idle fix, so they are not claimed identical to pre-activity reports.

A sequential900-frame1080p uncapped/profiled spawn run averages174.72FPS/5.723ms after120 warm-up frames, compared with154.96FPS/6.453ms in the preceding activity package. Asset preparation averages0.159ms versus0.541ms; render preparation1.476ms versus1.521ms. This is an observed package comparison in one room, with autonomous scene time and normal run-to-run variation, not a whole-campaign144FPS guarantee. No simulation, texture resolution, model geometry or gaze updates were disabled.

## Optional spatial batches

`--world-partition` groups whole opaque/cutout world triangles into1024-unit cells during map setup. It preserves vertex attributes and winding, retains actual bounds for triangles spanning cells, and leaves entity, sky and translucent batches unchanged. Invalid data or more than64 cells falls back to the original batch. It is disabled by default pending broader benchmarks.

Station02 retains324429 triangles and743 materials, while loaded draw meshes rise921→1670. In the inspected plaza, union-of-view candidates fall69425→32011 triangles; its image is pixel-identical. These are candidates after PVS/frustum/layer checks, excluding sky/viewmodels, not GPU draw-call or fragment counts. Preliminary sequential900-frame1080p profiles measured159.74→165.91FPS at spawn and125.65→127.01FPS in the plaza; normal run variation and different autonomous scene durations prevent a robust general speed claim. A later frozen-scene diagnostic intentionally ends before its script completes and returns an incomplete-script error; it is excluded from these comparisons.

An initial hall camera intersected a pillar and was rejected as visibility evidence. Area portals, occluders, native LODs and campaign-wide culling validation remain unfinished.
