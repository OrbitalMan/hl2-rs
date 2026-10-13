//! Gibs of broken props: CBreakableProp::Break -> PropBreakableCreateAll
//! (props_shared.cpp) -> BreakModelCreateSingle / BreakModelCreate_Prop (props.cpp), and
//! the gibs' SUB_StartFadeOut / SUB_FadeOut (baseentity.cpp).
//!
//! Each non-ragdoll `break` block of the broken prop's .phy becomes a runtime prop body
//! at the parent's pose (offset in the parent's model space), with the parent's velocity
//! at that point, its angular velocity and an outward burst (CBreakableProp::Break's
//! defBurstScale 100 unless the block sets "burst"). After `fadetime` the gib fades by
//! int(256 * tick) alpha per tick while it is outside the player's 2D view cone and not
//! held, and is removed at alpha 0.
use crate::physics::Physics;
use glam::{Quat, Vec3};
use modkit_core::World;
use rapier3d::prelude::RigidBodyHandle;

/// CBreakableProp::Break: params.defBurstScale.
pub const DEFAULT_BURST: f32 = 100.;
/// Gib ids start here so their entity ids (`projectile_entity`) never meet a
/// projectile's.
pub const GIB_ID_BASE: u64 = 1 << 32;
/// CBasePlayer::m_flFieldOfView (cosine) used by FInViewCone.
const PLAYER_FIELD_OF_VIEW: f32 = 0.766;
/// CBreakableProp::m_PerformanceMode PM_NO_GIBS.
const PM_NO_GIBS: i32 = 1;

