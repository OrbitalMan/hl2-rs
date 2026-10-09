//! Host adapter for the retained Source-coordinate player and collision code.
use crate::{FlyCamera, source_direction, source_to_bevy};
use anyhow::{Result, bail};
use bevy::{
    app::RunFixedMainLoopSystems,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use hl2_simulation::physics::Physics;
use modkit_core::{
    World,
    movement::{Input, Player, TICK},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    tick: u64,
    #[serde(default)]
    actions: Vec<crate::gameplay::Action>,
    #[serde(default)]
    ui: Vec<crate::console::Action>,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    secondary: bool,
    label: Option<String>,
    eye: Option<[f32; 3]>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    /// Horizontal 4:3 field of view in degrees, like the Source `fov` command.
    fov: Option<f32>,
    paused: Option<bool>,
    dev_overlay: Option<bool>,
    fly: Option<bool>,
    #[serde(default)]
    forward: f32,
    #[serde(default)]
    side: f32,
    #[serde(default)]
    jump: bool,
    #[serde(default)]
    crouch: bool,
    #[serde(default)]
    sprint: bool,
    #[serde(default)]
    slow: bool,
}
pub fn read_script(path: &Path) -> Result<Vec<Command>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > 1024 * 1024 {
        bail!("movement script exceeds 1 MiB");
    }
    let commands: Vec<Command> = serde_json::from_slice(&bytes)?;
    if commands.is_empty() || commands.len() > 1024 {
        bail!("movement script must have 1..1024 commands");
    }
    let mut previous = 0;
    for c in &commands {
        if c.ui.len() > 64
            || c.ui.iter().any(|a| !a.valid())
            || c.actions.len() > 64
            || c.actions.iter().any(|a| !a.valid())
            || c.tick <= previous
            || c.tick > 6000
            || !c.forward.is_finite()
            || !c.side.is_finite()
            || c.forward.abs() > 1.
            || c.side.abs() > 1.
            || c.eye.is_some_and(|p| p.iter().any(|n| !n.is_finite()))
            || c.yaw.is_some_and(|n| !n.is_finite())
            || c.pitch.is_some_and(|n| !(-89. ..=89.).contains(&n))
            || c.fov.is_some_and(|n| !(1. ..=170.).contains(&n))
            || c.label.as_ref().is_some_and(|s| s.len() > 128)
        {
            bail!("invalid movement command at tick {}", c.tick);
        }
        previous = c.tick;
    }
    Ok(commands)
}
#[derive(Serialize)]
struct Sample {
    label: String,
    host_tick: u64,
    paused: bool,
    fly: bool,
    player: Player,
    gameplay: Option<serde_json::Value>,
    ray_entity: Option<usize>,
    console: Option<serde_json::Value>,
}
#[derive(Resource)]
pub struct Simulation {
    pub player: Player,
    pub physics: Physics,
    eye: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Scripted horizontal 4:3 field of view in degrees (default 75).
    pub fov: f32,
    input: Input,
    fly: bool,
    pub paused: bool,
    pub loading: bool,
    pub focused: bool,
    pub jump_suppressed: bool,
    pub transition: bool,
    pub commands: Vec<Command>,
    next: usize,
    host_tick: u64,
    samples: Vec<Sample>,
    pub finished: bool,
    /// Keep simulating with idle input after the script ends (`--capture-live` bursts).
    pub run_after_script: bool,
    pub dev_overlay: bool,
    pub pending_video_config: Option<hl2_ui::config::VideoSettings>,
    /// PlayerDeathThink: every button must be released before one press respawns.
    death_buttons_released: bool,
}
impl Simulation {
    pub fn new(
        world: &World,
        eye: glam::Vec3,
        yaw: f32,
        pitch: f32,
        fly: bool,
        commands: Vec<Command>,
    ) -> Self {
        Self {
            player: Player::new(eye),
            physics: Physics::new(world),
            eye,
            yaw,
            pitch,
            fov: 75.,
            input: Input::default(),
            fly,
            paused: false,
            loading: false,
            focused: true,
            jump_suppressed: false,
            transition: false,
            commands,
            next: 0,
            host_tick: 0,
            samples: vec![],
            finished: false,
            run_after_script: false,
            dev_overlay: false,
            pending_video_config: None,
            death_buttons_released: true,
        }
    }
    pub fn eye(&self) -> Vec3 {
        Vec3::from_array(self.eye.to_array())
    }
    pub fn flying(&self) -> bool {
        self.fly
    }
    pub fn paused(&self) -> bool {
        self.paused || self.loading
    }
    pub fn loading(&self) -> bool {
        self.loading
    }
    pub fn loading_failed(&mut self) {
        self.loading = false;
        self.transition = true;
    }
    pub fn change_map(&mut self, world: &World, eye: glam::Vec3, direct: bool) {
        self.physics = Physics::new(world);
        self.eye = eye;
        self.player = Player::new(eye);
        if direct {
            self.fly = false;
            self.yaw = world.spawn().1;
            self.pitch = 0.;
            self.paused = false;
        }
        self.loading = false;
        self.transition = true;
        self.input = Input::default();
        self.jump_suppressed = true;
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"player": self.player, "eye": self.eye.to_array(), "fly": self.fly,
            "paused": self.paused(), "dev_overlay":self.dev_overlay, "dead": self.player.dead, "loading":self.loading,"host_tick": self.host_tick, "script_finished": self.finished,
            "samples": self.samples, "colliders": self.physics.colliders.len(),
            "native_convex_shapes": self.physics.native_shape_count, "native_shape_fallbacks": self.physics.native_shape_fallbacks,
            "skipped_colliders": self.physics.skipped, "dynamic_props": self.physics.dynamic.len()})
    }
    fn change_fly(&mut self, fly: bool) {
        if self.fly != fly {
            self.fly = fly;
            self.player = Player::new(self.eye);
        }
    }
    pub fn console_effects(
        &mut self,
        ui: &mut crate::console::Console,
        game: &mut crate::gameplay::Gameplay,
        effects: Vec<hl2_ui::console::Effect>,
    ) {
        use hl2_ui::console::Effect;
        for effect in effects {
            match effect {
                Effect::Loadout => {
                    for name in game.weapons.keys() {
                        game.inventory.give(name, &game.weapons, game.scene.time);
                    }
                    game.inventory.refill_ammo(&game.weapons);
                    game.inventory.suit = true;
                    game.inventory
                        .give("weapon_crowbar", &game.weapons, game.scene.time);
                    ui.source.log("Granted the six implemented weapons, ammunition and suit. Remaining HL2 weapons are not implemented.");
                }
                Effect::Noclip(value) => {
                    self.fly = value.unwrap_or(!self.fly);
                    self.player = Player::new(self.eye);
                    ui.source.log(format!("noclip {}", u8::from(self.fly)));
                }
                Effect::Getpos => ui.source.log(format!(
                    "setpos {:.6} {:.6} {:.6}; setang {:.6} {:.6} 0",
                    self.eye.x,
                    self.eye.y,
                    self.eye.z,
                    -self.pitch.to_degrees(),
                    self.yaw.to_degrees()
                )),
                Effect::Setpos { x, y, z } => {
                    self.player.feet = glam::Vec3::new(x, y, z.unwrap_or(self.player.feet.z));
                    self.eye = self.player.eye();
                    ui.source.log(format!(
                        "Player origin: {} {} {}",
                        self.player.feet.x, self.player.feet.y, self.player.feet.z
                    ));
                }
                Effect::Setang(a) => {
                    self.pitch = -a.x.to_radians().clamp(-1.53, 1.53);
                    self.yaw = a.y.to_radians();
                    ui.source.log(format!(
                        "View angles: {} {} 0",
                        -self.pitch.to_degrees(),
                        self.yaw.to_degrees()
                    ));
                }
                Effect::Map(map) => {
                    ui.map_request = Some(map);
                    self.loading = true;
                }
                Effect::Fire {
                    target,
                    input,
                    parameter,
                    delay,
                } => {
                    if game.scene.send_named(&target, &input, &parameter, delay) {
                        ui.source
                            .log(format!("Queued {target}.{input} after {delay} seconds."));
                    }
                }
                Effect::Quit => ui.quit_requested = true,
                Effect::ApplyConfig(cfg) => {
                    self.fov = cfg.video.fov;
                    self.pending_video_config = Some(cfg.video.clone());
                    ui.source.config = *cfg;
                }
            }
        }
    }
    fn step(
        &mut self,
        mut game: Option<&mut crate::gameplay::Gameplay>,
        mut ui: Option<&mut crate::console::Console>,
    ) {
        if (self.finished && !self.run_after_script) || self.loading {
            return;
        }
        if !self.commands.is_empty() {
            self.transition = false;
        }
        self.host_tick += 1;
        let mut label = None;
        if let Some(c) = self
            .commands
            .get(self.next)
            .filter(|c| c.tick == self.host_tick)
            .cloned()
        {
            if let Some(eye) = c.eye {
                self.eye = glam::Vec3::from_array(eye);
                self.player = Player::new(self.eye);
            }
            if let Some(yaw) = c.yaw {
                self.yaw = yaw.to_radians();
            }
            if let Some(pitch) = c.pitch {
                self.pitch = pitch.to_radians();
            }
            if let Some(fov) = c.fov {
                self.fov = fov;
            }
            if let Some(fly) = c.fly {
                self.change_fly(fly);
            }
            if let Some(enabled) = c.dev_overlay {
                self.dev_overlay = enabled;
            }
            if let Some(paused) = c.paused {
                self.paused = paused;
            }
            self.input = Input {
                forward: c.forward,
                side: c.side,
                yaw: self.yaw,
                jump: c.jump,
                crouch: c.crouch,
                sprint: c.sprint,
                slow: c.slow,
            };
            if let Some(game) = game.as_deref_mut() {
                game.buttons(c.primary, c.secondary);
                game.actions.extend(c.actions);
            }
            if let (Some(ui), Some(game)) = (ui.as_deref_mut(), game.as_deref_mut()) {
                let before = ui.source.mode;
                for action in &c.ui {
                    self.ui_action(ui, game, action);
                }
                if ui.source.mode != before {
                    self.paused = ui.source.paused();
                    self.transition = true;
                    self.jump_suppressed |= c.jump;
                    game.consume_attacks();
                }
            }
            if !c.jump {
                self.jump_suppressed = false;
            }
            self.input.jump &= !self.jump_suppressed;
            label = c.label;
            self.next += 1;
        }
        if (self.paused() || self.transition)
            && let Some(game) = game.as_deref_mut()
        {
            game.consume_attacks();
        }
        if !self.paused() && (self.focused || !self.commands.is_empty()) && !self.transition {
            if let Some(game) = game.as_deref_mut() {
                let direction =
                    glam::Vec3::from_array(source_direction(self.yaw, self.pitch).to_array());
                game.tick(
                    &mut self.physics,
                    &self.player,
                    self.eye,
                    direction,
                    self.fly,
                );
            }
            if self.fly {
                let forward =
                    glam::Vec3::from_array(source_direction(self.yaw, self.pitch).to_array());
                let right = glam::Vec3::new(self.yaw.sin(), -self.yaw.cos(), 0.);
                let vertical = f32::from(self.input.jump) - f32::from(self.input.crouch);
                let direction = (forward * self.input.forward
                    + right * self.input.side
                    + glam::Vec3::Z * vertical)
                    .normalize_or_zero();
                self.eye += direction * if self.input.sprint { 900. } else { 300. } * TICK;
            } else {
                let before = hl2_simulation::footsteps::State {
                    velocity: self.player.velocity,
                    grounded: self.player.grounded,
                    crouched: self.player.crouched,
                    noclip: false,
                };
                let feet = self.player.feet;
                self.player.step(self.input, &self.physics, TICK);
                self.eye = self.player.eye();
                if let Some(game) = game.as_deref_mut() {
                    game.step_sounds(&self.physics, before, feet, &self.player);
                    if game.player_outcome(&mut self.player) {
                        self.death_buttons_released = false;
                        self.eye = self.player.eye();
                    }
                }
            }
            if let Some(game) = game.as_deref_mut() {
                let world = game.world.clone();
                game.scene
                    .update_soundscape(&world, glam::Vec3::from_array(self.eye.to_array()));
            }
        }
        if self.player.dead && !self.paused() && !self.transition {
            let any_button = self.input.forward != 0.
                || self.input.side != 0.
                || self.input.jump
                || self.input.crouch
                || game.as_deref().is_some_and(|g| g.primary || g.secondary);
            if !self.death_buttons_released {
                self.death_buttons_released = !any_button;
            } else if any_button
                && let (Some(game), Some(ui)) = (game.as_deref(), ui.as_deref_mut())
            {
                // Singleplayer respawn() issues "reload": the last save, else the map again.
                // Saves are not implemented, so the current map restarts fresh.
                ui.map_request = Some(game.world.name.clone());
                self.loading = true;
            }
        }
        if game.as_ref().is_some_and(|g| g.scene.transition.is_some()) {
            self.loading = true;
        }
        if let Some(label) = label {
            self.samples.push(Sample {
                label,
                host_tick: self.host_tick,
                paused: self.paused,
                fly: self.fly,
                player: self.player.clone(),
                gameplay: game.as_deref().map(|g| g.report(&self.physics)),
                console: ui.as_deref().map(|u| u.report()),
                ray_entity: self
                    .physics
                    .ray(
                        self.eye,
                        glam::Vec3::from_array(source_direction(self.yaw, self.pitch).to_array()),
                        128.,
                    )
                    .map(|(id, _)| id),
            });
        }
        self.finished = !self.commands.is_empty() && self.next == self.commands.len();
    }
}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MovementControlsSet;

