# Local validation — 2026-10-05

## Live monitor feeds and CPU performance work (2026-10-05)

All **285 normal workspace tests and 16 owned-install tests** pass, with strict all-target Clippy, formatting and diff checks. Incremental collision queries match 400 independent sweeps against full rebuilds across moves, rotations and enable changes. Packaged movement (26 assertions) and weapon/door (17 assertions) regressions pass. A further 22 checks verify live monitor captures, camera selection/state, zero missing textures, pose agreement, two landmark/inventory transitions and the retained projectile replay. Captures were inspected locally.

Bevy now renders the owned Breen and Kleiner camera rooms into a shared 256x256 target, with authored scanline overlays and a supported subset of material proxies. The Breen capture still has the map's Combine slate over the portrait, and the Kleiner scene is not staged correctly. These controlled feeds do not prove the normal broadcast or security scene works. Camera sky/fog, area connectivity, client camera ordering, recursive feedback and remaining TV noise proxies are incomplete. The retained host shares camera/entity state but has no live render-target adapter.

The performance regression remains material. Default-VSync 1080p measured about 30 FPS both before and after CPU improvements. Mean fixed simulation fell from 4.02 to 2.61 ms per tick; diagnostics fell from 1.39 to 0.022 ms per frame. A subsequent constant-pose upload fix plus no-VSync measured about 35 FPS; this is not an isolated comparison for that fix. GPU/render-thread cost and 144 FPS are unverified. See [measurements and reproduction](performance.md).

Final Bevy package SHA256 `443618608F260D49BF504FA457CD318104EF8F4833B56DBA137B329CA7CC8E16`; retained package `0A6AC5F3C4966ACD9C10AA239056C16399E69EACC49909C3CB79183CC4C9F987`. Evidence is in ignored artifacts/bevy-monitors*, retained-monitors-physics and performance*. Native decompiler evidence, databases, reference tools and captures remain outside the public source snapshot.

## Owned explosion emitters and developer overlay (2026-10-05)

The shared emitter adds distinct grenade smoke/fire/embers/debris, electric bounce/expiry sparks and two expanding AR2 shock rings. Five emitter tests cover emission/lifetime ranges, ring diameter/continuous geometry, pause-safe dispatch, delayed catch-up and bounded/reset state. All **278 normal tests and16 owned-install tests** pass, with strict all-target Clippy and formatting checks. Owned tests decode the nine particle materials with their intended blend modes.

Packaged Bevy captures `bevy-particles-fire`, `bevy-particles-smoke`, `bevy-particles-ball` and `bevy-dev-overlay` pass **26 checks**: actual weapon dispatch/ammo use, distinct combustion/smoke lifetimes, unique effect IDs, two live final rings, sparks, no missing effect assets, frozen clocks and visible live developer statistics. The retained host also completed `secondary-projectiles.json`, including both projectile types. F1 preserves the prototype's compact four-row header, amber control hints and bottom status bar using its MIT-licensed ProggyClean font. Packaged 720p/1080p captures were inspected, and real Windows F1 presses independently verified toggling on and off; the frame rate is a rolling real-time measurement, including paused frames, not a benchmark against Source. Screenshots were inspected locally.

Bevy package SHA256 `8B1F83391676239D2A0BFF31F396519390B6544555ADD7E0069F2FD0B70EFA50`; retained package `9A1FF807EBC705E1BD5408B2EB0AD2DDF8C2E3FBB80B67E256D5DBD9868AB5B6`. Native comparison attempts preserved stock configuration fingerprints but missed the requested explosion states; their captures are rejected as fidelity evidence. Emission constants are corroborated by private retail binary anchors and the pinned published SDK, rather than those unsuccessful captures. Ambient smoke tint, RNG, cached particle collision planes/random bounce damping, no-Z glow pixel visibility, exact beam tessellation/scrolling, soft-depth shading and particle-manager scheduling remain incomplete. The new effects are a verified implementation increment, not a 1:1 rendering claim.

## Retail animation research checkpoint (2026-10-05)

