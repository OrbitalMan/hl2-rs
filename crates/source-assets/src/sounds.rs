//! Source actor-model gender registry and sound-script wave metadata.
use crate::keyvalues;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

const MAX_ACTORS: usize = 255;
const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
const MAX_NAME_BYTES: usize = 255;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ActorGender {
    #[default]
    None,
    Male,
    Female,
}

#[derive(Default)]
pub struct ActorRegistry {
    actors: BTreeMap<String, ActorGender>,
}

impl ActorRegistry {
    /// `scripts/global_actors.txt` maps model basenames to genders. Duplicate
    /// keys keep their first value; unrecognized values mean GENDER_NONE.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() > MAX_REGISTRY_BYTES {
            bail!("actor registry exceeds 1 MiB reader limit");
        }
        let entries = keyvalues::parse(&keyvalues::decode_text(data)?)?;
        let root = entries.first().context("empty actor registry")?;
        if !matches!(root.value, keyvalues::Value::Block(_)) {
            bail!("actor registry root must be a block");
        }
        let mut actors = BTreeMap::new();
        for entry in root.children() {
            if entry.key.len() > MAX_NAME_BYTES || entry.key.contains('\0') {
                bail!("actor registry name exceeds supported bounds");
            }
            let key = entry.key.to_ascii_lowercase();
            if actors.contains_key(&key) {
                continue;
            }
            if actors.len() == MAX_ACTORS {
                bail!("actor registry exceeds 255 unique actors");
            }
            let gender = match entry.text() {
                Some(value) if value.eq_ignore_ascii_case("male") => ActorGender::Male,
                Some(value) if value.eq_ignore_ascii_case("female") => ActorGender::Female,
                _ => ActorGender::None,
            };
            actors.insert(key, gender);
        }
        Ok(Self { actors })
    }

    pub fn len(&self) -> usize {
        self.actors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// Directory, extension and ASCII case do not identify gender. Only the
    /// registered basename does. Unknown/missing models remain GENDER_NONE.
    pub fn gender(&self, model: Option<&str>) -> Result<ActorGender> {
        let Some(model) = model else {
            return Ok(ActorGender::None);
        };
        if model.len() > 4096 || model.contains('\0') {
            bail!("actor model path exceeds supported bounds");
        }
        let leaf = model.rsplit(['/', '\\']).next().unwrap_or("");
        let stem = leaf.rsplit_once('.').map_or(leaf, |(stem, _)| stem);
        // Native uses a 256-byte basename buffer. Reject oversized names rather
        // than aliasing an unrelated actor through silent truncation.
        if stem.len() > MAX_NAME_BYTES {
            bail!("actor model basename exceeds 255 bytes");
        }
        Ok(self
            .actors
            .get(&stem.to_ascii_lowercase())
            .copied()
            .unwrap_or_default())
    }
}

fn gender_token(path: &str) -> Option<usize> {
    path.as_bytes()
        .windows(b"$gender".len())
        .position(|bytes| bytes.eq_ignore_ascii_case(b"$gender"))
}

/// Explicit GenderExpandString semantics. This is separate from raw EmitSound:
/// raw WAV/MP3 names bypass script selection and are not implicitly expanded.
/// Only the first case-insensitive token is replaced. NONE leaves it intact.
pub fn expand_literal(path: &str, gender: ActorGender) -> Result<String> {
    if path.len() > MAX_NAME_BYTES || path.contains('\0') {
        bail!("sound wave name exceeds supported bounds");
    }
    let replacement = match gender {
        ActorGender::None => return Ok(path.to_owned()),
        ActorGender::Male => "male",
        ActorGender::Female => "female",
    };
    let Some(at) = gender_token(path) else {
        return Ok(path.to_owned());
    };
    Ok(format!("{}{replacement}{}", &path[..at], &path[at + 7..]))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wave {
    pub path: String,
    pub gender: ActorGender,
}

impl Wave {
    /// Script registration expands a template into two tagged alternatives.
    /// Plain waves retain GENDER_NONE; that is not a universal gender wildcard.
    pub fn register(path: &str) -> Result<Vec<Self>> {
        let genders: &[ActorGender] = if gender_token(path).is_some() {
            &[ActorGender::Male, ActorGender::Female]
        } else {
            &[ActorGender::None]
        };
        genders
            .iter()
            .map(|&gender| {
                Ok(Self {
                    path: expand_literal(path, gender)?,
                    gender,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_uses_registered_basename_and_first_duplicate() -> Result<()> {
        let registry = ActorRegistry::parse(
            br#"globalactors { "lead" "Female" "LEAD" "male" "male_guess" "none" "robot" "mechanical" }"#,
        )?;
        assert_eq!(registry.len(), 3);
        assert_eq!(
            registry.gender(Some(r"Models\cast\LEAD.MDL"))?,
            ActorGender::Female
        );
        assert_eq!(registry.gender(Some("other/lead"))?, ActorGender::Female);
        for model in [
            None,
            Some(""),
            Some("models/male_guess.mdl"),
            Some("robot.mdl"),
            Some("models/female_99.mdl"),
        ] {
            assert_eq!(registry.gender(model)?, ActorGender::None);
        }
        Ok(())
    }

    #[test]
    fn literal_and_registered_expansion_are_distinct() -> Result<()> {
        let path = "^vo/$GeNdEr01/$gender.wav";
        assert_eq!(expand_literal(path, ActorGender::None)?, path);
        assert_eq!(
            expand_literal(path, ActorGender::Female)?,
            "^vo/female01/$gender.wav"
        );
        let waves = Wave::register(path)?;
        assert_eq!(
            waves[0],
            Wave {
                path: "^vo/male01/$gender.wav".into(),
                gender: ActorGender::Male
            }
        );
        assert_eq!(waves[1].gender, ActorGender::Female);
        assert_eq!(
            Wave::register("vo/common.wav")?[0].gender,
            ActorGender::None
        );
        Ok(())
    }

    #[test]
    fn registry_and_wave_reader_budgets_are_explicit() {
        let mut registry = String::from("actors {");
        for id in 0..256 {
            registry.push_str(&format!("\"actor{id}\" \"male\" "));
        }
        registry.push('}');
        assert!(ActorRegistry::parse(registry.as_bytes()).is_err());
        assert!(ActorRegistry::parse(&vec![b' '; MAX_REGISTRY_BYTES + 1]).is_err());
        assert!(Wave::register(&"x".repeat(256)).is_err());
        assert!(Wave::register("bad\0.wav").is_err());
        assert!(ActorRegistry::default()
            .gender(Some(&"x".repeat(256)))
            .is_err());
    }
}
