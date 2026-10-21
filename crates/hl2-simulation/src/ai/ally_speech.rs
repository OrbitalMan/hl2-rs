//! Player-ally speech (SDK 2013 ai_playerally.cpp CAI_PlayerAlly::SpeakIfAllowed /
//! IsAllowedToSpeak / IsOkToSpeak and ai_speech.cpp CAI_Expresser): citizens, rebels,
//! Barney and Alyx answer concepts (TLK_WOUND, TLK_STARTCOMBAT, ...) through the response
//! rules in scripts/talker (`talker::Talker`). Scene responses play their speak event's
//! sound; choreographed gestures and lip-synced scene playback are not used here.
use super::talker::{CriteriaSet, ResponseKind, Talker, CONCEPT_WEIGHT};
use std::collections::BTreeMap;

/// sv_npc_talker_maxdist: allies farther from the player stay quiet (non-priority).
pub const TALKER_MAX_DIST: f32 = 1024.;
/// How long a `speak`/`sentence` line holds the speaker and the friendly talk semaphore
/// (an estimate: the wave length is not known to the simulation); scene responses use
/// their scene's length.
const SPEAK_SECONDS: f64 = 2.5;

/// ConceptInfo_t rows of g_ConceptInfos for the concepts spoken here: (concept,
/// SPEECH_PRIORITY, minConceptDelay, maxConceptDelay); -1 = no delay. Category delays are
/// -1 for all of them.
const CONCEPTS: [(&str, bool, f32, f32); 4] = [
    ("TLK_WOUND", false, -1., -1.),
    ("TLK_ENEMY_DEAD", false, -1., -1.),
    ("TLK_STARTCOMBAT", false, 30., 0.),
    ("TLK_HIDEANDRELOAD", true, 45., 60.),
];

/// What a speaker contributes to the criteria set (ModifyOrAppendCriteria).
pub struct Speaker<'a> {
    pub id: usize,
    pub classname: &'a str,
    pub name: &'a str,
    pub health: f32,
    pub max_health: f32,
    /// "Idle", "Alert", "Combat" ...
    pub npc_state: &'a str,
    pub enemy: Option<&'a str>,
    pub weapon: Option<&'a str>,
    pub distance_to_player: f32,
    pub map: &'a str,
    /// "male" / "female" ($gender in scene paths).
    pub gender: &'a str,
    pub gagged: bool,
    pub now: f64,
}

#[derive(Clone, Debug, Default)]
struct NpcSpeech {
    speaking_until: f64,
    /// Concept -> (respeak allowed after, spoke once with speakonce).
    history: BTreeMap<String, (f64, bool)>,
    /// GetLastEnemyTime.
    last_enemy_time: Option<f64>,
    last_schedule: Option<String>,
}

/// The ally speech manager (CAI_AllySpeechManager) and expressers.
#[derive(Clone, Debug, Default)]
pub struct AllySpeech {
    talker: Option<Talker>,
    /// Scene path ($gender expanded, lowercase) -> its first speak event's sound and the
    /// scene's length.
    scene_sounds: BTreeMap<String, (String, f32)>,
    /// Global per-concept delays (ConceptDelayExpired).
    concept_until: BTreeMap<&'static str, f64>,
    /// g_AIFriendliesTalkSemaphore: (holder, until).
    semaphore: Option<(usize, f64)>,
    npcs: BTreeMap<usize, NpcSpeech>,
    pub spoken: u64,
    pub refused: u64,
}

