//! Owned projectile sprite materials. Source particles, dynamic lights and beam
//! rings need separate implementations; these quads retain normal world depth.
use crate::{
    projectiles::{EffectKind, ProjectileKind, Projectiles},
    rendering::Materials,
};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use source_assets::{vpk::Vfs, vtf};
use std::collections::{BTreeMap, HashMap};

const SPRITES: &[&str] = &[
    "effects/ar2_altfire1",
    "effects/ar2_altfire1b",
    "effects/combinemuzzle1",
    "effects/combinemuzzle1_nocull",
    "effects/combinemuzzle2_nocull",
    "effects/fire_cloud1",
    "effects/fire_cloud2",
];
struct Sprite {
    texture: Texture2D,
    kind: usize,
}
pub struct ProjectileVisuals {
    sprites: HashMap<String, Sprite>,
    previous: HashMap<u64, glam::Vec3>,
    rng: u64,
    pub errors: BTreeMap<String, String>,
    pub ball_frames: u64,
    pub effect_frames: u64,
}
fn load(vfs: &Vfs, material: &str) -> Result<Sprite> {
    let base = vfs
        .base_texture(material)?
        .context("projectile sprite base texture missing")?;
    let data = vfs
        .read(&format!("materials/{}.vtf", base.trim_end_matches(".vtf")))?
        .context("projectile sprite texture missing")?;
    let image = vtf::decode(&data, 512)?;
    let texture = Texture2D::from_rgba8(image.width, image.height, &image.rgba);
    texture.set_filter(FilterMode::Linear);
    unsafe {
        get_internal_gl().quad_context.texture_set_wrap(
            texture.raw_miniquad_id(),
            miniquad::TextureWrap::Clamp,
            miniquad::TextureWrap::Clamp,
        );
    }
    Ok(Sprite {
        texture,
        kind: crate::rendering::kind(vfs, material),
    })
}
fn basis(direction: Vec3) -> (Vec3, Vec3) {
    let direction = direction.normalize_or_zero();
    let mut right = direction.cross(Vec3::Z).normalize_or_zero();
    if right.length_squared() == 0. {
        right = Vec3::Y;
    }
    (right, right.cross(direction).normalize_or_zero())
}
fn blur(displacement: f32, index: usize) -> (f32, f32) {
    let speed = displacement.clamp(0., 32.);
    (
        (speed * 0.5).min(4.) * (index + 1) as f32,
        ((speed - 4.) / 28.).clamp(0., 1.) * (1. - index as f32 / 12.),
    )
}
impl ProjectileVisuals {
    pub fn new(vfs: &Vfs) -> Self {
        let mut result = Self {
            sprites: HashMap::new(),
            previous: HashMap::new(),
            rng: 0x953732fb,
            errors: BTreeMap::new(),
            ball_frames: 0,
            effect_frames: 0,
        };
        for name in SPRITES {
            match load(vfs, name) {
                Ok(sprite) => {
                    result.sprites.insert((*name).into(), sprite);
                }
                Err(error) => {
                    result.errors.insert((*name).into(), format!("{error:#}"));
                }
            }
        }
        result
    }
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u32 << 24) as f32
    }
    #[allow(clippy::too_many_arguments)]
    fn quad(
        &self,
        material: &str,
        center: Vec3,
        axes: (Vec3, Vec3),
        size: f32,
        roll: f32,
        intensity: f32,
        materials: &Materials,
    ) {
        let Some(sprite) = self.sprites.get(material) else {
            return;
        };
        if size <= 0. || intensity <= 0. {
            return;
        }
        let (sin, cos) = roll.sin_cos();
        let right = (axes.0 * cos + axes.1 * sin) * size;
        let up = (axes.0 * -sin + axes.1 * cos) * size;
        let shade = (intensity.clamp(0., 1.) * 255.).round() as u8;
        let positions = [
            center - right - up,
            center - right + up,
            center + right + up,
            center + right - up,
        ];
        let uv = [vec2(0., 1.), vec2(0., 0.), vec2(1., 0.), vec2(1., 1.)];
        let vertices = positions
            .into_iter()
            .zip(uv)
            .map(|(position, uv)| Vertex {
                position,
                uv,
                color: [shade, shade, shade, 255],
                normal: Vec4::ZERO,
            })
            .collect();
        materials.select(None, sprite.kind, None, Vec2::ZERO, Vec4::ONE);
        draw_mesh(&Mesh {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(sprite.texture.clone()),
        });
    }
    pub fn draw(
        &mut self,
        projectiles: &Projectiles,
        direction: Vec3,
        time: f64,
        paused: bool,
        materials: &Materials,
    ) {
        let axes = basis(direction);
        self.previous
            .retain(|id, _| projectiles.active.iter().any(|p| p.id == *id));
        for ball in projectiles
            .active
            .iter()
            .filter(|p| p.kind == ProjectileKind::CombineBall)
        {
            let position = vec3(ball.position.x, ball.position.y, ball.position.z);
            let (brightness, size) = if paused {
                (0.2, 1.5)
            } else {
                (0.2 + self.random() * 0.1, 1.5 + self.random())
            };
            self.quad(
                "effects/combinemuzzle1",
                position,
                axes,
                ball.radius * size,
                0.,
                brightness,
                materials,
            );
            let previous = self
                .previous
                .insert(ball.id, ball.position)
                .unwrap_or(ball.previous_position);
            let displacement = ball.position - previous;
            let movement = displacement.normalize_or_zero();
            for i in 0..8 {
                let (distance, brightness) = blur(displacement.length(), i);
                let center = ball.position - movement * distance;
                self.quad(
                    "effects/ar2_altfire1b",
                    vec3(center.x, center.y, center.z),
                    axes,
                    ball.radius,
                    0.,
                    brightness,
                    materials,
                );
            }
            // DrawHaloOriented's roll is radians, and the free ball uses a
            // pulsating billboard rather than the held-ball model.
            self.quad(
                "effects/ar2_altfire1",
                position,
                axes,
                ball.radius + (time as f32 * 25.).sin(),
                ball.spawned_at as f32,
                1.,
                materials,
            );
            self.ball_frames += 1;
        }
        for effect in &projectiles.effects {
            let age = (time - effect.at).max(0.) as f32;
            let position = vec3(effect.position.x, effect.position.y, effect.position.z);
            match effect.kind {
                EffectKind::BallImpact => {
                    let normal =
                        vec3(effect.normal.x, effect.normal.y, effect.normal.z).normalize_or_zero();
                    let axes = basis(normal);
                    self.quad(
                        "effects/combinemuzzle1_nocull",
                        position + normal * 0.5,
                        axes,
                        effect.radius * 10.,
                        effect.at as f32,
                        (1. - age / 0.25).max(0.),
                        materials,
                    );
                    self.quad(
                        "effects/combinemuzzle2_nocull",
                        position + normal * 0.5,
                        axes,
                        effect.radius * (2. + age / 0.5 * 2.),
                        effect.at as f32,
                        (1. - age / 0.5).max(0.),
                        materials,
                    );
                }
                EffectKind::GrenadeExplosion => {
                    // Bounded owned fire sprites, pending Source particle-cloud simulation.
                    self.quad(
                        "effects/fire_cloud1",
                        position,
                        axes,
                        32. + age * 64.,
                        0.,
                        (1. - age / 0.5).max(0.),
                        materials,
                    );
                    self.quad(
                        "effects/fire_cloud2",
                        position,
                        axes,
                        24. + age * 48.,
                        1.,
                        (1. - age / 0.5).max(0.),
                        materials,
                    );
                }
                EffectKind::BallExplosion => (), // Native sparks/beam rings need their own particle path.
            }
            self.effect_frames += 1;
        }
        gl_use_default_material();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blur_uses_rendered_frame_displacement_and_retains_eight_samples() {
        assert_eq!(blur(0., 0), (0., 0.));
        assert_eq!(blur(4., 7), (16., 0.));
        assert_eq!(blur(32., 0), (4., 1.));
        let (distance, intensity) = blur(100., 7);
        assert_eq!(distance, 32.);
        assert!((intensity - 5. / 12.).abs() < 0.00001);
    }
    #[test]
    fn billboard_axes_stay_orthogonal_at_horizontal_and_vertical_views() {
        for direction in [Vec3::X, Vec3::Y, Vec3::Z, vec3(1., 2., 3.)] {
            let (right, up) = basis(direction);
            assert!((right.length() - 1.).abs() < 0.00001);
            assert!((up.length() - 1.).abs() < 0.00001);
            assert!(right.dot(up).abs() < 0.00001);
            assert!(direction.normalize().dot(right).abs() < 0.00001);
        }
    }
}
