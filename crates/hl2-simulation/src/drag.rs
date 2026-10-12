//! VPhysics air drag. Retail vphysics.dll (private evidence, physics-research-20261011/12):
//! the drag controller (10012ec0) runs every simulation step on objects with drag enabled
//! and scales the linear velocity by `-0.5 x query x air density x dt` and the angular
//! velocity by `-query x air density x dt` (each factor at least -1). The queries
//! (100173c0, 100172d0) sum per-axis |basis x velocity| in the object's frame, the
//! object's drag coefficient multiplying only the first axis term (retail grouping kept).
//! The bases (100187a0, recomputed on SetMass) come from the collision model's AABB in
//! metres (x 0.0254), the public IPhysicsCollision::CollideGetOrthographicAreas fractions
//! and the inverse mass (linear) or inverse rotational inertia (angular). Native
//! `air_density` prints 2.00 (session 12, d1_trainstation_02).
//!
//! Not verified: the orthographic fractions are computed here as the projected area of
//! the hull vertices over the AABB face (vphysics' own method is not traced); the bases
//! pair with Source-local axes (the retail velocity frame may be IVP's (x, -z, y), which
//! would swap the y and z terms); Rapier's principal inertia frame stands in for the
//! core's.
use glam::{Vec2, Vec3};

/// Native `air_density` (CPhysicsEnvironment default, observed 2.00).
pub const AIR_DENSITY: f32 = 2.;
/// objectparams_t default dragCoefficient / angDragCoefficient.
pub const DEFAULT_COEFFICIENT: f32 = 1.;

