//! Preload owned cue alternatives before App::run; systems use only memory/assets.
use anyhow::{Context, Result, bail};
use bevy::{audio::AudioSinkPlayback, prelude::*};
use hl2_simulation::sounds::{Library, SoundRequest};
use source_assets::vpk::Vfs;
use std::collections::{BTreeSet, HashMap};

const MAX_PRELOADED_BYTES: usize = 512 * 1024 * 1024;
const MAX_PLAYERS: usize = 256;
/// rodio 0.22 keeps each player alive with mono 48 kHz silence spans of 512 samples. A
/// sound appended in another format has its first 512 samples resampled as if they were in
/// that filler format (the weapon-selection tick's attack played 2.2x fast and lost ~12 ms).
/// Sounds are therefore converted once at load to this rate; stereo one-shots also start
/// with one filler-length span of silence so the misread span is silent.
const BACKEND_RATE: u32 = 48_000;
const BACKEND_FILLER_SAMPLES: usize = 512;

/// Resample interleaved samples with Catmull-Rom interpolation (each channel separately).
fn resample(samples: &[f32], channels: usize, from: u32, to: u32) -> Vec<f32> {
    let frames = samples.len() / channels;
    if from == to || frames == 0 {
        return samples.to_vec();
    }
    let out_frames = (frames as u64 * u64::from(to)).div_ceil(u64::from(from)) as usize;
    let at = |frame: isize, channel: usize| -> f32 {
        samples[frame.clamp(0, frames as isize - 1) as usize * channels + channel]
    };
    let mut out = Vec::with_capacity(out_frames * channels);
    for i in 0..out_frames {
        let position = i as f64 * f64::from(from) / f64::from(to);
        let (base, t) = (
            position.floor() as isize,
            (position - position.floor()) as f32,
        );
        for channel in 0..channels {
            let [p0, p1, p2, p3] = [-1, 0, 1, 2].map(|d| at(base + d, channel));
            out.push(
                p1 + 0.5
                    * t
                    * (p2 - p0
                        + t * (2. * p0 - 5. * p1 + 4. * p2 - p3 + t * (3. * (p1 - p2) + p3 - p0))),
            );
        }
    }
    out
}

