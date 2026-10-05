# Design

## Goal and architecture

Reconstruct HL2 in Rust using owned content. No host-game transport or Valve DLL dependency. Native analysis/databases stay private; public source contains original implementation and permitted third-party resources.

Owned VPK/loose/map content -> source-assets bounded decoders -> modkit-core contracts/player/pose math -> hl2-simulation gameplay/choreography/Rapier -> Bevy or retained presentation. hl2-ui records CPU draw commands for both hosts. Setup or asynchronous transitions load resources; frame systems consume prepared data.

AGENTS.md records fixed-step order and renderer invariants. Bevy uses custom materials/lightmaps, separate sky/world/viewmodel/HUD cameras, GPU skinning/current bounds and player/monitor PVS/frustum rejection. Identical material/lightmap pairs share handles. Spatial splitting remains off by default. Replacing Rapier is a separate behavior migration.

## Next bounded plan: decompilation coverage in the fresh chat

The owner requested continued progress toward complete decompilation, then explicitly moved that work to a new chat. This handoff records the plan only. No new Ghidra run, recovery attempt, export or database rebuild was started here.

1. Reconcile a hashed installed-binary inventory with the existing 22-module corpus. A read-only scan found 126 DLL/EXE files across the installation, including tools, middleware and other game variants. Classify baseline HL2 runtime, optional renderer/audio/input modules, tools, third-party dependencies, duplicate bytes and other game variants; record evidence and exclusions. File count alone does not identify the required runtime set. Preserve current private binary copies, exports and catalogues; do not overwrite them or key expanded inventories by basename alone.
2. Investigate the ten recorded overlapping-input-varnode export failures using raw instructions, function boundaries, calling conventions and Ghidra diagnostics. Save each genuinely different recovery attempt separately, with binary hash, address, settings, result and original error. Do not erase failures, invent bodies or equate a successful retry with verified semantics.
3. Audit discovery as well as export success: entry points, exports, imports/thunks, RTTI/vtables, indirect-call targets and executable ranges without identified functions. Classify padding/data/shared tails and uncertain boundaries explicitly. Promote candidates only with evidence; attempted functions are not a proven denominator for the original program.
4. Keep the exact-byte SQLite catalogue authoritative for export retrieval. The unchanged gamedb parser misses 9,713 successful bodies and has 2,810 range mismatches; it remains a supplemental navigator. Add new evidence through audited, versioned private outputs with module/hash/address identity, preserving original bytes and unresolved references. Track export, naming/type recovery, Rust implementation and native behavior verification separately.
5. Recover reviewed names, types, state and behavior for animation/choreography and first-level AI, then feed confirmed findings into the existing shared Rust crates and compare them against the original game. Published SDK code corroborates behavior but is not proof of the installed retail implementation.

After each bounded pass, record exact coverage, remaining failures/uncertainties, commands and next step. Completion claims require an explicit runtime scope, justified function discovery and reviewed semantics; a pseudocode export or database import alone is insufficient. All native binaries, scripts, recovered code and databases remain under private work/hl2-decompiled or work/references, outside public source checkouts.

## Queued runtime plan: authored NPC gestures

1. Preserve raw delta/post transforms, bone masks, autolayers and model-tag dependencies in bounded readers. Preserve base clips and account explicitly for child budgets.
2. Implement shared layered pose composition with independent synthetic references and owned-data comparisons. Verify retail/native evidence separately from SDK behavior.
3. Execute choreography gestures with authored timing/tag retiming, ramps, priorities and pause/cancel semantics. Both renderers consume the same composed poses.
4. Compare a real intro/security scene against original captures/audio/timings, then extend head/facial/lip controls and actor scheduling. Seeded developer actors are not campaign proof.

Validate each step. After two failed attempts at one approach, document it and choose a different evidence-based approach. Full AI, staging, save state, arsenal and shaders remain separate tasks.

## This handoff's validation

Documentation only: preserve history/invariants and the source-only whitelist, update the decompilation priority and private checkpoint, check links and audit the public snapshot, then commit/push to bevy-migration. Do not rebuild/replay unchanged runtime code. The owner will continue in a fresh chat.