/// Geometry part of the drag bases: AABB extents (metres) and orthographic fractions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragGeometry {
    pub extents: Vec3,
    pub ortho: Vec3,
}
impl DragGeometry {
    /// From body-local hull points in metres.
    pub fn from_points(points: &[Vec3]) -> Option<Self> {
        let (first, rest) = points.split_first()?;
        let (lo, hi) = rest
            .iter()
            .fold((*first, *first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let extents = hi - lo;
        let fraction = |area: f32, a: f32, b: f32| {
            if a * b > 0. {
                (area / (a * b)).clamp(0., 1.)
            } else {
                0.
            }
        };
        let projected =
            |pick: fn(&Vec3) -> Vec2| convex_area(&points.iter().map(pick).collect::<Vec<_>>());
        let ortho = Vec3::new(
            fraction(projected(|p| Vec2::new(p.y, p.z)), extents.y, extents.z),
            fraction(projected(|p| Vec2::new(p.x, p.z)), extents.x, extents.z),
            fraction(projected(|p| Vec2::new(p.x, p.y)), extents.x, extents.y),
        );
        Some(Self { extents, ortho })
    }
    /// Linear basis (100187a0 fields +14/+18/+1c): face area x fraction x inverse mass.
    pub fn linear_basis(&self, inv_mass: f32) -> Vec3 {
        let d = self.extents;
        Vec3::new(
            d.y * d.z * self.ortho.x,
            d.y * d.x * self.ortho.y,
            d.z * d.x * self.ortho.z,
        ) * inv_mass
    }
    /// Angular basis (fields +20/+24/+28) from the half extents, the inverse rotational
    /// inertia per axis and the two orthographic fractions of the faces sweeping round it.
    pub fn angular_basis(&self, inv_inertia: Vec3) -> Vec3 {
        let h = self.extents * 0.5;
        let (x2, y2, z2) = (h.x * h.x, h.y * h.y, h.z * h.z);
        // Each term: a^4/2 x b + a^2/3 x b x b^2 + a^2 x b x c^2 (retail operand order).
        let term =
            |a2: f32, b: f32, b2: f32, c2: f32| a2 * 0.5 * a2 * b + a2 / 3. * b * b2 + a2 * b * c2;
        Vec3::new(
            term(y2, h.x, x2, z2) * inv_inertia.x * self.ortho.y
                + term(z2, h.x, x2, y2) * inv_inertia.x * self.ortho.z,
            term(y2, h.z, z2, x2) * inv_inertia.y * self.ortho.x
                + term(x2, h.z, z2, y2) * inv_inertia.y * self.ortho.z,
            term(z2, h.y, y2, x2) * inv_inertia.z * self.ortho.x
                + term(x2, h.y, y2, z2) * inv_inertia.z * self.ortho.y,
        )
    }
}

/// 100173c0: |b.z v.z| + |b.y v.y| + |b.x v.x| x coefficient (local velocity, m/s).
pub fn linear_query(basis: Vec3, local: Vec3, coefficient: f32) -> f32 {
    (basis.z * local.z).abs() + (basis.y * local.y).abs() + (basis.x * local.x).abs() * coefficient
}
/// 100172d0: |b.z w.z| + |b.x w.x| x coefficient + |b.y w.y| (rad/s).
pub fn angular_query(basis: Vec3, local: Vec3, coefficient: f32) -> f32 {
    (basis.z * local.z).abs() + (basis.x * local.x).abs() * coefficient + (basis.y * local.y).abs()
}
/// 10012ec0: the multiplier added to a velocity this step (`v += v x factor`), in
/// [-1, 0]; positive or NaN products leave the velocity alone.
pub fn step_factor(query: f32, scale: f32, density: f32, dt: f32) -> f32 {
    let factor = scale * query * density * dt;
    if factor < -1. {
        -1.
    } else if factor < 0. {
        factor
    } else {
        0.
    }
}

/// Area of the convex hull of 2D points (monotone chain).
fn convex_area(points: &[Vec2]) -> f32 {
    let mut p: Vec<Vec2> = points.to_vec();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup();
    if p.len() < 3 {
        return 0.;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vec2> = Vec::with_capacity(p.len() * 2);
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 {
            Box::new(p.iter())
        } else {
            Box::new(p.iter().rev())
        };
        for &q in iter {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0.
            {
                hull.pop();
            }
            hull.push(q);
        }
        hull.pop();
    }
    let n = hull.len();
    (0..n)
        .map(|i| hull[i].perp_dot(hull[(i + 1) % n]))
        .sum::<f32>()
        .abs()
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cube_points(half: Vec3) -> Vec<Vec3> {
        (0..8)
            .map(|i| {
                Vec3::new(
                    if i & 1 == 0 { -half.x } else { half.x },
                    if i & 2 == 0 { -half.y } else { half.y },
                    if i & 4 == 0 { -half.z } else { half.z },
                )
            })
            .collect()
    }
    #[test]
    fn box_fills_its_faces_and_bases_scale_with_inverse_mass() {
        let g = DragGeometry::from_points(&cube_points(Vec3::new(0.5, 1., 2.))).unwrap();
        assert_eq!(g.extents, Vec3::new(1., 2., 4.));
        assert!((g.ortho - Vec3::ONE).length() < 1e-6);
        assert_eq!(g.linear_basis(0.5), Vec3::new(4., 1., 2.));
    }
    #[test]
    fn frag_hull_reproduces_the_native_fit_order() {
        // w_grenade.phy: an octagonal prism 3.6 x 3.6 x 9.0 units, 1 kg.
        let r = 1.8077 * 0.0254;
        let mut points = Vec::new();
        for k in 0..8 {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.;
            for z in [-4.40, 4.56] {
                points.push(Vec3::new(r * a.cos(), r * a.sin(), z * 0.0254));
            }
        }
        let g = DragGeometry::from_points(&points).unwrap();
        let b = g.linear_basis(1.);
        // Average over a tumbling body: |v_i| ~ |v| / 2 per axis.
        let c = 0.5 * AIR_DENSITY * (b.x + b.y + b.z) * 0.5 * 0.0254;
        assert!((5.0e-4..7.0e-4).contains(&c), "per-unit drag {c}");
    }
    #[test]
    fn factors_clamp_at_minus_one_and_ignore_positive_products() {
        assert_eq!(step_factor(1e6, -0.5, 2., 0.015), -1.);
        assert_eq!(step_factor(1., 0.5, 2., 0.015), 0.);
        assert!((step_factor(1., -0.5, 2., 0.015) + 0.015).abs() < 1e-7);
        assert!(
            (convex_area(&[Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y, Vec2::splat(0.5)]) - 1.).abs()
                < 1e-6
        );
    }
}
