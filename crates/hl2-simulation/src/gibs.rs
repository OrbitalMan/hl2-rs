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
//!
//! Gibs take the parent's skin (BreakModelCreateSingle) and the break block's collision
//! group (a fade time makes debris). A breakable prop without .phy gibs breaks into its
//! prop_data chunk set instead (PropBreakableCreateAll's else-branch: `breakable_count`
//! random chunk models).
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
    /// The parent's m_nSkin.
    pub skin: i32,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Gib {
    pub id: u64,
    /// Gib model path (render meshes and physics under "{model}#0").
    pub model: String,
    /// m_nSkin (the parent's, or the chunk set's breakable_skin).
    pub skin: i32,
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
    /// The host's mesh key: the model path, with "#skin" for other skins.
    pub fn render_key(&self) -> String {
        render_key(&self.model, self.skin)
    }
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
    rng: u64,
}
/// The host's mesh key for a gib model drawn with `skin`.
pub fn render_key(model: &str, skin: i32) -> String {
    if skin == 0 {
        model.to_string()
    } else {
        format!("{model}#{skin}")
    }
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
            // CBreakableProp::Break: params.defCollisionGroup is the prop's own group.
            let parent_debris = world
                .entities
                .get(parent.entity)
                .and_then(|e| e.get("spawnflags"))
                .and_then(|v| v.trim().parse::<u32>().ok())
                .is_some_and(|flags| flags & 4 != 0);
            let Some(list) = world.model_breaks.get(&parent.model_key) else {
                self.spawn_chunks(&parent, world, physics, time);
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
                if gib.debris.unwrap_or(parent_debris) {
                    physics.set_body_debris(body);
                }
                // nActualSkin: the parent's skin (past the gib's skin families the
                // studio renderer falls back to skin 0).
                self.push(
                    body,
                    &gib.model,
                    parent.skin,
                    parent.entity,
                    (position, parent.rotation),
                    center,
                    time,
                    gib.fade_time,
                    physics,
                );
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        body: RigidBodyHandle,
        model: &str,
        skin: i32,
        parent: usize,
        (position, rotation): (Vec3, Quat),
        center: Vec3,
        time: f64,
        fade_time: f32,
        physics: &mut Physics,
    ) {
        let id = GIB_ID_BASE + self.next_id;
        self.next_id += 1;
        physics.bind_projectile_entity(body, crate::physics::projectile_entity(id));
        let fade_at = if fade_time > 0. {
            time + f64::from(fade_time)
        } else {
            f64::INFINITY
        };
        self.active.push(Gib {
            id,
            model: model.to_string(),
            skin,
            parent,
            body,
            position,
            rotation,
            alpha: 255,
            fade_at,
            next_think: fade_at,
            center,
        });
        self.spawned += 1;
    }
    /// RandomFloat(min, max) from a fixed-seed xorshift stream.
    fn random(&mut self, min: f32, max: f32) -> f32 {
        if self.rng == 0 {
            self.rng = 0x9e37_79b9_7f4a_7c15;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        min + (self.rng >> 40) as f32 / (1u64 << 24) as f32 * (max - min)
    }
    /// PropBreakableCreateAll without .phy gibs: `breakable_count` chunks of the
    /// prop_data chunk set, each a random model no larger than the prop allows, at a
    /// random point on the mid plane across the prop's smallest axis, moving at the
    /// default burst (100 u/s) away from the prop origin without inheriting its motion,
    /// turned to the prop's angles with its longest axes aligned (AlignBoxes), debris,
    /// fading after 5-10 s.
    fn spawn_chunks(
        &mut self,
        parent: &PropBreak,
        world: &World,
        physics: &mut Physics,
        time: f64,
    ) {
        let Some(data) = world.model_prop_data.get(&parent.model_key) else {
            return;
        };
        let Some(name) = data.breakable_model.as_deref() else {
            return;
        };
        if data.breakable_count <= 0 {
            return;
        }
        // GetRandomChunkModel: the first set whose name starts with the requested one.
        let Some((_, set)) = world
            .breakable_chunks
            .iter()
            .find(|(set, _)| set.starts_with(name))
        else {
            return;
        };
        let Some((mins, maxs)) = local_bounds(world, &parent.model_key) else {
            return;
        };
        let size = maxs - mins;
        let smallest = smallest_axis(size);
        // CPropData::ParsePropFromKV: drop the smallest axis, floor(area / 32^2).
        let mut flat = size;
        flat[smallest] = 1.;
        let max_size = (flat.x * flat.y * flat.z / (32. * 32.)).floor() as usize;
        for _ in 0..data.breakable_count {
            if set.is_empty() {
                break;
            }
            let last = max_size.min(set.len() - 1);
            // RandomInt(0, last).
            let index = ((self.random(0., 1.) * (last + 1) as f32) as usize).min(last);
            let model = &set[index];
            let key = format!("{model}#0");
            if !world.model_physics.contains_key(&key) {
                if !self.skipped.contains(model) {
                    self.skipped.push(model.clone());
                }
                continue;
            }
            let mut normalized = Vec3::new(
                self.random(0., 1.),
                self.random(0., 1.),
                self.random(0., 1.),
            );
            normalized[smallest] = 0.5;
            let position = parent.origin + parent.rotation * (mins + normalized * size);
            let velocity = (position - parent.origin).normalize_or_zero() * DEFAULT_BURST;
            let chunk_size = local_bounds(world, &key).map_or(Vec3::ONE, |(a, b)| b - a);
            let rotation = align_boxes(parent.rotation, size, chunk_size);
            let body = physics.add_prop_body(
                world,
                &key,
                Vec3::splat(2.),
                position,
                rotation,
                velocity,
                Vec3::ZERO,
            );
            physics.set_body_debris(body);
            let fade_time = self.random(5., 10.);
            self.push(
                body,
                model,
                data.breakable_skin,
                parent.entity,
                (position, rotation),
                local_center(world, &key),
                time,
                fade_time,
                physics,
            );
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

/// The model-space bounds of a model's collision pieces (its OBB).
fn local_bounds(world: &World, key: &str) -> Option<(Vec3, Vec3)> {
    let mut points = world
        .model_collision
        .get(key)
        .into_iter()
        .flatten()
        .flat_map(|piece| piece.vertices.iter().copied());
    let first = points.next()?;
    Some(points.fold((first, first), |(a, b), p| (a.min(p), b.max(p))))
}
/// The center of a gib model's collision pieces (local; zero without pieces).
fn local_center(world: &World, key: &str) -> Vec3 {
    local_bounds(world, key).map_or(Vec3::ZERO, |(min, max)| (min + max) * 0.5)
}
/// props_shared.cpp GreatestAxis / SmallestAxis (ties as in the SDK).
fn greatest_axis(v: Vec3) -> usize {
    if v.x > v.y {
        if v.x > v.z {
            0
        } else {
            2
        }
    } else if v.y > v.z {
        1
    } else {
        2
    }
}
fn smallest_axis(v: Vec3) -> usize {
    if v.x < v.y {
        if v.x < v.z {
            0
        } else {
            2
        }
    } else if v.y < v.z {
        1
    } else {
        2
    }
}
/// AlignBoxes: up to two 90 degree turns (MatrixRot90 on the rotation's columns) that
/// put the second box's greatest axes along the first's.
fn align_boxes(rotation: Quat, extents1: Vec3, extents2: Vec3) -> Quat {
    let (mut ext1, mut ext2) = (extents1, extents2);
    let mut rotations = Vec::new();
    let (axis0, axis1) = (greatest_axis(ext1), greatest_axis(ext2));
    if axis0 != axis1 {
        rotations.push((axis0, axis1));
        ext2[axis1] = ext2[axis0];
    }
    ext1[axis0] = 0.;
    ext2[axis0] = 0.;
    let (axis0, axis1) = (greatest_axis(ext1), greatest_axis(ext2));
    if axis0 != axis1 {
        rotations.push((axis0, axis1));
    }
    let mut matrix = glam::Mat3::from_quat(rotation);
    while let Some((axis0, axis1)) = rotations.pop() {
        let (col0, col1) = (matrix.col(axis0), matrix.col(axis1));
        *matrix.col_mut(axis0) = col1;
        *matrix.col_mut(axis1) = -col0;
    }
    Quat::from_mat3(&matrix).normalize()
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
            skin: 0,
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

    #[test]
    fn gibs_keep_the_parent_skin_and_their_collision_group() {
        let mut world = world();
        // A fade time without an earlier "debris" key makes the piece debris.
        if let Some(list) = world.model_breaks.get_mut("models/box.mdl#0") {
            list[0].debris = Some(true);
            list[1].debris = Some(false);
        }
        let mut physics = Physics::new(&world);
        let mut gibs = Gibs::default();
        gibs.spawn(
            vec![PropBreak {
                skin: 2,
                ..parent(Quat::IDENTITY)
            }],
            &world,
            &mut physics,
            0.,
        );
        assert_eq!(gibs.active[0].skin, 2);
        assert_eq!(gibs.active[0].render_key(), "models/box_gib1.mdl#2");
        assert_eq!(render_key("models/box_gib1.mdl", 0), "models/box_gib1.mdl");
    }
    #[test]
    fn props_without_phy_gibs_break_into_their_chunk_set() {
        use modkit_core::{ConvexPiece, PropData};
        let mut world = World::default();
        let cube = |half: Vec3| ConvexPiece {
            vertices: [-1., 1.]
                .iter()
                .flat_map(|x| [-1., 1.].map(move |y| (*x, y)))
                .flat_map(|(x, y)| [-1., 1.].map(move |z| Vec3::new(x, y, z) * half))
                .collect(),
            indices: Vec::new(),
        };
        // A flat 96 x 48 x 8 board (synthetic): max size floor(96 x 48 / 1024) = 4.
        world.model_collision.insert(
            "models/board.mdl#0".into(),
            vec![cube(Vec3::new(48., 24., 4.))],
        );
        world.model_prop_data.insert(
            "models/board.mdl#0".into(),
            PropData {
                health: 20.,
                breakable_model: Some("WoodChunks".into()),
                breakable_count: 3,
                breakable_skin: 1,
                ..Default::default()
            },
        );
        let chunks: Vec<String> = (0..6).map(|i| format!("models/chunk{i}.mdl")).collect();
        world.breakable_chunks = vec![("WoodChunks".into(), chunks.clone())];
        for (i, model) in chunks.iter().enumerate() {
            let key = format!("{model}#0");
            world
                .model_physics
                .insert(key.clone(), PhysicsSolid::default());
            // Chunks are long along their own y axis.
            world
                .model_collision
                .insert(key, vec![cube(Vec3::new(1., 2. + i as f32, 1.))]);
        }
        let mut physics = Physics::new(&world);
        let mut gibs = Gibs::default();
        let mut board = parent(Quat::IDENTITY);
        board.model_key = "models/board.mdl#0".into();
        board.velocity = Vec3::new(0., 0., -300.);
        gibs.spawn(vec![board], &world, &mut physics, 1.);
        assert_eq!(gibs.active.len(), 3);
        for gib in &gibs.active {
            // Never larger than chunk 4; the breakable_skin; fade after 5-10 s.
            assert!(chunks[..5].contains(&gib.model), "{}", gib.model);
            assert_eq!(gib.skin, 1);
            assert!((6. ..=11.).contains(&gib.fade_at), "{}", gib.fade_at);
            // On the mid plane across the smallest (z) axis, inside the board.
            assert!((gib.position.z - 100.).abs() < 1e-3);
            assert!(gib.position.x.abs() <= 48. && gib.position.y.abs() <= 24.);
            // Burst away from the origin, the parent's fall not inherited.
            let state = physics.body_state(gib.body).unwrap();
            assert!((state.velocity.length() - 100.).abs() < 1e-2);
            assert!(state.velocity.z.abs() < 1e-3);
            // AlignBoxes: the chunk's long y axis turned onto the board's long x axis.
            assert!((gib.rotation * Vec3::Y).x.abs() > 0.999);
        }
    }
    #[test]
    fn align_boxes_turns_the_longest_axes_together() {
        // Already aligned: unchanged.
        let q = align_boxes(
            Quat::IDENTITY,
            Vec3::new(10., 5., 1.),
            Vec3::new(4., 2., 1.),
        );
        assert!(q.angle_between(Quat::IDENTITY) < 1e-5);
        // Second box longest along z, then y: both turned onto x, then y.
        let q = align_boxes(
            Quat::IDENTITY,
            Vec3::new(10., 5., 1.),
            Vec3::new(1., 2., 4.),
        );
        assert!((q * Vec3::Z).x.abs() > 0.999);
        assert!((q * Vec3::Y).y.abs() > 0.999);
    }
}
