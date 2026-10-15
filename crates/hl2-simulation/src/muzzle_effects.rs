//! Muzzle flashes (AE_MUZZLEFLASH / AE_NPC_MUZZLEFLASH -> C_BaseAnimating::
//! DispatchMuzzleEffect -> CTempEnts::MuzzleFlash_*) and bullet tracers (FireBullets
//! m_iTracerFreq -> UTIL_Tracer "Tracer" -> FX_PlayerTracer / FX_Tracer), SDK 2013
//! client c_te_legacytempents.cpp, fx.cpp, fx_tracer.cpp, clientsideeffects_test.cpp.
//! Particles are spawned in their attachment's frame at the event and keep that frame
//! (the SDK's local-space emitters follow the attachment for their 0.0001-0.2 s life);
//! muzzle dynamic lights (muzzleflash_light elights) and tracer whiz sounds are not
//! modeled.
use crate::projectile_visuals::Quad;
use glam::{Mat4, Vec3};

/// The weapon-type token of a muzzle event's options ("PISTOL muzzle").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuzzleKind {
    Combine,
    Smg1,
    Pistol,
    Shotgun,
    Revolver,
    Rpg,
}
/// DispatchMuzzleEffect: "<type> <attachment>".
pub fn parse(options: &str) -> Option<(MuzzleKind, String)> {
    let mut tokens = options.split_whitespace();
    let kind = match tokens.next()?.to_ascii_uppercase().as_str() {
        "COMBINE" => MuzzleKind::Combine,
        "SMG1" => MuzzleKind::Smg1,
        "PISTOL" => MuzzleKind::Pistol,
        "SHOTGUN" => MuzzleKind::Shotgun,
        "357" => MuzzleKind::Revolver,
        "RPG" => MuzzleKind::Rpg,
        _ => return None,
    };
    Some((kind, tokens.next()?.to_owned()))
}

/// One SimpleParticle: position/velocity in the emitter frame (x forward).
#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub material: &'static str,
    pub offset: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub size: (f32, f32),
    pub color: [u8; 3],
    pub alpha: (f32, f32),
    pub roll: f32,
    pub roll_delta: f32,
}
const PLAYER_FLASH: [&str; 4] = [
    "effects/muzzleflash1_noz",
    "effects/muzzleflash2_noz",
    "effects/muzzleflash3_noz",
    "effects/muzzleflash4_noz",
];
const NPC_FLASH: [&str; 4] = [
    "effects/muzzleflash1",
    "effects/muzzleflash2",
    "effects/muzzleflash3",
    "effects/muzzleflash4",
];
const COMBINE_PLAYER: [&str; 2] = ["effects/combinemuzzle1_noz", "effects/combinemuzzle2_noz"];
const COMBINE_NPC: [&str; 2] = ["effects/combinemuzzle1", "effects/combinemuzzle2"];
const DUST: &str = "particle/particle_smokegrenade";
pub const TRACER_MATERIAL: &str = "effects/spark";
/// C_WeaponStunStick::DrawModel halo material.
pub const STUNSTICK_GLOW: &str = "effects/stunstick";
/// Every sprite material these effects draw (preloaded with the projectile sprites).
pub const MATERIALS: &[&str] = &[
    STUNSTICK_GLOW,
    "effects/muzzleflash1_noz",
    "effects/muzzleflash2_noz",
    "effects/muzzleflash3_noz",
    "effects/muzzleflash4_noz",
    "effects/muzzleflash1",
    "effects/muzzleflash2",
    "effects/muzzleflash3",
    "effects/muzzleflash4",
    "effects/combinemuzzle1_noz",
    "effects/combinemuzzle2_noz",
    "effects/combinemuzzle1",
    "effects/combinemuzzle2",
];

