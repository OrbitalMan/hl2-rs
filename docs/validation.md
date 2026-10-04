# Local validation — 2026-10-04

This remains a partial Rust reconstruction. The latest release package is SHA256 4BBA946E8AAEFEC18CE7746BA2C03CDF86A29DF78A4855EF5EFB596384C7B9EB, built 2026-10-04T06:00:28.2205472Z and tested through the normal launchers. bin/build-info.json is authoritative after a later rebuild. Historical checks below describe earlier packages.

| Current check | Result |
| --- | --- |
| Workspace unit tests | 210 passed: 122 runtime, 34 core and 54 asset-reader tests. |
| Explicit owned-install checks | All six passed: MP3, compressed G-Man speech, security03, G-Man actor events/model and authored walk/run records. These are ignored in the normal suite. |
| Formatting and all-target strict Clippy | Passed; the vendored miniquad dependency retains two existing compiler warnings. |
| Existing weapon/shotgun/selection fixtures | All 27 assertions passed on the preceding iteration12 package; their implementation is unchanged in this batch. |
| Packaged SMG/AR2 projectile fixture | All 20 assertions passed, including partial-clip AR2 charge-time reload/holster veto. |
| Compiled choreography | All 1,837 installed scenes and 15,114 events parsed privately with zero failures. The first map loaded all 44 referenced scenes in the packaged run. |
| First-map scene fixture | 33 authored-controller and sequence-sampling checks passed; explicit Resume bypasses missing movement readiness. |
| Packaged G-Man intro actor fixture | All 12 checks passed: owned model/rig, 17 actor events, two ADPCM voice decodes and navigation reports. Captures use a debug camera, not the original intro view. |
| PC navigation graph reader | Seven synthetic parser regressions pass. Owned first map: AIN37, revision 5366, 163 nodes/341 links/zero tail. |
| Authored locomotion data | Six new synthetic reader/sampler regressions pass. Owned forward walk is 80 units/1s; run is 125.87412 units/0.6s. No runtime locomotion or weighted blending yet. |

The latest G-Man fixture renders the previously excluded `cycler_actor` model and resolves all 17 authored actor events through 18 seconds. Both compressed voice files successfully request backend playback with zero audio errors: mono 22.05kHz, 146,740 frames (6.654875s) for riseshine and 461,823 frames (20.944354s) for gman_02. Output is trimmed to each original `fact` count. Synthetic mono/stereo controls compare independent signed sample values and reject malformed blocks, custom coefficient tables and excessive allocations. Complete blocks and an explicit `fact` count are required; other WAV codecs remain unsupported. These checks do not prove native mixing or that facial/gesture animation accompanies speech. The inspected capture is an idle G-Man in the map's offstage area; intro cameras, fades, compositing, flexes and gestures remain unfinished.

A wider private speech diagnostic examined all 44 first-map scene references (40 unique scenes), 72 active SPEAK definitions and 74 distinct wave entries. It decoded 73: 69 PCM WAV and four MS ADPCM. The diagnostic explicitly failed on `vo/npc/$gender01/pain04.wav`, which requires unresolved actor-dependent gender substitution; the literal asset path is absent. No scene or symbolic-cue lookup failed. This is partial format coverage, not successful playback of every campaign line. The failing diagnostic and exact results remain outside the public source; the six supported owned-install regressions above still pass.

The navigation census on the earlier preview package, with identical graph-reader/report code, decoded 72 installed graphs and reported six BSP revision mismatches. It loaded 78/79 maps; the known zero-byte d2_coast_02 remains rejected. Reports treat missing/rejected navigation as optional data diagnostics instead of aborting world display. Graph decoding and movement curves are groundwork for NPC movement; they do not establish a working first level.

