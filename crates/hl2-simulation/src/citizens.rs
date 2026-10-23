//! npc_citizen model selection (SDK 2013 npc_citizen17.cpp CNPC_Citizen::SelectModel),
//! applied to the map's entities before models load. The citizen type (`citizentype`;
//! CT_DEFAULT takes it from the map name), the random-head spawnflags (least-used head
//! among the citizens already spawned, ties at random) and the medic flag choose
//! `models/Humans/<Group01|Group02|Group03[m]>/<head>.mdl`; CT_UNIQUE keeps its model.
//! Native draws heads with an unseeded random stream; ours is seeded per map, so the
//! heads match native's rules but not necessarily native's draw.
use modkit_core::World;

/// g_ppszRandomHeads, in SDK order (no female_05).
pub const RANDOM_HEADS: [&str; 15] = [
    "male_01.mdl",
    "male_02.mdl",
    "female_01.mdl",
    "male_03.mdl",
    "female_02.mdl",
    "male_04.mdl",
    "female_03.mdl",
    "male_05.mdl",
    "female_04.mdl",
    "male_06.mdl",
    "female_06.mdl",
    "male_07.mdl",
    "female_07.mdl",
    "male_08.mdl",
    "male_09.mdl",
];
pub const SF_NPC_START_EFFICIENT: u32 = 1 << 4;
pub const SF_NPC_TEMPLATE: u32 = 1 << 11;
pub const SF_CITIZEN_MEDIC: u32 = 1 << 17;
pub const SF_CITIZEN_RANDOM_HEAD: u32 = 1 << 18;
pub const SF_CITIZEN_RANDOM_HEAD_MALE: u32 = 1 << 22;
pub const SF_CITIZEN_RANDOM_HEAD_FEMALE: u32 = 1 << 23;
/// CitizenType_t.
pub const CT_DEFAULT: i32 = 0;
pub const CT_DOWNTRODDEN: i32 = 1;
pub const CT_REFUGEE: i32 = 2;
pub const CT_REBEL: i32 = 3;
pub const CT_UNIQUE: i32 = 4;