Private read-only evidence identifies the inherited retail actor eye-position entry through RTTI-associated tables, the m_latchedEyeOrigin datamap field and the literal eyes attachment path. The station gesture census resolves 65 nonempty event occurrences and their 311 automatic child-layer references, with 59 empty-parameter events recorded separately. This research changes no runtime behavior and does not count as a gesture playback test. See [research notes](research.md) for anchors, scope and remaining work. The latest code tests, captures and executable fingerprints remain those of the attachment milestone below; the subsequent documentation update requires no rebuild.

## Animated gaze attachments (2026-10-05)

273 normal workspace tests and 15 owned installed-game tests passed, with strict all-target Clippy, formatting and diff checks. The reader validates attachment tables, names, bone indices, supported flags, finite nonsingular transforms and view offsets. Owned Barney, Kleiner, human, G-Man and metrocop models decode attachment bases that match their loaded skeletons.

The rebuilt Bevy package repeated all 26 authored-attention assertions. Thirteen additional checks verify active animated attachments, finite unit forward vectors, pose-cache coverage and self targeting exactly 100 units along attachment forward. Monitor/player/self/final images were inspected. The new presentation used 30 actor pose samples for 40 eye draws and 29 attached NPC origins in this controlled scene; this is instrumentation of the eye pass, not an FPS comparison. A rebuilt retained-host scene/pause/cancel capture completed and was inspected. It continues sharing scene interests without the Bevy iris adapter.

Bevy package SHA-256 7F058DDC849FA4FBB4C7888D911F0AA57B1DD9C72CA49D049B5D7331D7762B37; retained 9845C39907430C898E0B772D57D6AEC37185A6AAA918DB05BB5041B5B6F151A3. Latest evidence is in ignored artifacts/bevy-attachments*. Published SDK attachment behavior and the subsequent reviewed retail attachment path support this adapter; complete native PVS/think-time latching and derived actor eye overrides remain unverified. Dynamic monitor textures remain unsupported and explicitly reported as absent procedural _rt_camera content. Head poses, layered gestures, flexes and ordinary first-level campaign completion remain unfinished.


## Authored LOOKAT attention (2026-10-05)

272 normal release workspace tests and 14 installed-game ignored tests passed, with strict all-target Clippy and formatting. The added owned security02 test checks named marks, monitor/player overlap, authored intensity, held scene time with continued interest refresh, cancellation tail expiry and target deletion. Synthetic tests check start-time binding, target aliases, missing-target diagnostics, non-NPC no-op, queue priority and zero-time ramp boundaries.

The packaged Bevy security02 fixture passed 26 assertions across monitor/player/self/end captures: both eye meshes consume the same authored world target, iris bases change, scene pause refreshes interests, UI pause freezes both clocks, overlapping events retain queue order independently of importance, and cancellation expires the tail. Captures were visually inspected. Barney's pose is explicitly seeded and unrelated entry triggers disabled before room entry; no ordinary campaign or native differential behavior is inferred. The retained packaged fixture verifies shared queue execution/pause/expiry, while its iris presentation remains unchanged. The combined packaged weapon regression passed 17 assertions.

Private pinned-SDK C++ curve reference comparison passed all 2,974 sampled points from 28 owned LOOKAT events; maximum absolute error was 0.000000476. This validates compiled default-curve math against the SDK, not the installed retail AI implementation. The retail catalogue supplies view-target network registration anchors only. Head poses, gestures, facial/eyelid flexes, lipsync, previous-target retention, random/tactical/synthetic interests, PVS gating and exact actor scheduling remain unfinished.

Bevy package SHA-256 627A5C0B7ACE97175771FD5D9655536A8C4DC6CD71589A92B6157F81AFB86014; retained package C4441464244EDBE0D170704E1D14A3A24DB33CDBC6F649B824D406643D018DC9. Evidence remains in ignored artifacts/bevy-attention*, with reference sources/harness/data kept outside public checkouts.


## Bevy shared pause/console and campaign host (2026-10-05)

