//! Developer NPC spawning: the F4 overlay's data model (class and weapon choice, spawn
//! at the crosshair) and the Source console commands npc_create / npc_create_aimed /
//! npc_create_equipment (ai_concommands.cpp CC_NPC_Create, CC_NPC_Create_Aimed). The
//! class/weapon lists are developer choices, not SDK data. Placement follows the SDK.
//! The running world's entity tables are fixed at map load, so spawning activates a
//! hidden pooled entity appended at load (`append_pool`): every load-time system
//! (models, animation, physics, AI) builds it like a map NPC.
use glam::Vec3;
use serde::Serialize;

/// Classes offered by the overlay (developer choice, not SDK data). The weapon column
/// comes from the SDK rules instead (bits_CAP_USE_WEAPONS + Weapon_CanUse on the
/// loaded model; `Overlay::set_usable`).
pub const SPAWN_CLASSES: &[&str] = &[
    "npc_metropolice",
    "npc_combine_s",
    "npc_combine_s_shotgunner",
    "npc_combine_s_elite",
    "npc_combine_s_prisonguard",
    "npc_citizen",
    "npc_citizen_rebel",
    "npc_citizen_refugee",
    "npc_citizen_medic",
    "npc_barney",
    "npc_alyx",
    "npc_vortigaunt",
    "npc_dog",
    "npc_kleiner",
    "npc_eli",
    "npc_mossman",
    "npc_breen",
    "npc_gman",
    "npc_monk",
    "npc_zombie",
    "npc_zombie_torso",
    "npc_fastzombie",
    "npc_poisonzombie",
    "npc_headcrab",
    "npc_headcrab_fast",
    "npc_headcrab_black",
    "npc_headcrab_poison",
    "npc_barnacle",
    "npc_antlion",
    "npc_antlionguard",
    "npc_ichthyosaur",
    "npc_crow",
    "npc_pigeon",
    "npc_seagull",
    "npc_stalker",
    "npc_manhack",
    "npc_cscanner",
    "npc_clawscanner",
    "npc_rollermine",
    "npc_turret_floor",
    "npc_turret_ceiling",
    "npc_combine_camera",
    "npc_sniper",
    "npc_strider",
    "npc_combinegunship",
    "npc_helicopter",
    "npc_combinedropship",
];
/// An overlay entry's entity class and extra keyvalues: citizen variants are npc_citizen
/// with a `citizentype` (CT_REFUGEE 2, CT_REBEL 3) and the medic spawnflag; every pooled
/// citizen has SF_CITIZEN_RANDOM_HEAD, so CNPC_Citizen::SelectModel gives each slot its
/// own least-used head (`citizens::select_models`). Plain npc_citizen takes the map's
/// citizen type like native npc_create.
pub fn spawn_class(label: &str) -> (&str, Vec<(&'static str, String)>) {
    use crate::citizens::{SF_CITIZEN_MEDIC, SF_CITIZEN_RANDOM_HEAD};
    let head = SF_CITIZEN_RANDOM_HEAD.to_string();
    match label {
        "npc_citizen" => ("npc_citizen", vec![("spawnflags", head)]),
        "npc_citizen_rebel" => (
            "npc_citizen",
            vec![("citizentype", "3".into()), ("spawnflags", head)],
        ),
        "npc_citizen_refugee" => (
            "npc_citizen",
            vec![("citizentype", "2".into()), ("spawnflags", head)],
        ),
        "npc_citizen_medic" => (
            "npc_citizen",
            vec![
                ("citizentype", "3".into()),
                (
                    "spawnflags",
                    (SF_CITIZEN_RANDOM_HEAD | SF_CITIZEN_MEDIC).to_string(),
                ),
            ],
        ),
        // CNPC_Combine::CalcWeaponProficiency gives a shotgun holder COMBINE_SKIN_SHOTGUNNER
        // (1); the pool entity is built with it (the F4 shotgun choice on the plain
        // soldier keeps skin 0: skins are chosen at map load).
        "npc_combine_s_shotgunner" => ("npc_combine_s", vec![("skin", "1".into())]),
        // CNPC_CombineS::Precache: m_fIsElite for this model (sk_combine_guard_health,
        // AR2 combine balls).
        "npc_combine_s_elite" => (
            "npc_combine_s",
            vec![("model", "models/combine_super_soldier.mdl".into())],
        ),
        "npc_combine_s_prisonguard" => (
            "npc_combine_s",
            vec![("model", "models/combine_soldier_prisonguard.mdl".into())],
        ),
        class => (class, Vec::new()),
    }
}
/// The weapon the overlay preselects for an entry (developer choice; native npc_create
/// gives none unless npc_create_equipment is set). Used only when the class can use it.
pub fn default_weapon(label: &str) -> &'static str {
    match label {
        "npc_citizen_rebel" | "npc_citizen_medic" => "weapon_smg1",
        "npc_metropolice" => "weapon_stunstick",
        "npc_combine_s" | "npc_combine_s_prisonguard" => "weapon_smg1",
        "npc_combine_s_shotgunner" => "weapon_shotgun",
        "npc_combine_s_elite" => "weapon_ar2",
        "npc_barney" => "weapon_pistol",
        "npc_alyx" => "weapon_alyxgun",
        "npc_monk" => "weapon_annabelle",
        _ => "",
    }
}
/// Hidden pooled entities per overlay class (appended at map load).
pub const POOL_PER_CLASS: usize = 3;
/// Keyvalue marking a pooled spawn entity (not a Source key).
pub const POOL_KEY: &str = "hl2rs_spawn_pool";
/// Where pooled entities wait (far below any HL2 map).
pub const POOL_ORIGIN: Vec3 = Vec3::new(0., 0., -15000.);
/// The model each class's Spawn/Precache sets when the map gives none (SDK 2013
/// hl2/npc_*.cpp; citizens pick from groups at random in the SDK, one is used here).
pub fn default_model(class: &str) -> Option<&'static str> {
    Some(match class {
        "npc_metropolice" => "models/police.mdl",
        "npc_combine_s" | "npc_sniper" => "models/combine_soldier.mdl",
        "npc_citizen" => "models/humans/group01/male_07.mdl",
        "npc_barney" => "models/barney.mdl",
        "npc_alyx" => "models/alyx.mdl",
        "npc_vortigaunt" => "models/vortigaunt.mdl",
        "npc_dog" => "models/dog.mdl",
        "npc_kleiner" => "models/kleiner.mdl",
        "npc_eli" => "models/eli.mdl",
        "npc_mossman" => "models/mossman.mdl",
        "npc_breen" => "models/breen.mdl",
        "npc_gman" => "models/gman.mdl",
        "npc_monk" => "models/monk.mdl",
        "npc_zombie" => "models/zombie/classic.mdl",
        "npc_zombie_torso" => "models/zombie/classic_torso.mdl",
        "npc_fastzombie" => "models/zombie/fast.mdl",
        "npc_poisonzombie" => "models/zombie/poison.mdl",
        "npc_headcrab" => "models/headcrabclassic.mdl",
        "npc_headcrab_fast" => "models/headcrab.mdl",
        "npc_headcrab_black" | "npc_headcrab_poison" => "models/headcrabblack.mdl",
        "npc_barnacle" => "models/barnacle.mdl",
        "npc_antlion" => "models/antlion.mdl",
        "npc_antlionguard" => "models/antlion_guard.mdl",
        "npc_ichthyosaur" => "models/ichthyosaur.mdl",
        "npc_crow" => "models/crow.mdl",
        "npc_pigeon" => "models/pigeon.mdl",
        "npc_seagull" => "models/seagull.mdl",
        "npc_stalker" => "models/stalker.mdl",
        "npc_manhack" => "models/manhack.mdl",
        "npc_cscanner" | "npc_clawscanner" => "models/combine_scanner.mdl",
        "npc_rollermine" => "models/roller.mdl",
        "npc_turret_floor" => "models/combine_turrets/floor_turret.mdl",
        "npc_turret_ceiling" => "models/combine_turrets/ceiling_turret.mdl",
        "npc_combine_camera" => "models/combine_camera/combine_camera.mdl",
        "npc_strider" => "models/combine_strider.mdl",
        "npc_combinegunship" => "models/gunship.mdl",
        "npc_helicopter" => "models/combine_helicopter.mdl",
        "npc_combinedropship" => "models/combine_dropship.mdl",
        _ => return None,
    })
}
/// Pooled slots per class: one for the very large models (load time), else
/// POOL_PER_CLASS.
pub fn pool_count(class: &str) -> usize {
    match class {
        "npc_strider"
        | "npc_combinegunship"
        | "npc_helicopter"
        | "npc_combinedropship"
        | "npc_antlionguard"
        | "npc_ichthyosaur" => 1,
        _ => POOL_PER_CLASS,
    }
}
/// Append the hidden spawn pool to a freshly loaded map's entities (before models
/// load). Existing entity ids are unchanged; the pool goes at the end.
pub fn append_pool(world: &mut modkit_core::World) {
    for label in SPAWN_CLASSES {
        let (class, keyvalues) = spawn_class(label);
        let Some(model) = default_model(class) else {
            continue;
        };
        for _ in 0..pool_count(class) {
            let mut properties: Vec<(String, String)> = vec![
                ("classname".into(), class.into()),
                (
                    "origin".into(),
                    format!("{} {} {}", POOL_ORIGIN.x, POOL_ORIGIN.y, POOL_ORIGIN.z),
                ),
                (POOL_KEY.into(), (*label).into()),
            ];
            // Citizens get their head from SelectModel (citizens::select_models).
            if class != "npc_citizen" && !keyvalues.iter().any(|(k, _)| *k == "model") {
                properties.push(("model".into(), model.into()));
            }
            properties.extend(keyvalues.iter().map(|(k, v)| ((*k).into(), v.clone())));
            if class == "npc_metropolice" {
                properties.push(("additionalequipment".into(), "weapon_stunstick".into()));
            }
            world.entities.push(modkit_core::Entity { properties });
        }
    }
}
/// The first unused pooled entity of an overlay entry (a class or citizen variant).
pub fn free_slot(
    world: &modkit_core::World,
    label: &str,
    used: &std::collections::BTreeSet<usize>,
) -> Option<usize> {
    world
        .entities
        .iter()
        .enumerate()
        .find_map(|(id, e)| (e.get(POOL_KEY) == Some(label) && !used.contains(&id)).then_some(id))
}
/// Flying classes (bits_CAP_MOVE_FLY) are placed 36 units back along the aim.
fn flies(class: &str) -> bool {
    matches!(
        class,
        "npc_manhack"
            | "npc_cscanner"
            | "npc_clawscanner"
            | "npc_combinegunship"
            | "npc_helicopter"
            | "npc_combinedropship"
    )
}
/// MAX_TRACE_LENGTH.
pub const MAX_TRACE_LENGTH: f32 = 56755.84;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SpawnRequest {
    pub classname: String,
    pub name: Option<String>,
    pub equipment: String,
    /// npc_create_aimed: face the player's aim yaw (pitch and roll 0).
    pub aimed: bool,
    /// The F4 overlay's per-spawn side (npc_create: the class default).
    pub allegiance: Allegiance,
    /// The overlay entry (citizen variants); empty: `classname`.
    pub variant: String,
    /// The F4 NumGrenades keyvalue for classes that read it (CNPC_Combine); None: the
    /// entity's own value (npc_create: none, so 0 like native).
    pub grenades: Option<i32>,
}
impl SpawnRequest {
    /// The pool entry this request draws from.
    pub fn pool_label(&self) -> &str {
        if self.variant.is_empty() {
            &self.classname
        } else {
            &self.variant
        }
    }
}
/// The F4 overlay's per-spawn side toward the player, like an ai_relationship entity
/// aimed at the spawned NPC (Foe: D_HT, Friend: D_LI; Default: the class relationship
/// table). The opposite side (friends hating foes) needs NPC-vs-NPC enemies, which the
/// AI does not choose yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Allegiance {
    #[default]
    Default,
    Foe,
    Friend,
}
impl Allegiance {
    pub fn disposition(self) -> Option<super::relationships::Disposition> {
        use super::relationships::Disposition;
        match self {
            Self::Default => None,
            Self::Foe => Some(Disposition::HT),
            Self::Friend => Some(Disposition::LI),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Default => "class default",
            Self::Foe => "foe (hates the player)",
            Self::Friend => "friend (likes the player)",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Default => Self::Foe,
            Self::Foe => Self::Friend,
            Self::Friend => Self::Default,
        }
    }
}

