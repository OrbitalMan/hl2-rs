//! Client-side NPC death ragdolls (C_BaseAnimating::BecomeRagdollOnClient ->
//! CreateClientRagdoll -> ragdoll_shared.cpp RagdollCreate): the model's .phy ragdoll as
//! jointed physics bodies placed at the death pose, nudged by the death force, then
//! driving the skinned mesh. Bones without a solid keep their death-pose transform
//! relative to their parent. DESIGN 14n session 30 (11) steps (b)-(c).
use crate::entities::Scene;
use crate::physics::Physics;
use glam::{Mat4, Quat, Vec3};
use modkit_core::{RagdollModel, World};
use rapier3d::prelude::RigidBodyHandle;
use std::collections::BTreeMap;
use std::sync::Arc;

/// g_ragdoll_maxcount (client): older ragdolls are removed beyond this count.
pub const MAX_RAGDOLLS: usize = 8;

pub struct Ragdoll {
    model: Arc<RagdollModel>,
    bodies: Vec<RigidBodyHandle>,
    /// Every studio bone's parent and death-pose transform relative to its parent (model
    /// space for root bones).
    parents: Vec<Option<usize>>,
    locals: Vec<Mat4>,
    inverse_binds: Vec<Mat4>,
    scale: f32,
    serial: u64,
}

#[derive(Default)]
pub struct Ragdolls {
    pub active: BTreeMap<usize, Ragdoll>,
    serial: u64,
    pub spawned: u32,
    /// Deaths without a ragdoll (no .phy ragdoll, rig or instance).
    pub skipped: u32,
    pub removed: u32,
    /// Every NPC's origin at the previous tick (its root velocity at death).
    last_origins: BTreeMap<usize, Vec3>,
}