The crosshair fix corrects the screen accessor from width to height and follows the native surface's second UV inset. Fresh 1280x720 and borderless 1920x1080 captures from the latest 4BBA946E package each match the original's five single-pixel positions and 22x22 footprint, with zero audio/model/texture errors. A preceding 1281x721 capture checks integer-center rounding without claiming an original odd-size comparison. Tests cover the owned VTF POINTSAMPLE flag and non-square coordinates. Earlier E0A62CB0 controls used identical HUD code. A destination half-pixel experiment enlarged the dots and was rejected. RGB/tone mapping and native font rasterization are not claimed equal.

The current projectile fixture records three contact grenades and two energy balls, three grenade detonations, ball bounces and two expirations. Secondary reserves are consumed independently of magazine rounds. AR2 reserve stays unchanged during windup; attempted reload/switch cannot interrupt it, and expiry does not inflict radial damage. Grenade flight/explosion and ball flight captures were inspected. The run reports zero audio, texture, projectile-model or visual errors and no capacity/collision-budget failures. Rapier contact behavior, guidance, damage relationships/filters, underwater gates, dissolve animation and complete particles remain approximate or missing.

Accepted original-engine controls corroborate SMG held-trigger reserve progression from three to zero with primary ammo unchanged, its magazine-reload interruption, and AR2 delayed reserve consumption plus charge-time reload/holster veto. Native AR2 convars report radius10, mass150 and duration2. Captures/logs show one ball after release and none later; exact native trajectory, damage and expiry timing were not measured. Unfocused startup, forwarded wait commands and surplus ammo pickups contaminated earlier attempts; those captures were rejected. The original process closed and all six freshly hashed stock configuration entries matched. Three automatic stock metadata writes were recorded separately; this is not a claim that the entire installation was unchanged.

The scene fixture uses actual security03 SECTION6.8806338, Trigger3 7.013968, early STOPPOINT9.5939703 and tail11.1273012. It verifies scene-clock pause/resume, the locked door before its real trigger, opening to90degrees, early completion, tail completion and cancellation. A customs officer's installed motionright SEQUENCE verifies scene-owned sampling, pause freezing, resume and baseline restoration. The first NPC camera intersected a wall and was rejected; the fixture was corrected using owned BSP geometry. Gesture layers, actor movement/readiness, facial flexes, lip sync, camera/train intro and full AI remain unfinished. Unsupported behavior is diagnosed; the run has zero audio/model/texture errors.

Installed MP3 playback now decodes the trainstation cue to stereo44.1kHz, 3,997,440 frames (90.6449s) and successfully requests backend playback. The earlier door-music failure is resolved. Whole-file first decode can block; streaming, spatial mixing, soundscapes, DSP and paused audio remain open.

Current package evidence is under ignored artifacts/iteration13-*; iteration12-* retains the preceding HUD/native-comparison controls. Private native comparisons, cache census and validators stay in ../../work/hl2-decompiled and ../../work/publishing. No installed game data, native code or research database is a build dependency.

## Historical validation — 2026-10-03

This validates a partial Rust reconstruction, not a completed Source translation or playable campaign. The current packaged executable was tested through launch.cmd. Its SHA256 is B8B94759463E387362E6935174DE7CE07608E8218AAAB1956F243E2373622F31, built at 2026-10-03T09:26:51.6771832Z. bin/build-info.json is authoritative after a later rebuild.

## Automated checks

| Check | Result |
| --- | --- |
| cargo test --workspace --locked | 118 tests passed: 62 runtime, 21 common simulation/animation, 35 asset-reader tests. |
| cargo clippy --workspace --all-targets --locked -- -D warnings | Passed; two existing compiler warnings remain inside the vendored miniquad dependency. |
| cargo fmt --all --check and git diff --check | Passed. |
| Release packaging | scripts/build.ps1 compiled/copied bin/hl2-rs.exe used by launch.cmd and recorded its fingerprint. |
| Installed maps | 78/79 parsed. Installed d2_coast_02.bsp is zero bytes and is correctly rejected. artifacts/verification-iteration9.json records the result. Parsing does not validate campaign gameplay. |

