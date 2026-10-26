# Lighting follow-up, 2026-10-26

## Ordinary shadows and local lamps

The reported global/vertical direction is the normal HL2 shadow policy, rather than a
request to aim every ordinary prop/NPC shadow away from each nearby lamp:

- Low/simple shadows project straight down. SDK `game/client/clientshadowmgr.cpp`
  `GetShadowDirection` explicitly returns (0, 0, -1) when render-to-texture is inactive.
- Medium/High ordinary shadows use the map's `shadow_control` direction (or the level
  default). This vector is independent of each local baked emitter; it need not represent
  the sun. `SetShadowDirection` normalizes one map-wide vector and refreshes the shadows.
- High additionally casts flashlight depth shadows from the placed flashlight. This is a
  separate light-relative shadow, so the flashlight and ordinary projection can coexist.
- The SDK allows an entity-specific direction override. `C_BaseEntity` normally declines
  it, or delegates to an authored shadow-direction entity. Special entity overrides and
  live `shadow_control` input changes are not implemented/verified by this report.

Installed-client evidence: the exact `r_shadowdir` registrar binds its callback; that
callback selects either the fixed downward vector or the stored global direction based
on the render-to-texture mode. Its setter copies/normalizes one vector and updates shadows
when that mode is active. Exports were retrieved from the frozen index by full installed
module identity, dataset/variant and body hash, and reviewed privately. This confirms the
mode/global-vector contract; a full native scene comparison and all entity overrides
are **not tested**. SDK references: `GetShadowDirection`, `SetShadowDirection` and
`game/client/c_baseentity.cpp` `GetShadowCastDirection`.

HL2-RS already follows this ordinary direction policy in `hl2-simulation::shadows` and
`hl2-bevy::shadows`. No nearest-local-light direction change is made. Visible emitter
brightness and shadows over paper overlays are separate defects under investigation.

The owned `d1_canals_01` map supplies direction approximately
(-0.1677313, 0.0449435, -0.9848077), colour (128,128,128) and cast distance 75.
This is authored map direction, not a nearest-lamp query. The 15 existing shadow-policy
unit tests pass; no runtime code is changed by this report checkpoint.

## Paper and junk at the canal corridor

The sheets inspected near the start corridor are the static model
`models/props_junk/garbage256_composite002b.mdl` (origin approximately 631,-8107,-124),
using the garbage002a/garbage003a materials. They are not BSP overlays. The baseline
packaged capture shows these sheets; the owned-map test verifies their baked static-prop
receiver identity.

SDK `game/client/clientshadowmgr.cpp` `AddShadowToReceiver`, static-prop branch:
ordinary prop/blob shadows are excluded. Render-to-texture shadows may reach static props
when cast by an NPC/player; flashlight projections have a separate path. HL2-RS follows
this policy. Its new owned regression projects onto the actual paper geometry: no
ordinary-prop receiver triangles, 17 receiver triangles with NPC-style reception enabled.
A faint flashlight shadow affects only the added beam, preserving the baked illumination.

No nearest-light or static-prop receiver policy is changed. An NPC Medium/High failure on
these papers has not been reproduced visually, and a native visual comparison is
**not tested**. This result does not establish that all BSP decals/overlays composite
correctly: their depth offset was an initial hypothesis, not the cause of these sheets.
The owner confirmed a physics object at Medium/High: this is the SDK-excluded ordinary
prop-to-static-prop case, so no receiver-policy change is made.