/// 16-bit PCM WAV at the backend rate; see [`BACKEND_RATE`].
fn backend_wav(data: Vec<u8>, looped: bool) -> Result<Vec<u8>> {
    let (channels, rate, samples) = hl2_simulation::sounds::wav_samples(&data)?;
    if rate == BACKEND_RATE && channels == 1 {
        return Ok(data);
    }
    let mut pcm = resample(&samples, usize::from(channels), rate, BACKEND_RATE);
    if channels != 1 && !looped {
        pcm.splice(0..0, std::iter::repeat_n(0., BACKEND_FILLER_SAMPLES));
    }
    let bytes = u32::try_from(pcm.len() * 2).context("converted audio exceeds WAV size")?;
    let mut wav = Vec::with_capacity(44 + pcm.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&BACKEND_RATE.to_le_bytes());
    wav.extend_from_slice(&(BACKEND_RATE * u32::from(channels) * 2).to_le_bytes());
    wav.extend_from_slice(&(channels * 2).to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    for sample in pcm {
        wav.extend_from_slice(&((sample.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes());
    }
    Ok(wav)
}
/// `--mute-ambient`: skip map-start ambient_generic loops so test recordings isolate cues.
pub static MUTE_AMBIENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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
        let looped: BTreeSet<String> = ambient
            .iter()
            .filter(|a| a.looped)
            .filter_map(|a| library.all_alternatives(&a.request.name).ok())
            .flatten()
            .collect();
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
                let data = backend_wav(data, looped.contains(&path))?;
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
/// `--audio-trace PATH`: one JSON line per sound request, sink start and sink removal,
/// with the sink's last observed playback position, for diagnosing cut-off sounds.
#[derive(Resource)]
pub struct AudioTrace {
    file: std::io::BufWriter<std::fs::File>,
    start: std::time::Instant,
    live: HashMap<Entity, (String, f64, f32)>,
}
impl AudioTrace {
    pub fn create(path: &std::path::Path) -> Result<Self> {
        Ok(Self {
            file: std::io::BufWriter::new(std::fs::File::create(path)?),
            start: std::time::Instant::now(),
            live: HashMap::new(),
        })
    }
    fn write(&mut self, mut line: serde_json::Value) {
        use std::io::Write;
        line["t"] = serde_json::json!(self.start.elapsed().as_secs_f64());
        // Tracing must never stop the game; a failed write only loses diagnostics.
        let _ = writeln!(self.file, "{line}").and_then(|()| self.file.flush());
    }
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
    let mute = MUTE_AMBIENT.load(std::sync::atomic::Ordering::Relaxed);
    for ambient in prepared.ambient.into_iter().filter(|_| !mute) {
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
    (simulation, mut trace): (Res<crate::movement::Simulation>, Option<ResMut<AudioTrace>>),
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
    for request in &game.sound_requests {
        if let Some(trace) = trace.as_deref_mut() {
            trace.write(serde_json::json!({"event":"request","cue":request.name,"frame":audio.frame,"scene_time":game.scene.time,"paused":paused}));
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
    (mut trace, mut removed): (Option<ResMut<AudioTrace>>, RemovedComponents<SoundPlayer>),
) {
    if let Some(trace) = trace.as_deref_mut() {
        for entity in removed.read() {
            if let Some((path, seconds, position)) = trace.live.remove(&entity) {
                trace.write(serde_json::json!({"event":"removed","path":path,"frame":audio.frame,
                    "last_position":position,"duration":seconds,"remaining":seconds - f64::from(position)}));
            }
        }
        for (entity, player, sink) in &players {
            let Some(sink) = sink else { continue };
            let position = sink.position().as_secs_f32();
            if let Some(live) = trace.live.get_mut(&entity) {
                live.2 = position;
            } else {
                let seconds = audio
                    .library
                    .decoded
                    .get(&player.path)
                    .map_or(0., |summary| summary.seconds());
                trace
                    .live
                    .insert(entity, (player.path.clone(), seconds, position));
                trace.write(serde_json::json!({"event":"started","path":player.path,"frame":audio.frame,
                    "queued_frame":player.queued_frame,"position":position,"duration":seconds,"volume":sink.volume().to_linear(),"paused":sink.is_paused()}));
            }
        }
    }
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
    fn backend_resampling_preserves_tone_and_pads_stereo_one_shots() {
        // A 1.8 kHz tone (the selection tick's bursts) at 22050 Hz keeps its level and pitch.
        let tone: Vec<f32> = (0..22050)
            .map(|i| (i as f32 * std::f32::consts::TAU * 1800. / 22050.).sin() * 0.5)
            .collect();
        let out = resample(&tone, 1, 22050, BACKEND_RATE);
        assert_eq!(out.len(), 48000);
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        assert!((rms(&out) / rms(&tone) - 1.).abs() < 0.01);
        let crossings = out.windows(2).filter(|w| w[0] <= 0. && w[1] > 0.).count();
        assert!((1799..=1801).contains(&crossings));
        assert_eq!(resample(&tone, 1, 22050, 22050), tone);
        // Stereo one-shots start with one filler span of silence; loops and mono do not.
        let wav = |channels: u16, frames: usize| {
            let data = [0x00u8, 0x40].repeat(frames * usize::from(channels));
            let mut wav = b"RIFF".to_vec();
            wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt     ");
            wav.extend_from_slice(&channels.to_le_bytes());
            wav.extend_from_slice(&44100u32.to_le_bytes());
            wav.extend_from_slice(&(44100 * 2 * u32::from(channels)).to_le_bytes());
            wav.extend_from_slice(&(2 * channels).to_le_bytes());
            wav.extend_from_slice(b" data");
            wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
            wav.extend_from_slice(&data);
            wav
        };
        let samples = |bytes: Vec<u8>| hl2_simulation::sounds::wav_samples(&bytes).unwrap();
        let (channels, rate, one_shot) = samples(backend_wav(wav(2, 441), false).unwrap());
        assert_eq!((channels, rate), (2, BACKEND_RATE));
        assert_eq!(one_shot.len(), BACKEND_FILLER_SAMPLES + 2 * 480);
        assert!(one_shot[..BACKEND_FILLER_SAMPLES].iter().all(|&v| v == 0.));
        assert!((one_shot[BACKEND_FILLER_SAMPLES] - 0.5).abs() < 1e-3);
        assert_eq!(
            samples(backend_wav(wav(2, 441), true).unwrap()).2.len(),
            2 * 480
        );
        assert_eq!(
            samples(backend_wav(wav(1, 441), false).unwrap()).2.len(),
            480
        );
    }
    /// Plain 8-bit unsigned or 16-bit signed PCM samples of a RIFF WAVE, as f32.
    fn pcm_samples(bytes: &[u8]) -> Option<Vec<f32>> {
        let (mut offset, mut bits) = (12, None);
        while offset + 8 <= bytes.len() {
            let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
            let body = bytes.get(offset + 8..offset + 8 + size)?;
            match &bytes[offset..offset + 4] {
                b"fmt " if u16::from_le_bytes([body[0], body[1]]) == 1 => {
                    bits = Some(u16::from_le_bytes([body[14], body[15]]))
                }
                b"data" => {
                    return match bits? {
                        8 => Some(body.iter().map(|&b| (f32::from(b) - 128.) / 128.).collect()),
                        16 => Some(
                            body.as_chunks::<2>()
                                .0
                                .iter()
                                .map(|c| f32::from(i16::from_le_bytes(*c)) / 32768.)
                                .collect(),
                        ),
                        _ => None,
                    };
                }
                _ => {}
            }
            offset += 8 + size + (size & 1);
        }
        None
    }
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
                let decoded: Vec<f32> = source.decoder().collect();
                let samples = decoded.len() as u64;
                // The backend's samples must equal the file's, from the very first one.
                let expected_samples =
                    pcm_samples(&source.bytes).unwrap_or_else(|| decoded.clone());
                let first = decoded
                    .iter()
                    .zip(&expected_samples)
                    .position(|(a, b)| (a - b).abs() > 1. / 64.);
                println!(
                    "{cue} {path}: first differing sample {first:?}; first 8 backend {:?} file {:?}",
                    &decoded[..8],
                    &expected_samples[..8]
                );
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
