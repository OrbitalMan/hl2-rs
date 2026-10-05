# Design

## Goal and architecture

Reconstruct HL2 in Rust using owned content. No host-game transport or Valve DLL dependency. Native analysis/databases stay private; public source contains original implementation and permitted third-party resources.

Owned VPK/loose/map content -> source-assets bounded decoders -> modkit-core contracts/player/pose math -> hl2-simulation gameplay/choreography/Rapier -> Bevy or retained presentation. hl2-ui records CPU draw commands for both hosts. Setup or asynchronous transitions load resources; frame systems consume prepared data.

AGENTS.md records fixed-step order and renderer invariants. Bevy uses custom materials/lightmaps, separate sky/world/viewmodel/HUD cameras, GPU skinning/current bounds and player/monitor PVS/frustum rejection. Identical material/lightmap pairs share handles. Spatial splitting remains off by default. Replacing Rapier is a separate behavior migration.

## Active bounded plan: private decompilation coverage

The fresh chat is now authorized to continue the decompilation work. This pass changes private research and public evidence documentation; runtime gesture work remains queued. Preserve the original 22-module project, exports and exact-byte catalogue.

Bound this pass to: (a) a new path/hash-keyed inventory with PE imports, exports, duplicate groups and explicit scope decisions; (b) read-only inspection of all ten failed exports, with separately recorded baseline and evidence-based recovery attempts on disposable program state; (c) discovery accounting for the existing corpus, including executable bytes outside function bodies, entry/export targets and unresolved call/data-pointer candidates. Keep generated scripts, logs and bodies under private work/hl2-decompiled. Never silently mutate the baseline database or count plausible pointer targets as confirmed functions.

Validation: reconcile all installed paths and baseline binary hashes; audit saved report identities/counts and original corpus hashes; record raw instructions, calling conventions, input storage and recovery outcomes for each failure. Audit gaps as uncertain code/data/padding rather than claiming a complete denominator. Check public documentation, source whitelist and publication audit. Runtime code is unchanged, so gameplay rebuilds are not required for this pass.

Completed coverage-pass-20261005-01:126 installed paths/hashes and 632 import edges; all 22 baseline hashes match. Three loader-referenced modules add 1,091 identified addresses/1,090 original bodies. Eleven original failures have separate relaxed-input recovery bodies with verified transaction rollback, giving 88,884 identified addresses with some pseudocode across 25 selected modules. The baseline catalogue is unchanged; a path/hash/address supplement audits 301 artifacts and preserves all rejected attempts. Its gamedb navigator misses 263 bodies and has 36 range mismatches. Full discovery/ABI/semantics remain unverified.

Next bounded pass: review a small set of unlabelled RTTI/vtable pointer groups and executable orphan ranges using a disposable project. Require slot/reference and instruction-flow evidence before creating a candidate function, compare inferred boundaries with raw flow, and keep rejected/shared-tail cases. Extend a new database version rather than rewriting this pass. Reconcile remaining optional renderer/video/audio loader branches. The current 22-module audit's1,459,199 gap bytes and 13,830 pointer-slot observations are uncertain evidence, not a count of missing functions. Review the recovered x87 callees/types before treating those bodies as behavior specifications.

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

## Publication validation

Public changes for this pass are documentation only: preserve history/invariants and the source-only whitelist, update evidence and the private checkpoint, check links and audit the public snapshot, then commit/push to bevy-migration. Do not rebuild/replay unchanged runtime code.
