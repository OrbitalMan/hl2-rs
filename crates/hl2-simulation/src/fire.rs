//! Burning props (SDK props.cpp CBreakableProp::OnTakeDamage/Ignite, EntityFlame.cpp
//! CEntityFlame): which damage sets a prop on fire, the flame's lifetime, its 0.2 s
//! thinks (direct burn damage to the prop, radius burn damage around it) and its sound
//! loop. Not modeled yet: the client "burning_character" particle system (no PCF reader),
//! EF_DIMLIGHT, burning NPCs, FireSystem heat and water extinguishing.
use crate::player_damage::{DMG_BLAST, DMG_BULLET, DMG_BURN};
use crate::projectiles::{Damage, DamageTarget};
use glam::Vec3;
use serde::Serialize;
use std::collections::BTreeMap;

/// DMG_DIRECT (shareddefs.h).
pub const DMG_DIRECT: u32 = 1 << 28;
/// FLAME_DAMAGE_INTERVAL / FLAME_DIRECT_DAMAGE (5 per second) and the radius damage
/// FlameThink deals around an attached flame (4.0 a think).
pub const DAMAGE_INTERVAL: f64 = 0.2;
pub const DIRECT_DAMAGE: f32 = 5. * 0.2;
pub const RADIUS_DAMAGE: f32 = 4.;
/// PROP_EXPLOSION_IGNITE_RADIUS.
pub const EXPLOSION_IGNITE_RADIUS: f32 = 32.;
/// The cues a flame plays (the host preloads them).
pub const SOUNDS: &[&str] = &[
    "General.BurningObject",
    "General.BurningFlesh",
    "General.StopBurning",
];

/// The fire interactions of one prop (prop_data fire_interactions).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interactions {
    pub flammable: bool,
    pub explosive_resist: bool,
    pub ignite_halfhealth: bool,
}
impl From<&modkit_core::PropData> for Interactions {
    fn from(d: &modkit_core::PropData) -> Self {
        Self {
            flammable: d.flammable,
            explosive_resist: d.explosive_resist,
            ignite_halfhealth: d.ignite_halfhealth,
        }
    }
}