All 267 normal workspace release tests and thirteen owned-install ignored tests pass, with strict all-target workspace Clippy, formatting and diff checks passing. Portable extraction preserves the parser/history command bodies; added regressions cover Unicode editing/completion, menu selection, input-transition consumption, setpos/getpos semantics, landmark placement, weapon deadline rebasing and input-only changelevel behavior. The first synthetic trigger test lacked collision brush planes; its fixture was corrected before the final passing run.

The packaged console/campaign fixtures pass 45 checks: cheat gating, quoted separators, Unicode input/deletion, queued entity input, frozen scene clocks, selection-before-pause cancellation, returning from Console to Pause, outside clicks, held-attack consumption and release, actual paused/resumed audio sinks, 720p/borderless1920x1080 captures, two real station exit transitions and failed/direct map loads. Station01-to02 arrival is (-4256,-152,64), and station02-to03 is (-5176,-4486,64), matching the owned landmarks. Consumed pistol ammunition carries 17 then16 rounds and the reset scene clock permits firing. A preliminary run exposed repeated station03 reloads from a no-touch changelevel brush; the final run has exactly two transitions. Source's published SF_CHANGELEVEL_NOTOUCH0x2 contract is the reference for this repair, not complete retail transition equivalence.

Successful loads remove old map draws/audio and replace resources; the final direct-map return to station02 observes233 entity mesh draws, zero pose/visibility mismatches, five cameras,972 live mesh assets and336 image assets. Those are bounded fixture counts, not total memory/performance measurements. Missing-map failure preserves player/scene state; console history/cheat state survives successful loads. Captures of the menu, console, direct map and final station03 view were inspected.

Bevy package SHA256 `2451FA2168C164D773A0D0E4F65DA5EF25AE963B070E041E7D2D8A1E9CDA8411`, built `2026-10-05T04:19:51.8087859Z`; shaders are unchanged from the prior effects milestone. The same package passes all17 combined door/weapon and26 bench movement assertions. Retained package SHA256 `F5783625C947D08DA441BF59A2F60C04781029ADE0CD662F4C55D900FFC6FDDA`, built `2026-10-05T04:21:58.6495258Z`, completed its existing timed pause/console replay with cheats/unknown/missing-map handling, zero unintended shots and inspected captures. The initial direct invocation omitted the retained CLI's `view` subcommand and was corrected before this successful run. Local evidence is artifacts/bevy-console-*, bevy-campaign* and bevy-console-regression-*.

Resume, Console and Quit are the implemented menu subset; save/load/options and the full Source registry remain missing. Input/layout migration and controlled teleported trigger progression do not establish ordinary campaign completion, native font/render parity or complete transition behavior. Saved entity/AI/global state, full player motion/crouch transfer, scripted attention/facial animation and first-level gameplay remain unfinished.

## Bevy projectile and impact presentation (2026-10-05)

All 260 normal workspace release tests and thirteen owned-install ignored tests pass, with strict all-target workspace Clippy, formatting and diff checks passing. The ignored checks require HL2_ROOT for the HUD fixture; the initial unset-variable run was corrected and the complete suite rerun. Added coverage includes bounded receiver projection, 256-mark retention, preload-only impact dispatch and a real trainstation door collision hit against its posed render mesh. No original-engine particle/studio-decal equivalence is claimed.

Five packaged effect fixtures pass 40 assertions: real observed grenade/decal draws, pose agreement, paused simulation, secondary-ammo consumption, delayed ball launch, contact explosion, bullet-mark raster changes and marked-door opening. Captures were inspected. Their functional package was `8FAB2598F1C4E9C0CAF1A8F1F512169F5FA2BDE9FED8B094905765F0203BB2B1`; the final report-wording-only rebuild is `E01AF842D27665F861A8892A981A3B3C78F5238D9F1E865B544182BEC2DA0DE0`, built `2026-10-05T03:45:24.7691506Z`, and passes the combined 17-assertion door/weapon fixture. The effects shader hash is `1A2755078D438BC13A90C558D873F75102343FCA3283B6C4BCEEFD15C665673C`. The retained host was rebuilt to `A858A78F2D4D11E6DFD82CA10976873489EC2C00F2AF293E4CEA841135565E42` and its bounded smoke capture inspected. Local evidence is under artifacts/bevy-effect-final-*, bevy-effects-final-* and bevy-effects-retained-*.

