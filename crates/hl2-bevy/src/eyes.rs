//! Owned iris projection with a bounded visible-interest fallback for gaze.
use crate::{
    FlyCamera,
    assets::LoadedMap,
    bevy_to_source,
    rendering::{EyeMesh, SourceEntity},
};
use bevy::{mesh::VertexAttributeValues, prelude::*};
use std::collections::BTreeMap;
#[derive(Resource, Default)]
pub struct Eyes {
    actors: BTreeMap<usize, (String, source_assets::eyes::Eyeball, f32)>,
    draws: usize,
    tracking_player: usize,
    tracking_npc: usize,
    projections: Vec<serde_json::Value>,
}
impl Eyes {
    pub fn new(loaded: &LoadedMap) -> Self {
        let mut result = Self::default();
        for i in &loaded.world.model_instances {
            let Some(id) = i.entity else { continue };
            if !loaded.world.entities[id].class().starts_with("npc_") {
                continue;
            }
            if let Some((_, eye)) = loaded
                .eyes
                .iter()
                .find(|((key, _), _)| key == &i.asset_key())
            {
                result
                    .actors
                    .insert(id, (i.asset_key(), eye.clone(), i.scale));
            }
        }
        result
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"draws":self.draws,"tracking_player":self.tracking_player,"tracking_npc":self.tracking_npc,"projections":self.projections,
            "policy":"nearest visible player/NPC within 1024 units and 75-degree eye cone; interest queues and VCD LOOKAT/head/facial blending remain unfinished",
            "shader":"owned Eyes iris alpha over sclera with retail planar basis; glints, eyelid flexes and Source model lighting remain unfinished"})
    }
}
fn bone_pose(
    game: &crate::gameplay::Gameplay,
    key: &str,
    id: usize,
    bone: usize,
) -> Option<glam::Mat4> {
    let rig = game.world.rigs.get(key)?;
    let state = &game.scene.states[id];
    let matrices = rig.matrices(&state.animation, game.scene.animation_time(id));
    Some(*matrices.get(bone)? * rig.bones.get(bone)?.inverse_bind.inverse())
}
pub fn present(
    mut eyes: ResMut<Eyes>,
    game: Res<crate::gameplay::Gameplay>,
    sim: Res<crate::movement::Simulation>,
    camera: Query<&Transform, With<FlyCamera>>,
    draws: Query<(&SourceEntity, &EyeMesh, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok(camera) = camera.single() else { return };
    let mut candidates = vec![(
        None,
        glam::Vec3::from_array(bevy_to_source(camera.translation).to_array()),
    )];
    for (&id, (key, eye, scale)) in &eyes.actors {
        let state = &game.scene.states[id];
        if state.visible
            && !state.killed
            && let Some(bone) = bone_pose(&game, key, id, eye.bone)
        {
            candidates.push((
                Some(id),
                state.origin + state.rotation * (bone.transform_point3(eye.origin) * *scale),
            ));
        }
    }
    eyes.projections.clear();
    eyes.draws = 0;
    eyes.tracking_player = 0;
    eyes.tracking_npc = 0;
    for (owner, mesh_eye, handle) in &draws {
        let state = &game.scene.states[owner.0];
        if !state.visible || state.killed {
            continue;
        }
        let Some(bone) = bone_pose(&game, &mesh_eye.key, owner.0, mesh_eye.eye.bone) else {
            continue;
        };
        let origin = bone.transform_point3(mesh_eye.eye.origin);
        let head_forward = -bone.transform_vector3(mesh_eye.eye.forward).normalize();
        let world_origin = state.origin + state.rotation * (origin * mesh_eye.scale);
        let target = candidates
            .iter()
            .filter_map(|&(id, p)| {
                if id == Some(owner.0) {
                    return None;
                }
                let local = state.rotation.inverse() * (p - state.origin) / mesh_eye.scale;
                let distance = p.distance(world_origin);
                if !(1. ..=1024.).contains(&distance)
                    || (local - origin).normalize().dot(head_forward) <= 0.259
                {
                    return None;
                }
                let excluded = [owner.0, id.unwrap_or(usize::MAX)];
                if sim
                    .physics
                    .projectile_ray(world_origin, p, &excluded)
                    .is_some()
                {
                    return None;
                }
                Some((id, local, distance))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let projection =
            source_assets::eyes::projection(&mesh_eye.eye, bone, target.map(|(_, p, _)| p));
        if let Some((id, _, _)) = target {
            if id.is_some() {
                eyes.tracking_npc += 1;
            } else {
                eyes.tracking_player += 1;
            }
        }
        eyes.projections.push(serde_json::json!({"entity":owner.0,"surface":mesh_eye.eye.surface,"rows":projection.iter().map(|r|r.to_array()).collect::<Vec<_>>(),
            "target":target.map(|(id,_,_)|id.map_or("player".to_string(),|i|format!("#{i}")))}));
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let uvs = positions
                .iter()
                .map(|p| {
                    let point =
                        glam::Vec3::from_array(bevy_to_source(Vec3::from_array(*p)).to_array())
                            / mesh_eye.scale;
                    [
                        projection[0].dot(point.extend(1.)),
                        projection[1].dot(point.extend(1.)),
                    ]
                })
                .collect::<Vec<_>>();
            if !matches!(mesh.attribute(Mesh::ATTRIBUTE_UV_1),Some(VertexAttributeValues::Float32x2(old)) if *old == uvs)
            {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs);
            }
            eyes.draws += 1;
        }
    }
}