pub struct MovementPlugin;
impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        let mut virtual_time = Time::<Virtual>::default();
        virtual_time.set_max_delta(std::time::Duration::from_millis(50));
        app.insert_resource(virtual_time);
        app.insert_resource(Time::<Fixed>::from_duration(
            std::time::Duration::from_millis(15),
        ))
        .add_systems(
            RunFixedMainLoop,
            controls
                .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                .in_set(MovementControlsSet),
        )
        .add_systems(FixedUpdate, fixed_step)
        .add_systems(
            RunFixedMainLoop,
            present.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        );
    }
}
type InputDevices<'w> = (
    Res<'w, ButtonInput<KeyCode>>,
    Res<'w, ButtonInput<MouseButton>>,
    Res<'w, AccumulatedMouseMotion>,
    Option<Res<'w, bevy::input::mouse::AccumulatedMouseScroll>>,
);
pub fn parse_keycode(name: &str) -> Option<KeyCode> {
    match name {
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        "Digit0" => Some(KeyCode::Digit0),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "Space" => Some(KeyCode::Space),
        "ControlLeft" | "Ctrl" => Some(KeyCode::ControlLeft),
        "ControlRight" => Some(KeyCode::ControlRight),
        "ShiftLeft" | "Shift" => Some(KeyCode::ShiftLeft),
        "ShiftRight" => Some(KeyCode::ShiftRight),
        "AltLeft" | "Alt" => Some(KeyCode::AltLeft),
        "AltRight" => Some(KeyCode::AltRight),
        "ArrowUp" | "Up" => Some(KeyCode::ArrowUp),
        "ArrowDown" | "Down" => Some(KeyCode::ArrowDown),
        "ArrowLeft" | "Left" => Some(KeyCode::ArrowLeft),
        "ArrowRight" | "Right" => Some(KeyCode::ArrowRight),
        "Enter" => Some(KeyCode::Enter),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        "Tab" => Some(KeyCode::Tab),
        "Escape" => Some(KeyCode::Escape),
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),
        _ => None,
    }
}