Five unused sand/shot base textures are absent in the tested owned installation and remain named preload errors. Sprite/model loads and emitted playback requests succeed in the fixtures. Marks follow current entity/rigid transforms, but later skeletal deformation is unfinished. Native particles, dynamic lights, sparks/rings and exact gamma/shader behavior remain incomplete. The door projection repair fixes a retained-host gap as well: raw bind vertices and an approximately one-unit collision/render separation prevented marks on this leaf.

## Deferred door opener inputs (2026-10-05)

The shared OpenAwayFrom input now resolves a named entity, player, caller or activator at delivery time and chooses the prop-door swing using its current origin. Missing names retain the default forward swing. Locks and explicit opendir override the target; repeated Open/OpenAwayFrom inputs do not retarget an already opening/open prop door. The published props.cpp handler corroborates this contract; native linkage/blocker handling remains incomplete.

All 258 normal workspace release tests and strict all-target Clippy pass. The prior eleven owned-reader checks remain valid: no asset reader changed in this increment. The packaged 330-tick test-inputs/bevy-door-inputs.json fixture passes eleven assertions for both opener sides, both leaves, closed collision, input support, capture completion and pose agreement. Its screenshot was inspected. Final Bevy package SHA256 `0CB871816976CDDC867AD2D2C8CF26E14C93270BE14AF275E6D53657C39E47AF`, built `2026-10-05T00:33:28.3574059Z`; shader hashes are unchanged. The retained package was rebuilt to `EA9182C7EFE35EED59574166ED8239AD4F31E7695F7BF51619BB596ACB811AF5`. A final packaged combined weapon/door rerun also passed all 17 assertions after the repeated-open guard. Evidence stays local in artifacts/bevy-door-input*.

## Bevy sky, eyes and paired-door milestone (2026-10-05)

All 256 normal workspace release tests and eleven explicit owned-install ignored tests pass, including bounded MDL eyeball parsing/projection and real Barney, citizen, G-Man and Vortigaunt metadata. Strict all-target workspace Clippy passes with the same two vendored warnings; formatting and diff checks pass. Logs are local under artifacts/bevy-sky-eyes-doors-{final-tests,owned-tests,clippy}.log.

Packaged Bevy executable SHA256 `3080C3F9478AE0F4895DF50D7F820AD829A04DF8EE1A7956B07F4C3F92D29823`, built `2026-10-05T00:19:33.2396796Z`. Source shader SHA256 `B8142E26AEB52878C7A93BAA8C43C734946FBFE347B67FB0D0F55B0CC70781E4`; HUD shader remains unchanged. Retained-host package `59637DF46F3E736DA2325BCFEAD4CB223BB39C8D4DFCD44B2F58E320898FC80A` was rebuilt after the shared door/sky changes and completed a bounded packaged smoke capture. The separate main checkout was not modified.

The packaged weapon/door fixture passes all 17 assertions, with paired use now completing both entrance leaves. The new 330-tick door-swing fixture plus two eye captures pass 14 checks: both leaves swing away from inside and outside openers, return to their authored closed poses, block the closed-door ray, and keep visible/collider poses aligned. Both Barney eyes select the player from either side and change their projection rows; other actors select visible NPC targets. The final door, two close eye views and outdoor/blocked sky captures were inspected. Evidence stays local under artifacts/bevy-door-swing.*, bevy-eyes-final-{left,right}.*, bevy-sky-final-{outside,blocked}.* and bevy-doors-eyes-assertions.json. Clouds and miniature Citadel scenery render behind the station; opaque closed doors and entrance pillars occlude them.

Eyes use owned sclera/iris textures and bounded animated-bone projection, corroborated with selected retail StudioRender methods and published shader references. Four Vortigaunt materials now use their explicit authored Eyes_dx8 fallback; the first-map material errors are the three unsupported dynamic monitors. Full EyeRefract, glints, eyelid/facial flexes, native attention queues and VCD LOOKAT remain unfinished. The nearest visible player/NPC policy is an explicit approximation. Sky rendering is LDR with leaf eligibility and independent world depth, without native polygon sky masks, HDR or fog. Paired door use respects fixed opendir overrides, but blocked-door reversal, native master/slave ownership and arbitrary initial positions remain unfinished. These controlled checks do not establish a complete campaign or native parity.

