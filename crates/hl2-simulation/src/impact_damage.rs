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
            // An AddOutput "physdamagescale <v>" at runtime overrides the map keyvalue.
            let scale = scene
                .keyvalue_overrides
                .get(&(npc, "physdamagescale".to_string()))
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v >= 0.)
                .unwrap_or_else(|| energy_scale(entity));
            let damage = npc_damage(
                *mass,
                scale,
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

/// CBreakableProp m_impactEnergyScale: keyvalue physdamagescale, 0 or absent -> 0.1
/// (CBreakableProp::Spawn).
pub fn prop_energy_scale(entity: &modkit_core::Entity, overridden: Option<&str>) -> f32 {
    let scale = overridden
        .or(entity.get("physdamagescale"))
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v >= 0.)
        .unwrap_or(0.);
    if scale == 0. {
        0.1
    } else {
        scale
    }
}

/// CalculatePhysicsImpactDamage (linear) for a physics prop of `mass` (allowStaticDamage
/// true, as CPhysicsProp::VPhysicsCollision passes): the speed both bodies lost, the
/// other weighted by its mass, per kilogram of this prop. A static or unmoving other
/// contributes only this prop's own loss.
pub fn prop_damage(mass: f32, energy_scale: f32, own: Impactor, other: Impactor) -> f32 {
    if mass <= 0. || energy_scale <= 0. {
        return 0.;
    }
    // myMinVelocity is 0 in the NPC table: no filtering of this prop's own loss.
    let delta = own.pre.length() - own.post.length();
    let mut energy = delta * delta * mass;
    let delta = other.pre.length() - other.post.length();
    let mut other_energy = delta * delta * other.mass;
    if other.mass >= LARGE_MASS_MIN {
        other_energy *= LARGE_MASS_SCALE;
        let dz = other.pre.z - other.post.z;
        if delta > 0. && dz < 0. && other.pre.z < 0. {
            let factor = (dz / delta).abs();
            other_energy *= 1. + factor * (LARGE_MASS_FALLING_SCALE - 1.);
        }
    }
    energy += other_energy;
    energy *= energy_scale / mass;
    let damage = read_table(&NPC_LINEAR, energy);
    if other.mass > 0. && other.mass < SMALL_MASS_MAX {
        damage.clamp(0., SMALL_MASS_CAP)
    } else {
        damage
    }
}

/// CPhysicsProp::VPhysicsCollision physics damage for prop_physics (breakables take it
/// through the entity damage path; unbreakable props ignore it there): the NPC table
/// (no prop names another table here yet), x10 on the first collision after a gravity
/// gun launch (`launched`, cleared by any collision). Objects the player holds take
/// none from the player and no static damage; NPCs and props hit by them are handled
/// by their own side of the event.
pub fn queue_prop_damage(
    collisions: &[PhysicsCollision],
    world: &World,
    scene: &mut Scene,
    launched: &mut std::collections::HashSet<usize>,
    held: &dyn Fn(usize) -> bool,
) {
    for event in collisions {
        for index in 0..2 {
            let other = 1 - index;
            let Some(id) = event.entities[index] else {
                continue;
            };
            let Some(entity) = world.entities.get(id) else {
                continue;
            };
            if !entity.class().starts_with("prop_physics")
                || scene.states.get(id).is_none_or(|s| s.killed)
                || event.entities[other] == Some(id)
            {
                continue;
            }
            let first_after_launch = launched.remove(&id);
            // Held objects: no static damage (other side static) and none from the player.
            if held(id) && event.masses[other] <= 0. {
                continue;
            }
            let overridden = scene
                .keyvalue_overrides
                .get(&(id, "physdamagescale".to_string()))
                .map(String::as_str);
            let mut damage = prop_damage(
                event.masses[index],
                prop_energy_scale(entity, overridden),
                Impactor {
                    mass: event.masses[index],
                    pre: event.pre_velocities[index],
                    post: event.post_velocities[index],
                },
                Impactor {
                    mass: event.masses[other],
                    pre: event.pre_velocities[other],
                    post: event.post_velocities[other],
                },
            );
            if first_after_launch {
                damage *= 10.;
            }
            if damage > 0. {
                scene.entity_damage.push(Damage {
                    target: DamageTarget::Entity(id),
                    amount: damage,
                    dissolve: false,
                    direction: event.pre_velocities[index].normalize_or_zero(),
                    origin: event.origins[index],
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
    #[test]
    fn launched_crate_breaks_on_the_floor_only_with_the_launch_bonus() {
        let crate_ = modkit_core::Entity {
            properties: vec![("physdamagescale".into(), "0".into())],
        };
        // physdamagescale 0 on a prop means the 0.1 default.
        assert_eq!(prop_energy_scale(&crate_, None), 0.1);
        assert_eq!(prop_energy_scale(&crate_, Some("1")), 1.);
        let floor = Impactor {
            mass: 0.,
            pre: Vec3::ZERO,
            post: Vec3::ZERO,
        };
        // A 20 kg crate stopping from 500 u/s on the world: 500^2 x 0.1 = 25000 -> 5,
        // x10 after a gravity gun launch: 50 (wood_crate001a breaks at 30 health).
        let stop = hit(20., 500., 0.);
        assert_eq!(prop_damage(20., 0.1, stop, floor), 5.);
        // A gentle 100 u/s landing does nothing.
        assert_eq!(prop_damage(20., 0.1, hit(20., 100., 0.), floor), 0.);
        // A 1 kg bottle thrown at the crate is capped at 5.
        assert_eq!(
            prop_damage(20., 1., hit(20., 0., 0.), hit(1., 1500., 0.)),
            5.
        );
    }

    #[test]
    fn prop_launched_into_a_wall_queues_launch_damage() {
        use rapier3d::prelude::*;
        const S: f32 = 1. / 39.37;
        // Synthetic scene (not game data): a wall at x 0, a 20 kg prop entity 100 units
        // out flying at it at 1500 u/s, as if launched by the gravity gun.
        let world = World {
            entities: vec![modkit_core::Entity {
                properties: vec![("classname".into(), "prop_physics".into())],
            }],
            ..Default::default()
        };
        let mut scene = Scene::new(&world);
        let mut physics = crate::physics::Physics::new(&world);
        physics.colliders.insert(
            ColliderBuilder::cuboid(8. * S, 200. * S, 200. * S).translation(vector![
                -8. * S,
                0.,
                0.
            ]),
        );
        let body = physics.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector![100. * S, 0., 0.])
                .linvel(vector![-1500. * S, 0., 0.])
                .gravity_scale(0.)
                .ccd_enabled(true),
        );
        physics.colliders.insert_with_parent(
            ColliderBuilder::cuboid(16. * S, 16. * S, 4. * S)
                .mass(20.)
                .user_data(1),
            body,
            &mut physics.bodies,
        );
        physics.dynamic.insert(0, body);
        physics.launched.insert(0);
        let mut total = 0.;
        for _ in 0..20 {
            physics.tick(0.015);
            queue_prop_damage(
                &physics.resolved_collisions,
                &world,
                &mut scene,
                &mut physics.launched,
                &|_| false,
            );
            total += scene.entity_damage.drain(..).map(|d| d.amount).sum::<f32>();
        }
        assert!(physics.launched.is_empty());
        assert!(total >= 50., "launch damage {total}");
    }
}
