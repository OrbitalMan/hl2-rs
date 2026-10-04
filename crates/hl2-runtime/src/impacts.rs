use crate::{
    entities::Scene,
    physics::{Physics, RayHit},
    rendering::Materials,
};
use glam::Vec3 as GVec3;
use macroquad::prelude::*;
use modkit_core::{decals, Surface, World};
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::{BTreeMap, HashMap};
struct Mark {
    mesh: Mesh,
    entity: usize,
    scale: f32,
}
pub struct Impacts {
    marks: Vec<Mark>,
    textures: HashMap<String, Texture2D>,
    properties: BTreeMap<String, keyvalues::Entry>,
    pub created: usize,
    pub unclippable: usize,
}
impl Impacts {
    pub fn new(vfs: &Vfs) -> Self {
        let mut properties = BTreeMap::new();
        if let Ok(Some(manifest)) = vfs.read("scripts/surfaceproperties_manifest.txt") {
            if let Ok(entries) = keyvalues::parse(&String::from_utf8_lossy(&manifest)) {
                for file in entries
                    .iter()
                    .flat_map(|e| e.children())
                    .filter(|e| e.key.eq_ignore_ascii_case("file"))
                {
                    if let Some(file) = file.text() {
                        if let Ok(Some(data)) = vfs.read(file) {
                            if let Ok(entries) = keyvalues::parse(&String::from_utf8_lossy(&data)) {
                                for entry in entries {
                                    properties.insert(entry.key.to_lowercase(), entry);
                                }
                            }
                        }
                    }
                }
            }
        }
        Self {
            marks: Vec::new(),
            textures: HashMap::new(),
            properties,
            created: 0,
            unclippable: 0,
        }
    }
    fn property(&self, name: &str, key: &str) -> Option<String> {
        let mut current = name.to_lowercase();
        for _ in 0..32 {
            let e = self.properties.get(&current)?;
            if let Some(value) = e.get(key).and_then(|e| e.text()) {
                return Some(value.into());
            }
            current = e
                .get("base")
                .and_then(|e| e.text())
                .unwrap_or("default")
                .to_lowercase();
            if current == e.key.to_lowercase() {
                break;
            }
        }
        None
    }
    pub fn add(
        &mut self,
        hit: RayHit,
        melee: bool,
        world: &World,
        physics: &Physics,
        scene: &mut Scene,
        vfs: &Vfs,
    ) {
        let (surfaces, origin, rotation, scale): (&[Surface], _, _, _) =
            if let Some(e) = world.entities.get(hit.entity) {
                let state = &scene.states[hit.entity];
                if state.killed {
                    return;
                }
                let (origin, rotation) = physics
                    .entity_pose(hit.entity)
                    .unwrap_or((state.origin, state.rotation));
                if let Some(model) = e
                    .get("model")
                    .and_then(|s| s.strip_prefix('*'))
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|id| world.brush_models.iter().find(|m| m.id == id))
                {
                    (&model.surfaces[..], origin, rotation, 1.)
                } else if let Some(instance) = world
                    .model_instances
                    .iter()
                    .find(|i| i.entity == Some(hit.entity))
                {
                    let Some(surfaces) = world.model_assets.get(&instance.asset_key()) else {
                        return;
                    };
                    (&surfaces[..], origin, rotation, instance.scale)
                } else {
                    return;
                }
            } else {
                (&world.surfaces[..], GVec3::ZERO, glam::Quat::IDENTITY, 1.)
            };
        let center = rotation.inverse() * (hit.position - origin) / scale;
        let normal = rotation.inverse() * hit.normal;
        let receiver = surfaces.iter().find(|s| {
            !decals::project(std::slice::from_ref(s), center, normal, 0.15, 0.).is_empty()
        });
        let Some(receiver) = receiver else {
            self.unclippable += 1;
            return;
        };
        let prop = vfs
            .material_value(&receiver.material, "$surfaceprop")
            .ok()
            .flatten()
            .unwrap_or("default".into());
        if !melee {
            if let Some(sound) = self.property(&prop, "bulletimpact") {
                scene.sounds.push(sound.into());
            }
        }
        let family = match self
            .property(&prop, "gamematerial")
            .as_deref()
            .unwrap_or("C")
        {
            "M" | "V" | "G" => "metal",
            "W" => "wood",
            "Y" => "glass",
            "N" => "sand",
            _ => "concrete",
        };
        let material = format!("decals/{family}/shot{}", self.created % 5 + 1);
        if !self.textures.contains_key(&material) {
            let Ok(texture) = crate::viewer::texture(vfs, &material) else {
                return;
            };
            unsafe {
                get_internal_gl().quad_context.texture_set_wrap(
                    texture.raw_miniquad_id(),
                    miniquad::TextureWrap::Clamp,
                    miniquad::TextureWrap::Clamp,
                );
            }
            self.textures.insert(material.clone(), texture);
        }
        let texture = self.textures[&material].clone();
        let factor = vfs
            .material_value(&material, "$decalscale")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.1);
        let vertices = decals::project(
            surfaces,
            center,
            normal,
            texture.width() * factor * 0.5 / scale,
            (self.created as f32 * 2.399963).rem_euclid(std::f32::consts::TAU),
        );
        if vertices.is_empty() {
            self.unclippable += 1;
            return;
        }
        let mesh = Mesh {
            indices: (0..vertices.len()).map(|i| i as u16).collect(),
            vertices: vertices
                .into_iter()
                .map(|v| {
                    Vertex::new(
                        v.position.x,
                        v.position.y,
                        v.position.z,
                        v.uv.x,
                        v.uv.y,
                        WHITE,
                    )
                })
                .collect(),
            texture: Some(texture),
        };
        if self.marks.len() >= 256 {
            self.marks.remove(0);
        }
        self.marks.push(Mark {
            mesh,
            entity: hit.entity,
            scale,
        });
        self.created += 1;
    }
    pub fn draw(&self, scene: &Scene, physics: &Physics, materials: &Materials) {
        materials.select(None, 4, None, Vec2::ZERO, vec4(1., 1., 1., 1.));
        for mark in &self.marks {
            let transform = if let Some(state) = scene.states.get(mark.entity) {
                if state.killed || !state.visible {
                    continue;
                }
                let (origin, rotation) = physics
                    .entity_pose(mark.entity)
                    .unwrap_or((state.origin, state.rotation));
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(mark.scale),
                    rotation,
                    vec3(origin.x, origin.y, origin.z),
                )
            } else {
                Mat4::IDENTITY
            };
            unsafe {
                get_internal_gl().quad_gl.push_model_matrix(transform);
            }
            draw_mesh(&mark.mesh);
            unsafe {
                get_internal_gl().quad_gl.pop_model_matrix();
            }
        }
    }
}
