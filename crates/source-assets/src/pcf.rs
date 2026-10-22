//! Particle system files (particles/*.pcf): binary DMX ("dmx encoding binary 2 format
//! pcf 1", every installed HL2 file) and the DmeParticleSystemDefinition elements they
//! hold (definition attributes, renderers, operators, initializers, emitters, children).
//! Reading only: the particle runtime that evaluates the operators is separate.
use anyhow::{bail, Context, Result};
use glam::Vec3;

/// A DMX attribute value (DmAttributeType_t; arrays are AT_*_ARRAY).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Element index (None: null).
    Element(Option<usize>),
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Binary(Vec<u8>),
    ObjectId([u8; 16]),
    Color([u8; 4]),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    QAngle([f32; 3]),
    Quaternion([f32; 4]),
    Matrix([f32; 16]),
    Array(Vec<Value>),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub type_name: String,
    pub name: String,
    pub attributes: Vec<(String, Value)>,
}
impl Element {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self.at.checked_add(n).context("DMX offset overflow")?;
        let bytes = self.data.get(self.at..end).context("DMX truncated")?;
        self.at = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn floats<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut out = [0.; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }
    fn cstr(&mut self) -> Result<String> {
        let rest = self.data.get(self.at..).context("DMX truncated")?;
        let len = rest
            .iter()
            .position(|b| *b == 0)
            .context("unterminated DMX string")?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.at += len + 1;
        Ok(s)
    }
    fn count(&mut self, limit: usize, what: &str) -> Result<usize> {
        let n = self.i32()?;
        let n = usize::try_from(n).with_context(|| format!("negative DMX {what} count"))?;
        if n > limit {
            bail!("implausible DMX {what} count {n}");
        }
        Ok(n)
    }
    fn value(&mut self, kind: u8, elements: usize) -> Result<Value> {
        Ok(match kind {
            1 => {
                let index = self.i32()?;
                match index {
                    -1 => Value::Element(None),
                    // -2: an external element (GUID + id string); not used by PCF files.
                    -2 => bail!("external DMX element references are unsupported"),
                    i => {
                        let i = usize::try_from(i).context("bad DMX element index")?;
                        if i >= elements {
                            bail!("DMX element index {i} out of range");
                        }
                        Value::Element(Some(i))
                    }
                }
            }
            2 => Value::Int(self.i32()?),
            3 => Value::Float(self.f32()?),
            4 => Value::Bool(self.u8()? != 0),
            5 => Value::String(self.cstr()?),
            6 => {
                let n = self.count(16 << 20, "binary")?;
                Value::Binary(self.take(n)?.to_vec())
            }
            7 => Value::ObjectId(self.take(16)?.try_into()?),
            8 => Value::Color(self.take(4)?.try_into()?),
            9 => Value::Vector2(self.floats()?),
            10 => Value::Vector3(self.floats()?),
            11 => Value::Vector4(self.floats()?),
            12 => Value::QAngle(self.floats()?),
            13 => Value::Quaternion(self.floats()?),
            14 => Value::Matrix(self.floats()?),
            15..=28 => {
                let n = self.count(1 << 20, "array")?;
                Value::Array(
                    (0..n)
                        .map(|_| self.value(kind - 14, elements))
                        .collect::<Result<_>>()?,
                )
            }
            other => bail!("unknown DMX attribute type {other}"),
        })
    }
}

/// Every element of a binary DMX file, version 2 (a short string dictionary for type
/// and attribute names; inline element names and string values).
pub fn read_dmx(data: &[u8]) -> Result<Vec<Element>> {
    let header_end = data
        .iter()
        .position(|b| *b == 0)
        .context("DMX header unterminated")?;
    let header = String::from_utf8_lossy(&data[..header_end]);
    if !header.contains("encoding binary 2 ") {
        bail!("unsupported DMX encoding: {}", header.trim());
    }
    let mut r = Reader {
        data,
        at: header_end + 1,
    };
    let string_count = usize::try_from(r.i16()?).context("negative DMX string count")?;
    let strings = (0..string_count)
        .map(|_| r.cstr())
        .collect::<Result<Vec<_>>>()?;
    let string = |r: &mut Reader| -> Result<String> {
        let i = usize::try_from(r.i16()?).context("negative DMX string index")?;
        strings
            .get(i)
            .cloned()
            .context("DMX string index out of range")
    };
    let element_count = r.count(1 << 20, "element")?;
    let mut elements = Vec::with_capacity(element_count);
    for _ in 0..element_count {
        let type_name = string(&mut r)?;
        let name = r.cstr()?;
        r.take(16)?;
        elements.push(Element {
            type_name,
            name,
            attributes: Vec::new(),
        });
    }
    for element in 0..element_count {
        let n = r.count(4096, "attribute")?;
        for _ in 0..n {
            let name = string(&mut r)?;
            let kind = r.u8()?;
            let value = r.value(kind, element_count)?;
            elements[element].attributes.push((name, value));
        }
    }
    Ok(elements)
}