/// Console state: the npc_create_equipment cvar.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Console {
    pub equipment: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Command {
    Spawn(SpawnRequest),
    SetEquipment(String),
}
impl Console {
    /// Parse one console line: `npc_create <class> [name]`, `npc_create_aimed <class>
    /// [name]`, `npc_create_equipment <weapon>`. Ok(None) for other commands.
    pub fn parse(&mut self, line: &str) -> Result<Option<Command>, String> {
        let args: Vec<&str> = line.split_whitespace().collect();
        let Some(command) = args.first() else {
            return Ok(None);
        };
        match command.to_ascii_lowercase().as_str() {
            "npc_create_equipment" => {
                let value = args
                    .get(1)
                    .copied()
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_owned();
                self.equipment = value.clone();
                Ok(Some(Command::SetEquipment(value)))
            }
            c @ ("npc_create" | "npc_create_aimed") => {
                let class = args
                    .get(1)
                    .ok_or_else(|| format!("{c}: missing npc class"))?;
                if !class.starts_with("npc_") {
                    // CreateEntityByName succeeds only for CAI_BaseNPC classes.
                    return Err(format!("{c}: {class} is not an NPC class"));
                }
                // Our citizen variants (npc_citizen_rebel ...) are npc_citizen with
                // keyvalues; native npc_create has only npc_citizen.
                let (classname, _) = spawn_class(class);
                let variant = if classname != *class {
                    (*class).to_owned()
                } else {
                    String::new()
                };
                Ok(Some(Command::Spawn(SpawnRequest {
                    classname: classname.to_owned(),
                    // CC_NPC_Create names the NPC when given exactly two arguments.
                    name: (args.len() == 3).then(|| args[2].to_owned()),
                    equipment: self.equipment.clone(),
                    aimed: c == "npc_create_aimed",
                    allegiance: Allegiance::Default,
                    variant,
                    grenades: None,
                })))
            }
            _ => Ok(None),
        }
    }
}

