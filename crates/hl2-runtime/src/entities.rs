//! Deterministic map entity I/O. Unsupported inputs are reported instead of silently emulated.
use crate::physics;
use glam::{Quat, Vec3};
use modkit_core::{parse_vec3, Entity, World};
use serde::Serialize;
use source_assets::{
    scenes::{Cache, ChoreoScene, EventType},
    vpk::Vfs,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

enum ScenePause {
    Input,
    Section {
        blocked: bool,
        automated: Option<(bool, f64)>,
    },
    UnsupportedControl,
}
struct Playback {
    elapsed: f64,
    next: usize,
    completed_early: bool,
    pause: Option<ScenePause>,
    actors: Vec<Option<usize>>,
}
struct Choreography {
    data: Arc<ChoreoScene>,
    order: Vec<usize>,
    playback: Option<Playback>,
}
#[derive(Clone)]
struct SceneAnimation {
    owner: usize,
    event: usize,
    previous: String,
    previous_started: f64,
    previous_done: Option<f64>,
}
fn number(e: &Entity, key: &str, default: f32) -> f32 {
    e.get(key)
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
#[derive(Clone)]
struct Output {
    name: String,
    target: String,
    input: String,
    parameter: String,
    delay: f64,
    remaining: i32,
}
fn output(name: &str, text: &str) -> Option<Output> {
    let separator = if text.contains('\x1b') { '\x1b' } else { ',' };
    let values: Vec<_> = text.split(separator).collect();
    if values.len() != 5 {
        return None;
    }
    let delay = values[3].parse::<f64>().ok()?;
    if !delay.is_finite() || delay < 0. {
        return None;
    }
    Some(Output {
        name: name.into(),
        target: values[0].into(),
        input: values[1].into(),
        parameter: values[2].into(),
        delay,
        remaining: values[4].parse().ok()?,
    })
}
#[derive(Clone)]
struct Pending {
    due: f64,
    sequence: u64,
    target: String,
    input: String,
    parameter: String,
    caller: usize,
    activator: usize,
}
#[derive(Clone)]
pub struct State {
    pub origin: Vec3,
    pub rotation: Quat,
    pub visible: bool,
    pub enabled: bool,
    pub killed: bool,
    pub animation: String,
    pub default_animation: String,
    pub animation_started: f64,
    animation_done: Option<f64>,
    scene_animation: Option<SceneAnimation>,
    script_actor: Option<usize>,
    pub scripted_by: Option<usize>,
    script_finish: Option<f64>,
    post_idle_done: Option<(f64, usize)>,
    shuffle: Vec<usize>,
    last_shuffle: Option<usize>,
    base_origin: Vec3,
    base_rotation: Quat,
    translation: Vec3,
    angle: f32,
    axis: Vec3,
    fraction: f32,
    target: f32,
    rate: f32,
    pub locked: bool,
    return_at: Option<f64>,
    wait: f64,
    touching: bool,
    last_trigger: f64,
    pub value: f32,
    timer_at: f64,
    outputs: Vec<Output>,
}
#[derive(Default, Serialize)]
pub struct Diagnostics {
    pub inputs_delivered: u64,
    pub outputs_fired: u64,
    pub trigger_entries: u64,
    pub door_completions: u64,
    pub unsupported: BTreeMap<String, u64>,
    pub malformed_outputs: usize,
    pub budget_exhaustions: u64,
    pub scenes_loaded: usize,
    pub scene_events_started: u64,
    pub scene_completions: u64,
}
#[derive(Serialize)]
pub struct ChoreographyState {
    pub entity: usize,
    pub targetname: String,
    pub file: String,
    pub elapsed: Option<f64>,
    pub pause: bool,
    pub pause_reason: Option<&'static str>,
    pub completed_early: bool,
    pub active: bool,
}
#[derive(Serialize)]
pub struct SceneAnimationState {
    pub entity: usize,
    pub targetname: String,
    pub animation: String,
    pub sample_time: f32,
    pub scene_owner: Option<usize>,
    pub scripted_by: Option<usize>,
    pub clip_loaded: bool,
}
pub struct Scene {
    pub states: Vec<State>,
    pub time: f64,
    sequence: u64,
    random: u64,
    queue: Vec<Pending>,
    pub diagnostics: Diagnostics,
    pub transition: Option<(String, String)>,
    pub sounds: Vec<String>,
    unsupported: BTreeSet<String>,
    choreography: BTreeMap<usize, Choreography>,
}
impl Scene {
    pub fn new(world: &World) -> Self {
        Self::with_campaign(world, true)
    }
    pub fn with_campaign(world: &World, new_game: bool) -> Self {
        let mut scene = Self {
            states: Vec::new(),
            time: 0.,
            sequence: 0,
            random: 1,
            queue: Vec::new(),
            diagnostics: Diagnostics::default(),
            transition: None,
            sounds: Vec::new(),
            unsupported: BTreeSet::new(),
            choreography: BTreeMap::new(),
        };
        for e in &world.entities {
            let base_rotation = physics::angles(
                parse_vec3(e.get("angles").unwrap_or("0 0 0")).unwrap_or(Vec3::ZERO),
            );
            let flags = number(e, "spawnflags", 0.) as u32;
            let model = e
                .get("model")
                .and_then(|m| m.strip_prefix('*'))
                .and_then(|m| m.parse::<usize>().ok())
                .and_then(|id| world.brush_models.iter().find(|m| m.id == id));
            let bounds = model.map(|m| m.maxs - m.mins).unwrap_or(Vec3::splat(72.));
            let direction = parse_vec3(e.get("movedir").unwrap_or("0 0 0")).unwrap_or(Vec3::ZERO);
            let direction = if direction.x == -1. {
                Vec3::Z
            } else if direction.x == -2. {
                -Vec3::Z
            } else {
                let p = direction.x.to_radians();
                let y = direction.y.to_radians();
                Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), -p.sin())
            };
            let travel = number(
                e,
                "movedistance",
                (bounds - Vec3::splat(2.))
                    .max(Vec3::ZERO)
                    .dot(direction.abs())
                    - number(e, "lip", 0.),
            )
            .max(0.01);
            let rotating = matches!(e.class(), "func_door_rotating" | "prop_door_rotating");
            let angle =
                number(e, "distance", 90.).to_radians() * if flags & 2 != 0 { -1. } else { 1. };
            let axis = if flags & 64 != 0 {
                Vec3::X
            } else if flags & 128 != 0 {
                Vec3::Y
            } else {
                Vec3::Z
            };
            let fraction = if e.class() == "prop_door_rotating" {
                number(e, "spawnpos", 0.).min(1.)
            } else if matches!(e.class(), "func_door" | "func_door_rotating") && flags & 1 != 0 {
                1.
            } else {
                0.
            };
            let speed = number(e, "speed", 100.).max(0.01);
            let mut outputs = Vec::new();
            for (k, v) in &e.properties {
                if k.starts_with("On") || k == "OutValue" {
                    match output(k, v) {
                        Some(o) => outputs.push(o),
                        None => scene.diagnostics.malformed_outputs += 1,
                    }
                }
            }
            let enabled =
                e.get("StartDisabled") != Some("1") && e.get("startdisabled") != Some("1");
            let mut s = State {
                origin: e.origin(),
                rotation: base_rotation,
                visible: e.get("rendermode") != Some("10"),
                enabled,
                killed: false,
                animation: e
                    .get("DefaultAnim")
                    .unwrap_or(if e.class() == "npc_metropolice" {
                        "idle_baton"
                    } else if e.class() == "npc_citizen" {
                        "idle_subtle"
                    } else {
                        "idle"
                    })
                    .to_lowercase(),
                animation_started: 0.,
                default_animation: e.get("DefaultAnim").unwrap_or("").into(),
                animation_done: None,
                scene_animation: None,
                script_actor: None,
                scripted_by: None,
                script_finish: None,
                post_idle_done: None,
                shuffle: Vec::new(),
                last_shuffle: None,
                base_origin: e.origin(),
                base_rotation,
                translation: if rotating {
                    Vec3::ZERO
                } else {
                    direction * travel
                },
                angle: if rotating { angle } else { 0. },
                axis,
                fraction,
                target: fraction,
                rate: if rotating {
                    speed.to_radians() / angle.abs().max(0.001)
                } else {
                    speed / travel
                },
                locked: flags & 2048 != 0 || e.get("locked") == Some("1"),
                return_at: None,
                wait: number(
                    e,
                    if e.class() == "prop_door_rotating" {
                        "returndelay"
                    } else {
                        "wait"
                    },
                    -1.,
                ) as f64,
                touching: false,
                last_trigger: -1e6,
                value: number(e, "startvalue", 0.),
                timer_at: number(e, "RefireTime", 1.).max(0.015) as f64,
                outputs,
            };
            s.origin = s.base_origin + s.translation * s.fraction;
            s.rotation = s.base_rotation * Quat::from_axis_angle(s.axis, s.angle * s.fraction);
            scene.states.push(s);
        }
        for id in 0..world.entities.len() {
            if world.entities[id].class() == "logic_auto" {
                scene.fire(id, "OnMapSpawn", usize::MAX);
                if new_game {
                    scene.fire(id, "OnNewGame", usize::MAX);
                }
            }
        }
        scene
    }
    /// Preload map choreography before simulation; no blocking asset reads in tick.
    pub fn load_choreography(&mut self, world: &World, vfs: &Vfs) -> anyhow::Result<usize> {
        use anyhow::Context;
        if !world
            .entities
            .iter()
            .any(|e| e.class() == "logic_choreographed_scene")
        {
            return Ok(0);
        }
        let bytes = vfs
            .read("scenes/scenes.image")?
            .context("installed scenes.image missing")?;
        let cache = Cache::parse(bytes)?;
        let mut parsed: BTreeMap<String, Arc<ChoreoScene>> = BTreeMap::new();
        for (id, entity) in world.entities.iter().enumerate() {
            if entity.class() != "logic_choreographed_scene" {
                continue;
            }
            let path = entity.get("SceneFile").unwrap_or("");
            let result = if let Some(data) = parsed.get(path) {
                Ok(Some(data.clone()))
            } else {
                cache.scene(path).map(|scene| {
                    scene.map(|data| {
                        let data = Arc::new(data);
                        parsed.insert(path.into(), data.clone());
                        data
                    })
                })
            };
            match result {
                Ok(Some(data)) => self.install_choreography(id, data),
                Ok(None) => {
                    self.unsupported_input(entity.class(), &format!("missing-scene:{path}"))
                }
                Err(error) => {
                    self.unsupported_input(entity.class(), &format!("scene:{path}:{error:#}"))
                }
            }
        }
        Ok(self.choreography.len())
    }
    fn install_choreography(&mut self, id: usize, data: Arc<ChoreoScene>) {
        let mut order: Vec<_> = (0..data.events.len()).collect();
        order.sort_by(|a, b| {
            data.events[*a]
                .start
                .total_cmp(&data.events[*b].start)
                .then_with(|| data.events[*a].name.cmp(&data.events[*b].name))
        });
        self.choreography.insert(
            id,
            Choreography {
                data,
                order,
                playback: None,
            },
        );
        self.diagnostics.scenes_loaded = self.choreography.len();
    }
    fn scene_actor(
        &self,
        world: &World,
        scene: usize,
        name: &str,
        activator: usize,
    ) -> Option<usize> {
        let mut name = name;
        if let Some(slot) = name
            .strip_prefix('!')
            .and_then(|s| s.to_lowercase().strip_prefix("target").map(str::to_owned))
            .and_then(|s| s.parse::<usize>().ok())
        {
            if (1..=8).contains(&slot) {
                name = world.entities[scene]
                    .get(&format!("target{slot}"))
                    .unwrap_or("");
            }
        }
        if name.eq_ignore_ascii_case("!activator") {
            return (activator < self.states.len()).then_some(activator);
        }
        world
            .entities
            .iter()
            .enumerate()
            .find(|(id, entity)| {
                !self.states[*id].killed
                    && (entity.class().starts_with("npc_")
                        || entity.class().starts_with("prop_dynamic"))
                    && entity
                        .get("targetname")
                        .is_some_and(|target| glob(&name.to_lowercase(), &target.to_lowercase()))
            })
            .map(|(id, _)| id)
    }
    /// Installed animation names required by loaded VCDs, grouped by model/skin.
    /// Gestures are requested for future layer support, but are not substituted for a body sequence.
    pub fn required_animation_clips(&self, world: &World) -> BTreeMap<String, BTreeSet<String>> {
        let mut result: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (id, scene) in &self.choreography {
            for event in &scene.data.events {
                if !event.active()
                    || !matches!(event.kind, EventType::Sequence | EventType::Gesture)
                    || event.name.eq_ignore_ascii_case("NULL")
                    || event.parameters[0].is_empty()
                {
                    continue;
                }
                let Some(actor) = event.actor.and_then(|a| {
                    self.scene_actor(world, *id, &scene.data.actors[a].name, usize::MAX)
                }) else {
                    continue;
                };
                if let Some(instance) = world
                    .model_instances
                    .iter()
                    .find(|i| i.entity == Some(actor))
                {
                    result
                        .entry(instance.asset_key())
                        .or_default()
                        .insert(event.parameters[0].to_lowercase());
                }
            }
        }
        result
    }
    /// Sequence sampling uses the paused choreography clock, not wall/game time.
    pub fn animation_time(&self, id: usize) -> f32 {
        let state = &self.states[id];
        if let Some(animation) = &state.scene_animation {
            if let Some(scene) = self.choreography.get(&animation.owner) {
                if let Some(play) = &scene.playback {
                    return (play.elapsed - scene.data.events[animation.event].start as f64).max(0.)
                        as f32;
                }
            }
        }
        (self.time - state.animation_started).max(0.) as f32
    }
    /// Read-only capture metadata; this neither advances nor resolves scene readiness.
    pub fn choreography_states(&self, world: &World) -> Vec<ChoreographyState> {
        self.choreography
            .iter()
            .map(|(id, scene)| {
                let play = scene.playback.as_ref();
                let pause_reason = play.and_then(|p| p.pause.as_ref()).map(|p| match p {
                    ScenePause::Input => "input",
                    ScenePause::Section { blocked: true, .. } => "actor_condition",
                    ScenePause::Section { blocked: false, .. } => "section",
                    ScenePause::UnsupportedControl => "unsupported_control",
                });
                ChoreographyState {
                    entity: *id,
                    targetname: world.entities[*id].get("targetname").unwrap_or("").into(),
                    file: world.entities[*id].get("SceneFile").unwrap_or("").into(),
                    elapsed: play.map(|p| p.elapsed),
                    pause: pause_reason.is_some(),
                    pause_reason,
                    completed_early: play.is_some_and(|p| p.completed_early),
                    active: play.is_some(),
                }
            })
            .collect()
    }
    /// Named NPC/dynamic-prop samples, including whether an installed rig supplies the clip.
    pub fn animation_states(&self, world: &World) -> Vec<SceneAnimationState> {
        world
            .entities
            .iter()
            .enumerate()
            .filter_map(|(id, entity)| {
                if !(entity.class().starts_with("npc_")
                    || entity.class().starts_with("prop_dynamic"))
                {
                    return None;
                }
                let targetname = entity.get("targetname").filter(|n| !n.is_empty())?;
                let state = &self.states[id];
                if state.killed || state.animation.is_empty() {
                    return None;
                }
                let clip_loaded = world
                    .model_instances
                    .iter()
                    .find(|i| i.entity == Some(id))
                    .and_then(|i| world.rigs.get(&i.asset_key()))
                    .is_some_and(|rig| rig.clips.contains_key(&state.animation));
                Some(SceneAnimationState {
                    entity: id,
                    targetname: targetname.into(),
                    animation: state.animation.clone(),
                    sample_time: self.animation_time(id),
                    scene_owner: state.scene_animation.as_ref().map(|a| a.owner),
                    scripted_by: state.scripted_by,
                    clip_loaded,
                })
            })
            .collect()
    }
    fn restore_scene_animation(&mut self, actor: usize) {
        let state = &mut self.states[actor];
        if let Some(animation) = state.scene_animation.take() {
            state.animation = animation.previous;
            state.animation_started = animation.previous_started;
            state.animation_done = animation.previous_done;
        }
    }
    fn clear_scene_animations(&mut self, owner: usize) {
        for actor in 0..self.states.len() {
            if self.states[actor]
                .scene_animation
                .as_ref()
                .is_some_and(|s| s.owner == owner)
            {
                self.restore_scene_animation(actor);
            }
        }
    }
    fn start_choreography(&mut self, world: &World, id: usize, activator: usize) {
        let Some(mut scene) = self.choreography.remove(&id) else {
            self.unsupported_input("logic_choreographed_scene", "Start:scene-not-loaded");
            return;
        };
        if scene.playback.is_none() {
            let actors = scene
                .data
                .actors
                .iter()
                .map(|a| self.scene_actor(world, id, &a.name, activator))
                .collect();
            scene.playback = Some(Playback {
                elapsed: 0.,
                next: 0,
                completed_early: false,
                pause: None,
                actors,
            });
            self.fire(id, "OnStart", id);
        }
        self.choreography.insert(id, scene);
    }
    fn cancel_choreography(&mut self, id: usize) {
        if self
            .choreography
            .get_mut(&id)
            .is_some_and(|s| s.playback.take().is_some())
        {
            self.clear_scene_animations(id);
            self.fire(id, "OnCanceled", id);
        }
    }
    fn tick_choreography(&mut self, world: &World, dt: f32) {
        let ids: Vec<_> = self.choreography.keys().copied().collect();
        for id in ids {
            let mut scene = self.choreography.remove(&id).unwrap();
            if self.states[id].killed {
                scene.playback = None;
                self.clear_scene_animations(id);
            }
            if let Some(mut play) = scene.playback.take() {
                let mut canceled = false;
                if let Some(ScenePause::Section { blocked, automated }) = &play.pause {
                    if let Some((resume, due)) = automated {
                        if self.time >= *due {
                            canceled = !resume;
                            play.pause = None;
                        }
                    } else if !blocked {
                        play.pause = None;
                    }
                }
                if canceled {
                    self.clear_scene_animations(id);
                    self.fire(id, "OnCanceled", id);
                } else {
                    if play.pause.is_none() {
                        let end = play.elapsed + dt as f64;
                        while let Some(index) = scene.order.get(play.next).copied() {
                            let event = &scene.data.events[index];
                            if event.start as f64 > end {
                                break;
                            }
                            play.next += 1;
                            if !event.active() || event.name.eq_ignore_ascii_case("NULL") {
                                continue;
                            }
                            let actor = event.actor.and_then(|actor| play.actors[actor]);
                            if event.actor.is_some() && actor.is_none() {
                                self.unsupported_input(
                                    "logic_choreographed_scene",
                                    "missing-actor",
                                );
                                continue;
                            }
                            self.diagnostics.scene_events_started += 1;
                            match event.kind {
                                EventType::FireTrigger => {
                                    if let Ok(trigger) = event.parameters[0].parse::<u8>() {
                                        if (1..=16).contains(&trigger) {
                                            self.fire(
                                                id,
                                                &format!("OnTrigger{trigger}"),
                                                actor.unwrap_or(id),
                                            );
                                        }
                                    }
                                }
                                EventType::StopPoint => {
                                    if !play.completed_early {
                                        play.completed_early = true;
                                        self.diagnostics.scene_completions += 1;
                                        self.fire(id, "OnCompletion", id);
                                    }
                                }
                                EventType::Speak => {
                                    if actor.is_some() && !event.parameters[0].is_empty() {
                                        self.sounds.push(event.parameters[0].clone());
                                    }
                                }
                                EventType::Sequence => {
                                    if let Some(actor) = actor {
                                        if self.states[actor].scripted_by.is_some()
                                            && event.flags & 32 == 0
                                        {
                                            self.unsupported_input(
                                                "logic_choreographed_scene",
                                                "SEQUENCE:scripted-actor-busy",
                                            );
                                        } else {
                                            self.restore_scene_animation(actor);
                                            let previous = SceneAnimation {
                                                owner: id,
                                                event: index,
                                                previous: self.states[actor].animation.clone(),
                                                previous_started: self.states[actor]
                                                    .animation_started,
                                                previous_done: self.states[actor].animation_done,
                                            };
                                            if self.animate(
                                                world,
                                                actor,
                                                &event.parameters[0],
                                                false,
                                            ) {
                                                self.states[actor].scene_animation = Some(previous);
                                            }
                                        }
                                    }
                                }
                                EventType::Section => {
                                    let blocked = scene.data.events.iter().any(|e| {
                                        e.active()
                                            && e.resume_condition()
                                            && e.start <= event.start
                                            && e.actor.is_some_and(|a| play.actors[a].is_some())
                                    });
                                    let tokens: Vec<_> =
                                        event.parameters[0].split_whitespace().collect();
                                    let automated = if tokens.len() == 3
                                        && tokens[0].eq_ignore_ascii_case("automate")
                                    {
                                        tokens[2]
                                            .parse::<f64>()
                                            .ok()
                                            .filter(|d| d.is_finite() && *d > 0.)
                                            .and_then(|d| {
                                                if tokens[1].eq_ignore_ascii_case("resume") {
                                                    Some((true, self.time + d))
                                                } else if tokens[1].eq_ignore_ascii_case("cancel") {
                                                    Some((false, self.time + d))
                                                } else {
                                                    None
                                                }
                                            })
                                    } else {
                                        None
                                    };
                                    if blocked && automated.is_none() {
                                        self.unsupported_input(
                                            "logic_choreographed_scene",
                                            "SECTION:actor-resume-condition",
                                        );
                                    }
                                    play.pause = Some(ScenePause::Section { blocked, automated });
                                }
                                EventType::Loop | EventType::SubScene => {
                                    self.unsupported_input(
                                        "logic_choreographed_scene",
                                        &format!("control:{:?}", event.kind),
                                    );
                                    play.pause = Some(ScenePause::UnsupportedControl);
                                }
                                EventType::Interrupt
                                | EventType::PermitResponses
                                | EventType::Unspecified => {
                                    self.unsupported_input(
                                        "logic_choreographed_scene",
                                        &format!("event:{:?}", event.kind),
                                    );
                                }
                                _ => self.unsupported_input(
                                    "logic_choreographed_scene",
                                    &format!("actor-event:{:?}", event.kind),
                                ),
                            }
                            if play.pause.is_some() {
                                play.elapsed = event.start as f64;
                                break;
                            }
                        }
                        if play.pause.is_none() {
                            play.elapsed = end;
                        }
                    }
                    if play.pause.is_none()
                        && play.next == scene.order.len()
                        && play.elapsed > scene.data.stop_time() as f64
                    {
                        if !play.completed_early {
                            self.diagnostics.scene_completions += 1;
                            self.fire(id, "OnCompletion", id);
                        }
                        self.clear_scene_animations(id);
                    } else {
                        for actor in 0..self.states.len() {
                            if self.states[actor]
                                .scene_animation
                                .as_ref()
                                .is_some_and(|a| {
                                    a.owner == id
                                        && play.elapsed
                                            > scene.data.events[a.event]
                                                .end
                                                .unwrap_or(scene.data.events[a.event].start)
                                                as f64
                                })
                            {
                                self.restore_scene_animation(actor);
                            }
                        }
                        scene.playback = Some(play);
                    }
                }
            }
            self.choreography.insert(id, scene);
        }
    }
    pub fn fire(&mut self, id: usize, name: &str, activator: usize) {
        let Some(state) = self.states.get_mut(id) else {
            return;
        };
        for output in state
            .outputs
            .iter_mut()
            .filter(|o| o.name.eq_ignore_ascii_case(name) && o.remaining != 0)
        {
            if output.remaining > 0 {
                output.remaining -= 1;
            }
            self.sequence += 1;
            self.diagnostics.outputs_fired += 1;
            self.queue.push(Pending {
                due: self.time + output.delay,
                sequence: self.sequence,
                target: output.target.clone(),
                input: output.input.clone(),
                parameter: output.parameter.clone(),
                caller: id,
                activator,
            });
        }
    }
    fn targets(&self, world: &World, p: &Pending) -> Vec<usize> {
        let target = p.target.to_lowercase();
        if target == "!self" || target == "!caller" {
            return vec![p.caller];
        }
        if target == "!activator" || target == "!player" {
            return vec![p.activator];
        }
        world
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.get("targetname")
                    .is_some_and(|n| glob(&target, &n.to_lowercase()))
            })
            .map(|(id, _)| id)
            .collect()
    }
    pub fn send(&mut self, id: usize, input: &str, parameter: &str) {
        self.sequence += 1;
        self.queue.push(Pending {
            due: self.time,
            sequence: self.sequence,
            target: format!("#{id}"),
            input: input.into(),
            parameter: parameter.into(),
            caller: id,
            activator: usize::MAX,
        });
    }
    /// Console input uses the normal deferred I/O queue and target-name matching.
    pub fn send_named(&mut self, target: &str, input: &str, parameter: &str, delay: f64) -> bool {
        if target.is_empty()
            || input.is_empty()
            || !delay.is_finite()
            || delay < 0.
            || !(self.time + delay).is_finite()
        {
            return false;
        }
        self.sequence += 1;
        self.queue.push(Pending {
            due: self.time + delay,
            sequence: self.sequence,
            target: target.into(),
            input: input.into(),
            parameter: parameter.into(),
            caller: usize::MAX,
            activator: usize::MAX,
        });
        true
    }
    pub fn use_entity(&mut self, world: &World, id: usize) {
        if world.entities.get(id).is_some_and(|e| {
            matches!(
                e.class(),
                "func_door" | "func_door_rotating" | "prop_door_rotating"
            )
        }) {
            self.send(id, "Toggle", "");
        } else if world
            .entities
            .get(id)
            .is_some_and(|e| e.class() == "func_button")
        {
            self.fire(id, "OnPressed", usize::MAX);
        }
    }
    fn deliver(&mut self, world: &World, id: usize, p: &Pending) {
        if id >= self.states.len() {
            self.unsupported_input("player", &p.input);
            return;
        }
        if self.states[id].killed {
            return;
        }
        self.diagnostics.inputs_delivered += 1;
        let e = &world.entities[id];
        let class = e.class();
        let input = p.input.to_lowercase();
        let value = p.parameter.parse::<f32>().unwrap_or(0.);
        let door = matches!(
            class,
            "func_door" | "func_door_rotating" | "prop_door_rotating" | "func_movelinear"
        );
        match input.as_str() {
            "start" if class == "logic_choreographed_scene" => {
                self.start_choreography(world, id, p.activator)
            }
            "pause" if class == "logic_choreographed_scene" => {
                if let Some(play) = self
                    .choreography
                    .get_mut(&id)
                    .and_then(|s| s.playback.as_mut())
                {
                    play.pause = Some(ScenePause::Input);
                }
            }
            "resume" if class == "logic_choreographed_scene" => {
                if let Some(play) = self
                    .choreography
                    .get_mut(&id)
                    .and_then(|s| s.playback.as_mut())
                {
                    if !matches!(play.pause, Some(ScenePause::UnsupportedControl)) {
                        play.pause = None;
                    }
                }
            }
            "cancel" if class == "logic_choreographed_scene" => self.cancel_choreography(id),
            "kill" => {
                self.states[id].killed = true;
                self.states[id].visible = false;
                self.states[id].enabled = false;
            }
            "enable" => {
                self.states[id].enabled = true;
                self.states[id].timer_at =
                    self.time + number(e, "RefireTime", 1.).max(0.015) as f64;
            }
            "disable" => self.states[id].enabled = false,
            "toggle" if !door => self.states[id].enabled = !self.states[id].enabled,
            "turnon" => self.states[id].visible = true,
            "turnoff" => self.states[id].visible = false,
            "lock" => self.states[id].locked = true,
            "unlock" => self.states[id].locked = false,
            "open" | "close" | "toggle" if door => {
                if self.states[id].locked && input != "close" {
                    self.fire(id, "OnLockedUse", p.activator);
                    return;
                }
                let target = if input == "open" {
                    1.
                } else if input == "close" {
                    0.
                } else {
                    1. - self.states[id].target
                };
                self.states[id].target = target;
                self.states[id].return_at = None;
                self.fire(
                    id,
                    if target > 0. { "OnOpen" } else { "OnClose" },
                    p.activator,
                );
            }
            "setposition" if class == "func_movelinear" => {
                self.states[id].target = value.clamp(0., 1.)
            }
            "trigger" if class == "logic_relay" => {
                if self.states[id].enabled {
                    self.fire(id, "OnTrigger", p.activator);
                    if number(e, "spawnflags", 0.) as u32 & 1 != 0 {
                        self.states[id].enabled = false;
                    }
                }
            }
            "firetimer" => self.fire(id, "OnTimer", p.activator),
            "add" | "subtract" | "setvalue" | "setvaluenofire" if class == "math_counter" => {
                let min = number(e, "min", 0.);
                let max = number(e, "max", 0.);
                let old = self.states[id].value;
                let mut next = match input.as_str() {
                    "add" => old + value,
                    "subtract" => old - value,
                    _ => value,
                };
                if max > min {
                    next = next.clamp(min, max);
                }
                self.states[id].value = next;
                if input != "setvaluenofire" {
                    self.fire_value(id, "OutValue", next, p.activator);
                    if max > min && next >= max && old < max {
                        self.fire(id, "OnHitMax", p.activator);
                    }
                    if max > min && next <= min && old > min {
                        self.fire(id, "OnHitMin", p.activator);
                    }
                }
            }
            "setvalue" | "setvaluetest" | "test" if class == "logic_branch" => {
                if input != "test" {
                    self.states[id].value = value;
                }
                if input != "setvalue" {
                    self.fire(
                        id,
                        if self.states[id].value != 0. {
                            "OnTrue"
                        } else {
                            "OnFalse"
                        },
                        p.activator,
                    );
                }
            }
            "invalue" if class == "logic_case" => {
                let matched =
                    (1..=16).find(|i| e.get(&format!("Case{i:02}")) == Some(p.parameter.as_str()));
                self.fire(
                    id,
                    &matched.map_or("OnDefault".into(), |i| format!("OnCase{i:02}")),
                    p.activator,
                );
            }
            "playsound" if class == "ambient_generic" => {
                if let Some(sound) = e.get("message") {
                    self.sounds.push(sound.into());
                }
            }
            "changelevel" if class == "trigger_changelevel" => {
                if let Some(map) = e.get("map") {
                    self.transition = Some((map.into(), e.get("landmark").unwrap_or("").into()));
                }
            }
            "sethealth" if class.starts_with("npc_") => self.states[id].value = value,
            "setdefaultanimation" if class.starts_with("prop_dynamic") => {
                // CDynamicProp stores the name without restarting or validating the active clip.
                self.states[id].default_animation = p.parameter.clone();
            }
            "setanimation" if class.starts_with("prop_dynamic") || class.starts_with("npc_") => {
                self.animate(world, id, &p.parameter, true);
            }
            "beginsequence" if class == "scripted_sequence" => self.begin_sequence(world, id),
            "cancelsequence" if class == "scripted_sequence" => self.end_sequence(world, id, true),
            "pickrandom" | "pickrandomshuffle" if class == "logic_case" => {
                let cases = self.states[id]
                    .outputs
                    .iter()
                    .filter_map(|o| {
                        o.name
                            .strip_prefix("OnCase")
                            .and_then(|n| n.parse::<usize>().ok())
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if !cases.is_empty() {
                    self.random = self
                        .random
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1);
                    let chosen = if input == "pickrandomshuffle" {
                        let state = &mut self.states[id];
                        let new_batch = state.shuffle.is_empty();
                        if new_batch {
                            state.shuffle = cases;
                        }
                        let candidates: Vec<_> = state
                            .shuffle
                            .iter()
                            .enumerate()
                            .filter(|(_, case)| {
                                !new_batch
                                    || state.shuffle.len() == 1
                                    || Some(**case) != state.last_shuffle
                            })
                            .map(|(index, _)| index)
                            .collect();
                        let index = candidates[(self.random >> 32) as usize % candidates.len()];
                        let case = state.shuffle.swap_remove(index);
                        state.last_shuffle = Some(case);
                        case
                    } else {
                        cases[(self.random >> 32) as usize % cases.len()]
                    };
                    self.fire(id, &format!("OnCase{chosen:02}"), p.activator);
                }
            }
            _ => self.unsupported_input(class, &p.input),
        }
    }
    fn animate(&mut self, world: &World, id: usize, name: &str, finish: bool) -> bool {
        let key = world
            .model_instances
            .iter()
            .find(|i| i.entity == Some(id))
            .map(|i| i.asset_key());
        let clip = key
            .as_ref()
            .and_then(|key| world.rigs.get(key))
            .and_then(|r| r.clips.get(&name.to_lowercase()));
        let Some(clip) = clip else {
            self.unsupported_input(world.entities[id].class(), &format!("animation:{name}"));
            return false;
        };
        self.states[id].animation = name.to_lowercase();
        self.states[id].scene_animation = None;
        self.states[id].animation_started = self.time;
        self.states[id].animation_done =
            finish.then_some(self.time + clip.duration().max(0.015) as f64);
        true
    }
    fn begin_sequence(&mut self, world: &World, id: usize) {
        let script = &world.entities[id];
        let name = script.get("m_iszEntity").unwrap_or("");
        let actor = world
            .entities
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                !self.states[*i].killed
                    && e.class().starts_with("npc_")
                    && (e.get("targetname") == Some(name) || e.class() == name)
            })
            .min_by(|(a, _), (b, _)| {
                self.states[*a]
                    .origin
                    .distance_squared(script.origin())
                    .total_cmp(&self.states[*b].origin.distance_squared(script.origin()))
            })
            .map(|(i, _)| i);
        let Some(actor) = actor else {
            self.unsupported_input("scripted_sequence", &format!("actor:{name}"));
            return;
        };
        if self.states[actor]
            .scripted_by
            .is_some_and(|owner| owner != id)
        {
            self.sequence += 1;
            self.queue.push(Pending {
                due: self.time + 0.25,
                sequence: self.sequence,
                target: format!("#{id}"),
                input: "BeginSequence".into(),
                parameter: String::new(),
                caller: id,
                activator: usize::MAX,
            });
            return;
        }
        let movement = number(script, "m_fMoveTo", 0.) as i32;
        if movement == 4 {
            self.states[actor].origin = script.origin();
            self.states[actor].rotation = self.states[id].rotation;
        } else if movement != 0 {
            self.unsupported_input("scripted_sequence", &format!("movement:{movement}"));
            return;
        }
        let play = script.get("m_iszPlay").unwrap_or("");
        if !self.animate(world, actor, play, false) {
            return;
        }
        let duration = world
            .model_instances
            .iter()
            .find(|i| i.entity == Some(actor))
            .and_then(|i| world.rigs.get(&i.asset_key()))
            .and_then(|r| r.clips.get(&play.to_lowercase()))
            .map(|c| c.duration())
            .unwrap_or(0.);
        self.states[actor].scripted_by = Some(id);
        self.states[id].script_actor = Some(actor);
        self.states[id].script_finish = Some(self.time + duration.max(0.015) as f64);
        self.fire(id, "OnBeginSequence", actor);
    }
    fn end_sequence(&mut self, world: &World, id: usize, cancel: bool) {
        let actor = self.states[id].script_actor.take();
        self.states[id].script_finish = None;
        self.states[id].post_idle_done = None;
        if let Some(actor) = actor {
            self.states[actor].scripted_by = None;
            let name = world.entities[id]
                .get("m_iszPostIdle")
                .filter(|s| !s.is_empty())
                .or(world.entities[actor].get("DefaultAnim"))
                .unwrap_or(if world.entities[actor].class() == "npc_metropolice" {
                    "idle_baton"
                } else {
                    "idle_subtle"
                });
            let animated = self.animate(world, actor, name, false);
            self.fire(
                id,
                if cancel {
                    "OnCancelSequence"
                } else {
                    "OnEndSequence"
                },
                actor,
            );
            if !cancel {
                let duration = if animated
                    && world.entities[id]
                        .get("m_iszPostIdle")
                        .is_some_and(|n| !n.is_empty())
                {
                    world
                        .model_instances
                        .iter()
                        .find(|i| i.entity == Some(actor))
                        .and_then(|i| world.rigs.get(&i.asset_key()))
                        .and_then(|r| r.clips.get(&name.to_lowercase()))
                        .map_or(0., |c| c.duration())
                } else {
                    0.
                };
                self.states[id].post_idle_done = Some((self.time + duration as f64, actor));
            }
        }
    }
    fn fire_value(&mut self, id: usize, name: &str, value: f32, activator: usize) {
        let before = self.sequence;
        self.fire(id, name, activator);
        for p in self
            .queue
            .iter_mut()
            .filter(|p| p.sequence > before && p.parameter.is_empty())
        {
            p.parameter = value.to_string();
        }
    }
    fn unsupported_input(&mut self, class: &str, input: &str) {
        let key = format!("{class}.{input}");
        *self.diagnostics.unsupported.entry(key.clone()).or_default() += 1;
        if self.unsupported.insert(key.clone()) {
            eprintln!("Unimplemented entity input: {key}");
        }
    }
    pub fn tick(&mut self, world: &World, player_feet: Vec3, dt: f32) {
        self.time += dt as f64;
        self.tick_choreography(world, dt);
        for id in 0..world.entities.len() {
            let e = &world.entities[id];
            if self.states[id].killed {
                continue;
            }
            if let Some((due, actor)) = self.states[id].post_idle_done {
                if due <= self.time {
                    self.states[id].post_idle_done = None;
                    self.fire(id, "OnPostIdleEndSequence", actor);
                }
            }
            if self.states[id]
                .script_finish
                .is_some_and(|t| t <= self.time)
            {
                self.end_sequence(world, id, false);
            }
            if self.states[id]
                .animation_done
                .is_some_and(|t| t <= self.time)
            {
                self.states[id].animation_done = None;
                self.fire(id, "OnAnimationDone", usize::MAX);
            }
            if matches!(
                e.class(),
                "func_door" | "func_door_rotating" | "prop_door_rotating" | "func_movelinear"
            ) {
                if self.states[id].return_at.is_some_and(|t| t <= self.time) {
                    self.send(id, "Close", "");
                    self.states[id].return_at = None;
                }
                let s = &mut self.states[id];
                if (s.target - s.fraction).abs() > 0.00001 {
                    s.fraction += (s.target - s.fraction).clamp(-s.rate * dt, s.rate * dt);
                    s.origin = s.base_origin + s.translation * s.fraction;
                    s.rotation =
                        s.base_rotation * Quat::from_axis_angle(s.axis, s.angle * s.fraction);
                    if (s.target - s.fraction).abs() < 0.00001 {
                        let open = s.target > 0.;
                        if open && s.wait >= 0. {
                            s.return_at = Some(self.time + s.wait);
                        }
                        self.diagnostics.door_completions += 1;
                        self.fire(
                            id,
                            if open { "OnFullyOpen" } else { "OnFullyClosed" },
                            usize::MAX,
                        );
                    }
                }
            }
            if e.class() == "logic_timer"
                && self.states[id].enabled
                && self.states[id].timer_at <= self.time
            {
                self.states[id].timer_at =
                    self.time + number(e, "RefireTime", 1.).max(0.015) as f64;
                self.fire(id, "OnTimer", usize::MAX);
            }
            if e.class().starts_with("trigger_") && self.states[id].enabled {
                let inside = self.player_inside(world, id, player_feet);
                if inside && !self.states[id].touching {
                    self.diagnostics.trigger_entries += 1;
                    self.fire(id, "OnStartTouch", usize::MAX);
                    self.fire(id, "OnStartTouchAll", usize::MAX);
                }
                if !inside && self.states[id].touching {
                    self.fire(id, "OnEndTouch", usize::MAX);
                    self.fire(id, "OnEndTouchAll", usize::MAX);
                }
                if inside
                    && matches!(e.class(), "trigger_once" | "trigger_multiple")
                    && self.time - self.states[id].last_trigger >= number(e, "wait", 0.2) as f64
                {
                    self.fire(id, "OnTrigger", usize::MAX);
                    self.states[id].last_trigger = self.time;
                    if e.class() == "trigger_once" {
                        self.states[id].enabled = false;
                    }
                }
                if inside && e.class() == "trigger_changelevel" && !self.states[id].touching {
                    self.send(id, "ChangeLevel", "");
                }
                self.states[id].touching = inside;
            }
        }
        for _ in 0..2048 {
            let Some(index) = self
                .queue
                .iter()
                .enumerate()
                .filter(|(_, p)| p.due <= self.time)
                .min_by(|(_, a), (_, b)| a.due.total_cmp(&b.due).then(a.sequence.cmp(&b.sequence)))
                .map(|(i, _)| i)
            else {
                return;
            };
            let p = self.queue.remove(index);
            let targets = if let Some(id) = p
                .target
                .strip_prefix('#')
                .and_then(|s| s.parse::<usize>().ok())
            {
                vec![id]
            } else {
                self.targets(world, &p)
            };
            for id in targets {
                self.deliver(world, id, &p);
            }
        }
        self.diagnostics.budget_exhaustions += 1;
    }
    fn player_inside(&self, world: &World, id: usize, feet: Vec3) -> bool {
        let e = &world.entities[id];
        let Some(model) = e
            .get("model")
            .and_then(|s| s.strip_prefix('*'))
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|id| world.brush_models.iter().find(|m| m.id == id))
        else {
            return false;
        };
        let state = &self.states[id];
        let center = feet + Vec3::Z * 36.;
        model.brushes.iter().any(|b| {
            !b.planes.is_empty()
                && b.planes.iter().all(|p| {
                    let normal = state.rotation * p.normal;
                    normal.dot(center - state.origin)
                        <= p.distance + normal.abs().dot(Vec3::new(16., 16., 36.))
                })
        })
    }
}
fn glob(pattern: &str, value: &str) -> bool {
    if let Some((first, last)) = pattern.split_once('*') {
        value.starts_with(first) && value.ends_with(last)
    } else {
        pattern == value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entity(class: &str, name: &str, values: &[(&str, &str)]) -> Entity {
        Entity {
            properties: [("classname", class), ("targetname", name)]
                .into_iter()
                .chain(values.iter().copied())
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
    fn choreography(events: &[(EventType, f32, &str)]) -> Arc<ChoreoScene> {
        let mut data = b"bvcd\x04".to_vec();
        data.extend(0u32.to_le_bytes());
        data.push(events.len() as u8);
        let mut strings = vec!["event".into(), "".into()];
        for (kind, start, parameter) in events {
            data.push(*kind as u8);
            data.extend(0i16.to_le_bytes());
            data.extend(start.to_le_bytes());
            data.extend((-1f32).to_le_bytes());
            data.extend((strings.len() as i16).to_le_bytes());
            strings.push((*parameter).into());
            data.extend(1i16.to_le_bytes());
            data.extend(1i16.to_le_bytes());
            data.extend([0, 8]);
            data.extend(0f32.to_le_bytes());
            data.extend([0, 0, 0, 0]);
            if *kind == EventType::Gesture {
                data.extend((-1f32).to_le_bytes());
            }
            data.extend([0, 0]);
            if *kind == EventType::Loop {
                data.push(255);
            }
            if *kind == EventType::Speak {
                data.push(0);
                data.extend(1i16.to_le_bytes());
                data.push(0);
            }
        }
        data.extend([0, 0, 0]);
        Arc::new(ChoreoScene::parse(&data, &strings).unwrap())
    }
    #[test]
    fn scene_stop_point_completes_once_without_cutting_off_tail() {
        let world = World {
            entities: vec![
                entity(
                    "logic_choreographed_scene",
                    "scene",
                    &[
                        ("OnStart", "started,Add,1,0,-1"),
                        ("OnCompletion", "finished,Add,1,0,-1"),
                        ("OnTrigger1", "tail,Add,1,0,-1"),
                    ],
                ),
                entity("math_counter", "started", &[]),
                entity("math_counter", "finished", &[]),
                entity("math_counter", "tail", &[]),
            ],
            ..Default::default()
        };
        let mut scene = Scene::new(&world);
        scene.install_choreography(
            0,
            choreography(&[
                (EventType::StopPoint, 0.2, "noaction"),
                (EventType::FireTrigger, 0.5, "1"),
            ]),
        );
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.25);
        assert_eq!(scene.states[1].value, 1.);
        assert_eq!(scene.states[2].value, 1.);
        assert_eq!(scene.states[3].value, 0.);
        assert!(scene.choreography[&0].playback.is_some());
        scene.tick(&world, Vec3::ZERO, 0.5);
        assert_eq!(scene.states[2].value, 1.);
        assert_eq!(scene.states[3].value, 1.);
        assert_eq!(scene.diagnostics.scene_completions, 1);
        assert!(scene.choreography[&0].playback.is_none());
    }
    #[test]
    fn scene_input_pause_resume_and_cancel_do_not_invent_completion() {
        let world = World {
            entities: vec![
                entity(
                    "logic_choreographed_scene",
                    "scene",
                    &[
                        ("OnCompletion", "count,Add,100,0,-1"),
                        ("OnCanceled", "count,Add,10,0,-1"),
                        ("OnTrigger1", "count,Add,1,0,-1"),
                    ],
                ),
                entity("math_counter", "count", &[]),
            ],
            ..Default::default()
        };
        let mut scene = Scene::new(&world);
        scene.install_choreography(
            0,
            choreography(&[
                (EventType::FireTrigger, 1., "1"),
                (EventType::StopPoint, 2., "noaction"),
            ]),
        );
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.send(0, "Pause", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        let before = scene.choreography[&0].playback.as_ref().unwrap().elapsed;
        scene.tick(&world, Vec3::ZERO, 4.);
        assert_eq!(
            scene.choreography[&0].playback.as_ref().unwrap().elapsed,
            before
        );
        assert_eq!(scene.states[1].value, 0.);
        scene.send(0, "Resume", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 1.1);
        assert_eq!(scene.states[1].value, 1.);
        scene.send(0, "Cancel", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 4.);
        assert_eq!(scene.states[1].value, 11.);
        assert_eq!(scene.diagnostics.scene_completions, 0);
    }
    #[test]
    fn scene_actor_conditions_hold_section_until_explicit_resume() {
        let world = World {
            entities: vec![
                entity(
                    "logic_choreographed_scene",
                    "scene",
                    &[
                        ("target1", "actor"),
                        ("OnTrigger1", "door,Unlock,,0,-1"),
                        ("OnTrigger1", "door,Open,,0.1,-1"),
                    ],
                ),
                entity("npc_citizen", "actor", &[]),
                entity("prop_door_rotating", "door", &[("locked", "1")]),
            ],
            ..Default::default()
        };
        let mut data = Arc::unwrap_or_clone(choreography(&[
            (EventType::MoveTo, 0., "mark"),
            (EventType::Section, 0.1, "noaction"),
            (EventType::FireTrigger, 0.2, "1"),
        ]));
        data.events[0].actor = Some(0);
        data.events[0].flags |= 1;
        data.actors.push(source_assets::scenes::Actor {
            name: "!target1".into(),
            active: true,
            channels: Vec::new(),
        });
        let mut scene = Scene::new(&world);
        scene.install_choreography(0, Arc::new(data));
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 2.);
        assert!(scene.states[2].locked);
        assert_eq!(scene.states[2].target, 0.);
        scene.tick(&world, Vec3::ZERO, 2.);
        assert!(scene.states[2].locked);
        assert!(scene
            .diagnostics
            .unsupported
            .contains_key("logic_choreographed_scene.SECTION:actor-resume-condition"));
        scene.send(0, "Resume", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 0.11);
        assert!(!scene.states[2].locked);
        assert_eq!(scene.states[2].target, 0.);
        scene.tick(&world, Vec3::ZERO, 0.11);
        assert_eq!(scene.states[2].target, 1.);
    }
    #[test]
    fn scene_sequence_uses_installed_clip_scene_clock_and_restores_baseline() {
        use modkit_core::{
            animation::{Clip, Rig},
            ModelInstance,
        };
        let world = World {
            entities: vec![
                entity(
                    "logic_choreographed_scene",
                    "scene",
                    &[("target1", "actor")],
                ),
                entity("npc_citizen", "actor", &[("DefaultAnim", "idle")]),
            ],
            model_instances: vec![ModelInstance {
                background: false,
                model: "actor.mdl".into(),
                origin: Vec3::ZERO,
                angles: Vec3::ZERO,
                skin: 2,
                scale: 1.,
                kind: "npc_citizen".into(),
                solid: false,
                solid_mode: None,
                entity: Some(1),
            }],
            rigs: BTreeMap::from([(
                "actor.mdl#2".into(),
                Rig {
                    clips: BTreeMap::from([(
                        "performance".into(),
                        Clip {
                            fps: 30.,
                            looping: false,
                            frames: vec![vec![]; 31],
                            events: Vec::new(),
                        },
                    )]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let mut data = Arc::unwrap_or_clone(choreography(&[
            (EventType::Sequence, 0., "Performance"),
            (EventType::Gesture, 0.25, "wave"),
            (EventType::StopPoint, 3., "noaction"),
        ]));
        data.actors.push(source_assets::scenes::Actor {
            name: "!target1".into(),
            active: true,
            channels: Vec::new(),
        });
        for event in &mut data.events[..2] {
            event.actor = Some(0);
            event.end = Some(2.);
        }
        let mut scene = Scene::new(&world);
        scene.install_choreography(0, Arc::new(data));
        assert_eq!(
            scene.required_animation_clips(&world),
            BTreeMap::from([(
                "actor.mdl#2".into(),
                BTreeSet::from(["performance".into(), "wave".into()])
            ),])
        );
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 0.4);
        // A gesture without layer support must not replace the actual body sequence.
        assert_eq!(scene.states[1].animation, "performance");
        assert!((scene.animation_time(1) - 0.4).abs() < 1e-6);
        scene.send(0, "Pause", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        let frozen = scene.animation_time(1);
        scene.tick(&world, Vec3::ZERO, 5.);
        assert_eq!(scene.states[1].animation, "performance");
        assert_eq!(scene.animation_time(1), frozen);
        let clocks = scene.choreography_states(&world);
        assert_eq!(clocks[0].pause_reason, Some("input"));
        assert!(clocks[0].active && clocks[0].pause);
        let samples = scene.animation_states(&world);
        assert_eq!(samples[0].scene_owner, Some(0));
        assert_eq!(samples[0].sample_time, frozen);
        assert!(samples[0].clip_loaded);
        scene.send(0, "Resume", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 0.5);
        assert!((scene.animation_time(1) - frozen - 0.5).abs() < 1e-6);
        scene.tick(&world, Vec3::ZERO, 1.1);
        assert_eq!(scene.states[1].animation, "idle");
        assert!(scene.choreography[&0].playback.is_some());
        // Cancel restores the baseline too, rather than leaving the NPC in a scene pose.
        scene.send(0, "Cancel", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 0.25);
        assert_eq!(scene.states[1].animation, "performance");
        scene.send(0, "Cancel", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        assert_eq!(scene.states[1].animation, "idle");
        let clocks = scene.choreography_states(&world);
        assert!(!clocks[0].active);
        assert!(clocks[0].elapsed.is_none());
        assert!(scene.animation_states(&world)[0].scene_owner.is_none());
        let mut missing =
            Arc::unwrap_or_clone(choreography(&[(EventType::Sequence, 0., "missing_clip")]));
        missing.actors.push(source_assets::scenes::Actor {
            name: "actor".into(),
            active: true,
            channels: Vec::new(),
        });
        missing.events[0].actor = Some(0);
        missing.events[0].end = Some(1.);
        scene.install_choreography(0, Arc::new(missing));
        scene.send(0, "Start", "");
        scene.tick(&world, Vec3::ZERO, 0.015);
        scene.tick(&world, Vec3::ZERO, 0.1);
        assert_eq!(scene.states[1].animation, "idle");
        assert!(scene
            .diagnostics
            .unsupported
            .contains_key("npc_citizen.animation:missing_clip"));
    }
    #[test]
    fn named_input_uses_deferred_glob_delivery_and_rejects_invalid_delay() {
        let world = World {
            entities: vec![
                entity("math_counter", "count_a", &[]),
                entity("math_counter", "count_b", &[]),
            ],
            ..Default::default()
        };
        let mut scene = Scene::new(&world);
        assert!(!scene.send_named("count*", "Add", "10", f64::NAN));
        assert!(!scene.send_named("count*", "Add", "10", -1.));
        assert!(scene.send_named("count*", "Add", "2", 0.1));
        assert!(scene.send_named("COUNT_A", "Add", "3", 0.1));
        scene.tick(&world, Vec3::ZERO, 0.05);
        assert_eq!(scene.states[0].value, 0.);
        scene.tick(&world, Vec3::ZERO, 0.06);
        assert_eq!(scene.states[0].value, 5.);
        assert_eq!(scene.states[1].value, 2.);
    }
    #[test]
    #[ignore = "requires an installed owned HL2 copy; isolated scene I/O, not campaign parity"]
    fn owned_security03_blocks_movement_condition_and_delivers_real_gate_after_resume() {
        let game = source_assets::install::discover().unwrap();
        let data = std::fs::read(game.join("hl2/maps/d1_trainstation_01.bsp")).unwrap();
        let bsp = source_assets::bsp::Bsp::parse(&data).unwrap();
        let world = bsp.world("d1_trainstation_01").unwrap();
        let vfs = Vfs::mount(&game).unwrap();
        let mut scene = Scene::new(&world);
        // Isolate this one scene from the unrelated camera/train introductory chain.
        scene.queue.clear();
        assert_eq!(scene.load_choreography(&world, &vfs).unwrap(), 44);
        let id = world
            .entities
            .iter()
            .position(|e| e.get("targetname") == Some("security_03"))
            .unwrap();
        let door = world
            .entities
            .iter()
            .position(|e| e.get("targetname") == Some("storage_room_door"))
            .unwrap();
        assert!(scene.states[door].locked);
        assert!(scene.send_named("security_03", "Start", "", 0.));
        for _ in 0..600 {
            scene.tick(&world, Vec3::ZERO, 0.015);
        }
        let playback = scene.choreography[&id].playback.as_ref().unwrap();
        assert!((playback.elapsed - 6.88063383102417).abs() < 1e-6);
        assert!(scene.states[door].locked);
        scene.send_named("security_03", "Resume", "", 0.);
        for _ in 0..25 {
            scene.tick(&world, Vec3::ZERO, 0.015);
        }
        assert!(!scene.states[door].locked);
        assert_eq!(scene.states[door].target, 1.);
        let parsed = &scene.choreography[&id].data;
        assert!(parsed
            .events
            .iter()
            .any(|e| e.kind == EventType::FireTrigger
                && e.parameters[0] == "3"
                && (e.start - 7.013968).abs() < 1e-6));
        assert!(parsed
            .events
            .iter()
            .any(|e| e.kind == EventType::StopPoint && (e.start - 9.59397).abs() < 1e-6));
    }
    #[test]
    fn setting_default_animation_preserves_playback_and_completion() {
        use modkit_core::{
            animation::{Clip, Rig},
            ModelInstance,
        };
        let w = World {
            entities: vec![
                entity(
                    "prop_dynamic",
                    "prop",
                    &[
                        ("DefaultAnim", "closed"),
                        ("OnAnimationDone", "result,Add,1,0,-1"),
                    ],
                ),
                entity("math_counter", "result", &[]),
            ],
            model_instances: vec![ModelInstance {
                background: false,
                model: "prop.mdl".into(),
                origin: Vec3::ZERO,
                angles: Vec3::ZERO,
                skin: 0,
                scale: 1.,
                kind: "prop_dynamic".into(),
                solid: false,
                solid_mode: None,
                entity: Some(0),
            }],
            rigs: BTreeMap::from([(
                "prop.mdl#0".into(),
                Rig {
                    clips: BTreeMap::from([(
                        "opening".into(),
                        Clip {
                            events: Vec::new(),
                            fps: 30.,
                            looping: false,
                            frames: vec![vec![]; 31],
                        },
                    )]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let mut scene = Scene::new(&w);
        scene.send(0, "SetAnimation", "opening");
        scene.tick(&w, Vec3::ZERO, 0.015);
        let started = scene.states[0].animation_started;
        let due = scene.states[0].animation_done;
        scene.tick(&w, Vec3::ZERO, 0.2);
        // Native accepts a deferred name even when no such sequence is loaded.
        scene.send(0, "SetDefaultAnimation", "DeferredCase");
        scene.tick(&w, Vec3::ZERO, 0.015);
        assert_eq!(scene.states[0].default_animation, "DeferredCase");
        assert_eq!(scene.states[0].animation, "opening");
        assert_eq!(scene.states[0].animation_started, started);
        assert_eq!(scene.states[0].animation_done, due);
        assert!(scene.diagnostics.unsupported.is_empty());
        scene.send(0, "SetDefaultAnimation", "");
        scene.tick(&w, Vec3::ZERO, 0.015);
        assert!(scene.states[0].default_animation.is_empty());
        assert_eq!(scene.states[0].animation_done, due);
        scene.tick(&w, Vec3::ZERO, 0.9);
        assert_eq!(scene.states[1].value, 1.);
        scene.tick(&w, Vec3::ZERO, 0.9);
        assert_eq!(scene.states[1].value, 1.);
    }
    #[test]
    fn shuffle_exhausts_cases_and_avoids_repeating_across_batches() {
        let w = World {
            entities: vec![
                entity(
                    "logic_case",
                    "choose",
                    &[
                        ("OnCase01", "result,SetValue,1,0,-1"),
                        ("OnCase02", "result,SetValue,2,0,-1"),
                        ("OnCase03", "result,SetValue,3,0,-1"),
                    ],
                ),
                entity("math_counter", "result", &[]),
            ],
            ..Default::default()
        };
        let mut s = Scene::new(&w);
        let mut chosen = Vec::new();
        for _ in 0..30 {
            s.send(0, "PickRandomShuffle", "");
            s.tick(&w, Vec3::ZERO, 0.015);
            chosen.push(s.states[1].value as i32);
        }
        for batch in chosen.as_chunks::<3>().0 {
            assert_eq!(
                batch.iter().copied().collect::<BTreeSet<_>>(),
                BTreeSet::from([1, 2, 3])
            );
        }
        for boundary in (3..chosen.len()).step_by(3) {
            assert_ne!(chosen[boundary - 1], chosen[boundary]);
        }
    }
    #[test]
    fn scripted_post_idle_output_waits_for_its_clip() {
        use modkit_core::{
            animation::{Clip, Rig},
            ModelInstance,
        };
        let w = World {
            entities: vec![
                entity(
                    "scripted_sequence",
                    "script",
                    &[
                        ("m_iszEntity", "actor"),
                        ("m_iszPlay", "play"),
                        ("m_iszPostIdle", "post"),
                        ("OnEndSequence", "result,SetValue,1,0,-1"),
                        ("OnPostIdleEndSequence", "result,SetValue,2,0,-1"),
                    ],
                ),
                entity("npc_citizen", "actor", &[]),
                entity("math_counter", "result", &[]),
            ],
            model_instances: vec![ModelInstance {
                background: false,
                model: "actor.mdl".into(),
                origin: Vec3::ZERO,
                angles: Vec3::ZERO,
                skin: 0,
                scale: 1.,
                kind: "npc_citizen".into(),
                solid: true,
                solid_mode: None,
                entity: Some(1),
            }],
            rigs: BTreeMap::from([(
                "actor.mdl#0".into(),
                Rig {
                    clips: BTreeMap::from([
                        (
                            "play".into(),
                            Clip {
                                events: Vec::new(),
                                fps: 30.,
                                looping: false,
                                frames: vec![vec![]; 31],
                            },
                        ),
                        (
                            "post".into(),
                            Clip {
                                events: Vec::new(),
                                fps: 30.,
                                looping: false,
                                frames: vec![vec![]; 61],
                            },
                        ),
                    ]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let mut s = Scene::new(&w);
        s.send(0, "BeginSequence", "");
        s.tick(&w, Vec3::ZERO, 0.015);
        s.tick(&w, Vec3::ZERO, 1.1);
        assert_eq!(s.states[2].value, 1.);
        s.tick(&w, Vec3::ZERO, 1.5);
        assert_eq!(s.states[2].value, 1.);
        s.tick(&w, Vec3::ZERO, 0.6);
        assert_eq!(s.states[2].value, 2.);
    }
    #[test]
    fn duplicate_outputs_delays_and_fire_limits() {
        let w = World {
            entities: vec![
                entity(
                    "logic_relay",
                    "start",
                    &[
                        ("OnTrigger", "count,Add,2,0.1,1"),
                        ("OnTrigger", "count,Add,3,0.1,-1"),
                    ],
                ),
                entity("math_counter", "count", &[]),
            ],
            ..Default::default()
        };
        let mut s = Scene::new(&w);
        s.send(0, "Trigger", "");
        s.tick(&w, Vec3::ZERO, 0.015);
        assert_eq!(s.states[1].value, 0.);
        s.tick(&w, Vec3::ZERO, 0.1);
        assert_eq!(s.states[1].value, 5.);
        s.send(0, "Trigger", "");
        s.tick(&w, Vec3::ZERO, 0.015);
        s.tick(&w, Vec3::ZERO, 0.1);
        assert_eq!(s.states[1].value, 8.);
    }
    #[test]
    fn rotating_door_lock_and_completion() {
        let w = World {
            entities: vec![entity(
                "prop_door_rotating",
                "door",
                &[("locked", "1"), ("speed", "90")],
            )],
            ..Default::default()
        };
        let mut s = Scene::new(&w);
        s.send(0, "Open", "");
        s.tick(&w, Vec3::ZERO, 0.015);
        assert_eq!(s.states[0].target, 0.);
        s.send(0, "Unlock", "");
        s.send(0, "Open", "");
        for _ in 0..70 {
            s.tick(&w, Vec3::ZERO, 0.015);
        }
        assert!((s.states[0].rotation * Vec3::X - Vec3::Y).length() < 0.001);
        assert_eq!(s.diagnostics.door_completions, 1);
    }
    #[test]
    fn escaped_output_parameters_can_contain_commas() {
        let o = output("OnTrigger", "x\x1bSetValue\x1ba,b\x1b0\x1b-1").unwrap();
        assert_eq!(o.parameter, "a,b");
    }
}