Regression tests cover Source resource conditions and localization, font alternatives, numeric HUD interpolation/health pulses/cancellation, QuickInfo thresholds/audio latches, wheel/slot order, reload transfer/interruption, primary cadence/spread, shotgun primary/secondary pellets/pump/deadlines/delayed input, health/battery pickup gates and sounds, bounded MDL sound events and melee ray/hull/facing refinement. The melee geometry remains Rapier-derived, not Source collision equivalence.

The SMG1/AR2 tests additionally cover first-empty-click versus next-attempt reload, unchanged animation/primary deadline, strict elapsed idle-reload deadline and independent half-second sound throttles across release/switches. These use reviewed static native and published SDK evidence; no original-engine dynamic empty-fire oracle was run in this pass. Next-best-weapon ranking and secondary deadlines are still missing.

The first published GitHub Actions run passed formatting, strict workspace Clippy and unit tests on a clean Windows runner: [run 37095841557](https://github.com/kvalls/hl2-rs/actions/runs/37095841557), public commit 002dbde. That run covers the initial 97-test source; later commits receive their own runs. A separate fresh public clone also passed all-target workspace cargo check without private research files.

## Packaged weapon/HUD regression

launch.cmd --input-script test-inputs/weapons-hud.json completed 1,369 frames and approximately 41 simulation seconds on the BC12E81B package. artifacts/iteration8-weapons-report.json and iteration8-weapons-assertions.json are local evidence. Earlier matching checks are recorded under iteration6 and iteration7.

- 64 attacks, 82 traced bullet/melee impacts, 82 receiver-clipped marks and zero unclippable impacts. Three crowbar floor hits played two distinct crowbar impact WAV variants as well as the swing sound.
- Six installed model sound events: four .357 reload events and AR2 rotate/push events. The AR2's distinct engine reload sound also played. 200 audio requests and no decode/playback errors; one low-ammo warning WAV played as the SMG crossed its warning threshold, and one installed empty-click WAV played before its automatic reload.
- All six supported primary weapons were selected, fired and reloaded where applicable. Shotgun shell insertion/interruption/pump and SMG empty-clip automatic reload were exercised. SMG reserve transfer and final clip count appear in the report.
- Primary and secondary attack actions confirmed the pending weapon without increasing attacks. Holding primary attack for over one second after confirmation caused no firing before release. Cancellation retained the current weapon; expiry cleared the menu without switching.
- Stock resources determine the five small boxes/expanded column, colors, rounded corners, labels and icon fonts. Numeric armor at zero fades out. QuickInfo brackets are present during selection; the center crosshair is a separate HUD element. F1 development banners remain hidden by default.

The shipped test input is runtime testing data; no native decompiler scripts or game-derived assets are in the source tree. The starting map still lacks normal campaign weapon acquisition; F3/test loadout does not demonstrate campaign progression. The first pistol and revolver scripted clicks occurred before their draw animations ended and were correctly blocked.

## Packaged shotgun secondary regression

launch.cmd --input-script test-inputs/shotgun-secondary.json completed nine simulation seconds on the BC12E81B package: 277 rendered frames, six attacks, 52 traced impacts/receiver-clipped marks and 73 audio requests with no audio errors. It played two double-shot WAVs, four single-shot WAVs, four pump sounds and all three installed reload variants. Evidence: artifacts/iteration8-shotgun-report.json and iteration8-shotgun-assertions.json.

Held secondary confirmation left six shells and zero attacks until release. Double fire consumed two shells; a second request during cooldown was blocked. With one shell, secondary fire used primary attack. Holding secondary during a one-shell reload left reloading active. At two shells it interrupted reload, and the retained shot fired after release when the insertion deadline elapsed. The last case consumed two shells and twelve pellets. This does not validate Source prediction seeds, recoil or pellet hull traces.

## Physical wheel/click checks

The current package also completed test-inputs/selection-held.json: 203 frames, four attacks and 37 audio requests with no audio errors. Slot selection while secondary was already held confirmed pistol and shotgun immediately without shooting. A new primary click fired while the consumed secondary remained held. Secondary release/repress in one poll fired the shotgun double shot; wheel selection while primary was held confirmed crowbar, retained suppression, then a new click after release and draw completion fired. The first fixture attempt clicked before the crowbar draw deadline and was revised rather than counted as successful release verification. Evidence: artifacts/iteration8-selection-report.json and iteration8-selection-assertions.json.

The preceding package (B941BF020B6B08192AE10AA3EEDB8F76CA0B8E6750B6CEC56D19739BD65D6C18) was launched with --time-scale 0.1 to keep the native-duration selector open long enough for separate Computer Use actions. This run used actual window input, with no input script. The current package additionally checks held secondary confirmation through the normal handlers in its input fixture.

Wheel selected pistol, left click confirmed it, wheel selected revolver, right click confirmed it, Q returned to pistol, and Escape canceled a pending SMG selection. The final report records zero attacks, pistol clip 18 and revolver clip 6. Both confirmation clicks were consumed. F12 saved the capture; F10 wrote the report and closed the game.

Evidence: artifacts/iteration5-manual-report.json, iteration5-manual-input.log and iteration5-manual-pistol.png. Slow simulation was a testing aid; normal launch defaults to time scale 1. A covered-window capture initially showed the foreground app and was rejected; the targeted game was activated and recaptured before input.

Earlier packaged mouse tests established that a 90-pixel upward move increases pitch by 0.103680 radians, and a rightward move decreases Source yaw by 0.103680 radians. The input conversion code remains in place; this iteration's physical check targeted selection rather than remeasuring mouse deltas.

## Original-game comparison and sky checks

The owned original engine was run through the separate work/hl2-decompiled/original-oracle write-path wrapper. Loaded engine/client/server DLL paths were confirmed to be installed stock modules. Post-load commands used the original launcher's built-in -hijack forwarding. The usable localized engine capture is oracle_pistol_localized.png in that private folder. Startup captures made before asynchronous map load are rejected.

The Rust and native captures establish the menu's bucket geometry, weapon labels/icons, font-cell sizing and QuickInfo placement. Fontdue coverage/hinting/glow is not identical to native GDI, and the world/viewmodel shading still differs visibly. This comparison does not prove pixel-equivalent UI or rendering.

The original shotgun was drawn at normal time, then tested at host_timescale 0.1. Separate post-load cfg invocations pressed/released +attack2/-attack2 through the original console. Engine captures oracle_shotgun_ready.png and oracle_shotgun_after.png show six shells before and four after one discharge, with reserve unchanged at 30. Retail button registrations/callbacks and IN_ATTACK2 aggregation were reviewed separately. The earlier baseline showed a crossbow and was rejected; shotgun selection was repeated and visually verified before firing. Computer Use could not activate this original-game window on two attempts (foreground process-id error), so no further UI inputs were issued; the engine's own command forwarding/capture route completed the check.

3D sky uses the camera leaf's packed LEAF_FLAGS_SKY, separately from LEAF_FLAGS_SKY2D. At trainstation spawn that flag is true, as expected. At camera (-4494, 96, 22), a 60-frame run of the preceding package recorded false visibility, zero sky frames and no lookup error; its capture shows an opaque wall with no distant scenery. Evidence: artifacts/iteration5-hidden-sky-report.json and iteration5-hidden-sky.png. The ordinary stairwell/floor captures also show opaque occlusion.

The background pass already cleared depth before the world pass. Sky visibility gating is a verified missing rule that is now implemented; these checks do not reproduce the user's exact through-wall viewpoint and do not establish a universal fix. 2D sky textures, fog and general PVS/areaportal culling remain missing.

## Translucent depth comparisons

The preceding 4056 package reproduced bright window/light-shaft patches, hidden entrance columns and cyan barrier effects drawing over opaque map walls. The reviewed backend disabled depth testing whenever a material disabled depth writes. Depth comparison and the write mask now apply independently, and clears temporarily enable writes then restore the current mask.

The AD3ABF2A package repeated 60-frame captures at camera (-4494,96,22), yaw zero, and (-4100,-2000,150), yaw zero, on d1_trainstation_02 in flight. Matching before/after images show the hidden effects and columns removed while the visible hall geometry remains. A front-side capture at (-2700,-2000,150), yaw180, still shows the entrance columns. No texture/model load errors were reported. Evidence: artifacts/iteration7-depth-before.png, iteration7-depth-after.png, iteration7-hall-before.png, iteration7-hall-after.png, iteration7-entry-visible.png and the corresponding reports. The later package changes automatic empty-fire behavior and retains the same rendering code. These views establish correction of the reproduced depth defect, not full Source shader equivalence or exhaustive visibility coverage.

## Movement checks

Native-method-backed tests cover the first standing/duck jump tick, held-jump release, ledge departure, diagonal command cropping before boost, signed overspeed correction and next-tick air friction across the 140 threshold and ascent/descent transition. See docs/research.md for the exact anchors and arithmetic. These tests use controlled collision worlds; they do not validate full retail movement over installed campaign geometry. Surface properties, moving-ground handling, ground quadrants, suit/sprint gating and crouch transitions remain separate work.

The README showcase was captured with the AD3ABF2A package through launch.cmd at the tested hall viewpoint, using a slow simulation and developer pistol loadout. Computer Use activated the exact Rust window, toggled F1 diagnostics, requested F12 capture, then closed the run with F10. An initial screenshot covered by Discord was rejected before game input. docs/images/hl2-rs-trainstation.png is the deliberately selected public preview; other captures/reports and native analysis remain local. It does not establish campaign completion or serve as a redistributable game asset.

All six stock configuration manifest entries remained unchanged after original-game testing. No installed executable/DLL was patched. Test game processes were closed.

## Native research coverage and limits

The separate native batch covered 22 inventoried runtime modules: 87,793 identified functions attempted, 87,783 exported, 10 individual failures and 20,132 warning comments. coverage.json and hashes stay in work/hl2-decompiled. These are approximate pseudocode exports; most methods remain untranslated, and the inventory is not every installed tool/library/episode.

Source reconstruction is manual: installed formats/resources, published SDK references, selected retail binary methods and original-engine tests support the implemented behavior. Export completion is not exact source recovery or automatic Rust translation.

## Public source audit

A source-only snapshot of the 112 intended public files passed universal-modder `publish check` against the owned installation: zero failures, one warning. The warning is the upstream miniquad Metal debug-capture path `/Users/fedor/wtf1.gputrace`, behind a disabled capture flag; it is not a local user's path or a game file. The vendored crate retains its MIT/Apache licenses. Cargo metadata resolves the workspace from the snapshot without native research. The public main branch starts from the reviewed current source tree; older private development history is retained locally because it once included a research-only Java inspection helper.

## Remaining gaps

NPC navigation/schedules/combat, player damage/death, remaining weapons and other secondary/projectile attacks, recoil/muzzle effects, VCD choreography/dialogue, HEV sentence scheduling, animation blends/IK/attachments, vehicles, save/load and complete campaign state remain missing. Rapier shapes/mass, movement, door blocking, scripted movement and shaders remain partial. Numeric damage-message dispatch and several HUD panels/gates/animation commands are incomplete. The README lists the current boundaries.

Trainstation still reports unsupported inputs including SetPoliceGoal, SetParentAttachment, act-busy queues, tone-map controls, template NPC spawning and logic_choreographed_scene.Start. These prevent faithful campaign progression even though the map loads, renders and the test weapons function. No FAL or crossover was used.


## Earlier LDR sky loader foundation

Eight synthetic tests verify all six actual LDR material/texture references, transforms, bounded patch includes, missing/malformed assets and static 2D VTF limits. The earlier D6E5D582 package contained this loader without rendering integration and completed a 20-frame smoke run. The current iteration integrates it as described below.

## LDR sky and bench iteration

The A598E2C8 sky package completed nine 30-frame camera runs: four cardinal views, up/down, a translated up view, the previous hidden-wall viewpoint and the hall. It loaded all six actual LDR material/texture references with no sky asset, visibility, texture or model errors. Eight sky-visible views recorded 30 background frames each; the hidden-wall view recorded zero 2D and 3D frames. Captures remain in artifacts/iteration9-sky-*. Cloud orientation was visually compared against original-engine captures at matched cardinal viewpoints; the near-vertical views differ slightly in pitch because Rust clamps below 89 degrees. Full native shaders/fog/polygon masking are not reproduced.

The final B8B94759 package repeated the sky preview and all three weapon/HUD fixtures. All 27 private report assertions passed, including confirmation/held-input suppression, reload/secondary behavior, clipped impacts and audio-error checks. Evidence: artifacts/iteration9-{weapons,shotgun,selection}-report.json and corresponding assertions. Prior iteration8 results above remain historical evidence.

An actual packaged drop onto the station bench at x=-2206,y=-1669 previously settled at feet z=64.03125 on its oversized NPC-only blocker. The corrected player filter settles at z=36.59562 on the retained model geometry. The private installed-map sweep independently found z=36.60043, and a bench-gap overlap changed from blocked to clear. Both policies retain the clip brushes/colliders; synthetic tests ensure SOLID, PLAYERCLIP and mixed solid/monster-clip still block players. Evidence: artifacts/iteration9-bench-{before,after}-report.json and private bench-runtime-trace.json. Render-derived model collision remains approximate; native PHY equivalence is unfinished.

Three new movement tests cover crouched jump boost using independent maximum speed, full airborne crouch acceleration, signed backward overspeed and three released chain hops with gravity/landing-friction ordering. These tests use controlled worlds and reviewed retail methods. No complete original-game movement trace over campaign geometry was recorded.

The owned original game confirmed the reported three-shell secondary sequence as 3→1→0, with reserve unchanged: a double shot followed by a one-shell primary fallback. Accepted screenshots and rejected setup trials are documented in the separate original-oracle/three-shell-check.md. All six stock configuration entries still matched afterward, and the original process was closed.

The private Database Method audit and exact-byte catalogue are documented in research.md. Full export accounting passes; native/Rust semantic parity is not inferred from it. No database, pseudocode, decompiler tool or game-derived asset was added to this repository.

## Iteration10: air crouch, contacts and console

139 workspace tests passed (74 runtime, 30 core, 35 asset readers), along with strict all-target Clippy, formatting and diff checks. New regressions exercise the one-time air crouch lift, blocked unducking, a 40-unit ledge, sequential/corner plane clipping, overlap escape, rotated convex bevels and translated collider-cache invalidation. Controlled wall-jump probes now retain the unobstructed apex instead of stopping on false contacts; these are not complete original-engine movement traces.

The E11C2795 package repeated all three weapon fixtures with all 27 report assertions passing, and completed pause/console and bench movement fixtures with 20 additional assertions passing. Captures show crouched apex feet approximately 36 units above an ordinary jump while head height stays nearly equal; both jumps land back on the bench. Console/menu freeze simulation, reject disabled cheats and unknown commands, grant six supported weapons, prevent focused/held-resume firing and return to gameplay. A separate direct-map fixture loaded d1_trainstation_01 and retained it after a missing-map request. Captures/reports remain in ignored artifacts/iteration10-final-*. The final packaged fingerprint is recorded in bin/build-info.json.

The owned pause-menu capture guided title/font/spacing corrections. This remains a partial working menu: Resume, Console and Quit. Ambient audio does not pause; save/load/options and the complete command registry are missing. Original bench-height evidence corroborates private PHY decoding, not public integration. Latest playtest reports of double-door pose/collision disagreement and missing SMG/AR2 secondary-ammo HUD remain open and are next priorities.

## Iteration11: static PHY, entrance doors and secondary HUD

154 workspace tests passed (83 runtime, 30 core, 41 asset readers), strict all-target Clippy and formatting passed. New tests cover bounded PHY offsets/checksums/coordinates/tree cycles, root-bone guards, separate convex pieces, solid-mode/cache separation, door idle-pose collision, secondary-ammo reserves and selection eligibility. Public reader tests use synthetic data. The later HUD geometry and window-mode checks are recorded separately below.

The BD11C68D package, built 2026-10-03T21:05:21Z, ran all three existing weapon fixtures with all 27 report assertions passing. Separate HUD, door and bench movement fixtures passed 21 assertions. Captures/reports remain in ignored artifacts/iteration11-*. Station02 loaded 65 model/skin collision assets with 327 pieces and instantiated 1,312 static convex colliders, with zero native-hull fallbacks or collision warnings. A private census also decoded all 60 unique solid static model names in this map; other maps/formats are not implied supported.

The station bench now settles at feet z=36.17742157, matching the accepted original 36.177422 float-height measurement within 0.0001. Ordinary and crouched jumps land on that height; crouching raises the feet approximately 36 while retaining nearly equal head height. This does not verify complete Havok, prediction or ground-duck timing.

The closed entrance door stops the player at x=-3216.0393. Scripted E changes entity350's rotation from 180 to 270 degrees without moving the player; subsequent forward movement reaches x=-3032.3394 beyond the door plane. Captures show the closed panel and the exterior after passage. The door run reports one existing audio gap: music/HL2_song26_trainstation1.mp3 cannot decode because only PCM WAV is supported. The validator explicitly expects and records this known failure; other model/texture/audio errors are absent. Weapon, HUD and movement runs have no audio errors.

The HUD fixture verifies unarmed/no-suit state, real secondary grants, active pistol ammo retained during pending selection, SMG and AR2 secondary reserves, and return to pistol. Actual captures were inspected; an initial AR2 capture still had SMG selected and was rejected, then the corrected fixture selected AR2. Secondary attack requests currently retain ammo because the grenade/energy-ball attacks remain unsupported; no placeholder projectile is counted as implemented.

Accepted original-engine captures verify static unarmed white and armed yellow pistol reticles at 1280x720. Original selector/SMG captures failed setup and are rejected as dynamic comparison evidence. All six fresh stock configuration entries were unchanged after testing, and the stock process was closed. No native DLL patch, game-derived fixture, scene cache, database or pseudocode was added to the public repository.

The installed HUD Position/Size rules now move the primary ammo panel to make room for ALT ammo: 0.5-second Deaccel outward and 0.4-second return. StopAnimation interrupts an in-progress geometry track. Added intermediate/endpoint/reversal and 1920x1080 geometry regressions; the combined suite passed 156 tests before the final deferred-animation regression was added. The owned 24px crosshair sprite remains unchanged; accepted original/Rust 720p crops differ by one pixel in footprint, so exact raster parity is not claimed. New borderless mode uses the primary desktop's current resolution; explicit width/height provides a 1920x1080 window.

The final packaged SHA256 is 5602BC4857ACFC280FFB734A50ECED212BC506A42D7C360EAD13BA873B1CA695, release built 2026-10-03T21:16:51Z. launch-1080p.cmd completed the nine-capture hud-slide fixture with the requested active weapons and no model/texture/audio errors; start, intermediate and endpoint captures were inspected. Windows fitted that titled window to 1920x1061. launch-borderless.cmd then produced a confirmed 1920x1080 capture. The source-only 125-file publication audit reported zero failures and two warnings for the upstream Metal debug path and its documentation mention.

SetDefaultAnimation now preserves active playback/start time/completion while storing the deferred name, as confirmed by retail callback server.dll 0x10223100. Its targeted regression passed; choosing the default clip after completion remains separate work.
