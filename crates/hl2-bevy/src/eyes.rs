//! Owned iris projection with authored scene interests and a visible-interest fallback.
use crate::{
    FlyCamera,
    assets::LoadedMap,
    bevy_to_source,
    rendering::{EyeMesh, SourceEntity},
};
use bevy::{mesh::VertexAttributeValues, prelude::*};
use std::collections::BTreeMap;
struct Actor {
    key: String,
    gaze: source_assets::eyes::Gaze,
    scale: f32,
}
#[derive(Resource, Default)]
pub struct Eyes {
    actors: BTreeMap<usize, Actor>,
    pose_samples: usize,
    attachment_actors: usize,
    origins: Vec<serde_json::Value>,
    draws: usize,
    tracking_player: usize,
    tracking_npc: usize,
    tracking_entity: usize,
    tracking_scene: usize,
    projections: Vec<serde_json::Value>,
    /// Bone-local eye forward/up per (entity, eyeball surface) for FACS eyelids; flexes
    /// consume the previous presentation's basis.
    pub lid_bases: BTreeMap<(usize, usize), (glam::Vec3, glam::Vec3)>,
}
impl Eyes {
    pub fn new(loaded: &LoadedMap) -> Self {
        let mut result = Self::default();
        for i in &loaded.world.model_instances {
            let Some(id) = i.entity else { continue };
            if !loaded.world.entities[id].class().starts_with("npc_") {
                continue;
            }
            if let Some(gaze) = loaded.gaze.get(&i.asset_key()) {
                result.actors.insert(
                    id,
                    Actor {
                        key: i.asset_key(),
                        gaze: gaze.clone(),
                        scale: i.scale,
                    },
                );
            }
        }
        result
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"draws":self.draws,"pose_samples":self.pose_samples,"attachment_actors":self.attachment_actors,"origins":self.origins,"tracking_player":self.tracking_player,"tracking_npc":self.tracking_npc,"tracking_entity":self.tracking_entity,"tracking_scene":self.tracking_scene,"projections":self.projections,
            "policy":"newest valid authored LOOKAT interest, shared per actor; nearest-visible fallback outside scripted interests; animated owned eyes attachments supply actor origin/forward; native random/head/facial/PVS latch behavior remains unfinished",
            "shader":"owned Eyes iris alpha over sclera with retail planar basis; glints and Source model lighting remain unfinished; FACS eyelids follow the previous frame's eye basis"})
    }
}
fn bone_pose(
    game: &crate::gameplay::Gameplay,
    key: &str,
    id: usize,
    bone: usize,
    poses: &BTreeMap<usize, Vec<glam::Mat4>>,
) -> Option<glam::Mat4> {
    let rig = game.world.rigs.get(key)?;
    Some(*poses.get(&id)?.get(bone)? * rig.bones.get(bone)?.inverse_bind.inverse())
}
pub fn present(
    mut eyes: ResMut<Eyes>,
    game: Res<crate::gameplay::Gameplay>,
    sim: Res<crate::movement::Simulation>,
    camera: Query<&Transform, With<FlyCamera>>,
    draws: Query<(
        &SourceEntity,
        &EyeMesh,
        &Mesh3d,
        Option<&bevy::mesh::skinning::SkinnedMesh>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    performance: Option<Res<crate::performance::Performance>>,
) {
    let _timing = crate::performance::scope(performance.as_deref(), "eyes");
    let Ok(camera) = camera.single() else { return };
    // Evaluate each visible actor once per presentation; target and iris consumers
    // share these skin transforms rather than sampling the full rig for every eye.
    let mut pose_keys: BTreeMap<usize, &str> = eyes
        .actors
        .iter()
        .map(|(&id, a)| (id, a.key.as_str()))
        .collect();
    for (owner, eye, _, _) in &draws {
        pose_keys.entry(owner.0).or_insert(&eye.key);
    }
    let poses: BTreeMap<_, _> = pose_keys
        .into_iter()
        .filter_map(|(id, key)| {
            let state = &game.scene.states[id];
            if state.killed || !state.visible {
                return None;
            }
            let rig = game.world.rigs.get(key)?;
            Some((id, game.scene.actor_matrices(rig, id)))
        })
        .collect();
    let mut candidates = vec![(
        None,
        glam::Vec3::from_array(bevy_to_source(camera.translation).to_array()),
        glam::Vec3::ZERO,
    )];
    let mut origins = Vec::new();
    let mut attachment_actors = 0;
    for (&id, actor) in &eyes.actors {
        let state = &game.scene.states[id];
        if !state.visible || state.killed {
            continue;
        }
        let attached = actor.gaze.attachment.as_ref().and_then(|attachment| {
            bone_pose(&game, &actor.key, id, attachment.bone, &poses)
                .map(|bone| (bone * attachment.local, attachment.world_align))
        });
        let (origin, forward) = if let Some((attachment, world_align)) = attached {
            attachment_actors += 1;
            (
                state.origin
                    + state.rotation
                        * (attachment.transform_point3(glam::Vec3::ZERO) * actor.scale),
                if world_align {
                    glam::Vec3::X
                } else {
                    state.rotation * attachment.transform_vector3(glam::Vec3::X).normalize()
                },
            )
        } else {
            // Base NPC eye offset is already in world axes; PVS-dependent latching
            // and derived retail eye-position overrides are not modeled here.
            (
                state.origin + actor.gaze.view_offset * actor.scale,
                state.rotation * glam::Vec3::X,
            )
        };
        candidates.push((Some(id), origin, forward));
        origins.push(serde_json::json!({"entity":id,"origin":origin.to_array(),"forward":forward.to_array(),"attachment":attached.is_some()}));
    }
    eyes.pose_samples = poses.len();
    eyes.attachment_actors = attachment_actors;
    eyes.origins = origins;
    eyes.projections.clear();
    eyes.lid_bases.clear();
    eyes.draws = 0;
    eyes.tracking_player = 0;
    eyes.tracking_npc = 0;
    eyes.tracking_entity = 0;
    eyes.tracking_scene = 0;
    #[derive(Clone)]
    struct Selected {
        point: glam::Vec3,
        label: String,
        interest: Option<hl2_simulation::attention::Interest>,
    }
    let mut actor_targets = BTreeMap::<usize, Option<Selected>>::new();
    for (owner, mesh_eye, handle, skinned) in &draws {
        let state = &game.scene.states[owner.0];
        if !state.visible || state.killed {
            continue;
        }
        let Some(bone) = bone_pose(&game, &mesh_eye.key, owner.0, mesh_eye.eye.bone, &poses) else {
            continue;
        };
        let origin = bone.transform_point3(mesh_eye.eye.origin);
        let head_forward = -bone.transform_vector3(mesh_eye.eye.forward).normalize();
        let world_origin = state.origin + state.rotation * (origin * mesh_eye.scale);
        // SetViewtarget is per actor; both eyes consume the same world target.
        let selected = actor_targets.entry(owner.0).or_insert_with(|| {
            use hl2_simulation::attention::Target;
            let actor_eye = candidates.iter().find(|(id, _, _)| *id == Some(owner.0));
            let (actor_eye, forward) = actor_eye.map_or(
                (world_origin, state.rotation * head_forward),
                |(_, p, f)| (*p, *f),
            );
            let position = |target| match target {
                Target::Player => Some(candidates[0].1),
                Target::Entity(id) => game.scene.states.get(id).filter(|s| !s.killed).map(|s| {
                    candidates
                        .iter()
                        .find(|(candidate, _, _)| *candidate == Some(id))
                        .map_or(s.origin, |(_, p, _)| *p)
                }),
            };
            if let Some((interest, point)) = game
                .scene
                .look_targets
                .eye_target(owner.0, actor_eye, forward, position)
            {
                let label = match interest.target {
                    Target::Player => "player".into(),
                    Target::Entity(id) => format!("#{id}"),
                };
                return Some(Selected {
                    point,
                    label,
                    interest: Some(interest.clone()),
                });
            }
            candidates
                .iter()
                .filter_map(|&(id, p, _)| {
                    if id == Some(owner.0) {
                        return None;
                    }
                    let distance = p.distance(actor_eye);
                    if !(1. ..=1024.).contains(&distance)
                        || (p - actor_eye).normalize().dot(forward) <= 0.259
                    {
                        return None;
                    }
                    let excluded = [owner.0, id.unwrap_or(usize::MAX)];
                    if sim
                        .physics
                        .projectile_ray(actor_eye, p, &excluded)
                        .is_some()
                    {
                        return None;
                    }
                    Some((id, p, distance))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .map(|(id, point, _)| Selected {
                    point,
                    label: id.map_or("player".into(), |i| format!("#{i}")),
                    interest: None,
                })
        });
        let local_target = selected
            .as_ref()
            .map(|s| state.rotation.inverse() * (s.point - state.origin) / mesh_eye.scale);
        let projection = source_assets::eyes::projection(&mesh_eye.eye, bone, local_target);
        eyes.lid_bases.insert(
            (owner.0, mesh_eye.eye.surface),
            source_assets::eyes::local_basis(
                bone,
                source_assets::eyes::basis(&mesh_eye.eye, bone, local_target),
            ),
        );
        if let Some(selected) = selected {
            if selected.interest.is_some() {
                eyes.tracking_scene += 1;
            }
            if selected.label == "player" {
                eyes.tracking_player += 1;
            } else {
                eyes.tracking_entity += 1;
                let id = selected
                    .label
                    .strip_prefix('#')
                    .and_then(|s| s.parse::<usize>().ok());
                if id.is_some_and(|id| game.world.entities[id].class().starts_with("npc_")) {
                    eyes.tracking_npc += 1;
                }
            }
        }
        eyes.projections.push(serde_json::json!({"entity":owner.0,"surface":mesh_eye.eye.surface,"rows":projection.iter().map(|r|r.to_array()).collect::<Vec<_>>(),
            "target":selected.as_ref().map(|s|&s.label),"world_target":selected.as_ref().map(|s|s.point.to_array()),"scripted_interest":selected.as_ref().and_then(|s|s.interest.as_ref())}));
        if let Some(mesh) = meshes.get(&handle.0) {
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let uvs = positions
                .iter()
                .enumerate()
                .map(|(vertex, p)| {
                    let point =
                        glam::Vec3::from_array(bevy_to_source(Vec3::from_array(*p)).to_array())
                            / mesh_eye.scale;
                    let point = if skinned.is_some() {
                        crate::gpu_skinning::skin_vertex(mesh, vertex, point, &poses[&owner.0])
                    } else {
                        point
                    };
                    [
                        projection[0].dot(point.extend(1.)),
                        projection[1].dot(point.extend(1.)),
                    ]
                })
                .collect::<Vec<_>>();
            if !matches!(mesh.attribute(Mesh::ATTRIBUTE_UV_1),Some(VertexAttributeValues::Float32x2(old)) if *old == uvs)
                && let Some(mut mesh) = meshes.get_mut(&handle.0)
            {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs);
            }
            eyes.draws += 1;
        }
    }
}
