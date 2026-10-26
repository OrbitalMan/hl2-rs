# Performance measurements - 2026-10-05

## Flashlight on the weapon (2026-10-25)

Packaged Metal, Apple M1 Pro, `d1_trainstation_02`, 1920x1080, HDR Full, Medium shadows.
Clear close-wall view: eye -4324 -224 64, yaw 180, pitch 0; pistol selected at tick 20.
Baseline: `cdaf4be`; new package: this continuation atop it. Three alternating runs per
build/state, with rotated order, no concurrent builds or other agent replays. Scripts end
at tick 900 (13.5 simulated seconds), with a 2,000-frame safety cap. Each report confirms
completion and at least 300 measured frames. Actual reports contain 559-574 measured
frames after 120 warm-up frames (679-694 total), about 10.9-11.1 measured seconds per run.

| State | Prior mean frame | New mean frame | Prior model-light CPU | New model-light CPU |
| --- | ---: | ---: | ---: | ---: |
| Flashlight off | 19.484 ms | 19.490 ms | 0.02638 ms | 0.02710 ms |
| Flashlight on | 19.525 ms | 19.328 ms | 0.02726 ms | 0.02864 ms |

Frame ranges: off prior 19.463-19.497 / new 19.388-19.576 ms; on prior 19.491-19.575 /
new 19.190-19.435 ms. No frame regression was measured in this view. The lower new on
mean does not isolate a speedup or GPU cost. Model-light presentation adds about
0.0007 ms off and 0.0014 ms on. No extra light, render pass or shadow map was added;
the beam matrix is computed once per frame when lit, and changed uniforms are published
only to viewmodel materials. Off/dark returns disabled data without projection work.
No frame allocations, asset reads or font work were added. These results do not establish
a general FPS guarantee, costs for every weapon/view, or native timing parity.

