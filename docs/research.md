# Sources and native research

The requested references are [universal-modder](https://github.com/rehan-remade/universal-modder), [iw4L](https://github.com/vladtrc/iw4L), and [2010-rust-rewrite-mashup](https://github.com/chasmlol/2010-rust-rewrite-mashup). Their local checkouts informed the discovery/backups workflow, owned-asset loading, shared world representation and simulation/renderer boundaries. No crossover runtime was integrated.

| Reference | Checked-out commit |
| --- | --- |
| universal-modder | `15d6f9d5fbd32de9b1884f29ddec3be9133bd912` |
| iw4L | `bfe3c2aa700c06285c12c7cd659e3a5f42468f12` |
| 2010-rust-rewrite-mashup | `f608f85e407ff1b7689d54a9aafdd16e95711ac4` |

The asset readers use Valve's published [BSP](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/bspfile.h), [VTF](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/vtf/vtf.h) and [studio model](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/studio.h) definitions. Geometry decoding uses vmdl 0.2 with a Source-specific adapter; compressed animation decoding is implemented separately. Coordinates are Z-up, with Source yaw about Z.

Movement behavior was checked against [gamemovement.cpp](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/shared/gamemovement.cpp) and [hl_gamemovement.cpp](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/shared/hl2/hl_gamemovement.cpp). Door and logic rules use the published server definitions. Pistol/crowbar timing uses installed weapon scripts, skill settings and the published [pistol implementation](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/server/hl2/weapon_pistol.cpp) and [crowbar definitions](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/game/server/hl2/weapon_crowbar.h), with selected retail binary checks below. Published SDK behavior is a reference, not proof of equivalence to this retail build.

## Separate local native analysis

Installed base HL2 was Steam build `19307283`, patch `9912070`, with x86 binaries. Research lives in `../../work/hl2-decompiled`, outside this repository. That folder contains all decompiler/inspection scripts, private binary copies, hash inventory, Ghidra project, C pseudocode and indices. The Rust build never depends on them.

The earlier bounded server pass exported only ten selected functions. The replacement batch analyzed and exported identified functions across 22 first-party runtime modules: server/client/engine, VPhysics, StudioRender, MaterialSystem, DX9 shaders, filesystem/cache/scene/sound/input/UI libraries, tier0/vstdlib, launcher and hl2.exe.

Across that inventory, 87,793 identified functions were attempted and 87,783 exported. Ten exports failed with overlapping-varnode errors; generated C contains 20,132 warning comments. Server/client/engine exports account for 18,893 / 14,683 / 13,824 identified functions respectively. All copied binary hashes match the recorded inventory.

This is whole-module pseudocode export for the selected inventory. It is not restored original source, proven function-boundary coverage, or a complete decompilation of every installed binary. Tools, third-party middleware and other games/episodes remain outside this inventory. Most recovered functions are unnamed/untyped and have not been translated into Rust. See the private folder's README and coverage.json for exact coverage and failures.

Selected facts verified against the hashed retail server binary:

| Anchor | Evidence |
| --- | --- |
| CreateInterface `0x10469c00` / factory `0x104698b0` | Interface-name list walk, creator dispatch and success/failure status. |
| CWeaponPistol primary vtable `0x105ce070`, slot 272 -> `0x104206c0` | PrimaryAttack identification corroborated with the published SDK. Referenced float32 values at `0x104a84ec` and `0x104a84f8` are 0.1 and 0.2. |
| CWeaponCrowbar vtable `0x105cbc78`; refire data `0x104a8500` | Refire constant is float32 0.4. RTTI/vtable anchors are recorded in the private index. |
| CAI_ScriptedSequence datamap | BeginSequence `0x10245d50`, CancelSequence `0x10245e60`, MoveToPosition `0x10245ec0`. Actor acquisition and busy-actor retry paths examined. Rust currently supports only a subset. |

Addresses refer to preferred image addresses in this exact binary, not reusable signatures. The implementation is manual behavior reconstruction from formats, scripts, SDK references and selected native evidence. More exported C does not automatically supply missing NPC AI, choreography, physics or campaign state. Those need individual implementation and original-game differential tests.

## Runtime boundary

Installed assets -> `source-assets` readers -> `modkit-core` world/animation/movement contracts -> Rust host, rendering and simulation. `modkit-core::ModPlugin` and JSON sandbox edits operate on that shared representation. Native game DLL loading, hooks and decompiled C compilation are absent.

The Windows input dependency is a vendored Rust miniquad crate, not recovered Valve code. Its virtual-key fallback follows [MapVirtualKeyW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapvirtualkeyw); attribution and patch notes are in third_party/README.md.

## Further HL2 milestones

1. Record original-game movement, weapon and door traces and compare tick-by-tick against Rust, including collision edge cases.
2. Decode PHY shapes/material/mass data, implement motion transfer/blocked doors, and add water/ladders and missing movement rules.
3. Implement NPC navigation/schedules/combat, all weapons/items and player damage/death.
4. Complete animation events/blending/IK/attachments and VCD scene/dialogue playback.
5. Implement save/load and full map-to-map campaign state, then validate campaign sequences end to end.
6. Improve Source material/lighting and spatial/audio-format support with matched scene comparisons.

Original saves/config were backed up before testing. No game binary was patched. No FAL calls were made.

## Translucent depth and shotgun secondary behavior

The vendored miniquad OpenGL backend coupled depth testing to `depth_write` and never applied `glDepthMask`. Runtime translucent/additive pipelines requested LessOrEqual comparison with writes disabled, so the backend rendered them without any depth test. The patch separates comparison and write state, enables writes temporarily during a depth clear, and restores the pipeline mask afterward. Matching blocked trainstation views reproduce and correct light-shaft, column and barrier bleed-through. This is a reviewed renderer dependency defect; no native game implementation was copied into the backend.

Shotgun secondary fire uses installed scripts and animation durations together with the pinned published [shotgun implementation](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/server/hl2/weapon_shotgun.cpp). Two shells discharge twelve pellets; a single remaining shell uses primary fire. Reload interruption waits for two shells for secondary input and retains the insertion deadline and delayed attack. The original engine's matched captures confirm six shells before and four after one alternate discharge, with reserve unchanged. Prediction seeds, recoil and pellet hull traces remain unverified.

The retail `+attack2`/`-attack2` registrations and callbacks in client.dll were reviewed: callbacks `0x101676d0`/`0x10167710`, kbutton storage `0x104cfaec`, and input aggregation `0x10167020` establish the secondary bit `0x800`. These anchors are private-analysis references, not runtime hooks. The original test used its built-in console forwarding after map load; stock configuration hashes remained unchanged.

## Retail movement ordering

Selected retail server methods establish `FullWalkMove` (`0x1018f080`) applying `StartGravity` (`0x10190b10`) before `CheckJumpButton` (`0x1018b400`). A standing jump adds 160 to the velocity after the first gravity half-step; a ducked jump assigns 160. The jump method applies another half-step before the sweep, and `FinishGravity` (`0x1018d880`) runs after categorizing the swept position. At a 15 ms tick with gravity 600, the reconstructed first standing jump moves 2.265 units and ends at 146.5 units/s; ducked values are 2.3325 and 151.

`CheckParameters` (`0x1018b820`) crops diagonal input before jump boost. `CheckJumpButton` permits a negative capped speed addition, including a neutral forward command. `CategorizePosition` (`0x1018af80`) resets surface friction to one and returns early above vertical speed 140; failed static-ground checks during slower ascent assign 0.25. `PlayerMove` (`0x101900d0`) preserves that result into the next tick's `AirAccelerate` (`0x1018a660`) in optimized WALK. Regression checks cover these arithmetic and ordering boundaries. Moving-ground checks, native ground quadrants, material modifiers, crouch transitions, suit/sprint gating, water and ladders remain incomplete; this is not full movement equivalence.

The address-keyed database review additionally verified HandleDuckingSpeedCrop (`0x1018f7f0`) and Duck (`0x1018cfe0`). Command components are scaled by one third only while ducked on ground; maximum speed is a separate value. A settled normal-speed duck jump therefore uses a cropped forward command of approximately 63.333 for its 0.1 boost, but retains maximum speed 190 and boost cap 209. Airborne duck commands are not cropped. The previous Rust code incorrectly used a 69.667 jump cap and reduced crouched air acceleration. Tests now cover these boundaries, signed backward overspeed and repeated released jumps without an extra ground-friction tick. Automatic hopping was not added; a held jump still needs release. These are selected native rules, not a complete original-engine movement trace.

## Retail HUD and primary-weapon comparison

The retail SMG1 and AR2 vtables (`0x105d0648` / `0x105c9760`) share `HandleFireOnEmpty` at `0x100fd2e0` and `ReloadOrSwitchWeapons` at `0x100fe880`. The first empty attempt sets the latch and emits a sound only after its separate deadline; the next attempts reload. The sound interval at `0x104a8504` is float32 0.5. Empty clicks do not choose a dry-fire animation or advance the primary deadline. The implementation uses separate per-weapon latch/sound state and the strictly elapsed primary deadline for idle reload, corroborated with the pinned [base-weapon reference](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/shared/basecombatweapon_shared.cpp). This pass has selected static native evidence and Rust regression checks, but no dynamic original-game empty-fire oracle. Next-best-weapon ranking, secondary deadlines and custom reload flags remain separate work.

This pass used the installed `ClientScheme.res`, `HudLayout.res`, `HudAnimations.txt`, weapon scripts, localization and fonts. The corresponding published singleplayer SDK reference was pinned to `b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474`. Reference methods include [weapon selection](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/client/hl2/hud_weaponselection.cpp), [numeric HUD rendering](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/client/hud_numericdisplay.cpp) and [animation interpolation](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/vgui2/vgui_controls/AnimationController.cpp).

Matching retail `client.dll` SHA256 is `96C5DD5E3D25CE8F8CC321D123B77DBFE8FA6AAEF39DF518786055F3CD507FA9`. Reviewed inferred methods: `CHudWeaponSelection::Paint` at `0x10226260`, `OnThink` at `0x10226170`, selection input at `0x101e85c0`, rounded panel drawing at `0x102ccd00`, and corner sizing at `0x102ceaf0`. The audit in the separate native folder records vtable/RTTI corroboration and resource overrides. These are manually reviewed anchors, not automatic translation of the entire exported client.

The retail timing constants establish the 0.5-second inactivity interval and 0.75-second additional hide delay. The attack-button mask is `0x801`: both primary and secondary attacks confirm and are consumed. Resource dimensions, colors, selected-column expansion and stock rounded corners replace the earlier invented selector design.

The owned original executable was launched with a private `gameinfo.txt` wrapper in `../../work/hl2-decompiled/original-oracle`. Its loaded engine/client/server paths were verified to be the installed stock DLLs. Configs, screenshots and generated node graphs target the separate oracle folder. Initial startup cfg captures ran before the asynchronous map load and are explicitly rejected as comparison evidence.

The original launcher's built-in `-hijack +command` forwarding was corroborated in `launcher.dll` at `0x10005160`/`0x10002ab0`, with the engine receiver at `0x10228dd0`. Forwarding commands after the client loaded, with `host_timescale 0.1`, produced the usable `oracle_pistol_localized.png` engine capture. The wrapper loads a private copy of the owned HL2 English localization under its own game-directory name. Original configuration hashes were rechecked after the oracle closed. No native DLL patch or custom input injection was used.

Windows font probes and the retail VGUI font loader establish positive character-cell heights, distinct from fontdue's EM size. Rust reads the font's OS/2 Windows ascent/descent metrics to convert them. Fonts with a `yres` range retain their specified pixel height; proportional alternatives scale with screen height. Fontdue coverage, hinting and generated blur/scanlines still differ from native GDI rasterization, so pixel-exact text is not claimed.

Retail primary attack checks corroborate SMG1 0.075-second cadence, AR2 0.1-second cadence and .357 0.75-second cooldown, plus their spread constants. Ammo/damage/slot/icon data comes from installed scripts. Shotgun reload and pump timing and .357/AR2 reload sound events come from installed model sequences. The source spread distribution is reproduced with a different deterministic generator; Source prediction seeds, recoil, projectiles/secondaries, tracers, shell effects and shotgun hull traces remain separate work.

Numeric HUD rules use the installed alpha/color/glow events and Source's Linear/Accel/Deaccel/Spline interpolation, including square-root Deaccel and delayed starting-value capture. HealthLow/HealthPulse/HealthLoop follow installed RunEvent delays with the controller's one-execution-per-event-per-frame guard. QuickInfo uses owned HL2crosshairs glyphs/qi_center, health/clip fractions, 25-health/quarter-clip thresholds, and one warning sound per latch transition. References: [QuickInfo](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/client/hl2/hud_quickinfo.cpp), [health](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/client/hl2/hud_health.cpp). Reviewed retail anchors include numeric Paint 0x1015f430, battery think 0x1021c830, QuickInfo Paint 0x10221b40 and Think 0x10221a10. HUD damage messages and several gates/panels remain absent.

Crowbar trace order follows [CBaseHLBludgeonWeapon](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/server/basebludgeonweapon.cpp): full 75-unit ray, +/-16 hull shortened by 1.732*16, target-origin dot >= 0.70721, then extended center/eight-corner ray refinement. Moving rigid-body origins take precedence over static map state. Collision shapes/masks/material response remain approximate; the sequencing is not proof of complete Source trace equivalence.

3D sky eligibility follows the published [sky view](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/client/viewrender.cpp) and [BSP leaf flags](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/public/bspfile.h). The containing leaf packs area:9/flags:7 at offset 6; LEAF_FLAGS_SKY indicates 3D sky in its PVS and LEAF_FLAGS_SKY2D is distinct. The runtime preserves the loaded BSP through transitions and gates its sky pass accordingly, failing closed with a diagnostic on malformed lookup. Installed-map checks and packaged hidden-sky captures corroborate the flag handling, not a universal through-wall fix.

## LDR sky geometry and player clip masks

The hashed retail engine's reviewed sky methods are MakeSkyVec at `0x1012ab80`, DrawSkyBox at `0x1012ad20` and LoadNamedSkys at `0x1012b060` (research aliases for unnamed exports). The coordinate table and draw-to-material mapping establish the six face orientations, rotated top/bottom, fixed 1/512 UV inset and camera-relative distance. The runtime applies the installed VMT transform after that inset, samples Linear/Clamp, and draws an unlit LDR background before 3D scenery and world geometry. For the station's half-height side textures the vertical transform is scale two; wrapping would repeat clouds below the horizon. HDR compressed reconstruction, exposure/color-space parity, fog and native sky polygon/face masks remain separate work.

The retail server movement vtable's PlayerSolidMask method at `0x10190440` returns `0x201400b` normally and `0x1400b` for brush-only queries, corroborated with the pinned [game movement reference](https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/shared/gamemovement.cpp). Neither includes CONTENTS_MONSTERCLIP. The station has 24 monster-only world clip brushes, including oversized NPC blockers around its static benches. Player sweeps and stationary overlap queries now filter retained world brushes by the brush-only mask. Static bench model geometry remains available; native PHY shapes, model solid-mode equivalence and broad collision-mask fidelity are unfinished.

The owned original executable was also tested with three shotgun shells: the first secondary action changed the clip from three to one; a later secondary action fired the remaining shell as a single shot, leaving zero with reserve unchanged. This corroborates the reviewed ItemPostFrame dispatch to PrimaryAttack at clip one. It does not establish complete shotgun recoil, damage, sound or prediction equivalence.

## Database Method evaluation

The supplied workflow was evaluated with [gamedb](https://github.com/smileybaal/gamedb), pinned to `f75321b1f3dcd259fe80c9f62e296a74cc03d866`, built and tested in the separate private reference folder. All 88 existing C export files across the 22 inventoried modules were indexed. The unchanged parser represented 78,070 of 87,783 successful exports, omitting 9,713 functions; 2,810 indexed read ranges were incomplete or otherwise mismatched. Same-name functions at shared DLL image-base addresses also caused a false cross-DLL edge in a reviewed sky call graph. These measured failures prevent treating the parsed database as a complete translation inventory.

A separate private SQLite catalogue now preserves all 222 source/index/metadata artifacts byte-for-byte, all 87,783 successful export bodies, all ten explicit export failures, binary fingerprints and recorded native call/string references. Functions are keyed by module and native address. Whole-corpus hash/range audits pass, and an unchanged rebuild preserves the database bytes. The three reviewed sky exports retrieve completely by module/address. The supplemental gamedb navigator and exact-byte catalogue remain outside this Rust repository and are not build dependencies.

This improves provenance, focused lookup and omission tracking. It proves export accounting, not original-source recovery or native/Rust semantic parity. Approximate types/function boundaries, warning-bearing pseudocode, indirect calls and unrecovered structures still need review. Each implemented behavior needs a native/resource anchor, a Rust location, a meaningful regression and an original-engine comparison where practical; indexing never marks a behavior complete automatically.
