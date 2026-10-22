//! A particle system runtime for installed PCF definitions (`source_assets::pcf`), first
//! for C_EntityFlame's "burning_character" (particles/burning_fx.pcf) on burning props.
//! The operator code of Source's particle library is not in the SDK (closed
//! particles.lib), so each supported operator follows its PCF parameters' documented
//! meaning and the SDK's particle attributes (public/particles/particles.h: 3 radius,
//! 4 rotation, 7 alpha, 8 creation time, ...); retail behavior was not decompiled and
//! nothing here is native-verified. Movement Lock to Bone follows the model's center
//! (no bones yet). Unsupported operators are counted, not guessed: Movement Lock to
//! Control Point, Position Modify Warp Random, Lifetime
//! Pre-Age Noise, Sequence Two Random's second sequence, and the SpriteCard dual
//! sequence / frame blending. Noise operators draw uniform randoms (no Perlin field).
use crate::projectile_visuals::Quad;
use glam::Vec3;
use serde::Serialize;
use source_assets::pcf::{Operator, ParticleSystem};
use source_assets::vtf::SheetSequence;
use std::collections::BTreeMap;

/// The installed systems this runtime plays and where they come from.
pub const FILES: &[&str] = &["particles/burning_fx.pcf"];
/// C_EntityFlame's system.
pub const BURNING_CHARACTER: &str = "burning_character";

/// A material path from a PCF ("particle\\x\\y.vmt" -> "particle/x/y").
pub fn material_name(path: &str) -> String {
    let name = path.replace('\\', "/").to_ascii_lowercase();
    let name = name.strip_prefix("materials/").unwrap_or(&name);
    name.strip_suffix(".vmt").unwrap_or(name).to_owned()
}

/// Definitions by name and each material's sheet.
#[derive(Default)]
pub struct Library {
    pub systems: BTreeMap<String, ParticleSystem>,
    pub sheets: BTreeMap<String, Vec<SheetSequence>>,
    /// SpriteCard $OVERBRIGHTFACTOR per material.
    pub overbright: BTreeMap<String, f32>,
}
impl Library {
    pub fn overbright(&self, material: &str) -> f32 {
        self.overbright.get(material).copied().unwrap_or(1.)
    }
    /// Every material the systems draw with.
    pub fn materials(&self) -> Vec<String> {
        let mut m: Vec<String> = self
            .systems
            .values()
            .filter_map(|s| s.material.as_deref().map(material_name))
            .collect();
        m.sort();
        m.dedup();
        m
    }
}

#[derive(Clone, Debug)]
struct Particle {
    position: Vec3,
    previous: Vec3,
    created: f32,
    life: f32,
    radius: f32,
    initial_radius: f32,
    rotation: f32,
    color: Vec3,
    initial_color: Vec3,
    alpha: f32,
    initial_alpha: f32,
    sequence: usize,
    flip: bool,
    /// The particle's random-table index (per-particle randoms of operators).
    seed: u32,
}