/// A prop that broke this tick: its pose and motion before it was removed.
#[derive(Clone, Debug)]
pub struct PropBreak {
    pub entity: usize,
    /// The broken prop's model asset key (`World::model_breaks`).
    pub model_key: String,
    pub origin: Vec3,
    pub rotation: Quat,
    /// Units/s.
    pub velocity: Vec3,
    /// World-space radians/s.
    pub angular_velocity: Vec3,
    pub center_of_mass: Vec3,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Gib {
    pub id: u64,
    /// Gib model path (render meshes and physics under "{model}#0").
    pub model: String,
    pub parent: usize,
    #[serde(skip)]
    pub body: RigidBodyHandle,
    pub position: Vec3,
    pub rotation: Quat,
    /// m_clrRender alpha (255 opaque; kRenderTransTexture once fading starts).
    pub alpha: u8,
    /// When SUB_FadeOut first runs (spawn + fadetime); infinite without a fade time.
    pub fade_at: f64,
    #[serde(skip)]
    next_think: f64,
    /// Local center of the gib's collision bounds (WorldSpaceCenter).
    #[serde(skip)]
    center: Vec3,
}
impl Gib {
    pub fn entity(&self) -> usize {
        crate::physics::projectile_entity(self.id)
    }
    fn world_center(&self) -> Vec3 {
        self.position + self.rotation * self.center
    }
}

#[derive(Default)]
pub struct Gibs {
    pub active: Vec<Gib>,
    next_id: u64,
    pub spawned: usize,
    pub removed: usize,
    /// Gib models not created (ragdoll gibs need ragdolls; models without physics).
    pub skipped: Vec<String>,
}
impl Gibs {
    /// PropBreakableCreateAll for every prop broken this tick.
    pub fn spawn(
        &mut self,
        breaks: Vec<PropBreak>,
        world: &World,
        physics: &mut Physics,
        time: f64,
    ) {
        for parent in breaks {
            let performance = world
                .entities
                .get(parent.entity)
                .and_then(|e| e.get("PerformanceMode"))
                .and_then(|v| v.trim().parse::<i32>().ok());
            if performance == Some(PM_NO_GIBS) {
                continue;
            }
            let Some(list) = world.model_breaks.get(&parent.model_key) else {
                continue;
            };
            for gib in list {
                let key = format!("{}#0", gib.model);
                if gib.ragdoll || !world.model_physics.contains_key(&key) {
                    if !self.skipped.contains(&gib.model) {
                        self.skipped.push(gib.model.clone());
                    }
                    continue;
                }
                let center = local_center(world, &key);
                // VectorTransform(offset - placementOrigin, parent matrix); placement
                // attachments are not read (none on the HL2 props checked so far).
                let position = parent.origin + parent.rotation * gib.offset;
                let mut velocity = parent.velocity
                    + parent
                        .angular_velocity
                        .cross(position - parent.center_of_mass);
                let burst = gib.burst.unwrap_or(DEFAULT_BURST);
                if burst != 0. {
                    // From the parent origin; with no offset, toward the piece's center.
                    let mut direction = position - parent.origin;
                    if direction == Vec3::ZERO {
                        direction = position + parent.rotation * center - parent.origin;
                    }
                    velocity += direction.normalize_or_zero() * burst;
                }
                let local_angular_degrees = parent.rotation.inverse()
                    * (parent.angular_velocity * 180. / std::f32::consts::PI);
                let body = physics.add_prop_body(
                    world,
                    &key,
                    Vec3::splat(2.),
                    position,
                    parent.rotation,
                    velocity,
                    local_angular_degrees,
                );
                let id = GIB_ID_BASE + self.next_id;
                self.next_id += 1;
                physics.bind_projectile_entity(body, crate::physics::projectile_entity(id));
                let fade_at = if gib.fade_time > 0. {
                    time + f64::from(gib.fade_time)
                } else {
                    f64::INFINITY
                };
                self.active.push(Gib {
                    id,
                    model: gib.model.clone(),
                    parent: parent.entity,
                    body,
                    position,
                    rotation: parent.rotation,
                    alpha: 255,
                    fade_at,
                    next_think: fade_at,
                    center,
                });
                self.spawned += 1;
            }
        }
    }
    /// Pose read-back and SUB_FadeOut once per tick. `forward` is the player's view
    /// direction; `held` says whether the player holds an entity (FVPHYSICS_PLAYER_HELD).
    pub fn tick(
        &mut self,
        physics: &mut Physics,
        time: f64,
        dt: f32,
        eye: Vec3,
        forward: Vec3,
        held: impl Fn(usize) -> bool,
    ) {
        let facing = Vec3::new(forward.x, forward.y, 0.).normalize_or_zero();
        let mut removed = Vec::new();
        for gib in &mut self.active {
            if let Some(state) = physics.body_state(gib.body) {
                gib.position = state.position;
                gib.rotation = state.rotation;
            }
            if time < gib.next_think {
                continue;
            }
            let los = gib.world_center() - eye;
            let in_view =
                Vec3::new(los.x, los.y, 0.).normalize_or_zero().dot(facing) > PLAYER_FIELD_OF_VIEW;
            if held(gib.entity()) || in_view {
                gib.next_think = time + 1.;
                gib.alpha = 255;
                continue;
            }
            // SUB_PerformFadeOut: speed = max(1, int(256 * min(frametime, 0.1))).
            let speed = ((256. * dt.min(0.1)) as i32).max(1);
            gib.alpha = (i32::from(gib.alpha) - speed).max(0) as u8;
            if gib.alpha == 0 {
                removed.push(gib.id);
            } else {
                gib.next_think = time;
            }
        }
        for id in removed {
            if let Some(index) = self.active.iter().position(|g| g.id == id) {
                let gib = self.active.remove(index);
                physics.remove_body(gib.body);
                self.removed += 1;
            }
        }
    }
}

/// The center of a gib model's collision pieces (local; zero without pieces).
fn local_center(world: &World, key: &str) -> Vec3 {
    let mut points = world
        .model_collision
        .get(key)
        .into_iter()
        .flatten()
        .flat_map(|piece| piece.vertices.iter().copied());
    let Some(first) = points.next() else {
        return Vec3::ZERO;
    };
    let (min, max) = points.fold((first, first), |(a, b), p| (a.min(p), b.max(p)));
    (min + max) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use modkit_core::{BreakModel, PhysicsSolid};

    /// A breakable "box.mdl" with two gibs (synthetic values, not game data).
    fn world() -> World {
        let mut world = World::default();
        let gib = |model: &str, offset: Vec3, fade_time: f32| BreakModel {
            model: model.into(),
            ragdoll: false,
            offset,
            health: 1.,
            fade_time,
            debris: None,
            motion_disabled: false,
            burst: None,
        };
        world.model_breaks.insert(
            "models/box.mdl#0".into(),
            vec![
                gib("models/box_gib1.mdl", Vec3::new(10., 0., 0.), 2.),
                gib("models/box_gib2.mdl", Vec3::ZERO, 0.),
                BreakModel {
                    ragdoll: true,
                    ..gib("models/box_ragdoll.mdl", Vec3::ZERO, 2.)
                },
            ],
        );
        for model in ["models/box_gib1.mdl", "models/box_gib2.mdl"] {
            world
                .model_physics
                .insert(format!("{model}#0"), PhysicsSolid::default());
        }
        world
    }
    fn parent(rotation: Quat) -> PropBreak {
        PropBreak {
            entity: 3,
            model_key: "models/box.mdl#0".into(),
            origin: Vec3::new(0., 0., 100.),
            rotation,
            velocity: Vec3::new(50., 0., 0.),
            angular_velocity: Vec3::ZERO,
            center_of_mass: Vec3::new(0., 0., 100.),
        }
    }
    #[test]
    fn gibs_spawn_at_the_parent_pose_with_its_velocity_and_burst() {
        let world = world();
        let mut physics = Physics::new(&world);
        let mut gibs = Gibs::default();
        let turned = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        gibs.spawn(vec![parent(turned)], &world, &mut physics, 0.);
        // Ragdoll gibs need ragdolls: skipped and reported.
        assert_eq!(gibs.active.len(), 2);
        assert_eq!(gibs.skipped, vec!["models/box_ragdoll.mdl".to_string()]);
        // The offset is in the parent's model space: +x turned to +y.
        let first = physics.body_state(gibs.active[0].body).unwrap();
        assert!(first.position.distance(Vec3::new(0., 10., 100.)) < 1e-3);
        // Parent velocity plus the default 100 u/s burst away from the parent origin.
        assert!(first.velocity.distance(Vec3::new(50., 100., 0.)) < 1e-3);
        // Without an offset or collision pieces there is no burst direction.
        let second = physics.body_state(gibs.active[1].body).unwrap();
        assert!(second.velocity.distance(Vec3::new(50., 0., 0.)) < 1e-3);
        // Gibs are prop entities: +USE / the gravity gun find them by entity id.
        assert_eq!(
            physics.entity_body(gibs.active[0].entity()),
            Some(gibs.active[0].body)
        );
    }
    #[test]
    fn gibs_fade_only_out_of_view_and_unheld_then_disappear() {
        let world = world();
        let mut physics = Physics::new(&world);
        let mut gibs = Gibs::default();
        gibs.spawn(vec![parent(Quat::IDENTITY)], &world, &mut physics, 0.);
        let eye = Vec3::new(-200., 0., 164.);
        let dt = 0.015;
        let mut time = 0.;
        let mut tick = |gibs: &mut Gibs, physics: &mut Physics, forward: Vec3, held: bool| {
            time += f64::from(dt);
            gibs.tick(physics, time, dt, eye, forward, |_| held);
        };
        // Before its 2 s fade time nothing happens.
        for _ in 0..100 {
            tick(&mut gibs, &mut physics, Vec3::NEG_X, false);
        }
        assert_eq!(gibs.active[0].alpha, 255);
        // Looking at it (2D view cone) or holding it postpones the fade.
        for _ in 0..60 {
            tick(&mut gibs, &mut physics, Vec3::X, false);
        }
        assert_eq!(gibs.active[0].alpha, 255);
        for _ in 0..70 {
            tick(&mut gibs, &mut physics, Vec3::NEG_X, true);
        }
        assert_eq!(gibs.active[0].alpha, 255);
        // Looking away: 3 alpha per 15 ms tick (int(256 * 0.015)), gone after 85 ticks.
        // The second gib has no fade time and stays.
        let mut ticks = 0;
        while gibs.active.len() == 2 {
            tick(&mut gibs, &mut physics, Vec3::NEG_X, false);
            ticks += 1;
            assert!(ticks < 200);
        }
        assert!((85..=86 + 67).contains(&ticks), "{ticks}");
        assert_eq!(gibs.removed, 1);
        assert_eq!(gibs.active[0].model, "models/box_gib2.mdl");
    }
}