/// Where the spawned NPC goes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Placement {
    pub origin: Vec3,
    /// Yaw in degrees (npc_create keeps the spawn angles, 0).
    pub yaw: f32,
}
/// World queries placement needs (MASK_NPCSOLID).
pub trait PlacementWorld {
    /// AI_TraceLine: the end point, or None when nothing is hit.
    fn trace_line(&self, start: Vec3, end: Vec3) -> Option<Vec3>;
    /// UTIL_DropToFloor: the floor position below, if any.
    fn drop_to_floor(&self, class: &str, origin: Vec3) -> Option<Vec3>;
    /// The 1-unit upward hull check: true when the NPC's hull fits.
    fn hull_fits(&self, class: &str, origin: Vec3) -> bool;
}
/// CC_NPC_Create / CC_NPC_Create_Aimed placement. Err for "Bad Position" (the SDK
/// removes the NPC). A miss leaves the NPC at its spawn origin (world origin here).
pub fn place(
    request: &SpawnRequest,
    eye: Vec3,
    forward: Vec3,
    world: &dyn PlacementWorld,
) -> Result<Placement, String> {
    let forward = forward.normalize_or_zero();
    let yaw = if request.aimed {
        forward.y.atan2(forward.x).to_degrees()
    } else {
        0.
    };
    let Some(end) = world.trace_line(eye, eye + forward * MAX_TRACE_LENGTH) else {
        return Ok(Placement {
            origin: Vec3::ZERO,
            yaw,
        });
    };
    let origin = if flies(&request.classname) {
        end - forward * 36.
    } else {
        let raised = end + Vec3::Z * 12.;
        world
            .drop_to_floor(&request.classname, raised)
            .unwrap_or(raised)
    };
    if !world.hull_fits(&request.classname, origin) {
        // The SDK message, then our diagnostics (aim end and the tested origin).
        return Err(format!(
            "Can't create {}.  Bad Position! (aim end {:.1} {:.1} {:.1}, origin {:.1} {:.1} {:.1})",
            request.classname, end.x, end.y, end.z, origin.x, origin.y, origin.z
        ));
    }
    Ok(Placement { origin, yaw })
}