/// A [0, 1) source.
pub trait Random {
    fn unit(&mut self) -> f32;
    fn float(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
    fn int(&mut self, low: i32, high: i32) -> i32 {
        (low + ((high - low + 1) as f32 * self.unit()) as i32).min(high)
    }
}
impl<F: FnMut() -> f32> Random for F {
    fn unit(&mut self) -> f32 {
        self().clamp(0., 0.999_999)
    }
}

fn flash(
    material: &'static str,
    x: f32,
    life: f32,
    size: f32,
    color: [u8; 3],
    end_alpha: f32,
    roll: f32,
) -> Particle {
    Particle {
        material,
        offset: Vec3::X * x,
        velocity: Vec3::ZERO,
        life,
        size: (size, size),
        color,
        alpha: (255., end_alpha),
        roll: roll.to_radians(),
        roll_delta: 0.,
    }
}
/// MuzzleFlash_*_Player / _NPC particles for one event.
pub fn particles(kind: MuzzleKind, first_person: bool, r: &mut impl Random) -> Vec<Particle> {
    let mut out = Vec::new();
    let warm = |r: &mut dyn FnMut() -> i32| [255, 255, (200 + r()) as u8];
    match (kind, first_person) {
        // MuzzleFlash_Pistol_Player / _357_Player: a dust puff, then five flashes
        // (pistol life 0.01, scale 1-1.25, step 4; 357 0.01, 1.25-1.5, step 8).
        (MuzzleKind::Pistol | MuzzleKind::Revolver, true) => {
            let pistol = kind == MuzzleKind::Pistol;
            if !pistol || r.int(0, 3) != 0 {
                let color = r.int(200, 255) as u8;
                let start = r.int(2, 4) as f32;
                let (life, speed) = if pistol {
                    (r.float(0.25, 0.5), r.float(48., 64.))
                } else {
                    (r.float(0.5, 1.), r.float(8., 64.))
                };
                let alpha = r.int(64, 128) as f32;
                let up = r.float(4., 16.);
                let roll = r.int(0, 360) as f32;
                let roll_delta = if pistol {
                    r.float(-0.1, 0.1)
                } else {
                    r.float(-0.5, 0.5)
                };
                out.push(Particle {
                    material: DUST,
                    offset: Vec3::X * 8.,
                    // forward * speed + world up (kept in the frame: up is +Z there).
                    velocity: Vec3::new(speed, 0., up),
                    life,
                    size: (start, start * if pistol { 4. } else { 8. }),
                    color: [color; 3],
                    alpha: (alpha, 0.),
                    roll: roll.to_radians(),
                    roll_delta,
                });
            }
            let (scale, step) = if pistol {
                (r.float(1., 1.25), 4.)
            } else {
                (r.float(1.25, 1.5), 8.)
            };
            for i in 1..6 {
                let material = PLAYER_FLASH[r.int(0, 3) as usize];
                let color = warm(&mut || r.int(0, 55));
                let size = r.float(6., 8.) * (8 - i) as f32 / 6. * scale;
                let roll = r.int(0, 360) as f32;
                out.push(flash(
                    material,
                    i as f32 * step * scale,
                    0.01,
                    size,
                    color,
                    255.,
                    roll,
                ));
            }
        }
        // MuzzleFlash_SMG1_Player (0.025 s) / _Shotgun_Player (one frame): five flashes.
        (MuzzleKind::Smg1 | MuzzleKind::Shotgun, true) => {
            let scale = r.float(1.25, 1.5);
            let life = if kind == MuzzleKind::Smg1 {
                0.025
            } else {
                0.0001
            };
            for i in 1..6 {
                let material = PLAYER_FLASH[r.int(0, 3) as usize];
                let color = warm(&mut || r.int(0, 55));
                let size = r.float(6., 8.) * (8 - i) as f32 / 6. * scale;
                let roll = r.int(0, 360) as f32;
                out.push(flash(
                    material,
                    i as f32 * 8. * scale,
                    life,
                    size,
                    color,
                    255.,
                    roll,
                ));
            }
        }
        // MuzzleFlash_Combine_Player: five flashes and a dimmer core at the muzzle.
        (MuzzleKind::Combine, true) => {
            let scale = r.float(2., 2.25);
            for i in 1..6 {
                let material = COMBINE_PLAYER[r.int(0, 1) as usize];
                let color = warm(&mut || r.int(0, 55));
                let size = r.float(6., 8.) * (12 - i) as f32 / 12. * scale;
                let roll = r.int(0, 360) as f32;
                out.push(flash(
                    material,
                    i as f32 * 8. * scale,
                    0.025,
                    size,
                    color,
                    255.,
                    roll,
                ));
            }
            let material = COMBINE_PLAYER[r.int(0, 1) as usize];
            let alpha = r.int(64, 128) as f32;
            let size = r.float(10., 16.);
            let roll = r.int(0, 360) as f32;
            let mut core = flash(material, 0., 0.025, size, [255; 3], 32., roll);
            core.alpha = (alpha, 32.);
            out.push(core);
        }
        // FX_MuzzleEffectAttached: pistol 0.5 / SMG1 1.0 (one frame), shotgun 0.75 (0.1 s).
        (
            MuzzleKind::Pistol | MuzzleKind::Smg1 | MuzzleKind::Shotgun | MuzzleKind::Revolver,
            false,
        ) => {
            let (scale, one_frame) = match kind {
                MuzzleKind::Pistol | MuzzleKind::Revolver => (0.5, true),
                MuzzleKind::Smg1 => (1., true),
                _ => (0.75, false),
            };
            let flash_scale = r.float(scale - 0.25, scale + 0.25).clamp(0.5, 8.);
            for i in 1..9 {
                let material = NPC_FLASH[r.int(0, 3) as usize];
                let size = r.float(6., 9.) * (12 - i) as f32 / 9. * flash_scale;
                let roll = r.int(0, 360) as f32;
                let life = if one_frame { 0.0001 } else { 0.1 };
                out.push(flash(
                    material,
                    i as f32 * 2. * scale,
                    life,
                    size,
                    [255; 3],
                    128.,
                    roll,
                ));
            }
        }
        // MuzzleFlash_Combine_NPC front burst (the side burst and embers are not modeled).
        (MuzzleKind::Combine, false) => {
            let scale = r.float(1., 1.5);
            let burst = r.float(50., 150.);
            for i in 1..6 {
                let material = COMBINE_NPC[r.int(0, 1) as usize];
                let size = r.float(6., 8.) * (6. * 1.25 - i as f32) / 6. * scale;
                let roll = r.int(0, 360) as f32;
                let mut p = flash(
                    material,
                    i as f32 * 2. * scale,
                    0.1,
                    size,
                    [255; 3],
                    0.,
                    roll,
                );
                p.velocity = Vec3::X * burst;
                out.push(p);
            }
        }
        // The RPG has no first-person flash; its NPC flash is not modeled.
        (MuzzleKind::Rpg, _) => {}
    }
    out
}

/// A spawned emitter: its frame (attachment frame, in world space or the viewmodel's
/// model space) and particles.
#[derive(Clone, Debug)]
pub struct Emitter {
    pub frame: Mat4,
    pub born: f64,
    pub particles: Vec<Particle>,
    pub first_person: bool,
}
/// A bullet tracer line. Player tracers (FX_PlayerTracer) are static segments for
/// 0.01 s; others (FX_Tracer) are FX_AddDiscreetLine lines moving at `speed`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tracer {
    pub start: Vec3,
    pub direction: Vec3,
    pub born: f64,
    pub speed: f32,
    pub length: f32,
    pub clip: f32,
    pub width: f32,
    pub life: f32,
}
/// TRACER_SPEED.
const TRACER_SPEED: f32 = 5000.;
/// FX_PlayerTracer from TracerCallback's start (data.m_vStart + right * 4, z - 0.5): none
/// under 256 units; a segment from 8 + U(-24, 64) along the shot, U(0.1, 0.6) of its
/// length, width U(0.5, 0.75), 0.01 s.
pub fn player_tracer(
    start: Vec3,
    end: Vec3,
    right: Vec3,
    now: f64,
    r: &mut impl Random,
) -> Option<Tracer> {
    let start = start + right * 4. - Vec3::Z * 0.5;
    let delta = end - start;
    let length = delta.length();
    if length < 256. {
        return None;
    }
    let direction = delta / length;
    let from = start + direction * (8. + r.float(-24., 64.));
    let segment = length * r.float(0.1, 0.6);
    Some(Tracer {
        start: from,
        direction,
        born: now,
        speed: 0.,
        length: segment,
        clip: 0.,
        width: r.float(0.5, 0.75),
        life: 0.01,
    })
}
/// FX_Tracer: none under 256 units; a 64-128 unit line moving at TRACER_SPEED, clipped
/// to the end, width U(0.75, 0.9), alive until its tail passes the end.
pub fn npc_tracer(start: Vec3, end: Vec3, now: f64, r: &mut impl Random) -> Option<Tracer> {
    let delta = end - start;
    let distance = delta.length();
    if distance < 256. {
        return None;
    }
    let length = r.float(64., 128.);
    Some(Tracer {
        start,
        direction: delta / distance,
        born: now,
        speed: TRACER_SPEED,
        length,
        clip: distance,
        width: r.float(0.75, 0.9),
        life: (distance + length) / TRACER_SPEED,
    })
}
impl Tracer {
    /// The visible segment at `now` (CFXDiscreetLine::Draw / CFXStaticLine).
    pub fn segment(&self, now: f64) -> Option<(Vec3, Vec3)> {
        let age = (now - self.born) as f32;
        if age < 0. || age > self.life.max(0.016) {
            return None;
        }
        if self.speed == 0. {
            return Some((self.start, self.start + self.direction * self.length));
        }
        let head = (self.speed * age).min(self.clip);
        let tail = (self.speed * age - self.length).clamp(0., self.clip);
        (head > tail).then(|| {
            (
                self.start + self.direction * tail,
                self.start + self.direction * head,
            )
        })
    }
}

