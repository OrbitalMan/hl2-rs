# Performance research: implementation leads and validation

Research checkpoint: 2026-11-11. Runtime inspected at `2208bcb618adfd3072ea5fd00553babdcb9a7866`.
This document records external implementation evidence, the corresponding HL2-RS code,
and bounded experiments. No runtime optimization has been implemented in this checkpoint.
Fresh local packaged measurements are complete and recorded below, separately from historical results.
The measured baseline predates published upstream trace improvements; see revision scope below.

## Scope and the reported gap

The owner reports 250+ FPS in original Half-Life 2 versus roughly 60+ FPS in HL2-RS on an
RTX 5070, on `d1_trainstation_02`. This is a user-supplied observation, not a matched
benchmark recorded here. Resolution, quality settings, view, actor population and elapsed
scene time are not established. At exactly 60 and 250 FPS, frame budgets are 16.67 and
4.00 ms: the difference is 12.67 ms of critical-path time. A small shader improvement
cannot by itself remove that gap if simulation and render preparation dominate.

The investigation followed successful particle, crowd and renderer demonstrations through
available author explanations, source code and comments. It covers simulation, animation,
physics, submission, geometry, visibility, effects, lighting and shaders. It is a bounded
survey, not an exhaustive search of the internet. External throughput figures belong to
their authors and are not independently reproduced HL2-RS improvements.

## What the existing evidence already establishes

The November 10 Windows profile in [performance.md](performance.md) used an RTX 3070,
Ryzen 7 3700X, release package `078d8d1`, 1080p owner settings and temporary reverted probes.
It must remain separate from the owner's RTX 5070 report and fresh contributor measurements.

| Retained Windows scenario | Mean frame | Relevant measured work |
| --- | ---: | --- |
| Station02 plaza, empty | 13.6 ms | About 5.3 ms per fixed tick |
| Same plaza, 30 hunting manhacks | 42.1 ms | About 10.5 ms per fixed tick; AI 3.9–6.1 ms |
| Rapier in that crowd | 2.39–2.41 ms per tick | About 1 ms broad phase and 1 ms query maintenance |
| Crowd with hitbox posing disabled, diagnostic only | 24.4 ms | Fixed cost 6.49 ms, versus 8.50 ms in its paired control |

The hitbox ablation barely changed Rapier's own cost. It attributes expensive work elsewhere;
it is not an acceptable optimization because accurate hits still require current hitboxes.
The tree measurements used 5,659 colliders, four awake bodies and no contacts. This makes
unconditional query/tree work a stronger lead than reducing solver iterations.

Recorded render CPU scopes include graph 3.4–3.8 ms, preparation 1.7–2.2 ms and writes
0.5–0.9 ms per frame. Crowd shadows rose from roughly 0.04 to 1.5–1.7 ms. Instrumented
Bevy GPU passes omitted custom passes: their small total does not establish complete GPU
idle time. Fixed-tick and frame scopes overlap and have different denominators; do not sum
them as an exclusive frame decomposition. Slow frames also run additional fixed ticks,
which can amplify a simulation bottleneck.

GPU skinning, cached PVS with active player/monitor views, shared material handles, CCD
gating, unchanged-pose checks, a shadow atlas and cached shadow cases already exist.
They are not new discoveries or unimplemented recommendations. The existing on-demand
hitbox/query-maintenance plan is in [DESIGN.md](DESIGN.md), section 14s.

## Revision scope: already implemented upstream

Read-only fetched-ref inspection on 2026-11-11 found main at `6d0b2f6`, four commits after the
local measured baseline. Two changes already address the repeated-trace opportunity:

