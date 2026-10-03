//! Rapier collision/rigid-body adapter. This is not Valve's proprietary VPhysics solver.
use glam::{Quat, Vec3};
use modkit_core::{movement::CollisionWorld, Brush, Surface, Trace, World};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::prelude::*;
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub entity: usize,
    pub position: Vec3,
    pub normal: Vec3,
}
const SCALE: f32 = 1. / 39.37;
fn enabled_filter() -> QueryFilter<'static> {
    static ENABLED: fn(ColliderHandle, &Collider) -> bool = |_, c| c.is_enabled();
    QueryFilter::default().exclude_sensors().predicate(&ENABLED)
}
fn vector(p: Vec3) -> Vector<Real> {
    vector![p.x * SCALE, p.y * SCALE, p.z * SCALE]
}
fn pose(origin: Vec3, rotation: Quat) -> Isometry<Real> {
    Isometry::from_parts(
        Translation::from(vector(origin)),
        Rotation::from_quaternion(rapier3d::na::Quaternion::new(
            rotation.w, rotation.x, rotation.y, rotation.z,
        )),
    )
}
pub fn angles(a: Vec3) -> Quat {
    Quat::from_rotation_z(a.y.to_radians())
        * Quat::from_rotation_y(a.x.to_radians())
        * Quat::from_rotation_x(a.z.to_radians())
}
fn brush_shape(brush: &Brush) -> Option<SharedShape> {
    let p = &brush.planes;
    let mut points = Vec::new();
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            for k in j + 1..p.len() {
                let det = p[i].normal.dot(p[j].normal.cross(p[k].normal));
                if det.abs() < 1e-5 {
                    continue;
                }
                let point = (p[j].normal.cross(p[k].normal) * p[i].distance
                    + p[k].normal.cross(p[i].normal) * p[j].distance
                    + p[i].normal.cross(p[j].normal) * p[k].distance)
                    / det;
                if point.is_finite()
                    && p.iter().all(|v| v.normal.dot(point) <= v.distance + 0.05)
                    && !points
                        .iter()
                        .any(|v: &Vec3| v.distance_squared(point) < 0.0001)
                {
                    points.push(point);
                }
            }
        }
    }
    SharedShape::convex_hull(
        &points
            .into_iter()
            .map(|p| Point::from(vector(p)))
            .collect::<Vec<_>>(),
    )
}
fn mesh_shape(surfaces: &[Surface], convex: bool) -> Option<SharedShape> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for s in surfaces {
        let base = vertices.len() as u32;
        vertices.extend(s.vertices.iter().map(|v| Point::from(vector(v.position))));
        indices.extend(
            s.indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|i| [i[0] + base, i[1] + base, i[2] + base]),
        );
    }
    if convex {
        SharedShape::convex_hull(&vertices)
    } else {
        SharedShape::trimesh_with_flags(
            vertices,
            indices,
            TriMeshFlags::MERGE_DUPLICATE_VERTICES | TriMeshFlags::FIX_INTERNAL_EDGES,
        )
        .ok()
    }
}
#[derive(Default)]
pub struct Physics {
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    query: QueryPipeline,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad: BroadPhaseMultiSap,
    narrow: NarrowPhase,
    impulses: ImpulseJointSet,
    multibody: MultibodyJointSet,
    ccd: CCDSolver,
    entity_colliders: BTreeMap<usize, Vec<ColliderHandle>>,
    pub dynamic: BTreeMap<usize, RigidBodyHandle>,
    pub skipped: usize,
}
impl Physics {
    pub fn new(world: &World) -> Self {
        let mut p = Self::default();
        for brush in &world.brushes {
            if let Some(shape) = brush_shape(brush) {
                p.colliders
                    .insert(ColliderBuilder::new(shape).friction(0.8));
            } else {
                p.skipped += 1;
            }
        }
        for surface in &world.terrain {
            if let Some(shape) = mesh_shape(std::slice::from_ref(surface), false) {
                p.colliders
                    .insert(ColliderBuilder::new(shape).friction(0.8));
            }
        }
        for (id, e) in world.entities.iter().enumerate() {
            let Some(model_id) = e
                .get("model")
                .and_then(|m| m.strip_prefix('*'))
                .and_then(|s| s.parse::<usize>().ok())
            else {
                continue;
            };
            let Some(model) = world.brush_models.iter().find(|m| m.id == model_id) else {
                continue;
            };
            let solid = matches!(
                e.class(),
                "func_brush"
                    | "func_wall"
                    | "func_wall_toggle"
                    | "func_door"
                    | "func_door_rotating"
                    | "func_movelinear"
                    | "func_rotating"
                    | "func_breakable"
                    | "func_physbox"
                    | "func_detail"
            );
            if !solid || e.get("solid") == Some("0") || e.get("Solidity") == Some("1") {
                continue;
            }
            let position = pose(
                e.origin(),
                angles(
                    modkit_core::parse_vec3(e.get("angles").unwrap_or("0 0 0"))
                        .unwrap_or(Vec3::ZERO),
                ),
            );
            for b in &model.brushes {
                if let Some(shape) = brush_shape(b) {
                    let c = p.colliders.insert(
                        ColliderBuilder::new(shape)
                            .position(position)
                            .user_data(id as u128 + 1)
                            .friction(0.8),
                    );
                    p.entity_colliders.entry(id).or_default().push(c);
                }
            }
        }
        let mut shape_cache = BTreeMap::new();
        for instance in &world.model_instances {
            if instance.background {
                continue;
            }
            if !instance.solid {
                continue;
            }
            let Some(asset) = world.model_assets.get(&instance.asset_key()) else {
                continue;
            };
            let dynamic = instance.kind.starts_with("prop_physics");
            let key = (instance.asset_key(), dynamic);
            let shape = shape_cache
                .entry(key)
                .or_insert_with(|| mesh_shape(asset, dynamic));
            let Some(shape) = shape else {
                p.skipped += 1;
                continue;
            };
            // Scaled props need separately scaled meshes; skip their collider rather than use a wrong size.
            if (instance.scale - 1.).abs() > 0.001 {
                p.skipped += 1;
                continue;
            }
            let position = pose(instance.origin, angles(instance.angles));
            let entity = instance.entity;
            let collider = ColliderBuilder::new(shape.clone())
                .friction(0.7)
                .restitution(0.05)
                .user_data(entity.map_or(0, |i| i as u128 + 1));
            if dynamic {
                let body = p.bodies.insert(
                    RigidBodyBuilder::dynamic()
                        .position(position)
                        .ccd_enabled(true)
                        .linear_damping(0.05)
                        .angular_damping(0.1),
                );
                let c = p
                    .colliders
                    .insert_with_parent(collider.mass(10.), body, &mut p.bodies);
                if let Some(id) = entity {
                    p.dynamic.insert(id, body);
                    p.entity_colliders.entry(id).or_default().push(c);
                }
            } else {
                let c = p.colliders.insert(collider.position(position));
                if let Some(id) = entity {
                    p.entity_colliders.entry(id).or_default().push(c);
                }
            }
        }
        p.query.update(&p.colliders);
        p
    }
    pub fn set_entity(&mut self, id: usize, origin: Vec3, rotation: Quat, enabled: bool) {
        let dynamic = self.dynamic.get(&id);
        if let Some(body) = dynamic.and_then(|h| self.bodies.get_mut(*h)) {
            body.set_enabled(enabled);
        }
        if let Some(colliders) = self.entity_colliders.get(&id) {
            for h in colliders {
                if let Some(c) = self.colliders.get_mut(*h) {
                    if dynamic.is_none() {
                        c.set_position(pose(origin, rotation));
                    }
                    c.set_enabled(enabled);
                }
            }
        }
    }
    pub fn tick(&mut self, dt: f32) {
        self.pipeline.step(
            &vector![0., 0., -600. * SCALE],
            &IntegrationParameters {
                dt,
                ..Default::default()
            },
            &mut self.islands,
            &mut self.broad,
            &mut self.narrow,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulses,
            &mut self.multibody,
            &mut self.ccd,
            Some(&mut self.query),
            &(),
            &(),
        );
    }
    pub fn entity_pose(&self, id: usize) -> Option<(Vec3, Quat)> {
        let b = self.bodies.get(*self.dynamic.get(&id)?)?;
        let p = b.position();
        let q = p.rotation.quaternion();
        Some((
            Vec3::new(p.translation.x, p.translation.y, p.translation.z) / SCALE,
            Quat::from_xyzw(q.i, q.j, q.k, q.w),
        ))
    }
    pub fn ray(&self, origin: Vec3, direction: Vec3, distance: f32) -> Option<(usize, Vec3)> {
        self.impact_ray(origin, direction, distance)
            .map(|h| (h.entity, h.position))
    }
    pub fn impact_ray(&self, origin: Vec3, direction: Vec3, distance: f32) -> Option<RayHit> {
        let ray = Ray::new(
            Point::from(vector(origin)),
            vector![direction.x, direction.y, direction.z],
        );
        let (h, hit) = self.query.cast_ray_and_get_normal(
            &self.bodies,
            &self.colliders,
            &ray,
            distance * SCALE,
            true,
            enabled_filter(),
        )?;
        let id = self.colliders[h].user_data as usize;
        Some(RayHit {
            entity: id.wrapping_sub(1),
            position: origin + direction * (hit.time_of_impact / SCALE),
            normal: Vec3::new(hit.normal.x, hit.normal.y, hit.normal.z),
        })
    }
    pub fn impulse(&mut self, id: usize, direction: Vec3, strength: f32) {
        if let Some(h) = self.dynamic.get(&id) {
            if let Some(b) = self.bodies.get_mut(*h) {
                b.apply_impulse(
                    vector![
                        direction.x * strength,
                        direction.y * strength,
                        direction.z * strength
                    ],
                    true,
                );
            }
        }
    }
    /// Symmetric box sweep for melee's secondary trace. Geometry still uses Rapier shapes.
    pub fn impact_hull(
        &self,
        origin: Vec3,
        direction: Vec3,
        distance: f32,
        half: Vec3,
    ) -> Option<RayHit> {
        if distance <= 0. || direction.length_squared() < 1e-8 {
            return None;
        }
        let shape = Cuboid::new(vector(half));
        let options = ShapeCastOptions {
            max_time_of_impact: 1.,
            target_distance: 0.,
            stop_at_penetration: true,
            compute_impact_geometry_on_penetration: true,
        };
        let (handle, hit) = self.query.cast_shape(
            &self.bodies,
            &self.colliders,
            &pose(origin, Quat::IDENTITY),
            &vector(direction * distance),
            &shape,
            options,
            enabled_filter(),
        )?;
        Some(RayHit {
            entity: (self.colliders[handle].user_data as usize).wrapping_sub(1),
            position: origin + direction * distance * hit.time_of_impact,
            normal: Vec3::new(hit.normal1.x, hit.normal1.y, hit.normal1.z),
        })
    }
}
impl CollisionWorld for Physics {
    fn trace_hull(&self, start: Vec3, end: Vec3, mins: Vec3, maxs: Vec3) -> Trace {
        let half = (maxs - mins) * 0.5;
        let center = (maxs + mins) * 0.5;
        let delta = end - start;
        let stationary = delta.length_squared() < 0.000001;
        // A standing hull merely touching the floor must not prevent uncrouching.
        // Shrink a stationary overlap probe by half the Source collision epsilon.
        let shape = Cuboid::new(vector(if stationary {
            (half - Vec3::splat(0.015625)).max(Vec3::splat(0.001))
        } else {
            half
        }));
        let position = Isometry::translation(
            (start.x + center.x) * SCALE,
            (start.y + center.y) * SCALE,
            (start.z + center.z) * SCALE,
        );
        if stationary {
            let start_solid = self
                .query
                .intersection_with_shape(
                    &self.bodies,
                    &self.colliders,
                    &position,
                    &shape,
                    enabled_filter(),
                )
                .is_some();
            return Trace {
                fraction: if start_solid { 0. } else { 1. },
                normal: Vec3::ZERO,
                start_solid,
            };
        }
        let options = ShapeCastOptions {
            max_time_of_impact: 1.,
            target_distance: 0.03125 * SCALE,
            stop_at_penetration: false,
            compute_impact_geometry_on_penetration: true,
        };
        if let Some((_, hit)) = self.query.cast_shape(
            &self.bodies,
            &self.colliders,
            &position,
            &vector(delta),
            &shape,
            options,
            enabled_filter(),
        ) {
            let n = hit.normal1;
            Trace {
                fraction: hit.time_of_impact.clamp(0., 1.),
                normal: Vec3::new(n.x, n.y, n.z),
                start_solid: false,
            }
        } else {
            Trace {
                fraction: 1.,
                normal: Vec3::ZERO,
                start_solid: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn melee_box_sweep_reaches_a_nearby_offset_target_but_not_outside_its_width() {
        let mut physics = Physics::default();
        physics.colliders.insert(
            ColliderBuilder::cuboid(5. * SCALE, 5. * SCALE, 5. * SCALE)
                .translation(vector![40. * SCALE, 20. * SCALE, 0.])
                .user_data(1),
        );
        physics.query.update(&physics.colliders);
        assert!(physics.impact_ray(Vec3::ZERO, Vec3::X, 75.).is_none());
        assert_eq!(
            physics
                .impact_hull(Vec3::ZERO, Vec3::X, 75. - 1.732 * 16., Vec3::splat(16.))
                .unwrap()
                .entity,
            0
        );
        assert!(physics
            .impact_hull(
                Vec3::new(0., -20., 0.),
                Vec3::X,
                75. - 1.732 * 16.,
                Vec3::splat(16.)
            )
            .is_none());
    }
    #[test]
    fn touching_floor_allows_standing_but_penetration_does_not() {
        let mut p = Physics::default();
        p.colliders
            .insert(ColliderBuilder::cuboid(10., 10., 1.).translation(vector![0., 0., -1.]));
        p.query.update(&p.colliders);
        let mins = Vec3::new(-16., -16., 0.);
        let maxs = Vec3::new(16., 16., 72.);
        assert!(!p.trace_hull(Vec3::ZERO, Vec3::ZERO, mins, maxs).start_solid);
        let below = -Vec3::Z;
        assert!(p.trace_hull(below, below, mins, maxs).start_solid);
    }
    #[test]
    fn killed_dynamic_entity_no_longer_blocks_raycasts() {
        let mut p = Physics::default();
        let body = p.bodies.insert(RigidBodyBuilder::dynamic());
        let c = p.colliders.insert_with_parent(
            ColliderBuilder::ball(1.).user_data(1),
            body,
            &mut p.bodies,
        );
        p.dynamic.insert(0, body);
        p.entity_colliders.insert(0, vec![c]);
        p.tick(0.015);
        assert!(p.ray(Vec3::new(-100., 0., 0.), Vec3::X, 200.).is_some());
        p.set_entity(0, Vec3::ZERO, Quat::IDENTITY, false);
        p.tick(0.015);
        assert!(p.ray(Vec3::new(-100., 0., 0.), Vec3::X, 200.).is_none());
    }
}
