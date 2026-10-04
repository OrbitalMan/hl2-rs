//! Owned WAV/MP3 playback and Source symbolic names. DSP/soundscapes remain separate work.
use anyhow::{bail, Context, Result};
use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use modkit_core::World;
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, ErrorKind};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as DecodeError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

const MAX_ENCODED_AUDIO: usize = 32 * 1024 * 1024;
const MAX_PCM_AUDIO: usize = 128 * 1024 * 1024;

#[derive(serde::Serialize)]
pub struct AudioSummary {
    codec: String,
    channels: u16,
    sample_rate: u32,
    frames: u32,
    seconds: f64,
}

fn validate_wav(data: &[u8]) -> Result<()> {
    let mut wav = hound::WavReader::new(Cursor::new(data))?;
    let spec = wav.spec();
    if !(1..=2).contains(&spec.channels) || spec.sample_rate == 0 {
        bail!("unsupported audio channel count or sample rate");
    }
    if spec.sample_format == hound::SampleFormat::Float {
        for sample in wav.samples::<f32>() {
            if !sample?.is_finite() {
                bail!("nonfinite audio sample");
            }
        }
    } else {
        for sample in wav.samples::<i32>() {
            sample?;
        }
    }
    Ok(())
}

// The playback backend accepts WAV bytes. Decode MP3 in Rust into an in-memory
// PCM WAV, retaining channel order/rate. No converted game files are written.
fn mp3_to_wav(data: Vec<u8>) -> Result<Vec<u8>> {
    let stream = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let options = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let mut format = symphonia::default::get_probe()
        .format(&hint, stream, &options, &MetadataOptions::default())?
        .format;
    let track = format.default_track().context("MP3 has no audio track")?;
    let track_id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut spec = None;
    let mut pcm = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet)?;
        let current = *decoded.spec();
        let channels = current.channels.count();
        if !(1..=2).contains(&channels) || !(8000..=192000).contains(&current.rate) {
            bail!("unsupported MP3 channel count or sample rate");
        }
        if spec.is_some_and(|previous| previous != current) {
            bail!("MP3 changes channel layout or sample rate");
        }
        spec = Some(current);
        let count = decoded
            .frames()
            .checked_mul(channels)
            .context("audio size overflow")?;
        let total = pcm
            .len()
            .checked_add(count)
            .context("audio size overflow")?;
        if total > MAX_PCM_AUDIO / 2 {
            bail!("decoded audio exceeds 128 MiB limit");
        }
        let mut samples = SampleBuffer::<i16>::new(decoded.capacity() as u64, current);
        samples.copy_interleaved_ref(decoded);
        pcm.extend_from_slice(samples.samples());
    }
    if pcm.is_empty() {
        bail!("MP3 has no decoded audio samples");
    }
    let spec = spec.context("MP3 has no decoded audio")?;
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut cursor,
        hound::WavSpec {
            channels: spec.channels.count() as u16,
            sample_rate: spec.rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    for sample in pcm {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(cursor.into_inner())
}

fn playback_bytes(path: &str, data: Vec<u8>) -> Result<Vec<u8>> {
    if data.len() > MAX_ENCODED_AUDIO {
        bail!("encoded audio exceeds 32 MiB limit");
    }
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "wav" => {
            validate_wav(&data)?;
            Ok(data)
        }
        "mp3" => mp3_to_wav(data),
        _ => bail!("only WAV and MP3 audio are supported currently"),
    }
}

