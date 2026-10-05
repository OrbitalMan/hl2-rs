//! Retained GPU adapter for shared owned projectile sprite commands.
use crate::{projectiles::Projectiles, rendering::Materials};
use macroquad::prelude::*;
use source_assets::vpk::Vfs;
use std::collections::HashMap;
pub struct ProjectileVisuals {
    source: hl2_simulation::projectile_visuals::ProjectileVisuals,
    textures: HashMap<String, Texture2D>,
}
impl std::ops::Deref for ProjectileVisuals {
    type Target = hl2_simulation::projectile_visuals::ProjectileVisuals;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}
impl ProjectileVisuals {
    pub fn new(vfs: &Vfs) -> Self {
        let source = hl2_simulation::projectile_visuals::ProjectileVisuals::new(vfs);
        let textures = source
            .sprites
            .iter()
            .map(|(name, s)| {
                let texture = Texture2D::from_rgba8(s.image.width, s.image.height, &s.image.rgba);
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
        Self { source, textures }
    }
    pub fn draw(
        &mut self,
        projectiles: &Projectiles,
        direction: Vec3,
        time: f64,
        paused: bool,
        materials: &Materials,
    ) {
        self.source.frame(
            projectiles,
            glam::Vec3::from_array(direction.to_array()),
            time,
            paused,
        );
        for quad in &self.source.quads {
            let sprite = &self.source.sprites[&quad.material];
            let uv = [vec2(0., 1.), vec2(0., 0.), vec2(1., 0.), vec2(1., 1.)];
            let vertices = quad
                .positions
                .iter()
                .zip(uv)
                .map(|(p, uv)| Vertex {
                    position: Vec3::from_array(p.to_array()),
                    uv,
                    color: quad.color,
                    normal: Vec4::ZERO,
                })
                .collect();
            materials.select(None, sprite.kind, None, Vec2::ZERO, Vec4::ONE);
            draw_mesh(&Mesh {
                vertices,
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: Some(self.textures[&quad.material].clone()),
            });
        }
        gl_use_default_material();
    }
}