## Bevy shared audio milestone (2026-10-05 local)

Commit 4c443658 shares the retained symbolic sound resolver, actor-gender alternatives, WAV/MS ADPCM/MP3 decoder and viewmodel sound-event cursor. Bevy preloads referenced alternatives before App.run, emits owned PCM through monitored audio sinks, and pauses/resumes pending and active players. The recorded milestone passed 252 normal tests, ten owned-install tests and strict Clippy. Its packaged 625-tick weapon fixture passed 17 checks with 23 actual sinks started and no playback errors. The controlled first-map scene fixture started seven sinks including Barney speech, exercised two pauses and one resume, and ended with all five remaining sinks paused. All 155 registered wave references decoded; three unused named references were missing, with no emitted request failures. Logs/reports stay in artifacts/bevy-audio-*.

Audio milestone package hashes are superseded by the sky/eye/door packages above. Streaming, spatial mixing, Source DSP/soundscapes, channel semantics, native random selection and lip sync remain incomplete. Linux audio linking is unverified; Windows is the tested platform.

## Bevy shared HUD migration (2026-10-05 local)

The retained Source-resource HUD now lives in engine-independent `hl2-ui`. Both hosts consume its explicit CPU draw canvas; Bevy uses a separate 2D overlay camera with pooled meshes and normal/additive materials. Parsing/layout, font-cell metrics, glyph blur/scanlines, numeric animations, secondary-panel geometry, bucket selection and white-default remapping remain shared. File reads happen before the app runs. All 250 normal workspace release tests and ten owned-install ignored tests pass; strict all-target workspace Clippy, formatting and diff checks pass with the same two vendored warnings. Logs: artifacts/bevy-hud-final-{tests,owned-tests,clippy,build}.log.

Final Bevy package SHA256 `692041E2AFE8D6A6838960B136427CF76C419D8CBACF92193C9736DD77C667C0`, built `2026-10-04T23:18:16.0187428Z`. HUD shader SHA256 `AB587C5AB3A5022D24F0FDAA6872FC7561827C085C2F7705DB5AAB12CC54EE38`; Source shader remains unchanged. The final packaged crosshair captures at 1280x720 and borderless 1920x1080 each have exactly five single pixels at the accepted retail offsets, spanning 22x22. The final 345-tick HUD fixture retains pistol ammo while SMG is pending, changes to SMG, fires one grenade and ends with shotgun pending while SMG remains active. The inspected final image contains selection plus primary/ALT ammo; render diagnostics report 114 quads, 115 pooled meshes and 66 owned textures. These counts do not prove performance gains.

Start/intermediate/end secondary-panel captures were inspected on the preceding 4FE789 package with the same UI adapter and animation logic. Its door/weapon fixture passed 17 assertions and bench fixture passed 26; final source additionally calls the same draw_status observation before receiving the suit, updates CLI/report wording and adds the HUD fixture. Retained host package `F0603FA01C67FD4EB49CD2E6C5FD32DD83A4F3BD4CAB08B9F2AAF14B5EF05C06` passed the white-crosshair raster measurement and nine-capture hud-slide fixture, with no audio/texture errors; the SMG endpoint image was inspected. Evidence is local under artifacts/bevy-hud-* and hud-slide-*.

Native GDI font output, full hide gates/animation commands and exact linear-versus-gamma blending remain incomplete. Pause menu/console, audio, projectile/impact visuals and campaign host transitions still need migration. Underlying scene/AI/physics and Source-rendering limits remain; this is not a completed campaign or full original-engine parity.


## Bevy entity/gameplay migration (2026-10-04)

The shared simulation now retains entities, weapons, projectiles, selection, NPC locomotion and actor preparation; both hosts use the same implementation. The five moved full modules were compared byte-for-byte after newline normalization, with only scene_sound_request visibility widened to preserve a retained host test. All 249 workspace release tests and nine explicit owned-install tests pass; strict workspace/all-target Clippy passes with the same two vendored warnings. Logs: artifacts/bevy-gameplay-tests.log, bevy-gameplay-owned-tests.log and bevy-gameplay-clippy.log.