impl Ragdolls {
    /// RagdollApplyAnimationAsVelocity approximated by root motion: each NPC's origin
    /// change over the last tick, for the ragdolls of NPCs that die on the next one.
    pub fn track(&mut self, world: &World, scene: &Scene) {
        self.last_origins.clear();
        for (id, entity) in world.entities.iter().enumerate() {
            if entity.class().starts_with("npc_") {
                if let Some(state) = scene.states.get(id).filter(|s| !s.killed) {
                    self.last_origins.insert(id, state.origin);
                }
            }
        }
    }
    /// RagdollCreate for an NPC killed this tick: bodies at their bones' current world
    /// matrices, the death force (scene.npc_death_forces) on the solid nearest the hit.
    pub fn spawn(
        &mut self,
        world: &World,
        scene: &mut Scene,
        physics: &mut Physics,
        id: usize,
        dt: f32,
    ) {
        let Some(instance) = world.model_instances.iter().find(|i| i.entity == Some(id)) else {
            self.skipped += 1;
            return;
        };
        let key = instance.asset_key();
        let (Some(rig), Some(model)) = (world.rigs.get(&key), world.model_ragdolls.get(&key))
        else {
            self.skipped += 1;
            return;
        };
        // actor_matrices are skinning matrices (pose x inverse bind): the bones' model
        // space poses are those times the bind pose.
        let skin = scene.actor_matrices(rig, id);
        if skin.len() != rig.bones.len() {
            self.skipped += 1;
            return;
        }
        let matrices: Vec<Mat4> = skin
            .iter()
            .zip(&rig.bones)
            .map(|(m, b)| *m * b.inverse_bind.inverse())
            .collect();
        let state = &scene.states[id];
        let entity = Mat4::from_scale_rotation_translation(
            Vec3::splat(instance.scale),
            state.rotation,
            state.origin,
        );
        let poses: Vec<(Vec3, Quat)> = model
            .bodies
            .iter()
            .map(|b| {
                let (_, rotation, translation) =
                    (entity * matrices[b.bone]).to_scale_rotation_translation();
                (translation, rotation.normalize())
            })
            .collect();
        // Root velocity over the last tick (a teleport or respawn is not motion).
        let velocity = self
            .last_origins
            .get(&id)
            .map(|last| (state.origin - *last) / dt.max(1e-3))
            .filter(|v| v.length() < 1000.)
            .unwrap_or(Vec3::ZERO);
        self.serial += 1;
        let bodies = physics.spawn_ragdoll(world, model, &poses, velocity, self.serial);
        if bodies.len() != model.bodies.len() {
            physics.remove_ragdoll(&bodies);
            self.skipped += 1;
            return;
        }
        if let Some((force, position)) = scene.npc_death_forces.remove(&id) {
            let nearest = poses
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    a.1 .0
                        .distance_squared(position)
                        .total_cmp(&b.1 .0.distance_squared(position))
                })
                .map_or(0, |(i, _)| i);
            physics.ragdoll_nudge(&bodies, nearest, force);
        }
        let parents: Vec<_> = rig.bones.iter().map(|b| b.parent).collect();
        let locals = parents
            .iter()
            .enumerate()
            .map(|(i, parent)| match parent {
                Some(p) => matrices[*p].inverse() * matrices[i],
                None => matrices[i],
            })
            .collect();
        scene.ragdoll_matrices.insert(id, skin);
        self.active.insert(
            id,
            Ragdoll {
                model: model.clone(),
                bodies,
                parents,
                locals,
                inverse_binds: rig.bones.iter().map(|b| b.inverse_bind).collect(),
                scale: instance.scale,
                serial: self.serial,
            },
        );
        self.spawned += 1;
        while self.active.len() > MAX_RAGDOLLS {
            let Some(oldest) = self
                .active
                .iter()
                .min_by_key(|(_, r)| r.serial)
                .map(|(id, _)| *id)
            else {
                break;
            };
            self.remove(scene, physics, oldest);
        }
    }
    pub fn remove(&mut self, scene: &mut Scene, physics: &mut Physics, id: usize) {
        if let Some(r) = self.active.remove(&id) {
            physics.remove_ragdoll(&r.bodies);
            scene.ragdoll_matrices.remove(&id);
            self.removed += 1;
        }
    }
    /// After the physics step: each ragdoll's bone matrices in its entity's model space,
    /// with the entity origin following the root solid (render placement and bounds).
    /// A ragdoll whose entity came back (respawn, Kill removal) is dropped.
    pub fn update(&mut self, scene: &mut Scene, physics: &mut Physics) {
        let gone: Vec<usize> = self
            .active
            .keys()
            .copied()
            .filter(|id| scene.states.get(*id).is_none_or(|s| !s.killed))
            .collect();
        for id in gone {
            self.remove(scene, physics, id);
        }
        for (&id, ragdoll) in &self.active {
            let Some(poses) = physics.ragdoll_poses(&ragdoll.bodies) else {
                continue;
            };
            let scale = Vec3::splat(ragdoll.scale);
            let mut simulated: Vec<Option<Mat4>> = vec![None; ragdoll.parents.len()];
            for (body, (position, rotation)) in ragdoll.model.bodies.iter().zip(&poses) {
                if let Some(slot) = simulated.get_mut(body.bone) {
                    *slot = Some(Mat4::from_scale_rotation_translation(
                        scale, *rotation, *position,
                    ));
                }
            }
            let state = &mut scene.states[id];
            state.origin = poses[0].0;
            let entity = Mat4::from_scale_rotation_translation(scale, state.rotation, state.origin);
            // Studio bones list parents first.
            let mut world_bones: Vec<Mat4> = Vec::with_capacity(ragdoll.parents.len());
            for (i, parent) in ragdoll.parents.iter().enumerate() {
                let bone = match (simulated[i], parent) {
                    (Some(m), _) => m,
                    (None, Some(p)) if *p < i => world_bones[*p] * ragdoll.locals[i],
                    _ => entity * ragdoll.locals[i],
                };
                world_bones.push(bone);
            }
            let to_model = entity.inverse();
            scene.ragdoll_matrices.insert(
                id,
                world_bones
                    .iter()
                    .zip(&ragdoll.inverse_binds)
                    .map(|(m, inverse_bind)| to_model * *m * *inverse_bind)
                    .collect(),
            );
        }
        scene.ragdoll_frame = scene.ragdoll_frame.wrapping_add(1);
    }
}
