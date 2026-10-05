use crate::{
    entities::Scene,
    physics::{Physics, RayHit},
    rendering::Materials,
};
use macroquad::prelude::*;
use modkit_core::World;
use source_assets::vpk::Vfs;
use std::collections::HashMap;
pub struct Impacts {
    source: hl2_simulation::impacts::Impacts,
    textures: HashMap<String, Texture2D>,
    meshes: HashMap<usize, Mesh>,
}
impl std::ops::Deref for Impacts {
    type Target = hl2_simulation::impacts::Impacts;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}
impl Impacts {
    pub fn new(vfs: &Vfs, world: &World) -> Self {
        let source = hl2_simulation::impacts::Impacts::new(vfs, world);
        let textures = source
            .textures
            .iter()
            .map(|(name, t)| {
                let texture = Texture2D::from_rgba8(t.image.width, t.image.height, &t.image.rgba);
                texture.set_filter(FilterMode::Linear);
                unsafe {
                    get_internal_gl().quad_context.texture_set_wrap(
                        texture.raw_miniquad_id(),
                        miniquad::TextureWrap::Clamp,
                        miniquad::TextureWrap::Clamp,
                    );
                }
                (name.clone(), texture)
            })
            .collect();
        Self {
            source,
            textures,
            meshes: HashMap::new(),
        }
    }
    pub fn add(
        &mut self,
        hit: RayHit,
        melee: bool,
        world: &World,
        physics: &Physics,
        scene: &mut Scene,
        _vfs: &Vfs,
    ) {
        self.source.add(hit, melee, world, physics, scene);
    }
    pub fn draw(&mut self, scene: &Scene, physics: &Physics, materials: &Materials) {
        self.meshes
            .retain(|id, _| self.source.marks.iter().any(|m| m.id == *id));
        materials.select(None, 4, None, Vec2::ZERO, Vec4::ONE);
        for mark in &self.source.marks {
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
                    Vec3::from_array(origin.to_array()),
                )
            } else {
                Mat4::IDENTITY
            };
            let mesh = self.meshes.entry(mark.id).or_insert_with(|| Mesh {
                indices: (0..mark.vertices.len()).map(|i| i as u16).collect(),
                vertices: mark
                    .vertices
                    .iter()
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
                texture: Some(self.textures[&mark.material].clone()),
            });
            unsafe {
                get_internal_gl().quad_gl.push_model_matrix(transform);
            }
            draw_mesh(mesh);
            unsafe {
                get_internal_gl().quad_gl.pop_model_matrix();
            }
        }
    }
}
