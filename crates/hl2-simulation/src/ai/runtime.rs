//! The AI think loop in the NPC stage (CAI_BaseNPC::NPCThink every 0.1 s): per-NPC
//! GatherConditions (sight of the player and hated NPCs, hearing, enemy memory and
//! COND_NEW_ENEMY/SEE_ENEMY/ENEMY_OCCLUDED/LOST_ENEMY), SelectIdealState and
//! MaintainSchedule through the class behavior. Switched by `enabled` (the host's
//! `ai_enable`, on by default like Source's ai_disabled 0). NPCs owned by a
//! scripted_sequence or a choreographed scene are NPC_STATE_SCRIPT and the AI leaves
//! them alone. Simplified from the SDK: every NPC thinks every 0.1 s (no PVS/efficiency
//! throttling, no think staggering). Class layers: npc_metropolice and the humanoids
//! (npc_citizen, npc_barney, npc_alyx, npc_combine_s); other classes have no AI yet.
use super::assault;
use super::conditions::*;
use super::host::{yaw_of, PhysicsSight, SceneMotor};
use super::humanoid::{self, Humanoid};
use super::metropolice::{CopRequest, Metropolice, PlayerView, PLAYER};
use super::police::{PoliceGoal, Policing};
use super::relationships::{classify, Class, Disposition, Relationships};
use super::schedule::{AiNpc, Behavior, Context, Schedules};
use super::senses::{self, SightWorld, SoundEnt, Viewer};
use super::state::{select_ideal_state, NpcState, StateInputs};
use crate::{
    entities::Scene,
    npc::{Controller, GoalKey},
    npc_probe::ActorHull,
    npc_weapons::{self, NpcShot, NpcWeapon, NpcWeaponDef, WeaponEvent},
    physics::Physics,
};
use glam::Vec3;
use modkit_core::World;
use serde::Serialize;
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::BTreeMap;

/// NPCThink's default interval.
pub const THINK_INTERVAL: f64 = 0.1;
/// SOUND_* interests of npc_metropolice (GetSoundInterests; PLAYER_VEHICLE has no
/// CSoundEnt type here).
const METROPOLICE_SOUNDS: u32 = senses::sound::WORLD
    | senses::sound::COMBAT
    | senses::sound::PLAYER
    | senses::sound::DANGER
    | senses::sound::PHYSICS_DANGER
    | senses::sound::BULLET_IMPACT
    | senses::sound::MOVE_AWAY;
/// m_flFieldOfView of npc_metropolice.
const METROPOLICE_FOV: f32 = -0.2;
/// NPC_Metropolice sentences take their volume/soundlevel from this sound script entry.
pub const METROPOLICE_SENTENCE_PARAMETERS: &str = "NPC_Metropolice.SentenceParameters";

