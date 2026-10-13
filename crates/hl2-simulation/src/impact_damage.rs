//! Physics impact damage to NPCs: CBaseCombatCharacter::VPhysicsShadowCollision ->
//! CalculatePhysicsImpactDamage (SDK physics_impact_damage.cpp) with the NPC table. The
//! retail server.dll table (private physics-research-20261011, damage registry "npc")
//! has the same entries and thresholds. Only the linear path is modeled: the angular
//! (slice) path needs FVPHYSICS_DMG_SLICE props, and FVPHYSICS_HEAVY_OBJECT,
//! FVPHYSICS_NO_IMPACT_DMG/NO_NPC_IMPACT_DMG, dissolve and floating NPCs are not set by
//! anything here yet.
use crate::{
    entities::Scene,
    physics::PhysicsCollision,
    projectiles::{Damage, DamageTarget},
};
use glam::Vec3;
use modkit_core::World;

/// impactdamagetable_t (npc): linear (speed^2 threshold, damage) entries.
const NPC_LINEAR: [(f32, f32); 5] = [
    (150. * 150., 5.),
    (250. * 250., 10.),
    (350. * 350., 50.),
    (500. * 500., 100.),
    (1000. * 1000., 500.),
];
const MIN_SPEED_SQR: f32 = 24. * 24.;
const MIN_MASS: f32 = 2.;
const SMALL_MASS_MAX: f32 = 5.;
const SMALL_MASS_CAP: f32 = 5.;
const SMALL_MASS_MIN_SPEED_SQR: f32 = 36. * 36.;
const LARGE_MASS_MIN: f32 = 500.;
const LARGE_MASS_SCALE: f32 = 4.;
const LARGE_MASS_FALLING_SCALE: f32 = 5.;
/// CBaseCombatCharacter m_impactEnergyScale (keyvalue physdamagescale, default 1). The
/// d1_trainstation_02 plaza cops set 0: native thrown crates break on them and only
/// make them flinch (oracle T015556Z).
pub fn energy_scale(entity: &modkit_core::Entity) -> f32 {
    entity
        .get("physdamagescale")
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v >= 0.)
        .unwrap_or(1.)
}

/// ReadDamageTable: the entry before the first threshold above `energy`.
fn read_table(table: &[(f32, f32)], energy: f32) -> f32 {
    let i = table
        .iter()
        .take_while(|(threshold, _)| energy >= *threshold)
        .count();
    if i > 0 {
        table[i - 1].1
    } else {
        0.
    }
}

/// The colliding object as the NPC's shadow sees it.
#[derive(Clone, Copy, Debug)]
pub struct Impactor {
    pub mass: f32,
    pub pre: Vec3,
    pub post: Vec3,
}

/// CalculatePhysicsImpactDamage (linear) for an NPC shadow of `npc_mass` (it does not
/// lose speed itself) hit by `other`; allowStaticDamage false.
pub fn npc_damage(npc_mass: f32, energy_scale: f32, other: Impactor) -> f32 {
    let speed_sqr = other.pre.length_squared();
    if other.mass < MIN_MASS
        || (other.mass < SMALL_MASS_MAX && speed_sqr < SMALL_MASS_MIN_SPEED_SQR)
        || speed_sqr < MIN_SPEED_SQR
        || npc_mass <= 0.
    {
        return 0.;
    }
    let delta = other.pre.length() - other.post.length();
    let mut energy = delta * delta * other.mass;
    if other.mass >= LARGE_MASS_MIN {
        energy *= LARGE_MASS_SCALE;
        let dz = other.pre.z - other.post.z;
        if delta > 0. && dz < 0. && other.pre.z < 0. {
            let factor = (dz / delta).abs();
            energy *= 1. + factor * (LARGE_MASS_FALLING_SCALE - 1.);
        }
    }
    energy *= energy_scale / npc_mass;
    let damage = read_table(&NPC_LINEAR, energy);
    // Small dynamic objects never do more than the cap.
    if other.mass < SMALL_MASS_MAX {
        damage.clamp(0., SMALL_MASS_CAP)
    } else {
        damage
    }
}

/// Queue DMG_CRUSH damage for NPCs struck by moving physics objects (resolved collision
/// events of the previous step; applied
/// with the other entity damage next tick, like the deferred damage queue). Objects the
/// player holds do no damage (bDamageFromHeldObjects false); NPCs ignore what they
/// stand on (not tracked here).
pub fn queue_npc_damage(
    collisions: &[PhysicsCollision],
    world: &World,
    scene: &mut Scene,
    held: &dyn Fn(usize) -> bool,
) {
    for event in collisions {
        for index in 0..2 {
            let other = 1 - index;
            let Some(npc) = event.entities[index] else {
                continue;
            };
            let Some(entity) = world.entities.get(npc) else {
                continue;
            };
            if !entity.class().starts_with("npc_")
                || scene.states.get(npc).is_none_or(|s| s.killed)
                || event.masses[other] <= 0.
                || event.entities[other].is_some_and(|id| id == npc || held(id))
            {
                continue;
            }
            let Some(mass) = world
                .model_instances
                .iter()
                .find(|i| i.entity == Some(npc))
                .and_then(|i| world.model_mass.get(&i.asset_key()))
            else {
                continue;
            };
            let damage = npc_damage(
                *mass,
                energy_scale(entity),
                Impactor {
                    mass: event.masses[other],
                    pre: event.pre_velocities[other],
                    post: event.post_velocities[other],
                },
            );
            if damage > 0. {
                scene.entity_damage.push(Damage {
                    target: DamageTarget::Entity(npc),
                    amount: damage,
                    dissolve: false,
                    direction: event.pre_velocities[other].normalize_or_zero(),
                    origin: event.origins[other],
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hit(mass: f32, speed: f32, after: f32) -> Impactor {
        Impactor {
            mass,
            pre: Vec3::X * speed,
            post: Vec3::X * after,
        }
    }
    #[test]
    fn launched_barrel_kills_and_light_junk_is_capped() {
        // 30 kg at the gravity gun's 1500 u/s stopping on a 90 kg cop: 1500^2 x 30 / 90
        // = 750000 -> the 500^2 entry, 100 damage.
        assert_eq!(npc_damage(90., 1., hit(30., 1500., 0.)), 100.);
        // A 7 kg paint can: 175000 -> 50.
        assert_eq!(npc_damage(90., 1., hit(7., 1500., 0.)), 50.);
        // A 1 kg bottle is under the 2 kg minimum; a 3 kg object caps at 5.
        assert_eq!(npc_damage(90., 1., hit(1., 1500., 0.)), 0.);
        assert_eq!(npc_damage(90., 1., hit(3., 1500., 0.)), 5.);
        // Slow or glancing (no speed lost) contacts do nothing.
        assert_eq!(npc_damage(90., 1., hit(30., 20., 0.)), 0.);
        assert_eq!(npc_damage(90., 1., hit(30., 800., 800.)), 0.);
        // Large masses read 4x the energy.
        assert_eq!(npc_damage(90., 1., hit(600., 120., 0.)), 100.);
        // physdamagescale 0 (the trainstation_02 plaza cops) takes no impact damage.
        assert_eq!(npc_damage(90., 0., hit(30., 1500., 0.)), 0.);
        let cop = modkit_core::Entity {
            properties: vec![("physdamagescale".into(), "0".into())],
        };
        assert_eq!(energy_scale(&cop), 0.);
        assert_eq!(
            energy_scale(&modkit_core::Entity { properties: vec![] }),
            1.
        );
    }
}