pub struct Audio {
    names: BTreeMap<String, Vec<String>>,
    choices: HashMap<String, usize>,
    rng: u64,
    pub variants_played: BTreeMap<String, usize>,
    pub decoded: BTreeMap<String, AudioSummary>,
    cache: HashMap<String, Sound>,
    pub errors: BTreeMap<String, String>,
    pub played: usize,
}
impl Audio {
    pub fn new(vfs: &Vfs) -> Self {
        let mut names = BTreeMap::new();
        let result = (|| -> Result<()> {
            let manifest = vfs
                .read("scripts/game_sounds_manifest.txt")?
                .context("sound manifest absent")?;
            let tokens = keyvalues::tokens(&String::from_utf8_lossy(&manifest))?;
            for pair in tokens
                .windows(2)
                .filter(|p| p[0].eq_ignore_ascii_case("precache_file"))
            {
                let Some(data) = vfs.read(&pair[1])? else {
                    continue;
                };
                for entry in keyvalues::parse(&String::from_utf8_lossy(&data))? {
                    let mut waves = Vec::new();
                    for child in entry.children() {
                        if child.key.eq_ignore_ascii_case("wave") {
                            if let Some(wave) = child.text() {
                                waves.push(wave.to_string());
                            }
                        }
                        if child.key.eq_ignore_ascii_case("rndwave") {
                            for wave in child
                                .children()
                                .iter()
                                .filter(|e| e.key.eq_ignore_ascii_case("wave"))
                            {
                                if let Some(wave) = wave.text() {
                                    waves.push(wave.to_string());
                                }
                            }
                        }
                    }
                    if !waves.is_empty() {
                        names.insert(entry.key.to_lowercase(), waves);
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Sound manifest: {e:#}");
        }
        Self {
            names,
            choices: HashMap::new(),
            rng: 0x92ea79123,
            variants_played: BTreeMap::new(),
            decoded: BTreeMap::new(),
            cache: HashMap::new(),
            errors: BTreeMap::new(),
            played: 0,
        }
    }
    pub async fn play(&mut self, vfs: &Vfs, name: &str, looped: bool, volume: f32) -> Result<()> {
        let key = name.to_lowercase();
        let resolved = if let Some(waves) = self.names.get(&key) {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            let mut choice = self.rng as usize % waves.len();
            if waves.len() > 1 && self.choices.get(&key) == Some(&choice) {
                choice = (choice + 1) % waves.len();
            }
            self.choices.insert(key, choice);
            waves[choice].clone()
        } else {
            name.to_string()
        };
        let path = resolved
            .trim_start_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .trim_start_matches("sound/")
            .replace('\\', "/");
        if self.errors.contains_key(&path) {
            return Ok(());
        }
        if !self.cache.contains_key(&path) {
            let result = (|| -> Result<Vec<u8>> {
                let data = vfs
                    .read(&format!("sound/{path}"))?
                    .context("sound asset absent")?;
                playback_bytes(&path, data)
            })();
            match result {
                Ok(data) => {
                    let wav = hound::WavReader::new(Cursor::new(&data))?;
                    let spec = wav.spec();
                    self.decoded.insert(
                        path.clone(),
                        AudioSummary {
                            codec: path.rsplit('.').next().unwrap_or("").to_ascii_lowercase(),
                            channels: spec.channels,
                            sample_rate: spec.sample_rate,
                            frames: wav.duration(),
                            seconds: f64::from(wav.duration()) / f64::from(spec.sample_rate),
                        },
                    );
                    let sound = match load_sound_from_bytes(&data).await {
                        Ok(sound) => sound,
                        Err(error) => {
                            self.errors
                                .insert(path, format!("playback backend: {error}"));
                            return Ok(());
                        }
                    };
                    self.cache.insert(path.clone(), sound);
                }
                Err(e) => {
                    self.errors.insert(path, format!("{e:#}"));
                    return Ok(());
                }
            }
        }
        play_sound(
            &self.cache[&path],
            PlaySoundParams {
                looped,
                volume: volume.clamp(0., 1.),
            },
        );
        *self.variants_played.entry(path).or_default() += 1;
        self.played += 1;
        Ok(())
    }
    pub async fn ambient(&mut self, vfs: &Vfs, world: &World) -> Result<()> {
        for e in world
            .entities
            .iter()
            .filter(|e| e.class() == "ambient_generic")
        {
            let flags = e
                .get("spawnflags")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            if flags & 16 == 0 {
                if let Some(sound) = e.get("message") {
                    let volume = e
                        .get("health")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(5.)
                        / 10.;
                    self.play(vfs, sound, flags & 32 == 0, volume * 0.3).await?;
                }
            }
        }
        Ok(())
    }
    pub fn stop(&self) {
        for sound in self.cache.values() {
            stop_sound(sound);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_pcm_keeps_its_rate_channels_and_samples() {
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 2,
                sample_rate: 22050,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for sample in [i16::MIN, i16::MAX, -123, 456] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        let original = cursor.into_inner();
        assert_eq!(
            playback_bytes("synthetic.WAV", original.clone()).unwrap(),
            original
        );
    }

    #[test]
    fn invalid_compressed_audio_is_rejected_without_a_substitute() {
        assert!(playback_bytes("invalid.mp3", vec![0; 128]).is_err());
        assert!(playback_bytes("empty.mp3", vec![]).is_err());
        assert!(playback_bytes("unsupported.ogg", vec![]).is_err());
        assert!(playback_bytes("truncated.wav", b"RIFF".to_vec()).is_err());
    }

    #[test]
    fn nonfinite_pcm_and_surround_audio_are_rejected() {
        for (channels, sample) in [(1, f32::NAN), (3, 0.)] {
            let mut cursor = Cursor::new(Vec::new());
            let mut writer = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels,
                    sample_rate: 48000,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .unwrap();
            for _ in 0..channels {
                writer.write_sample(sample).unwrap();
            }
            writer.finalize().unwrap();
            assert!(playback_bytes("invalid.wav", cursor.into_inner()).is_err());
        }
    }

    #[test]
    #[ignore = "requires an owned installed Half-Life 2 copy"]
    fn installed_trainstation_music_decodes_to_nonempty_pcm() {
        let root = source_assets::install::discover().unwrap();
        let vfs = Vfs::mount(&root).unwrap();
        let path = "music/HL2_song26_trainstation1.mp3";
        let original = vfs.read(&format!("sound/{path}")).unwrap().unwrap();
        let converted = playback_bytes(path, original).unwrap();
        validate_wav(&converted).unwrap();
        let mut wav = hound::WavReader::new(Cursor::new(converted)).unwrap();
        let spec = wav.spec();
        let samples = wav
            .samples::<i16>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(samples.len() > spec.sample_rate as usize * spec.channels as usize);
        assert!(samples.iter().any(|sample| sample.unsigned_abs() > 100));
        println!(
            "installed music: {spec:?}, {} interleaved samples",
            samples.len()
        );
    }
}
