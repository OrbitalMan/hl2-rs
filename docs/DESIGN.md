# Design

## Goal and architecture

Reconstruct HL2 in Rust using owned content. No host-game transport or Valve DLL dependency. Native analysis/databases stay private; public source contains original implementation and permitted third-party resources.

Owned VPK/loose/map content -> source-assets bounded decoders -> modkit-core contracts/player/pose math -> hl2-simulation gameplay/choreography/Rapier -> Bevy or retained presentation. hl2-ui records CPU draw commands for both hosts. Setup or asynchronous transitions load resources; frame systems consume prepared data.

AGENTS.md records fixed-step order and renderer invariants. Bevy uses custom materials/lightmaps, separate sky/world/viewmodel/HUD cameras, GPU skinning/current bounds and player/monitor PVS/frustum rejection. Identical material/lightmap pairs share handles. Spatial splitting remains off by default. Replacing Rapier is a separate behavior migration.

## Next bounded plan: authored NPC gestures

1. Preserve raw delta/post transforms, bone masks, autolayers and model-tag dependencies in bounded readers. Preserve base clips and account explicitly for child budgets.
2. Implement shared layered pose composition with independent synthetic references and owned-data comparisons. Verify retail/native evidence separately from SDK behavior.
3. Execute choreography gestures with authored timing/tag retiming, ramps, priorities and pause/cancel semantics. Both renderers consume the same composed poses.
4. Compare a real intro/security scene against original captures/audio/timings, then extend head/facial/lip controls and actor scheduling. Seeded developer actors are not campaign proof.

Validate each step. After two failed attempts at one approach, document it and choose a different evidence-based approach. Full AI, staging, save state, arsenal and shaders remain separate tasks.

## This handoff's validation

Documentation only: adapt the three templates, preserve history/invariants, introduce a source-only whitelist, check links, tracked-file coverage and generated/private exclusions. Audit the staged snapshot, then commit/push to bevy-migration. Do not rebuild/replay unchanged runtime code. The owner will continue in a fresh chat.
