//! env_physexplosion (SDK physobj.cpp CPhysExplosion::Explode): an instant physics
//! push on VPhysics entities, by target name (else class name within the radius) or
//! every entity in the radius.
//!
//! Each target's push is CalculateExplosiveDamageForce: damage = magnitude - 0.4 x
//! distance (at least 1), force = min(damage x ImpulseScale(75, 4), ImpulseScale(75,
//! 400)) x RandomFloat(0.85, 1.15) kg in/s along (target center - explosion origin),
//! applied at the explosion origin (VPhysicsTakeDamage -> ApplyForceOffset). Without
//! SF_PHYSEXPLOSION_NODAMAGE the target also takes DMG_BLAST damage. The d1_trainstation_02
//! playground uses it for the tic-tac-toe blocks (OnPlayerUse, magnitude 18) and the
//! swings (trigger_once, magnitude 0.2/0.25), all NODAMAGE.
//!
//! Not modeled: SF_PHYSEXPLOSION_PUSH_PLAYER / DISORIENT_PLAYER (counted in
//! `unsupported`), the target center is the body's center of mass rather than the
//! collision bounds' center (BodyTarget -> WorldSpaceCenter).
use crate::{
    entities::Scene,
    physics::Physics,
    projectiles::{Damage, DamageTarget},
};
use glam::Vec3;
use modkit_core::World;

const SF_NODAMAGE: u32 = 0x1;
const SF_PUSH_PLAYER: u32 = 0x2;
const SF_RADIAL: u32 = 0x4;
const SF_TEST_LOS: u32 = 0x8;

#[derive(Default, serde::Serialize)]
pub struct PhysExplosions {
    pub explosions: u64,
    pub pushes: u64,
    pub unsupported: u64,
    #[serde(skip)]
    rng: u64,
}
impl PhysExplosions {
    fn random(&mut self, min: f32, max: f32) -> f32 {
        if self.rng == 0 {
            self.rng = 0x2545_f491_4f6c_dd1d;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        min + (self.rng >> 40) as f32 / (1u64 << 24) as f32 * (max - min)
    }
    /// Explodes every env_physexplosion whose Explode input arrived this tick.
    pub fn apply(&mut self, world: &World, scene: &mut Scene, physics: &mut Physics) {
        for id in std::mem::take(&mut scene.phys_explosions) {
            let Some(e) = world.entities.get(id) else {
                continue;
            };
            self.explosions += 1;
            let number = |key: &str| {
                e.get(key)
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.)
            };
            let magnitude = number("magnitude");
            let radius = number("radius");
            let inner_radius = number("inner_radius");
            let flags = number("spawnflags") as u32;
            if flags & SF_PUSH_PLAYER != 0 {
                self.unsupported += 1;
            }
            let origin = e.origin();
            for target in targets(world, physics, e, origin, radius) {
                let Some(body) = physics.entity_body(target) else {
                    continue;
                };
                let Some(spot) = physics.center_of_mass(body) else {
                    continue;
                };
                let mut from = origin;
                if flags & SF_RADIAL != 0 {
                    from.z = spot.z;
                }
                let distance = from.distance(spot);
                if radius != 0. && distance > radius {
                    continue;
                }
                if flags & SF_TEST_LOS != 0 {
                    let start = if inner_radius != 0. {
                        origin + (spot - origin).normalize_or_zero() * inner_radius
                    } else {
                        origin
                    };
                    if physics
                        .projectile_ray(start, spot, &[target])
                        .is_some_and(|hit| hit.entity != target)
                    {
                        continue;
                    }
                }
                let damage = (magnitude - distance / 2.5).max(1.);
                let force = (damage * 75. * 4.).min(75. * 400.) * self.random(0.85, 1.15);
                let direction = (spot - from).normalize_or_zero();
                physics.impulse_at(target, direction * (force / 39.37), from);
                self.pushes += 1;
                if flags & SF_NODAMAGE == 0 {
                    scene.entity_damage.push(Damage {
                        target: DamageTarget::Entity(target),
                        amount: damage,
                        kind: crate::player_damage::DMG_BLAST,
                        dissolve: false,
                        direction,
                        origin: from,
                    });
                }
            }
        }
    }
}

/// CPhysExplosion::FindEntity: every entity named targetentityname (else of that class
/// within the radius), or with no target name every entity within the radius.
fn targets(
    world: &World,
    physics: &Physics,
    explosion: &modkit_core::Entity,
    origin: Vec3,
    radius: f32,
) -> Vec<usize> {
    let within = |id: usize| {
        physics
            .entity_pose(id)
            .is_some_and(|(position, _)| position.distance(origin) <= radius)
    };
    match explosion
        .get("targetentityname")
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(name) => {
            let named: Vec<usize> = world
                .entities
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.get("targetname")
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                })
                .map(|(id, _)| id)
                .collect();
            if !named.is_empty() {
                return named;
            }
            world
                .entities
                .iter()
                .enumerate()
                .filter(|(id, e)| e.class().eq_ignore_ascii_case(name) && within(*id))
                .map(|(id, _)| id)
                .collect()
        }
        None => (0..world.entities.len()).filter(|id| within(*id)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modkit_core::{Entity, PhysicsSolid};

    #[test]
    fn named_target_gets_the_falloff_impulse_from_the_explosion_origin() {
        // Synthetic: a NODAMAGE env_physexplosion (magnitude 18) 12 units from a 2 kg
        // body named "block"; a second explosion with damage and no name.
        let mut world = World::default();
        let entity = |pairs: &[(&str, &str)]| Entity {
            properties: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };
        world.entities.push(entity(&[
            ("classname", "env_physexplosion"),
            ("origin", "-12 0 0"),
            ("magnitude", "18"),
            ("spawnflags", "1"),
            ("targetentityname", "block"),
        ]));
        world.entities.push(entity(&[
            ("classname", "prop_physics"),
            ("targetname", "block"),
        ]));
        world.entities.push(entity(&[
            ("classname", "env_physexplosion"),
            ("origin", "0 0 -20"),
            ("magnitude", "10"),
            ("radius", "64"),
        ]));
        world.model_physics.insert(
            "models/block.mdl#0".into(),
            PhysicsSolid {
                mass: 2.,
                ..Default::default()
            },
        );
        let mut physics = Physics::new(&world);
        let body = physics.add_prop_body(
            &world,
            "models/block.mdl#0",
            Vec3::splat(4.),
            Vec3::ZERO,
            glam::Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        physics.bind_projectile_entity(body, 1);
        let mut scene = Scene::new(&world);
        let mut explosions = PhysExplosions::default();
        scene.phys_explosions.push(0);
        explosions.apply(&world, &mut scene, &mut physics);
        // (18 - 12 / 2.5) x 300 x [0.85, 1.15] kg in/s on 2 kg, along +x.
        let v = physics.body_state(body).unwrap().velocity;
        let expected = 13.2 * 300. / 2.;
        assert!(v.y.abs() < 1e-3 && v.z.abs() < 1e-3, "{v}");
        assert!(v.x > expected * 0.84 && v.x < expected * 1.16, "{v}");
        assert!(scene.entity_damage.is_empty());
        // Without a name: everything with a body in the radius, plus DMG_BLAST damage.
        scene.phys_explosions.push(2);
        explosions.apply(&world, &mut scene, &mut physics);
        assert_eq!(scene.entity_damage.len(), 1);
        assert_eq!(scene.entity_damage[0].kind, crate::player_damage::DMG_BLAST);
        assert_eq!(scene.entity_damage[0].amount, 10. - 20. / 2.5);
        assert_eq!((explosions.explosions, explosions.pushes), (2, 2));
    }
}
