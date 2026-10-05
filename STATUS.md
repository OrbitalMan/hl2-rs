# STATUS

Handoff: 2026-10-05, Atlantic/Canary. Verified runtime revision: `28c0bb3a66c50a9baa60fdda001b850ded4640a1`; the documentation commit follows it. Read AGENTS.md, this file and docs/DESIGN.md first. Historical MODLOG entries describe their own dates, not current limitations.

## Goal

Faithful Rust reconstruction of Half-Life 2 and relevant Source behavior using assets from an owned installation. This is a rewrite, not a passthrough mod. Full 1:1 behavior and ordinary first-level campaign completion remain unfinished. No FAL or crossovers.

## Setup

- Owned Steam HL2 app220, tested build19307283/patch9912070; other versions **not tested**. Default install `C:/Program Files (x86)/Steam/steamapps/common/Half-Life 2`, or discovered library/HL2_ROOT.
- Windows11 Pro64-bit, Rust1.99.0 MSVC, Visual Studio C++ tools/Windows SDK; Bevy0.19.1/wgpu and direct Rapier0.26.1. No Source DLL or mod loader dependency.
- Previous implementation chat: Codex Desktop, GPT-6.1 Extra High/Normal Speed. Choose fresh-chat settings explicitly.
- Active checkout: workspace `outputs/hl2-rs-bevy`, branch `bevy-migration`, remote https://github.com/kvalls/hl2-rs. Sibling `outputs/hl2-rs` remains main at b4b1731530a9ca146f9d45532f00ef7f9fc69e7d. Preserve macroquad-prototype at f9995dab2640d15cda8d7d7b10d08af7e691c8ea and unverified wip/scripted-scenes at d0a08c8d36e1b7d3496b6627b5a974337641ca9c.
- Private evidence/checkpoints: workspace `work/hl2-decompiled`, `work/references`, `work/publishing`, reached as `../../work/...` from the active checkout. These are not public runtime dependencies.
- Bevy release `bin/hl2-bevy.exe`: SHA256 `78348779124CD24019E41969A6EBC33D61EE2852C96CA4673C02DA9736FA9D9D`, built2026-10-05T16:08:57Z. Retained release `bin/hl2-rs.exe`: `9C198FBC252375BC6331D1BB5FA6BF1FF25E65836A9AFA5B0F46ED48229C610A`, built16:14:21Z. Generated metadata/shader hashes are in bin/build*-info.json; executables are ignored.
- Source commits/pushes and game testing are authorized. Leave user-launched games alone. An old scheduled continuation belongs to the old chat; do not assume it follows a fresh chat. The latest request is to finish this handoff and await the new chat, not continue implementation here.

## What works (tested)

- Shared bounded VPK/BSP/material/model/animation/audio readers, owned PHY convex pieces, fixed15ms movement including air crouch, synchronized door/collider poses and limited Rapier props.
- Six primary weapons, shotgun one-shell secondary fallback, SMG contact grenades/AR2 charge and ball behavior, selection/ammo/HUD animation, pause/console subset and actual Bevy audio sinks. Controlled fixtures test these; the full arsenal is not implemented.
- Bevy host consumes shared gameplay/entity state and presents sky, GPU skeletal animation, animated iris projection, authored timed LOOKAT interests, impacts/partial particles, live camera monitors and landmark/inventory transitions. General AI and scenes remain partial.
- Native owned1080 captures establish white five-dot22x22 reticle and yellow pistol/SMG21x17. Seven Bevy720/1080/borderless captures and retained1080 captures match dot offsets. Owned Windows GDI setup fixes blur/fractional positioning; color/tone/general-font parity is not claimed.
- launch-bevy-1080p.cmd and launch-bevy-borderless.cmd both captured1920x1080. Older launchers in this checkout still run the retained host. Each build script now targets its own executable.
- GPU skinning, player/monitor BSP PVS and shared materials improve measured local performance. Optional --world-partition splits opaque/cutout world batches; inspected plaza/spawn images are identical on/off. It stays off by default.
- Latest runtime checks:294 normal tests,19 owned-install checks, strict workspace all-target Clippy/fmt; packaged movement26, weapons/doors17, monitor/campaign18 and accepted image9 checks. These are recorded checks for28c0bb3, not rerun claims for documentation edits. See docs/validation.md.