/// F4 overlay keys (keyboard or movement-script `spawn_overlay` actions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayKey {
    Toggle,
    Up,
    Down,
    Left,
    Right,
    Enter,
}
/// What an overlay key asks the host to do.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum OverlayCommand {
    Spawn(SpawnRequest),
    /// Toggle notarget (the AI does not sense the player).
    ToggleNotarget,
    /// Toggle the AI think loop (ai_enable).
    ToggleAi,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Column {
    #[default]
    Classes,
    Options,
}
/// One option row of the right column.
#[derive(Clone, Debug, PartialEq, Eq)]
enum OptionRow {
    /// A weapon ("" = none).
    Weapon(String),
    Allegiance,
    /// NumGrenades (classes that read it).
    Grenades,
    Notarget,
    Ai,
}
/// A drawn panel row: text, selected (the current choice), under the cursor.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PanelRow {
    pub text: String,
    pub selected: bool,
    pub cursor: bool,
}
/// The overlay as the host draws it: a title and two columns.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Panel {
    pub title: String,
    pub classes: Vec<PanelRow>,
    pub options: Vec<PanelRow>,
}
/// The F4 grenade counts Enter cycles through (a developer choice: native npc_create
/// spawns soldiers with NumGrenades 0, so they never throw; maps set it per soldier).
pub const GRENADE_CHOICES: &[i32] = &[0, 1, 2, 3, 5, 10, 99];
/// Classes whose spawn reads the NumGrenades keyvalue (CNPC_Combine::KeyValue).
pub fn reads_num_grenades(class: &str) -> bool {
    class.eq_ignore_ascii_case("npc_combine_s")
}
/// Class rows shown at once; the list scrolls to keep the selection visible.
pub const VISIBLE_CLASSES: usize = 14;

