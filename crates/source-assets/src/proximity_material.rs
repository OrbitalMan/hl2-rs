//! Bounded SDK PlayerProximity/Subtract/Clamp frame proxies for brush materials.
use crate::keyvalues::Entry;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
enum Op {
    Distance {
        result: String,
        scale: f32,
    },
    Subtract {
        result: String,
        a: String,
        b: String,
    },
    Clamp {
        result: String,
        source: String,
        min: f32,
        max: f32,
    },
}
#[derive(Clone, Debug, Default)]
pub struct Frames {
    values: BTreeMap<String, f32>,
    ops: Vec<Op>,
}
impl Frames {
    pub fn parse(entries: &[Entry], properties: &BTreeMap<String, String>) -> Result<Self> {
        if entries.len() > 128 {
            bail!("material proxy count exceeds 128");
        }
        let mut out = Self::default();
        for (key, value) in properties {
            if let Ok(value) = value.parse::<f32>() {
                if !value.is_finite() {
                    bail!("nonfinite material variable {key}");
                }
                out.values.insert(key.to_ascii_lowercase(), value);
            }
        }
        let text = |e: &Entry, name: &str| -> Result<String> {
            Ok(e.get(name)
                .and_then(Entry::text)
                .context("proxy variable absent")?
                .to_ascii_lowercase())
        };
        let number = |e: &Entry, name: &str, default: f32| -> Result<f32> {
            let value = e
                .get(name)
                .and_then(Entry::text)
                .map_or(Ok(default), str::parse::<f32>)?;
            if !value.is_finite() {
                bail!("nonfinite proxy {name}");
            }
            Ok(value)
        };
        for e in entries {
            let op = match e.key.to_ascii_lowercase().as_str() {
                "playerproximity" => Op::Distance {
                    result: text(e, "resultVar")?,
                    scale: number(e, "scale", 0.002)?,
                },
                "subtract" => Op::Subtract {
                    result: text(e, "resultVar")?,
                    a: text(e, "srcVar1")?,
                    b: text(e, "srcVar2")?,
                },
                "clamp" => {
                    let min = number(e, "min", 0.)?;
                    let max = number(e, "max", 1.)?;
                    if min > max {
                        bail!("proxy clamp range reversed");
                    }
                    Op::Clamp {
                        result: text(e, "resultVar")?,
                        source: text(e, "srcVar1")?,
                        min,
                        max,
                    }
                }
                _ => continue, // Other outputs do not participate in the frame expression.
            };
            out.ops.push(op);
        }
        Ok(out)
    }
    pub fn active(&self) -> bool {
        !self.ops.is_empty()
    }
    /// SDK proximity is between the bound entity and local player's world-space centers.
    /// Texture frame vars are integers; conversion truncates before texture frame wrap.
    pub fn sample(&self, distance: f32, counts: [usize; 2]) -> [usize; 2] {
        let mut vars = self.values.clone();
        let get = |vars: &BTreeMap<String, f32>, key: &str| vars.get(key).copied().unwrap_or(0.);
        for op in &self.ops {
            let (result, value) = match op {
                Op::Distance { result, scale } => (result, distance.max(0.) * scale),
                Op::Subtract { result, a, b } => (result, get(&vars, a) - get(&vars, b)),
                Op::Clamp {
                    result,
                    source,
                    min,
                    max,
                } => (result, get(&vars, source).clamp(*min, *max)),
            };
            vars.insert(result.clone(), value);
        }
        let keys = ["$frame", "$frame2"];
        std::array::from_fn(|i| (get(&vars, keys[i]).max(0.) as usize) % counts[i].max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_distance_frames_have_near_transition_and_far_black_frame() {
        let entries = crate::keyvalues::parse(
            r#"PlayerProximity { resultVar "$distance" scale .2 }
            Subtract { srcVar1 "$distance" srcVar2 "$near" resultVar "$delta" }
            Clamp { srcVar1 "$delta" min 0 max 30 resultVar "$frame" }
            Clamp { srcVar1 "$delta" min 0 max 30 resultVar "$frame2" }"#,
        )
        .unwrap();
        let f = Frames::parse(&entries, &BTreeMap::from([("$near".into(), "24".into())])).unwrap();
        assert_eq!(f.sample(119., [31, 31]), [0, 0]);
        assert_eq!(f.sample(125., [31, 31]), [1, 1]);
        assert_eq!(f.sample(200., [31, 31]), [16, 16]);
        assert_eq!(f.sample(270., [31, 31]), [30, 30]);
        assert_eq!(f.sample(900., [31, 31]), [30, 30]);
        let second =
            Frames::parse(&entries, &BTreeMap::from([("$near".into(), "22".into())])).unwrap();
        assert_eq!(second.sample(125., [31, 31]), [3, 3]);
        assert_eq!(f.sample(125., [31, 31]), [1, 1]); // Per-entity sampling is independent.
    }
    #[test]
    fn invalid_proxy_numbers_and_ranges_are_rejected() {
        for text in [
            r#"PlayerProximity { resultVar "$frame" scale NaN }"#,
            r#"Clamp { srcVar1 "$x" resultVar "$frame" min 2 max 1 }"#,
        ] {
            assert!(
                Frames::parse(&crate::keyvalues::parse(text).unwrap(), &BTreeMap::new()).is_err()
            );
        }
    }
}