Evidence: `artifacts/flashlight-viewmodel/timing-summary.json`,
`perf-{baseline,current}-{off,on}-{1,2,3}.json` and their logs; `benchmark.py` records the
rotated launch order and fails below 300 measured frames. Reproduce one launch from the
retained copy and its timing script (reset the copy's saved settings to the values above):

```sh
artifacts/flashlight-viewmodel/current/hl2-bevy --background --no-setup --mute --map d1_trainstation_02 --width 1920 --height 1080 --frames 2000 --profile --movement-script artifacts/flashlight-viewmodel/perf-on.json --report artifacts/flashlight-viewmodel/reproduce.json
```

Repeat off/on three times per package using `perf-off.json` for off. Capture fixtures pause
at tick 120 and are not timing substitutes. Earlier High and power-HUD descriptions below
have been corrected: their 600-frame cap was not reached because the script ended sooner.
Their numbers remain recorded with the actual short-window limitation.

## High flashlight depth shadows (2026-10-25)

Packaged Metal on Apple M1 Pro, `d1_trainstation_02`, 1920x1080, HDR Full,
a 600-frame safety cap, profiling on. Prior package: `f0d6945` (High still had no flashlight depth
map); new package: this continuation's constant-tap filter. Three runs per state,
without concurrent builds/replays. Cameras: plaza eye -3104 -2018 200, yaw 12, pitch -10;
citizen queue eye -4040 -2460 136, yaw 90, pitch -8. The queue capture visibly contains
three ordinary citizen NPCs. Scripts grant the suit/toggle the light at tick 1, finish
at tick 240 and end the launch early. These runs do not reach 600 frames.

| View and state | Mean frame | Median | Run range |
| --- | ---: | ---: | ---: |
| Plaza, prior High (no depth) | 20.044 ms | 19.943 ms | 19.066-21.124 ms |
| Plaza, new Medium | 20.600 ms | 20.758 ms | 20.274-20.768 ms |
| Plaza, new High | 21.979 ms | 22.052 ms | 21.377-22.507 ms |
| Three citizens, prior High (no depth) | 17.525 ms | 17.456 ms | 17.070-18.049 ms |
| Three citizens, new Medium | 17.576 ms | 17.266 ms | 17.135-18.326 ms |
| Three citizens, new High | 21.359 ms | 20.205 ms | 19.821-24.050 ms |

Correction recorded in the viewmodel continuation: these are short script-limited windows,
not 600-frame measurements. The nine `opt-plaza-*` reports contain 126-179 total frames
and only 6-59 measured frames after the 120-frame warm-up. Retain the numbers as observed,
noisy evidence; they need longer runs before precise cost attribution.

High minus Medium adds about 1.38 ms in the plaza and 3.78 ms mean in the citizen
view (2.94 ms by medians). The citizen High result includes a 24.05 ms outlier. These
are total frame costs of the requested depth pass and filter, not isolated GPU timings
or FPS guarantees. The plaza's shared lit path is 0.56 ms above the prior mean, within
the prior run range; that difference has not been isolated. In the citizen view the
Medium/prior means differ by 0.05 ms.

A later paired flashlight-off plaza set has prior/new medians 19.422/19.286 ms,
means 21.171/19.310 ms and ranges 19.289-24.803 / 19.185-19.458 ms. The prior set has a
large outlier; it does not demonstrate a speedup. No off regression was measured in these
runs. Off, Low/Medium and zero-brightness battery flicker create no depth pass. The final
battery-flicker guard was added after these timed runs; its normal lit-beam path is
unchanged and the packaged image checks were repeated afterward.

Cost controls: only High/lit enables the 1024-square map; cookie-zero, far-faded and
back-facing fragments skip its eight comparison samples. Existing Bevy visibility,
GPU skinning and opaque draws are reused; alpha-tested casters sample alpha in the
custom depth fragment. Inspection established that Bevy already omits the opaque fragment
stage, so a redundant override was removed. Moving the Poisson table to shader constants
preserved decoded capture pixels (fence identical, actor max one level); no measurable
speedup is claimed. A single depth-only control (maps rendered, filter bypassed) was too
noisy to isolate filter cost. Render-graph wall times include waits; GPU timestamps were
unavailable, so those times are not attributed to the shadow pass. Further batching or
filter tradeoffs need new evidence and fidelity review; no extra batching mode was enabled.

Evidence in `artifacts/flashlight-shadows`: `timing-summary.json`,
`perf-baseline-plaza-high-*`, `opt-plaza-{medium,high}-*`,
`final-crowd-{baseline-high,optimized-medium,optimized-high}-*`,
`final-plaza-{baseline,optimized}-off-*` reports and logs. Earlier `citizens`-named runs
showed mostly cops/benches; they are retained as a supplemental view, excluded from the
citizen table. Initial mismatched-resolution/HDR captures and two wall-facing fence views
are rejected, as recorded in STATUS and validation. No native timing comparison or
Windows runtime measurement was performed.

Reproduce a timed High run from the disposable package and its retained timing script:

```sh
artifacts/flashlight-shadows/optimized/hl2-bevy --background --no-setup --mute --map d1_trainstation_02 --width 1920 --height 1080 --frames 600 --profile --movement-script artifacts/flashlight-shadows/perf-crowd-high.json --report artifacts/flashlight-shadows/reproduce.json
```

Reset the copy's saved video settings to 1920x1080, HDR Full before each run; repeat
High/Medium three times using `perf-crowd-medium.json` for Medium. Use
`perf-plaza-{high,medium,off}.json` for the plaza. Public capture fixtures pause at tick
120 and are not timing substitutes. The optimized copy retains the timed package; the
final package adds only the zero-brightness gate described above.

## Flashlight power HUD (2026-10-25)

Packaged Metal, Apple M1 Pro, `d1_trainstation_02` spawn corridor, eye -4304 -224 64,
yaw -90, pitch -10, 1920x1080, a 600-frame safety cap, profiling on. Three alternating flashlight-off
and flashlight-on runs of each build, without a build or another replay running alongside.
Baseline package: `822e5f8`; new package: power HUD changes atop that revision. Copies use
the same saved settings; timing scripts give the suit and toggle the flashlight at tick 1,
then finish at tick 240, which ends the launch before its frame cap.

| State | Prior mean frame | New mean frame | Prior HUD CPU | New HUD CPU |
| --- | ---: | ---: | ---: | ---: |
| Off / fully recharged | 18.330 ms | 18.243 ms | 0.0365 ms | 0.0390 ms |
| On / auxiliary panel visible | 18.509 ms | 18.672 ms | 0.0383 ms | 0.0566 ms |

These power-HUD runs contain 182-189 total frames and 62-69 measured frames after warm-up;
the earlier 600-frame wording described a cap that was not reached.

The active HUD adds about 0.018 ms CPU to the prior on case, emitting 55 HUD quads instead
of 20. Off emits the same 20 quads. New on runs are 19.147, 18.470 and 18.399 ms, versus
18.536, 18.544 and 18.446 ms before. Frame time is noisy; the mean increase does not isolate
the GPU cost of this small panel or establish an FPS guarantee. No render pass was added.
Labels/icons are prepared on the map worker; drawing uses retained glyph quads.

Evidence: `artifacts/power-hud/timing-summary.json`, `base_bounded_{off,on}_{1,2,3}.json`,
`current_{off,on}_{1,2,3}.json` and their logs. With the completed 240-tick evidence script,
run the packaged copy as follows (substitute `off` for the other state):

```sh
artifacts/power-hud/current/hl2-bevy --background --no-setup --mute --map d1_trainstation_02 --width 1920 --height 1080 --frames 600 --profile --movement-script artifacts/power-hud/bounded_script_on.json --report artifacts/power-hud/reproduce.json
```

Those scripts preserve the camera above and exit when the script finishes, without pausing. Repeat off/on three
times per build. The replay fixtures also validate the HUD but pause for their final capture,
so they are not substitutes for the timing scripts.
Rejected sandbox launches could not access Metal. The first GPU launch rejected a 900-tick
script before completion at 600 frames; none of those rejected runs enter the table.


2026-10-08 owner-report milestone: prepared frame textures are shared and frame updates perform no asset I/O. Encoded/decoded budgets are bounded. Performance impact and FPS on the new material path are **not tested**; capture bursts verify visibility stability, not throughput. Existing measurements below retain their original scope.

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
