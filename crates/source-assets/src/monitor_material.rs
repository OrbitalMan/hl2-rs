//! Bounded monitor proxies: literal Sine/TextureScroll/LinearRamp and variable translation.
//! Other proxy inputs and matrix operations are reported, rather than silently evaluated.
use crate::keyvalues::Entry;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Animation {
    proxies: Vec<Proxy>,
    variables: Vec<Variable>,
    pub unsupported: Vec<String>,
}
#[derive(Clone, Debug)]
struct Variable {
    values: [f32; 4],
    size: usize,
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
    Ramp {
        variable: usize,
        channel: Option<usize>,
        rate: f32,
        initial: f32,
    },
    Translate {
        variable: Option<usize>,
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
fn variable(
    name: &str,
    properties: &BTreeMap<String, String>,
    names: &mut BTreeMap<String, usize>,
    variables: &mut Vec<Variable>,
) -> Result<usize> {
    let name = name.to_lowercase();
    if let Some(&index) = names.get(&name) {
        return Ok(index);
    }
    let text = properties.get(&name).context("missing material variable")?;
    let values = text
        .trim()
        .trim_matches(['[', ']'])
        .split_whitespace()
        .map(str::parse::<f32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("nonliteral material variable")?;
    if !(1..=4).contains(&values.len()) || values.iter().any(|value| !value.is_finite()) {
        bail!("invalid material variable");
    }
    let index = variables.len();
    let mut initial = Variable {
        values: [0.; 4],
        size: values.len(),
    };
    initial.values[..values.len()].copy_from_slice(&values);
    variables.push(initial);
    names.insert(name, index);
    Ok(index)
}
fn result_variable(entry: &Entry) -> Result<(String, Option<usize>)> {
    let name = entry
        .get("resultVar")
        .and_then(Entry::text)
        .context("missing resultVar")?
        .to_lowercase();
    if let Some((name, component)) = name.split_once('[') {
        let channel = component
            .strip_suffix(']')
            .context("invalid result component")?
            .parse::<usize>()
            .context("invalid result component")?;
        Ok((name.into(), Some(channel)))
    } else {
        Ok((name, None))
    }
}
impl Animation {
    pub fn parse(entries: &[Entry]) -> Self {
        Self::parse_with_properties(entries, &BTreeMap::new())
    }
    /// Seed referenced variables once from the resolved VMT. Frame samples read no assets.
    pub fn parse_with_properties(entries: &[Entry], properties: &BTreeMap<String, String>) -> Self {
        let mut result = Self::default();
        let mut names = BTreeMap::new();
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
                } else if entry.key.eq_ignore_ascii_case("LinearRamp") {
                    let (name, channel) = result_variable(entry)?;
                    let variable = variable(&name, properties, &mut names, &mut result.variables)?;
                    if channel.is_some_and(|channel| channel >= result.variables[variable].size) {
                        bail!("result component outside material vector");
                    }
                    Ok(Proxy::Ramp {
                        variable,
                        channel,
                        rate: number(entry, "rate", 1.)?,
                        initial: number(entry, "initialValue", 0.)?,
                    })
                } else if entry.key.eq_ignore_ascii_case("TextureTransform") {
                    let (name, channel) = result_variable(entry)?;
                    if name != "$texture2transform" || channel.is_some() {
                        bail!("unsupported transform result {name}");
                    }
                    for field in ["centerVar", "scaleVar", "rotateVar"] {
                        if entry
                            .get(field)
                            .and_then(Entry::text)
                            .is_some_and(|v| !v.is_empty())
                        {
                            bail!("unsupported transform {field}");
                        }
                    }
                    let variable = entry
                        .get("translateVar")
                        .and_then(Entry::text)
                        .filter(|name| !name.is_empty())
                        .map(|name| variable(name, properties, &mut names, &mut result.variables))
                        .transpose()?;
                    if variable.is_some_and(|index| result.variables[index].size < 2) {
                        bail!("translation requires a material vector");
                    }
                    Ok(Proxy::Translate { variable })
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
    /// Ordered writes at the paused simulation clock. TextureScroll wraps its offsets;
    /// LinearRamp stays unbounded and translation wraps through the texture's Repeat sampler.
    pub fn sample(&self, mut color: [f32; 3], time: f32) -> ([f32; 3], [[f32; 3]; 2]) {
        let mut uv = [[1., 0., 0.], [0., 1., 0.]];
        let mut variables = self.variables.clone();
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
                Proxy::Ramp {
                    variable,
                    channel,
                    rate,
                    initial,
                } => {
                    let value = rate * time + initial;
                    if let Some(channel) = channel {
                        variables[variable].values[channel] = value;
                    } else {
                        variables[variable].values.fill(value);
                    }
                }
                Proxy::Translate { variable } => {
                    let translation = variable.map_or([0.; 4], |index| variables[index].values);
                    uv = [[1., 0., translation[0]], [0., 1., translation[1]]];
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
    fn ramp_then_translation_preserves_component_and_moves_down_without_clamping() {
        let entries = crate::keyvalues::parse(
            r#"LinearRamp {resultVar "$offset[1]" rate "-.91" initialValue ".125"}
            TextureTransform {translateVar "$offset" resultVar "$texture2transform"}"#,
        )
        .unwrap();
        let properties = BTreeMap::from([("$offset".into(), "[.25 0]".into())]);
        let animation = Animation::parse_with_properties(&entries, &properties);
        assert!(animation.unsupported.is_empty());
        let (color, first) = animation.sample([0.2, 0.3, 0.4], 0.);
        let (_, later) = animation.sample(color, 2.);
        assert_eq!(color, [0.2, 0.3, 0.4]);
        assert_eq!(first, [[1., 0., 0.25], [0., 1., 0.125]]);
        assert!((later[1][2] - -1.695).abs() < 1e-6);
        // A fixed feature sampled at uv+offset advances toward larger screen UV.y.
        assert!((first[1][2] - later[1][2] - 1.82).abs() < 1e-6);
        assert_eq!(animation.sample(color, 2.).1, later); // paused clock
        let ordered = crate::keyvalues::parse(
            r#"LinearRamp {resultVar "$offset[1]" rate "1"}
            LinearRamp {resultVar "$offset[1]" rate "2"}
            TextureTransform {translateVar "$offset" resultVar "$texture2transform"}"#,
        )
        .unwrap();
        assert_eq!(
            Animation::parse_with_properties(&ordered, &properties)
                .sample(color, 3.)
                .1[1][2],
            6.
        );
    }
    #[test]
    fn malformed_ramps_and_unsupported_matrix_inputs_are_reported() {
        let entries = crate::keyvalues::parse(
            r#"LinearRamp {resultVar "$missing[1]" rate "1"}
            LinearRamp {resultVar "$offset[2]" rate "1"}
            LinearRamp {resultVar "$offset[1]" rate "NaN"}
            LinearRamp {resultVar "$offset[no]" rate "1"}
            LinearRamp {resultVar "$invalid" rate "1"}
            TextureTransform {translateVar "$scalar" resultVar "$texture2transform"}
            TextureTransform {translateVar "$offset" rotateVar "$scalar" resultVar "$texture2transform"}
            TextureTransform {translateVar "$offset" resultVar "$basetexturetransform"}"#,
        )
        .unwrap();
        let properties = BTreeMap::from([
            ("$offset".into(), "[0 0]".into()),
            ("$scalar".into(), "1".into()),
            ("$invalid".into(), "[0 NaN]".into()),
        ]);
        let animation = Animation::parse_with_properties(&entries, &properties);
        assert_eq!(animation.unsupported.len(), 8);
        assert_eq!(
            animation.sample([1.; 3], 1.).1,
            [[1., 0., 0.], [0., 1., 0.]]
        );
    }
    #[test]
    #[ignore = "requires owned HL2 installation"]
    fn owned_kleiner_monitor_has_downward_ramp_and_translation() -> Result<()> {
        let vfs = crate::vpk::Vfs::mount(&crate::install::discover()?)?;
        let text = vfs
            .read("materials/dev/dev_tvmonitor1a.vmt")?
            .context("monitor material")?;
        let entries = crate::keyvalues::parse(std::str::from_utf8(&text)?)?;
        let material = &entries[0];
        let properties = material
            .children()
            .iter()
            .filter_map(|entry| {
                entry
                    .text()
                    .map(|text| (entry.key.to_lowercase(), text.into()))
            })
            .collect();
        let animation = Animation::parse_with_properties(
            material
                .get("Proxies")
                .context("monitor proxies")?
                .children(),
            &properties,
        );
        assert!(!animation
            .unsupported
            .iter()
            .any(|warning| warning.starts_with("LinearRamp:")));
        assert!(!animation.unsupported.iter().any(|warning| {
            warning.starts_with("TextureTransform:") && warning.contains("$texture2transform")
        }));
        assert_eq!(
            animation.sample([1.; 3], 0.).1,
            [[1., 0., 0.], [0., 1., 0.]]
        );
        assert!((animation.sample([1.; 3], 1.).1[1][2] - -0.91).abs() < 1e-6);
        assert!((animation.sample([1.; 3], 2.).1[1][2] - -1.82).abs() < 1e-6);
        assert!(animation
            .unsupported
            .iter()
            .any(|warning| warning.starts_with("GaussianNoise:")));
        Ok(())
    }
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