- `4805c93` replaces repeated flight sweeps past NPCs with one sweep excluding
  `CONTENTS_MONSTER`. Upstream records fixed-tick samples 8.50 → 7.95/7.75 ms;
  frame differences were explicitly within noise. [Flight change](https://github.com/kvalls/hl2-rs/commit/4805c93).
- `7f0e9ff` replaces repeated sight rays and exclusion-vector growth with one filtered ray.
  Upstream records crowd frames 37.1–39.8 → 27.9/28.4 ms, and fixed ticks
  7.75–7.95 → 7.22/7.29 ms. These are retained upstream Windows results, not fresh local
  measurements or a matched RTX 5070 comparison. [Sight change](https://github.com/kvalls/hl2-rs/commit/7f0e9ff),
  [measurement scope](https://github.com/kvalls/hl2-rs/blob/6d0b2f6/docs/performance.md).

The newest fetched published session53 tip is `1473203`, with additional manhack appearance,
asset/fidelity and lamp-beam/PVS work. It still poses NPC hitboxes each tick and passes the
query pipeline into the physics step; the existing 14s redesign remains planned.
[Published work](https://github.com/kvalls/hl2-rs/compare/6d0b2f6...1473203),
[latest plan](https://github.com/kvalls/hl2-rs/blob/1473203/docs/DESIGN.md).
Do not duplicate the completed trace changes or describe baseline timings as latest upstream.
Published branch evidence cannot establish the status of unpublished work.

## Successful cases: what actually made them fast

### Millions of agents: reduce neighbor work before increasing parallelism

The BabylonJS/WebGPU boids author reports approximately 16 million 2D or four million 3D
boids before falling below 60 FPS on an M1 Max. An accessible author reply explicitly says
that the acceleration structure is a plain uniform grid; the linked repository repeats the
claim. This supplies a useful chain from headline to author comment to implementation,
not an independently verified performance result. [Author post and replies](https://www.reddit.com/r/GraphicsProgramming/comments/13cemv5/webgpu_millions_of_flocking_boids_with_babylonjs/),
[project repository](https://github.com/jtsorlinis/BoidsWebGPU).

NVIDIA's particle implementation explains the transferable mechanism: bin or sort particles
by spatial cell, inspect neighboring cells, and improve memory coherence. Variable occupancy
requires correct overflow handling, such as count/scan/scatter or sorted cell ranges.
A grid is not automatically linear under arbitrarily dense clustering. [NVIDIA implementation paper](https://developer.download.nvidia.com/assets/cuda/files/particles.pdf).

Sebastian Lague's tutorial shader provides a useful counterexample: it moves neighbor
calculation to a compute shader but still loops over every boid for every invocation.
Parallel throughput does not eliminate quadratic work. It also uses squared distances for
radius comparisons. [Author shader](https://github.com/SebLague/Boids/blob/master/Assets/Scripts/BoidCompute.compute).

These boids do not reproduce HL2 schedules, speech, squads, weapons, hitgroups, scripts or
dynamic prop interactions. The transferable experiment is exact candidate reduction, not
replacing campaign actors with simplified GPU creatures.

### Factorio: stop repeating work and share results

Factorio developers measured dormant roboport updates falling from 1 to 0.025 ms per tick
in one save. A separate shared registration/bucket system for overlapping radar coverage
improved overall performance by 3.6% in another save. The subsystem reduction is not a
40-fold whole-game speedup. [Developer measurements](https://www.factorio.com/blog/post/fff-421).

Their path-cache work reuses suitable internal portions of previously computed paths instead
of requiring closely matching endpoints. Their parallel update experiment also demonstrates
that adding threads can become slower through cache contention, whereas read-only work can
scale better. [Path reuse](https://www.factorio.com/blog/post/fff-121),
[parallel update investigation](https://www.factorio.com/blog/post/fff-215).

For HL2-RS this motivates dirty flags, reusable pose results, exact active-work lists and
immutable job inputs. Every wakeup path must remain correct; offscreen scripts and NPC
obligations cannot be dropped because an actor is outside the current view.

### Millions of particles: representation and blend mode matter

Mike Turitzin's shipped PARTICULATE renders over two million particles to two VR eye buffers
at 90 Hz. His tested compute-rasterization variants outperform conventional rendering,
but specifically for single-pixel additive particles. Avoiding fragment-quad waste and
primitive-order requirements is central; cloud distribution affects the outcome.
This suggests a specialist spark path, not a general replacement for smoke billboards.
[Author's implementation analysis](https://miketuritzin.com/post/rendering-particles-with-compute-shaders/).

Xiao Jiang reports over one million particles at 60 FPS on GTX 1080/i7-8700K, using two
compute dispatches and one draw. The implementation keeps state in ping-pong structured
buffers, compacts live particles, and produces indirect arguments. It avoids individual CPU
particle objects and submissions. Full HL2 scenes, sorted transparency and equivalent gameplay
collision are not demonstrated. [Author's implementation](https://xiaojiangbrian.com/work/gpu-particles/).

The immediate HL2-RS opportunity is compact compatible sprite batches and reduced mesh
updates. Compute simulation is a later question if simulation itself remains expensive.
Keep alpha order, material behavior, simulation clocks and random/event sequences intact.

### GPU-driven rendering: the work before the draw call still matters

Ubisoft's Trials Rising presentation reports an initial construction-site CPU figure falling
from 51.43 to 24.09 ms and GPU from 22.17 to 17.95 ms on Xbox One. That iteration still
missed its target because gathering instance data and batching remained expensive.
Persistent pooled geometry/instance data and filtering address more than command submission.
[Primary GDC presentation](https://media.gdcvault.com/gdc2019/presentations/Drazhevskyi_Oleksandr_GPU_Driven_Rendering.pdf).

The Last General's developer reports tripled FPS and around 90% fewer batches. Author replies
identify unexpected fragmentation caused by objects instantiated at different times in Unity's
renderer, alongside occupancy, hierarchy and dependency changes. This is a reminder to inspect
actual batches; Unity component-reset remedies are not Bevy APIs.
[Author post and technical replies](https://www.reddit.com/r/Unity3D/comments/1nc1pt3/tripled_performance_on_my_dots_game_in_a_month/).

Niagara's culling shader combines bounds checks, LOD selection, depth-pyramid tests and a late
recovery pass. Its author measured a tradeoff: larger meshlets reduced culling effectiveness in
an artificial scene. Fewer batches and finer culling can oppose each other.
[Culling implementation](https://github.com/zeux/niagara/blob/master/src/shaders/drawcull.comp.glsl),
[meshlet measurements](https://zeux.io/2023/01/16/meshlet-size-tradeoffs/).

### Animation budgets and shader tuning have limits

Epic's animation budgeting lowers update frequency or interpolates according to significance.
Coconut Lizard's experiment reports 256 humanoids improving from 12 to 27 FPS, but explicitly
warns of immersion-breaking degradation around 128–255 actors. This trades temporal quality
for time; it does not justify silently changing HL2 animation or AI cadence.
[Official mechanism](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-budget-allocator-in-unreal-engine),
[measured experiment](https://www.coconutlizard.co.uk/blog/animation-budget-allocator/).

AMD explains why occupancy alone is not a shader performance target: available work,
registers, memory and latency determine the useful next change. Apple's tile-local rendering
guidance motivates attachment/bandwidth inspection on Metal, without establishing a portable
renderer rewrite. Meshoptimizer documents indexing/cache/fetch ordering and cautions that
overdraw reordering can be unhelpful on tile GPUs. [AMD analysis](https://gpuopen.com/learn/occupancy-explained/),
[Apple documentation](https://developer.apple.com/documentation/Metal/rendering-a-scene-with-deferred-lighting-in-swift),
[mesh layout guidance](https://github.com/zeux/meshoptimizer).

## Geometry headlines: distinguish source scenes from shaded frames

The Nyx author advertises roughly 1.6 billion unique / 18.9 billion instanced triangles and
links the implementation; author comments point to Epic's technical presentations. The
repository clarifies that these are source-scene totals, with actual per-frame work selected
by visibility and LOD. Its current example uses Ryzen 9950X / 96 GiB / RTX 5070, DX12 mesh
shaders, two-pass hierarchical-depth culling, compressed asynchronous pages and cached
shadows. It defaults to DLSS Quality with fallback, so output 4K is not proof of native 4K
shading. Current repository counts differ from the older post. This is a renderer prototype,
not a complete campaign comparison or a drop-in Bevy feature.
[Author post and comments](https://www.reddit.com/r/GraphicsProgramming/comments/1rm9mf0/i_built_a_nanitestyle_virtualized_geometry/),
[implementation and architecture](https://github.com/moonlovelj/Nyx).

The legacy Vulkan Guide sample reports 125,000 objects, 40 million triangles across two mesh
passes and 290 FPS on RTX 2080, with CPU processing under 0.5 ms. GPU-resident state and
indirect command construction explain its low submission cost; the chapter explicitly labels
itself outdated. These are sample results, not equivalent HL2 gameplay or predicted savings.
[Implementation guide](https://vkguide.dev/docs/gpudriven/gpu_driven_engines/).

## Current code: concrete opportunities and existing safeguards

| Area | Inspected implementation | Implication to test |
| --- | --- | --- |
| Hitboxes | `hl2-simulation/src/gameplay.rs`, `pose_npc_hitboxes` | Composes every enabled actor's skeleton each fixed tick; existing on-demand plan comes first |
| Query trees | `hl2-simulation/src/physics.rs`, `refresh_entity_queries` and `tick_inner` | Incremental scripted refresh exists; Rapier step also receives the query pipeline |
| Sensing | `hl2-simulation/src/ai/runtime.rs`, NPC sight gather | Cached ID list is cloned and scanned per think; exact spatial candidates may reduce work |
| Sight/flight | Baseline `hl2-simulation/src/ai/host.rs`; upstream commits above | Repeat traversal already removed upstream; validate those changes rather than duplicate them |
| Pose consumers | `hl2-simulation/src/entities/choreography/pose.rs`, Bevy eyes/shadow/render adapters | Exact poses are composed in several consumers; renderer already caches samples |
| Effects | `hl2-bevy/src/effects.rs`, quad-pool update | Handles/entities are pooled, but changed quads create replacement mesh arrays |
| Shadows | `hl2-bevy/src/shadows.rs` and child modules | Ranking, atlas and unchanged cases already exist; separate remaining stages before redesign |
| Lighting | `hl2-bevy/src/rendering/lighting.rs` | Unchanged illumination and cubemap choices are cached; profile invalidation/work first |
| Submission | `hl2-bevy/src/rendering/drawing.rs` | M1 direct-draw/CPU-clustering policy already reflects earlier measurements |

All source paths in this table are relative to `crates/`. These are observed code patterns,
not fresh timing attribution. Exact actor pose reuse must distinguish fixed simulation time
from presentation time, retain gestures/ragdolls/pose parameters, and publish current bounds
before visibility. The sensing change must preserve candidate order, relationships, hearing,
LOS, spawn/kill/teleport handling and dense-cell correctness.

## Ranked, independently reversible experiments

1. **Finish the existing on-demand hitbox plan.** Use conservative bounds to identify candidate
   actors; sample their exact current pose before hitbox-capable narrow queries. Audit bullet,
   melee, projectile, decal, impact and NPC paths. Compare hit entity, hitgroup, bone and distance
   against the reference, including moving limbs and the first shot after spawn or teleport.
2. **Finish query-maintenance ownership.** Track every insert, remove, enable and pose change;
   refresh at the existing pre-query/post-physics boundaries. Refit current bounds and measure
   amortized rebalancing. Passing no query pipeline to the step without replacing maintenance
   is incorrect. Preserve doors, props, ragdolls and maker spawns.
3. **Validate upstream sight/flight improvements on matched fixtures.** Reuse the completed
   one-query changes, preserving their collision semantics. Count casts/candidates and compare
   trace and gameplay outcomes. Do not implement a second independent copy of this work.
4. **Count rendering work by view.** Attribute extraction, changed materials/meshes, upload bytes,
   batches and draws to world, miniature sky, monitors, weapon and shadows. Test existing direct
   versus indirect overrides on each adapter; do not extrapolate the Metal policy to RTX.
5. **Batch compatible effect sprites.** Compare compact instanced records or reusable contiguous
   geometry with mesh-per-quad updates. Include zero effects, normal combat and dense smoke/sparks.
   Retain alpha ordering; separately report effects CPU, render-asset cost and total frame time.
6. **Test exact spatial sensing candidates and shared pose results.** First count candidates and
   pose compositions. Preserve stable decision order and exact clocks; avoid lossy neighbor caps.
   Reuse scratch capacity where allocations are measured before attempting a broad data rewrite.
7. **Attribute crowd shadows.** Separate pose sampling, silhouette production, projection, mesh
   writes and atlas updates. Verify moving actors, stationary crowds, doors and camera motion.
8. **Test existing world partitioning before adding an occlusion pipeline.** Compare candidate
   triangles with actual submissions. Any later occlusion experiment needs conservative current
   bounds, newly visible recovery and every monitor view; visibility never suspends gameplay.
9. **Time custom GPU passes at high resolution.** Include blur, bloom, flashlight depth and
   composition. Inspect attachment/copy traffic and shader bottlenecks only after attribution.
   Validate quality and resource lifetime alongside timing; ordinary Bevy GPU totals omit these.
10. **Evaluate load-time mesh layout and immutable jobs.** Preserve lightmap seams, mesh-local
    flex IDs and alpha order. A new dependency or materially different architecture needs the
    existing owner decision process. Quantization, simplified LOD and reduced update cadence
    are separate fidelity-sensitive proposals, not assumed free optimizations.

The growing after-AI stage in the retained profile also deserves an active-count/lifetime
trace: it may be legitimate workload growth or a cleanup issue. A memory leak is not proven.
Every experiment needs its own before/after result; estimated savings from different scopes
must not be added into a promised FPS total.

## Measurement and acceptance plan

Use isolated packaged release builds, complete replay outcomes, declared settings and fixed
scene time/view/workload. Run alternating repeated A/B trials after warmup with no concurrent
build or GPU workload. Record hardware, backend, package hash, resolution, internal render
scale, quality, VSync, plugins and profile status. Separate cold setup and shader priming from
steady state. Record p50/p95/p99 frame time, ticks per frame, fixed cost per tick, render work,
allocation/upload counts and GPU timings where supported. Measure profiling overhead separately.

Cover station02 spawn/plaza, ordinary campaign actors, hunting manhacks, moving props,
script-heavy interactions, outdoor geometry, effects, monitor views and map transitions.
Each timing fixture must finish its intended workload; a fixed frame cap can sample unequal
simulation duration. Pair visual comparisons with gameplay/trace assertions. Diagnostic runs
that disable work establish attribution only and cannot qualify an optimization.

Fresh local packaged tests: **18 valid completed sensitivity runs**, detailed below. Matched native RTX 5070 comparison:
**not tested**. Windows validation of any future runtime change: **not tested** here. No
performance gain, sustained FPS target or complete campaign fidelity is established by this report.

## Fresh packaged measurements: indoor station02 spawn

The existing release package matches `2208bcb618adfd3072ea5fd00553babdcb9a7866`, SHA256
`51cf256188b5e0446165cc2dc55ad90729768143683151f9c4e5182c41895fc5`.
Apple M1 Pro / Metal, 10 logical CPUs, 16 GiB, advanced-options/debug-tools plugins;
muted headless uncapped profiling. Explicit baseline: 1920x1080, FOV 75, HDR Full,
High model/texture/shader/shadow, World water, no AA, trilinear, motion blur and color
correction on, multicore on, VSync off. These are controlled settings, not the owner's
unestablished RTX 5070 settings or the local saved configuration.

Six variants ran in three sequential interleaved orders. Each completed the same script at
host tick 600; the 12,000-frame limit is only a safety cap. All 18 valid launches exited 0,
with zero GPU completion errors, pose/visibility mismatches and texture errors. Two initial
Low trials used an invalid water enum and remain rejected; two asset warnings and existing
runtime warnings remain. No native game was launched and no user-launched application closed.

| Variant | Trial mean frame times, ms | Median, ms | FPS from median | Trial p95 upper bounds, ms | Trial p99 upper bounds, ms |
| --- | --- | ---: | ---: | --- | --- |
| Baseline | 5.879 / 5.811 / 5.751 | 5.811 | 172.1 | 10 / 9 / 9 | 13 / 11 / 11 |
| 720p | 5.781 / 5.735 / 5.982 | 5.781 | 173.0 | 9 / 9 / 10 | 12 / 11 / 13 |
| PVS disabled | 7.736 / 7.674 / 7.784 | 7.736 | 129.3 | 10 / 10 / 11 | 14 / 14 / 14 |
| World partition | 5.773 / 5.775 / 5.811 | 5.775 | 173.2 | 9 / 9 / 9 | 13 / 13 / 14 |
| CPU skinning | 12.568 / 12.640 / 12.476 | 12.568 | 79.6 | 20 / 21 / 21 | 23 / 22 / 23 |
| Low at 1080p | 5.680 / 5.796 / 5.650 | 5.680 | 176.1 | 9 / 9 / 9 | 11 / 13 / 11 |

PVS disabled costs 33.1% more median frame time; CPU skinning costs 116.3% more. These
measure already implemented mechanisms, not a newly optimized build. The 720p and partition
differences (0.51% and 0.61%) are inside trial variation. Low differs by 2.25%, overlaps baseline
variation and changes quality; no general gain is established.

At the final viewpoint partitioning reduces candidates 52,768 to 7,257 triangles (about 86%)
but raises candidate meshes 50 to 62 and loaded meshes 1,835 to 2,587. The tiny timing change
shows why candidate triangle savings alone do not establish a faster frame. These are CPU
candidates unioned across views, excluding sky/viewmodels, not actual GPU draws.

The scene is the indoor spawn facing the stairwell exit, without a spawned crowd, not the
plaza. Baseline final feet (-4304,-224,-63.96875), eye (-4304,-224,0.03125), grounded, tick 600
agree. A separate untimed 1920x1080 capture shows lit, textured stairs and walls; its readback
cost is excluded. Evidence: `artifacts/performance-research-2026-11-11/baseline-visual.png`.

Frame statistics discard 120 gameplay frames and use 1 ms histogram upper bounds for p95/p99;
p50 is unavailable. Different rendered-frame counts mean the discarded frames cover different
simulation intervals; this is not a cycle-isolated skinning test. CPU scopes include setup,
discard 30 calls, overlap, and are not GPU timing. Existing compressed memory and about
640 MiB swap qualify results; unchanged swap occupancy does not establish zero paging.
Power/thermal sensors, full custom-pass GPU timestamps, live windowed FPS, heavy crowds
locally and Windows RTX 5070/native comparison are **not tested**.

Reproduce from the repository root:

```text
python3 artifacts/performance-research-2026-11-11/benchmark.py
```

The ignored experiment directory retains the isolated package, exact per-run arguments and
configs, raw reports/logs, `measurements.json`, `summary.json`, environment snapshots and
rejected trials. A populated matrix is resumed rather than fresh evidence; independent reruns
need a fresh isolated package/output set. The full report is
`artifacts/performance-research-2026-11-11/reports/HL2RS performance research.md`.
No new runtime dependency or production optimization was introduced.

## Research access limits

Accessible Reddit author replies materially explained both the boids grid and Unity batch
fragmentation. YouTube descriptions/source links were discovery leads; no claim is made that
videos, their audiovisual contents or their comments were exhaustively inspected. Some pages
failed to fetch, including an exact-version Rapier documentation route and pinned Bevy occlusion
API. Current unversioned documentation is not proof that a feature exists unchanged in the pinned
runtime. No third-party crate was patched or runtime prerequisite introduced.

A point-cloud fractal proof of concept and an invisible-particle simulation update were excluded
from full-game throughput evidence. Headline counts without workload, representation, hardware,
settings and implementation detail remain leads, not performance results.