## What doesn't work yet

- General NPC schedules/navigation/combat and full first-level camera/train/Breen/Kleiner staging; ordinary campaign completion.
- Layered gestures/autolayers/delta-post composition, weighted blends, head poses, facial flexes/lip sync/IK, ragdolls and full studio events.
- Remaining weapons, exact spread/prediction, complete damage/death, vehicles, save/load and complete entity/global/player transfer.
- Exact Havok/VPhysics dynamics, animated collision, moving platforms and blocked-door handling. Rapier/Avian comparative performance is **not tested**; a physics migration was not requested.
- Source HDR/fog/shadows/refraction/material stages, area portals/occluders/LODs, recursive camera/proxy fidelity, complete particle effects/DSP/soundscapes and VGUI/console parity.
- Station03 has25 known unsupported-material errors, unchanged in the latest campaign fixture. The local zero-byte d2_coast_02.bsp is a known rejection.
- Other platforms, native/campaign-wide144FPS and complete pixel parity are **not tested** or established.

## Which game owns the player

Not applicable: this is a rewrite. Rust simulation owns player/gameplay state. The installed original supplies content and is run separately as a comparison oracle.

## The current problem

The host migration is substantially connected, but choreography/model reconstruction is incomplete. Real intro/security gestures cannot be reproduced by the current base-clip-only pose path. Existing absolute delta conversion loses information needed for arbitrary post composition. Full-body blending a parent gesture is incorrect: all65 named sampled parents have zero bone masks and refer to311 child layers. Preserve raw transforms, child dependencies, weights and authored retiming before implementing gesture execution, then address actor readiness/AI and first-level staging.

Private database/native exports and published SDK research corroborate selected behavior. The entire game has **not** been decompiled and translated; indexing functions is not a completed rewrite. Keep byte/hash provenance, mark inferred signatures and keep native code/databases outside public Rust sources.

## Evidence

- Runtime milestones: df6af1b GPU skinning; b888144 cached PVS;322afaa activity lookup; c2b5220 material sharing;28c0bb3 reticles/optional spatial batching.
- Local logs: artifacts/crosshair-partition-tests.log, crosshair-partition-owned.log, crosshair-partition-clippy.log; partition-movement-final.assertions.json26 passed; partition-weapons-final.assertions.json17 passed; partition-gameplay-checks.json18 passed; crosshair-partition-image-checks.json9 accepted plus rejected hall. Prior GPU/PVS/attention captures remain in artifacts; docs/validation.md records their scope.
- Native1080 oracle: private work/hl2-decompiled/original-oracle/crosshair-1080-20261005T153947Z, with installed module hashes, measurements and cleanup. Six installed settings stayed unchanged; test process closed.
- Last runtime source audit:212 files,0 failures,2 known upstream-path warnings. Reaudit this documentation snapshot before publication.
- Private gesture evidence: work/hl2-decompiled/animation-next-2026-10-05.md, owned census and pinned SDK notes.65 named plus59 empty active gesture occurrences; native handling of empty events is unproven. That research note has older build headers; current revisions/builds are above.
- Uncapped/profiled1080 station02: CPU skinning35.05→GPU66.85FPS; PVS disabled66.69→enabled156.94; subsequent material package174.72. Separate local comparisons with run variation/profiling overhead, not one isolated cumulative benchmark or campaign guarantee. Spatial gains are small/experimental; see docs/performance.md.

## What we've already tried

