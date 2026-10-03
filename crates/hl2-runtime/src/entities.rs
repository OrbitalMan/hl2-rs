//! Deterministic map entity I/O. Unsupported inputs are reported instead of silently emulated.
use crate::physics;
use glam::{Quat, Vec3};
use modkit_core::{parse_vec3, Entity, World};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
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
    pub animation_started: f64,
    animation_done: Option<f64>,
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
                animation_done: None,
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
            "setanimation" | "setdefaultanimation"
                if class.starts_with("prop_dynamic") || class.starts_with("npc_") =>
            {
                self.animate(world, id, &p.parameter, input == "setanimation");
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
