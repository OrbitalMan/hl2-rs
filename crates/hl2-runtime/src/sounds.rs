//! Macroquad audio adapter using the shared owned-wave decoder and Source resolver.
use anyhow::{Context, Result};
pub use hl2_simulation::sounds::SoundRequest;
use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use modkit_core::World;
use source_assets::vpk::Vfs;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
pub struct Audio {
    library: hl2_simulation::sounds::Library,
    cache: HashMap<String, Sound>,
}
impl Deref for Audio {
    type Target = hl2_simulation::sounds::Library;
    fn deref(&self) -> &Self::Target {
        &self.library
    }
}
impl DerefMut for Audio {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.library
    }
}
impl Audio {
    pub fn new(vfs: &Vfs) -> Self {
        Self {
            library: hl2_simulation::sounds::Library::new(vfs),
            cache: HashMap::new(),
        }
    }
    pub async fn play(&mut self, vfs: &Vfs, name: &str, looped: bool, volume: f32) -> Result<()> {
        self.play_request(vfs, &name.into(), looped, volume).await
    }

    pub async fn play_request(
        &mut self,
        vfs: &Vfs,
        request: &SoundRequest,
        looped: bool,
        volume: f32,
    ) -> Result<()> {
        let path = match self.resolve(request) {
            Ok(path) => path,
            Err(error) => {
                self.errors
                    .insert(request.name.clone(), format!("{error:#}"));
                return Ok(());
            }
        };
        if self.errors.contains_key(&path) {
            return Ok(());
        }
        if !self.cache.contains_key(&path) {
            let result = (|| -> Result<(Vec<u8>, hl2_simulation::sounds::AudioSummary)> {
                let data = vfs
                    .read(&format!("sound/{path}"))?
                    .context("sound asset absent")?;
                hl2_simulation::sounds::decode(&path, data)
            })();
            match result {
                Ok((data, summary)) => {
                    self.decoded.insert(path.clone(), summary);
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
