//! Lip sync: speech phonemes drive mouth flex controllers while an actor's line plays.
//! Follows client C_BaseFlex::ProcessVisemes / AddVisemesForSentence / AddViseme /
//! ComputeBlendedSetting (pinned SDK, statically compared with retail client.dll).
use source_assets::sentence::{FlexSettings, Sentence};
use source_assets::vpk::Vfs;
use std::collections::BTreeMap;
use std::sync::Arc;

/// `phonemefilter` default: box filter duration in seconds.
const PHONEME_FILTER: f32 = 0.08;
/// `phonemedelay` default.
const PHONEME_DELAY: f32 = 0.;
const STRONG_CROSSFADE_START: f32 = 0.6;
const WEAK_CROSSFADE_START: f32 = 0.4;

struct Voice {
    actor: usize,
    sentence: Arc<Sentence>,
    length: f32,
    start: f64,
}
#[derive(Default)]
pub struct LipSync {
    /// Weak, normal and strong phoneme classes (expressions/phonemes*.vfe).
    classes: [Option<Arc<FlexSettings>>; 3],
    /// Sentence data and sound length per preloaded wave path.
    sentences: BTreeMap<String, (Arc<Sentence>, f32)>,
    voices: Vec<Voice>,
}
impl LipSync {
    /// Setup-time load of the phoneme flex settings (no frame IO).
    pub fn load_classes(&mut self, vfs: &Vfs) {
        for (slot, name) in ["phonemes_weak", "phonemes", "phonemes_strong"]
            .into_iter()
            .enumerate()
        {
            self.classes[slot] = vfs
                .read(&format!("expressions/{name}.vfe"))
                .ok()
                .flatten()
                .and_then(|data| FlexSettings::parse(&data).ok())
                .map(Arc::new);
        }
    }
    /// Register a preloaded wave's sentence and duration (seconds).
    pub fn add_sentence(&mut self, wave: &str, sentence: Sentence, length: f32) {
        if !sentence.phonemes.is_empty() && length.is_finite() && length > 0. {
            self.sentences
                .insert(wave.to_lowercase(), (Arc::new(sentence), length));
        }
    }
    pub fn has_sentence(&self, wave: &str) -> bool {
        self.sentences.contains_key(&wave.to_lowercase())
    }
    /// A host started playing `wave` for `actor` at scene time `time`.
    pub fn start(&mut self, actor: usize, wave: &str, time: f64) {
        if let Some((sentence, length)) = self.sentences.get(&wave.to_lowercase()) {
            self.voices.push(Voice {
                actor,
                sentence: sentence.clone(),
                length: *length,
                start: time,
            });
        }
    }
    /// Drop finished voices (ProcessVisemes ignores them after length + 2 seconds).
    pub fn cleanup(&mut self, time: f64) {
        self.voices
            .retain(|v| ((time - v.start) as f32) < v.length + 2.);
    }
    pub fn speaking(&self, actor: usize) -> bool {
        self.voices.iter().any(|v| v.actor == actor)
    }
    /// Controller additions (lowercase names) from every active voice of `actor`.
    pub fn visemes(&self, actor: usize, time: f64) -> BTreeMap<String, f32> {
        let mut out = BTreeMap::new();
        let Some(normal) = &self.classes[1] else {
            return out;
        };
        for voice in self.voices.iter().filter(|v| v.actor == actor) {
            let elapsed = (time - voice.start) as f32;
            if elapsed >= voice.length + 2. {
                continue;
            }
            let t = elapsed - PHONEME_DELAY;
            let emphasis = voice.sentence.intensity(t, voice.length);
            let phonemes = &voice.sentence.phonemes;
            for (k, phoneme) in phonemes.iter().enumerate() {
                let mut dt = PHONEME_FILTER;
                // LOD0 models crossfade over the current phoneme's neighborhood.
                if t > phoneme.start && t < phoneme.end && k + 1 < phonemes.len() {
                    let next = &phonemes[k + 1];
                    let duration = phoneme.end - phoneme.start;
                    let reach = if next.start == phoneme.end {
                        next.end
                    } else {
                        next.start
                    } - t;
                    dt = dt.max(reach.min(duration));
                }
                let t1 = (phoneme.start - t) / dt;
                let t2 = (phoneme.end - t) / dt;
                if t1 < 1. && t2 > 0. {
                    let scale = t2.min(1.) - t1.max(0.);
                    self.add_viseme(&mut out, normal, emphasis, phoneme.code, scale);
                }
            }
        }
        out
    }
    /// AddViseme with ComputeBlendedSetting's weak/normal/strong amounts.
    fn add_viseme(
        &self,
        out: &mut BTreeMap<String, f32>,
        normal: &FlexSettings,
        emphasis: f32,
        code: u16,
        scale: f32,
    ) {
        // The normal class is required; missing phonemes are skipped.
        let Some(normal_setting) = normal.indexed(code) else {
            return;
        };
        let weak = self.classes[0].as_ref().and_then(|c| c.indexed(code));
        let strong = self.classes[2].as_ref().and_then(|c| c.indexed(code));
        // (weak, normal, strong) amounts.
        let (a_weak, a_normal, a_strong) = if emphasis > STRONG_CROSSFADE_START {
            if strong.is_some() {
                let frac = (1. - emphasis) / (1. - STRONG_CROSSFADE_START);
                (0., frac * 2. * STRONG_CROSSFADE_START, 1. - frac)
            } else {
                (0., 2. * STRONG_CROSSFADE_START, 0.)
            }
        } else if emphasis < WEAK_CROSSFADE_START {
            if weak.is_some() {
                let frac = (WEAK_CROSSFADE_START - emphasis) / WEAK_CROSSFADE_START;
                (frac, (1. - frac) * 2. * WEAK_CROSSFADE_START, 0.)
            } else {
                (0., 2. * WEAK_CROSSFADE_START, 0.)
            }
        } else {
            (0., 2. * emphasis, 0.)
        };
        for (setting, amount) in [
            (weak, a_weak),
            (Some(normal_setting), a_normal),
            (strong, a_strong),
        ] {
            let Some(setting) = setting.filter(|_| amount != 0.) else {
                continue;
            };
            for (name, weight) in setting {
                *out.entry(name.clone()).or_insert(0.) += amount * scale * weight;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use source_assets::sentence::Phoneme;
    fn settings(code: u16, weight: f32) -> FlexSettings {
        // Minimal VFE: one key "jaw_drop", one setting, index table up to `code`.
        let mut d = vec![0u8; 112];
        d[0..4].copy_from_slice(&((86i32 << 16) + (70 << 8) + 69).to_le_bytes());
        let set =
            |d: &mut Vec<u8>, at: usize, v: i32| d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let settings_at = d.len();
        d.extend([0u8; 24]);
        let weights_at = d.len();
        d.extend(0i32.to_le_bytes());
        d.extend(weight.to_le_bytes());
        d.extend(0f32.to_le_bytes());
        let index_at = d.len();
        for c in 0..=code {
            d.extend((if c == code { 0i32 } else { -1 }).to_le_bytes());
        }
        let key_table = d.len();
        d.extend(0i32.to_le_bytes());
        let key_name = d.len() as i32;
        d.extend(b"jaw_drop\0");
        set(&mut d, key_table, key_name);
        set(&mut d, 76, 1);
        set(&mut d, 80, settings_at as i32);
        set(&mut d, 88, i32::from(code) + 1);
        set(&mut d, 92, index_at as i32);
        set(&mut d, 96, 1);
        set(&mut d, 100, key_table as i32);
        set(&mut d, settings_at + 8, 1);
        set(&mut d, settings_at + 20, (weights_at - settings_at) as i32);
        FlexSettings::parse(&d).unwrap()
    }
    #[test]
    fn visemes_box_filter_phonemes_while_the_line_plays() {
        let mut lips = LipSync::default();
        lips.classes[1] = Some(Arc::new(settings(593, 0.5)));
        let sentence = Sentence {
            phonemes: vec![Phoneme {
                code: 593,
                start: 0.2,
                end: 0.6,
            }],
            emphasis: Vec::new(),
        };
        lips.add_sentence("vo/test.wav", sentence, 1.);
        assert!(lips.has_sentence("VO/Test.wav"));
        lips.start(7, "vo/test.wav", 10.);
        // Before the phoneme: nothing. Inside: neutral emphasis (0.5) gives amount 1.
        assert!(lips.visemes(7, 10.05).is_empty());
        let mid = lips.visemes(7, 10.4)["jaw_drop"];
        assert!((mid - 0.5).abs() < 1e-5, "{mid}");
        // Entering the phoneme, the box filter ramps in.
        let edge = lips.visemes(7, 10.16)["jaw_drop"];
        assert!(edge > 0. && edge < mid);
        assert!(lips.visemes(3, 10.4).is_empty());
        lips.cleanup(12.9);
        assert!(lips.speaking(7));
        lips.cleanup(13.1);
        assert!(!lips.speaking(7));
    }
    #[test]
    #[ignore = "requires owned HL2 installation"]
    fn owned_security_speech_has_phonemes_that_move_the_mouth() {
        let game = source_assets::install::discover().unwrap();
        let data = std::fs::read(game.join("hl2/maps/d1_trainstation_01.bsp")).unwrap();
        let world = source_assets::bsp::Bsp::parse(&data)
            .unwrap()
            .world("d1_trainstation_01")
            .unwrap();
        let vfs = Vfs::mount(&game).unwrap();
        let mut scene = crate::entities::Scene::new(&world);
        scene.load_choreography(&world, &vfs).unwrap();
        let library = crate::sounds::Library::new(&vfs);
        let mut waves = std::collections::BTreeSet::new();
        for request in scene.required_sound_requests(&world) {
            if request.name.to_lowercase().contains("ba_") || request.name.contains("Barney") {
                waves.extend(library.alternatives(&request).unwrap_or_default());
            }
        }
        assert!(!waves.is_empty(), "no Barney speech in the map's scenes");
        let mut with_phonemes = 0;
        for wave in &waves {
            let data = vfs.read(&format!("sound/{wave}")).unwrap().unwrap();
            let Some(chunk) = source_assets::sentence::vdat_chunk(&data).unwrap() else {
                continue;
            };
            let sentence = Sentence::parse(chunk).unwrap();
            let Some(last) = sentence.phonemes.last().copied() else {
                continue;
            };
            with_phonemes += 1;
            let lips = &mut scene.lipsync;
            lips.add_sentence(wave, sentence.clone(), last.end + 0.1);
            lips.start(1, wave, 0.);
            let mid = sentence.phonemes[sentence.phonemes.len() / 2];
            let values = lips.visemes(1, f64::from((mid.start + mid.end) / 2.));
            assert!(
                values.values().any(|v| *v > 0.05),
                "{wave}: no viseme at {mid:?}: {values:?}"
            );
            lips.cleanup(1e6);
        }
        eprintln!(
            "LIPSYNC {with_phonemes}/{} Barney waves carry phonemes",
            waves.len()
        );
        assert!(with_phonemes * 2 >= waves.len());
    }
}