/// The sound script entry holding an NPC sentence's volume/soundlevel/pitch
/// (CAI_Sentence::Init), by sentence or group name ("!NAME" / "#GROUP").
pub fn sentence_parameters(sentence: &str) -> Option<&'static str> {
    let name = sentence.trim_start_matches(['!', '#']).to_ascii_uppercase();
    name.starts_with("METROPOLICE_")
        .then_some(METROPOLICE_SENTENCE_PARAMETERS)
}
/// What the host knows about the local player this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerState {
    pub feet: Vec3,
    pub velocity: Vec3,
    pub crouched: bool,
    pub on_ground: bool,
    pub dead: bool,
    pub suit: bool,
    /// flying/noclip players are not sensed.
    pub noclip: bool,
}
impl PlayerState {
    fn center(&self) -> Vec3 {
        self.feet + Vec3::Z * if self.crouched { 18. } else { 36. }
    }
    fn eye(&self) -> Vec3 {
        self.feet + Vec3::Z * if self.crouched { 28. } else { 64. }
    }
}
/// A hit on the player for the host (damage through the player damage path, the
/// velocity impulse, the view punch and the screen fade).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayerHit {
    pub attacker: usize,
    pub damage: f32,
    pub punch: Vec3,
    pub impulse: Vec3,
    pub fade: Option<crate::player_damage::ScreenFade>,
}
/// A melee swing that struck an NPC (CTraceFilterMelee: DMG_CLUB on a hated NPC); the
/// host applies it through the shared hit path.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MeleeHit {
    pub attacker: usize,
    pub victim: usize,
    pub damage: f32,
    pub position: Vec3,
    pub direction: Vec3,
}
#[derive(Clone, Debug, Serialize)]
pub enum AiClass {
    Metropolice(Box<Metropolice>),
    Humanoid(Box<Humanoid>),
}
impl AiClass {
    pub fn cop(&self) -> Option<&Metropolice> {
        match self {
            Self::Metropolice(cop) => Some(cop),
            _ => None,
        }
    }
    pub fn cop_mut(&mut self) -> Option<&mut Metropolice> {
        match self {
            Self::Metropolice(cop) => Some(cop),
            _ => None,
        }
    }
    pub fn humanoid(&self) -> Option<&Humanoid> {
        match self {
            Self::Humanoid(h) => Some(h),
            _ => None,
        }
    }
    /// The hosted CAI_AssaultBehavior.
    pub fn assault(&self) -> Option<&assault::Assault> {
        match self {
            Self::Metropolice(cop) => cop.assault.as_ref(),
            Self::Humanoid(h) => h.assault.as_ref(),
        }
    }
    pub fn assault_mut(&mut self) -> &mut Option<assault::Assault> {
        match self {
            Self::Metropolice(cop) => &mut cop.assault,
            Self::Humanoid(h) => &mut h.assault,
        }
    }
    /// The aiscripted_schedule goal (position, run).
    pub fn script_goal(&self) -> Option<(Vec3, bool)> {
        match self {
            Self::Metropolice(cop) => cop.script_goal,
            Self::Humanoid(h) => h.script_goal,
        }
    }
    fn script_goal_mut(&mut self) -> &mut Option<(Vec3, bool)> {
        match self {
            Self::Metropolice(cop) => &mut cop.script_goal,
            Self::Humanoid(h) => &mut h.script_goal,
        }
    }
    fn requests_mut(&mut self) -> &mut Vec<CopRequest> {
        match self {
            Self::Metropolice(cop) => &mut cop.requests,
            Self::Humanoid(h) => &mut h.requests,
        }
    }
    fn behavior_mut(&mut self) -> &mut dyn Behavior {
        match self {
            Self::Metropolice(cop) => cop.as_mut(),
            Self::Humanoid(h) => h.as_mut(),
        }
    }
    /// m_flFieldOfView.
    fn field_of_view(&self) -> f32 {
        match self {
            Self::Metropolice(_) => METROPOLICE_FOV,
            Self::Humanoid(h) => h.kind.field_of_view(),
        }
    }
    /// GetSoundInterests.
    fn sound_interests(&self) -> u32 {
        match self {
            Self::Metropolice(_) => METROPOLICE_SOUNDS,
            Self::Humanoid(h) => h.kind.sound_interests(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct AiActor {
    pub npc: AiNpc,
    pub class: AiClass,
    pub next_think: f64,
    pub last_think: f64,
    /// The active weapon (Weapon_Equip of `additionalequipment`; DESIGN 14w).
    pub weapon: Option<NpcWeapon>,
    /// The F4 per-spawn disposition toward the player (an ai_relationship aimed at
    /// this NPC); None: the class rules.
    pub player_disposition: Option<Disposition>,
    /// The origin and body yaw (degrees) at the last think (report only).
    pub origin: [f32; 3],
    pub yaw: f32,
    /// An NPC that shot this one since the last think (OnTakeDamage: the attacker).
    pub damaged_by: Option<usize>,
    #[serde(skip)]
    goal: Option<GoalKey>,
    /// Animation events up to this time were handled.
    events_until: f64,
    /// The ACT_IDLE -> ACT_IDLE_ANGRY translation input at the last think.
    idle_angry: bool,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Diagnostics {
    pub thinks: u64,
    /// Melee swings that struck an NPC.
    pub npc_melee_hits: u64,
    pub scripted_skips: u64,
    pub sentences: u64,
    /// Sentences FOkToMakeSound refused (equal or lower priority while speaking).
    pub culled_sentences: u64,
    pub player_hits: u64,
    pub unknown_goals: BTreeMap<String, u32>,
    /// AI move requests the npc Controller refused, by reason.
    pub blocked_moves: BTreeMap<String, u32>,
    /// NPC weapon shots fired and pellets that struck the player (host-counted).
    pub shots_fired: u64,
    pub bullet_hits_on_player: u64,
}
#[derive(Serialize)]
pub struct AiRuntime {
    /// The `ai_enable` switch (the host turns it on by default, like Source).
    pub enabled: bool,
    #[serde(skip)]
    schedules: Schedules,
    #[serde(skip)]
    relationships: Relationships,
    pub sounds: SoundEnt,
    pub actors: BTreeMap<usize, AiActor>,
    /// ai_goal_police entities by id.
    pub goals: BTreeMap<usize, PoliceGoal>,
    serial: usize,
    /// weapon_stunstick script SoundData (melee_hit, melee_miss).
    stunstick_sounds: (Option<String>, Option<String>),
    /// sk_npc_dmg_stunstick from skill.cfg. The runtime is built on the map-load worker
    /// thread, so `tick` hands it to the simulation thread's STUNSTICK_DAMAGE.
    #[serde(skip)]
    stunstick_damage: Option<f32>,
    /// NPC weapon definitions (installed scripts and skill.cfg).
    #[serde(skip)]
    weapon_defs: BTreeMap<String, NpcWeaponDef>,
    /// FireBullets requests from NPC weapons this tick, for the host's bullet path.
    pub shots: Vec<NpcShot>,
    /// AE_NPC_MUZZLEFLASH events (NPC, options) for the host's effects.
    #[serde(skip)]
    pub muzzle_flashes: Vec<(usize, String)>,
    /// Operators whose weapon called DoMuzzleFlash this tick (CBaseCombatCharacter ->
    /// the active weapon's world model: every NPC gun except the AR2).
    pub weapon_muzzle_flashes: Vec<usize>,
    /// Hits on the player for the host to apply.
    pub player_hits: Vec<PlayerHit>,
    /// Melee hits on NPCs this tick (stunstick swings).
    pub melee_hits: Vec<MeleeHit>,
    pub diagnostics: Diagnostics,
    random: u64,
    /// Debug (F4): the AI does not sense the player (ai_ignoreplayers / notarget).
    pub ignore_player: bool,
    /// Active weapons of NPCs whose class has no AI behavior here (Weapon_Equip from
    /// additionalequipment or F4): held and drawn, never fired.
    pub held_weapons: BTreeMap<usize, NpcWeapon>,
    /// The AI ran last tick (switching it off returns NPCs to idle once).
    #[serde(skip)]
    was_enabled: bool,
    /// CAI_BaseNPC m_flSoundWaitTime / m_nSoundPriority per NPC (FOkToMakeSound).
    #[serde(skip)]
    sound_wait: BTreeMap<usize, (f64, i32)>,
    /// CRallyPoint::Lock: rally point entity -> the NPC holding it.
    pub rally_locks: BTreeMap<usize, usize>,
    /// CAssaultPoint::m_flTimeLastUsed by entity.
    #[serde(skip)]
    assault_used: BTreeMap<usize, f64>,
    /// StartActive ai_goal_assault entities were activated.
    #[serde(skip)]
    goals_started: bool,
    /// aiscripted_schedule controllers that found their actor (m_bDidFireOnce).
    #[serde(skip)]
    scripted_fired: std::collections::BTreeSet<usize>,
    /// F4 per-spawn allegiance toward the player of any NPC (AI or not): its side in
    /// NPC-vs-NPC fights (DESIGN 14n step 1).
    pub allegiances: BTreeMap<usize, Disposition>,
    /// Entities whose class is npc_* (sight candidates), built on first use.
    #[serde(skip)]
    npc_ids: Option<Vec<usize>>,
    /// ai_relationship overrides (subject, target) -> disposition (target PLAYER for
    /// "!player"), and the pairs each active ai_relationship entity applied.
    #[serde(serialize_with = "serialize_pairs")]
    pub relationship_overrides: BTreeMap<(usize, usize), Disposition>,
    #[serde(skip)]
    applied_relationships: BTreeMap<usize, Vec<(usize, usize)>>,
}
/// A pair-keyed map as a list of (subject, target, value) for the JSON report.
fn serialize_pairs<S: serde::Serializer, V: Serialize>(
    map: &BTreeMap<(usize, usize), V>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(map.iter().map(|((a, b), v)| (a, b, v)))
}
/// The yaw of an entity's "angles" keyvalue (degrees).
fn yaw_keyvalue(e: &modkit_core::Entity) -> f32 {
    modkit_core::parse_vec3(e.get("angles").unwrap_or("0 0 0")).map_or(0., |a| a.y)
}
/// SentencePriority_t of the metrocop sentence groups as npc_metropolice.cpp speaks
/// them (-1 = SENTENCE_PRIORITY_INVALID: plays without the sound timer).
fn sentence_priority(group: &str) -> i32 {
    const HIGH: [&str; 12] = [
        "METROPOLICE_PLAYERHIT",
        "METROPOLICE_HES_RUNNING",
        "METROPOLICE_TAKE_HIM_DOWN",
        "METROPOLICE_REFIND_ENEMY",
        "METROPOLICE_DANGER",
        "METROPOLICE_FT_MOUNT",
        "METROPOLICE_FT_DISMOUNT",
        "METROPOLICE_SO_BEGIN",
        "METROPOLICE_SO_END",
        // TASK_SPEAK_SENTENCE -> SpeakSentence (HIGH).
        "METROPOLICE_ACTIVATE_BATON",
        "METROPOLICE_DEACTIVATE_BATON",
        "METROPOLICE_MOVE_ALONG",
    ];
    const MEDIUM: [&str; 9] = [
        "METROPOLICE_BACK_UP",
        "METROPOLICE_FREEZE",
        "METROPOLICE_OVER_HERE",
        "METROPOLICE_ARREST_IN_POS",
        "METROPOLICE_HEARD_SOMETHING",
        "METROPOLICE_LAST_OF_SQUAD",
        "METROPOLICE_MAN_DOWN",
        "METROPOLICE_FT_APPROACH",
        "METROPOLICE_AS_ADV",
    ];
    if group.starts_with("METROPOLICE_PAIN") || group == "METROPOLICE_DIE" {
        -1
    } else if HIGH.iter().any(|g| group.starts_with(g)) {
        2
    } else if MEDIUM.iter().any(|g| group.starts_with(g)) {
        1
    } else {
        0
    }
}
impl AiRuntime {
    /// The AI for the map's NPCs (metrocops) and ai_goal_police entities. `vfs` reads
    /// skill.cfg (sk_npc_dmg_stunstick) and the stunstick's sound script.
    pub fn new(world: &World, vfs: Option<&Vfs>) -> Self {
        let mut schedules = Schedules::default();
        super::police::add_schedules(&mut schedules);
        super::metropolice::add_schedules(&mut schedules);
        humanoid::add_schedules(&mut schedules);
        let mut stunstick_sounds = (None, None);
        let mut stunstick_damage = None;
        if let Some(vfs) = vfs {
            if let Ok(Some(skill)) = vfs.read("cfg/skill.cfg") {
                if let Ok(tokens) = keyvalues::tokens(&String::from_utf8_lossy(&skill)) {
                    if let Some(value) = tokens
                        .windows(2)
                        .find(|p| p[0].eq_ignore_ascii_case("sk_npc_dmg_stunstick"))
                        .and_then(|p| p[1].parse::<f32>().ok())
                    {
                        super::metropolice::STUNSTICK_DAMAGE.with(|d| d.set(value));
                        stunstick_damage = Some(value);
                    }
                }
            }
            if let Ok(Some(script)) = vfs.read("scripts/weapon_stunstick.txt") {
                if let Ok(entries) = keyvalues::parse(&String::from_utf8_lossy(&script)) {
                    let sounds = entries.first().and_then(|r| r.get("SoundData"));
                    let get = |key: &str| {
                        sounds
                            .and_then(|s| s.get(key))
                            .and_then(|e| e.text())
                            .map(str::to_owned)
                    };
                    stunstick_sounds = (get("melee_hit"), get("melee_miss"));
                }
            }
        }
        let weapon_defs = vfs
            .and_then(|vfs| npc_weapons::definitions(vfs).ok())
            .unwrap_or_default();
        let mut actors = BTreeMap::new();
        let mut goals = BTreeMap::new();
        let mut held_weapons = BTreeMap::new();
        for (id, e) in world.entities.iter().enumerate() {
            if let Some(actor) = Self::actor_for(id, e, &weapon_defs) {
                actors.insert(id, actor);
                continue;
            }
            if let Some(weapon) = Self::equip(
                e.class(),
                e.get("additionalequipment").unwrap_or(""),
                &weapon_defs,
            ) {
                held_weapons.insert(id, weapon);
            }
            if e.class() == "ai_goal_police" {
                {
                    let angles = e
                        .get("angles")
                        .and_then(modkit_core::parse_vec3)
                        .unwrap_or(Vec3::ZERO);
                    goals.insert(
                        id,
                        PoliceGoal {
                            entity: id,
                            origin: e.origin(),
                            yaw: angles.y,
                            radius: e
                                .get("PoliceRadius")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0.),
                            target: None,
                            spawnflags: e
                                .get("spawnflags")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            override_knockout: false,
                        },
                    );
                }
            }
        }
        Self {
            enabled: false,
            ignore_player: false,
            held_weapons,
            was_enabled: false,
            sound_wait: BTreeMap::new(),
            rally_locks: BTreeMap::new(),
            assault_used: BTreeMap::new(),
            goals_started: false,
            scripted_fired: Default::default(),
            allegiances: BTreeMap::new(),
            npc_ids: None,
            relationship_overrides: BTreeMap::new(),
            applied_relationships: BTreeMap::new(),
            schedules,
            relationships: Relationships::default(),
            sounds: SoundEnt::default(),
            actors,
            goals,
            serial: 0,
            stunstick_sounds,
            stunstick_damage,
            weapon_defs,
            shots: Vec::new(),
            muzzle_flashes: Vec::new(),
            weapon_muzzle_flashes: Vec::new(),
            player_hits: Vec::new(),
            melee_hits: Vec::new(),
            diagnostics: Diagnostics::default(),
            random: 0x9e37_79b9_7f4a_7c15,
        }
    }
    /// A uniform value in [-1, 1) from the AI's stream (shot spread, blood noise).
    /// The world model of an NPC's active weapon (CBaseCombatWeapon::Equip follows the
    /// owner bone-merged), lowercase; None when unarmed.
    pub fn weapon_world_model(&self, id: usize) -> Option<String> {
        let class = &self
            .actors
            .get(&id)
            .and_then(|a| a.weapon.as_ref())
            .or_else(|| self.held_weapons.get(&id))?
            .class;
        self.weapon_model(class)
    }
    /// A weapon class's world model, lowercase.
    pub fn weapon_model(&self, class: &str) -> Option<String> {
        self.weapon_defs
            .get(class)
            .map(|d| d.world_model.to_lowercase())
            .filter(|m| !m.is_empty())
    }
    /// CWeaponStunStick::m_bActive of each NPC whose active weapon is a stunstick turned
    /// on (npc_metropolice SetBatonState).
    pub fn active_stunsticks(&self) -> Vec<usize> {
        self.actors
            .iter()
            .filter(|(_, a)| {
                a.class.cop().is_some_and(|cop| cop.baton_active)
                    && a.weapon
                        .as_ref()
                        .is_some_and(|w| w.class == "weapon_stunstick")
            })
            .map(|(id, _)| *id)
            .collect()
    }
    /// NPCs whose current enemy or enemy memory is the player.
    pub fn npcs_knowing_player(&self) -> std::collections::BTreeSet<usize> {
        self.actors
            .iter()
            .filter(|(_, a)| {
                a.npc.enemy == Some(PLAYER) || a.npc.enemies.find(Some(PLAYER)).is_some()
            })
            .map(|(id, _)| *id)
            .collect()
    }
    pub fn uniform(&mut self) -> f32 {
        self.random() * 2. - 1.
    }
    fn random(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random >> 40) as f32 / (1u32 << 24) as f32
    }
    /// SetPoliceGoal / ActivateBaton on cops and EnableKnockOut / DisableKnockOut on
    /// ai_goal_police (processed even while the AI is off, so map-spawn inputs stick).
    fn inputs(&mut self, world: &World, scene: &mut Scene, controller: &mut Controller) {
        if !self.goals_started && !self.actors.is_empty() {
            // CAI_GoalEntity::Activate at spawn for StartActive goals.
            self.goals_started = true;
            for (id, e) in world.entities.iter().enumerate() {
                if e.class() == "ai_relationship"
                    && e.get("StartActive").is_some_and(|v| v.trim() == "1")
                {
                    self.apply_relationship(world, scene, id, true);
                }
                if e.class() == "ai_goal_assault"
                    && e.get("StartActive").is_some_and(|v| v.trim() == "1")
                {
                    scene
                        .ai_inputs
                        .push((id, "activate".into(), String::new(), usize::MAX));
                }
            }
        }
        for (target, input, parameter, _) in std::mem::take(&mut scene.ai_inputs) {
            let class = world.entities.get(target).map_or("", |e| e.class());
            if class == "ai_goal_assault" {
                self.assault_goal_input(world, scene, target, &input);
                continue;
            }
            if class == "aiscripted_schedule" {
                self.scripted_schedule_input(world, scene, target, &input);
                continue;
            }
            if class == "ai_relationship" {
                self.apply_relationship(world, scene, target, input == "applyrelationship");
                continue;
            }
            if matches!(class, "info_node_link" | "info_node_link_controller") {
                // CAI_DynamicLink(Controller)::InputTurnOn/Off: the graph links' state.
                if !controller.set_dynamic_link(target, input == "turnon") {
                    *self
                        .diagnostics
                        .unknown_goals
                        .entry(format!("dynamic link #{target}"))
                        .or_default() += 1;
                }
                continue;
            }
            match input.as_str() {
                "setpolicegoal" => {
                    let goal = world.entities.iter().enumerate().find_map(|(id, e)| {
                        (e.class() == "ai_goal_police"
                            && e.get("targetname")
                                .is_some_and(|n| n.eq_ignore_ascii_case(&parameter)))
                        .then_some(id)
                    });
                    match (
                        goal.and_then(|g| self.goals.get(&g)),
                        self.actors.get_mut(&target),
                    ) {
                        (Some(goal), Some(actor)) if actor.class.cop().is_some() => {
                            let cop = actor.class.cop_mut().expect("cop");
                            // CAI_PolicingBehavior::Enable clears the schedule.
                            cop.policing = Some(Policing::new(goal.clone()));
                            actor.npc.schedule = None;
                        }
                        _ => {
                            *self
                                .diagnostics
                                .unknown_goals
                                .entry(parameter.clone())
                                .or_default() += 1;
                        }
                    }
                }
                "activatebaton" => {
                    if let Some(actor) = self.actors.get_mut(&target) {
                        if let AiClass::Metropolice(cop) = &mut actor.class {
                            let on = !matches!(parameter.trim(), "" | "0");
                            cop.set_baton_state(&mut actor.npc, on);
                        }
                    }
                }
                "enableknockout" | "disableknockout" => {
                    if let Some(goal) = self.goals.get_mut(&target) {
                        goal.override_knockout = input == "enableknockout";
                    }
                }
                _ => {}
            }
        }
    }
    /// CAI_GoalEntity actors: the NPCs whose targetname (SearchType 0) or classname
    /// (SearchType 1) matches the goal's "actor" keyvalue.
    fn goal_actors(&self, world: &World, goal: usize) -> Vec<usize> {
        let Some(e) = world.entities.get(goal) else {
            return Vec::new();
        };
        let name = e.get("actor").unwrap_or("").to_lowercase();
        let by_class = e.get("SearchType").is_some_and(|v| v.trim() == "1");
        if name.is_empty() {
            return Vec::new();
        }
        self.actors
            .keys()
            .copied()
            .filter(|&id| {
                world.entities.get(id).is_some_and(|a| {
                    let key = if by_class {
                        a.class().to_owned()
                    } else {
                        a.get("targetname").unwrap_or("").to_owned()
                    };
                    crate::entities::glob(&name, &key.to_lowercase())
                })
            })
            .collect()
    }
    /// Entities of a class matching a name (FindEntityByName: a trailing '*' matches
    /// any suffix).
    fn named(world: &World, class: &str, name: &str) -> Vec<usize> {
        let name = name.to_lowercase();
        world
            .entities
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.class() == class
                    && e.get("targetname")
                        .is_some_and(|n| crate::entities::glob(&name, &n.to_lowercase()))
            })
            .map(|(id, _)| id)
            .collect()
    }
    fn assault_point(world: &World, id: usize) -> assault::AssaultPoint {
        let e = &world.entities[id];
        let num = |k: &str, d: f32| e.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d);
        let flag = |k: &str| {
            e.get(k)
                .is_some_and(|v| v.trim() != "0" && !v.trim().is_empty())
        };
        assault::AssaultPoint {
            entity: id,
            name: e.get("targetname").unwrap_or("").to_owned(),
            origin: e.origin(),
            yaw: yaw_keyvalue(e),
            next: e.get("nextassaultpoint").unwrap_or("").to_owned(),
            timeout: num("assaulttimeout", 3.),
            clear_on_contact: flag("clearoncontact"),
            allow_diversion: flag("allowdiversion"),
            diversion_radius: num("allowdiversionradius", 0.),
            never_timeout: flag("nevertimeout"),
            strict: flag("strict"),
            tolerance: num("assaulttolerance", assault::CUE_POINT_TOLERANCE),
            clear_on_arrival: num("spawnflags", 0.) as u32 & 1 != 0,
        }
    }
    /// ai_goal_assault inputs: Activate (EnableGoal: SetParameters for each actor),
    /// Deactivate (DisableGoal), BeginAssault (ReceiveAssaultCue(CUE_ENTITY_INPUT)).
    fn assault_goal_input(&mut self, world: &World, scene: &mut Scene, goal: usize, input: &str) {
        // Held templates (not spawned yet) and dead NPCs are not actors.
        let actors: Vec<usize> = self
            .goal_actors(world, goal)
            .into_iter()
            .filter(|&id| scene.states[id].visible && !scene.states[id].killed)
            .collect();
        match input {
            "activate" | "updateactors" => {
                let e = &world.entities[goal];
                let rally_name = e.get("rallypoint").unwrap_or("").to_owned();
                let cue = assault::Cue::from_keyvalue(
                    e.get("AssaultCue")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(1),
                );
                let random_select = e.get("RallySelectMethod").is_some_and(|v| v.trim() == "1");
                for id in actors {
                    if input == "updateactors"
                        && self.actors[&id]
                            .class
                            .assault()
                            .is_some_and(|a| a.goal == goal && a.active())
                    {
                        continue;
                    }
                    self.release_rally(id);
                    let origin = scene.states[id].origin;
                    // Unlocked rally points: highest priority, then nearest (or random).
                    let mut rallies: Vec<usize> =
                        Self::named(world, "assault_rallypoint", &rally_name)
                            .into_iter()
                            .filter(|r| !self.rally_locks.contains_key(r))
                            .collect();
                    let priority = |r: usize| {
                        world.entities[r]
                            .get("priority")
                            .and_then(|v| v.trim().parse::<i32>().ok())
                            .unwrap_or(0)
                    };
                    let best = if random_select {
                        (!rallies.is_empty()).then(|| {
                            let i = (self.random() * rallies.len() as f32) as usize;
                            rallies[i.min(rallies.len() - 1)]
                        })
                    } else {
                        rallies.sort_by(|&a, &b| {
                            priority(b).cmp(&priority(a)).then(
                                world.entities[a]
                                    .origin()
                                    .distance_squared(origin)
                                    .total_cmp(
                                        &world.entities[b].origin().distance_squared(origin),
                                    ),
                            )
                        });
                        rallies.first().copied()
                    };
                    let Some(rally) = best else {
                        *self
                            .diagnostics
                            .unknown_goals
                            .entry(format!("rally {rally_name}"))
                            .or_default() += 1;
                        continue;
                    };
                    let r = &world.entities[rally];
                    let rally_point = assault::RallyPoint {
                        entity: rally,
                        origin: r.origin(),
                        yaw: yaw_keyvalue(r),
                        assault_point: r.get("assaultpoint").unwrap_or("").to_owned(),
                        delay: r
                            .get("assaultdelay")
                            .and_then(|v| v.trim().parse().ok())
                            .unwrap_or(0.),
                        priority: priority(rally),
                    };
                    // The assault chain by name, following nextassaultpoint.
                    let mut chain: Vec<(String, Vec<assault::AssaultPoint>)> = Vec::new();
                    let mut name = rally_point.assault_point.clone();
                    while !name.is_empty()
                        && chain.len() < 64
                        && !chain.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name))
                    {
                        let points: Vec<_> = Self::named(world, "assault_assaultpoint", &name)
                            .into_iter()
                            .map(|p| Self::assault_point(world, p))
                            .collect();
                        let next = points.first().map(|p| p.next.clone()).unwrap_or_default();
                        chain.push((name.clone(), points));
                        name = next;
                    }
                    // FindAssaultPoint: one candidate, else random excluding the most
                    // recently used (the hull fit test is not modeled).
                    let candidates = chain.first().map(|c| c.1.clone()).unwrap_or_default();
                    let pick = match candidates.len() {
                        0 => None,
                        1 => Some(0),
                        n => {
                            let recent = (0..n).max_by(|&a, &b| {
                                let t = |i: usize| {
                                    self.assault_used
                                        .get(&candidates[i].entity)
                                        .copied()
                                        .unwrap_or(-1.)
                                };
                                t(a).total_cmp(&t(b))
                            });
                            let others: Vec<usize> =
                                (0..n).filter(|&i| Some(i) != recent).collect();
                            let i = (self.random() * others.len() as f32) as usize;
                            Some(others[i.min(others.len() - 1)])
                        }
                    };
                    let Some(pick) = pick else {
                        *self
                            .diagnostics
                            .unknown_goals
                            .entry(format!("assault point {}", rally_point.assault_point))
                            .or_default() += 1;
                        continue;
                    };
                    let point = candidates[pick].clone();
                    self.assault_used.insert(point.entity, scene.time);
                    self.rally_locks.insert(rally, id);
                    let seed = 0x9e37_79b9_7f4a_7c15 ^ (id as u64) << 8;
                    let actor = self.actors.get_mut(&id).expect("actor");
                    *actor.class.assault_mut() = Some(assault::Assault::new(
                        goal,
                        cue,
                        rally_point,
                        point,
                        chain,
                        seed,
                    ));
                    // InitializeBehavior: ClearSchedule.
                    actor.npc.schedule = None;
                }
            }
            "deactivate" => {
                for id in actors {
                    let actor = self.actors.get_mut(&id).expect("actor");
                    if let Some(a) = actor
                        .class
                        .assault_mut()
                        .as_mut()
                        .filter(|a| a.goal == goal)
                    {
                        a.disable();
                        actor.npc.schedule = None;
                    }
                    self.release_rally(id);
                }
            }
            "beginassault" => {
                for id in actors {
                    let actor = self.actors.get_mut(&id).expect("actor");
                    if let Some(a) = actor.class.assault_mut().as_mut() {
                        a.receive_cue(assault::Cue::EntityInput);
                        a.provoke = true;
                    }
                }
            }
            _ => {}
        }
    }
    /// CAI_Relationship ApplyRelationship / RevertRelationship: subjects and targets by
    /// targetname or classname ("!player" for the player), within `radius` of the entity
    /// when set; disposition 1 hate, 2 fear, 3 like, 4 neutral; Reciprocal applies it both
    /// ways. Revert removes the pairs this entity applied. Rank (priority) is not modeled.
    fn apply_relationship(&mut self, world: &World, scene: &Scene, rel: usize, apply: bool) {
        if let Some(pairs) = self.applied_relationships.remove(&rel) {
            for pair in pairs {
                self.relationship_overrides.remove(&pair);
            }
        }
        if !apply {
            return;
        }
        let e = &world.entities[rel];
        let disposition = match e.get("disposition").map(str::trim) {
            Some("1") => Disposition::HT,
            Some("2") => Disposition::FR,
            Some("3") => Disposition::LI,
            Some("4") => Disposition::NU,
            _ => return,
        };
        let radius = e
            .get("radius")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .unwrap_or(0.);
        let at = e.origin();
        let matching = |pattern: &str| -> Vec<usize> {
            let pattern = pattern.trim().to_lowercase();
            if pattern.is_empty() {
                return Vec::new();
            }
            if pattern == "!player" {
                return vec![PLAYER];
            }
            world
                .entities
                .iter()
                .enumerate()
                .filter(|(i, x)| {
                    (crate::entities::glob(
                        &pattern,
                        &x.get("targetname").unwrap_or("").to_lowercase(),
                    ) || crate::entities::glob(&pattern, x.class()))
                        && (radius <= 0.
                            || scene
                                .states
                                .get(*i)
                                .is_some_and(|s| s.origin.distance(at) <= radius))
                })
                .map(|(i, _)| i)
                .collect()
        };
        let subjects = matching(e.get("subject").unwrap_or(""));
        let targets = matching(e.get("target").unwrap_or(""));
        let reciprocal = e.get("Reciprocal").is_some_and(|v| v.trim() == "1");
        let mut pairs = Vec::new();
        for &a in &subjects {
            for &b in &targets {
                if a == b {
                    continue;
                }
                pairs.push((a, b));
                if reciprocal {
                    pairs.push((b, a));
                }
            }
        }
        for &pair in &pairs {
            self.relationship_overrides.insert(pair, disposition);
        }
        self.applied_relationships.insert(rel, pairs);
    }
    /// UnlockRallyPoint for an NPC.
    fn release_rally(&mut self, id: usize) {
        self.rally_locks.retain(|_, holder| *holder != id);
    }
    /// aiscripted_schedule StartSchedule/StopSchedule (CAI_ScriptedSchedule): walk or run
    /// the m_iszEntity NPC(s) to the goal entity (ScheduledMoveToGoalEntity with
    /// SCHED_IDLE_WALK interrupts); forcestate sets the NPC state first.
    fn scripted_schedule_input(
        &mut self,
        world: &World,
        scene: &mut Scene,
        script: usize,
        input: &str,
    ) {
        let e = &world.entities[script];
        let name = e.get("m_iszEntity").unwrap_or("").to_lowercase();
        let graball = e.get("graball").is_some_and(|v| v.trim() == "1");
        let flags = e
            .get("spawnflags")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0);
        // InputStartSchedule: once, unless SF_SCRIPT_REPEATABLE (4); StopSchedule does
        // nothing before the first start.
        let fired = self.scripted_fired.contains(&script);
        if (input == "startschedule" && fired && flags & 4 == 0)
            || (input == "stopschedule" && !fired)
        {
            return;
        }
        // Acquisition: live NPCs matching the selector within m_flRadius of the
        // controller (0: anywhere).
        let radius = e
            .get("m_flRadius")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .unwrap_or(0.);
        let at = e.origin();
        let mut targets: Vec<usize> = self
            .actors
            .keys()
            .copied()
            .filter(|&id| {
                !scene.states[id].killed
                    && scene.states[id].visible
                    && (radius <= 0. || scene.states[id].origin.distance(at) <= radius)
                    && world.entities.get(id).is_some_and(|a| {
                        crate::entities::glob(
                            &name,
                            &a.get("targetname").unwrap_or("").to_lowercase(),
                        ) || crate::entities::glob(&name, a.class())
                    })
            })
            .collect();
        if !graball {
            targets.truncate(1);
        }
        if input == "startschedule" {
            if targets.is_empty() {
                // ScriptThink: no actor yet, try again in a second.
                scene.send_delayed(script, "StartSchedule", "", 1.);
                return;
            }
            self.scripted_fired.insert(script);
        }
        let schedule = e
            .get("schedule")
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0);
        let goal_name = e.get("goalent").unwrap_or("");
        let goal = world.entities.iter().enumerate().find_map(|(id, g)| {
            g.get("targetname")
                .is_some_and(|n| n.eq_ignore_ascii_case(goal_name))
                .then(|| {
                    if scene.states.get(id).is_some_and(|s| s.origin != Vec3::ZERO) {
                        scene.states[id].origin
                    } else {
                        g.origin()
                    }
                })
        });
        let state = match e
            .get("forcestate")
            .and_then(|v| v.trim().parse::<i32>().ok())
        {
            Some(1) => Some(NpcState::Idle),
            Some(2) => Some(NpcState::Alert),
            Some(3) => Some(NpcState::Combat),
            _ => None,
        };
        for id in targets {
            let actor = self.actors.get_mut(&id).expect("actor");
            if input == "stopschedule" {
                if actor.class.script_goal_mut().take().is_some() {
                    actor.npc.schedule = None;
                }
                continue;
            }
            let Some(goal) = goal else {
                *self
                    .diagnostics
                    .unknown_goals
                    .entry(format!("aiscripted goal {goal_name}"))
                    .or_default() += 1;
                continue;
            };
            // SCHED_SCRIPT_WALK_TO_GOAL 1, RUN_TO_GOAL 2, ENEMY_IS_GOAL_AND_RUN_TO_GOAL 6;
            // path goals (4, 5) and enemy-is-goal (3) are not modeled.
            let run = match schedule {
                1 => false,
                2 | 6 => true,
                other => {
                    *self
                        .diagnostics
                        .unknown_goals
                        .entry(format!("aiscripted schedule {other}"))
                        .or_default() += 1;
                    continue;
                }
            };
            if let Some(state) = state {
                actor.npc.state = state;
                actor.npc.ideal_state = state;
            }
            *actor.class.script_goal_mut() = Some((goal, run));
            actor.npc.force_schedule("SCHED_AISCRIPTED_MOVE_TO_GOAL");
        }
    }
    /// The PoliceTarget: "!player" or a named entity (resolved each think).
    fn goal_target(world: &World, scene: &Scene, goal: usize) -> Option<(usize, Vec3)> {
        let name = world.entities.get(goal)?.get("PoliceTarget")?;
        if name.eq_ignore_ascii_case("!player") {
            return Some((PLAYER, Vec3::ZERO));
        }
        world.entities.iter().enumerate().find_map(|(id, e)| {
            (e.get("targetname")
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
                && !scene.states[id].killed)
                .then(|| (id, scene.states[id].origin + Vec3::Z * 36.))
        })
    }
    /// The AI actor of an entity whose class has a behavior (npc_metropolice and the
    /// humanoids).
    fn actor_for(
        id: usize,
        e: &modkit_core::Entity,
        defs: &BTreeMap<String, NpcWeaponDef>,
    ) -> Option<AiActor> {
        let kind = humanoid::Kind::for_class(e.class());
        if e.class() != "npc_metropolice" && kind.is_none() {
            return None;
        }
        let flags = e
            .get("spawnflags")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let weapon = Self::equip(e.class(), e.get("additionalequipment").unwrap_or(""), defs);
        let baton = weapon
            .as_ref()
            .is_some_and(|w| w.class == "weapon_stunstick");
        let mut npc = AiNpc::new(id);
        npc.shot_regulator = Self::regulator(e.class(), weapon.as_ref(), defs, id);
        let class = match kind {
            Some(kind) => AiClass::Humanoid(Box::new(Humanoid::new(
                kind,
                weapon.is_some(),
                0x6a09_e667_f3bc_c909 ^ id as u64,
            ))),
            None => AiClass::Metropolice(Box::new(Metropolice::new(flags, baton))),
        };
        Some(AiActor {
            npc,
            class,
            next_think: 0.,
            last_think: 0.,
            weapon,
            player_disposition: None,
            origin: [0.; 3],
            yaw: 0.,
            damaged_by: None,
            goal: None,
            events_until: 0.,
            idle_angry: false,
        })
    }
    /// CAI_BaseNPC::Spawn: `additionalequipment` (not empty or "0") becomes the active
    /// weapon for classes with bits_CAP_USE_WEAPONS (Weapon_Equip; Spawn does not run
    /// Weapon_CanUse).
    fn equip(
        class: &str,
        equipment: &str,
        defs: &BTreeMap<String, NpcWeaponDef>,
    ) -> Option<NpcWeapon> {
        let equipment = equipment.trim().to_ascii_lowercase();
        if equipment.is_empty() || equipment == "0" || !npc_weapons::uses_weapons(class) {
            return None;
        }
        let def = defs
            .get(&equipment)
            .cloned()
            .or_else(|| npc_weapons::builtin(&equipment))?;
        Some(NpcWeapon::equip(&def))
    }
    /// OnUpdateShotRegulator of `npc_class` for a gun (Reset(true) at equip).
    fn regulator(
        npc_class: &str,
        weapon: Option<&NpcWeapon>,
        defs: &BTreeMap<String, NpcWeaponDef>,
        id: usize,
    ) -> Option<npc_weapons::ShotRegulator> {
        let class = &weapon?.class;
        let spec = defs
            .get(class)
            .cloned()
            .or_else(|| npc_weapons::builtin(class))?
            .fire?;
        let rule = match (npc_class, class.as_str()) {
            ("npc_metropolice", "weapon_pistol") => npc_weapons::RegulatorRule::MetropolicePistol,
            ("npc_citizen" | "npc_barney" | "npc_alyx", _) => {
                npc_weapons::RegulatorRule::PlayerCompanion
            }
            _ => npc_weapons::RegulatorRule::Weapon,
        };
        Some(npc_weapons::ShotRegulator::for_weapon(
            &spec,
            rule,
            0.,
            0x2545_f491_4f6c_dd1d ^ id as u64,
        ))
    }
    fn weapon_def(&self, class: &str) -> Option<NpcWeaponDef> {
        self.weapon_defs
            .get(class)
            .cloned()
            .or_else(|| npc_weapons::builtin(class))
    }
    /// Weapon_ShootPosition (m_HackedGunPos (0, 0, 55) of npc_metropolice) and the
    /// GetActualShootPosition target: the enemy's last known position plus its noisy
    /// BodyTarget offset (player: view offset x U(0.7, 1); NPC: the bell curve from the
    /// low body point to the eyes), and the seconds since the enemy became valid.
    fn weapon_aim(
        &mut self,
        id: usize,
        scene: &Scene,
        player: PlayerState,
        now: f64,
    ) -> Option<(Vec3, Vec3, f32)> {
        let actor = &self.actors[&id];
        let enemy = actor.npc.enemy?;
        let memory = actor.npc.enemies.find(Some(enemy)).cloned();
        let shoot = scene.states[id].origin + Vec3::Z * 55.;
        let since_valid = memory.as_ref().map_or(0., |m| (now - m.time_valid) as f32);
        let target = if enemy == PLAYER {
            let feet = memory.map_or(player.feet, |m| m.last_known);
            let view = if player.crouched { 28. } else { 64. };
            feet + Vec3::Z * view * (0.7 + 0.3 * self.random())
        } else {
            let feet = memory.map_or(scene.states.get(enemy)?.origin, |m| m.last_known);
            feet + Vec3::Z * (27. + 37. * (0.5 * self.random() + 0.5 * self.random()))
        };
        Some((shoot, target, since_valid))
    }
    /// A runtime-spawned entity (DESIGN 14r step 3): give it an AI actor when its
    /// class has a behavior. True when one was added.
    pub fn add_actor(&mut self, id: usize, e: &modkit_core::Entity) -> bool {
        match Self::actor_for(id, e, &self.weapon_defs) {
            Some(actor) => {
                self.actors.insert(id, actor);
                true
            }
            None => false,
        }
    }
    /// One fixed tick of the NPC stage (before the movement controller tick).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        world: &World,
        scene: &mut Scene,
        controller: &mut Controller,
        physics: &Physics,
        transients: &[ActorHull],
        player: PlayerState,
    ) {
        if let Some(value) = self.stunstick_damage {
            super::metropolice::STUNSTICK_DAMAGE.with(|d| d.set(value));
        }
        self.inputs(world, scene, controller);
        if !self.enabled {
            if self.was_enabled {
                self.was_enabled = false;
                self.stop_all(world, scene, controller);
            }
            return;
        }
        self.was_enabled = true;
        let player = if self.ignore_player {
            PlayerState {
                noclip: true,
                ..player
            }
        } else {
            player
        };
        let now = scene.time;
        self.sounds.purge(now);
        let precriminal = scene.globals.is_on("gordon_precriminal");
        let ids: Vec<usize> = self.actors.keys().copied().collect();
        for id in ids {
            let state = &scene.states[id];
            if state.killed || !state.visible {
                continue;
            }
            // NPC_STATE_SCRIPT: scripts own the actor; the AI yields.
            if scene.ai_scripted(id) {
                let actor = self.actors.get_mut(&id).expect("actor");
                if actor.npc.state != NpcState::Script {
                    actor.npc.state = NpcState::Script;
                    actor.npc.schedule = None;
                }
                if let Some(key) = actor.goal.take() {
                    controller.cancel(key);
                    controller.forget(key);
                    scene.ai_forget_move(key);
                }
                if let Some(cop) = actor.class.cop_mut() {
                    cop.scripted = true;
                }
                self.diagnostics.scripted_skips += 1;
                continue;
            }
            let think_at = self.actors[&id].next_think;
            // Animation events of the AI's sequence since the last tick.
            let since = self.actors[&id].events_until.min(now);
            self.actors.get_mut(&id).expect("actor").events_until = now;
            let mut events = scene.ai_events(world, id, since, now);
            // Operator_FrameUpdate: the weapon world model's events go to its operator.
            if let Some(weapon) = self.actors.get_mut(&id).and_then(|a| a.weapon.as_mut()) {
                if let Some(rig) = self
                    .weapon_defs
                    .get(&weapon.class)
                    .and_then(|d| world.rigs.get(&npc_weapons::rig_key(&d.world_model)))
                {
                    for (name, event, options) in weapon.due_events(rig, now) {
                        // AE_NPC_MUZZLEFLASH is a client event (DispatchMuzzleEffect).
                        if name.eq_ignore_ascii_case("AE_NPC_MUZZLEFLASH") {
                            self.muzzle_flashes.push((id, options));
                        } else {
                            events.push((name, event));
                        }
                    }
                }
            }
            if !events.is_empty() {
                let knock_out = self.knock_out_player(id, world, scene, physics, player);
                let ctx = self.context(id, world, scene, player, now, 0.);
                let aim = self.weapon_aim(id, scene, player, now);
                let def = self.actors[&id]
                    .weapon
                    .as_ref()
                    .and_then(|w| self.weapon_def(&w.class));
                let mut state = self.random;
                let mut uniform = move || {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state >> 40) as f32 / (1u32 << 23) as f32 - 1.
                };
                let npc_class = world.entities.get(id).map_or("", |e| e.class());
                let actor = self.actors.get_mut(&id).expect("actor");
                let AiActor { class, weapon, .. } = actor;
                let mut fired = false;
                let mut reloaded = false;
                for (name, event) in events {
                    if !(3000..4000).contains(&event) {
                        // Class animation events (the humanoids' AE_* are not modeled).
                        if let Some(cop) = class.cop_mut() {
                            cop.anim_event(&name, event, &ctx, knock_out);
                        }
                        continue;
                    }
                    // Weapon events reach only the active weapon (Operator_HandleAnimEvent);
                    // without one they are dropped (an unarmed cop swings at the air).
                    let outcome = match (weapon.as_mut(), def.as_ref()) {
                        (Some(w), Some(d)) => w.handle_event(
                            event,
                            d,
                            id,
                            npc_class,
                            aim,
                            &mut uniform,
                            &mut self.shots,
                        ),
                        _ => WeaponEvent::None,
                    };
                    match outcome {
                        WeaponEvent::MeleeHit => {
                            if let Some(cop) = class.cop_mut() {
                                cop.anim_event(&name, event, &ctx, knock_out);
                            }
                        }
                        WeaponEvent::Reloaded | WeaponEvent::ReloadSound => reloaded = true,
                        WeaponEvent::Fired => {
                            fired = true;
                            self.diagnostics.shots_fired += 1;
                            // CWeaponAR2::FireNPCPrimaryAttack leaves DoMuzzleFlash out.
                            if weapon.as_ref().is_some_and(|w| w.class != "weapon_ar2") {
                                self.weapon_muzzle_flashes.push(id);
                            }
                            // Operator_HandleAnimEvent: InsertSound(SOUND_COMBAT |
                            // SOUND_CONTEXT_GUNFIRE, operator origin, SOUNDENT_VOLUME_*, 0.2).
                            if let Some(volume) =
                                def.as_ref().and_then(|d| d.fire).map(|f| f.sound_volume)
                            {
                                self.sounds.insert(
                                    senses::sound::COMBAT,
                                    scene.states[id].origin,
                                    volume,
                                    0.2,
                                    Some(id),
                                    now,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                self.random = self.random.wrapping_add(0x9e37_79b9_7f4a_7c15) | 1;
                // WeaponSound(SINGLE_NPC / RELOAD_NPC) at the operator.
                for (played, key) in [(fired, "single_shot_npc"), (reloaded, "reload_npc")] {
                    if let Some(cue) = def
                        .as_ref()
                        .and_then(|d| d.sounds.get(key))
                        .filter(|_| played)
                    {
                        scene.sounds.push(crate::sounds::SoundRequest {
                            origin: Some(scene.states[id].origin + Vec3::Z * 55.),
                            ..cue.clone().into()
                        });
                    }
                }
                self.apply_requests(id, world, scene, physics, player);
            }
            if now + 1e-9 < think_at {
                continue;
            }
            self.think(
                id,
                world,
                scene,
                controller,
                physics,
                transients,
                player,
                precriminal,
            );
        }
    }
    /// Would a stunstick hit knock the player out (the cop's policing goal)?
    fn knock_out_player(
        &self,
        id: usize,
        world: &World,
        scene: &Scene,
        physics: &Physics,
        player: PlayerState,
    ) -> bool {
        let Some(policing) = self.actors[&id]
            .class
            .cop()
            .and_then(|c| c.policing.as_ref())
        else {
            return false;
        };
        let visible = PhysicsSight { physics, world }.visible(
            scene.states[id].origin + Vec3::Z * 64.,
            player.center(),
            &[id],
        );
        policing.goal.should_knock_out(player.center(), visible)
    }
    fn context(
        &mut self,
        id: usize,
        _world: &World,
        scene: &Scene,
        player: PlayerState,
        now: f64,
        _dt: f32,
    ) -> Context {
        let actor = &self.actors[&id];
        let enemy = actor.npc.enemy.map(|e| {
            let position = if e == PLAYER {
                player.feet
            } else {
                scene.states[e].origin
            };
            (e, position)
        });
        let enemy_eye = actor.npc.enemy.map(|e| {
            if e == PLAYER {
                player.eye()
            } else {
                scene.states[e].origin + Vec3::Z * super::host::EYE_HEIGHT
            }
        });
        let enemy_lkp = actor
            .npc
            .enemy
            .and_then(|e| actor.npc.enemies.find(Some(e)))
            .map(|m| m.last_known);
        let weapon_range = actor
            .weapon
            .as_ref()
            .and_then(|w| self.weapon_def(&w.class))
            .and_then(|d| d.fire)
            .map(|f| (f.min_range, f.max_range));
        let is_cop = actor.class.cop().is_some();
        let cop_target = actor.class.cop().and_then(|c| c.target_entity);
        let random = self.random();
        Context {
            now,
            origin: scene.states[id].origin,
            enemy,
            // SetTarget(player): the cop's target is the player while precriminal; an
            // NPC enemy after SetTarget(GetEnemy()) in combat.
            target: match cop_target {
                Some(t) if t != PLAYER => scene.states.get(t).map(|s| s.origin),
                _ => (is_cop && !player.noclip).then_some(player.feet),
            },
            best_sound: None,
            player: (!player.noclip).then_some(player.feet),
            yaw: yaw_of(scene, id),
            random,
            enemy_eye,
            enemy_lkp,
            weapon_range,
            best_sound_volume: 0.,
            // m_flDistTooFar (SF_NPC_LONG_RANGE is not read yet).
            dist_too_far: 1024.,
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn think(
        &mut self,
        id: usize,
        world: &World,
        scene: &mut Scene,
        controller: &mut Controller,
        physics: &Physics,
        transients: &[ActorHull],
        player: PlayerState,
        precriminal: bool,
    ) {
        let now = scene.time;
        let mut ctx = self.context(id, world, scene, player, now, 0.);
        let origin = ctx.origin;
        // The active gun's firing data and clip (GatherAttackConditions, CheckAmmo).
        let gun = self.actors[&id].weapon.as_ref().and_then(|w| {
            self.weapon_def(&w.class)
                .and_then(|d| d.fire)
                .map(|f| (f, w.clip1))
        });
        let eye = origin + Vec3::Z * 64.;
        let facing = scene.states[id].rotation * Vec3::X;
        let sight = PhysicsSight { physics, world };
        let class_name = world.entities.get(id).map_or("", |e| e.class());
        let my_class = classify(class_name, precriminal);
        let field_of_view = self.actors[&id].class.field_of_view();
        let sound_interests = self.actors[&id].class.sound_interests();
        let player_visible = !player.noclip
            && !player.dead
            && senses::in_view_cone(
                &Viewer {
                    entity: id,
                    class: my_class,
                    eye,
                    facing,
                    field_of_view,
                    look_distance: senses::LOOK_DISTANCE,
                },
                player.eye(),
            )
            && player.eye().distance(eye) <= senses::LOOK_DISTANCE
            && sight.visible(eye, player.eye(), &[id]);
        let fvisible = !player.noclip && sight.visible(eye, player.center(), &[id]);
        // NPC-vs-NPC sight (DESIGN 14n step 1): hated, live NPCs in the view cone and look
        // distance with LOS to their center.
        let npc_ids = self
            .npc_ids
            .get_or_insert_with(|| {
                world
                    .entities
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.class().starts_with("npc_"))
                    .map(|(i, _)| i)
                    .collect()
            })
            .clone();
        let viewer = Viewer {
            entity: id,
            class: my_class,
            eye,
            facing,
            field_of_view,
            look_distance: senses::LOOK_DISTANCE,
        };
        let seen_npcs: Vec<(usize, Vec3)> = npc_ids
            .into_iter()
            .filter(|&other| {
                let Some(state) = scene.states.get(other) else {
                    return false;
                };
                let center = state.origin + Vec3::Z * 36.;
                other != id
                    && !state.killed
                    && state.visible
                    && center.distance(eye) <= senses::LOOK_DISTANCE
                    && self.npc_disposition(world, id, other, precriminal) == Disposition::HT
                    && senses::in_view_cone(&viewer, center)
                    && sight.visible(eye, center, &[id, other])
            })
            .map(|other| (other, scene.states[other].origin))
            .collect();
        // Policing goal state (the goal entity can change through inputs).
        let goal_entity = self.actors[&id]
            .class
            .cop()
            .and_then(|cop| cop.policing.as_ref().map(|p| p.goal.entity));
        let goal = goal_entity.and_then(|g| {
            let mut goal = self.goals.get(&g)?.clone();
            let target = Self::goal_target(world, scene, g);
            goal.target = target.map(|t| t.0);
            Some((goal, target))
        });
        let player_override = self.relationship_overrides.get(&(id, PLAYER)).copied();
        let attacker_disposition = self.actors[&id]
            .damaged_by
            .map(|a| self.npc_disposition(world, id, a, precriminal));
        let relationships = &self.relationships;
        let precriminal_hate = relationships
            .get(my_class, classify("player", precriminal))
            .0
            == Disposition::HT;
        let low_clip = self.actors[&id].weapon.as_ref().is_some_and(|w| {
            self.weapon_def(&w.class)
                .is_some_and(|d| d.fire.is_some() && w.clip1 > 0 && w.clip1 < d.max_clip1 / 4 + 1)
        });
        let actor = self.actors.get_mut(&id).expect("actor");
        actor.npc.conditions = Conditions::default();
        // CheckAmmo: an empty clip (NPCs have unlimited reserve ammunition), and
        // CAI_BaseHumanoid::CheckAmmo's low clip (below a quarter of the clip plus one).
        if gun.is_some_and(|(_, clip)| clip == 0) {
            actor.npc.conditions.set(COND_NO_PRIMARY_AMMO);
        } else if low_clip {
            actor.npc.conditions.set(COND_LOW_PRIMARY_AMMO);
        }
        if let Some(assault) = actor.class.assault_mut().as_mut() {
            if std::mem::take(&mut assault.provoke) {
                actor.npc.conditions.set(COND_PROVOKED);
            }
        }
        // The class's view of the player and its relationship toward him.
        let class_hates_player = match &mut actor.class {
            AiClass::Metropolice(cop) => {
                cop.scripted = false;
                cop.precriminal = precriminal;
                cop.now = now;
                cop.player = (!player.noclip && !player.dead).then(|| PlayerView {
                    feet: player.feet,
                    center: player.center(),
                    velocity: player.velocity,
                    visible: fvisible,
                    on_me: (player.feet.z - (origin.z + 72.)).abs() < 2.
                        && (player.feet - origin).truncate().length() < 32.,
                    on_ground: player.on_ground,
                    suit: player.suit,
                });
                if let (Some(policing), Some((goal, target))) = (&mut cop.policing, goal) {
                    policing.goal = goal;
                    policing.target_center =
                        target.map(|(t, at)| if t == PLAYER { player.center() } else { at });
                    policing.target_visible = if target.is_some_and(|t| t.0 == PLAYER) {
                        fvisible
                    } else {
                        true
                    };
                    // CAI_PolicingBehavior: SetTarget(goal->GetTarget()), the player even
                    // when notarget (his senses ignore him; the policing tasks still face
                    // him).
                    if target.is_some_and(|t| t.0 == PLAYER) {
                        ctx.target = Some(player.feet);
                    }
                }
                // Look: the player when hated, never while precriminal unless the cop is
                // chasing him.
                cop.hates_player(precriminal_hate || !precriminal, now)
            }
            AiClass::Humanoid(h) => {
                h.now = now;
                h.armed = actor.weapon.is_some();
                precriminal_hate
            }
        };
        // A cop keeps the player as its enemy for the rest of a chase.
        let chasing = actor.class.cop().is_some_and(|cop| cop.chase_until > now);
        if actor.npc.state == NpcState::Script || actor.npc.state == NpcState::None {
            actor.npc.state = NpcState::Idle;
        }
        if player_visible {
            actor.npc.conditions.set(COND_SEE_PLAYER);
        }
        // F4 allegiance, then an ai_relationship toward !player, then the class rules.
        let hates = match actor.player_disposition.or(player_override) {
            Some(disposition) => disposition == Disposition::HT,
            None => class_hates_player,
        };
        let previous_enemy = actor.npc.enemy;
        if hates && player_visible {
            actor.npc.conditions.set(COND_SEE_HATE);
            actor
                .npc
                .enemies
                .update(Some(PLAYER), player.feet, 0., true, now);
            if actor.npc.enemy.is_none() {
                actor.npc.enemy = Some(PLAYER);
            }
        }
        if !hates && actor.npc.enemy == Some(PLAYER) && !chasing {
            actor.npc.enemy = None;
        }
        // ChooseEnemy rejects a notarget player (BestEnemy), e.g. one the policing
        // behavior made its enemy with SetEnemy (native: the cop falls back to alert).
        if player.noclip && actor.npc.enemy == Some(PLAYER) {
            actor.npc.enemy = None;
        }
        // Damage from an NPC: COND_LIGHT_DAMAGE, the attacker remembered where it stands and
        // made the enemy when there is none (unless liked).
        if let Some(attacker) = actor.damaged_by.take() {
            actor.npc.conditions.set(COND_LIGHT_DAMAGE);
            if let Some(state) = scene.states.get(attacker).filter(|s| !s.killed) {
                if attacker_disposition != Some(Disposition::LI) {
                    actor
                        .npc
                        .enemies
                        .update(Some(attacker), state.origin, 0., true, now);
                    if actor.npc.enemy.is_none() {
                        actor.npc.enemy = Some(attacker);
                    }
                }
            }
        }
        // Hated NPCs in sight: memory, and BestEnemy (the nearest) when there is none.
        for &(other, feet) in &seen_npcs {
            actor.npc.conditions.set(COND_SEE_HATE);
            actor.npc.enemies.update(Some(other), feet, 0., true, now);
        }
        if let Some(e) = actor.npc.enemy.filter(|&e| e != PLAYER) {
            if scene.states.get(e).is_none_or(|s| s.killed || !s.visible) {
                actor.npc.conditions.set(COND_ENEMY_DEAD);
                actor.npc.enemy = None;
            }
        }
        if actor.npc.enemy.is_none() {
            actor.npc.enemy = seen_npcs
                .iter()
                .min_by(|a, b| a.1.distance(origin).total_cmp(&b.1.distance(origin)))
                .map(|&(other, _)| other);
        }
        actor.npc.enemies.refresh(
            now,
            |e| {
                if e == PLAYER {
                    Some(player.feet)
                } else {
                    scene.states.get(e).map(|s| s.origin)
                }
            },
            |e| {
                if e == PLAYER {
                    player.dead
                } else {
                    scene.states.get(e).is_none_or(|s| s.killed)
                }
            },
        );
        let has_baton = actor.class.cop().is_some_and(|cop| cop.has_baton);
        if let Some(enemy) = actor.npc.enemy {
            if previous_enemy != Some(enemy) {
                actor.npc.conditions.set(COND_NEW_ENEMY);
            }
            let seen = if enemy == PLAYER {
                player_visible
            } else {
                seen_npcs.iter().any(|&(o, _)| o == enemy)
            };
            // The enemy's feet, center, eyes and velocity (the player's view or an NPC's
            // standing hull).
            let (t_feet, t_center, t_eye, t_velocity) = if enemy == PLAYER {
                (player.feet, player.center(), player.eye(), player.velocity)
            } else {
                let o = scene.states[enemy].origin;
                (o, o + Vec3::Z * 36., o + Vec3::Z * 64., Vec3::ZERO)
            };
            // GatherEnemyConditions: OnFoundPlayer/OnFoundEnemy when LOS to the enemy
            // is gained (bits_MEMORY_HAD_LOS), OnLostPlayerLOS/OnLostEnemyLOS when lost.
            let los = enemy == PLAYER && fvisible;
            if los != actor.npc.had_enemy_los {
                actor.npc.had_enemy_los = los;
                let (a, b) = if los {
                    ("OnFoundPlayer", "OnFoundEnemy")
                } else {
                    ("OnLostPlayerLOS", "OnLostEnemyLOS")
                };
                let requests = actor.class.requests_mut();
                requests.push(CopRequest::Output(a));
                requests.push(CopRequest::Output(b));
            }
            if seen {
                actor.npc.conditions.set(COND_SEE_ENEMY);
                // CWeaponStunStick::WeaponMeleeAttack1Condition (player in 48 units in
                // 0.35 s, within 70 of height and facing within 0.7).
                let delta = t_center - (origin + Vec3::Z * 36.);
                let forward = facing.truncate().normalize_or_zero();
                let projected = (t_feet + t_velocity * 0.35 - origin).truncate();
                if has_baton {
                    if delta.z.abs() > 70. {
                        actor.npc.conditions.set(COND_TOO_FAR_TO_ATTACK);
                    } else if delta.truncate().normalize_or_zero().dot(forward) < 0.7 {
                        actor.npc.conditions.set(COND_NOT_FACING_ATTACK);
                    } else if projected.length() <= 48. || delta.truncate().length() <= 48. {
                        actor.npc.conditions.set(COND_CAN_MELEE_ATTACK1);
                    } else {
                        actor.npc.conditions.set(COND_TOO_FAR_TO_ATTACK);
                    }
                } else if let Some((spec, clip)) = gun {
                    // GatherAttackConditions: the weapon's LOS from Weapon_ShootPosition to
                    // the enemy's eyes, else its BodyTarget, and
                    // CBaseCombatWeapon::WeaponRangeAttack1Condition(flDot, flDist).
                    // WeaponLOSCondition sets COND_WEAPON_SIGHT_OCCLUDED when the world
                    // blocks the shot (friends do not block our sight traces yet).
                    let shoot = origin + Vec3::Z * 55.;
                    let los = sight.visible(shoot, t_eye, &[id, enemy])
                        || sight.visible(shoot, t_center, &[id, enemy]);
                    if los {
                        actor.npc.conditions.set(COND_WEAPON_HAS_LOS);
                    } else {
                        actor.npc.conditions.set(COND_WEAPON_SIGHT_OCCLUDED);
                    }
                    let to = t_feet - origin;
                    let dot = to.truncate().normalize_or_zero().dot(forward);
                    let distance = to.length();
                    let condition = if clip == 0 {
                        COND_NO_PRIMARY_AMMO
                    } else if distance < spec.min_range {
                        COND_TOO_CLOSE_TO_ATTACK
                    } else if distance > spec.max_range {
                        COND_TOO_FAR_TO_ATTACK
                    } else if dot < 0.5 {
                        COND_NOT_FACING_ATTACK
                    } else {
                        COND_CAN_RANGE_ATTACK1
                    };
                    if condition != COND_CAN_RANGE_ATTACK1 || los {
                        actor.npc.conditions.set(condition);
                    }
                }
            } else {
                actor.npc.conditions.set(COND_ENEMY_OCCLUDED);
                if now - actor.npc.enemies.last_time_seen(Some(enemy)) > 10. {
                    actor.npc.conditions.set(COND_LOST_ENEMY);
                }
            }
        }
        // Listen.
        let (heard, best) = senses::listen(eye, id, sound_interests, &self.sounds);
        actor.npc.conditions = actor.npc.conditions.union(heard);
        ctx.best_sound = best.map(|s| s.origin);
        ctx.best_sound_volume = best.map_or(0., |s| s.volume);
        // SelectIdealState.
        let (ideal, _) = select_ideal_state(
            actor.npc.state,
            &StateInputs {
                conditions: actor.npc.conditions,
                has_enemy: actor.npc.enemy.is_some(),
                since_unknown_enemy: 999.,
                threat: None,
                best_sound: best.map(|s| {
                    (
                        s.origin,
                        s.kind
                            & (senses::sound::COMBAT
                                | senses::sound::DANGER
                                | senses::sound::BULLET_IMPACT)
                            != 0,
                    )
                }),
                // CAI_BaseNPC::ShouldGoToIdleState is false for cops: alert returns to
                // idle only through TASK_SUGGEST_STATE (SCHED_ALERT_STAND).
                go_idle: false,
                ideal: match actor.npc.ideal_state {
                    NpcState::None | NpcState::Script => NpcState::Idle,
                    other => other,
                },
            },
        );
        // SetIdealState(SelectIdealState()): later thinks fall back to this ideal.
        actor.npc.ideal_state = ideal;
        if ideal != actor.npc.state {
            actor.npc.state = ideal;
            actor.npc.schedule = None;
        }
        ctx.enemy = actor.npc.enemy.map(|e| {
            (
                e,
                if e == PLAYER {
                    player.feet
                } else {
                    scene.states[e].origin
                },
            )
        });
        if let AiClass::Metropolice(cop) = &mut actor.class {
            cop.gather_conditions(&mut actor.npc, &ctx);
        }
        let dt = (now - actor.last_think).clamp(0., 0.2) as f32;
        actor.last_think = now;
        actor.origin = origin.to_array();
        actor.yaw = yaw_of(scene, id);
        actor.next_think = now + THINK_INTERVAL;
        // MaxYawSpeed and NPC_TranslateActivity's ACT_IDLE_ANGRY by class.
        let weapon_drawn = actor.weapon.is_some();
        let (yaw_speed, idle_angry) = match &actor.class {
            AiClass::Metropolice(cop) => (
                if actor.npc.activity.starts_with("ACT_WALK") {
                    25.
                } else if actor.npc.activity.starts_with("ACT_RUN") {
                    15.
                } else {
                    45.
                },
                weapon_drawn && (actor.npc.state == NpcState::Combat || cop.baton_active),
            ),
            AiClass::Humanoid(h) => (
                h.kind.max_yaw_speed(&actor.npc.activity),
                weapon_drawn && h.idle_angry(actor.npc.state, now),
            ),
        };
        let mut motor = SceneMotor {
            entity: id,
            world,
            scene,
            controller,
            physics,
            transients,
            goal: &mut actor.goal,
            serial: &mut self.serial,
            dt: if dt > 0. { dt } else { THINK_INTERVAL as f32 },
            yaw_speed,
            idle_angry,
            weapon: actor.weapon.as_ref().map(|w| w.class.clone()),
            weapon_activity: None,
            blocked_moves: &mut self.diagnostics.blocked_moves,
        };
        actor.npc.maintain(
            &self.schedules,
            actor.class.behavior_mut(),
            &ctx,
            &mut motor,
        );
        // The idle activity is translated again when the angry-idle inputs change
        // (MaintainActivity re-translates the ideal ACT_IDLE).
        if idle_angry != actor.idle_angry {
            actor.idle_angry = idle_angry;
            if actor.npc.activity == "ACT_IDLE" {
                super::schedule::Motor::set_activity(&mut motor, "ACT_IDLE");
            }
        }
        // Weapon_SetActivity on the weapon's world model for this think's activities.
        if let (Some((activity, duration)), Some(weapon)) =
            (motor.weapon_activity.take(), actor.weapon.as_mut())
        {
            let rig = self
                .weapon_defs
                .get(&weapon.class)
                .and_then(|d| world.rigs.get(&npc_weapons::rig_key(&d.world_model)));
            if let Some(rig) = rig {
                weapon.set_activity(rig, &activity, duration, now);
            }
        }
        self.diagnostics.thinks += 1;
        // Assault point outputs (OnArrival, OnAssaultClear; activator = this NPC).
        let mut unlock = false;
        if let Some(assault) = actor.class.assault_mut().as_mut() {
            for (entity, output) in std::mem::take(&mut assault.outputs) {
                scene.fire(entity, output, id);
            }
            unlock = std::mem::take(&mut assault.unlock);
        }
        if unlock {
            self.rally_locks.retain(|_, holder| *holder != id);
        }
        // Policing outputs on the goal entity.
        if let Some(policing) = actor.class.cop_mut().and_then(|c| c.policing.as_mut()) {
            let goal = policing.goal.entity;
            for output in std::mem::take(&mut policing.outputs) {
                scene.fire(goal, output, id);
            }
        }
        self.apply_requests(id, world, scene, physics, player);
    }
    /// Carry out the cop's requests (sentences, look targets, outputs, sounds, hits).
    fn apply_requests(
        &mut self,
        id: usize,
        world: &World,
        scene: &mut Scene,
        physics: &Physics,
        _player: PlayerState,
    ) {
        let origin = scene.states[id].origin;
        let requests = {
            let actor = self.actors.get_mut(&id).expect("actor");
            std::mem::take(actor.class.requests_mut())
        };
        for request in requests {
            match request {
                CopRequest::Sentence(group) => {
                    // CAI_SentenceBase::Speak: FOkToMakeSound (no squads yet) culls a
                    // sentence of equal or lower priority while the last one's timer runs;
                    // JustMadeSound(priority, 2 s) sets it to 2 + U(1.5, 2.0) s.
                    let priority = sentence_priority(group);
                    if priority >= 0 {
                        let now = scene.time;
                        if let Some(&(until, last)) = self.sound_wait.get(&id) {
                            if now <= until && priority <= last {
                                self.diagnostics.culled_sentences += 1;
                                continue;
                            }
                        }
                        let wait = now + 2. + 1.5 + 0.5 * f64::from(self.random());
                        self.sound_wait.insert(id, (wait, priority));
                    }
                    self.diagnostics.sentences += 1;
                    scene.sounds.push(crate::sounds::SoundRequest {
                        origin: Some(origin + Vec3::Z * 64.),
                        actor: Some(crate::sounds::SoundActor {
                            name: scene
                                .entity(world, id)
                                .and_then(|e| e.get("targetname"))
                                .unwrap_or("")
                                .into(),
                            model: String::new(),
                            entity: Some(id),
                        }),
                        ..format!("#{group}").into()
                    });
                }
                CopRequest::LookAtPlayer {
                    importance,
                    duration,
                } => scene.look_targets.add(
                    id,
                    crate::attention::Target::Player,
                    importance,
                    scene.time,
                    f64::from(duration),
                ),
                CopRequest::Output(name) => scene.fire(id, name, usize::MAX),
                CopRequest::Sound(name) => {
                    let cue = match name {
                        "melee_hit" => self.stunstick_sounds.0.clone(),
                        "melee_miss" => self.stunstick_sounds.1.clone(),
                        other => Some(other.to_owned()),
                    };
                    if let Some(cue) = cue {
                        scene.sounds.push(crate::sounds::SoundRequest {
                            origin: Some(origin + Vec3::Z * 36.),
                            ..cue.into()
                        });
                    }
                }
                CopRequest::PlayerHit {
                    damage,
                    punch,
                    impulse,
                    fade,
                    knock_out,
                } => {
                    self.diagnostics.player_hits += 1;
                    if knock_out {
                        let cop = self.actors[&id].class.cop();
                        if let Some(goal) =
                            cop.and_then(|c| c.policing.as_ref()).map(|p| p.goal.entity)
                        {
                            scene.fire(goal, "OnKnockOut", usize::MAX);
                        }
                    }
                    self.player_hits.push(PlayerHit {
                        attacker: id,
                        damage,
                        punch,
                        impulse,
                        fade: fade.map(|(color, duration, hold, flags)| {
                            crate::player_damage::ScreenFade::new(color, duration, hold, flags)
                        }),
                    });
                }
                CopRequest::CallForJustice => self.call_for_justice(id, world, scene, physics),
                CopRequest::Swing {
                    start,
                    end,
                    mins,
                    maxs,
                    damage,
                } => {
                    let victim = self.melee_victim(id, world, scene, start, end, mins, maxs);
                    let cue = if let Some(victim) = victim {
                        self.diagnostics.npc_melee_hits += 1;
                        let center = scene.states[victim].origin + Vec3::Z * 36.;
                        self.melee_hits.push(MeleeHit {
                            attacker: id,
                            victim,
                            damage,
                            position: center,
                            direction: (center - (origin + Vec3::Z * 36.)).normalize_or_zero(),
                        });
                        // CTraceFilterMelee: InsertSound(SOUND_COMBAT, damage position, 200, 0.2).
                        self.sounds.insert(
                            senses::sound::COMBAT,
                            center,
                            200.,
                            0.2,
                            Some(id),
                            scene.time,
                        );
                        self.stunstick_sounds.0.clone()
                    } else {
                        self.stunstick_sounds.1.clone()
                    };
                    if let Some(cue) = cue {
                        scene.sounds.push(crate::sounds::SoundRequest {
                            origin: Some(origin + Vec3::Z * 36.),
                            ..cue.into()
                        });
                    }
                }
            }
        }
    }
    /// CheckTraceHullAttack's NPC part: the nearest NPC along the swept box (`mins`..`maxs`
    /// around the segment) whose hull it touches and that the attacker hates
    /// (CTraceFilterMelee only damages hated combat characters). World blocking is not
    /// traced (the player swing does not trace it either).
    #[allow(clippy::too_many_arguments)]
    fn melee_victim(
        &self,
        id: usize,
        world: &World,
        scene: &Scene,
        start: Vec3,
        end: Vec3,
        mins: Vec3,
        maxs: Vec3,
    ) -> Option<usize> {
        let precriminal = scene.globals.is_on("gordon_precriminal");
        world
            .entities
            .iter()
            .enumerate()
            .filter(|(other, e)| *other != id && e.class().starts_with("npc_"))
            .filter_map(|(other, e)| {
                let state = scene.states.get(other)?;
                if state.killed || !state.visible {
                    return None;
                }
                let (hull_min, hull_max) = crate::physics::npc_hull(e.class())?;
                let box_min = state.origin + hull_min - maxs;
                let box_max = state.origin + hull_max - mins;
                (super::metropolice::segment_hits_box(start, end, box_min, box_max)
                    && self.npc_disposition(world, id, other, precriminal) == Disposition::HT)
                    .then(|| {
                        (
                            other,
                            (state.origin + Vec3::Z * 36.).distance_squared(start),
                        )
                    })
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(other, _)| other)
    }
    /// AdministerJustice's search: the first other allowed-to-respond cop within 512
    /// units that sees this cop and the player.
    fn call_for_justice(&mut self, id: usize, world: &World, scene: &Scene, physics: &Physics) {
        let origin = scene.states[id].origin + Vec3::Z * 36.;
        let sight = PhysicsSight { physics, world };
        let helper = self.actors.iter().find_map(|(other, actor)| {
            let cop = actor.class.cop()?;
            let center = scene.states[*other].origin + Vec3::Z * 36.;
            (*other != id
                && !scene.states[*other].killed
                && cop.spawnflags & super::metropolice::SF_METROPOLICE_ALLOWED_TO_RESPOND != 0
                && center.distance(origin) < 512.
                && sight.visible(origin, center, &[id, *other])
                && cop.player.is_some_and(|p| p.visible))
            .then_some(*other)
        });
        if let Some(helper) = helper {
            let now = scene.time;
            let ctx = Context {
                now,
                origin: scene.states[helper].origin,
                yaw: yaw_of(scene, helper),
                random: self.random(),
                ..Default::default()
            };
            let actor = self.actors.get_mut(&helper).expect("actor");
            if let AiClass::Metropolice(cop) = &mut actor.class {
                cop.administer_justice(&mut actor.npc, &ctx);
            }
        }
    }
    /// g_interactionHitByPlayerThrownPhysObj for an NPC struck by a prop the player
    /// launched or threw (the host's physics-collision hook).
    pub fn hit_by_player_thrown_object(&mut self, id: usize, scene: &Scene) {
        let now = scene.time;
        let random = self.random();
        let Some(actor) = self.actors.get_mut(&id) else {
            return;
        };
        let ctx = Context {
            now,
            origin: scene.states[id].origin,
            yaw: yaw_of(scene, id),
            random,
            ..Default::default()
        };
        // npc_metropolice's interaction; the other classes do not handle it.
        if let AiClass::Metropolice(cop) = &mut actor.class {
            cop.hit_by_player_thrown_object(&mut actor.npc, &ctx);
        }
    }
    /// +USE on an AI NPC (npc_metropolice's SetUse(PrecriminalUse)). False when the AI
    /// is off or `id` is not an AI cop, so the host falls back to the ordinary use (the
    /// companions' CommanderUse/speech is not reconstructed).
    pub fn player_used(
        &mut self,
        id: usize,
        world: &World,
        scene: &mut Scene,
        physics: &Physics,
        player: PlayerState,
    ) -> bool {
        if !self.enabled || self.actors.get(&id).is_none_or(|a| a.class.cop().is_none()) {
            return false;
        }
        if !scene.ai_scripted(id) {
            let ctx = self.context(id, world, scene, player, scene.time, 0.);
            let actor = self.actors.get_mut(&id).expect("actor");
            if let AiClass::Metropolice(cop) = &mut actor.class {
                cop.precriminal_use(&mut actor.npc, &ctx);
            }
            self.apply_requests(id, world, scene, physics, player);
        }
        true
    }
    /// A spawned NPC's additionalequipment (F4 / npc_create_equipment) through
    /// Weapon_Equip (classes without bits_CAP_USE_WEAPONS get nothing). The cop's baton
    /// exists only with weapon_stunstick; classes without AI here just hold it.
    pub fn set_equipment(&mut self, id: usize, class: &str, equipment: &str) {
        let weapon = Self::equip(class, equipment, &self.weapon_defs);
        let regulator = Self::regulator(class, weapon.as_ref(), &self.weapon_defs, id);
        if let Some(actor) = self.actors.get_mut(&id) {
            match &mut actor.class {
                AiClass::Metropolice(cop) => {
                    cop.has_baton = weapon
                        .as_ref()
                        .is_some_and(|w| w.class == "weapon_stunstick");
                }
                AiClass::Humanoid(h) => h.armed = weapon.is_some(),
            }
            actor.weapon = weapon;
            actor.npc.shot_regulator = regulator;
        } else if let Some(weapon) = weapon {
            self.held_weapons.insert(id, weapon);
        } else {
            self.held_weapons.remove(&id);
        }
    }
    /// An NPC's bullet struck AI actor `target` (the victim responds at its next think).
    pub fn npc_attacked(&mut self, target: usize, attacker: usize) {
        if target == attacker {
            return;
        }
        if let Some(actor) = self.actors.get_mut(&target) {
            actor.damaged_by = Some(attacker);
        }
    }
    /// CSoundEnt::InsertSound for host-side sounds (the player's gunfire).
    pub fn insert_combat_sound(&mut self, origin: Vec3, volume: f32, now: f64) {
        self.sounds
            .insert(senses::sound::COMBAT, origin, volume, 0.2, None, now);
    }
    /// A spawn's disposition toward the player (F4 allegiance).
    pub fn set_player_disposition(&mut self, id: usize, disposition: Option<Disposition>) {
        if let Some(actor) = self.actors.get_mut(&id) {
            actor.player_disposition = disposition;
        }
        match disposition {
            Some(d) => self.allegiances.insert(id, d),
            None => self.allegiances.remove(&id),
        };
    }
    /// IRelationType of NPC `subject` toward NPC `target`. F4 allegiances make sides: two
    /// NPCs with allegiances hate each other when they differ and like each other when
    /// they match; one with an allegiance hates an NPC whose class (SDK table) hates the
    /// player when its allegiance is friendly, and one whose class likes the player when
    /// it is hostile (our extension: the SDK has only per-entity ai_relationship). Without
    /// allegiances the SDK class table decides.
    pub fn npc_disposition(
        &self,
        world: &World,
        subject: usize,
        target: usize,
        precriminal: bool,
    ) -> Disposition {
        let class = |id: usize| {
            classify(
                world.entities.get(id).map_or("", |e| e.class()),
                precriminal,
            )
        };
        let side = |id: usize| -> Option<Disposition> {
            self.allegiances.get(&id).copied().or_else(|| {
                match self.relationships.get(class(id), Class::Player).0 {
                    Disposition::HT => Some(Disposition::HT),
                    Disposition::LI => Some(Disposition::LI),
                    _ => None,
                }
            })
        };
        if let Some(&d) = self.relationship_overrides.get(&(subject, target)) {
            return d;
        }
        match (
            self.allegiances.get(&subject),
            self.allegiances.get(&target),
        ) {
            (Some(a), Some(b)) => {
                if a == b {
                    Disposition::LI
                } else {
                    Disposition::HT
                }
            }
            (Some(a), None) => match side(target) {
                Some(b) if b != *a => Disposition::HT,
                _ => Disposition::NU,
            },
            (None, Some(b)) => match side(subject) {
                Some(a) if a != *b => Disposition::HT,
                _ => Disposition::NU,
            },
            (None, None) => self.relationships.get(class(subject), class(target)).0,
        }
    }
    /// The weapons `class` can be given on a model with `activities` (upper case):
    /// bits_CAP_USE_WEAPONS and Weapon_CanUse over the installed weapon definitions.
    pub fn usable_weapons(
        &self,
        class: &str,
        activities: &std::collections::BTreeSet<String>,
    ) -> Vec<String> {
        npc_weapons::usable_weapons(class, activities, &self.weapon_defs)
    }
    /// Switching the AI off (debug): every AI NPC drops its schedule, enemy and
    /// movement goal and returns to its idle activity.
    fn stop_all(&mut self, world: &World, scene: &mut Scene, controller: &mut Controller) {
        for (&id, actor) in self.actors.iter_mut() {
            if let Some(key) = actor.goal.take() {
                controller.cancel(key);
                controller.forget(key);
                scene.ai_forget_move(key);
            }
            actor.npc.schedule = None;
            actor.npc.enemy = None;
            if !scene.ai_scripted(id) {
                scene.ai_set_activity(world, id, "ACT_IDLE");
            }
        }
    }
    /// CSoundEnt::InsertSound from gameplay (gunfire, impacts, explosions, footsteps).
    pub fn insert_sound(&mut self, kind: u32, origin: Vec3, volume: f32, duration: f64, now: f64) {
        self.sounds
            .insert(kind, origin, volume, duration, None, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modkit_core::Entity;

    fn entity(pairs: &[(&str, &str)]) -> Entity {
        Entity {
            properties: pairs
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        }
    }
    /// A cop at the origin facing +X, a police goal with the player as its target and a
    /// relay that counts the goal's first warning (synthetic map data).
    fn world() -> World {
        World {
            entities: vec![
                entity(&[
                    ("classname", "npc_metropolice"),
                    ("targetname", "cop"),
                    ("origin", "0 0 0"),
                    ("additionalequipment", "weapon_stunstick"),
                ]),
                entity(&[
                    ("classname", "ai_goal_police"),
                    ("targetname", "post"),
                    ("origin", "0 0 9"),
                    ("angles", "0 0 0"),
                    ("PoliceRadius", "100"),
                    ("PoliceTarget", "!player"),
                    ("OnFirstWarning", "counter,Add,1,0,-1"),
                ]),
                entity(&[
                    ("classname", "env_global"),
                    ("globalstate", "gordon_precriminal"),
                    ("initialstate", "1"),
                    ("spawnflags", "1"),
                ]),
            ],
            ..Default::default()
        }
    }
    struct Rig {
        world: World,
        scene: Scene,
        physics: Physics,
        controller: Controller,
        ai: AiRuntime,
    }
    fn rig() -> Rig {
        let world = world();
        let scene = Scene::new(&world);
        let physics = Physics::new(&world);
        let ai = AiRuntime::new(&world, None);
        Rig {
            world,
            scene,
            physics,
            controller: Controller::new(None, Default::default()),
            ai,
        }
    }
    fn run(r: &mut Rig, ticks: usize, player: PlayerState) {
        for _ in 0..ticks {
            r.scene.time += 0.015;
            r.ai.tick(
                &r.world,
                &mut r.scene,
                &mut r.controller,
                &r.physics,
                &[],
                player,
            );
        }
    }
    fn player_at(x: f32) -> PlayerState {
        PlayerState {
            feet: Vec3::new(x, 0., 0.),
            on_ground: true,
            ..Default::default()
        }
    }

    #[test]
    fn ai_is_off_by_default_and_shoves_a_close_player_when_enabled() {
        let mut r = rig();
        run(&mut r, 20, player_at(30.));
        assert!(r.ai.actors[&0].npc.schedule.is_none());
        assert!(r.scene.sounds.is_empty());
        r.ai.enabled = true;
        run(&mut r, 20, player_at(30.));
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        assert!(cop.precriminal);
        assert!(cop.warnings >= 1);
        assert!(r
            .scene
            .sounds
            .iter()
            .any(|s| s.name.starts_with("#METROPOLICE_BACK_UP") && s.origin.is_some()));
        assert!(r.ai.diagnostics.thinks >= 2);
    }

    #[test]
    fn using_a_calm_cop_counts_as_bothering_him() {
        let mut r = rig();
        let far = player_at(500.);
        assert!(!r.ai.player_used(0, &r.world, &mut r.scene, &r.physics, far));
        r.ai.enabled = true;
        run(&mut r, 5, far);
        assert!(!r.ai.player_used(2, &r.world, &mut r.scene, &r.physics, far));
        for _ in 0..3 {
            assert!(r.ai.player_used(0, &r.world, &mut r.scene, &r.physics, far));
        }
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        assert_eq!(
            cop.warnings,
            super::super::metropolice::METROPOLICE_MAX_WARNINGS
        );
        assert!(r
            .scene
            .sounds
            .iter()
            .any(|s| s.name.starts_with("#METROPOLICE_BACK_UP")));
    }

    #[test]
    fn a_pooled_cop_is_hidden_until_placed_then_thinks() {
        let mut world = World {
            entities: vec![entity(&[
                ("classname", "env_global"),
                ("globalstate", "gordon_precriminal"),
                ("initialstate", "1"),
                ("spawnflags", "1"),
            ])],
            ..Default::default()
        };
        crate::ai::spawn::append_pool(&mut world);
        let id =
            crate::ai::spawn::free_slot(&world, "npc_metropolice", &Default::default()).unwrap();
        let mut r = Rig {
            scene: Scene::new(&world),
            physics: Physics::new(&world),
            ai: AiRuntime::new(&world, None),
            controller: Controller::new(None, Default::default()),
            world,
        };
        r.ai.enabled = true;
        assert!(!r.scene.states[id].visible);
        run(&mut r, 20, player_at(30.));
        assert_eq!(r.ai.diagnostics.thinks, 0, "hidden slots do not think");
        r.scene.states[id].origin = Vec3::ZERO;
        r.scene.states[id].visible = true;
        r.ai.set_equipment(id, "npc_metropolice", "weapon_stunstick");
        run(&mut r, 20, player_at(30.));
        let cop = r.ai.actors[&id].class.cop().expect("cop");
        assert!(cop.has_baton && cop.warnings >= 1);
    }

    #[test]
    fn debug_switches_ignore_the_player_and_stop_the_ai() {
        let mut r = rig();
        r.ai.enabled = true;
        r.ai.ignore_player = true;
        run(&mut r, 20, player_at(30.));
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        assert_eq!(cop.warnings, 0, "an ignored player is not shoved");
        r.ai.ignore_player = false;
        // F4 allegiance: a foe spawn hates the player, a friend never makes him its enemy.
        r.ai.set_player_disposition(0, Some(Disposition::HT));
        run(&mut r, 20, player_at(30.));
        assert_eq!(r.ai.actors[&0].npc.enemy, Some(PLAYER), "forced hate");
        r.ai.enabled = false;
        run(&mut r, 1, player_at(30.));
        assert!(r.ai.actors[&0].npc.schedule.is_none());
        assert!(r.ai.actors[&0].npc.enemy.is_none());
        r.ai.enabled = true;
        r.ai.set_player_disposition(0, Some(Disposition::LI));
        run(&mut r, 20, player_at(30.));
        assert_ne!(r.ai.actors[&0].npc.enemy, Some(PLAYER), "a friend");
    }

    #[test]
    fn weapon_equip_goes_to_any_weapon_user_and_never_to_natural_attackers() {
        let world = World {
            entities: vec![
                entity(&[
                    ("classname", "npc_monk"),
                    ("additionalequipment", "weapon_smg1"),
                ]),
                entity(&[
                    ("classname", "npc_zombie"),
                    ("additionalequipment", "weapon_smg1"),
                ]),
                entity(&[
                    ("classname", "npc_citizen"),
                    ("additionalequipment", "weapon_smg1"),
                ]),
            ],
            ..Default::default()
        };
        let mut ai = AiRuntime::new(&world, None);
        // Map additionalequipment: the monk holds it (his class has no AI layer here yet),
        // the zombie has no bits_CAP_USE_WEAPONS, the citizen's AI actor equips it.
        assert_eq!(ai.held_weapons[&0].class, "weapon_smg1");
        assert!(!ai.held_weapons.contains_key(&1));
        assert_eq!(ai.actors[&2].weapon.as_ref().unwrap().class, "weapon_smg1");
        assert!(ai.actors[&2].npc.shot_regulator.is_some());
        ai.set_equipment(1, "npc_zombie", "weapon_pistol");
        assert!(!ai.held_weapons.contains_key(&1));
        ai.set_equipment(0, "npc_monk", "");
        assert!(!ai.held_weapons.contains_key(&0), "unequipped");
        ai.set_equipment(0, "npc_monk", "weapon_ar2");
        assert_eq!(ai.held_weapons[&0].class, "weapon_ar2");
        ai.set_equipment(2, "npc_citizen", "weapon_ar2");
        assert_eq!(ai.actors[&2].weapon.as_ref().unwrap().class, "weapon_ar2");
        assert_eq!(
            ai.actors[&2].npc.shot_regulator.as_ref().unwrap().rule,
            npc_weapons::RegulatorRule::PlayerCompanion
        );
    }

    #[test]
    fn a_runtime_spawned_cop_gets_an_actor_and_thinks() {
        let mut r = rig();
        r.ai.enabled = true;
        let cop = modkit_core::Entity {
            properties: vec![
                ("classname".into(), "npc_metropolice".into()),
                ("origin".into(), "0 60 0".into()),
                ("additionalequipment".into(), "weapon_stunstick".into()),
            ],
        };
        let id = r.scene.spawn_entity(&r.world, cop.clone());
        assert!(r.ai.add_actor(id, &cop));
        // Classes without an AI layer here (Kleiner: CAI_BaseActor, no combat).
        let kleiner = modkit_core::Entity {
            properties: vec![("classname".into(), "npc_kleiner".into())],
        };
        assert!(!r.ai.add_actor(id + 1, &kleiner));
        run(&mut r, 20, player_at(30.));
        let spawned = r.ai.actors[&id].class.cop().expect("cop");
        assert!(spawned.has_baton);
        assert!(r.ai.actors[&id].last_think > 0.);
    }

    #[test]
    fn scripted_cops_are_left_to_their_script() {
        let mut r = rig();
        r.ai.enabled = true;
        r.scene.states[0].scripted_by = Some(2);
        run(&mut r, 20, player_at(30.));
        assert_eq!(r.ai.actors[&0].npc.state, NpcState::Script);
        assert!(r.ai.actors[&0].npc.schedule.is_none());
        assert!(r.scene.sounds.is_empty());
        assert!(r.ai.diagnostics.scripted_skips > 0);
        r.scene.states[0].scripted_by = None;
        run(&mut r, 10, player_at(30.));
        assert_ne!(r.ai.actors[&0].npc.state, NpcState::Script);
    }

    #[test]
    fn policing_warns_a_noclip_player_like_native_notarget() {
        // The oracle runs noclip + notarget, and native still harasses at 80 units: the
        // policing conditions read the goal target, not the cop's senses.
        let mut r = rig();
        r.scene
            .ai_inputs
            .push((0, "setpolicegoal".into(), "post".into(), usize::MAX));
        r.ai.enabled = true;
        let player = PlayerState {
            noclip: true,
            ..player_at(150.)
        };
        run(&mut r, 40, player);
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        let policing = cop.policing.as_ref().unwrap();
        assert!(policing.warnings >= 1, "{:?}", r.ai.actors[&0].npc.schedule);
        // One warning per 4-6 s: the warn schedule does not fail and re-warn at once.
        run(&mut r, 150, player);
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        let policing = cop.policing.as_ref().unwrap();
        assert_eq!(policing.warnings, 1, "{:?}", r.ai.actors[&0].npc.schedule);
        assert!(!policing.hostile);
        // The idle stand is interrupted once the next harass time passes (4-6 s), not
        // only when its own wait ends.
        run(&mut r, 300, player);
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        assert_eq!(cop.policing.as_ref().unwrap().warnings, 2);
    }

    #[test]
    fn assault_goal_and_scripted_schedule_inputs_reach_the_cop() {
        let world = World {
            entities: vec![
                entity(&[
                    ("classname", "npc_metropolice"),
                    ("targetname", "cop1"),
                    ("origin", "450 0 0"),
                    ("additionalequipment", "weapon_pistol"),
                ]),
                entity(&[
                    ("classname", "ai_goal_assault"),
                    ("targetname", "cop1_assault"),
                    ("actor", "cop1"),
                    ("rallypoint", "cop1_rp"),
                    ("AssaultCue", "1"),
                ]),
                entity(&[
                    ("classname", "assault_rallypoint"),
                    ("targetname", "cop1_rp"),
                    ("origin", "632 0 32"),
                    ("angles", "0 180 0"),
                    ("assaultpoint", "cop1_ap"),
                    ("priority", "1"),
                ]),
                entity(&[
                    ("classname", "assault_assaultpoint"),
                    ("targetname", "cop1_ap"),
                    ("origin", "632 0 8"),
                    ("assaulttimeout", "3.0"),
                ]),
                entity(&[
                    ("classname", "aiscripted_schedule"),
                    ("targetname", "cop1_sched"),
                    ("m_iszEntity", "cop1"),
                    ("goalent", "cop1_goal"),
                    ("schedule", "2"),
                ]),
                entity(&[
                    ("classname", "info_target"),
                    ("targetname", "cop1_goal"),
                    ("origin", "376 -60 0"),
                ]),
            ],
            ..Default::default()
        };
        let scene = Scene::new(&world);
        let physics = Physics::new(&world);
        let ai = AiRuntime::new(&world, None);
        let mut r = Rig {
            world,
            scene,
            physics,
            controller: Controller::new(None, Default::default()),
            ai,
        };
        let far = player_at(-2000.);
        // StartSchedule: a run to the goal entity, once (no SF_SCRIPT_REPEATABLE).
        r.scene
            .ai_inputs
            .push((4, "startschedule".into(), String::new(), usize::MAX));
        run(&mut r, 1, far);
        let actor = &r.ai.actors[&0];
        assert_eq!(
            actor.npc.schedule.as_deref(),
            Some("SCHED_AISCRIPTED_MOVE_TO_GOAL")
        );
        let cop = actor.class.cop().expect("cop");
        assert_eq!(cop.script_goal, Some((Vec3::new(376., -60., 0.), true)));
        r.ai.actors.get_mut(&0).unwrap().npc.schedule = None;
        r.scene
            .ai_inputs
            .push((4, "startschedule".into(), String::new(), usize::MAX));
        run(&mut r, 1, far);
        assert!(r.ai.actors[&0].npc.schedule.is_none());
        // Activate: the rally point is locked by the cop, the assault point resolved.
        r.scene
            .ai_inputs
            .push((1, "activate".into(), String::new(), usize::MAX));
        run(&mut r, 1, far);
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        let assault = cop.assault.as_ref().expect("assault");
        assert!(assault.active());
        assert_eq!(assault.rally.entity, 2);
        assert_eq!(assault.rally.yaw, 180.);
        assert_eq!(assault.point.entity, 3);
        assert_eq!(r.ai.rally_locks.get(&2), Some(&0));
        // BeginAssault cues it; Deactivate disables it and frees the rally point.
        r.scene
            .ai_inputs
            .push((1, "beginassault".into(), String::new(), usize::MAX));
        r.scene
            .ai_inputs
            .push((1, "deactivate".into(), String::new(), usize::MAX));
        run(&mut r, 1, far);
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        let assault = cop.assault.as_ref().unwrap();
        assert_eq!(assault.received, super::super::assault::Cue::EntityInput);
        assert!(!assault.active());
        assert!(r.ai.rally_locks.is_empty());
    }
    #[test]
    fn friend_and_foe_cops_target_each_other_and_relationships_override_classes() {
        let world = World {
            entities: vec![
                entity(&[
                    ("classname", "npc_metropolice"),
                    ("targetname", "friend"),
                    ("origin", "0 0 0"),
                    ("angles", "0 0 0"),
                    ("additionalequipment", "weapon_pistol"),
                ]),
                entity(&[
                    ("classname", "npc_metropolice"),
                    ("targetname", "foe"),
                    ("origin", "300 0 0"),
                    ("angles", "0 180 0"),
                    ("additionalequipment", "weapon_pistol"),
                ]),
                entity(&[
                    ("classname", "npc_citizen"),
                    ("targetname", "bystander"),
                    ("origin", "0 300 0"),
                ]),
                entity(&[
                    ("classname", "ai_relationship"),
                    ("subject", "npc_metropolice"),
                    ("target", "bystander"),
                    ("disposition", "1"),
                    ("StartActive", "1"),
                ]),
            ],
            ..Default::default()
        };
        let scene = Scene::new(&world);
        let physics = Physics::new(&world);
        let ai = AiRuntime::new(&world, None);
        let mut r = Rig {
            world,
            scene,
            physics,
            controller: Controller::new(None, Default::default()),
            ai,
        };
        // Same class, no allegiances: the SDK table says neutral.
        assert_eq!(r.ai.npc_disposition(&r.world, 0, 1, true), Disposition::NU);
        r.ai.set_player_disposition(0, Some(Disposition::LI));
        r.ai.set_player_disposition(1, Some(Disposition::HT));
        assert_eq!(r.ai.npc_disposition(&r.world, 0, 1, true), Disposition::HT);
        assert_eq!(r.ai.npc_disposition(&r.world, 1, 0, true), Disposition::HT);
        r.ai.enabled = true;
        run(&mut r, 20, player_at(-3000.));
        assert_eq!(r.ai.actors[&0].npc.enemy, Some(1));
        assert_eq!(r.ai.actors[&1].npc.enemy, Some(0));
        // StartActive ai_relationship: cops hate the passive citizen (class rule NU).
        assert_eq!(r.ai.npc_disposition(&r.world, 1, 2, true), Disposition::HT);
        // The AI report (pair-keyed overrides included) serializes.
        assert!(serde_json::to_string(&r.ai).is_ok());
        r.scene
            .ai_inputs
            .push((3, "revertrelationship".into(), String::new(), usize::MAX));
        run(&mut r, 1, player_at(-3000.));
        assert_eq!(r.ai.npc_disposition(&r.world, 1, 2, true), Disposition::NU);
        // A killed enemy is dropped (COND_ENEMY_DEAD) for the next one.
        r.scene.states[1].killed = true;
        run(&mut r, 10, player_at(-3000.));
        assert_eq!(r.ai.actors[&0].npc.enemy, None);
    }
    #[test]
    fn stunstick_swings_strike_only_hated_npcs_in_the_swept_box() {
        let cop = |name: &str, origin: &str, angles: &str| {
            entity(&[
                ("classname", "npc_metropolice"),
                ("targetname", name),
                ("origin", origin),
                ("angles", angles),
                ("additionalequipment", "weapon_stunstick"),
            ])
        };
        let world = World {
            entities: vec![
                cop("friend", "0 0 0", "0 0 0"),
                cop("foe", "40 0 0", "0 180 0"),
                cop("ally", "0 40 0", "0 0 0"),
            ],
            ..Default::default()
        };
        let scene = Scene::new(&world);
        let physics = Physics::new(&world);
        let ai = AiRuntime::new(&world, None);
        let mut r = Rig {
            world,
            scene,
            physics,
            controller: Controller::new(None, Default::default()),
            ai,
        };
        r.ai.set_player_disposition(0, Some(Disposition::LI));
        r.ai.set_player_disposition(1, Some(Disposition::HT));
        r.ai.set_player_disposition(2, Some(Disposition::LI));
        let swing = |end: Vec3| CopRequest::Swing {
            start: Vec3::new(0., 0., 60.),
            end,
            mins: Vec3::new(-16., -16., -40.),
            maxs: Vec3::splat(16.),
            damage: 40.,
        };
        let player = player_at(-3000.);
        // Toward the hated foe 40 units ahead: a hit with a combat sound.
        r.ai.actors
            .get_mut(&0)
            .unwrap()
            .class
            .requests_mut()
            .push(swing(Vec3::new(32., 0., 60.)));
        r.ai.apply_requests(0, &r.world, &mut r.scene, &r.physics, player);
        assert_eq!(r.ai.melee_hits.len(), 1);
        assert_eq!(r.ai.melee_hits[0].victim, 1);
        assert_eq!(r.ai.melee_hits[0].damage, 40.);
        // Toward the liked ally: CTraceFilterMelee passes it (no damage).
        r.ai.melee_hits.clear();
        r.ai.actors
            .get_mut(&0)
            .unwrap()
            .class
            .requests_mut()
            .push(swing(Vec3::new(0., 32., 60.)));
        r.ai.apply_requests(0, &r.world, &mut r.scene, &r.physics, player);
        assert!(r.ai.melee_hits.is_empty());
    }
    #[test]
    fn set_police_goal_enables_policing_warnings_and_goal_outputs() {
        let mut r = rig();
        // logic_auto-style input before the AI is switched on still enables the goal.
        r.scene
            .ai_inputs
            .push((0, "setpolicegoal".into(), "post".into(), usize::MAX));
        run(&mut r, 1, player_at(500.));
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        assert!(cop.policing.is_some());
        r.ai.enabled = true;
        // Inside 2 x PoliceRadius (200) but outside the radius and the shove range.
        run(&mut r, 40, player_at(150.));
        let cop = r.ai.actors[&0].class.cop().expect("cop");
        let policing = cop.policing.as_ref().unwrap();
        assert!(policing.warnings >= 1, "{:?}", r.ai.actors[&0].npc.schedule);
        assert!(r
            .scene
            .sounds
            .iter()
            .any(|s| s.name == "#METROPOLICE_MOVE_ALONG_A"));
        // An unknown goal name is counted, not panicking.
        r.scene
            .ai_inputs
            .push((0, "setpolicegoal".into(), "nowhere".into(), usize::MAX));
        run(&mut r, 1, player_at(150.));
        assert_eq!(r.ai.diagnostics.unknown_goals["nowhere"], 1);
    }
    #[test]
    fn soldier_and_rebel_spawns_fight_each_other_with_their_class_schedules() {
        // Synthetic open ground: an F4-style foe soldier at the origin facing +X and a
        // friendly armed citizen 300 units ahead facing it (owner report: only cops
        // fought; the other classes stood idle with their weapons).
        let world = World {
            entities: vec![
                entity(&[
                    ("classname", "npc_combine_s"),
                    ("origin", "0 0 0"),
                    ("angles", "0 0 0"),
                    ("additionalequipment", "weapon_smg1"),
                ]),
                entity(&[
                    ("classname", "npc_citizen"),
                    ("origin", "300 0 0"),
                    ("angles", "0 180 0"),
                    ("additionalequipment", "weapon_smg1"),
                ]),
            ],
            ..Default::default()
        };
        let scene = Scene::new(&world);
        let physics = Physics::new(&world);
        let ai = AiRuntime::new(&world, None);
        let mut r = Rig {
            world,
            scene,
            physics,
            controller: Controller::new(None, Default::default()),
            ai,
        };
        assert!(r.ai.actors[&0].class.humanoid().is_some());
        assert!(r.ai.actors[&1].class.humanoid().is_some());
        r.ai.set_player_disposition(0, Some(Disposition::HT));
        r.ai.set_player_disposition(1, Some(Disposition::LI));
        r.ai.enabled = true;
        run(&mut r, 30, player_at(-3000.));
        assert_eq!(r.ai.actors[&0].npc.enemy, Some(1));
        assert_eq!(r.ai.actors[&1].npc.enemy, Some(0));
        assert_eq!(r.ai.actors[&0].npc.state, NpcState::Combat);
        let soldier = r.ai.actors[&0].npc.schedule.clone().unwrap_or_default();
        let rebel = r.ai.actors[&1].npc.schedule.clone().unwrap_or_default();
        assert!(soldier.starts_with("SCHED_COMBINE_"), "{soldier}");
        assert!(
            matches!(
                rebel.as_str(),
                "SCHED_RANGE_ATTACK1" | "SCHED_COMBAT_FACE" | "SCHED_WAKE_ANGRY"
            ),
            "{rebel}"
        );
        // Their ammo checks and the weapon's attack conditions run for every class.
        assert!(r.ai.actors[&1]
            .npc
            .unsupported_tasks
            .keys()
            .all(|k| !k.starts_with("schedule ")));
    }
}