Packaged executable SHA256 `604244930786CD48B5C16FDB0F1C82C476FDB3572B7FA5DA60E628AC0DCD7238`, built `2026-10-04T22:47:33.7451967Z`. The shader remains unchanged. The packaged bench fixture passed all 26 existing movement assertions with rigid bodies enabled. The packaged station doorway/weapon fixture passed 17 assertions: closed/open/reclosed walking and ray behavior, E use, selection retaining the active weapon until confirmation, pistol consuming one round, SMG spawning one grenade/consuming one secondary round, and AR2 charging before launching/consuming one round. Final 233 owned entity mesh draws have zero transform/visibility mismatches. Private validators and captures stay outside public source. Closed/open doorway and AR2 viewmodel captures were visually inspected.

This validates host integration, not original-engine parity. Retained AI, choreography and physics limitations persist. HUD/audio, impact/projectile effects and campaign host transitions remain unmigrated; audio cues are explicitly reported as unplayed. Missing clips remain bind poses, dynamic monitors remain unsupported, and sky/Source shaders remain incomplete. No FPS improvement or playable first-level campaign is claimed.


## Bevy player/collision migration

The walking preview package is SHA256 `3167DB5CAA551628CC0CF07CC9588F5425907BCD8834E33FC58A1D32B57C448A`, built 2026-10-04T21:45:47Z. Its packaged shader remains `DF39B6090638EC5238CAE9F93A1F29198D15EDE2B45295348E66E2463A3CF249`. `bin/build-bevy-info.json` is authoritative after rebuilding. The static-only package below is superseded.

The collision adapter, private convex implementation and NPC probes moved unchanged into `hl2-simulation`; both hosts import the same modules. Their 25 tests are preserved. The workspace suite passed 243 tests; a final Bevy rerun passed 11, including an additional input-capture regression (244 unique normal tests across the final source). Strict all-target workspace Clippy passes. The Bevy adapter uses an explicit 15 ms fixed timestep, samples look/input before the fixed loop and publishes the eye position afterward. Capture/resume discards transition-frame pointer motion and suppresses held jump until release.

The packaged station02 movement fixture completed all 280 host ticks and passed 26 report assertions. It settled on the bench at feet Z 36.17742157; ordinary and airborne-crouched jumps retain the same eye height and velocity while crouching raises feet by 36 once. Clear air uncrouching restores the matching trajectory. Paused samples preserve the complete player state and tick count; held jump cannot jump again on landing until released. The final 1280×720 capture was visually inspected. This is a controlled shared-player/collision replay, not an original-engine differential replay or campaign proof. Two named dynamic monitor materials remain unsupported; there are no model-load, texture-budget or capture-write failures.

Evidence stays local: `artifacts/bevy-movement.{json,png}`, `bevy-movement.assertions.json`, `bevy-movement-tests.log`, `bevy-movement-final-bevy-tests.log` and `bevy-movement-final-clippy.log`. Rendering still uses initial entity poses/bind poses. Rigid-body dynamics remain frozen to avoid moving colliders without their visible meshes. Weapons/HUD, interactive pause UI/console, NPC animation/AI, scene/entity I/O, audio and campaign progression have not migrated. See [migration instructions](bevy-migration.md).

## Bevy migration preview

The separate `bevy-migration` branch packages a static owned-map renderer, not the retained gameplay runtime. Final `bin/hl2-bevy.exe` SHA256 is `CD497A0BE933FEB94083C709A0CFC15E67C4877876C81A4EE9FAC902BA1ADD09`, built 2026-10-04T21:17:54Z. Packaged `source.wgsl` SHA256 is `DF39B6090638EC5238CAE9F93A1F29198D15EDE2B45295348E66E2463A3CF249`; `bin/build-bevy-info.json` is authoritative after rebuilding. See [build instructions and limits](bevy-migration.md).