/// One running system and its children.
#[derive(Clone, Debug)]
struct Instance {
    system: String,
    time: f32,
    delay: f32,
    emitted: f32,
    particles: Vec<Particle>,
    children: Vec<Instance>,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ControlPoints {
    /// CP0: the entity origin.
    pub origin: Vec3,
    /// CP1: the model (its world bounds for Position on Model Random).
    pub model_min: Vec3,
    pub model_max: Vec3,
}

#[derive(Clone, Debug)]
struct Effect {
    instance: Instance,
    points: ControlPoints,
    stopped: bool,
    /// How far the model moved since the last simulation step.
    moved: Vec3,
}

#[derive(Default, Serialize)]
pub struct Diagnostics {
    pub effects: usize,
    pub particles: usize,
    pub unsupported: BTreeMap<String, u32>,
}

/// The running effects, keyed by owner (an entity id).
#[derive(Default)]
pub struct Runtime {
    effects: BTreeMap<usize, Effect>,
    rng: u64,
    pub diagnostics: Diagnostics,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
/// The particle library's bias (retail C_OP_InterpolateRadius / C_OP_FadeOut: InitParams
/// stores 1/bias - 2, Operate divides x by (1 - x) * that + 1; bias 0 means 0.5).
fn bias(x: f32, amount: f32) -> f32 {
    let amount = if amount == 0. { 0.5 } else { amount };
    x / ((1. / amount - 2.) * (1. - x) + 1.)
}
/// A per-particle random in [0, 1) for one operator (the library's random table indexed
/// by the particle's seed plus the operator's offset).
fn particle_random(seed: u32, operator: usize) -> f32 {
    let mut x = seed ^ (operator as u32).wrapping_mul(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}
fn smooth(t: f32) -> f32 {
    t * t * (3. - 2. * t)
}

impl Runtime {
    fn random(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Start (or keep) the effect `system` on `owner`; control points follow `points`.
    pub fn start(&mut self, owner: usize, system: &str, points: ControlPoints, library: &Library) {
        if let Some(effect) = self.effects.get_mut(&owner) {
            let center = |p: &ControlPoints| (p.model_min + p.model_max) * 0.5;
            effect.moved += center(&points) - center(&effect.points);
            effect.points = points;
            effect.stopped = false;
            return;
        }
        if let Some(instance) = Self::instance(library, system, 0.) {
            self.effects.insert(
                owner,
                Effect {
                    instance,
                    points,
                    stopped: false,
                    moved: Vec3::ZERO,
                },
            );
        }
    }
    fn instance(library: &Library, system: &str, delay: f32) -> Option<Instance> {
        let def = library.systems.get(system)?;
        Some(Instance {
            system: system.to_owned(),
            time: 0.,
            delay,
            emitted: 0.,
            particles: Vec::new(),
            children: def
                .children
                .iter()
                .filter_map(|(child, delay)| Self::instance(library, child, *delay))
                .collect(),
        })
    }
    /// Stop emitting on `owner` (StopParticleEffects); live particles finish.
    pub fn stop(&mut self, owner: usize) {
        if let Some(effect) = self.effects.get_mut(&owner) {
            effect.stopped = true;
        }
    }
    pub fn active(&self, owner: usize) -> bool {
        self.effects.get(&owner).is_some_and(|e| !e.stopped)
    }
    pub fn owners(&self) -> Vec<usize> {
        self.effects.keys().copied().collect()
    }
    /// Advance every effect by `dt` seconds.
    pub fn simulate(&mut self, dt: f32, library: &Library) {
        let mut effects = std::mem::take(&mut self.effects);
        for effect in effects.values_mut() {
            let points = effect.points;
            let stopped = effect.stopped;
            let moved = std::mem::take(&mut effect.moved);
            self.simulate_instance(&mut effect.instance, &points, moved, stopped, dt, library);
        }
        effects.retain(|_, e| !(e.stopped && Self::empty(&e.instance)));
        self.diagnostics.effects = effects.len();
        self.diagnostics.particles = effects.values().map(|e| Self::count(&e.instance)).sum();
        self.effects = effects;
    }
    fn empty(instance: &Instance) -> bool {
        instance.particles.is_empty() && instance.children.iter().all(Self::empty)
    }
    fn count(instance: &Instance) -> usize {
        instance.particles.len() + instance.children.iter().map(Self::count).sum::<usize>()
    }
    fn unsupported(&mut self, name: &str) {
        *self
            .diagnostics
            .unsupported
            .entry(name.to_owned())
            .or_default() += 1;
    }
    fn simulate_instance(
        &mut self,
        instance: &mut Instance,
        points: &ControlPoints,
        moved: Vec3,
        stopped: bool,
        dt: f32,
        library: &Library,
    ) {
        for child in &mut instance.children {
            self.simulate_instance(child, points, moved, stopped, dt, library);
        }
        let Some(def) = library.systems.get(&instance.system) else {
            return;
        };
        instance.time += dt;
        let t = instance.time - instance.delay;
        if t < 0. {
            return;
        }
        // Operators on the live particles.
        let forces: Vec<Vec3> = def
            .forces
            .iter()
            .filter(|f| f.function == "random force")
            .map(|f| {
                let (min, max) = (
                    f.vector("min force").unwrap_or_default(),
                    f.vector("max force").unwrap_or_default(),
                );
                Vec3::new(
                    lerp(min.x, max.x, self.random()),
                    lerp(min.y, max.y, self.random()),
                    lerp(min.z, max.z, self.random()),
                )
            })
            .collect();
        let mut particles = std::mem::take(&mut instance.particles);
        for (op_index, op) in def.operators.iter().enumerate() {
            match op.function.as_str() {
                "Movement Basic" => {
                    let gravity = op.vector("gravity").unwrap_or_default()
                        + forces.iter().copied().sum::<Vec3>();
                    // Retail C_OP_BasicMovement: drag normalized to 30 steps a second,
                    // exp(log(1 - drag) x 30 x dt), times dt / previous dt (1 here: fixed
                    // ticks).
                    let drag = op.float("drag").unwrap_or(0.).clamp(0., 1.);
                    let keep = (1. - drag).powf(30. * dt);
                    for p in &mut particles {
                        let velocity = (p.position - p.previous) * keep;
                        p.previous = p.position;
                        p.position += velocity + gravity * dt * dt;
                    }
                }
                "Lifespan Decay" => particles.retain(|p| t - p.created < p.life),
                "Radius Scale" => {
                    let (start, end) = (
                        op.float("start_time").unwrap_or(0.),
                        op.float("end_time").unwrap_or(1.),
                    );
                    let (from, to) = (
                        op.float("radius_start_scale").unwrap_or(1.),
                        op.float("radius_end_scale").unwrap_or(1.),
                    );
                    let b = op.float("scale_bias").unwrap_or(0.5);
                    let ease = op.int("ease_in_and_out").unwrap_or(0) != 0;
                    // Retail C_OP_InterpolateRadius: only particles whose life fraction is
                    // in [start, end) change; easing replaces the bias.
                    if start < end {
                        for p in &mut particles {
                            let life = (t - p.created) / p.life.max(1e-3);
                            if p.life <= 0. || life < start || life >= end {
                                continue;
                            }
                            let f = (life - start) / (end - start);
                            let f = if ease { smooth(f) } else { bias(f, b) };
                            p.radius = p.initial_radius * lerp(from, to, f);
                        }
                    }
                }
                "Color Fade" => {
                    let c = op.color("color_fade").unwrap_or([255; 4]);
                    let target = Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.;
                    let (start, end) = (
                        op.float("fade_start_time").unwrap_or(0.),
                        op.float("fade_end_time").unwrap_or(1.),
                    );
                    let ease = op.int("ease_in_and_out").unwrap_or(0) != 0;
                    // Retail C_OP_ColorInterpolate: skipped when start == end.
                    if start != end {
                        for p in &mut particles {
                            if p.life <= 0. {
                                continue;
                            }
                            let f = ((t - p.created) / p.life - start) / (end - start);
                            let mut f = f.clamp(0., 1.);
                            if ease {
                                f = smooth(f);
                            }
                            p.color = p.initial_color.lerp(target, f);
                        }
                    }
                }
                "Alpha Fade In Random" | "Alpha Fade Out Random" => {
                    // Retail C_OP_FadeIn / C_OP_FadeOut: each particle's fade time is
                    // r^exponent x (max - min) + min (r from its random table); ages are
                    // life fractions when proportional (default 1). Fade in: spline, or
                    // f^"fade in curve exponent" when that is set. Fade out: spline with
                    // "ease in and out" (default 1), else 1 - Bias(f, "fade bias").
                    let fade_in = op.function == "Alpha Fade In Random";
                    let key = |name: &str| {
                        if fade_in {
                            format!("fade in {name}")
                        } else {
                            format!("fade out {name}")
                        }
                    };
                    let min = op.float(&key("time min")).unwrap_or(0.25);
                    let max = op.float(&key("time max")).unwrap_or(0.25);
                    let exponent = op.float(&key("time exponent")).unwrap_or(1.);
                    let proportional = op.int("proportional 0/1").unwrap_or(1) != 0;
                    let curve = op.float("fade in curve exponent").unwrap_or(0.);
                    let ease = op.int("ease in and out").unwrap_or(1) != 0;
                    let fade_bias = op.float("fade bias").unwrap_or(0.5);
                    for p in &mut particles {
                        let r = particle_random(p.seed, op_index);
                        let span = r.powf(exponent) * (max - min) + min;
                        if span <= 0. {
                            continue;
                        }
                        let (age, life) = if proportional {
                            ((t - p.created) / p.life.max(1e-3), 1.)
                        } else {
                            (t - p.created, p.life)
                        };
                        if fade_in {
                            let age = if proportional { age.clamp(0., 1.) } else { age };
                            if age < span {
                                let f = (age / span).clamp(0., 1.);
                                let f = if curve == 0. {
                                    smooth(f)
                                } else {
                                    f.powf(curve)
                                };
                                p.alpha = p.initial_alpha * f;
                            }
                        } else {
                            let start = life - span;
                            if start < age {
                                let f = ((age - start) / span).clamp(0., 1.);
                                let f = if ease { smooth(f) } else { bias(f, fade_bias) };
                                p.alpha = (p.initial_alpha * (1. - f)).max(0.);
                            }
                        }
                    }
                }
                "Movement Lock to Bone" => {
                    // Young particles ride with the model; the lock fades out between
                    // the lifetime fractions "lifetime fade start" and "... end".
                    let (start, end) = (
                        op.float("lifetime fade start").unwrap_or(0.),
                        op.float("lifetime fade end").unwrap_or(0.),
                    );
                    if moved != Vec3::ZERO {
                        for p in &mut particles {
                            let f = (t - p.created) / p.life.max(1e-3);
                            let lock = if end > start {
                                1. - ((f - start) / (end - start)).clamp(0., 1.)
                            } else {
                                f32::from(u8::from(f < end))
                            };
                            p.position += moved * lock;
                            p.previous += moved * lock;
                        }
                    }
                }
                "Remap Distance to Control Point to Scalar" => {
                    let (dmin, dmax) = (
                        op.float("distance minimum").unwrap_or(0.),
                        op.float("distance maximum").unwrap_or(1.),
                    );
                    let (omin, omax) = (
                        op.float("output minimum").unwrap_or(0.),
                        op.float("output maximum").unwrap_or(1.),
                    );
                    let field = op.int("output field").unwrap_or(7);
                    let center = (points.model_min + points.model_max) * 0.5;
                    for p in &mut particles {
                        let f = ((p.position.distance(center) - dmin) / (dmax - dmin).max(1e-3))
                            .clamp(0., 1.);
                        let v = lerp(omin, omax, f);
                        match field {
                            7 => p.alpha = p.initial_alpha * v,
                            3 => p.radius = p.initial_radius * v,
                            _ => {}
                        }
                    }
                }
                other => self.unsupported(other),
            }
        }
        // Emitters.
        let max = def.max_particles.unwrap_or(1000).max(0) as usize;
        let mut new = 0usize;
        if !stopped {
            for emitter in &def.emitters {
                if emitter.function != "emit_continuously" {
                    self.unsupported(&emitter.function);
                    continue;
                }
                let start = emitter.float("emission_start_time").unwrap_or(0.);
                let duration = emitter.float("emission_duration").unwrap_or(0.);
                let rate = emitter.float("emission_rate").unwrap_or(0.);
                if t < start || duration > 0. && t > start + duration {
                    continue;
                }
                instance.emitted += rate * dt;
                let whole = instance.emitted.floor();
                instance.emitted -= whole;
                new += whole as usize;
            }
        }
        let sheet = def
            .material
            .as_deref()
            .and_then(|m| library.sheets.get(&material_name(m)));
        for _ in 0..new {
            if particles.len() >= max {
                break;
            }
            let p = self.initialize(def, points, t, dt, sheet);
            particles.push(p);
        }
        instance.particles = particles;
    }
    fn initialize(
        &mut self,
        def: &ParticleSystem,
        points: &ControlPoints,
        t: f32,
        dt: f32,
        sheet: Option<&Vec<SheetSequence>>,
    ) -> Particle {
        let color = def.color.unwrap_or([255; 4]);
        let mut p = Particle {
            position: points.origin,
            previous: points.origin,
            created: t,
            life: 1.,
            radius: def.radius.unwrap_or(5.),
            initial_radius: 0.,
            rotation: 0.,
            color: Vec3::new(color[0] as f32, color[1] as f32, color[2] as f32) / 255.,
            initial_color: Vec3::ZERO,
            alpha: color[3] as f32 / 255.,
            initial_alpha: 0.,
            sequence: 0,
            flip: false,
            seed: 0,
        };
        p.seed = (self.random() * u32::MAX as f32) as u32;
        let mut velocity = Vec3::ZERO;
        // Initializers run in order; `remap initial scalar` reads fields set before it.
        for op in &def.initializers {
            let r = self.random();
            match op.function.as_str() {
                "Alpha Random" => {
                    let e = op.float("alpha_random_exponent").unwrap_or(1.);
                    p.alpha = lerp(
                        op.int("alpha_min").unwrap_or(255) as f32,
                        op.int("alpha_max").unwrap_or(255) as f32,
                        r.powf(e),
                    ) / 255.;
                }
                "Color Random" => {
                    let a = op.color("color1").unwrap_or([255; 4]);
                    let b = op.color("color2").unwrap_or([255; 4]);
                    let mix = |i: usize| lerp(a[i] as f32, b[i] as f32, r) / 255.;
                    p.color = Vec3::new(mix(0), mix(1), mix(2));
                }
                "Lifetime Random" => {
                    p.life = lerp(
                        op.float("lifetime_min").unwrap_or(1.),
                        op.float("lifetime_max").unwrap_or(1.),
                        r,
                    );
                }
                "lifetime from sequence" => {
                    let fps = op.float("Frames Per Second").unwrap_or(30.).max(1e-3);
                    if let Some(seq) = sheet.and_then(|s| s.get(p.sequence)) {
                        p.life = seq.frames.len() as f32 / fps;
                    }
                }
                "Radius Random" => {
                    let e = op.float("radius_random_exponent").unwrap_or(1.);
                    p.radius = lerp(
                        op.float("radius_min").unwrap_or(1.),
                        op.float("radius_max").unwrap_or(1.),
                        r.powf(e),
                    );
                }
                "Rotation Random" => {
                    p.rotation = op.float("rotation_initial").unwrap_or(0.)
                        + lerp(
                            op.float("rotation_offset_min").unwrap_or(0.),
                            op.float("rotation_offset_max").unwrap_or(360.),
                            r,
                        );
                }
                "Rotation Yaw Flip Random" => {
                    p.flip = r < op.float("Flip Percentage").unwrap_or(0.5);
                }
                "Sequence Random" => {
                    let (lo, hi) = (
                        op.int("sequence_min").unwrap_or(0).max(0),
                        op.int("sequence_max").unwrap_or(0).max(0),
                    );
                    let span = (hi - lo + 1).max(1) as f32;
                    p.sequence = (lo + ((r * span) as i32).min(hi - lo)) as usize;
                }
                "Position on Model Random" => {
                    // Hitboxes are approximated by the model's world bounds ("force to be
                    // inside model": kept inside, else scaled by "hitbox scale"); an
                    // upward direction bias of n picks the highest of n + 1 heights
                    // (native flames gather above a burning barrel's top).
                    let inside = op.int("force to be inside model").unwrap_or(0) != 0;
                    let scale = if inside {
                        1.
                    } else {
                        op.float("hitbox scale").unwrap_or(1.)
                    };
                    let center = (points.model_min + points.model_max) * 0.5;
                    let half = (points.model_max - points.model_min) * 0.5 * scale;
                    let bias = op.vector("direction bias").unwrap_or_default();
                    let mut u = Vec3::new(r, self.random(), self.random()) * 2. - Vec3::ONE;
                    for _ in 0..bias.z.max(0.).round() as usize {
                        u.z = u.z.max(self.random() * 2. - 1.);
                    }
                    p.position = center + u * half.min(Vec3::splat(256.));
                }
                "Position Within Sphere Random" => {
                    let (dmin, dmax) = (
                        op.float("distance_min").unwrap_or(0.),
                        op.float("distance_max").unwrap_or(0.),
                    );
                    let dir = Vec3::new(r, self.random(), self.random()) * 2. - Vec3::ONE;
                    let dir = dir.normalize_or(Vec3::Z);
                    p.position = points.origin + dir * lerp(dmin, dmax, self.random());
                }
                "Position Modify Offset Random" => {
                    let (min, max) = (
                        op.vector("offset min").unwrap_or_default(),
                        op.vector("offset max").unwrap_or_default(),
                    );
                    let mut offset = Vec3::new(
                        lerp(min.x, max.x, r),
                        lerp(min.y, max.y, self.random()),
                        lerp(min.z, max.z, self.random()),
                    );
                    if op.int("offset proportional to radius 0/1").unwrap_or(0) != 0 {
                        offset *= p.radius;
                    }
                    p.position += offset;
                }
                "Velocity Noise" => {
                    let (min, max) = (
                        op.vector("output minimum").unwrap_or_default(),
                        op.vector("output maximum").unwrap_or_default(),
                    );
                    velocity = Vec3::new(
                        lerp(min.x, max.x, r),
                        lerp(min.y, max.y, self.random()),
                        lerp(min.z, max.z, self.random()),
                    );
                }
                "Remap Noise to Scalar" => {
                    let v = lerp(
                        op.float("output minimum").unwrap_or(0.),
                        op.float("output maximum").unwrap_or(1.),
                        r,
                    );
                    match op.int("output field").unwrap_or(-1) {
                        3 => p.radius = v,
                        4 => p.rotation = v,
                        7 => p.alpha = v,
                        1 => p.life = v,
                        _ => self.unsupported("Remap Noise to Scalar field"),
                    }
                }
                "remap initial scalar" => {
                    let input = match op.int("input field").unwrap_or(-1) {
                        8 => t,
                        3 => p.radius,
                        7 => p.alpha,
                        1 => p.life,
                        _ => {
                            self.unsupported("remap initial scalar input");
                            continue;
                        }
                    };
                    let (imin, imax) = (
                        op.float("input minimum").unwrap_or(0.),
                        op.float("input maximum").unwrap_or(1.),
                    );
                    let f = ((input - imin) / (imax - imin).max(1e-6)).clamp(0., 1.);
                    let v = lerp(
                        op.float("output minimum").unwrap_or(0.),
                        op.float("output maximum").unwrap_or(1.),
                        f,
                    );
                    let scale = op
                        .int("output is scalar of initial random range")
                        .unwrap_or(0)
                        != 0;
                    let field = match op.int("output field").unwrap_or(-1) {
                        3 => &mut p.radius,
                        7 => &mut p.alpha,
                        4 => &mut p.rotation,
                        1 => &mut p.life,
                        _ => continue,
                    };
                    *field = if scale { *field * v } else { v };
                }
                other => self.unsupported(other),
            }
        }
        p.previous = p.position - velocity * dt;
        p.initial_radius = p.radius;
        p.initial_color = p.color;
        p.initial_alpha = p.alpha;
        p
    }
    /// Camera-facing sprites (render_animated_sprites, orientation 0) for every live
    /// particle: sheet frame by age, rotated about the view axis.
    pub fn draw(&self, eye: Vec3, direction: Vec3, library: &Library) -> Vec<Quad> {
        let right = direction.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = right.cross(direction).normalize_or(Vec3::Z);
        let mut quads = Vec::new();
        for effect in self.effects.values() {
            Self::draw_instance(&effect.instance, eye, right, up, library, &mut quads);
        }
        quads
    }
    fn draw_instance(
        instance: &Instance,
        eye: Vec3,
        right: Vec3,
        up: Vec3,
        library: &Library,
        quads: &mut Vec<Quad>,
    ) {
        for child in &instance.children {
            Self::draw_instance(child, eye, right, up, library, quads);
        }
        let Some(def) = library.systems.get(&instance.system) else {
            return;
        };
        let Some(material) = def.material.as_deref().map(material_name) else {
            return;
        };
        let renderer: Option<&Operator> = def
            .renderers
            .iter()
            .find(|r| r.function == "render_animated_sprites");
        let rate = renderer
            .and_then(|r| r.float("animation rate"))
            .unwrap_or(1.);
        let fps = renderer
            .and_then(|r| r.int("use animation rate as FPS"))
            .unwrap_or(0)
            != 0;
        let sheet = library.sheets.get(&material);
        let t = instance.time - instance.delay;
        // Far particles first (alpha blending).
        let mut order: Vec<&Particle> = instance.particles.iter().collect();
        order.sort_by(|a, b| {
            b.position
                .distance_squared(eye)
                .total_cmp(&a.position.distance_squared(eye))
        });
        for p in order {
            let age = (t - p.created).max(0.);
            let mut rect = [0., 0., 1., 1.];
            if let Some(seq) = sheet.and_then(|s| s.get(p.sequence % s.len().max(1))) {
                let n = seq.frames.len().max(1);
                let cycle = if fps {
                    age * rate / n as f32
                } else {
                    age / p.life.max(1e-3) * rate
                };
                let frac = if seq.clamp {
                    cycle.min(0.999)
                } else {
                    cycle.fract()
                };
                let frame = ((frac * n as f32) as usize).min(n - 1);
                if let Some(image) = seq.frames.get(frame).and_then(|f| f.images.first()) {
                    rect = *image;
                }
            }
            let (s, c) = p.rotation.to_radians().sin_cos();
            let r = (right * c + up * s) * p.radius;
            let u = (up * c - right * s) * p.radius;
            let (mut u0, v0, mut u1, v1) = (rect[0], rect[1], rect[2], rect[3]);
            if p.flip {
                std::mem::swap(&mut u0, &mut u1);
            }
            // SpriteCard $OVERBRIGHTFACTOR scales the colour (clamped to the vertex
            // colour's range).
            let color =
                (p.color * library.overbright(&material)).clamp(Vec3::ZERO, Vec3::ONE) * 255.;
            let color = color.round();
            quads.push(Quad {
                material: material.clone(),
                positions: [
                    p.position - r - u,
                    p.position - r + u,
                    p.position + r + u,
                    p.position + r - u,
                ],
                color: [
                    color.x as u8,
                    color.y as u8,
                    color.z as u8,
                    (p.alpha.clamp(0., 1.) * 255.).round() as u8,
                ],
                uv: [[u0, v1], [u0, v0], [u1, v0], [u1, v1]],
            });
        }
    }
}

/// Load the runtime's systems and their materials' sheets from the installation.
pub fn load(vfs: &source_assets::vpk::Vfs) -> (Library, Vec<String>) {
    let mut library = Library::default();
    let mut errors = Vec::new();
    for file in FILES {
        match vfs.read(file) {
            Ok(Some(data)) => match source_assets::pcf::particle_systems(&data) {
                Ok(systems) => {
                    for s in systems {
                        library.systems.insert(s.name.clone(), s);
                    }
                }
                Err(e) => errors.push(format!("{file}: {e:#}")),
            },
            _ => errors.push(format!("{file}: missing")),
        }
    }
    for material in library.materials() {
        if let Some(v) = vfs
            .material_value(&material, "$overbrightfactor")
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse::<f32>().ok())
        {
            library.overbright.insert(material.clone(), v);
        }
        let sheet = vfs
            .base_texture(&material)
            .ok()
            .flatten()
            .and_then(|base| {
                vfs.read(&format!("materials/{}.vtf", base.trim_end_matches(".vtf")))
                    .ok()
                    .flatten()
            })
            .map(|data| source_assets::vtf::sheet(&data));
        match sheet {
            Some(Ok(Some(sheet))) => {
                library.sheets.insert(material, sheet);
            }
            Some(Err(e)) => errors.push(format!("{material}: {e:#}")),
            _ => {}
        }
    }
    (library, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_bias_and_per_particle_fade_randoms() {
        // Schlick bias as the retail operators compute it: 0.5 is linear, 0 means 0.5,
        // the curve passes through (0.5, bias).
        for x in [0., 0.25, 0.5, 1.] {
            assert!((bias(x, 0.5) - x).abs() < 1e-6);
            assert!((bias(x, 0.) - x).abs() < 1e-6);
        }
        assert!((bias(0.5, 0.4) - 0.4).abs() < 1e-6);
        assert!((bias(1., 0.4) - 1.).abs() < 1e-6);
        // Per-particle randoms differ between particles and operators, stay in [0, 1)
        // and repeat for the same pair.
        let values: Vec<f32> = (0..64)
            .map(|seed| particle_random(seed * 7919, 3))
            .collect();
        assert!(values.iter().all(|v| (0. ..1.).contains(v)));
        assert!(values.iter().any(|v| *v < 0.3) && values.iter().any(|v| *v > 0.7));
        assert_ne!(particle_random(5, 1), particle_random(5, 2));
        assert_eq!(particle_random(5, 1), particle_random(5, 1));
    }
    #[test]
    #[ignore = "reads the installed game (HL2_ROOT)"]
    fn burning_character_runs_and_draws() {
        let game = source_assets::install::discover().unwrap();
        let vfs = source_assets::vpk::Vfs::mount(&game).unwrap();
        let (library, errors) = load(&vfs);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(library
            .sheets
            .contains_key("particle/fire_burning_character/fire_burning_character"));
        let mut runtime = Runtime::default();
        let points = ControlPoints {
            origin: Vec3::ZERO,
            model_min: Vec3::new(-12., -12., 0.),
            model_max: Vec3::new(12., 12., 44.),
        };
        runtime.start(7, BURNING_CHARACTER, points, &library);
        for _ in 0..200 {
            runtime.simulate(0.015, &library);
        }
        let quads = runtime.draw(Vec3::new(-200., 0., 30.), Vec3::X, &library);
        // Four systems emit (13.5 + 7 + 30 + 150 per second, capped by max_particles).
        assert!(
            runtime.diagnostics.particles > 50,
            "{}",
            runtime.diagnostics.particles
        );
        assert_eq!(quads.len(), runtime.diagnostics.particles);
        assert!(quads
            .iter()
            .any(|q| q.material.contains("fire_burning_character")));
        // Fire rises: flame particles sit above the model's base.
        let mean_z = quads.iter().map(|q| q.positions[0].z).sum::<f32>() / quads.len() as f32;
        assert!(mean_z > 0., "{mean_z}");
        // Stopped effects drain and disappear.
        runtime.stop(7);
        for _ in 0..600 {
            runtime.simulate(0.015, &library);
        }
        assert!(runtime.owners().is_empty());
    }
}
