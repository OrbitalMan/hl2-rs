# Local validation — 2026-10-03

This validates a partial Rust reconstruction, not a completed Source translation or playable campaign. The current packaged executable was tested through launch.cmd. Its SHA256 is D6E5D5824C721A452236D1F5C1664C0BCE08EDDFC7B8A26266CBFB6E8D71163B, built at 2026-10-03T04:29:04.9089857Z. bin/build-info.json is authoritative after a later rebuild.

## Automated checks

| Check | Result |
| --- | --- |
| cargo test --workspace --locked | 109 tests passed: 57 runtime, 18 common simulation/animation, 34 asset-reader tests. |
| cargo clippy --workspace --all-targets --locked -- -D warnings | Passed; two existing compiler warnings remain inside the vendored miniquad dependency. |
| cargo fmt --all --check and git diff --check | Passed. |
| Release packaging | scripts/build.ps1 compiled/copied bin/hl2-rs.exe used by launch.cmd and recorded its fingerprint. |
| Installed maps | 78/79 parsed. Installed d2_coast_02.bsp is zero bytes and is correctly rejected. artifacts/verification-iteration5.json records the result. Parsing does not validate campaign gameplay. |

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

Trainstation still reports unsupported inputs including SetPoliceGoal, SetParentAttachment, act-busy queues, tone-map controls, template NPC spawning and logic_choreographed_scene.Start. These prevent faithful campaign progression even though the map loads, renders and the test weapons function. No FAL or crossover was used. A user-authorized Codex continuation is configured for 09:26 Canary time on 2026-10-03, then every five hours and one minute; availability and account limits still determine whether a scheduled run can execute.


## LDR sky loader foundation

Eight new synthetic tests verify all six actual LDR material/texture references, transforms, bounded patch includes, missing/malformed assets and static 2D VTF limits. The loader is exported by source-assets but is not integrated into rendering yet. The final D6E5D582 package rebuilt all source and completed a 20-frame launch.cmd smoke run with 671/671 textures and no model/texture errors; unchanged weapon logic was exercised by the preceding BC12E81B package's three iteration8 fixtures. No visible 2D sky support is claimed.