The final source passes all 241 normal workspace tests (eight Bevy, 141 retained runtime, 34 core, 58 asset readers), all nine explicitly invoked owned-install checks, formatting and strict all-target workspace Clippy. Existing vendored miniquad warnings remain. New tests cover coordinate handedness/pitch, BSP-only winding normalization, material properties, visible-material selection and shared texture caching under an allocation budget. Existing gameplay tests validate the retained runtime; they do not establish gameplay in Bevy.

Both final packaged runs used `launch-bevy.cmd`, completed GPU capture at 188 frames, exited successfully and were visually inspected. Installed Steam build 19307283, patch 9912070, NVIDIA RTX 3070/Vulkan:

| Map and capture | Result |
| --- | --- |
| `d1_trainstation_01`, borderless 1920×1080, Source camera (-4690, -1186, 32.03125), yaw 165 | 314 shared base textures, 273,979,396 decoded bytes (about 261 MiB). Model clock front restored after correcting the different BSP/model winding. Seven named unsupported texture cases remain: three dynamic monitors and four Vortigaunt eye materials. |
| `d1_trainstation_02`, 1280×720, Source camera (-4304, -224, 1) | 233 shared base textures, 201,117,696 decoded bytes (about 192 MiB). Stairs, fences, props and walls render; two named dynamic monitor textures remain unsupported. |

Neither final report has texture-budget failures, model-load failures or capture-write failures. The 512 MiB cap is the adapter's decoded-base-texture budget, not a Bevy limit or a total memory measurement. An earlier preview failed that budget by decoding aliases separately and loading unused textures; it is retained as rejected evidence. A subsequent preview exposed wrongly culled model fronts and is also superseded. Bevy currently selects texture mips up to 2048 pixels versus the retained viewer's 512, so these are not equivalent performance comparisons.

Local captures/reports are `artifacts/bevy-station01-1080-winding.*` and `artifacts/bevy-station02-winding.*`. Static bind poses and initial door transforms are visible limitations. No movement/collision, weapons/HUD, NPC animation/AI, scene playback, audio or campaign progression is running in Bevy yet. Sky passes, Source material proxies, eyes, dynamic lighting, PVS/areaportals and accurate HDR/gamma remain missing. These captures do not prove Source rendering parity or improved performance.

## Retained runtime validation

This remains a partial Rust reconstruction. Iteration14's final release package is SHA256 07561D4CC09310D7C8837B95D44201C1F6B1871AEDCC6C016BD676669D74DCAD, built 2026-10-04T11:01:23.1984992Z and tested through launch.cmd. bin/build-info.json is authoritative after a later rebuild. The 1D9193BB preview below used the same behavior before the final test-source/format rebuild.

## Iteration14: actor speech and collision-checked scene movement

233 workspace tests passed (141 runtime, 34 core, 58 asset readers), as did all nine explicitly invoked owned-install checks, formatting and strict all-target Clippy. New tests cover dedicated NPC masks/self-exclusion, player/NPC blockers, doors, support/steps/drop limits, ground graph ordering, scene ownership, authored motion, cancellation and arrival-only gates. The ordinary Barney model and MDL eye-offset check catches the missing factory default found in the first packaged preview. The two pre-existing vendored compiler warnings remain.

The corrected 25-second Barney fixture passed 33/33 report assertions. It explicitly seeds the actor at the authored security02 monitor target; it does not reproduce or record the whole preceding retail campaign state. The owned run clip moves Barney, a live player blocks him, UI pause freezes actor/game/scene clocks, and SECTION holds the authored 6.8806338-second clock while the game clock continues. Moving the player away allows both real goals to arrive within their 3D tolerances and the actual scene output unlocks/opens the door. A second run verifies cancellation restores loaded idle and retains a fixed position. No scene Resume command bypasses this gate. Captures of movement and the opened door were inspected; audio/model/texture errors and simulation budget exhaustions are zero. Artifacts are local: iteration14-barney-grounded-report.json and its assertions. The earlier BF65 preview failed with MissingActor because Barney had no loaded default model; it is superseded.

