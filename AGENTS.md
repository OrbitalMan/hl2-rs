# AGENTS.md

## Project

HL2-RS is an experimental Rust rewrite of Half-Life 2 and relevant Source behavior, using content from an owned installation. It is not a passthrough mod and does not load Valve game/engine DLLs. Complete campaign and 1:1 fidelity have not been achieved.

## Hard rules, never break these

1. Keep game assets, native binaries, decompiled code, databases and research tools outside public source checkouts. Read installed content in place; binary analysis uses private copies. Generated screenshots/reports belong in ignored artifacts/.
2. Keep .gitignore as a source-only whitelist. Add narrow exceptions for reviewed source, documentation, fixtures or explicitly licensed project resources. Never force-add extracted game content or generated executables.
3. Commit/push only authorized work and stage only the task's files. The owner already authorized reviewed source commits/pushes to kvalls/hl2-rs, including milestones and checkpoints near 98% of the five-hour quota. Do not repeatedly ask. Never merge unverified replacements into main.
4. Keep the installed game read-only. Approved private working directories are workspace work/hl2-decompiled, work/references and work/publishing. Other unrelated files remain outside scope unless named by the owner.
5. Do not put credentials, tokens or passwords in project files, logs or commits.
6. No FAL, replacement assets, crossovers or unsolicited Discord messages. This phase reconstructs HL2. There is no single-player/offline-only project restriction; multiplayer/netcode fidelity is not implemented or tested.

## How to work

- Read STATUS.md, README.md, docs/DESIGN.md, docs/research.md and docs/bevy-migration.md first. Machine-local detail is in workspace work/publishing/continuation.md; its newest checkpoint supersedes history.
- Latest handoff priority: continue toward complete decompilation in a fresh chat. Follow the private research coverage plan in docs/DESIGN.md before resuming the queued NPC gesture implementation. The owner will start that chat; this chat finishes the checkpoint only.
- For substantial changes, record a bounded design/validation plan in docs/DESIGN.md before coding. Keep steps independently reviewable and reversible.
- Use instrumentation and visual evidence together. The owner explicitly authorizes testing, screenshots and computer use. Record positions, timings, state transitions and image comparisons. Test packaged executables. Do not close user-launched games or send input to unrelated apps.
- Reuse shared implementations: source-assets reads formats; modkit-core owns contracts/player/pose math; hl2-simulation owns shared gameplay/physics; hl2-ui owns portable HUD/menu/console; hl2-bevy and hl2-runtime are adapters. No asset reads/font rasterization in frame systems.
- Continue the approved Bevy migration on bevy-migration. Ask before materially different large refactors outside that scope. Preserve macroquad-prototype and unverified wip/scripted-scenes; main remains the retained host until replacement validation.
- Preserve 15 ms order: sample input first; scene -> weapons -> entity collider poses/query refresh -> NPC/projectiles -> rigid physics -> player. Publish presentation before transform propagation and current animation bounds before visibility. Preserve held input, pause and selection semantics.
- Bevy uses scripts/build-bevy.ps1 and launch-bevy.cmd; retained uses scripts/build.ps1 and launch.cmd. Shared changes require checking both hosts. Keep generated executable/shader metadata current after runtime changes.
- Use Cargo.lock. Run relevant tests, strict Clippy and formatting; verify parsers against owned files and rendering through packaged captures. Give exact test commands and expected results. Documentation-only edits require document/whitelist checks, not gameplay rebuilds.
- Research database/native exports are evidence, not automatic parity. Distinguish inferred names/signatures, SDK behavior and verified retail behavior. Seeded developer actors do not establish ordinary campaign completion.
- Explain changes plainly: actual behavior, verification, limitations and next bounded step.

## Honesty

- Write **not tested** when applicable. Distinguish unit tests, owned-file checks, controlled replays, native comparisons and ordinary playthroughs.
- After two unsuccessful real attempts, stop repeating that approach and record evidence in STATUS.md. Choose a materially different evidence-based approach; continue independent work rather than retrying random variations.
- Record failures/rejected captures alongside successes. Never claim complete decompilation, 1:1 fidelity or FPS guarantees without verification.

## Keep these files updated

- MODLOG.md: newest first; Changed / Why / Tested how / Result / Still broken or not tested / Next.
- STATUS.md: verified revision/builds, current problems, evidence, rejected approaches, reproduction commands and next work.
- docs/DESIGN.md: architecture and substantial plans. README.md: tested capabilities/limitations. docs/validation.md and docs/performance.md: detailed evidence.
- Private work/publishing/continuation.md: precise local checkpoint before quota exhaustion or handoff. When the owner requests a fresh chat, finish the handoff and await further instructions here instead of continuing implementation in this chat.

## Environment

- Windows 11 Pro 64-bit, observed version 10.0.26200; other platforms **not tested**.
- Owned Steam Half-Life 2 app220, tested build19307283/patch9912070; other versions **not tested**. Default installation: C:/Program Files (x86)/Steam/steamapps/common/Half-Life 2; discover another library or use HL2_ROOT.
- Rust1.99.0 stable MSVC, Visual Studio C++ tools/Windows SDK. Bevy0.19.1/wgpu and direct Rapier0.26.1. Switching to Avian was not requested; comparative physics performance is not tested.
- Loader: none. Game owning the player: not applicable; Rust simulation owns state and the installation supplies content.
- Agent: Codex Desktop. Previous implementation run used GPT-6.1 Extra High/Normal Speed; select new-chat settings explicitly.
- Active checkout: workspace outputs/hl2-rs-bevy on bevy-migration; sibling outputs/hl2-rs remains main.

## Runtime invariants to preserve

- Walking uses shared Player/Physics; F2/--fly enables flight. Escape cancels selection before pause; tilde opens console. Consume transition input and pause simulation/audio. Map workers freeze fixed/script clocks; replace map-owned resources only after success. Missing maps preserve the world; saved/global state transfer is unfinished.
- HUD uses owned resources and an ordered CPU canvas for both hosts. GDI font rasterization is setup-only.
- Sky background layer3/order-2, miniature sky layer4/order-1, world layer0: independent depth/color clears and BSP 2D/3D eligibility. Current animation bounds and all active player/monitor views drive visibility; missing data fails open, script-hidden/killed entities remain hidden. Spatial splitting is opt-in.
- Eyes use owned MDL records, animated eyes attachment origin/forward and iris projection before entity transform/scale. Preserve authored LOOKAT ramps, cancellation tails, pause refresh, cycler meshes and one target for both eyes. EyeRefract uses explicit Eyes_dx8 fallback; native head/facial/random/tactical attention is unfinished.
- Door use carries opener origin, opens linked leaves away and honors explicit direction. Blocked reversal/arbitrary spawn/native linkage remain incomplete.
- Keep the known zero-byte d2_coast_02.bsp rejection and station03's known materials distinct from new regressions. Research references are not integrated iw4L crossovers.

Template basis: [AGENTS starter](https://github.com/trevaintdead/ai-game-modding-guides/blob/main/templates/AGENTS-starter.md), adapted to existing owner authorization and this rewrite.
