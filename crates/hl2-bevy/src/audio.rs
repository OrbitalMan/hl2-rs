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
        for path in paths {
            let decoded = (|| -> Result<_> {
                let encoded = vfs
                    .read(&format!("sound/{path}"))?
                    .context("owned sound absent")?;
                let (data, summary) = hl2_simulation::sounds::decode(&path, encoded)?;
                if data.len() > MAX_PRELOADED_BYTES.saturating_sub(bytes) {
                    bail!("owned audio preload exceeds 512 MiB budget");
                }
                Ok((data, summary))
            })();
            match decoded {
                Ok((data, summary)) => {
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
    ) -> bool {
        let path = match self.library.resolve(request) {
            Ok(path) => path,
            Err(error) => {
                self.library
                    .errors
                    .insert(request.name.clone(), format!("{error:#}"));
                self.failed += 1;
                return false;
            }
        };
        let Some(handle) = self.handles.get(&path) else {
            self.library.errors.entry(path).or_insert_with(|| {
                "cue variant was not preloaded; no file reads are allowed in playback systems"
                    .into()
            });
            self.failed += 1;
            return false;
        };
        let settings = if looped {
            PlaybackSettings::LOOP
        } else {
            PlaybackSettings::DESPAWN
        };
        commands.spawn((
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
        true
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
        } else if audio.emit(&mut commands, &request, false, 0.4, paused) {
            count += 1;
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