The first-map speech census now resolves all 72 SPEAK actors across 44 scene references/40 unique scenes and 67 cues. All 75 registered wave files decode (71 PCM WAV, four MS ADPCM), with zero failures; 74 are eligible for those actual actors. Symbolic npc_citizen.pain04 now resolves the installed male actor's wave while preserving the female alternative. Literal WAV/MP3 emission does not implicitly expand gender templates. Native RNG equivalence, spatial mixing, non-English combined speech and lip sync remain unverified.

The final package repeated the Barney fixture (33/33 assertions) and G-Man actor/speech fixture (12/12). Its initial projectile rerun passed 19/20: the charge-veto capture at playback time8.0 occurred after the recorded charge deadline7.9849998, so it correctly observed a launched ball instead of a pending charge. AR2 stayed selected, clip27/reserve60 were unchanged, and the other19 checks passed. The fixture's capture now occurs at7.91, after its7.9 confirmation and before release; the failed report is retained. The earlier 1D9193BB preview passed20/20 and the compiled scene/sequence control33/33. Those runs had no audio/model/texture errors. The older scene control still uses manual Resume; the new Barney fixture is the movement-gate evidence. A final20-frame borderless 1920x1080 capture retains the verified default crosshair's five single pixels and22x22 footprint. The three older weapon/HUD fixtures remain historical iteration12 checks and were not rerun in this batch.

The corrected final-package projectile fixture passed20/20. Its charge-veto snapshot records simulation time7.9049998 against deadline7.9999998, with AR2 selected and clip27/reserve60 unchanged. Runtime weapon logic was unchanged; this corrects a capture scheduled exactly at the release boundary. Evidence: iteration14-final-package-secondary-timing-report.json and its assertions; the preceding19/20 report remains available.

Only ordinary Barney is registered for movement in this increment. The packaged room route is direct; graph routing has synthetic validation, not a matched retail route trace. Central forward gait steering, arrival-only SECTION resolution and conservative blocked retry do not reproduce native motor acceleration, turning, positive ETA preload, give-way, triangulation, dynamic links or hint locks. Weighted animation blends, eyes/facial shaders, gestures, attachments and the rest of the first campaign level remain unfinished. Barney's visible eye materials currently show the renderer's shader fallback.

## Historical iteration13 checks

| Earlier check | Result |
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

A wider iteration13 private speech diagnostic explicitly failed on the unexpanded `vo/npc/$gender01/pain04.wav`. That result remains historical evidence; iteration14's actor-aware registry and complete first-map wave census above supersede the template-resolution gap.

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

## Bevy shared audio migration (2026-10-05 Canary)

Release workspace tests passed (252), all ten owned ignored tests passed with HL2_ROOT set, and strict all-target Clippy passed. The packaged entity/weapon fixture retained all 17 assertions and started 23 real audio sinks with zero pending/failed requests or preload errors, including MP3 music, pistol, AR2 charge/release, SMG explosion and HUD cues. The controlled first-map security-room fixture started seven sinks including Barney speech, paused twice/resumed once and ended with all five remaining sinks paused. Its 155 decoded references include PCM, MS ADPCM and MP3; three unused installed references remain named absent-asset/script diagnostics (Intro.zoomout, ambience/particle_suck1.wav, ambience/steamburst1.wav), with no failed emitted request. Both packaged captures were inspected. A retained-host packaged smoke run also completed after decoder/event-cursor extraction.

This validates request selection, decoder compatibility, sink startup and host pause integration. It is not a captured-audio comparison against native mixing or an ordinary campaign playthrough. Spatialization, Source DSP/soundscapes, voice-channel replacement and lipsync are unfinished. The original ALSA resolver conflict was addressed by a tiny Rust re-export shim; its 27 used API names exist in the upstream bindings, but Linux compilation/playback has not been tested.

Audio package SHA-256 `389E440B8D3AB58FB22070682EE30E5F4C8BCD1DA295FA8E0725A250607A42CB`; final packaged scene rerun also passed sink/pause assertions. Retained package SHA-256 `254E4C50278CC33AA48A8F40AE7CBE8DFB6BF94A8BFCDC1F62F29EA926CC49D1`.
