//! Camera-centered, unlit LDR sky background; HDR and sky polygon masks are separate.
use anyhow::Result;
use macroquad::prelude::*;
use modkit_core::Entity;
use source_assets::{sky::UvTransform, vpk::Vfs};

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;
varying highp vec2 uv;
void main(){gl_Position=Projection*Model*vec4(position,1.0);uv=texcoord;}
"#;
const FRAGMENT: &str = r#"#version 100
precision mediump float;
uniform sampler2D Texture;
varying highp vec2 uv;
void main(){gl_FragColor=vec4(texture2D(Texture,uv).rgb,1.0);}
"#;
const CORNERS: [(f32, f32); 4] = [(-1., -1.), (-1., 1.), (1., 1.), (1., -1.)];
const INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];
// Retail engine DrawSkyBox traverses materials rt,lf,bk,ft,up,dn.
const DRAW_ORDER: [usize; 6] = [0, 2, 1, 3, 4, 5];

fn face_vector(face: usize, s: f32, t: f32) -> Vec3 {
    // Material order rt,bk,lf,ft,up,dn differs from VTF cubemap-file order.
    match face {
        0 => vec3(1., -s, t),
        1 => vec3(s, 1., t),
        2 => vec3(-1., s, t),
        3 => vec3(-s, -1., t),
        4 => vec3(-t, -s, 1.),
        5 => vec3(t, -s, -1.),
        _ => unreachable!("sky requires exactly six faces"),
    }
}
fn face_uv(s: f32, t: f32, transform: UvTransform) -> Vec2 {
    // Source first insets to a fixed 512-texel edge, then applies the VMT transform.
    // GPU sampling clamps the transformed coordinates, including half-height sides.
    let edge = 1. / 512.;
    let uv = glam::Vec2::new(
        ((s + 1.) * 0.5).clamp(edge, 1. - edge),
        1. - ((t + 1.) * 0.5).clamp(edge, 1. - edge),
    );
    let uv = transform.apply(uv);
    vec2(uv.x, uv.y)
}
fn vertices(face: usize, transform: UvTransform, far: f32) -> Vec<Vertex> {
    // At the cube corners distance is just below z_far, matching owned-retail d.
    let distance = far * 0.577_35;
    CORNERS
        .iter()
        .map(|&(s, t)| Vertex {
            position: face_vector(face, s, t) * distance,
            uv: face_uv(s, t, transform),
            color: [255; 4],
            normal: Vec4::ZERO,
        })
        .collect()
}
fn centered_camera(world: &Camera3D, direction: Vec3) -> Camera3D {
    Camera3D {
        position: Vec3::ZERO,
        target: direction,
        up: world.up,
        fovy: world.fovy,
        aspect: world.aspect,
        z_near: world.z_near,
        z_far: world.z_far,
        ..Default::default()
    }
}

