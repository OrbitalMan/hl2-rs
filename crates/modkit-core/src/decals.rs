//! Projected, triangle-clipped impact marks. Positions stay in the receiver's local space.
use crate::Surface;
use glam::{Vec2, Vec3};
#[derive(Clone, Copy, Debug)]
pub struct DecalVertex {
    pub position: Vec3,
    pub uv: Vec2,
}
pub fn project(
    surfaces: &[Surface],
    center: Vec3,
    normal: Vec3,
    radius: f32,
    angle: f32,
) -> Vec<DecalVertex> {
    if !radius.is_finite() || radius <= 0. || normal.length_squared() < 0.5 {
        return Vec::new();
    }
    let normal = normal.normalize();
    let axis = if normal.z.abs() < 0.9 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = normal.cross(axis).normalize();
    let up = normal.cross(right);
    let u = right * angle.cos() + up * angle.sin();
    let v = normal.cross(u);
    let mut result = Vec::new();
    for surface in surfaces.iter().filter(|s| !s.background) {
        for tri in surface.indices.as_chunks::<3>().0 {
            let p = tri.map(|i| surface.vertices[i as usize].position);
            let tri_normal = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
            if tri_normal.dot(normal).abs() < 0.85
                || p.iter().any(|p| (*p - center).dot(normal).abs() > 0.5)
            {
                continue;
            }
            let mut polygon = p
                .map(|position| DecalVertex {
                    position,
                    uv: Vec2::new((position - center).dot(u), (position - center).dot(v)),
                })
                .to_vec();
            for (axis, sign) in [(0, 1.), (0, -1.), (1, 1.), (1, -1.)] {
                let input = std::mem::take(&mut polygon);
                if input.is_empty() {
                    break;
                }
                for i in 0..input.len() {
                    let a = input[i];
                    let b = input[(i + 1) % input.len()];
                    let da = a.uv[axis] * sign - radius;
                    let db = b.uv[axis] * sign - radius;
                    if da <= 0. {
                        polygon.push(a);
                    }
                    if (da <= 0.) != (db <= 0.) {
                        let t = da / (da - db);
                        polygon.push(DecalVertex {
                            position: a.position.lerp(b.position, t),
                            uv: a.uv.lerp(b.uv, t),
                        });
                    }
                }
            }
            for i in 1..polygon.len().saturating_sub(1) {
                for mut vertex in [polygon[0], polygon[i], polygon[i + 1]] {
                    vertex.position += normal * 0.06;
                    vertex.uv = vertex.uv / (radius * 2.) + Vec2::splat(0.5);
                    result.push(vertex);
                }
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impact_is_clipped_at_surface_edge_and_never_spills_to_parallel_wall() {
        let surface = Surface {
            flex_source: None,
            background: false,
            material: String::new(),
            lightmap: None,
            vertices: [Vec3::ZERO, Vec3::X * 10., Vec3::Y * 10.]
                .into_iter()
                .map(|position| crate::Vertex {
                    normal: Default::default(),
                    position,
                    uv: Vec2::ZERO,
                    color: [255; 4],
                    light_uv: Vec2::ZERO,
                    skin: None,
                })
                .collect(),
            indices: vec![0, 1, 2],
        };
        let vertices = project(
            std::slice::from_ref(&surface),
            Vec3::new(1., 1., 0.),
            Vec3::Z,
            3.,
            0.,
        );
        assert!(!vertices.is_empty());
        assert!(vertices.iter().all(|v| v.position.x >= 0.
            && v.position.y >= 0.
            && v.position.x + v.position.y <= 10.01
            && v.uv.min_element() >= -0.001
            && v.uv.max_element() <= 1.001));
        assert!(project(&[surface], Vec3::new(1., 1., 3.), Vec3::Z, 3., 0.).is_empty());
    }
}