impl AllySpeech {
    /// Load the talker rules and resolve the scene responses of the concepts used here.
    pub fn load(vfs: &source_assets::vpk::Vfs) -> Self {
        let read = |p: &str| {
            vfs.read(p)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        };
        let talker = Talker::load("scripts/talker/response_rules.txt", &read);
        let mut scene_sounds = BTreeMap::new();
        let concepts: Vec<&str> = CONCEPTS.iter().map(|c| c.0).collect();
        let scenes: Vec<String> = talker
            .responses_for_concepts(&concepts)
            .into_iter()
            .filter(|(kind, _)| *kind == ResponseKind::Scene)
            .flat_map(|(_, path)| {
                ["male", "female"].map(|g| path.replace("$gender", g).to_ascii_lowercase())
            })
            .collect();
        if let Some(cache) = vfs
            .read("scenes/scenes.image")
            .ok()
            .flatten()
            .and_then(|b| source_assets::scenes::Cache::parse(b).ok())
        {
            for path in scenes {
                let sound = cache.scene(&path).ok().flatten().and_then(|scene| {
                    scene
                        .events
                        .iter()
                        .filter(|e| e.kind == source_assets::scenes::EventType::Speak)
                        .min_by(|a, b| a.start.total_cmp(&b.start))
                        .map(|e| (e.parameters[0].clone(), scene.stop_time()))
                });
                if let Some(sound) = sound.filter(|s| !s.0.is_empty()) {
                    scene_sounds.insert(path, sound);
                }
            }
        }
        Self {
            talker: Some(talker),
            scene_sounds,
            ..Default::default()
        }
    }
    /// The sound cues these concepts can play (audio preload) plus the death sounds.
    pub fn cues(&self) -> Vec<String> {
        let mut cues: Vec<String> = self.scene_sounds.values().map(|s| s.0.clone()).collect();
        if let Some(talker) = &self.talker {
            let concepts: Vec<&str> = CONCEPTS.iter().map(|c| c.0).collect();
            cues.extend(
                talker
                    .responses_for_concepts(&concepts)
                    .into_iter()
                    .filter(|(kind, _)| *kind == ResponseKind::Speak)
                    .map(|(_, v)| v),
            );
        }
        cues.extend(["NPC_Citizen.Die", "npc_barney.die", "npc_alyx.die"].map(String::from));
        cues.sort();
        cues.dedup();
        cues
    }
    /// GatherEnemyConditions: TLK_STARTCOMBAT when an enemy is seen and the last enemy
    /// was more than 30 s ago (or never); remembers the enemy time.
    pub fn start_combat(&mut self, id: usize, has_enemy: bool, sees_enemy: bool, now: f64) -> bool {
        let npc = self.npcs.entry(id).or_default();
        let start = sees_enemy && npc.last_enemy_time.is_none_or(|t| now - t > 30.);
        if has_enemy {
            npc.last_enemy_time = Some(now);
        }
        start
    }
    /// OnStartSchedule: true when `schedule` just started.
    pub fn schedule_started(&mut self, id: usize, schedule: Option<&str>) -> bool {
        let npc = self.npcs.entry(id).or_default();
        let changed = npc.last_schedule.as_deref() != schedule;
        npc.last_schedule = schedule.map(str::to_owned);
        changed && schedule.is_some()
    }
    /// SpeakIfAllowed(concept): the sound to play at the speaker, if allowed and a rule
    /// answers, else why not. `random` draws in [0, 1).
    pub fn speak(
        &mut self,
        concept: &'static str,
        speaker: &Speaker,
        random: &mut dyn FnMut() -> f32,
    ) -> Result<String, &'static str> {
        let now = speaker.now;
        let info = CONCEPTS.iter().find(|c| c.0 == concept);
        let priority = info.is_some_and(|c| c.1);
        let npc = self.npcs.entry(speaker.id).or_default();
        // IsOkToSpeak / IsAllowedToSpeak.
        let refusal = if speaker.health <= 0. || speaker.gagged {
            Some("dead_or_gagged")
        } else if !priority
            && self
                .semaphore
                .is_some_and(|(holder, until)| holder != speaker.id && now < until)
        {
            Some("semaphore")
        } else if !priority && now < npc.speaking_until {
            Some("speaking")
        } else if !priority && speaker.distance_to_player > TALKER_MAX_DIST {
            Some("far_from_player")
        } else if self.concept_until.get(concept).is_some_and(|&t| now < t) {
            Some("concept_delay")
        } else if npc
            .history
            .get(concept)
            .is_some_and(|&(until, once)| once || now < until)
        {
            // CanSpeakConcept: speakonce and respeakdelay of the last response.
            Some("respeak")
        } else {
            None
        };
        if let Some(reason) = refusal {
            self.refused += 1;
            return Err(reason);
        }
        let mut set = CriteriaSet::default();
        set.append("concept", concept, CONCEPT_WEIGHT);
        set.append("randomnum", format!("{}", (random() * 101.) as i32), 1.);
        set.append("map", speaker.map, 1.);
        set.append("classname", speaker.classname, 1.);
        set.append("name", speaker.name, 1.);
        set.append("health", format!("{}", speaker.health as i32), 1.);
        set.append(
            "healthfrac",
            format!("{:.3}", speaker.health / speaker.max_health.max(1.)),
            1.,
        );
        set.append("npcstate", format!("[NPCState::{}]", speaker.npc_state), 1.);
        if let Some(enemy) = speaker.enemy {
            set.append("enemy", enemy, 1.);
            set.append("timesincecombat", "-1", 1.);
        }
        set.append("weapon", speaker.weapon.unwrap_or("none"), 1.);
        set.append(
            "distancetoplayer",
            format!("{:.6}", speaker.distance_to_player),
            1.,
        );
        let chosen = self
            .talker
            .as_mut()
            .and_then(|t| t.find_best(&set, random))
            .ok_or("no_rule")?;
        // RG_ODDS: speak only that percent of the time.
        if chosen
            .params
            .odds
            .is_some_and(|odds| (random() * 100.) as i32 + 1 > odds)
        {
            return Err("odds");
        }
        let (sound, seconds) = match chosen.kind {
            ResponseKind::Scene => self
                .scene_sounds
                .get(
                    &chosen
                        .value
                        .replace("$gender", speaker.gender)
                        .to_ascii_lowercase(),
                )
                .map(|(s, t)| (s.clone(), f64::from(*t)))
                .ok_or("no_scene_sound")?,
            ResponseKind::Speak => (chosen.value.clone(), SPEAK_SECONDS),
            ResponseKind::Sentence => (
                format!("!{}", chosen.value.trim_start_matches('!')),
                SPEAK_SECONDS,
            ),
            _ => return Err("response_type"),
        };
        // Speaking holds the speaker and the semaphore; the group delay follows it.
        let delay = chosen
            .params
            .delay
            .map_or(2.8 + 0.4 * random(), |(start, range)| {
                start + range * random()
            });
        let npc = self.npcs.entry(speaker.id).or_default();
        npc.speaking_until = now + seconds;
        self.semaphore = Some((speaker.id, now + seconds + f64::from(delay)));
        let respeak = chosen
            .params
            .respeak_delay
            .map_or(0., |(start, range)| start + range * random());
        npc.history.insert(
            concept.to_owned(),
            (now + f64::from(respeak), chosen.params.speak_once),
        );
        if let Some(&(_, _, min, max)) = info.filter(|c| c.2 >= 0.) {
            // RandomFloat(min, max) (STARTCOMBAT's 30, 0 gives 0..30).
            self.concept_until
                .insert(concept, now + f64::from(min + (max - min) * random()));
        }
        self.spoken += 1;
        Ok(sound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn speech() -> AllySpeech {
        let rules = r#"
criterion "IsCitizen" "classname" "npc_citizen" "required"
criterion "ConceptTalkWound" "Concept" "TLK_WOUND" required
criterion "ConceptStartCombat" "Concept" "TLK_STARTCOMBAT" required
response CitizenWound
{
	scene "scenes/npc/$gender01/ow01.vcd"
}
response CitizenStartCombat speakonce speak "npc_citizen.headsup01"
rule CitizenWound
{
	criteria IsCitizen ConceptTalkWound
	response CitizenWound
}
rule CitizenStartCombat
{
	criteria IsCitizen ConceptStartCombat
	response CitizenStartCombat
}
"#;
        let talker = Talker::load("r", &|_| Some(rules.to_owned()));
        let mut scene_sounds = BTreeMap::new();
        scene_sounds.insert(
            "scenes/npc/female01/ow01.vcd".to_string(),
            ("npc_citizen.ow01".to_string(), 1.),
        );
        AllySpeech {
            talker: Some(talker),
            scene_sounds,
            ..Default::default()
        }
    }
    fn speaker(id: usize, now: f64, distance: f32) -> Speaker<'static> {
        Speaker {
            id,
            classname: "npc_citizen",
            name: "",
            health: 30.,
            max_health: 40.,
            npc_state: "Combat",
            enemy: Some("npc_combine_s"),
            weapon: Some("weapon_smg1"),
            distance_to_player: distance,
            map: "d1_trainstation_02",
            gender: "female",
            gagged: false,
            now,
        }
    }
    #[test]
    fn allies_speak_through_the_rules_with_the_sdk_gates() {
        let mut s = speech();
        let mut r = || 0.5;
        // A female citizen's scene response plays its speak event's sound.
        assert_eq!(
            s.speak("TLK_WOUND", &speaker(1, 0., 300.), &mut r),
            Ok("npc_citizen.ow01".into())
        );
        // Another ally waits for the friendly talk semaphore (the 1 s scene plus the
        // default 2.8-3.2 s delay); too far never speaks.
        assert_eq!(
            s.speak("TLK_WOUND", &speaker(2, 1., 300.), &mut r),
            Err("semaphore")
        );
        assert_eq!(
            s.speak("TLK_WOUND", &speaker(2, 10., 2000.), &mut r),
            Err("far_from_player")
        );
        // STARTCOMBAT: a global 0..30 s concept delay, and speakonce for this speaker.
        assert_eq!(
            s.speak("TLK_STARTCOMBAT", &speaker(2, 10., 300.), &mut r),
            Ok("npc_citizen.headsup01".into())
        );
        assert_eq!(
            s.speak("TLK_STARTCOMBAT", &speaker(3, 20., 300.), &mut r),
            Err("concept_delay")
        );
        assert_eq!(
            s.speak("TLK_STARTCOMBAT", &speaker(2, 100., 300.), &mut r),
            Err("respeak")
        );
        assert!(s
            .speak("TLK_STARTCOMBAT", &speaker(3, 100., 300.), &mut r)
            .is_ok());
        // GatherEnemyConditions' 30 s rule and schedule starts.
        assert!(s.start_combat(4, true, true, 0.));
        assert!(!s.start_combat(4, true, true, 10.));
        assert!(s.start_combat(4, false, true, 50.));
        assert!(s.schedule_started(4, Some("SCHED_HIDE_AND_RELOAD")));
        assert!(!s.schedule_started(4, Some("SCHED_HIDE_AND_RELOAD")));
        assert_eq!(s.spoken, 3);
    }
}