/// One CEntityFlame attached to an entity.
#[derive(Clone, Debug, Serialize)]
pub struct EntityFlame {
    /// m_flLifetime (absolute).
    pub until: f64,
    pub next_think: f64,
    /// m_flSize: the average of the entity's x and y extents, at least 16.
    pub size: f32,
    pub started: f64,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Fires {
    pub flames: BTreeMap<usize, EntityFlame>,
    /// Ignitions and burn-outs (report only).
    pub ignited: u32,
    pub extinguished: u32,
    #[serde(skip)]
    pub sounds: Vec<crate::sounds::SoundRequest>,
}
fn emitter(entity: usize) -> u64 {
    (5 << 40) | entity as u64
}
/// A uniform [0, 1) draw from the entity and the time (deterministic replays).
fn roll(entity: usize, now: f64, salt: u64) -> f32 {
    let mut x = (entity as u64)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(now.to_bits())
        .wrapping_add(salt.wrapping_mul(0xbf58_476d_1ce4_e5b9));
    x ^= x >> 31;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 24) as f32
}
impl Fires {
    pub fn burning(&self, entity: usize) -> bool {
        self.flames.contains_key(&entity)
    }
    /// CBreakableProp::Ignite -> CBaseAnimating::Ignite -> CEntityFlame::Create: only
    /// flammable props, once; the flame first thinks 0.1 s later; "General.BurningObject"
    /// loops from the prop.
    pub fn ignite(
        &mut self,
        entity: usize,
        interactions: Interactions,
        lifetime: f32,
        size: f32,
        origin: Vec3,
        now: f64,
    ) -> bool {
        if !interactions.flammable || self.burning(entity) {
            return false;
        }
        self.flames.insert(
            entity,
            EntityFlame {
                until: now + f64::from(lifetime),
                next_think: now + 0.1,
                size: size.max(16.),
                started: now,
            },
        );
        self.ignited += 1;
        self.sounds.push(crate::sounds::SoundRequest {
            origin: Some(origin),
            ..crate::sounds::SoundRequest::patch(
                "General.BurningObject",
                emitter(entity),
                crate::sounds::PatchOp::Play {
                    volume: 1.,
                    pitch: 100.,
                },
            )
        });
        true
    }
    /// The flame ends (burned out, or its entity broke or was removed): StopSound and
    /// "General.StopBurning".
    pub fn extinguish(&mut self, entity: usize, origin: Vec3) {
        if self.flames.remove(&entity).is_none() {
            return;
        }
        self.extinguished += 1;
        self.sounds.push(crate::sounds::SoundRequest::patch(
            "General.BurningObject",
            emitter(entity),
            crate::sounds::PatchOp::Destroy,
        ));
        self.sounds.push(crate::sounds::SoundRequest {
            origin: Some(origin),
            .."General.StopBurning".into()
        });
    }
    /// CBreakableProp::OnTakeDamage's fire rules for one hit of `amount` (already
    /// scaled by the prop's damage modifiers) on a prop with `health` of `max_health`.
    /// Returns the damage to apply (None: ignore the hit) and may raise `health` (the
    /// half-health ignition refills it so the prop burns longer).
    #[allow(clippy::too_many_arguments)]
    pub fn prop_damage(
        &mut self,
        entity: usize,
        interactions: Interactions,
        health: &mut f32,
        max_health: f32,
        amount: f32,
        kind: u32,
        inflictor: Vec3,
        center: Vec3,
        size: f32,
        now: f64,
    ) -> Option<f32> {
        // Only the prop's own flame burns it.
        if self.burning(entity) && kind & DMG_BURN != 0 && kind & DMG_DIRECT == 0 {
            return None;
        }
        let mut amount = amount;
        let mut deadly = amount >= *health;
        if deadly && kind & DMG_BLAST != 0 && interactions.explosive_resist {
            // Burn for a while instead of exploding now: up to 5 s away from the blast.
            let distance = center.distance(inflictor);
            let burn = if distance >= EXPLOSION_IGNITE_RADIUS {
                let factor = distance.min(256.) / 256.;
                (5. * factor).max(0.5) + 0.5 * roll(entity, now, 1)
            } else {
                0.1 + 0.1 * roll(entity, now, 2)
            };
            let ideal_health = health.min(5. * burn);
            amount = *health - ideal_health;
            deadly = amount >= *health;
        }
        let lifetime = 10. + 5. * roll(entity, now, 3);
        if !deadly && kind & (DMG_BLAST | DMG_BURN) != 0 {
            self.ignite(entity, interactions, lifetime, size, center, now);
        } else if !deadly && kind & DMG_BULLET != 0 && interactions.ignite_halfhealth {
            if *health - amount <= max_health / 2. && !self.burning(entity) {
                *health = max_health;
                self.ignite(entity, interactions, lifetime, size, center, now);
            } else if self.burning(entity) {
                // Explode right now!
                amount = *health;
            }
        }
        Some(amount)
    }
    /// CEntityFlame::FlameThink for every flame due: burned out -> extinguish; else
    /// radius burn damage around the flame (the player and other entities within
    /// size / 2, linear falloff) and direct burn damage to its entity. `position` is
    /// the entity's current center (None: gone).
    pub fn think(
        &mut self,
        now: f64,
        position: impl Fn(usize) -> Option<Vec3>,
        others: &[(usize, Vec3)],
        player_center: Option<Vec3>,
    ) -> Vec<Damage> {
        let mut damage = Vec::new();
        let due: Vec<usize> = self
            .flames
            .iter()
            .filter(|(_, f)| f.next_think <= now)
            .map(|(&e, _)| e)
            .collect();
        for entity in due {
            let Some(at) = position(entity) else {
                self.extinguish(entity, Vec3::ZERO);
                continue;
            };
            let flame = self.flames.get_mut(&entity).expect("flame");
            flame.next_think = now + DAMAGE_INTERVAL;
            if flame.until < now {
                self.extinguish(entity, at);
                continue;
            }
            let radius = flame.size / 2.;
            let falloff = |p: Vec3| {
                let d = p.distance(at);
                (d < radius).then(|| RADIUS_DAMAGE * (1. - d / radius))
            };
            if let Some(amount) = player_center.and_then(falloff) {
                damage.push(Damage {
                    target: DamageTarget::Player,
                    amount,
                    kind: DMG_BURN,
                    dissolve: false,
                    direction: Vec3::ZERO,
                    origin: at,
                });
            }
            for &(other, p) in others {
                if other == entity {
                    continue;
                }
                if let Some(amount) = falloff(p) {
                    damage.push(Damage {
                        target: DamageTarget::Entity(other),
                        amount,
                        kind: DMG_BURN,
                        dissolve: false,
                        direction: Vec3::ZERO,
                        origin: at,
                    });
                }
            }
            damage.push(Damage {
                target: DamageTarget::Entity(entity),
                amount: DIRECT_DAMAGE,
                kind: DMG_BURN | DMG_DIRECT,
                dissolve: false,
                direction: Vec3::ZERO,
                origin: at,
            });
        }
        damage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BARREL: Interactions = Interactions {
        flammable: true,
        explosive_resist: true,
        ignite_halfhealth: true,
    };
    #[test]
    fn a_shot_barrel_ignites_at_half_health_then_burns_out_into_an_explosion() {
        let mut fires = Fires::default();
        let mut health = 20.;
        let at = Vec3::ZERO;
        // 5 of 20: no fire yet.
        let hit = fires.prop_damage(1, BARREL, &mut health, 20., 5., DMG_BULLET, at, at, 32., 0.);
        assert_eq!(hit, Some(5.));
        health -= 5.;
        // Down to half: refilled to 20 and burning.
        let hit = fires.prop_damage(1, BARREL, &mut health, 20., 6., DMG_BULLET, at, at, 32., 1.);
        assert_eq!((hit, health), (Some(6.), 20.));
        health -= 6.;
        assert!(fires.burning(1));
        // Another bullet while burning: all of its health.
        let hit = fires.prop_damage(1, BARREL, &mut health, 20., 1., DMG_BULLET, at, at, 32., 2.);
        assert_eq!(hit, Some(health));
        // Flames from elsewhere are ignored while burning; its own flame is not.
        assert_eq!(
            fires.prop_damage(1, BARREL, &mut health, 20., 4., DMG_BURN, at, at, 32., 2.),
            None
        );
        assert_eq!(
            fires.prop_damage(
                1,
                BARREL,
                &mut health,
                20.,
                1.,
                DMG_BURN | DMG_DIRECT,
                at,
                at,
                32.,
                2.
            ),
            Some(1.)
        );
    }
    #[test]
    fn a_far_blast_makes_an_explosive_resistant_prop_burn_briefly_instead() {
        let mut fires = Fires::default();
        let mut health = 20.;
        // A deadly blast 128 units away: 2.5-3 s of burning (5 HP/s) left.
        let hit = fires
            .prop_damage(
                1,
                BARREL,
                &mut health,
                20.,
                100.,
                DMG_BLAST,
                Vec3::X * 128.,
                Vec3::ZERO,
                32.,
                0.,
            )
            .unwrap();
        let left = health - hit;
        assert!((12.5..=15.).contains(&left), "{left}");
        assert!(fires.burning(1));
        // Right next to the blast: 0.1-0.2 s.
        let mut fires = Fires::default();
        let mut health = 20.;
        let hit = fires
            .prop_damage(
                2,
                BARREL,
                &mut health,
                20.,
                100.,
                DMG_BLAST,
                Vec3::X * 8.,
                Vec3::ZERO,
                32.,
                0.,
            )
            .unwrap();
        assert!(health - hit <= 1.0 + 1e-4);
    }
    #[test]
    fn flames_burn_their_prop_and_neighbors_then_go_out() {
        let mut fires = Fires::default();
        assert!(!fires.ignite(1, Interactions::default(), 10., 32., Vec3::ZERO, 0.));
        assert!(fires.ignite(1, BARREL, 0.5, 40., Vec3::ZERO, 0.));
        let others = [(2, Vec3::X * 10.), (3, Vec3::X * 100.)];
        let damage = fires.think(0.1, |_| Some(Vec3::ZERO), &others, Some(Vec3::Y * 5.));
        let direct: Vec<_> = damage
            .iter()
            .filter(|d| d.target == DamageTarget::Entity(1))
            .collect();
        assert_eq!(direct.len(), 1);
        assert_eq!(direct[0].amount, DIRECT_DAMAGE);
        assert!(damage.iter().any(|d| d.target == DamageTarget::Entity(2)));
        assert!(!damage.iter().any(|d| d.target == DamageTarget::Entity(3)));
        assert!(damage.iter().any(|d| d.target == DamageTarget::Player));
        // Burned out after its lifetime.
        fires.think(0.7, |_| Some(Vec3::ZERO), &[], None);
        assert!(!fires.burning(1));
        assert_eq!(fires.extinguished, 1);
    }
}
