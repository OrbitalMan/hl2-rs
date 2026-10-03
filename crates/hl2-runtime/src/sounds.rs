//! Local WAV playback and Source symbolic sound-name resolution. DSP/soundscapes are separate work.
use anyhow::{bail, Context, Result};
use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use modkit_core::World;
use source_assets::{keyvalues, vpk::Vfs};
use std::collections::{BTreeMap, HashMap};
pub struct Audio {
    names: BTreeMap<String, Vec<String>>,
    choices: HashMap<String, usize>,
    rng: u64,
    pub variants_played: BTreeMap<String, usize>,
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
                if !path.to_lowercase().ends_with(".wav") {
                    bail!("only PCM WAV audio is supported currently");
                }
                let data = vfs
                    .read(&format!("sound/{path}"))?
                    .context("sound asset absent")?;
                let mut wav = hound::WavReader::new(std::io::Cursor::new(&data))?;
                let spec = wav.spec();
                if !(1..=2).contains(&spec.channels) {
                    bail!("unsupported channel count");
                }
                if spec.sample_format == hound::SampleFormat::Float {
                    for sample in wav.samples::<f32>() {
                        sample?;
                    }
                } else {
                    for sample in wav.samples::<i32>() {
                        sample?;
                    }
                }
                Ok(data)
            })();
            match result {
                Ok(data) => {
                    let sound = load_sound_from_bytes(&data).await?;
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
