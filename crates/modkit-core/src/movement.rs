//! Fixed-tick, Z-up movement. SDK-derived rules; surface modifiers/water/ladders remain separate work.
use crate::{Trace, World};
use glam::Vec3;
use serde::Serialize;
pub const TICK: f32 = 0.015;
pub trait CollisionWorld {
    fn trace_hull(&self, start: Vec3, end: Vec3, mins: Vec3, maxs: Vec3) -> Trace;
}
impl CollisionWorld for World {
    fn trace_hull(&self, start: Vec3, end: Vec3, mins: Vec3, maxs: Vec3) -> Trace {
        self.trace(start, end, mins, maxs)
    }
}
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub forward: f32,
    pub side: f32,
    pub yaw: f32,
    pub jump: bool,
    pub crouch: bool,
    pub sprint: bool,
    pub slow: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub feet: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub crouched: bool,
    pub ticks: u64,
    pub eye_height: f32,
    jump_held: bool,
    surface_friction: f32,
}
impl Player {
    pub fn new(eye: Vec3) -> Self {
        Self {
            feet: eye - Vec3::Z * 64.,
            velocity: Vec3::ZERO,
            grounded: false,
            crouched: false,
            ticks: 0,
            eye_height: 64.,
            jump_held: false,
            surface_friction: 1.,
        }
    }
    pub fn eye(&self) -> Vec3 {
        self.feet + Vec3::Z * self.eye_height
    }
    pub fn step(&mut self, input: Input, world: &impl CollisionWorld, dt: f32) {
        self.ticks += 1;
        let mins = Vec3::new(-16., -16., 0.);
        let standing = Vec3::new(16., 16., 72.);
        if input.crouch {
            self.crouched = true;
        } else if self.crouched
            && !world
                .trace_hull(self.feet, self.feet, mins, standing)
                .start_solid
        {
            self.crouched = false;
        }
        let maxs = Vec3::new(16., 16., if self.crouched { 36. } else { 72. });
        let target_eye = if self.crouched { 28. } else { 64. };
        self.eye_height += (target_eye - self.eye_height).clamp(-dt * 180., dt * 180.);
        let ground = world.trace_hull(self.feet, self.feet - Vec3::Z * 2., mins, maxs);
        self.grounded = ground.fraction < 1. && ground.normal.z >= 0.7 && self.velocity.z <= 140.;
        if self.grounded {
            self.velocity.z = 0.;
        }
        // Source starts the first gravity half-step before checking for a jump,
        // including while grounded. Ground walking clears it again below.
        self.velocity.z -= 300. * dt;
        let forward = Vec3::new(input.yaw.cos(), input.yaw.sin(), 0.);
        let right = Vec3::new(input.yaw.sin(), -input.yaw.cos(), 0.);
        // CheckParameters crops the command components before CheckJumpButton
        // uses forward movement for its boost, rather than only capping wishspeed.
        let input_length = input.forward.hypot(input.side).max(1.);
        let forward_move = input.forward / input_length;
        let side_move = input.side / input_length;
        let wish = forward * forward_move + right * side_move;
        let speed = if self.crouched {
            190. / 3.
        } else if input.sprint {
            320.
        } else if input.slow {
            150.
        } else {
            190.
        };
        let wishspeed = wish.length().min(1.) * speed;
        let wishdir = wish.normalize_or_zero();
        let jumping = input.jump && !self.jump_held && self.grounded;
        self.jump_held = input.jump;
        if jumping {
            self.grounded = false;
            if self.crouched {
                self.velocity.z = 160.;
            } else {
                self.velocity.z += 160.;
            }
            let boost = if input.sprint || self.crouched {
                0.1
            } else {
                0.5
            };
            let addition = (forward_move.abs() * speed * boost)
                .min(speed * (1. + boost) - self.velocity.truncate().length());
            // Retail permits a negative addition when already above the boost
            // limit. Its sign only flips for a negative forward command.
            self.velocity += forward
                * if forward_move < 0. {
                    -addition
                } else {
                    addition
                };
            self.velocity.z -= 300. * dt;
        }
        if self.grounded {
            self.velocity.z = 0.;
            let planar = Vec3::new(self.velocity.x, self.velocity.y, 0.);
            let magnitude = planar.length();
            if magnitude > 0. {
                let retained = (magnitude - magnitude.max(100.) * 4. * dt).max(0.) / magnitude;
                self.velocity.x *= retained;
                self.velocity.y *= retained;
            }
        }
        let cap = if self.grounded {
            wishspeed
        } else {
            wishspeed.min(30.)
        };
        let add = cap - self.velocity.dot(wishdir);
        if add > 0. {
            let friction = if self.grounded {
                1.
            } else {
                self.surface_friction
            };
            self.velocity += wishdir * (10. * wishspeed * dt * friction).min(add);
        }
        let before = self.feet;
        let old_velocity = self.velocity;
        let (flat, flat_velocity) = slide(world, before, old_velocity, dt, mins, maxs);
        self.feet = flat;
        self.velocity = flat_velocity;
        if self.grounded
            && (flat - before).truncate().length_squared() + 0.01
                < (old_velocity * dt).truncate().length_squared()
        {
            let up = world.trace_hull(before, before + Vec3::Z * 18.03125, mins, maxs);
            if !up.start_solid {
                let (raised, _) = slide(
                    world,
                    before + Vec3::Z * 18.03125 * up.fraction,
                    old_velocity,
                    dt,
                    mins,
                    maxs,
                );
                let down = world.trace_hull(raised, raised - Vec3::Z * 18.03125, mins, maxs);
                if !down.start_solid && down.normal.z >= 0.7 {
                    let stepped = raised - Vec3::Z * 18.03125 * down.fraction;
                    if (stepped - before).truncate().length_squared()
                        > (flat - before).truncate().length_squared()
                    {
                        self.feet = stepped;
                        self.velocity = old_velocity;
                        self.velocity.z = flat_velocity.z;
                    }
                }
            }
        }
        let ground = world.trace_hull(self.feet, self.feet - Vec3::Z * 2., mins, maxs);
        self.grounded = self.velocity.z <= 140. && ground.fraction < 1. && ground.normal.z >= 0.7;
        // CategorizePosition resets friction first. Its rapid-ascent return
        // precedes the failed-ground test which assigns quarter friction.
        // Optimized WALK carries this result into the next tick's AirAccelerate.
        self.surface_friction = if self.velocity.z <= 140. && self.velocity.z > 0. && !self.grounded
        {
            0.25
        } else {
            1.
        };
        // FullWalkMove categorizes the swept position before its final half-step.
        // In particular, walking off a ledge must not retain the old ground flag.
        self.velocity.z -= 300. * dt;
        if self.grounded {
            self.velocity.z = 0.;
            self.feet -= Vec3::Z * 2. * ground.fraction;
        }
    }
}
fn slide(
    world: &impl CollisionWorld,
    mut position: Vec3,
    mut velocity: Vec3,
    dt: f32,
    mins: Vec3,
    maxs: Vec3,
) -> (Vec3, Vec3) {
    let mut remaining = dt;
    let original = velocity;
    let mut planes = Vec::<Vec3>::new();
    for _ in 0..4 {
        let hit = world.trace_hull(position, position + velocity * remaining, mins, maxs);
        if hit.start_solid {
            return (position, Vec3::ZERO);
        }
        position += velocity * remaining * hit.fraction;
        if hit.fraction >= 1. {
            break;
        }
        remaining *= 1. - hit.fraction;
        planes.push(hit.normal);
        let incoming = velocity;
        let mut candidate = Vec3::ZERO;
        let mut found = false;
        for &plane in &planes {
            let clipped = incoming - plane * incoming.dot(plane);
            if planes.iter().all(|n| clipped.dot(*n) >= -0.01) {
                candidate = clipped;
                found = true;
                break;
            }
        }
        if !found && planes.len() == 2 {
            let axis = planes[0].cross(planes[1]).normalize_or_zero();
            candidate = axis * incoming.dot(axis);
        }
        velocity = candidate;
        if velocity.dot(original) <= 0. {
            velocity = Vec3::ZERO;
            break;
        }
    }
    (position, velocity)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Brush, Plane};
    fn floor() -> World {
        World {
            brushes: vec![Brush {
                contents: 1,
                planes: vec![
                    Plane {
                        normal: Vec3::Z,
                        distance: 0.,
                    },
                    Plane {
                        normal: -Vec3::Z,
                        distance: 1000.,
                    },
                ],
            }],
            ..Default::default()
        }
    }
    #[test]
    fn ground_acceleration_friction_and_speed_limit() {
        let w = floor();
        let mut p = Player::new(Vec3::Z * 64.05);
        for _ in 0..100 {
            p.step(
                Input {
                    forward: 1.,
                    ..Default::default()
                },
                &w,
                TICK,
            );
        }
        assert!((p.velocity.x - 190.).abs() < 0.01);
        assert!(p.grounded);
        for _ in 0..100 {
            p.step(Input::default(), &w, TICK);
        }
        assert!(p.velocity.length() < 0.01);
    }
    #[test]
    fn jump_boost_crops_diagonal_forward_command() {
        let w = floor();
        let mut p = Player::new(Vec3::Z * 64.05);
        p.step(Input::default(), &w, TICK);
        p.step(
            Input {
                forward: 1.,
                side: 1.,
                jump: true,
                ..Default::default()
            },
            &w,
            TICK,
        );
        // The cropped forward command is190/sqrt(2); its half-speed boost
        // already exceeds the30-unit wish-direction air cap, so no air gain.
        assert!((p.velocity.x - 190. / 2f32.sqrt() * 0.5).abs() < 0.0001);
        assert!(p.velocity.y.abs() < 0.0001);
    }
    #[test]
    fn jump_boost_keeps_negative_cap_addition() {
        let w = floor();
        for forward in [0., 1.] {
            let mut p = Player::new(Vec3::Z * 64.05);
            p.step(Input::default(), &w, TICK);
            p.velocity.x = 400.;
            p.step(
                Input {
                    forward,
                    jump: true,
                    ..Default::default()
                },
                &w,
                TICK,
            );
            // Native caps to190*1.5 even with no forward input: the signed
            // addition can be negative and zero input does not erase it.
            assert!((p.velocity.x - 285.).abs() < 0.0001);
        }
    }
    #[test]
    fn air_friction_respects_rapid_ascent_return_and_next_tick_order() {
        let w = World::default();
        for (initial_z_velocity, expected_gain) in [(144.5, 7.125), (144.6, 28.5)] {
            let mut p = Player::new(Vec3::Z * 1064.);
            p.velocity.z = initial_z_velocity;
            p.step(Input::default(), &w, TICK);
            p.step(
                Input {
                    forward: 1.,
                    ..Default::default()
                },
                &w,
                TICK,
            );
            // At categorization before final gravity, vz140 assigns0.25;
            // vz140.1 takes the rapid-ascent return and retains the reset1.
            assert!((p.velocity.x - expected_gain).abs() < 0.0001);
        }
    }
    #[test]
    fn air_friction_from_ascent_survives_the_first_descending_acceleration() {
        let w = World::default();
        let mut p = Player::new(Vec3::Z * 1064.);
        p.velocity.z = 10.;
        p.step(Input::default(), &w, TICK);
        assert!((p.velocity.z - 1.).abs() < 0.0001);
        let input = Input {
            forward: 1.,
            ..Default::default()
        };
        p.step(input, &w, TICK);
        assert!(p.velocity.z < 0.);
        assert!((p.velocity.x - 7.125).abs() < 0.0001);
        p.velocity.x = 0.;
        p.step(input, &w, TICK);
        assert!((p.velocity.x - 28.5).abs() < 0.0001);
    }
    #[test]
    fn standing_jump_first_tick_preserves_start_gravity() {
        let w = floor();
        let mut p = Player::new(Vec3::Z * 64.05);
        p.step(Input::default(), &w, TICK);
        assert!(p.grounded);
        let start = p.feet.z;
        p.step(
            Input {
                jump: true,
                ..Default::default()
            },
            &w,
            TICK,
        );
        // Retail FullWalkMove: -4.5, +160, -4.5, sweep, -4.5.
        assert!((p.feet.z - start - 2.265).abs() < 0.0001);
        assert!((p.velocity.z - 146.5).abs() < 0.0001);
        assert!(!p.grounded);
    }
    #[test]
    fn crouched_jump_first_tick_replaces_start_gravity() {
        let w = floor();
        let mut p = Player::new(Vec3::Z * 64.05);
        p.step(
            Input {
                crouch: true,
                ..Default::default()
            },
            &w,
            TICK,
        );
        assert!(p.grounded && p.crouched);
        let start = p.feet.z;
        p.step(
            Input {
                jump: true,
                crouch: true,
                ..Default::default()
            },
            &w,
            TICK,
        );
        // The ducked branch assigns 160, then applies the two remaining half-steps.
        assert!((p.feet.z - start - 2.3325).abs() < 0.0001);
        assert!((p.velocity.z - 151.).abs() < 0.0001);
        assert!(!p.grounded);
    }
    #[test]
    fn leaving_a_ledge_categorizes_before_finish_gravity() {
        let mut w = floor();
        w.brushes[0].planes.push(Plane {
            normal: Vec3::X,
            distance: 0.,
        });
        let mut p = Player::new(Vec3::new(15.5, 0., 64.05));
        p.step(Input::default(), &w, TICK);
        assert!(p.grounded);
        let height = p.feet.z;
        p.velocity.x = 190.;
        p.step(Input::default(), &w, TICK);
        assert!(p.feet.x > 16.);
        assert!(!p.grounded);
        assert!((p.feet.z - height).abs() < 0.0001);
        assert!((p.velocity.z + 4.5).abs() < 0.0001);
    }
    #[test]
    fn hl2_jump_apex_and_release_gate() {
        let w = floor();
        let mut p = Player::new(Vec3::Z * 64.05);
        p.step(Input::default(), &w, TICK);
        let start = p.feet.z;
        let mut top = 0f32;
        for _ in 0..100 {
            p.step(
                Input {
                    jump: true,
                    ..Default::default()
                },
                &w,
                TICK,
            );
            top = top.max(p.feet.z);
        }
        // At this fixed tick, upward sweep velocities are151,142,...,7.
        assert!((top - start - 20.145).abs() < 0.001, "apex {top}");
        assert!(p.grounded);
        assert!(p.feet.z < 0.1);
        p.step(Input::default(), &w, TICK);
        p.step(
            Input {
                jump: true,
                ..Default::default()
            },
            &w,
            TICK,
        );
        assert!(!p.grounded);
        assert!((p.velocity.z - 146.5).abs() < 0.0001);
    }
    #[test]
    fn fixed_input_replay_is_identical() {
        let w = floor();
        let mut a = Player::new(Vec3::Z * 64.05);
        let mut b = a.clone();
        for i in 0..200 {
            let input = Input {
                forward: 1.,
                side: 0.2,
                jump: i == 70,
                ..Default::default()
            };
            a.step(input, &w, TICK);
            b.step(input, &w, TICK);
        }
        assert_eq!(a.feet, b.feet);
        assert_eq!(a.velocity, b.velocity);
    }
}