/// Live muzzle emitters and tracers.
#[derive(Clone, Debug, Default)]
pub struct MuzzleEffects {
    pub emitters: Vec<Emitter>,
    pub tracers: Vec<Tracer>,
    pub flashes: u64,
    pub tracers_made: u64,
}
impl MuzzleEffects {
    pub fn flash(
        &mut self,
        kind: MuzzleKind,
        first_person: bool,
        frame: Mat4,
        now: f64,
        r: &mut impl Random,
    ) {
        let particles = particles(kind, first_person, r);
        if !particles.is_empty() {
            self.flashes += 1;
            self.emitters.push(Emitter {
                frame,
                born: now,
                particles,
                first_person,
            });
        }
    }
    pub fn tracer(&mut self, tracer: Option<Tracer>) {
        if let Some(t) = tracer {
            self.tracers_made += 1;
            self.tracers.push(t);
        }
    }
    /// Drop finished effects; a particle stays at least one drawn frame.
    pub fn retain(&mut self, now: f64) {
        self.emitters.retain(|e| {
            let age = (now - e.born) as f32;
            e.particles.iter().any(|p| age <= p.life.max(0.016))
        });
        self.tracers
            .retain(|t| ((now - t.born) as f32) <= t.life.max(0.016));
    }
    /// Camera-facing particle quads for the emitters of one space (`first_person`):
    /// `right`/`up` are that space's view axes; `scale` multiplies sizes and offsets.
    pub fn particle_quads(
        &self,
        first_person: bool,
        now: f64,
        right: Vec3,
        up: Vec3,
        scale: f32,
    ) -> Vec<Quad> {
        let mut out = Vec::new();
        for e in self
            .emitters
            .iter()
            .filter(|e| e.first_person == first_person)
        {
            let age = (now - e.born) as f32;
            for p in &e.particles {
                if age > p.life.max(0.016) {
                    continue;
                }
                let t = (age / p.life.max(1e-4)).clamp(0., 1.);
                let local = p.offset + p.velocity * age;
                let center = e.frame.transform_point3(local * scale);
                let size = (p.size.0 + (p.size.1 - p.size.0) * t) * scale;
                let alpha = (p.alpha.0 + (p.alpha.1 - p.alpha.0) * t).clamp(0., 255.) / 255.;
                out.push(billboard(
                    p.material,
                    center,
                    right,
                    up,
                    size,
                    p.roll + p.roll_delta * age,
                    p.color,
                    alpha,
                ));
            }
        }
        out
    }
    /// Tracer line quads facing `eye`.
    pub fn tracer_quads(&self, now: f64, eye: Vec3) -> Vec<Quad> {
        self.tracers
            .iter()
            .filter_map(|t| {
                let (a, b) = t.segment(now)?;
                let cross = (b - a).cross(b - eye).normalize_or_zero() * t.width;
                (cross != Vec3::ZERO).then(|| Quad {
                    material: TRACER_MATERIAL.into(),
                    positions: [a - cross, a + cross, b + cross, b - cross],
                    color: [255, 255, 255, 255],
                    uv: [[1., 0.], [0., 0.], [0., 1.], [1., 1.]],
                })
            })
            .collect()
    }
}
/// C_WeaponStunStick::DrawModel while active: two DrawHalo sprites 1 unit behind
/// attachment 1 (radius 4-6 at grey 0.1-0.2, then 2-3 at 0.9-1.0), redrawn with new random
/// sizes every frame. The 1-in-6 TE_BEAMTESLA arcs of ClientThink are not modeled.
pub fn stunstick_glow(attachment: Mat4, right: Vec3, up: Vec3, r: &mut impl Random) -> [Quad; 2] {
    let center = attachment.transform_point3(Vec3::NEG_X);
    let grey = |v: f32| [(v * 255.).round() as u8; 3];
    let (outer, outer_size) = (grey(r.float(0.1, 0.2)), r.float(4., 6.));
    let (inner, inner_size) = (grey(r.float(0.9, 1.)), r.float(2., 3.));
    [
        billboard(STUNSTICK_GLOW, center, right, up, outer_size, 0., outer, 1.),
        billboard(STUNSTICK_GLOW, center, right, up, inner_size, 0., inner, 1.),
    ]
}
/// An additive sprite quad (rgb x alpha) or, for the dust puff, a blended one.
#[allow(clippy::too_many_arguments)]
fn billboard(
    material: &str,
    center: Vec3,
    right: Vec3,
    up: Vec3,
    size: f32,
    roll: f32,
    color: [u8; 3],
    alpha: f32,
) -> Quad {
    let (sin, cos) = roll.sin_cos();
    let r = (right * cos + up * sin) * size;
    let u = (right * -sin + up * cos) * size;
    let blended = material == DUST;
    let c = |v: u8| {
        if blended {
            v
        } else {
            (f32::from(v) * alpha).round() as u8
        }
    };
    Quad {
        material: material.into(),
        positions: [
            center - r - u,
            center - r + u,
            center + r + u,
            center + r - u,
        ],
        color: [
            c(color[0]),
            c(color[1]),
            c(color[2]),
            if blended {
                (alpha * 255.).round() as u8
            } else {
                255
            },
        ],
        uv: crate::explosion_particles::UV,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_kinds_and_sdk_particle_sets() {
        assert_eq!(
            parse("PISTOL muzzle"),
            Some((MuzzleKind::Pistol, "muzzle".into()))
        );
        assert_eq!(parse("357 muzzle").unwrap().0, MuzzleKind::Revolver);
        assert_eq!(parse("COMBINE MUZZLE").unwrap().0, MuzzleKind::Combine);
        assert!(parse("FOO muzzle").is_none());
        let mut half = || 0.5;
        // NPC pistol: FX_MuzzleEffectAttached(0.5): eight one-frame flashes 1 unit apart.
        let npc = particles(MuzzleKind::Pistol, false, &mut half);
        assert_eq!(npc.len(), 8);
        assert!((npc[1].offset.x - 2.).abs() < 1e-4);
        assert_eq!(npc[0].life, 0.0001);
        // Player SMG1: five 0.025 s flashes; player shotgun: one frame.
        assert_eq!(particles(MuzzleKind::Smg1, true, &mut half).len(), 5);
        assert!(particles(MuzzleKind::Shotgun, true, &mut half)
            .iter()
            .all(|p| p.life == 0.0001));
        // Player 357: always the dust puff plus five flashes.
        let revolver = particles(MuzzleKind::Revolver, true, &mut half);
        assert_eq!(revolver.len(), 6);
        assert_eq!(revolver[0].material, DUST);
        assert!(particles(MuzzleKind::Rpg, true, &mut half).is_empty());
    }
    #[test]
    fn tracers_skip_short_shots_and_npc_lines_travel_at_tracer_speed() {
        let mut half = || 0.5;
        assert!(npc_tracer(Vec3::ZERO, Vec3::X * 200., 0., &mut half).is_none());
        let t = npc_tracer(Vec3::ZERO, Vec3::X * 1000., 0., &mut half).unwrap();
        assert_eq!(t.length, 96.);
        // After 0.1 s the head is at 500 and the tail 96 behind it.
        let (a, b) = t.segment(0.1).unwrap();
        assert!((b.x - 500.).abs() < 1e-3 && (a.x - 404.).abs() < 1e-3);
        // Clipped at the end; gone once the tail passes it.
        let (_, b) = t.segment(0.2).unwrap();
        assert!((b.x - 1000.).abs() < 1e-3);
        assert!(t.segment(0.3).is_none());
        let p = player_tracer(Vec3::ZERO, Vec3::X * 1000., Vec3::NEG_Y, 0., &mut half).unwrap();
        // Start: right * 4 (y -4), z - 0.5, then 8 + U(-24, 64) = 28 along the shot.
        assert!(
            (p.start.x - 28.).abs() < 0.1 && (p.start.y + 4.).abs() < 0.2,
            "{p:?}"
        );
        assert!(player_tracer(Vec3::ZERO, Vec3::X * 100., Vec3::NEG_Y, 0., &mut half).is_none());
    }
}
