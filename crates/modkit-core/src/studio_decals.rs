//! Decals on skinned studio models (DESIGN 13f). The engine builds them inside
//! studiorender (not in the SDK): the decal is clipped from the model's triangles as
//! they are posed at the moment of the hit and kept in the model's bind space with the
//! bones' weights, so it follows the animation afterwards.
//!
//! Space-agnostic: `posed`, `center` and `direction` share one frame; `bind` is the
//! mesh's own vertex space.
use glam::{Vec2, Vec3};

/// One mesh vertex: where it is now, where it is in the mesh, and its skin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinVertex {
    pub posed: Vec3,
    pub bind: Vec3,
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}
/// One decal vertex in the mesh's bind space; triangles are consecutive triples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioDecalVertex {
    pub bind: Vec3,
    /// Bind-space normal of the source triangle.
    pub normal: Vec3,
    pub uv: Vec2,
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}
/// Triangles whose posed normal faces the shot less than this are skipped (grazing
/// faces would smear the decal).
const MIN_FACING: f32 = 0.1;

/// Clip the posed triangles facing `direction` (the shot) to the square of half-size
/// `radius` around `center` in the plane across the shot, rotated by `angle`, within
/// `radius` in depth. Returns a triangle list.
pub fn project(
    vertices: &[SkinVertex],
    indices: &[u32],
    center: Vec3,
    direction: Vec3,
    radius: f32,
    angle: f32,
) -> Vec<StudioDecalVertex> {
    if !radius.is_finite() || radius <= 0. || direction.length_squared() < 1e-6 {
        return Vec::new();
    }
    let normal = -direction.normalize();
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
    for tri in indices.as_chunks::<3>().0 {
        let Some(corners) = tri
            .iter()
            .map(|&i| vertices.get(i as usize).copied())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let p = [corners[0].posed, corners[1].posed, corners[2].posed];
        let face = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        if face.dot(normal) < MIN_FACING {
            continue;
        }
        let local = p.map(|q| {
            let d = q - center;
            Vec3::new(d.dot(u), d.dot(v), d.dot(normal))
        });
        // Whole triangle outside one side of the decal box.
        if local.iter().all(|l| l.x > radius)
            || local.iter().all(|l| l.x < -radius)
            || local.iter().all(|l| l.y > radius)
            || local.iter().all(|l| l.y < -radius)
            || local.iter().all(|l| l.z > radius)
            || local.iter().all(|l| l.z < -radius)
        {
            continue;
        }
        // Clip in the decal plane, carrying barycentric coordinates.
        let mut polygon: Vec<(Vec2, Vec3)> = vec![
            (local[0].truncate(), Vec3::X),
            (local[1].truncate(), Vec3::Y),
            (local[2].truncate(), Vec3::Z),
        ];
        for (axis, sign) in [(0, 1.), (0, -1.), (1, 1.), (1, -1.)] {
            polygon = clip(&polygon, axis, sign, radius);
            if polygon.len() < 3 {
                break;
            }
        }
        if polygon.len() < 3 {
            continue;
        }
        let bind = [corners[0].bind, corners[1].bind, corners[2].bind];
        let bind_normal = (bind[1] - bind[0])
            .cross(bind[2] - bind[0])
            .normalize_or_zero();
        let vertex = |(st, bary): (Vec2, Vec3)| StudioDecalVertex {
            bind: bind[0] * bary.x + bind[1] * bary.y + bind[2] * bary.z,
            normal: bind_normal,
            uv: Vec2::new(st.x / (2. * radius) + 0.5, 0.5 - st.y / (2. * radius)),
            ..blend_skin(&corners, bary)
        };
        for k in 1..polygon.len() - 1 {
            result.push(vertex(polygon[0]));
            result.push(vertex(polygon[k]));
            result.push(vertex(polygon[k + 1]));
        }
    }
    result
}
/// The nearest front-facing posed triangle along the ray (`direction` normalized),
/// within `max`: where the decal really lands (the engine shoots studio decals with the
/// ray against the model's triangles; trace shapes only approximate the skin).
pub fn ray_distance(
    vertices: &[SkinVertex],
    indices: &[u32],
    origin: Vec3,
    direction: Vec3,
    max: f32,
) -> Option<f32> {
    let mut best: Option<f32> = None;
    for tri in indices.as_chunks::<3>().0 {
        let Some([a, b, c]) = tri
            .iter()
            .map(|&i| vertices.get(i as usize).map(|v| v.posed))
            .collect::<Option<Vec<_>>>()
            .and_then(|p| <[Vec3; 3]>::try_from(p).ok())
        else {
            continue;
        };
        // Moller-Trumbore, front faces only.
        let (e1, e2) = (b - a, c - a);
        if e1.cross(e2).dot(direction) >= 0. {
            continue;
        }
        let p = direction.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-9 {
            continue;
        }
        let s = origin - a;
        let u = s.dot(p) / det;
        let q = s.cross(e1);
        let v = direction.dot(q) / det;
        let t = e2.dot(q) / det;
        if u < 0. || v < 0. || u + v > 1. || !(0.0..=max).contains(&t) {
            continue;
        }
        if best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}
/// Sutherland-Hodgman against `sign * coord[axis] <= radius`.
fn clip(polygon: &[(Vec2, Vec3)], axis: usize, sign: f32, radius: f32) -> Vec<(Vec2, Vec3)> {
    let inside = |p: &(Vec2, Vec3)| sign * p.0[axis] <= radius;
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for (i, a) in polygon.iter().enumerate() {
        let b = &polygon[(i + 1) % polygon.len()];
        match (inside(a), inside(b)) {
            (true, true) => out.push(*b),
            (true, false) | (false, true) => {
                let (da, db) = (sign * a.0[axis] - radius, sign * b.0[axis] - radius);
                let t = da / (da - db);
                out.push((a.0.lerp(b.0, t), a.1.lerp(b.1, t)));
                if inside(b) {
                    out.push(*b);
                }
            }
            (false, false) => {}
        }
    }
    out
}
/// The corners' bone influences weighted by barycentric position; the four largest,
/// normalized.
fn blend_skin(corners: &[SkinVertex], bary: Vec3) -> StudioDecalVertex {
    let mut influences: Vec<(u16, f32)> = Vec::with_capacity(12);
    for (corner, weight) in corners.iter().zip(bary.to_array()) {
        for (joint, w) in corner.joints.iter().zip(corner.weights) {
            if w <= 0. || weight <= 0. {
                continue;
            }
            match influences.iter_mut().find(|(j, _)| j == joint) {
                Some((_, total)) => *total += w * weight,
                None => influences.push((*joint, w * weight)),
            }
        }
    }
    influences.sort_by(|a, b| b.1.total_cmp(&a.1));
    influences.truncate(4);
    let total: f32 = influences.iter().map(|(_, w)| w).sum();
    let mut joints = [corners[0].joints[0]; 4];
    let mut weights = [0.; 4];
    for (k, (joint, w)) in influences.iter().enumerate() {
        joints[k] = *joint;
        weights[k] = if total > 0. { w / total } else { 0. };
    }
    if total <= 0. {
        weights[0] = 1.;
    }
    StudioDecalVertex {
        bind: Vec3::ZERO,
        normal: Vec3::ZERO,
        uv: Vec2::ZERO,
        joints,
        weights,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A 20x20 quad in the y/z plane facing +x (bind) posed 100 units along +x, with
    /// its left half on bone 1 and its right half on bone 2 (synthetic).
    fn quad() -> (Vec<SkinVertex>, Vec<u32>) {
        let corner = |y: f32, z: f32| SkinVertex {
            posed: Vec3::new(100., y, z),
            bind: Vec3::new(0., y, z),
            joints: if y < 0. { [1, 0, 0, 0] } else { [2, 0, 0, 0] },
            weights: [1., 0., 0., 0.],
        };
        (
            vec![
                corner(-10., -10.),
                corner(10., -10.),
                corner(10., 10.),
                corner(-10., 10.),
            ],
            vec![0, 1, 2, 0, 2, 3],
        )
    }
    fn area(decal: &[StudioDecalVertex]) -> f32 {
        decal
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                (t[1].bind - t[0].bind)
                    .cross(t[2].bind - t[0].bind)
                    .length()
                    * 0.5
            })
            .sum()
    }
    #[test]
    fn a_shot_into_the_face_clips_a_square_in_bind_space() {
        let (vertices, indices) = quad();
        // Shot along -x into the face at its center, radius 3.
        let decal = project(
            &vertices,
            &indices,
            Vec3::new(100., 0., 0.),
            Vec3::NEG_X,
            3.,
            0.,
        );
        assert!((area(&decal) - 36.).abs() < 1e-3, "{}", area(&decal));
        for v in &decal {
            assert!(v.bind.x.abs() < 1e-5 && v.bind.y.abs() <= 3.001 && v.bind.z.abs() <= 3.001);
            assert!((0.0..=1.).contains(&v.uv.x) && (0.0..=1.).contains(&v.uv.y));
            assert!((v.weights.iter().sum::<f32>() - 1.).abs() < 1e-5);
        }
        // The weights blend across the bone seam (bones 1 and 2 both used).
        assert!(decal
            .iter()
            .any(|v| v.joints.contains(&1) && v.weights[0] < 1.));
    }
    #[test]
    fn the_ray_finds_the_front_face_it_crosses() {
        let (vertices, indices) = quad();
        let hit = ray_distance(
            &vertices,
            &indices,
            Vec3::new(130., 2., 3.),
            Vec3::NEG_X,
            64.,
        );
        assert!((hit.unwrap() - 30.).abs() < 1e-4);
        // Missing the quad, from behind it, or beyond the range.
        assert!(ray_distance(
            &vertices,
            &indices,
            Vec3::new(130., 20., 0.),
            Vec3::NEG_X,
            64.
        )
        .is_none());
        assert!(ray_distance(&vertices, &indices, Vec3::new(70., 0., 0.), Vec3::X, 64.).is_none());
        assert!(ray_distance(
            &vertices,
            &indices,
            Vec3::new(200., 0., 0.),
            Vec3::NEG_X,
            64.
        )
        .is_none());
    }
    #[test]
    fn back_faces_out_of_depth_and_degenerate_input_give_nothing() {
        let (vertices, indices) = quad();
        let center = Vec3::new(100., 0., 0.);
        // From behind the face.
        assert!(project(&vertices, &indices, center, Vec3::X, 3., 0.).is_empty());
        // Too far in front of the face.
        let far = Vec3::new(110., 0., 0.);
        assert!(project(&vertices, &indices, far, Vec3::NEG_X, 3., 0.).is_empty());
        assert!(project(&vertices, &indices, center, Vec3::NEG_X, 0., 0.).is_empty());
        assert!(project(&vertices, &[0, 1, 9], center, Vec3::NEG_X, 3., 0.).is_empty());
        // Near the quad's edge only the overlapping part remains.
        let edge = project(
            &vertices,
            &indices,
            Vec3::new(100., 9., 0.),
            Vec3::NEG_X,
            3.,
            0.,
        );
        assert!((area(&edge) - 4. * 6.).abs() < 1e-3, "{}", area(&edge));
    }
}