pub(crate) fn action_pressed(
    action: &str,
    default_key: KeyCode,
    keys: &ButtonInput<KeyCode>,
    config: Option<&hl2_ui::config::Config>,
) -> bool {
    let key = config
        .and_then(|c| c.keys.get(action))
        .and_then(|name| parse_keycode(name))
        .unwrap_or(default_key);
    keys.pressed(key)
}

fn action_just_pressed(
    action: &str,
    default_key: KeyCode,
    keys: &ButtonInput<KeyCode>,
    config: Option<&hl2_ui::config::Config>,
) -> bool {
    let key = config
        .and_then(|c| c.keys.get(action))
        .and_then(|name| parse_keycode(name))
        .unwrap_or(default_key);
    keys.just_pressed(key)
}

fn controls(
    (keys, buttons, mouse, scroll): InputDevices,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut sim: ResMut<Simulation>,
    mut game: Option<ResMut<crate::gameplay::Gameplay>>,
    ui: Option<Res<crate::console::Console>>,
) {
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    if keys.just_pressed(KeyCode::F1) && window.focused {
        sim.dev_overlay = !sim.dev_overlay;
    }
    if !sim.commands.is_empty() {
        return;
    }
    let cfg = ui.as_ref().map(|u| &u.source.config);
    if !action_pressed("+jump", KeyCode::Space, &keys, cfg) {
        sim.jump_suppressed = false;
    }
    if let Some(game) = game.as_deref_mut() {
        game.buttons(
            buttons.pressed(MouseButton::Left),
            buttons.pressed(MouseButton::Right),
        );
    }
    sim.input = Input::default();
    if sim.paused() || sim.transition || cursor.grab_mode == CursorGrabMode::None {
        if let Some(game) = game.as_deref_mut() {
            game.consume_attacks();
        }
        return;
    }
    if let Some(game) = game.as_deref_mut() {
        use crate::gameplay::Action;
        for (action_name, default_key, action) in [
            ("loadout", KeyCode::F3, Action::Loadout),
            ("+use", KeyCode::KeyE, Action::Use),
            ("+reload", KeyCode::KeyR, Action::Reload),
            ("lastinv", KeyCode::KeyQ, Action::Previous),
            ("impulse", KeyCode::KeyG, Action::Impulse),
        ] {
            if action_just_pressed(action_name, default_key, &keys, cfg) {
                game.actions.push(action);
            }
        }
        for (slot, (action_name, default_key)) in [
            ("slot1", KeyCode::Digit1),
            ("slot2", KeyCode::Digit2),
            ("slot3", KeyCode::Digit3),
            ("slot4", KeyCode::Digit4),
            ("slot5", KeyCode::Digit5),
            ("slot6", KeyCode::Digit6),
        ]
        .into_iter()
        .enumerate()
        {
            if action_just_pressed(action_name, default_key, &keys, cfg) {
                game.actions.push(Action::Slot { slot });
            }
        }
        if let Some(scroll) = scroll.as_deref().filter(|s| s.delta.y != 0.) {
            game.actions.push(Action::Wheel {
                delta: if scroll.delta.y > 0. { -1 } else { 1 },
            });
        }
    }
    let (sensitivity, invert) = cfg
        .map(|c| (c.mouse.sensitivity, c.mouse.invert))
        .unwrap_or((3.0, false));
    let sens_scale = (sensitivity / 3.0).clamp(0.01, 10.0);
    sim.yaw -= mouse.delta.x * 0.001152 * sens_scale;
    let pitch_factor = if invert { -1.0 } else { 1.0 };
    sim.pitch =
        (sim.pitch - mouse.delta.y * 0.001152 * sens_scale * pitch_factor).clamp(-1.55, 1.55);
    if keys.just_pressed(KeyCode::F2) {
        let fly = !sim.fly;
        sim.change_fly(fly);
    }
    sim.input = Input {
        forward: f32::from(action_pressed("+forward", KeyCode::KeyW, &keys, cfg))
            - f32::from(action_pressed("+back", KeyCode::KeyS, &keys, cfg)),
        side: f32::from(action_pressed("+moveright", KeyCode::KeyD, &keys, cfg))
            - f32::from(action_pressed("+moveleft", KeyCode::KeyA, &keys, cfg)),
        yaw: sim.yaw,
        jump: action_pressed("+jump", KeyCode::Space, &keys, cfg) && !sim.jump_suppressed,
        crouch: action_pressed("+duck", KeyCode::ControlLeft, &keys, cfg),
        sprint: action_pressed("+speed", KeyCode::ShiftLeft, &keys, cfg),
        slow: action_pressed("+walk", KeyCode::AltLeft, &keys, cfg),
    };
}
fn fixed_step(
    mut sim: ResMut<Simulation>,
    mut game: Option<ResMut<crate::gameplay::Gameplay>>,
    mut ui: Option<ResMut<crate::console::Console>>,
    mut exit: MessageWriter<AppExit>,
    performance: Option<Res<crate::performance::Performance>>,
) {
    let _timing = crate::performance::scope(performance.as_deref(), "fixed_simulation");
    sim.step(game.as_deref_mut(), ui.as_deref_mut());
    if ui.is_some_and(|u| u.quit_requested) {
        exit.write(AppExit::Success);
    }
}
pub(crate) fn present(
    sim: Res<Simulation>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<FlyCamera>>,
) {
    if let Ok((mut camera, mut projection)) = cameras.single_mut() {
        *camera = Transform::from_translation(source_to_bevy(sim.eye())).looking_to(
            source_to_bevy(source_direction(sim.yaw, sim.pitch)),
            Vec3::Y,
        );
        let fov = crate::video::vertical_fov(sim.fov);
        if let Projection::Perspective(p) = &mut *projection
            && p.fov != fov
        {
            p.fov = fov;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn step_input(app: &mut App) {
        app.world_mut()
            .run_system_once(crate::ui::update_ui)
            .unwrap();
        app.world_mut().run_system_once(controls).unwrap();
    }

    #[test]
    fn capturing_discards_pointer_motion_and_consumes_held_jump_until_release() {
        let mut app = App::new();
        app.insert_resource(Simulation::new(
            &World::default(),
            glam::Vec3::new(0., 0., 128.),
            0.,
            0.,
            false,
            vec![],
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(AccumulatedMouseMotion {
            delta: Vec2::new(100., 100.),
        })
        .add_message::<AppExit>()
        .add_message::<bevy::input::keyboard::KeyboardInput>();
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            CursorOptions::default(),
            PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        step_input(&mut app);
        let sim = app.world().resource::<Simulation>();
        assert!(sim.transition && sim.jump_suppressed);
        assert_eq!((sim.yaw, sim.pitch), (0., 0.));
        assert!(!sim.input.jump);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        step_input(&mut app);
        assert!(!app.world().resource::<Simulation>().input.jump);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        step_input(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        step_input(&mut app);
        assert!(app.world().resource::<Simulation>().input.jump);
    }
    #[test]
    fn ui_open_close_and_resume_consume_pointer_attack_and_held_jump() {
        use hl2_ui::console::Mode;
        let mut app = App::new();
        app.insert_resource(Simulation::new(
            &World::default(),
            glam::Vec3::Z * 128.,
            0.,
            0.,
            false,
            vec![],
        ))
        .insert_resource(crate::gameplay::Gameplay::synthetic(World::default()))
        .insert_resource(crate::console::Console::new(Default::default()))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(AccumulatedMouseMotion {
            delta: Vec2::splat(100.),
        })
        .add_message::<AppExit>()
        .add_message::<bevy::input::keyboard::KeyboardInput>();
        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            CursorOptions::default(),
            PrimaryWindow,
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Backquote);
        step_input(&mut app);
        assert_eq!(
            app.world()
                .resource::<crate::console::Console>()
                .source
                .mode,
            Mode::Console
        );
        assert!(app.world().resource::<Simulation>().paused);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        step_input(&mut app);
        let sim = app.world().resource::<Simulation>();
        assert!(!sim.paused && sim.transition && sim.jump_suppressed && !sim.input.jump);
        assert_eq!((sim.yaw, sim.pitch), (0., 0.));
        assert!(!app.world().resource::<crate::gameplay::Gameplay>().primary);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .source
            .mode = Mode::Pause;
        step_input(&mut app);
        assert!(app.world().resource::<Simulation>().paused);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        step_input(&mut app);
        assert!(
            app.world().resource::<Simulation>().paused,
            "click outside Resume cannot close the menu"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        step_input(&mut app);
        assert!(app.world().resource::<Simulation>().transition);
        assert!(!app.world().resource::<Simulation>().paused);
    }
    #[test]
    fn console_setpos_preserves_velocity_and_getpos_uses_eye() {
        let mut sim = Simulation::new(
            &World::default(),
            glam::Vec3::Z * 128.,
            0.,
            0.,
            false,
            vec![],
        );
        let mut game = crate::gameplay::Gameplay::synthetic(World::default());
        let mut ui = crate::console::Console::new(Default::default());
        sim.player.velocity = glam::Vec3::new(1., 2., 3.);
        sim.ui_action(
            &mut ui,
            &mut game,
            &crate::console::Action::Command {
                command:
                    "sv_cheats 1; setpos 10 20 30; setang 15 90; getpos; ent_fire missing Open"
                        .into(),
            },
        );
        assert_eq!(sim.player.feet, glam::Vec3::new(10., 20., 30.));
        assert_eq!(sim.player.velocity, glam::Vec3::new(1., 2., 3.));
        assert_eq!(sim.eye, glam::Vec3::new(10., 20., 94.));
        let position = ui
            .source
            .output
            .iter()
            .find(|s| s.starts_with("setpos 10.000000 20.000000 94.000000; setang "))
            .expect("camera origin log");
        let angles: Vec<_> = position
            .split("setang ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .collect();
        assert!((angles[0].parse::<f32>().unwrap() - 15.).abs() < 1e-4);
        assert!((angles[1].parse::<f32>().unwrap() - 90.).abs() < 1e-4);
    }
    #[test]
    fn pause_does_not_advance_player_and_resume_retains_fixed_tick() {
        let mut sim = Simulation::new(
            &World::default(),
            glam::Vec3::new(0., 0., 128.),
            0.,
            0.,
            false,
            vec![],
        );
        sim.step(None, None);
        let before = sim.player.clone();
        sim.paused = true;
        for _ in 0..10 {
            sim.step(None, None);
        }
        assert_eq!(sim.player.ticks, before.ticks);
        assert_eq!(sim.player.feet, before.feet);
        assert_eq!(sim.player.velocity, before.velocity);
        sim.paused = false;
        sim.step(None, None);
        assert_eq!(sim.player.ticks, before.ticks + 1);
    }
    #[test]
    fn movement_schedule_uses_fifteen_milliseconds() {
        let mut app = App::new();
        app.add_plugins(MovementPlugin);
        assert_eq!(
            app.world().resource::<Time<Fixed>>().timestep(),
            std::time::Duration::from_millis(15)
        );
    }
    #[test]
    fn config_key_bindings_and_mouse_sensitivity_are_respected() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        let mut sim = Simulation::new(
            &World::default(),
            glam::Vec3::Z * 128.,
            0.,
            0.,
            false,
            vec![],
        );
        sim.focused = true;
        let mut custom_config = hl2_ui::config::Config::default();
        custom_config.mouse.sensitivity = 6.0;
        custom_config.mouse.invert = true;
        custom_config.keys.insert("+forward".into(), "KeyI".into());
        custom_config.keys.insert("+jump".into(), "KeyJ".into());

        let mut ui = crate::console::Console::new(Default::default());
        ui.source.config = custom_config;

        app.insert_resource(sim)
            .insert_resource(ui)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(AccumulatedMouseMotion {
                delta: Vec2::new(50., 50.),
            })
            .add_message::<AppExit>()
            .add_message::<bevy::input::keyboard::KeyboardInput>();

        app.world_mut().spawn((
            Window {
                focused: true,
                ..default()
            },
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                visible: false,
                ..default()
            },
            PrimaryWindow,
        ));

        // Press remapped keys KeyI and KeyJ
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyI);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyJ);

        app.world_mut().run_system_once(controls).unwrap();

        let sim = app.world().resource::<Simulation>();
        assert_eq!(sim.input.forward, 1.0);
        assert!(sim.input.jump);
        // With 6.0 sensitivity (scale 2.0) and inverted pitch:
        // yaw: -50 * 0.001152 * 2.0 = -0.1152
        // pitch: 0 - 50 * 0.001152 * 2.0 * (-1.0) = +0.1152
        assert!((sim.yaw - (-0.1152)).abs() < 1e-4);
        assert!((sim.pitch - 0.1152).abs() < 1e-4);
    }
    #[test]
    fn config_effect_updates_simulation_fov_and_console_config() {
        let mut sim = Simulation::new(
            &World::default(),
            glam::Vec3::Z * 128.,
            0.,
            0.,
            false,
            vec![],
        );
        let mut game = crate::gameplay::Gameplay::synthetic(World::default());
        let mut ui = crate::console::Console::new(Default::default());
        assert_eq!(sim.fov, 75.0);

        let mut new_config = ui.source.config.clone();
        new_config.video.fov = 90.0;
        new_config.audio.volume = 0.42;

        sim.console_effects(
            &mut ui,
            &mut game,
            vec![hl2_ui::console::Effect::ApplyConfig(Box::new(
                new_config.clone(),
            ))],
        );

        assert_eq!(sim.fov, 90.0);
        assert_eq!(ui.source.config.video.fov, 90.0);
        assert_eq!(ui.source.config.audio.volume, 0.42);
    }
}
