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
    label: Option<String>,
    eye: Option<[f32; 3]>,
    yaw: Option<f32>,
    paused: Option<bool>,
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
        if c.tick <= previous
            || c.tick > 6000
            || !c.forward.is_finite()
            || !c.side.is_finite()
            || c.forward.abs() > 1.
            || c.side.abs() > 1.
            || c.eye.is_some_and(|p| p.iter().any(|n| !n.is_finite()))
            || c.yaw.is_some_and(|n| !n.is_finite())
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
}
#[derive(Resource)]
pub struct Simulation {
    pub player: Player,
    pub physics: Physics,
    eye: glam::Vec3,
    pub yaw: f32,
    pub pitch: f32,
    input: Input,
    fly: bool,
    paused: bool,
    focused: bool,
    jump_suppressed: bool,
    transition: bool,
    commands: Vec<Command>,
    next: usize,
    host_tick: u64,
    samples: Vec<Sample>,
    pub finished: bool,
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
            input: Input::default(),
            fly,
            paused: false,
            focused: true,
            jump_suppressed: false,
            transition: false,
            commands,
            next: 0,
            host_tick: 0,
            samples: vec![],
            finished: false,
        }
    }
    pub fn eye(&self) -> Vec3 {
        Vec3::from_array(self.eye.to_array())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"player": self.player, "eye": self.eye.to_array(), "fly": self.fly,
            "paused": self.paused, "host_tick": self.host_tick, "script_finished": self.finished,
            "samples": self.samples, "colliders": self.physics.colliders.len(),
            "native_convex_shapes": self.physics.native_shape_count, "native_shape_fallbacks": self.physics.native_shape_fallbacks,
            "skipped_colliders": self.physics.skipped, "dynamic_props": "frozen until entity presentation is migrated"})
    }
    fn change_fly(&mut self, fly: bool) {
        if self.fly != fly {
            self.fly = fly;
            self.player = Player::new(self.eye);
        }
    }
    fn step(&mut self) {
        if self.finished {
            return;
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
            if let Some(fly) = c.fly {
                self.change_fly(fly);
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
            label = c.label;
            self.next += 1;
        }
        if !self.paused && (self.focused || !self.commands.is_empty()) && !self.transition {
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
                // Collider poses stay frozen with the static presentation. Do not
                // advance rigid bodies without moving their visible meshes too.
                self.player.step(self.input, &self.physics, TICK);
                self.eye = self.player.eye();
            }
        }
        if let Some(label) = label {
            self.samples.push(Sample {
                label,
                host_tick: self.host_tick,
                paused: self.paused,
                fly: self.fly,
                player: self.player.clone(),
            });
        }
        self.finished = !self.commands.is_empty() && self.next == self.commands.len();
    }
}
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
            controls.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        )
        .add_systems(FixedUpdate, fixed_step)
        .add_systems(
            RunFixedMainLoop,
            present.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        );
    }
}
fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse: Res<AccumulatedMouseMotion>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut sim: ResMut<Simulation>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::F10) {
        exit.write(AppExit::Success);
    }
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if !sim.commands.is_empty() {
        return;
    }
    sim.transition = false;
    sim.focused = window.focused;
    if keys.just_pressed(KeyCode::Escape) || !window.focused {
        sim.transition = !sim.paused;
        sim.paused = true;
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        sim.jump_suppressed |= keys.pressed(KeyCode::Space);
    } else if buttons.just_pressed(MouseButton::Left) {
        sim.transition = sim.paused || cursor.grab_mode == CursorGrabMode::None;
        if sim.transition {
            sim.jump_suppressed |= keys.pressed(KeyCode::Space);
        }
        sim.paused = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if !keys.pressed(KeyCode::Space) {
        sim.jump_suppressed = false;
    }
    sim.input = Input::default();
    if sim.paused || sim.transition || cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    sim.yaw -= mouse.delta.x * 0.001152;
    sim.pitch = (sim.pitch - mouse.delta.y * 0.001152).clamp(-1.55, 1.55);
    if keys.just_pressed(KeyCode::F2) {
        let fly = !sim.fly;
        sim.change_fly(fly);
    }
    sim.input = Input {
        forward: f32::from(keys.pressed(KeyCode::KeyW)) - f32::from(keys.pressed(KeyCode::KeyS)),
        side: f32::from(keys.pressed(KeyCode::KeyD)) - f32::from(keys.pressed(KeyCode::KeyA)),
        yaw: sim.yaw,
        jump: keys.pressed(KeyCode::Space) && !sim.jump_suppressed,
        crouch: keys.pressed(KeyCode::ControlLeft),
        sprint: keys.pressed(KeyCode::ShiftLeft),
        slow: keys.pressed(KeyCode::AltLeft),
    };
}
fn fixed_step(mut sim: ResMut<Simulation>) {
    sim.step();
}
fn present(sim: Res<Simulation>, mut cameras: Query<&mut Transform, With<FlyCamera>>) {
    if let Ok(mut camera) = cameras.single_mut() {
        *camera = Transform::from_translation(source_to_bevy(sim.eye())).looking_to(
            source_to_bevy(source_direction(sim.yaw, sim.pitch)),
            Vec3::Y,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capturing_discards_pointer_motion_and_consumes_held_jump_until_release() {
        use bevy::ecs::system::RunSystemOnce;
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
        .add_message::<AppExit>();
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
        app.world_mut().run_system_once(controls).unwrap();
        let sim = app.world().resource::<Simulation>();
        assert!(sim.transition && sim.jump_suppressed);
        assert_eq!((sim.yaw, sim.pitch), (0., 0.));
        assert!(!sim.input.jump);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut().run_system_once(controls).unwrap();
        assert!(!app.world().resource::<Simulation>().input.jump);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        app.world_mut().run_system_once(controls).unwrap();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.world_mut().run_system_once(controls).unwrap();
        assert!(app.world().resource::<Simulation>().input.jump);
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
        sim.step();
        let before = sim.player.clone();
        sim.paused = true;
        for _ in 0..10 {
            sim.step();
        }
        assert_eq!(sim.player.ticks, before.ticks);
        assert_eq!(sim.player.feet, before.feet);
        assert_eq!(sim.player.velocity, before.velocity);
        sim.paused = false;
        sim.step();
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
}