- Debug-overhead explanation: packaged builds were already release; development dependency overrides existed. Profiling identified skinning/uploads and out-of-view preparation instead.
- GPU skinning/current bounds, cached PVS and material reuse: measured local gains with inspected actor/eye/monitor regressions. Do not disable simulation or animation to fake FPS.
- Equal reticle sizes: rejected in favor of original measurements; fix native rasterization instead. A retained yellow color filter was too strict because additive background raised green239–250; actual dot positions match.
- Hall culling comparison: rejected because its camera intersected a pillar. Valid plaza/spawn comparisons are accepted; do not reuse the bad viewpoint as proof.
- Frozen900-frame benchmark with6000-tick script: rendered but returned incomplete-script error; helper stopped. Excluded from published comparisons. Do not remove completion validation or call it an engine crash.
- Campaign texture-zero assertion: existing station03 errors caused failure; comparing the identical25 errors verified no new asset regression, not successful decoding.
- Kleiner implicit idle: owned ACT_IDLE resolves idle_subtle. Metrocop idle_baton was already valid; do not repeat the mistaken assumption that all literal idle labels are missing.
- Seeded lab/attention fixtures verify selected behavior, not natural broadcast staging or campaign completion. wip/scripted-scenes is still separate/unverified.

## Ideas not tried yet

1. Follow docs/DESIGN.md: bounded raw delta/post/mask/layer readers, shared composition, authored gesture execution, then real-scene comparison. Reuse shared crates and private native evidence.
2. Compare a real first-level scene against original captures/audio/timings, separating reader, scheduler, actor readiness and rendering defects.
3. Add area/occluder/LOD work only with current-view/monitor correctness checks and matched profiles. Keep spatial splitting opt-in until broader evidence supports a default change.

## Files that matter

- AGENTS.md: authorization/boundaries/order. MODLOG.md: historical tested changes. docs/DESIGN.md: architecture/next bounded plan.
- crates/source-assets/src/animation.rs and crates/modkit-core/src/animation.rs: decoding/pose math; raw delta/post/layer work starts here.
- crates/hl2-simulation/src/actors.rs, entities.rs, attention.rs, npc.rs: preparation, scene scheduling/interests and incomplete locomotion/AI.
- crates/hl2-simulation/src/physics.rs, player_convex.rs and crates/modkit-core/src/movement.rs: shared collision/player behavior.
- crates/hl2-bevy/src/main.rs, movement.rs, gameplay.rs, campaign.rs: fixed order/input/lifecycle. rendering.rs, gpu_skinning.rs, visibility.rs, eyes.rs, monitors.rs: presentation.
- crates/hl2-ui/src/hud.rs, native_font.rs: shared HUD/reticles. crates/hl2-runtime: retained host consuming shared state.
- docs/research.md, validation.md, performance.md, bevy-migration.md: evidence/contributor scope. Private work/publishing/continuation.md: precise local checkpoint, newest section first.

## Reproduce from this checkout

```powershell
.\scripts\build-bevy.ps1
.\launch-bevy-borderless.cmd --map d1_trainstation_02
.\launch-bevy.cmd --movement-script test-inputs/bevy-movement.json --capture artifacts/check-movement.png --report artifacts/check-movement.json
.\launch-bevy.cmd --movement-script test-inputs/bevy-entities-weapons.json --capture artifacts/check-weapons.png --report artifacts/check-weapons.json
cargo test --locked --workspace
$env:HL2_ROOT = 'C:/Program Files (x86)/Steam/steamapps/common/Half-Life 2'
cargo test --locked --workspace -- --ignored
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: controlled replays finish with captures/reports, no station02 texture errors or pose disagreements. F3 gives developer weapons; E uses doors, wheel/slots select, F1 toggles diagnostics and Escape/tilde open pause/console. Console help lists implemented commands; `sv_cheats 1; impulse 101` is supported. Crosshairs are small five-dot reticles. Developer loadouts/landmark replays are not ordinary campaign proof. Add --world-partition only for the experimental path. Retained uses scripts/build.ps1 and launch-borderless.cmd, without an extra view argument.

Template basis: [STATUS handoff](https://github.com/trevaintdead/ai-game-modding-guides/blob/main/templates/STATUS-handoff.md), adapted for a rewrite.
