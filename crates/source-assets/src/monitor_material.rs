//! Bounded literal Sine/TextureScroll subset for owned procedural monitor materials.
//! Variable-driven inputs and other proxies are reported, rather than silently evaluated.
use crate::keyvalues::Entry;
use anyhow::{bail, Context, Result};

#[derive(Clone, Debug, Default)]
pub struct Animation {
    proxies: Vec<Proxy>,
    pub unsupported: Vec<String>,
}
#[derive(Clone, Debug)]
enum Proxy {
    Sine {
        channel: Option<usize>,
        period: f32,
        min: f32,
        max: f32,
        offset: f32,
    },
    Scroll {
        rate: f32,
        angle: f32,
        scale: f32,
    },
}
fn number(entry: &Entry, name: &str, default: f32) -> Result<f32> {
    let Some(text) = entry.get(name).and_then(Entry::text) else {
        return Ok(default);
    };
    let value: f32 = text.parse().with_context(|| format!("nonliteral {name}"))?;
    if !value.is_finite() {
        bail!("nonfinite {name}");
    }
    Ok(value)
}
impl Animation {
    pub fn parse(entries: &[Entry]) -> Self {
        let mut result = Self::default();
        for entry in entries.iter().take(128) {
            let proxy = (|| -> Result<Proxy> {
                if entry.key.eq_ignore_ascii_case("Sine") {
                    let var = entry
                        .get("resultVar")
                        .and_then(Entry::text)
                        .unwrap_or("")
                        .to_lowercase();
                    let channel = match var.as_str() {
                        "$color" => None,
                        "$color[0]" => Some(0),
                        "$color[1]" => Some(1),
                        "$color[2]" => Some(2),
                        _ => bail!("unsupported result {var}"),
                    };
                    Ok(Proxy::Sine {
                        channel,
                        period: number(entry, "sinePeriod", 1.)?,
                        min: number(entry, "sineMin", 0.)?,
                        max: number(entry, "sineMax", 1.)?,
                        offset: number(entry, "timeOffset", 0.)?,
                    })
                } else if entry.key.eq_ignore_ascii_case("TextureScroll") {
                    let var = entry
                        .get("textureScrollVar")
                        .and_then(Entry::text)
                        .unwrap_or("");
                    if !var.eq_ignore_ascii_case("$texture2transform") {
                        bail!("unsupported scroll variable {var}");
                    }
                    Ok(Proxy::Scroll {
                        rate: number(entry, "textureScrollRate", 1.)?,
                        angle: number(entry, "textureScrollAngle", 0.)?,
                        scale: number(entry, "textureScale", 1.)?,
                    })
                } else {
                    bail!("unsupported proxy");
                }
            })();
            match proxy {
                Ok(proxy) => result.proxies.push(proxy),
                Err(error) => result.unsupported.push(format!("{}: {error:#}", entry.key)),
            }
        }
        if entries.len() > 128 {
            result.unsupported.push("proxy count exceeds128".into());
        }
        result
    }
    /// Ordered writes at the paused simulation clock. Matrix scrolling wraps negative offsets.
    pub fn sample(&self, mut color: [f32; 3], time: f32) -> ([f32; 3], [[f32; 3]; 2]) {
        let mut uv = [[1., 0., 0.], [0., 1., 0.]];
        for proxy in &self.proxies {
            match *proxy {
                Proxy::Sine {
                    channel,
                    period,
                    min,
                    max,
                    offset,
                } => {
                    let period = if period == 0. { 1. } else { period };
                    let wave = (std::f32::consts::TAU * (time - offset) / period).sin() * 0.5 + 0.5;
                    let value = (max - min) * wave + min;
                    if let Some(channel) = channel {
                        color[channel] = value;
                    } else {
                        color = [value; 3];
                    }
                }
                Proxy::Scroll { rate, angle, scale } => {
                    let (sin, cos) = angle.to_radians().sin_cos();
                    uv = [
                        [scale, 0., (time * cos * rate).rem_euclid(1.)],
                        [0., scale, (time * sin * rate).rem_euclid(1.)],
                    ];
                }
            }
        }
        (color, uv)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_literal_proxies_preserve_phase_channels_and_negative_scroll() {
        let entries=crate::keyvalues::parse(r#"Sine {resultVar "$color[0]" sinePeriod "2" sineMin "0.2" sineMax "0.6" timeOffset "0.5"}
            Sine {resultVar "$color[1]" sinePeriod "0" sineMin "0.9" sineMax "0.9"}
            TextureScroll {textureScrollVar "$texture2transform" textureScrollRate "0.5" textureScrollAngle "-90" textureScale "2"}
            GaussianNoise {resultVar "$alpha"}"#).unwrap();
        let animation = Animation::parse(&entries);
        assert_eq!(animation.unsupported.len(), 1);
        let (color, uv) = animation.sample([1.; 3], 1.);
        assert!((color[0] - 0.6).abs() < 1e-6);
        assert_eq!(color[1..], [0.9, 1.]);
        assert_eq!(uv[0][0], 2.);
        assert!((uv[1][2] - 0.5).abs() < 1e-6);
        assert_eq!(animation.sample([1.; 3], 1.), (color, uv));
        let invalid=crate::keyvalues::parse(r#"Sine {resultVar "$color[3]"} TextureScroll {textureScrollVar "$texture2transform" textureScrollRate "NaN"}"#).unwrap();
        assert_eq!(Animation::parse(&invalid).unsupported.len(), 2);
    }
}