/// One DmeParticleOperator: its functionName and parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Operator {
    pub function: String,
    pub params: Vec<(String, Value)>,
}
impl Operator {
    fn param(&self, name: &str) -> Option<&Value> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    }
    pub fn float(&self, name: &str) -> Option<f32> {
        match self.param(name)? {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f32),
            _ => None,
        }
    }
    pub fn int(&self, name: &str) -> Option<i32> {
        match self.param(name)? {
            Value::Int(v) => Some(*v),
            Value::Float(v) => Some(*v as i32),
            Value::Bool(v) => Some(i32::from(*v)),
            _ => None,
        }
    }
    pub fn vector(&self, name: &str) -> Option<Vec3> {
        match self.param(name)? {
            Value::Vector3(v) | Value::QAngle(v) => Some(Vec3::from_array(*v)),
            _ => None,
        }
    }
    pub fn color(&self, name: &str) -> Option<[u8; 4]> {
        match self.param(name)? {
            Value::Color(c) => Some(*c),
            _ => None,
        }
    }
}
/// A DmeParticleSystemDefinition.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSystem {
    pub name: String,
    pub material: Option<String>,
    pub max_particles: Option<i32>,
    pub radius: Option<f32>,
    pub color: Option<[u8; 4]>,
    /// Every definition attribute (bounding box, sorting, sequence numbers, ...).
    pub properties: Vec<(String, Value)>,
    pub renderers: Vec<Operator>,
    pub operators: Vec<Operator>,
    pub initializers: Vec<Operator>,
    pub emitters: Vec<Operator>,
    pub forces: Vec<Operator>,
    pub constraints: Vec<Operator>,
    /// DmeParticleChild: the child system's name and its delay.
    pub children: Vec<(String, f32)>,
}
/// The particle system definitions of a PCF file.
pub fn particle_systems(data: &[u8]) -> Result<Vec<ParticleSystem>> {
    let elements = read_dmx(data)?;
    let operators = |e: &Element, key: &str| -> Vec<Operator> {
        let Some(Value::Array(items)) = e.get(key) else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|v| match v {
                Value::Element(Some(i)) => elements.get(*i),
                _ => None,
            })
            .map(|op| Operator {
                function: match op.get("functionName") {
                    Some(Value::String(s)) => s.clone(),
                    _ => op.name.clone(),
                },
                params: op
                    .attributes
                    .iter()
                    .filter(|(k, _)| !k.eq_ignore_ascii_case("functionName"))
                    .cloned()
                    .collect(),
            })
            .collect()
    };
    Ok(elements
        .iter()
        .filter(|e| e.type_name == "DmeParticleSystemDefinition")
        .map(|e| {
            let children = match e.get("children") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| match v {
                        Value::Element(Some(i)) => elements.get(*i),
                        _ => None,
                    })
                    .filter_map(|child| {
                        let target = match child.get("child") {
                            Some(Value::Element(Some(i))) => elements.get(*i)?,
                            _ => return None,
                        };
                        let delay = match child.get("delay") {
                            Some(Value::Float(d)) => *d,
                            _ => 0.,
                        };
                        Some((target.name.clone(), delay))
                    })
                    .collect(),
                _ => Vec::new(),
            };
            ParticleSystem {
                name: e.name.clone(),
                material: match e.get("material") {
                    Some(Value::String(s)) => Some(s.replace('\\', "/").to_ascii_lowercase()),
                    _ => None,
                },
                max_particles: match e.get("max_particles") {
                    Some(Value::Int(n)) => Some(*n),
                    _ => None,
                },
                radius: match e.get("radius") {
                    Some(Value::Float(r)) => Some(*r),
                    _ => None,
                },
                color: match e.get("color") {
                    Some(Value::Color(c)) => Some(*c),
                    _ => None,
                },
                properties: e
                    .attributes
                    .iter()
                    .filter(|(_, v)| !matches!(v, Value::Array(_)))
                    .cloned()
                    .collect(),
                renderers: operators(e, "renderers"),
                operators: operators(e, "operators"),
                initializers: operators(e, "initializers"),
                emitters: operators(e, "emitters"),
                forces: operators(e, "forces"),
                constraints: operators(e, "constraints"),
                children,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A synthetic binary 2 file: one definition with one emitter operator.
    fn synthetic() -> Vec<u8> {
        let mut d = b"<!-- dmx encoding binary 2 format pcf 1 -->\n\0".to_vec();
        let strings = [
            "DmeElement",
            "DmeParticleSystemDefinition",
            "DmeParticleOperator",
            "particleSystemDefinitions",
            "emitters",
            "functionName",
            "emission_rate",
            "material",
        ];
        d.extend((strings.len() as i16).to_le_bytes());
        for s in strings {
            d.extend(s.as_bytes());
            d.push(0);
        }
        d.extend(3i32.to_le_bytes());
        for (t, name) in [(0i16, "root"), (1, "fire"), (2, "emit_continuously")] {
            d.extend(t.to_le_bytes());
            d.extend(name.as_bytes());
            d.push(0);
            d.extend([0u8; 16]);
        }
        // root: particleSystemDefinitions = [#1]
        d.extend(1i32.to_le_bytes());
        d.extend(3i16.to_le_bytes());
        d.push(15);
        d.extend(1i32.to_le_bytes());
        d.extend(1i32.to_le_bytes());
        // fire: emitters = [#2], material = "particle\fire.vmt"
        d.extend(2i32.to_le_bytes());
        d.extend(4i16.to_le_bytes());
        d.push(15);
        d.extend(1i32.to_le_bytes());
        d.extend(2i32.to_le_bytes());
        d.extend(7i16.to_le_bytes());
        d.push(5);
        d.extend(b"particle\\Fire.vmt\0");
        // emit_continuously: functionName, emission_rate 13.5
        d.extend(2i32.to_le_bytes());
        d.extend(5i16.to_le_bytes());
        d.push(5);
        d.extend(b"emit_continuously\0");
        d.extend(6i16.to_le_bytes());
        d.push(3);
        d.extend(13.5f32.to_le_bytes());
        d
    }
    #[test]
    fn binary_dmx_definitions_read_their_operators() {
        let systems = particle_systems(&synthetic()).unwrap();
        assert_eq!(systems.len(), 1);
        let fire = &systems[0];
        assert_eq!(fire.name, "fire");
        assert_eq!(fire.material.as_deref(), Some("particle/fire.vmt"));
        assert_eq!(fire.emitters[0].function, "emit_continuously");
        assert_eq!(fire.emitters[0].float("emission_rate"), Some(13.5));
        // Truncation is an error, not a panic.
        let data = synthetic();
        assert!(read_dmx(&data[..data.len() - 2]).is_err());
    }
    #[test]
    #[ignore = "requires an owned installed Half-Life 2 copy"]
    fn owned_particle_files_parse_and_burning_character_matches() {
        let vfs = crate::vpk::Vfs::mount(&crate::install::discover().unwrap()).unwrap();
        let manifest = String::from_utf8_lossy(
            &vfs.read("particles/particles_manifest.txt")
                .unwrap()
                .unwrap(),
        )
        .into_owned();
        let files: Vec<String> = crate::keyvalues::parse(&manifest).unwrap()[0]
            .children()
            .iter()
            .filter_map(|e| e.text().map(str::to_owned))
            .collect();
        assert_eq!(files.len(), 9);
        let mut total = 0;
        for file in &files {
            let data = vfs.read(file).unwrap().unwrap();
            total += particle_systems(&data).unwrap().len();
        }
        assert!(total > 50, "{total}");
        let systems =
            particle_systems(&vfs.read("particles/burning_fx.pcf").unwrap().unwrap()).unwrap();
        let names: Vec<&str> = systems.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"burning_character"), "{names:?}");
        let root = systems
            .iter()
            .find(|s| s.name == "burning_character")
            .unwrap();
        assert_eq!(root.emitters[0].function, "emit_continuously");
        assert_eq!(root.emitters[0].float("emission_rate"), Some(13.5));
        assert_eq!(root.renderers[0].function, "render_animated_sprites");
        assert_eq!(root.children[0].0, "burning_character_b");
        let child = systems
            .iter()
            .find(|s| s.name == "burning_character_d")
            .unwrap();
        assert_eq!(
            child.material.as_deref(),
            Some("particle/fire_burning_character/fire_burning_character.vmt")
        );
        assert_eq!(child.max_particles, Some(24));
    }
}