/// The F4 overlay: a scrolling class column and an options column with the weapons the
/// selected class can use (`set_usable`), its allegiance, notarget and the AI switch.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Overlay {
    pub open: bool,
    pub class: usize,
    pub column: Column,
    /// Cursor row in the options column.
    pub row: usize,
    /// The chosen weapon ("" = none).
    pub weapon: String,
    pub allegiance: Allegiance,
    /// The NumGrenades given to classes that read it (`reads_num_grenades`).
    pub grenades: i32,
    /// Usable weapons per class (bits_CAP_USE_WEAPONS + Weapon_CanUse on the model the
    /// spawn uses); classes missing here have none.
    usable: std::collections::BTreeMap<String, Vec<String>>,
}
impl Overlay {
    /// The weapons each class can be given, from the loaded models (host, map load).
    pub fn set_usable(&mut self, usable: std::collections::BTreeMap<String, Vec<String>>) {
        self.usable = usable;
        self.select_class(self.class);
    }
    pub fn usable(&self, class: &str) -> &[String] {
        self.usable.get(class).map_or(&[], Vec::as_slice)
    }
    fn label(&self) -> &'static str {
        SPAWN_CLASSES[self.class]
    }
    fn class_name(&self) -> &'static str {
        spawn_class(self.label()).0
    }
    fn rows(&self) -> Vec<OptionRow> {
        let mut rows = vec![OptionRow::Weapon(String::new())];
        rows.extend(
            self.usable(self.label())
                .iter()
                .cloned()
                .map(OptionRow::Weapon),
        );
        rows.push(OptionRow::Allegiance);
        if reads_num_grenades(self.class_name()) {
            rows.push(OptionRow::Grenades);
        }
        rows.extend([OptionRow::Notarget, OptionRow::Ai]);
        rows
    }
    fn select_class(&mut self, class: usize) {
        self.class = class;
        let label = self.label();
        let default = default_weapon(label);
        self.weapon = if self.usable(label).iter().any(|w| w == default) {
            default.into()
        } else {
            String::new()
        };
        let weapon = OptionRow::Weapon(self.weapon.clone());
        self.row = self.rows().iter().position(|r| *r == weapon).unwrap_or(0);
    }
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
    /// Apply one key: Up/Down move in the focused column (classes wrap; moving onto a
    /// weapon row chooses it), Left/Right switch columns, Enter toggles the option under
    /// the cursor (allegiance, notarget, AI) or else spawns.
    pub fn key(&mut self, key: OverlayKey) -> Option<OverlayCommand> {
        if key == OverlayKey::Toggle {
            self.toggle();
            return None;
        }
        if !self.open {
            return None;
        }
        let step = if key == OverlayKey::Up { -1 } else { 1 };
        match key {
            OverlayKey::Left => self.column = Column::Classes,
            OverlayKey::Right => self.column = Column::Options,
            OverlayKey::Up | OverlayKey::Down => match self.column {
                Column::Classes => {
                    let n = SPAWN_CLASSES.len() as isize;
                    self.select_class((self.class as isize + step).rem_euclid(n) as usize);
                }
                Column::Options => {
                    let rows = self.rows();
                    self.row = (self.row as isize + step).rem_euclid(rows.len() as isize) as usize;
                    if let OptionRow::Weapon(weapon) = &rows[self.row] {
                        self.weapon = weapon.clone();
                    }
                }
            },
            OverlayKey::Enter => {
                if self.column == Column::Options {
                    match self.rows()[self.row] {
                        OptionRow::Allegiance => {
                            self.allegiance = self.allegiance.next();
                            return None;
                        }
                        OptionRow::Grenades => {
                            let next = GRENADE_CHOICES
                                .iter()
                                .position(|&n| n == self.grenades)
                                .map_or(0, |i| (i + 1) % GRENADE_CHOICES.len());
                            self.grenades = GRENADE_CHOICES[next];
                            return None;
                        }
                        OptionRow::Notarget => return Some(OverlayCommand::ToggleNotarget),
                        OptionRow::Ai => return Some(OverlayCommand::ToggleAi),
                        OptionRow::Weapon(_) => {}
                    }
                }
                return Some(OverlayCommand::Spawn(self.request()));
            }
            OverlayKey::Toggle => {}
        }
        None
    }
    /// The overlay spawns at the crosshair like npc_create_aimed (facing the aim).
    pub fn request(&self) -> SpawnRequest {
        SpawnRequest {
            classname: self.class_name().into(),
            name: None,
            equipment: self.weapon.clone(),
            aimed: true,
            allegiance: self.allegiance,
            variant: self.label().into(),
            grenades: reads_num_grenades(self.class_name()).then_some(self.grenades),
        }
    }
    /// The panel to draw; `ai` and `notarget` are the host's live switches.
    pub fn panel(&self, ai: bool, notarget: bool) -> Panel {
        let total = SPAWN_CLASSES.len();
        let first = self
            .class
            .saturating_sub(VISIBLE_CLASSES / 2)
            .min(total.saturating_sub(VISIBLE_CLASSES));
        let last = (first + VISIBLE_CLASSES).min(total);
        let classes_focused = self.column == Column::Classes;
        let note = |text: String| PanelRow {
            text,
            selected: false,
            cursor: false,
        };
        let mut classes = Vec::new();
        if first > 0 {
            classes.push(note(format!("({first} more above)")));
        }
        for (index, class) in SPAWN_CLASSES.iter().enumerate().take(last).skip(first) {
            classes.push(PanelRow {
                text: (*class).into(),
                selected: index == self.class,
                cursor: classes_focused && index == self.class,
            });
        }
        if last < total {
            classes.push(note(format!("({} more below)", total - last)));
        }
        let natural = !crate::npc_weapons::uses_weapons(self.class_name());
        let options = self
            .rows()
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                let (text, selected) = match &row {
                    OptionRow::Weapon(w) if w.is_empty() => (
                        if natural {
                            "no weapons (natural attacks)".into()
                        } else {
                            "no weapon".into()
                        },
                        self.weapon.is_empty(),
                    ),
                    // Where the choice comes from: the class's HL2 map loadouts or
                    // Weapon_CanUse on its model.
                    OptionRow::Weapon(w) => (
                        if crate::npc_weapons::campaign_loadout(self.class_name())
                            .contains(&w.as_str())
                        {
                            format!("{w} (HL2 maps)")
                        } else {
                            format!("{w} (can use)")
                        },
                        *w == self.weapon,
                    ),
                    OptionRow::Allegiance => {
                        (format!("allegiance: {}", self.allegiance.label()), false)
                    }
                    OptionRow::Grenades => {
                        (format!("grenades: {} (NumGrenades)", self.grenades), false)
                    }
                    OptionRow::Notarget => (
                        format!("notarget: {}", if notarget { "ON" } else { "off" }),
                        false,
                    ),
                    OptionRow::Ai => (
                        format!("AI: {}", if ai { "ON" } else { "OFF (idle)" }),
                        false,
                    ),
                };
                PanelRow {
                    text,
                    selected,
                    cursor: !classes_focused && index == self.row,
                }
            })
            .collect();
        Panel {
            title: format!(
                "F4 NPC spawn ({}/{total}): Up/Down move, Left/Right column, Enter spawn / toggle",
                self.class + 1
            ),
            classes,
            options,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_pool_has_slots_for_every_overlay_class() {
        let mut world = modkit_core::World::default();
        append_pool(&mut world);
        assert_eq!(
            world.entities.len(),
            SPAWN_CLASSES.iter().map(|c| pool_count(c)).sum::<usize>()
        );
        assert!(SPAWN_CLASSES
            .iter()
            .all(|c| default_model(spawn_class(c).0).is_some()));
        // Citizen variants: rebel slots carry CT_REBEL and a random head, no model.
        let rebel = free_slot(&world, "npc_citizen_rebel", &Default::default()).unwrap();
        assert_eq!(world.entities[rebel].class(), "npc_citizen");
        assert_eq!(world.entities[rebel].get("citizentype"), Some("3"));
        assert_eq!(world.entities[rebel].get("model"), None);
        let mut used = std::collections::BTreeSet::new();
        for _ in 0..POOL_PER_CLASS {
            used.insert(free_slot(&world, "npc_combine_s", &used).unwrap());
        }
        assert!(free_slot(&world, "npc_combine_s", &used).is_none());
        let cop = free_slot(&world, "npc_metropolice", &used).unwrap();
        assert_eq!(world.entities[cop].get("model"), Some("models/police.mdl"));
    }
    struct Floor;
    impl PlacementWorld for Floor {
        fn trace_line(&self, start: Vec3, end: Vec3) -> Option<Vec3> {
            // Ground plane z = 0, a wall at x = 1000.
            let d = end - start;
            let t_floor = if d.z < 0. {
                -start.z / d.z
            } else {
                f32::INFINITY
            };
            let t_wall = if d.x > 0. {
                (1000. - start.x) / d.x
            } else {
                f32::INFINITY
            };
            let t = t_floor.min(t_wall);
            (t <= 1.).then(|| start + d * t)
        }
        fn drop_to_floor(&self, _: &str, origin: Vec3) -> Option<Vec3> {
            Some(Vec3::new(origin.x, origin.y, 0.))
        }
        fn hull_fits(&self, _: &str, origin: Vec3) -> bool {
            origin.x < 990.
        }
    }
    #[test]
    fn console_commands_follow_source_semantics() {
        let mut console = Console::default();
        assert_eq!(
            console
                .parse("npc_create_equipment weapon_stunstick")
                .unwrap(),
            Some(Command::SetEquipment("weapon_stunstick".into()))
        );
        let Some(Command::Spawn(r)) = console.parse("npc_create npc_metropolice cop1").unwrap()
        else {
            panic!()
        };
        assert_eq!(
            (
                r.classname.as_str(),
                r.name.as_deref(),
                r.equipment.as_str(),
                r.aimed
            ),
            ("npc_metropolice", Some("cop1"), "weapon_stunstick", false)
        );
        let Some(Command::Spawn(r)) = console.parse("npc_create_aimed npc_citizen").unwrap() else {
            panic!()
        };
        assert!(r.aimed && r.name.is_none());
        let Some(Command::Spawn(r)) = console.parse("npc_create npc_citizen_rebel").unwrap() else {
            panic!()
        };
        assert_eq!(
            (r.classname.as_str(), r.pool_label()),
            ("npc_citizen", "npc_citizen_rebel")
        );
        assert!(console.parse("npc_create prop_physics").is_err());
        assert!(console.parse("npc_create").is_err());
        assert_eq!(console.parse("sv_cheats 1").unwrap(), None);
    }
    #[test]
    fn placement_drops_raised_hits_to_the_floor_and_rejects_bad_positions() {
        let eye = Vec3::new(0., 0., 64.);
        let request = |class: &str, aimed| SpawnRequest {
            classname: class.into(),
            name: None,
            equipment: String::new(),
            aimed,
            ..Default::default()
        };
        let down = Vec3::new(1., 0., -0.25).normalize();
        let p = place(&request("npc_metropolice", true), eye, down, &Floor).unwrap();
        assert!(
            (p.origin - Vec3::new(256., 0., 0.)).length() < 1e-2,
            "{p:?}"
        );
        assert_eq!(p.yaw, 0.);
        let p = place(
            &request("npc_metropolice", false),
            eye,
            Vec3::new(0., 1., -0.25),
            &Floor,
        )
        .unwrap();
        assert_eq!(p.yaw, 0.);
        let p = place(&request("npc_manhack", true), eye, down, &Floor).unwrap();
        assert!((p.origin - (Vec3::new(256., 0., 0.) - down * 36.)).length() < 1e-2);
        // Into the wall: the hull does not fit.
        assert!(place(&request("npc_metropolice", true), eye, Vec3::X, &Floor).is_err());
        // Straight up: no hit, the NPC stays at its spawn origin.
        assert_eq!(
            place(&request("npc_citizen", true), eye, Vec3::Z, &Floor)
                .unwrap()
                .origin,
            Vec3::ZERO
        );
    }
    fn usable() -> std::collections::BTreeMap<String, Vec<String>> {
        [
            (
                "npc_metropolice",
                &["weapon_stunstick", "weapon_pistol", "weapon_smg1"][..],
            ),
            ("npc_combine_s", &["weapon_smg1", "weapon_ar2"][..]),
        ]
        .into_iter()
        .map(|(c, w)| (c.to_owned(), w.iter().map(|w| (*w).to_owned()).collect()))
        .collect()
    }
    #[test]
    fn overlay_columns_offer_only_usable_weapons_and_keep_the_selection_visible() {
        let mut o = Overlay::default();
        o.set_usable(usable());
        assert_eq!(o.key(OverlayKey::Up), None, "closed: keys do nothing");
        o.key(OverlayKey::Toggle);
        // The cop starts with its default weapon; the column lists none + usable + rows.
        assert_eq!(o.request().equipment, "weapon_stunstick");
        let panel = o.panel(false, false);
        let texts: Vec<&str> = panel.options.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "no weapon",
                "weapon_stunstick (HL2 maps)",
                "weapon_pistol (HL2 maps)",
                "weapon_smg1 (HL2 maps)",
                "allegiance: class default",
                "notarget: off",
                "AI: OFF (idle)"
            ]
        );
        assert!(panel
            .classes
            .iter()
            .any(|r| r.cursor && r.text == "npc_metropolice"));
        // Options column: Down moves onto the pistol and chooses it.
        o.key(OverlayKey::Right);
        o.key(OverlayKey::Down);
        assert_eq!(o.request().equipment, "weapon_pistol");
        // Enter on allegiance cycles it; on notarget/AI it asks the host.
        o.key(OverlayKey::Down);
        o.key(OverlayKey::Down);
        assert_eq!(o.key(OverlayKey::Enter), None);
        assert_eq!(o.allegiance, Allegiance::Foe);
        o.key(OverlayKey::Down);
        assert_eq!(
            o.key(OverlayKey::Enter),
            Some(OverlayCommand::ToggleNotarget)
        );
        o.key(OverlayKey::Down);
        assert_eq!(o.key(OverlayKey::Enter), Some(OverlayCommand::ToggleAi));
        // On a weapon row (wrapping down to "no weapon"), Enter spawns with the choices.
        o.key(OverlayKey::Down);
        let Some(OverlayCommand::Spawn(r)) = o.key(OverlayKey::Enter) else {
            panic!()
        };
        assert_eq!(
            (
                r.classname.as_str(),
                r.equipment.as_str(),
                r.allegiance,
                r.aimed
            ),
            ("npc_metropolice", "", Allegiance::Foe, true)
        );
        // A zombie: no weapon column, natural attacks only.
        o.key(OverlayKey::Left);
        let zombie = SPAWN_CLASSES
            .iter()
            .position(|c| *c == "npc_zombie")
            .unwrap();
        for _ in 0..zombie {
            o.key(OverlayKey::Down);
        }
        let panel = o.panel(true, true);
        assert_eq!(panel.options[0].text, "no weapons (natural attacks)");
        assert_eq!(panel.options.len(), 4);
        assert_eq!(o.request().equipment, "");
        // The class list scrolls: the selection is always one of the drawn rows.
        o.key(OverlayKey::Up);
        for _ in 0..SPAWN_CLASSES.len() {
            o.key(OverlayKey::Down);
            let panel = o.panel(true, false);
            assert!(panel.classes.len() <= VISIBLE_CLASSES + 2);
            assert!(
                panel
                    .classes
                    .iter()
                    .any(|r| r.selected && r.text == SPAWN_CLASSES[o.class]),
                "{panel:?}"
            );
        }
        // Wrapping up from the first class reaches the last, with "more above".
        while o.class != 0 {
            o.key(OverlayKey::Down);
        }
        o.key(OverlayKey::Up);
        let panel = o.panel(false, false);
        assert!(panel.classes[0].text.contains("more above"));
        assert_eq!(o.request().classname, "npc_combinedropship");
        // Combine soldiers have no stunstick: their default SMG1 is preselected.
        while SPAWN_CLASSES[o.class] != "npc_combine_s" {
            o.key(OverlayKey::Down);
        }
        assert_eq!(o.request().equipment, "weapon_smg1");
        assert_eq!(o.request().grenades, Some(0));
        // Their options add NumGrenades after the allegiance; Enter cycles the count.
        let texts: Vec<String> = o
            .panel(true, false)
            .options
            .into_iter()
            .map(|r| r.text)
            .collect();
        assert_eq!(texts[3], "allegiance: foe (hates the player)");
        assert_eq!(texts[4], "grenades: 0 (NumGrenades)");
        o.key(OverlayKey::Right);
        for _ in 0..3 {
            o.key(OverlayKey::Down);
        }
        for expected in [1, 2, 3, 5, 10, 99, 0, 1] {
            assert_eq!(o.key(OverlayKey::Enter), None);
            assert_eq!(o.grenades, expected);
        }
        assert_eq!(o.request().grenades, Some(1));
        // Classes that never read NumGrenades neither show nor send it.
        o.key(OverlayKey::Left);
        o.key(OverlayKey::Up);
        assert!(o
            .panel(true, false)
            .options
            .iter()
            .all(|r| !r.text.starts_with("grenades")));
        assert_eq!(o.request().grenades, None);
    }
}