pub struct Sky {
    name: String,
    faces: Vec<Mesh>,
    metadata: serde_json::Value,
    material: Material,
    far: f32,
}
impl Sky {
    pub fn load(vfs: &Vfs, entities: &[Entity]) -> Result<Option<Self>> {
        let Some(sky) = source_assets::sky::load(vfs, entities, 512)? else {
            return Ok(None);
        };
        let metadata = serde_json::json!(sky
            .faces
            .iter()
            .map(|face| {
                serde_json::json!({
                    "suffix": face.suffix, "material": face.material, "texture": face.texture,
                    "width": face.image.width, "height": face.image.height,
                    "uv_transform": face.transform.rows(),
                })
            })
            .collect::<Vec<_>>());
        let faces = sky
            .faces
            .iter()
            .enumerate()
            .map(|(index, face)| {
                let texture =
                    Texture2D::from_rgba8(face.image.width, face.image.height, &face.image.rgba);
                texture.set_filter(FilterMode::Linear);
                // A sky may transform v beyond one. Repeat would put clouds below the horizon.
                unsafe {
                    get_internal_gl().quad_context.texture_set_wrap(
                        texture.raw_miniquad_id(),
                        miniquad::TextureWrap::Clamp,
                        miniquad::TextureWrap::Clamp,
                    );
                }
                Mesh {
                    vertices: vertices(index, face.transform, 32000.),
                    indices: INDICES.to_vec(),
                    texture: Some(texture),
                }
            })
            .collect();
        let material = load_material(
            miniquad::ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                pipeline_params: miniquad::PipelineParams {
                    depth_test: miniquad::Comparison::Always,
                    depth_write: false,
                    color_blend: None,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        Ok(Some(Self {
            name: sky.name,
            faces,
            metadata,
            material,
            far: 32000.,
        }))
    }
    /// Draw before both miniature 3D scenery and the playable world, never after them.
    pub fn draw(&mut self, world_camera: &Camera3D, direction: Vec3) {
        if self.far != world_camera.z_far {
            let distance = world_camera.z_far / self.far;
            for face in &mut self.faces {
                for vertex in &mut face.vertices {
                    vertex.position *= distance;
                }
            }
            self.far = world_camera.z_far;
        }
        set_camera(&centered_camera(world_camera, direction));
        gl_use_material(&self.material);
        for index in DRAW_ORDER {
            draw_mesh(&self.faces[index]);
        }
        // Flush before returning so a later camera/material cannot reinterpret these draws.
        unsafe {
            get_internal_gl().flush();
        }
        gl_use_default_material();
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({ "name": self.name, "faces": self.metadata, "mode": "LDR" })
    }
}

pub fn prepare(vfs: &Vfs, entities: &[Entity]) -> (Option<Sky>, Option<String>) {
    match Sky::load(vfs, entities) {
        Ok(sky) => (sky, None),
        Err(error) => {
            let error = format!("2D sky assets: {error:#}");
            eprintln!("{error}");
            (None, Some(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn faces_preserve_retail_axes_orientation_and_far_bound() {
        let expected = [Vec3::X, Vec3::Y, -Vec3::X, -Vec3::Y, Vec3::Z, -Vec3::Z];
        for (face, axis) in expected.into_iter().enumerate() {
            assert_eq!(face_vector(face, 0., 0.), axis);
            let mesh = vertices(face, UvTransform::default(), 32000.);
            assert_eq!(mesh.len(), 4);
            assert!(mesh.iter().all(|v| v.position.length() < 32000.));
            let average = mesh.iter().map(|v| v.position).sum::<Vec3>() / 4.;
            assert_eq!(average.normalize(), axis);
        }
        // Adjacent sides share exact edge coordinates; top/bottom have the retail rotation.
        assert_eq!(face_vector(0, -1., 1.), face_vector(1, 1., 1.));
        assert_eq!(face_vector(0, 1., 1.), face_vector(3, -1., 1.));
        assert_eq!(face_vector(0, -1., 1.), face_vector(4, -1., -1.));
        assert_eq!(face_vector(0, -1., -1.), face_vector(5, -1., 1.));
        assert_eq!(INDICES, [0, 1, 2, 0, 2, 3]);
    }
    #[test]
    fn uv_inset_precedes_half_height_material_transform() {
        let identity = UvTransform::default();
        let edge = 1. / 512.;
        assert_eq!(face_uv(-1., -1., identity), vec2(edge, 1. - edge));
        assert_eq!(face_uv(1., 1., identity), vec2(1. - edge, edge));
        let half_height = UvTransform::parse("center 0 0 scale 1 2").unwrap();
        assert_eq!(face_uv(0., 1., half_height), vec2(0.5, 2. * edge));
        assert_eq!(face_uv(0., 0., half_height), vec2(0.5, 1.));
        assert_eq!(face_uv(0., -1., half_height), vec2(0.5, 2. - 2. * edge));
        // Sampling's Clamp wrap is necessary; clamping vertices before interpolation
        // would compress the entire half-height side and move its horizon.
        assert!(face_uv(0., -1., half_height).y > 1.);
    }
    #[test]
    fn sky_camera_removes_translation_and_preserves_projection_and_direction() {
        let direction = vec3(1., 2., 0.5).normalize();
        let mut reference = None;
        for position in [
            Vec3::ZERO,
            vec3(-4494., 96., 22.),
            vec3(19000., -8000., 2048.),
        ] {
            let world = Camera3D {
                position,
                target: position + direction,
                up: Vec3::Z,
                fovy: 1.1,
                aspect: Some(16. / 9.),
                z_near: 1.,
                z_far: 32000.,
                ..Default::default()
            };
            let sky = centered_camera(&world, direction);
            assert_eq!(sky.position, Vec3::ZERO);
            assert_eq!(sky.target, direction);
            assert_eq!(sky.fovy, world.fovy);
            assert_eq!(sky.aspect, world.aspect);
            let matrix =
                Mat4::perspective_rh_gl(sky.fovy, sky.aspect.unwrap(), sky.z_near, sky.z_far)
                    * Mat4::look_at_rh(sky.position, sky.target, sky.up);
            if let Some(reference) = reference {
                assert_eq!(matrix, reference);
            } else {
                reference = Some(matrix);
            }
        }
    }
}