/// FIELD_INTEGER keyvalue parsing (atoi: a leading integer, else 0; Hammer writes
/// "Default" for CT_DEFAULT in d1_trainstation_02).
fn atoi(value: &str) -> i64 {
    let value = value.trim();
    let end = value
        .char_indices()
        .find(|(i, c)| !(c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+'))))
        .map_or(value.len(), |(i, _)| i);
    value[..end].parse().unwrap_or(0)
}
/// The citizen type after CT_DEFAULT's map-name mapping.
pub fn citizen_type(citizentype: Option<&str>, map: &str) -> i32 {
    let ty = citizentype.map_or(0, |v| atoi(v) as i32);
    if ty != CT_DEFAULT {
        return ty;
    }
    let map = map.to_lowercase();
    const MAPPINGS: [(&str, i32); 7] = [
        ("trainstation", CT_DOWNTRODDEN),
        ("canals", CT_REFUGEE),
        ("town", CT_REFUGEE),
        ("coast", CT_REFUGEE),
        ("prison", CT_DOWNTRODDEN),
        ("c17", CT_REBEL),
        ("citadel", CT_DOWNTRODDEN),
    ];
    MAPPINGS
        .iter()
        .find(|(tag, _)| map.contains(tag))
        .map_or(CT_DOWNTRODDEN, |(_, ty)| *ty)
}
/// g_ppszModelLocs[type] with the medic suffix.
fn group(ty: i32, medic: bool) -> String {
    match ty {
        CT_REFUGEE => "Group02".into(),
        CT_REBEL => format!("Group03{}", if medic { "m" } else { "" }),
        _ => "Group01".into(),
    }
}
/// The head index of a model path's file name, if it is one of the random heads.
fn head_of(model: &str) -> Option<usize> {
    let name = model.rsplit('/').next()?;
    RANDOM_HEADS
        .iter()
        .position(|h| h.eq_ignore_ascii_case(name))
}
/// A deterministic stream for the RandomInt draws.
struct Random(u64);
impl Random {
    fn int(&mut self, low: usize, high: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        low + (self.0 % (high - low + 1) as u64) as usize
    }
}
/// CNPC_Citizen::SelectModel for every npc_citizen, in spawn order: map citizens first
/// (entity order), then templates (spawned later by their makers, so every map
/// citizen's head counts), then entities marked with `late_key` (the F4 pool).
/// Returns the number of citizens whose model was chosen.
pub fn select_models(world: &mut World, late_key: &str) -> usize {
    let map = world.name.clone();
    let seed = map.bytes().fold(0x9e37_79b9_7f4a_7c15u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    let mut random = Random(seed | 1);
    let flags =
        |e: &modkit_core::Entity| -> u32 { e.get("spawnflags").map_or(0, |v| atoi(v) as u32) };
    let mut order: Vec<(u8, usize)> = world
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.class().eq_ignore_ascii_case("npc_citizen"))
        .map(|(id, e)| {
            let rank = if e.get(late_key).is_some() {
                2
            } else if flags(e) & SF_NPC_TEMPLATE != 0 {
                1
            } else {
                0
            };
            (rank, id)
        })
        .collect();
    order.sort();
    let mut heads = [0usize; RANDOM_HEADS.len()];
    let mut chosen = 0;
    for (_, id) in order {
        let e = &world.entities[id];
        let spawnflags = flags(e);
        let ty = citizen_type(e.get("citizentype"), &map);
        let medic = spawnflags & SF_CITIZEN_MEDIC != 0;
        let model = e.get("model").unwrap_or("").to_owned();
        let random_flags =
            SF_CITIZEN_RANDOM_HEAD | SF_CITIZEN_RANDOM_HEAD_MALE | SF_CITIZEN_RANDOM_HEAD_FEMALE;
        let (name, head) = if spawnflags & random_flags != 0 || model.is_empty() {
            if spawnflags & SF_NPC_START_EFFICIENT != 0 {
                set_model(world, id, "models/humans/male_cheaple.mdl");
                chosen += 1;
                continue;
            }
            let gender = if spawnflags & SF_CITIZEN_RANDOM_HEAD_MALE != 0 {
                Some('m')
            } else if spawnflags & SF_CITIZEN_RANDOM_HEAD_FEMALE != 0 {
                Some('f')
            } else {
                None
            };
            // Candidates sorted by how many citizens wear them (stable: SDK order breaks
            // ties before the random pick among the least used).
            let mut candidates: Vec<(usize, usize)> = (0..RANDOM_HEADS.len())
                .filter(|i| gender.is_none_or(|g| RANDOM_HEADS[*i].starts_with(g)))
                .map(|i| (i, heads[i]))
                .collect();
            candidates.sort_by_key(|(_, n)| *n);
            let least = candidates[0].1;
            let limit = candidates.iter().take_while(|(_, n)| *n == least).count();
            let head = candidates[random.int(0, limit - 1)].0;
            (RANDOM_HEADS[head].to_owned(), Some(head))
        } else {
            let name = model.rsplit('/').next().unwrap_or(&model).to_owned();
            (name.clone(), head_of(&name))
        };
        if let Some(head) = head {
            heads[head] += 1;
        }
        if ty != CT_UNIQUE && !name.is_empty() {
            let path = format!("models/Humans/{}/{name}", group(ty, medic)).to_lowercase();
            set_model(world, id, &path);
            chosen += 1;
        }
    }
    chosen
}
/// CNPC_Citizen::SelectModel for a runtime spawn: the template (pooled citizen) whose
/// head is worn by the fewest living citizens (`alive`; pooled and SF_NPC_TEMPLATE
/// entities are not citizens in the world), a random one (`random` in [0, 1)) among the
/// ties. None without templates.
pub fn least_used_template(
    world: &World,
    templates: &[usize],
    pool_key: &str,
    alive: impl Fn(usize) -> bool,
    random: f32,
) -> Option<usize> {
    let head = |id: usize| {
        world
            .entities
            .get(id)
            .and_then(|e| head_of(e.get("model")?))
    };
    let mut counts = [0usize; RANDOM_HEADS.len()];
    for (id, e) in world.entities.iter().enumerate() {
        let flags = e.get("spawnflags").map_or(0, |v| atoi(v) as u32);
        if e.class().eq_ignore_ascii_case("npc_citizen")
            && e.get(pool_key).is_none()
            && flags & SF_NPC_TEMPLATE == 0
            && alive(id)
        {
            if let Some(h) = head(id) {
                counts[h] += 1;
            }
        }
    }
    let count = |id: usize| head(id).map_or(usize::MAX, |h| counts[h]);
    let least = templates.iter().map(|&t| count(t)).min()?;
    let ties: Vec<usize> = templates
        .iter()
        .copied()
        .filter(|&t| count(t) == least)
        .collect();
    ties.get(((random * ties.len() as f32) as usize).min(ties.len() - 1))
        .copied()
}
fn set_model(world: &mut World, id: usize, model: &str) {
    let properties = &mut world.entities[id].properties;
    match properties
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case("model"))
    {
        Some((_, v)) => *v = model.to_owned(),
        None => properties.push(("model".into(), model.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn citizen(pairs: &[(&str, &str)]) -> modkit_core::Entity {
        let mut properties = vec![("classname".to_owned(), "npc_citizen".to_owned())];
        properties.extend(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        );
        modkit_core::Entity { properties }
    }
    #[test]
    fn the_trainstation_queue_gets_three_different_heads() {
        // d1_trainstation_02 #580-582: male_08 with SF_CITIZEN_RANDOM_HEAD (266240).
        let mut world = World {
            name: "d1_trainstation_02".into(),
            entities: vec![
                citizen(&[
                    ("model", "models/humans/group01/male_02.mdl"),
                    ("spawnflags", "4096"),
                ]),
                citizen(&[
                    ("model", "models/humans/group01/male_08.mdl"),
                    ("spawnflags", "266240"),
                    ("citizentype", "Default"),
                ]),
                citizen(&[
                    ("model", "models/humans/group01/male_08.mdl"),
                    ("spawnflags", "266240"),
                ]),
                citizen(&[
                    ("model", "models/humans/group01/male_08.mdl"),
                    ("spawnflags", "266240"),
                ]),
            ],
            ..Default::default()
        };
        assert_eq!(select_models(&mut world, "pool"), 4);
        let models: Vec<&str> = world
            .entities
            .iter()
            .map(|e| e.get("model").unwrap())
            .collect();
        assert_eq!(models[0], "models/humans/group01/male_02.mdl");
        let mut queue = models[1..].to_vec();
        queue.sort();
        queue.dedup();
        assert_eq!(queue.len(), 3, "{models:?}");
        // male_02 is worn already: the least-used heads exclude it.
        assert!(
            models[1..].iter().all(|m| !m.ends_with("male_02.mdl")),
            "{models:?}"
        );
        assert!(models
            .iter()
            .all(|m| m.starts_with("models/humans/group01/")));
    }
    #[test]
    fn runtime_spawns_take_the_least_used_head() {
        // Map citizens wear male_01 twice and female_01 once; templates male_01,
        // female_01, female_02: female_02 (unworn) first, then male/female ties.
        let mut world = World {
            name: "d1_trainstation_02".into(),
            entities: vec![
                citizen(&[("model", "models/humans/group01/male_01.mdl")]),
                citizen(&[("model", "models/humans/group01/male_01.mdl")]),
                citizen(&[("model", "models/humans/group01/female_01.mdl")]),
                citizen(&[
                    ("model", "models/humans/group01/male_01.mdl"),
                    ("pool", "npc_citizen"),
                ]),
                citizen(&[
                    ("model", "models/humans/group01/female_01.mdl"),
                    ("pool", "npc_citizen"),
                ]),
                citizen(&[
                    ("model", "models/humans/group01/female_02.mdl"),
                    ("pool", "npc_citizen"),
                ]),
            ],
            ..Default::default()
        };
        let templates = [3, 4, 5];
        assert_eq!(
            least_used_template(&world, &templates, "pool", |_| true, 0.9),
            Some(5)
        );
        // A spawned female_02 citizen: now female_01 and female_02 tie at one.
        world
            .entities
            .push(citizen(&[("model", "models/humans/group01/female_02.mdl")]));
        assert_eq!(
            least_used_template(&world, &templates, "pool", |_| true, 0.),
            Some(4)
        );
        assert_eq!(
            least_used_template(&world, &templates, "pool", |_| true, 0.9),
            Some(5)
        );
        // Dead citizens do not count: with the map's female_01 dead it is least used.
        assert_eq!(
            least_used_template(&world, &templates, "pool", |id| id != 2, 0.9),
            Some(4)
        );
    }
    #[test]
    fn types_genders_medics_and_unique_follow_select_model() {
        let mut world = World {
            name: "d3_c17_01".into(),
            entities: vec![
                // CT_DEFAULT on a c17 map: rebel; a fixed head moves to Group03.
                citizen(&[("model", "models/humans/group01/female_02.mdl")]),
                citizen(&[(
                    "spawnflags",
                    &(SF_CITIZEN_RANDOM_HEAD_FEMALE | SF_CITIZEN_MEDIC).to_string(),
                )]),
                citizen(&[
                    ("citizentype", "2"),
                    ("spawnflags", &SF_CITIZEN_RANDOM_HEAD_MALE.to_string()),
                ]),
                citizen(&[("citizentype", "4"), ("model", "models/odessa.mdl")]),
            ],
            ..Default::default()
        };
        select_models(&mut world, "pool");
        let m = |i: usize| world.entities[i].get("model").unwrap().to_owned();
        assert_eq!(m(0), "models/humans/group03/female_02.mdl");
        assert!(
            m(1).starts_with("models/humans/group03m/female_"),
            "{}",
            m(1)
        );
        assert!(m(2).starts_with("models/humans/group02/male_"), "{}", m(2));
        assert_eq!(m(3), "models/odessa.mdl");
        assert_eq!(citizen_type(Some("Default"), "d1_canals_01"), CT_REFUGEE);
        assert_eq!(citizen_type(None, "d1_trainstation_01"), CT_DOWNTRODDEN);
        assert_eq!(citizen_type(Some("3"), "d1_trainstation_01"), CT_REBEL);
    }
}
