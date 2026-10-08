//! Preload owned cue alternatives before App::run; systems use only memory/assets.
use anyhow::{Context, Result, bail};
use bevy::{audio::AudioSinkPlayback, prelude::*};
use hl2_simulation::sounds::{Library, SoundRequest};
use source_assets::vpk::Vfs;
use std::collections::{BTreeSet, HashMap};

const MAX_PRELOADED_BYTES: usize = 512 * 1024 * 1024;
const MAX_PLAYERS: usize = 256;

struct Ambient {
    request: SoundRequest,
    looped: bool,
    volume: f32,
}
pub struct PreparedAudio {
    library: Library,
    waves: Vec<(String, Vec<u8>)>,
    ambient: Vec<Ambient>,
    bytes: usize,
    cues: usize,
    /// Speech phoneme data (wave path, sentence, seconds) for lip sync.
    pub sentences: Vec<(String, source_assets::sentence::Sentence, f32)>,
}
impl PreparedAudio {
    pub fn load(vfs: &Vfs, game: &crate::gameplay::Gameplay) -> Self {
        let mut library = Library::new(vfs);
        let mut cues = BTreeSet::new();
        let mut ambient = Vec::new();
        for entity in &game.world.entities {
            if entity.class() != "ambient_generic" {
                continue;
            }
            if let Some(name) = entity.get("message").filter(|name| !name.is_empty()) {
                cues.insert(name.to_owned());
                let flags = entity
                    .get("spawnflags")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                if flags & 16 == 0 {
                    let volume = entity
                        .get("health")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(5.)
                        / 10.;
                    ambient.push(Ambient {
                        request: name.into(),
                        looped: flags & 32 == 0,
                        volume: volume * 0.3,
                    });
                }
            }
        }
        cues.extend(
            game.scene
                .required_sound_requests(&game.world)
                .into_iter()
                .map(|r| r.name),
        );
        cues.extend(
            game.impacts
                .required_sound_requests()
                .into_iter()
                .map(|r| r.name),
        );
        for weapon in game.weapons.values() {
            cues.extend(weapon.sounds.values().cloned());
        }
        for rig in game.world.rigs.values() {
            for clip in rig.clips.values() {
                cues.extend(
                    clip.events
                        .iter()
                        .filter(|e| {
                            (e.id == 5004 || e.name == "AE_CL_PLAYSOUND") && !e.options.is_empty()
                        })
                        .map(|e| e.options.clone()),
                );
            }
        }
        cues.extend(
            [
                "Player.WeaponSelectionMoveSlot",
                "Player.WeaponSelectionClose",
                "Player.WeaponSelected",
                "Player.DenyWeaponSelection",
                "HUDQuickInfo.LowAmmo",
                "HUDQuickInfo.LowHealth",
                "NPC_CombineBall.Launch",
                "NPC_CombineBall.KillImpact",
                "NPC_CombineBall.Impact",
                "NPC_CombineBall.WhizFlyby",
                "NPC_CombineBall.Explosion",
                "BaseExplosionEffect.Sound",
                "HealthKit.Touch",
                "HealthVial.Touch",
                "ItemBattery.Touch",
                "BaseCombatCharacter.AmmoPickup",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let mut paths = BTreeSet::new();
        for cue in &cues {
            match library.all_alternatives(cue) {
                Ok(variants) => paths.extend(variants),
                Err(error) => {
                    library.errors.insert(cue.clone(), format!("{error:#}"));
                }
            }
        }
        let mut waves = vec![];
        let mut bytes = 0usize;
        let mut sentences = Vec::new();
        for path in paths {
            let decoded = (|| -> Result<_> {
                let encoded = vfs
                    .read(&format!("sound/{path}"))?
                    .context("owned sound absent")?;
                let sentence = match source_assets::sentence::vdat_chunk(&encoded) {
                    Ok(Some(chunk)) => Some(source_assets::sentence::Sentence::parse(chunk)?),
                    _ => None,
                };
                let (data, summary) = hl2_simulation::sounds::decode(&path, encoded)?;
                if data.len() > MAX_PRELOADED_BYTES.saturating_sub(bytes) {
                    bail!("owned audio preload exceeds 512 MiB budget");
                }
                Ok((data, summary, sentence))
            })();
            match decoded {
                Ok((data, summary, sentence)) => {
                    if let Some(sentence) = sentence {
                        sentences.push((path.clone(), sentence, summary.seconds() as f32));
                    }
                    bytes += data.len();
                    library.decoded.insert(path.clone(), summary);
                    waves.push((path, data));
                }
                Err(error) => {
                    library.errors.insert(path, format!("{error:#}"));
                }
            }
        }
        Self {
            library,
            waves,
            ambient,
            bytes,
            cues: cues.len(),
            sentences,
        }
    }
}

#[derive(Resource)]
pub struct Audio {
    library: Library,
    handles: HashMap<String, Handle<AudioSource>>,
    bytes: usize,
    cues: usize,
    requested: u64,
    started: u64,
    failed: u64,
    capacity_rejections: u64,
    frame: u64,
    pending_sinks: usize,
    active_sinks: usize,
    paused_sinks: usize,
    paused: bool,
    pause_changes: u64,
    resume_changes: u64,
}
#[derive(Component)]
pub struct SoundPlayer {
    path: String,
    queued_frame: u64,
    observed: bool,
}
impl Audio {
    fn emit(
        &mut self,
        commands: &mut Commands,
        request: &SoundRequest,
        looped: bool,
        volume: f32,
        paused: bool,
    ) -> Option<String> {
        let path = match self.library.resolve(request) {
            Ok(path) => path,
            Err(error) => {
                self.library
                    .errors
                    .insert(request.name.clone(), format!("{error:#}"));
                self.failed += 1;
                return None;
            }
        };
        let Some(handle) = self.handles.get(&path) else {
            self.library.errors.entry(path).or_insert_with(|| {
                "cue variant was not preloaded; no file reads are allowed in playback systems"
                    .into()
            });
            self.failed += 1;
            return None;
        };
        let settings = if looped {
            PlaybackSettings::LOOP
        } else {
            PlaybackSettings::DESPAWN
        };
        commands.spawn((
            crate::campaign::MapOwned,
            SoundPlayer {
                path: path.clone(),
                queued_frame: self.frame,
                observed: false,
            },
            AudioPlayer::new(handle.clone()),
            PlaybackSettings {
                volume: bevy::audio::Volume::Linear(volume.clamp(0., 1.)),
                paused,
                ..settings
            },
        ));
        self.requested += 1;
        Some(path)
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"preloaded_waves":self.handles.len(),"preloaded_bytes":self.bytes,"referenced_cues":self.cues,
            "requested":self.requested,"started_sinks":self.started,"failed_requests":self.failed,"capacity_rejections":self.capacity_rejections,
            "pending_sinks":self.pending_sinks,"active_sinks":self.active_sinks,"paused_sinks":self.paused_sinks,
            "pause_changes":self.pause_changes,"resume_changes":self.resume_changes,
            "variants":self.library.variants_played,"decoded":self.library.decoded,"errors":self.library.errors,
            "mixing":"retained 2D volume policy; Source spatialization/DSP/soundscapes are not implemented"})
    }
}
pub fn install(
    commands: &mut Commands,
    sources: &mut Assets<AudioSource>,
    prepared: PreparedAudio,
) {
    let handles = prepared
        .waves
        .into_iter()
        .map(|(path, bytes)| {
            (
                path,
                sources.add(AudioSource {
                    bytes: bytes.into(),
                }),
            )
        })
        .collect();
    let mut audio = Audio {
        library: prepared.library,
        handles,
        bytes: prepared.bytes,
        cues: prepared.cues,
        requested: 0,
        started: 0,
        failed: 0,
        capacity_rejections: 0,
        frame: 0,
        pending_sinks: 0,
        active_sinks: 0,
        paused_sinks: 0,
        paused: false,
        pause_changes: 0,
        resume_changes: 0,
    };
    for ambient in prepared.ambient {
        if audio.requested >= MAX_PLAYERS as u64 {
            audio.capacity_rejections += 1;
            continue;
        }
        audio.emit(
            commands,
            &ambient.request,
            ambient.looped,
            ambient.volume,
            false,
        );
    }
    commands.insert_resource(audio);
}
pub fn queue(
    mut commands: Commands,
    mut audio: ResMut<Audio>,
    mut game: ResMut<crate::gameplay::Gameplay>,
    simulation: Res<crate::movement::Simulation>,
    mut players: Query<(&mut PlaybackSettings, Option<&AudioSink>), With<SoundPlayer>>,
) {
    let paused = simulation.paused();
    if paused != audio.paused {
        if paused {
            audio.pause_changes += 1;
        } else {
            audio.resume_changes += 1;
        }
        audio.paused = paused;
    }
    let mut count = 0;
    for (mut settings, sink) in &mut players {
        count += 1;
        settings.paused = paused;
        if let Some(sink) = sink {
            if paused && !sink.is_paused() {
                sink.pause();
            } else if !paused && sink.is_paused() {
                sink.play();
            }
        }
    }
    for request in std::mem::take(&mut game.sound_requests) {
        if count >= MAX_PLAYERS {
            audio.capacity_rejections += 1;
            game.unplayed_sounds += 1;
        } else if let Some(path) = audio.emit(&mut commands, &request, false, 0.4, paused) {
            count += 1;
            // Actor speech drives lip sync from the chosen wave's phonemes.
            if let Some(actor) = request.actor.as_ref().and_then(|a| a.entity) {
                let time = game.scene.time;
                game.scene.lipsync.start(actor, &path, time);
            }
        } else {
            game.unplayed_sounds += 1;
        }
    }
}
pub fn observe(
    mut commands: Commands,
    mut audio: ResMut<Audio>,
    mut players: Query<(Entity, &mut SoundPlayer, Option<&AudioSink>)>,
) {
    audio.frame += 1;
    audio.pending_sinks = 0;
    audio.active_sinks = 0;
    audio.paused_sinks = 0;
    for (entity, mut player, sink) in &mut players {
        if let Some(sink) = sink {
            audio.active_sinks += 1;
            audio.paused_sinks += usize::from(sink.is_paused());
            if !player.observed {
                player.observed = true;
                audio.started += 1;
                audio.library.played += 1;
                *audio
                    .library
                    .variants_played
                    .entry(player.path.clone())
                    .or_default() += 1;
            }
        } else {
            audio.pending_sinks += 1;
            if audio.frame.saturating_sub(player.queued_frame) > 120 {
                audio.library.errors.insert(player.path.clone(), "playback sink did not start within 120 frames; audio device/backend may be unavailable".into());
                audio.failed += 1;
                commands.entity(entity).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::{Decodable, Source};

    #[test]
    #[ignore = "requires an owned installed Half-Life 2 copy"]
    fn installed_weapon_selection_backend_preserves_full_duration() {
        let root = source_assets::install::discover().unwrap();
        let vfs = Vfs::mount(&root).unwrap();
        let library = Library::new(&vfs);
        let mut cues: BTreeSet<String> = [
            "Player.WeaponSelectionMoveSlot",
            "Player.WeaponSelectionClose",
            "Player.WeaponSelected",
            "Player.DenyWeaponSelection",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let wanted = ["draw", "drawempty", "ir_draw"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        for weapon in hl2_simulation::gameplay::definitions(&vfs)
            .unwrap()
            .values()
        {
            let rig = source_assets::animation::load(&vfs, &weapon.viewmodel, &wanted).unwrap();
            for (name, clip) in &rig.clips {
                for event in &clip.events {
                    if (event.id == 5004 || event.name == "AE_CL_PLAYSOUND")
                        && !event.options.is_empty()
                    {
                        println!(
                            "{} {name}: sound {} at cycle {}",
                            weapon.viewmodel, event.options, event.cycle
                        );
                        cues.insert(event.options.clone());
                    }
                }
            }
        }
        for cue in cues {
            for path in library.all_alternatives(&cue).unwrap() {
                let encoded = vfs.read(&format!("sound/{path}")).unwrap().unwrap();
                let (bytes, summary) = hl2_simulation::sounds::decode(&path, encoded).unwrap();
                let summary = serde_json::to_value(summary).unwrap();
                let expected =
                    summary["frames"].as_u64().unwrap() * summary["channels"].as_u64().unwrap();
                let source = AudioSource {
                    bytes: bytes.into(),
                };
                let decoder = source.decoder();
                let duration = decoder.total_duration().unwrap().as_secs_f64();
                let samples = decoder.count() as u64;
                println!("{cue} {path}: {samples}/{expected} samples, {duration:.6} seconds");
                assert_eq!(samples, expected, "backend truncated {path}");
                assert!(
                    (duration - summary["seconds"].as_f64().unwrap()).abs() < 1e-6,
                    "backend changed duration of {path}"
                );
            }
        }
    }
}
