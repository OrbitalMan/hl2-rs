//! Bevy resource bridge for retained gameplay. Owned file reads occur before App::run.
use anyhow::Result;
use bevy::prelude::Resource;
use glam::Vec3;
use hl2_simulation::{
    actors,
    entities::Scene,
    gameplay::{Inventory, Weapon},
    npc::Controller,
    physics::Physics,
    projectiles::Projectiles,
    selection::{Selection, SelectionResult},
};
use modkit_core::{
    World,
    movement::{Player, TICK},
};
use serde::Deserialize;
use source_assets::vpk::Vfs;
use std::{collections::BTreeMap, sync::Arc};

/// A muzzle flash or tracer for the effects system (muzzle_effects.rs).
#[derive(Clone, Debug)]
pub enum ShotEffect {
    NpcFlash(hl2_simulation::muzzle_effects::MuzzleKind, glam::Mat4),
    /// A muzzle elight (operator id, origin, MuzzleFlash_Combine_NPC blue).
    MuzzleLight(usize, glam::Vec3, bool),
    /// Start, end, AR2Tracer.
    NpcTracer(Vec3, Vec3, bool),
    /// Start (ComputeTracerStartPosition), end, the view's right axis, AR2Tracer.
    PlayerTracer(Vec3, Vec3, Vec3, bool),
    /// AR2Impact origin and surface normal.
    Ar2Impact(Vec3, Vec3),
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Loadout,
    Slot {
        slot: usize,
    },
    Wheel {
        delta: i32,
    },
    Confirm,
    Cancel,
    Previous,
    Reload,
    Use,
    Impulse,
    /// Seed an isolated test actor; never called by ordinary campaign play.
    ActorPose {
        target: String,
        origin: [f32; 3],
        yaw: f32,
    },
    Send {
        target: String,
        input: String,
        #[serde(default)]
        parameter: String,
    },
    /// Test only: teleport a map prop_physics body (entity id) to a pose with an
    /// optional velocity, standing in for prop_physics_create until runtime entity
    /// creation exists (DESIGN 14r); never called by ordinary campaign play.
    PropPose {
        entity: usize,
        origin: [f32; 3],
        #[serde(default)]
        angles: [f32; 3],
        #[serde(default)]
        velocity: [f32; 3],
        /// As if just launched by the gravity gun (m_bFirstCollisionAfterLaunch).
        #[serde(default)]
        launched: bool,
    },
}
impl Action {
    pub fn valid(&self) -> bool {
        match self {
            Self::ActorPose {
                target,
                origin,
                yaw,
            } => {
                !target.is_empty()
                    && target.len() <= 128
                    && origin.iter().all(|v| v.is_finite())
                    && yaw.is_finite()
            }
            Self::PropPose {
                origin,
                angles,
                velocity,
                ..
            } => origin
                .iter()
                .chain(angles)
                .chain(velocity)
                .all(|v| v.is_finite() && v.abs() < 65536.),
            Self::Slot { slot } => *slot < 6,
            Self::Wheel { delta } => (-100..=100).contains(delta),
            Self::Send {
                target,
                input,
                parameter,
            } => {
                !target.is_empty()
                    && target.len() <= 128
                    && !input.is_empty()
                    && input.len() <= 128
                    && parameter.len() <= 1024
            }
            _ => true,
        }
    }
}
/// The `ai_enable` switch across map loads: on like Source (ai_disabled 0); `--no-ai`
/// turns it off at startup, the console and the F4 overlay switch it.
pub static AI_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
#[derive(Resource)]
pub struct Gameplay {
    pub world: Arc<World>,
    pub scene: Scene,
    pub inventory: Inventory,
    pub weapons: BTreeMap<String, Weapon>,
    pub selection: Selection,
    pub npcs: Controller,
    pub projectiles: Projectiles,
    /// Gibs of broken props (runtime prop bodies until DESIGN 14r entities).
    pub gibs: hl2_simulation::gibs::Gibs,
    pub phys_explosions: hl2_simulation::phys_explosion::PhysExplosions,
    pub impacts: hl2_simulation::impacts::Impacts,
    pub primary: bool,
    pub secondary: bool,
    pub actions: Vec<Action>,
    queued_primary: bool,
    queued_secondary: bool,
    primary_consumed: bool,
    secondary_consumed: bool,
    pub unplayed_sounds: u64,
    pub sound_cues: Vec<String>,
    pub sound_requests: Vec<hl2_simulation::sounds::SoundRequest>,
    weapon_sounds: hl2_simulation::sounds::WeaponAnimationSounds,
    pub footsteps: hl2_simulation::footsteps::Footsteps,
    /// Local player damage intake (skill 2, the owner's native setting).
    pub player_damage: hl2_simulation::player_damage::PlayerDamage,
    /// Client "Damage" messages not yet shown by the HUD.
    pub damage_messages: Vec<hl2_simulation::player_damage::DamageMessage>,
    /// HEV suit voice queue.
    pub suit: hl2_simulation::suit::Suit,
    /// Step 14 NPC AI (on unless `--no-ai` / `ai_enable 0`).
    pub ai: hl2_simulation::ai::runtime::AiRuntime,
    /// Velocity impulses on the player from NPC hits (ApplyAbsVelocityImpulse), applied
    /// before the next player move.
    pub player_impulse: glam::Vec3,
    /// F4 developer NPC spawn overlay and the npc_create* console state.
    pub spawn_overlay: hl2_simulation::ai::spawn::Overlay,
    pub spawn_console: hl2_simulation::ai::spawn::Console,
    /// Spawn requests and their CC_NPC_Create placement; runtime entity creation is not
    /// implemented (DESIGN step 14 note), so nothing is created yet.
    pub spawn_log: Vec<serde_json::Value>,
    /// Muzzle flashes and tracers made by recent ticks, numbered for the effects system
    /// (which keeps the newest number it consumed); entries older than 0.5 s are dropped.
    pub shot_effects: Vec<(u64, f64, ShotEffect)>,
    pub shot_serial: u64,
    /// Active stunsticks' attachment-1 frames this tick (C_WeaponStunStick glow).
    pub stunstick_glows: Vec<glam::Mat4>,
    /// Pooled spawn entities already placed.
    pub spawn_used: std::collections::BTreeSet<usize>,
}
/// The weapons each F4 class can be given: Weapon_CanUse on its pooled spawn's
/// loaded model (every activity of the model, loaded or not).
pub fn spawn_usable_weapons(
    world: &World,
    ai: &hl2_simulation::ai::runtime::AiRuntime,
) -> BTreeMap<String, Vec<String>> {
    use hl2_simulation::ai::spawn;
    spawn::SPAWN_CLASSES
        .iter()
        .filter_map(|class| {
            let id = spawn::free_slot(world, class, &Default::default())?;
            let class_name = spawn::spawn_class(class).0;
            let instance = world
                .model_instances
                .iter()
                .find(|i| i.entity == Some(id))?;
            let rig = world.rigs.get(&instance.asset_key())?;
            Some((
                (*class).to_owned(),
                ai.usable_weapons(class_name, &rig.activities),
            ))
        })
        .collect()
}
impl Gameplay {
    /// After the player move: this tick's damage (trigger_hurt, blasts, then the
    /// landing's fall damage) through the SDK player path, then death
    /// (Event_Killed/DeathSound). Returns true when the player died this tick.
    pub fn player_outcome(&mut self, player: &mut modkit_core::movement::Player) -> bool {
        use hl2_simulation::player_damage::{DMG_FALL, DamageInfo};
        // CHL2_Player::OnTakeDamage refuses everything while gordon_invulnerable is on
        // (d1_trainstation_01-03 and parts of 04); TakeHealth is separate.
        let invulnerable = self.scene.globals.is_on("gordon_invulnerable");
        let mut infos = std::mem::take(&mut self.scene.player_damage);
        if let Some(speed) = player.landed {
            let damage = hl2_simulation::gameplay::fall_damage(speed);
            if damage > 0. {
                // CMoveHelperServer::PlayerFallingDamage: the world inflicts DMG_FALL.
                infos.push(DamageInfo {
                    amount: damage,
                    kind: DMG_FALL,
                    inflictor: Vec3::ZERO,
                });
            }
        }
        for info in infos {
            if info.amount < 0. {
                if self.inventory.health > 0. {
                    self.inventory.health = (self.inventory.health - info.amount).min(100.);
                }
            } else if !invulnerable
                && self.player_damage.take(&mut self.inventory, info) > 0.
                && info.kind & DMG_FALL != 0
                && player.landed.is_some()
            {
                self.scene.sounds.push("Player.FallDamage".into());
            }
        }
        // PlayerFallingDamage: a landing that leaves the player dead cuts to black
        // (FlPlayerFallDeathDoesScreenFade, duration 0, hold 9999, OUT | STAYOUT).
        if player
            .landed
            .is_some_and(|speed| hl2_simulation::gameplay::fall_damage(speed) > 0.)
            && self.inventory.health <= 0.
            && !player.dead
        {
            use hl2_simulation::player_damage::{FFADE_OUT, FFADE_STAYOUT, ScreenFade};
            self.player_damage.fades.push(ScreenFade::new(
                [0, 0, 0, 255],
                0.,
                9999.,
                FFADE_OUT | FFADE_STAYOUT,
            ));
        }
        let suit = self.inventory.suit;
        for sound in self.player_damage.sounds.drain(..) {
            self.scene.sounds.push(sound.into());
        }
        let now = self.scene.time;
        for update in std::mem::take(&mut self.player_damage.suit) {
            self.suit
                .update(suit, update.sentence, update.no_repeat, now);
        }
        // Weapon SetSuitUpdate requests (crossbow/RPG out of ammo).
        for (sentence, no_repeat) in std::mem::take(&mut self.inventory.suit_updates) {
            self.suit.update(suit, &sentence, no_repeat, now);
        }
        let died = !player.dead && self.inventory.health <= 0.;
        if died {
            player.dead = true;
            player.crouched = false;
            self.scene.player_dead = true;
            // DeathSound: the accumulated damage bits decide the fall splat.
            self.scene.sounds.push(
                if self.player_damage.bits() & DMG_FALL != 0 {
                    "Player.FallGib"
                } else {
                    "Player.Death"
                }
                .into(),
            );
            // DeathSound: UTIL_EmitGroupnameSuit(HEV_DEAD) at once; Event_Killed then
            // clears the queued suit sentences.
            self.suit.emit(suit, "#HEV_DEAD");
            self.suit.clear();
            // Event_Killed holsters the active weapon.
            self.inventory.previous = std::mem::take(&mut self.inventory.active);
        }
        // CheckSuitUpdate (PostThink), then UpdateClientData after the damage.
        self.suit.think(suit && !player.dead, now);
        // UTIL_EmitSoundSuit: suitvolume 0.25 (SDK default, the owner's config.cfg).
        for sentence in std::mem::take(&mut self.suit.play) {
            let name = if let Some(group) = sentence.strip_prefix('#') {
                format!("#{group}")
            } else {
                format!("!{sentence}")
            };
            self.scene
                .sounds
                .push(hl2_simulation::sounds::SoundRequest {
                    volume: Some(0.25),
                    ..name.into()
                });
        }
        if let Some(message) = self.player_damage.client_message() {
            self.damage_messages.push(message);
        }
        died
    }
    /// CategorizeGroundSurface for the next movement step: the surfaceprop of the
    /// face under the feet (the same lookup as the footsteps), or the defaults when
    /// nothing is found or the surface table is not loaded.
    pub fn ground_surface(
        &self,
        physics: &Physics,
        feet: Vec3,
    ) -> modkit_core::movement::GroundSurface {
        self.impacts
            .ground_property(&self.world, physics, &self.scene, feet, 4.)
            .and_then(|prop| self.world.surface_materials.get(&prop.to_lowercase()))
            .map(|m| modkit_core::movement::GroundSurface {
                friction: m.friction,
                max_speed_factor: m.max_speed_factor,
                jump_factor: m.jump_factor,
            })
            .unwrap_or_default()
    }
    /// SDK step sounds for one movement tick: UpdateStepSound with the state before the move,
    /// then the jump or landing step the move produced.
    pub fn step_sounds(
        &mut self,
        physics: &Physics,
        before: hl2_simulation::footsteps::State,
        feet: Vec3,
        player: &modkit_core::movement::Player,
    ) {
        let world = self.world.clone();
        let surface_at = |game: &Self, at: Vec3| {
            game.impacts
                .ground_property(&world, physics, &game.scene, at, 4.)
                .map(|prop| game.impacts.step_surface(&prop))
        };
        let mut steps = Vec::new();
        let mut footsteps = std::mem::take(&mut self.footsteps);
        steps.extend(footsteps.update(before, TICK, || surface_at(self, feet)));
        if player.jumped {
            steps.extend(footsteps.jump(surface_at(self, feet)));
        }
        if let Some(fall) = player.landed {
            steps.extend(footsteps.land(fall, || surface_at(self, player.feet)));
        }
        self.footsteps = footsteps;
        for step in steps {
            self.scene
                .sounds
                .push(hl2_simulation::sounds::SoundRequest {
                    volume: Some(step.volume),
                    ..step.sound.into()
                });
        }
    }
    #[cfg(test)]
    pub(crate) fn synthetic(world: World) -> Self {
        Self {
            scene: Scene::new(&world),
            world: Arc::new(world),
            inventory: Inventory::default(),
            weapons: BTreeMap::new(),
            selection: Selection::new(),
            npcs: Controller::new(None, Default::default()),
            projectiles: Projectiles::default(),
            gibs: Default::default(),
            phys_explosions: Default::default(),
            impacts: Default::default(),
            primary: false,
            secondary: false,
            actions: vec![],
            queued_primary: false,
            queued_secondary: false,
            primary_consumed: false,
            secondary_consumed: false,
            unplayed_sounds: 0,
            sound_cues: vec![],
            sound_requests: vec![],
            weapon_sounds: Default::default(),
            footsteps: Default::default(),
            player_damage: Default::default(),
            damage_messages: Vec::new(),
            suit: Default::default(),
            ai: hl2_simulation::ai::runtime::AiRuntime::new(&World::default(), None),
            player_impulse: glam::Vec3::ZERO,
            spawn_overlay: Default::default(),
            spawn_console: Default::default(),
            spawn_log: Vec::new(),
            shot_effects: Vec::new(),
            shot_serial: 0,
            stunstick_glows: Vec::new(),
            spawn_used: Default::default(),
        }
    }
    pub fn load_with_campaign(
        mut world: World,
        vfs: &Vfs,
        revision: u32,
        new_game: bool,
    ) -> Result<Self> {
        let mut scene = Scene::with_campaign(&world, new_game);
        scene.load_choreography(&world, vfs)?;
        actors::prepare_choreography_animations(&mut world, vfs, &scene);
        let map = world.name.clone();
        let npcs = actors::prepare_npcs(&mut world, vfs, &map, revision);
        let weapons = hl2_simulation::gameplay::definitions(vfs)?;
        actors::prepare_weapons(&mut world, vfs, &weapons)?;
        let impacts = hl2_simulation::impacts::Impacts::new(vfs, &world);
        let skill = vfs
            .read("cfg/skill.cfg")?
            .map(|b| String::from_utf8_lossy(&b).into_owned());
        let player_damage =
            hl2_simulation::player_damage::PlayerDamage::with_skill(2, skill.as_deref());
        if let Some(text) = skill.as_deref() {
            scene.skill = hl2_simulation::skill::Skill::parse(text, 2);
        }
        let mut ai = hl2_simulation::ai::runtime::AiRuntime::new(&world, Some(vfs));
        ai.enabled = AI_ENABLED.load(std::sync::atomic::Ordering::Relaxed);
        let mut spawn_overlay = hl2_simulation::ai::spawn::Overlay::default();
        spawn_overlay.set_usable(spawn_usable_weapons(&world, &ai));
        Ok(Self {
            ai,
            player_impulse: glam::Vec3::ZERO,
            spawn_overlay,
            spawn_console: Default::default(),
            spawn_log: Vec::new(),
            shot_effects: Vec::new(),
            shot_serial: 0,
            stunstick_glows: Vec::new(),
            spawn_used: Default::default(),
            player_damage,
            damage_messages: Vec::new(),
            suit: Default::default(),
            impacts,
            world: Arc::new(world),
            scene,
            inventory: Inventory::default(),
            weapons,
            selection: Selection::new(),
            npcs,
            projectiles: Projectiles::default(),
            gibs: Default::default(),
            phys_explosions: Default::default(),
            primary: false,
            secondary: false,
            actions: vec![],
            queued_primary: false,
            queued_secondary: false,
            primary_consumed: false,
            secondary_consumed: false,
            unplayed_sounds: 0,
            sound_cues: vec![],
            sound_requests: vec![],
            weapon_sounds: Default::default(),
            footsteps: Default::default(),
        })
    }
    /// An npc_create request (F4 Enter or the console): CC_NPC_Create placement from
    /// the eye along the aim, then the next hidden pooled entity of the class is
    /// moved there and shown (runtime entity creation is not available; the pool is
    /// appended at map load).
    pub fn request_spawn(
        &mut self,
        request: hl2_simulation::ai::spawn::SpawnRequest,
        physics: &Physics,
        eye: Vec3,
        forward: Vec3,
    ) -> String {
        use hl2_simulation::ai::spawn;
        let placement = spawn::place(
            &request,
            eye,
            forward,
            &hl2_simulation::ai::host::PhysicsPlacement { physics },
        );
        let mut entity = None;
        let message = match &placement {
            Ok(p) => match spawn::free_slot(&self.world, request.pool_label(), &self.spawn_used) {
                Some(id) => {
                    self.spawn_used.insert(id);
                    let state = &mut self.scene.states[id];
                    state.origin = p.origin;
                    state.visible = true;
                    state.enabled = true;
                    self.scene.ai_set_yaw(id, p.yaw);
                    self.ai
                        .set_equipment(id, &request.classname, &request.equipment);
                    self.ai
                        .set_player_disposition(id, request.allegiance.disposition());
                    // The new weapon's idle (Weapon_TranslateActivity of ACT_IDLE); the AI
                    // picks its own activities from here when it runs this class.
                    let idle = hl2_simulation::entities::weapon_idle_activity(&request.equipment);
                    let world = self.world.clone();
                    self.scene.ai_set_activity(&world, id, idle);
                    entity = Some(id);
                    format!(
                        "Created {} ({}) at {:.0} {:.0} {:.0} yaw {:.0} (entity {id})",
                        request.classname,
                        if request.equipment.is_empty() {
                            "no weapon"
                        } else {
                            &request.equipment
                        },
                        p.origin.x,
                        p.origin.y,
                        p.origin.z,
                        p.yaw
                    )
                }
                None => format!(
                    "Can't create {}: all {} pooled slots are in use",
                    request.classname,
                    spawn::pool_count(&request.classname)
                ),
            },
            Err(e) => e.clone(),
        };
        self.spawn_log.push(serde_json::json!({"request":request,"placement":placement.ok(),"entity":entity,"message":message}));
        message
    }
    fn push_shot_effect(&mut self, effect: ShotEffect) {
        self.shot_serial += 1;
        self.shot_effects
            .push((self.shot_serial, self.scene.time, effect));
    }
    fn ai_player_state(
        &self,
        player: &Player,
        fly: bool,
    ) -> hl2_simulation::ai::runtime::PlayerState {
        hl2_simulation::ai::runtime::PlayerState {
            feet: player.feet,
            velocity: player.velocity,
            crouched: player.crouched,
            on_ground: player.grounded,
            dead: player.dead,
            suit: self.inventory.suit,
            noclip: fly,
        }
    }
    pub fn consume_attacks(&mut self) {
        self.primary_consumed = true;
        self.secondary_consumed = true;
        self.primary = false;
        self.secondary = false;
        self.queued_primary = false;
        self.queued_secondary = false;
        self.actions.clear();
    }
    pub fn buttons(&mut self, primary: bool, secondary: bool) {
        self.queued_primary |=
            primary && !(self.primary || self.queued_primary) && !self.primary_consumed;
        self.queued_secondary |=
            secondary && !(self.secondary || self.queued_secondary) && !self.secondary_consumed;
        self.primary_consumed &= primary;
        self.secondary_consumed &= secondary;
        self.primary = primary;
        self.secondary = secondary;
    }
    fn select(&mut self, result: SelectionResult) {
        if let Some(name) = result.weapon {
            self.inventory.give(&name, &self.weapons, self.scene.time);
        }
        self.scene
            .sounds
            .extend(result.sounds.into_iter().map(Into::into));
    }
    pub fn tick(
        &mut self,
        physics: &mut Physics,
        player: &Player,
        eye: Vec3,
        direction: Vec3,
        fly: bool,
    ) {
        self.selection
            .tick(&self.inventory, &self.weapons, self.scene.time);
        for action in std::mem::take(&mut self.actions) {
            let result = match action {
                Action::Loadout => {
                    for name in self.weapons.keys() {
                        self.inventory.give(name, &self.weapons, self.scene.time);
                    }
                    self.inventory.refill_ammo(&self.weapons);
                    self.inventory.suit = true;
                    self.inventory
                        .give("weapon_crowbar", &self.weapons, self.scene.time);
                    continue;
                }
                Action::Slot { slot } => {
                    self.selection
                        .slot(&self.inventory, &self.weapons, slot, self.scene.time)
                }
                Action::Wheel { delta } => {
                    self.selection
                        .wheel(&self.inventory, &self.weapons, delta, self.scene.time)
                }
                Action::Confirm => {
                    let result =
                        self.selection
                            .confirm(&self.inventory, &self.weapons, self.scene.time);
                    self.primary_consumed = true;
                    self.secondary_consumed = true;
                    result
                }
                Action::Cancel => self.selection.cancel(),
                Action::Previous => self.selection.last(&self.inventory, &self.weapons),
                Action::Reload => {
                    self.inventory
                        .reload(&self.weapons, &mut self.scene, &self.world);
                    continue;
                }
                Action::Use => {
                    // CPlayerPickupController first (drop while carrying, pick up a
                    // small prop), then the existing door/button use.
                    if self.inventory.player_use(
                        &self.weapons,
                        &self.world,
                        &mut self.scene,
                        physics,
                        eye,
                        direction,
                        player.feet,
                        player.crouched,
                    ) {
                        continue;
                    }
                    if let Some((id, _)) = physics.ray(eye, direction, 96.) {
                        let state = self.ai_player_state(player, fly);
                        if !self
                            .ai
                            .player_used(id, &self.world, &mut self.scene, physics, state)
                        {
                            self.scene.use_entity_at(&self.world, id, player.feet);
                        }
                    }
                    continue;
                }
                Action::Impulse => {
                    if let Some((id, _)) = physics.ray(eye, direction, 128.) {
                        physics.impulse(id, direction, 6.);
                    }
                    continue;
                }
                Action::ActorPose {
                    target,
                    origin,
                    yaw,
                } => {
                    if !self.scene.fixture_actor_pose(
                        &self.world,
                        &target,
                        Vec3::from_array(origin),
                        yaw,
                    ) {
                        *self
                            .scene
                            .diagnostics
                            .unsupported
                            .entry(format!("fixture actor pose target missing: {target}"))
                            .or_default() += 1;
                    }
                    continue;
                }
                Action::PropPose {
                    entity,
                    origin,
                    angles,
                    velocity,
                    launched,
                } => {
                    if launched {
                        physics.launched.insert(entity);
                    }
                    if !physics.place_body(
                        entity,
                        Vec3::from_array(origin),
                        Vec3::from_array(angles),
                        Vec3::from_array(velocity),
                    ) {
                        *self
                            .scene
                            .diagnostics
                            .unsupported
                            .entry(format!("fixture prop pose: no dynamic body {entity}"))
                            .or_default() += 1;
                    }
                    continue;
                }
                Action::Send {
                    target,
                    input,
                    parameter,
                } => {
                    self.scene.send_named(&target, &input, &parameter, 0.);
                    continue;
                }
            };
            self.select(result);
        }
        if self.selection.pending.is_some()
            && ((self.primary || self.queued_primary) && !self.primary_consumed
                || (self.secondary || self.queued_secondary) && !self.secondary_consumed)
        {
            let result = self
                .selection
                .confirm(&self.inventory, &self.weapons, self.scene.time);
            if result.weapon.is_some() {
                self.primary_consumed = true;
                self.secondary_consumed = true;
            }
            self.select(result);
        }
        self.scene.tick(&self.world, player.feet, TICK);
        // npc_template_maker spawns and CAI_BaseNPC::StartNPC floor drops (DESIGN 14s).
        hl2_simulation::npc_makers::tick(
            &self.world,
            &mut self.scene,
            physics,
            (!player.dead).then_some(hl2_simulation::npc_makers::Viewer {
                eye,
                forward: direction,
                feet: player.feet,
                crouched: player.crouched,
                notarget: self.ai.ignore_player,
            }),
            TICK,
        );
        self.inventory.advance_projectile_fire(
            &self.world,
            &mut self.scene,
            &self.weapons,
            eye,
            direction,
        );
        // CBasePlayer::PostThink -> CPlayerPickupController::Use(USE_SET): held attack
        // throws, held attack2 drops; the holstered weapon does not fire meanwhile.
        let carrying = self.inventory.carrying();
        if carrying {
            self.inventory.carry_tick(
                &self.weapons,
                &mut self.scene,
                physics,
                eye,
                direction,
                player.feet,
                player.crouched,
                self.primary,
                self.secondary,
            );
            self.primary_consumed |= self.primary;
            self.secondary_consumed |= self.secondary;
        }
        let allowed = self.selection.pending.is_none() && !carrying;
        let primary = allowed && (self.primary || self.queued_primary) && !self.primary_consumed;
        let secondary = allowed
            && (self.secondary || self.queued_secondary)
            && !self.secondary_consumed
            && matches!(
                self.inventory.active.as_str(),
                "weapon_shotgun"
                    | "weapon_smg1"
                    | "weapon_ar2"
                    | "weapon_frag"
                    | "weapon_crossbow"
                    | "weapon_bugbait"
                    | "weapon_physcannon"
            );
        self.inventory.set_attack_input(primary, secondary);
        self.inventory.ducking = player.crouched;
        let attacking = primary
            || secondary
            || allowed
                && (!self.primary_consumed && self.inventory.delayed_attack
                    || !self.secondary_consumed && self.inventory.delayed_secondary_attack);
        self.inventory.tick(
            &self.world,
            &mut self.scene,
            &self.weapons,
            player.feet,
            attacking,
            TICK,
        );
        if allowed
            && !self.secondary_consumed
            && (secondary || self.inventory.delayed_secondary_attack)
        {
            self.inventory.secondary_attack(
                &self.weapons,
                &self.world,
                &mut self.scene,
                physics,
                eye,
                direction,
            );
        } else if allowed && !self.primary_consumed && (primary || self.inventory.delayed_attack) {
            let active = self.inventory.active.clone();
            let clip = self.inventory.owned.get(&active).copied();
            self.inventory.attack(
                &self.weapons,
                &self.world,
                &mut self.scene,
                physics,
                eye,
                direction,
            );
            // The weapons' PrimaryAttack: CSoundEnt::InsertSound(SOUND_COMBAT, origin,
            // SOUNDENT_VOLUME_PISTOL/MACHINEGUN/SHOTGUN 1500 (the 357: 600), 0.2).
            let volume = match active.as_str() {
                "weapon_pistol" | "weapon_smg1" | "weapon_ar2" | "weapon_shotgun" => Some(1500.),
                "weapon_357" => Some(600.),
                _ => None,
            };
            if let Some(volume) =
                volume.filter(|_| self.inventory.owned.get(&active).copied() < clip)
            {
                self.ai
                    .insert_combat_sound(player.feet, volume, self.scene.time);
            }
        }
        // weapon_physcannon: ItemPreFrame/ItemPostFrame on the Rapier props (weapons stage).
        self.inventory.physcannon_frame(
            &self.weapons,
            &self.world,
            &mut self.scene,
            physics,
            eye,
            direction,
            player.feet,
            player.crouched,
        );
        self.queued_primary = false;
        self.queued_secondary = false;
        for (id, state) in self.scene.states.iter().enumerate() {
            physics.set_entity(id, state.origin, state.rotation, state.collides());
        }
        // Studio hitboxes follow each NPC's animated skeleton (bullet hitgroups).
        hl2_simulation::gameplay::pose_npc_hitboxes(&self.world, &self.scene, physics);
        // weapon_frag throws from this tick's animation events.
        if !self.inventory.frag.pending.is_empty() {
            let ground = physics
                .ground_ray(player.feet + glam::Vec3::Z * 4., -glam::Vec3::Z, 16.)
                .map(|hit| hit.normal);
            self.inventory.launch_frags(
                &self.weapons,
                physics,
                eye,
                direction,
                player.velocity,
                player.feet,
                ground,
                self.scene.time,
            );
        }
        // weapon_bugbait ThrowGrenade from this tick's EVENT_WEAPON_THROW.
        self.inventory
            .launch_bugbait(physics, eye, direction, player.velocity, self.scene.time);
        for launch in self.inventory.projectile_spawns.drain(..) {
            self.projectiles.spawn(launch, &mut self.scene);
        }
        // CWeaponRPG::UpdateLaserPosition after the weapon frame; missiles seek it.
        self.inventory.rpg_aim(
            physics,
            &self.world,
            eye,
            direction,
            player.feet + glam::Vec3::Z * if player.crouched { 18. } else { 36. },
            player.feet,
            self.scene.time,
        );
        self.projectiles.laser_dot = self.inventory.rpg.dot;
        physics.refresh_entity_queries();
        // NPC stage: the AI thinks (when enabled) before the movement controller tick.
        let ai_player = self.ai_player_state(player, fly);
        self.ai.tick(
            &self.world,
            &mut self.scene,
            &mut self.npcs,
            physics,
            &[],
            ai_player,
        );
        self.scene.npc_knows_player = self.ai.npcs_knowing_player();
        for hit in std::mem::take(&mut self.ai.player_hits) {
            if hit.damage > 0. {
                self.scene
                    .player_damage
                    .push(hl2_simulation::player_damage::DamageInfo {
                        amount: hit.damage,
                        kind: hl2_simulation::player_damage::DMG_CLUB,
                        inflictor: self.scene.states[hit.attacker].origin,
                    });
            }
            self.player_impulse += hit.impulse;
            if let Some(fade) = hit.fade {
                self.scene.screen_fades.push(fade);
            }
            // ViewPunch needs the punch-angle path of the arsenal branch (not here).
        }
        // NPC weapons' FireBullets (DESIGN 14w): the shared hit path, the player's hull,
        // impacts and blood like the player's own shots.
        let shots = std::mem::take(&mut self.ai.shots);
        if !shots.is_empty() {
            let target = (!player.dead).then_some(hl2_simulation::gameplay::PlayerTarget {
                feet: player.feet,
                crouched: player.crouched,
                yaw: direction.y.atan2(direction.x).to_degrees(),
            });
            let mut bullets = hl2_simulation::gameplay::NpcBullets::default();
            for shot in &shots {
                hl2_simulation::gameplay::npc_fire_bullets(
                    &self.world,
                    &mut self.scene,
                    physics,
                    shot,
                    target,
                    &mut || self.ai.uniform(),
                    &mut bullets,
                );
            }
            for &(victim, attacker) in &bullets.npc_hits {
                self.ai.npc_attacked(victim, attacker);
            }
            for (inflictor, amount) in bullets.player_hits {
                self.ai.diagnostics.bullet_hits_on_player += 1;
                self.scene
                    .player_damage
                    .push(hl2_simulation::player_damage::DamageInfo {
                        amount,
                        kind: hl2_simulation::player_damage::DMG_BULLET,
                        inflictor,
                    });
            }
            for (hit, direction) in bullets.impacts {
                self.impacts
                    .add_shot(hit, direction, &self.world, physics, &mut self.scene);
            }
            for (hit, color) in bullets.blood {
                self.impacts
                    .add_blood(hit, color, &self.world, physics, &self.scene);
            }
            // ComputeTracerStartPosition: an NPC's tracer starts at its weapon's first
            // attachment (the muzzle), else at the shoot position.
            for (origin, normal) in std::mem::take(&mut bullets.ar2_impacts) {
                self.push_shot_effect(ShotEffect::Ar2Impact(origin, normal));
            }
            for (attacker, start, end, ar2) in bullets.tracers {
                let start = self
                    .ai
                    .weapon_world_model(attacker)
                    .and_then(|model| {
                        self.scene.merged_attachment_frame(
                            &self.world,
                            attacker,
                            &format!("{model}#0"),
                            None,
                            1,
                        )
                    })
                    .map_or(start, |frame| frame.w_axis.truncate());
                self.push_shot_effect(ShotEffect::NpcTracer(start, end, ar2));
            }
        }
        // AE_NPC_MUZZLEFLASH: the weapon world model's named attachment on its owner.
        for (npc, options) in std::mem::take(&mut self.ai.muzzle_flashes) {
            let Some((kind, attachment)) = hl2_simulation::muzzle_effects::parse(&options) else {
                continue;
            };
            if let Some(frame) = self.ai.weapon_world_model(npc).and_then(|model| {
                self.scene.merged_attachment_frame(
                    &self.world,
                    npc,
                    &format!("{model}#0"),
                    Some(&attachment),
                    1,
                )
            }) {
                self.push_shot_effect(ShotEffect::NpcFlash(kind, frame));
                if kind == hl2_simulation::muzzle_effects::MuzzleKind::Combine {
                    self.push_shot_effect(ShotEffect::MuzzleLight(
                        npc,
                        frame.w_axis.truncate(),
                        true,
                    ));
                }
            }
        }
        // DoMuzzleFlash -> the weapon world model's ProcessMuzzleFlashEvent at attachment 1.
        for npc in std::mem::take(&mut self.ai.weapon_muzzle_flashes) {
            if let Some(frame) = self.ai.weapon_world_model(npc).and_then(|model| {
                self.scene
                    .merged_attachment_frame(&self.world, npc, &format!("{model}#0"), None, 1)
            }) {
                self.push_shot_effect(ShotEffect::MuzzleLight(npc, frame.w_axis.truncate(), false));
            }
        }
        for (start, end, right, ar2) in std::mem::take(&mut self.inventory.tracer_requests) {
            self.push_shot_effect(ShotEffect::PlayerTracer(start, end, right, ar2));
        }
        for (origin, normal) in std::mem::take(&mut self.inventory.ar2_impacts) {
            self.push_shot_effect(ShotEffect::Ar2Impact(origin, normal));
        }
        self.stunstick_glows = self
            .ai
            .active_stunsticks()
            .into_iter()
            .filter(|id| {
                self.scene
                    .states
                    .get(*id)
                    .is_some_and(|s| s.visible && !s.killed)
            })
            .filter_map(|id| {
                let model = self.ai.weapon_world_model(id)?;
                self.scene
                    .merged_attachment_frame(&self.world, id, &format!("{model}#0"), None, 1)
            })
            .collect();
        let horizon = self.scene.time - 0.5;
        self.shot_effects.retain(|(_, at, _)| *at >= horizon);
        actors::tick_npcs(
            &mut self.npcs,
            &mut self.scene,
            &self.world,
            physics,
            player,
            fly,
        );
        // CAI_BaseActor::MaintainLookTargets random glances (0.1 s NPC think).
        hl2_simulation::attention::think(
            &mut self.scene,
            &self.world,
            physics,
            player.velocity.length() > 1.,
        );
        let mut damage = self.projectiles.tick(
            &self.world,
            &mut self.scene,
            physics,
            player.feet,
            player.crouched,
            TICK,
        );
        // Bug bait splats/squeezes are for step 14 (antlion reactions); not consumed yet.
        self.inventory.bugbait_events.clear();
        self.projectiles.bugbait_events.clear();
        // CMissile::Explode -> CWeaponRPG::NotifyRocketDied.
        for event in std::mem::take(&mut self.projectiles.missile_events) {
            self.inventory
                .rpg_missile_event(event, &self.weapons, &self.world, &mut self.scene);
        }
        // env_physexplosion pushes (and blast damage) from this tick's inputs.
        self.phys_explosions
            .apply(&self.world, &mut self.scene, physics);
        // trigger_hurt touchers from this tick's scene logic share the entity path.
        damage.append(&mut self.scene.entity_damage);
        self.inventory
            .apply_projectile_damage(damage, &self.world, &mut self.scene, physics);
        // Explosive props broken by that damage explode before their gibs spawn; chains
        // resolve within the tick (bounded).
        for _ in 0..32 {
            if self.scene.prop_explosions.is_empty() {
                break;
            }
            let blast = self.projectiles.explode_props(
                &self.world,
                &mut self.scene,
                physics,
                player.feet,
                player.crouched,
            );
            self.inventory
                .apply_projectile_damage(blast, &self.world, &mut self.scene, physics);
        }
        // PropBreakableCreateAll for props broken by this tick's damage.
        self.gibs.spawn(
            std::mem::take(&mut self.scene.prop_breaks),
            &self.world,
            physics,
            self.scene.time,
        );
        physics.tick(TICK);
        // VPhysicsShadowCollision impact damage to NPCs (queued for the next tick).
        let held = |id: usize| {
            self.inventory.physcannon.held == Some(id)
                || self
                    .inventory
                    .carry
                    .as_ref()
                    .is_some_and(|c| c.entity == id)
        };
        hl2_simulation::impact_damage::queue_npc_damage(
            &physics.resolved_collisions,
            &self.world,
            &mut self.scene,
            &held,
        );
        hl2_simulation::impact_damage::queue_prop_damage(
            &physics.resolved_collisions,
            &self.world,
            &mut self.scene,
            &mut physics.launched,
            &held,
        );
        // Gib poses and SUB_FadeOut (not while seen or held).
        self.gibs
            .tick(physics, self.scene.time, TICK, eye, direction, held);
        // VPhysics impact sounds of props and thrown frags (PhysCollisionSound).
        self.impacts
            .physics_collision_sounds(physics, &self.world, &mut self.scene);
        for event in
            self.weapon_sounds
                .events(&self.world, &self.inventory, &self.weapons, self.scene.time)
        {
            self.scene.sounds.push(event.options.into());
        }
        for (hit, _melee, direction) in self.inventory.impacts.drain(..) {
            self.impacts
                .add_shot(hit, direction, &self.world, physics, &mut self.scene);
        }
        // TraceBleed: blood on the world behind shot NPCs.
        for (hit, color) in self.inventory.blood.drain(..) {
            self.impacts
                .add_blood(hit, color, &self.world, physics, &self.scene);
        }
        // Projectile UTIL_ImpactTrace (crossbow bolts in the world).
        for hit in std::mem::take(&mut self.projectiles.impacts) {
            self.impacts.add(hit, &self.world, physics, &mut self.scene);
        }
        // Projectile UTIL_DecalTrace (the bug bait's BeerSplash).
        for (hit, material) in std::mem::take(&mut self.projectiles.decals) {
            self.impacts
                .add_decal(hit, material, &self.world, physics, &self.scene);
        }
        for sound in self.scene.sounds.drain(..) {
            self.sound_cues.push(sound.name.clone());
            if self.sound_requests.len() < 1024 {
                self.sound_requests.push(sound);
            } else {
                self.unplayed_sounds += 1;
            }
        }
        let excess = self.sound_cues.len().saturating_sub(64);
        self.sound_cues.drain(..excess);
    }
    pub fn report(&self, physics: &Physics) -> serde_json::Value {
        let entities: Vec<_> = self.world.entities.iter().enumerate()
            .filter(|(_, e)| e.class().contains("door") || e.class().starts_with("prop_physics"))
            .map(|(id, e)| {
                let state = &self.scene.states[id];
                let (origin, rotation) = physics.entity_pose(id).unwrap_or((state.origin, state.rotation));
                serde_json::json!({"entity":id,"name":e.get("targetname"),"class":e.class(),
                    "origin":origin.to_array(),"rotation":rotation.to_array(),"visible":state.visible,"killed":state.killed})
            }).collect();
        // Composition failures drop a gesture layer silently in presentation; surface them.
        let compose_errors: BTreeMap<usize, Vec<String>> = self
            .scene
            .gestures
            .report()
            .keys()
            .filter_map(|&id| {
                let instance = self
                    .world
                    .model_instances
                    .iter()
                    .find(|i| i.entity == Some(id))?;
                let rig = self.world.rigs.get(&instance.asset_key())?;
                let params = self.scene.actor_pose_values(rig, id);
                let (_, errors) = self.scene.gestures.compose(
                    rig,
                    id,
                    &self.scene.states[id].animation,
                    self.scene.animation_time(id),
                    &params,
                );
                (!errors.is_empty())
                    .then(|| (id, errors.iter().map(|e| format!("{e:?}")).collect()))
            })
            .collect();
        serde_json::json!({"time":self.scene.time,"inventory":self.inventory,"carrying":self.inventory.carrying(),"player_damage":self.player_damage,"viewmodel":{"animation":self.inventory.animation,"at":self.inventory.animation_at,"frag":self.inventory.frag},"suit":self.suit,"globals":self.scene.globals,"pending":self.selection.pending,"gesture_compose_errors":compose_errors,
            "entities":entities,"io":self.scene.diagnostics,"choreography":self.scene.choreography_states(&self.world),
            "animations":self.scene.animation_states(&self.world),"look_targets":self.scene.look_targets.report(),"random_look_targets":self.scene.look_targets.random_report(),"heads":self.scene.heads.iter().map(|(id,h)| (id.to_string(), h.goal.to_array())).collect::<std::collections::BTreeMap<_,_>>(),"gesture_layers":self.scene.gestures.report(),"monitors":self.scene.monitors,"npc_goals":self.npcs.snapshots(),
            "ai":serde_json::to_value(&self.ai).unwrap_or_else(|e| serde_json::json!({"report_error":e.to_string()})),"npc_spawns":self.spawn_log,"spawn_overlay":self.spawn_overlay,"npc_makers":{"diagnostics":self.scene.makers.diagnostics,"live":self.scene.makers.makers.values().map(|m| (m.id.to_string(), m.live.clone())).collect::<std::collections::BTreeMap<_,_>>()},"last_npc_hit":self.scene.last_npc_hit,"npc_health":self.world.entities.iter().enumerate().filter(|(id,e)| e.class().starts_with("npc_") && self.scene.states[*id].value != 0.).map(|(id,_)| (id.to_string(), self.scene.states[id].value)).collect::<std::collections::BTreeMap<_,_>>(),"phys_explosions":self.phys_explosions,"gibs":{"active":self.gibs.active,"spawned":self.gibs.spawned,"removed":self.gibs.removed,"skipped":self.gibs.skipped},"projectiles":{"active":self.projectiles.active,"effects":self.projectiles.effects,"diagnostics":self.projectiles.diagnostics},"impacts":{"studio_created":self.impacts.studio_created,"created":self.impacts.created,"unclippable":self.impacts.unclippable,"active":self.impacts.marks.len(),"errors":self.impacts.errors},"transition":self.scene.transition,
            "unplayed_sounds":self.unplayed_sounds,"queued_sounds":self.sound_requests.len(),"recent_sound_cues":self.sound_cues})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn armed() -> (Gameplay, Physics, Player) {
        let mut game = Gameplay::synthetic(World::default());
        game.weapons.insert(
            "weapon_pistol".into(),
            Weapon {
                magazine: 18,
                default_clip: 18,
                ammo_type: "Pistol".into(),
                damage: 5.,
                slot: 1,
                ..Default::default()
            },
        );
        game.inventory.suit = true;
        game.inventory.give("weapon_pistol", &game.weapons, 0.);
        game.scene.time = 1.;
        (
            game,
            Physics::new(&World::default()),
            Player::new(Vec3::Z * 64.),
        )
    }
    fn step(game: &mut Gameplay, physics: &mut Physics, player: &Player) {
        game.tick(physics, player, player.eye(), Vec3::X, false);
    }
    #[test]
    fn fast_press_and_release_between_fixed_steps_fires_once() {
        let (mut game, mut physics, player) = armed();
        game.buttons(true, false);
        game.buttons(false, false);
        step(&mut game, &mut physics, &player);
        assert_eq!(game.inventory.shots, 1);
        assert_eq!(game.inventory.owned["weapon_pistol"], 17);
        step(&mut game, &mut physics, &player);
        assert_eq!(game.inventory.shots, 1);
    }
    #[test]
    fn confirming_selection_consumes_held_attack_until_release() {
        let (mut game, mut physics, player) = armed();
        game.actions.push(Action::Slot { slot: 1 });
        game.buttons(true, false);
        step(&mut game, &mut physics, &player);
        assert!(game.selection.pending.is_none());
        assert_eq!(game.inventory.shots, 0);
        for _ in 0..30 {
            game.buttons(true, false);
            step(&mut game, &mut physics, &player);
        }
        assert_eq!(game.inventory.shots, 0);
        game.buttons(false, false);
        game.buttons(true, false);
        step(&mut game, &mut physics, &player);
        assert_eq!(game.inventory.shots, 1);
    }
    #[test]
    fn pause_capture_consumes_click_and_discards_queued_actions() {
        let (mut game, mut physics, player) = armed();
        game.buttons(true, true);
        game.actions.push(Action::Loadout);
        game.consume_attacks();
        game.buttons(true, true);
        step(&mut game, &mut physics, &player);
        assert_eq!(game.inventory.shots, 0);
        assert!(game.actions.is_empty());
        game.buttons(false, true);
        game.buttons(true, true);
        step(&mut game, &mut physics, &player);
        assert_eq!(game.inventory.shots, 1);
    }
}
