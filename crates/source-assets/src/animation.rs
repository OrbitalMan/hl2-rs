//! Bounded Source MDL/ANI sequence reader. Blends, IK and flexes are separate work.
use crate::{bytes, f32le, i16le, i32le, u16le, u32le, vec3, vpk::Vfs};
use anyhow::{bail, Context, Result};
use glam::{Mat4, Quat, Vec3};
use modkit_core::animation::{Bone, Clip, Pose, Rig};
use std::collections::BTreeSet;
fn offset(data: &[u8], field: usize) -> Result<usize> {
    Ok(usize::try_from(i32le(data, field)?)?)
}
fn relative(data: &[u8], base: usize, field: usize) -> Result<usize> {
    Ok(usize::try_from(base as i64 + i32le(data, field)? as i64)?)
}
fn string(data: &[u8], at: usize) -> Result<String> {
    let tail = data.get(at..).context("string outside MDL")?;
    let end = tail
        .iter()
        .take(4096)
        .position(|b| *b == 0)
        .context("unterminated MDL string")?;
    Ok(std::str::from_utf8(&tail[..end])?.into())
}
struct SourceBone {
    bone: Bone,
    euler: Vec3,
    position_scale: Vec3,
    rotation_scale: Vec3,
}
fn bones(data: &[u8]) -> Result<Vec<SourceBone>> {
    let count = offset(data, 156)?;
    if count > 256 {
        bail!("MDL skeleton exceeds 256 bones");
    }
    let base = offset(data, 160)?;
    bytes(data, base, count * 216)?;
    (0..count)
        .map(|id| {
            let at = base + id * 216;
            let parent = i32le(data, at + 4)?;
            if parent >= id as i32 {
                bail!("unordered/cyclic skeleton");
            }
            let q = Quat::from_xyzw(
                f32le(data, at + 44)?,
                f32le(data, at + 48)?,
                f32le(data, at + 52)?,
                f32le(data, at + 56)?,
            )
            .normalize();
            let mut m = [0.; 16];
            for row in 0..3 {
                for col in 0..4 {
                    m[col * 4 + row] = f32le(data, at + 96 + (row * 4 + col) * 4)?;
                }
            }
            m[15] = 1.;
            Ok(SourceBone {
                bone: Bone {
                    name: string(data, relative(data, at, at)?)?,
                    parent: usize::try_from(parent).ok(),
                    bind: Pose {
                        position: vec3(data, at + 32)?,
                        rotation: q,
                    },
                    inverse_bind: Mat4::from_cols_array(&m),
                },
                euler: vec3(data, at + 60)?,
                position_scale: vec3(data, at + 72)?,
                rotation_scale: vec3(data, at + 84)?,
            })
        })
        .collect()
}
fn rle(data: &[u8], mut at: usize, mut frame: usize) -> Result<f32> {
    for _ in 0..4096 {
        let header = bytes(data, at, 2)?;
        let valid = header[0] as usize;
        let total = header[1] as usize;
        if total == 0 || valid == 0 || valid > total {
            bail!("invalid animation RLE span");
        }
        if frame < total {
            return Ok(i16le(data, at + 2 * (frame.min(valid - 1) + 1))? as f32);
        }
        frame -= total;
        at += 2 * (valid + 1);
    }
    bail!("animation RLE span budget exceeded")
}
fn values(data: &[u8], at: usize, frame: usize) -> Result<Vec3> {
    let mut out = [0.; 3];
    for (axis, value) in out.iter_mut().enumerate() {
        let off = i16le(data, at + axis * 2)?;
        if off > 0 {
            *value = rle(data, at + off as usize, frame)?;
        }
    }
    Ok(Vec3::from_array(out))
}
fn quaternion(data: &[u8], at: usize, wide: bool) -> Result<Quat> {
    let (x, y, z, negative) = if wide {
        let q = u64::from_le_bytes(bytes(data, at, 8)?.try_into()?);
        let value = |shift: u32| (((q >> shift) & 0x1fffffu64) as f32 - 1048576.) / 1048576.5;
        (value(0), value(21), value(42), q >> 63 != 0)
    } else {
        let z = u16le(data, at + 4)?;
        (
            (u16le(data, at)? as f32 - 32768.) / 32768.,
            (u16le(data, at + 2)? as f32 - 32768.) / 32768.,
            ((z & 0x7fff) as f32 - 16384.) / 16384.,
            z & 0x8000 != 0,
        )
    };
    let w = (1. - x * x - y * y - z * z).max(0.).sqrt() * if negative { -1. } else { 1. };
    Ok(Quat::from_xyzw(x, y, z, w).normalize())
}
fn euler(v: Vec3) -> Quat {
    Quat::from_rotation_z(v.z) * Quat::from_rotation_y(v.y) * Quat::from_rotation_x(v.x)
}
fn frame(data: &[u8], mut at: usize, bones: &[SourceBone], frame: usize) -> Result<Vec<Pose>> {
    let mut poses = bones
        .iter()
        .map(|b| b.bone.bind.clone())
        .collect::<Vec<_>>();
    for _ in 0..256 {
        let header = bytes(data, at, 4)?;
        let id = header[0] as usize;
        if id == 255 {
            break;
        }
        let bone = bones.get(id).context("animation bone outside skeleton")?;
        let flags = header[1];
        if flags & !0x3f != 0 {
            bail!("unsupported compressed animation flags {flags:#x}");
        }
        let delta = flags & 16 != 0;
        let raw_rotation_bytes = if flags & 2 != 0 {
            6
        } else if flags & 32 != 0 {
            8
        } else {
            0
        };
        let rotation = if raw_rotation_bytes > 0 {
            quaternion(data, at + 4, raw_rotation_bytes == 8)?
        } else if flags & 8 != 0 {
            let angles = values(data, at + 4, frame)? * bone.rotation_scale
                + if delta { Vec3::ZERO } else { bone.euler };
            euler(angles)
        } else if delta {
            Quat::IDENTITY
        } else {
            bone.bone.bind.rotation
        };
        let position = if flags & 1 != 0 {
            let at = at + 4 + raw_rotation_bytes;
            Vec3::new(
                half::f16::from_bits(u16le(data, at)?).to_f32(),
                half::f16::from_bits(u16le(data, at + 2)?).to_f32(),
                half::f16::from_bits(u16le(data, at + 4)?).to_f32(),
            )
        } else if flags & 4 != 0 {
            values(data, at + 4 + if flags & 8 != 0 { 6 } else { 0 }, frame)? * bone.position_scale
                + if delta {
                    Vec3::ZERO
                } else {
                    bone.bone.bind.position
                }
        } else if delta {
            Vec3::ZERO
        } else {
            bone.bone.bind.position
        };
        poses[id] = if delta {
            Pose {
                position: bone.bone.bind.position + position,
                rotation: bone.bone.bind.rotation * rotation,
            }
        } else {
            Pose { position, rotation }
        };
        let next = i16le(data, at + 2)?;
        if next == 0 {
            return Ok(poses);
        }
        if next < 4 {
            bail!("invalid animation record chain");
        }
        at += next as usize;
    }
    Ok(poses)
}
fn clip(
    vfs: &Vfs,
    mdl: &[u8],
    ani: Option<&[u8]>,
    bones: &[SourceBone],
    animation: usize,
    looping: bool,
) -> Result<Clip> {
    let count = offset(mdl, 180)?;
    if animation >= count {
        bail!("sequence animation outside table");
    }
    let at = offset(mdl, 184)? + animation * 100;
    bytes(mdl, at, 100)?;
    let count = offset(mdl, at + 16)?;
    if count == 0 || count > 4096 {
        bail!("animation exceeds frame limit");
    }
    let fps = f32le(mdl, at + 8)?.clamp(1., 240.);
    let section_frames = offset(mdl, at + 84)?;
    let section_base = relative(mdl, at, at + 80)?;
    let mut frames = Vec::with_capacity(count);
    for f in 0..count {
        let (mut block, mut index) = (i32le(mdl, at + 52)?, i32le(mdl, at + 56)?);
        let mut local_frame = f;
        if section_frames > 0 {
            let section = if count > section_frames && f == count - 1 {
                local_frame = 0;
                count / section_frames + 1
            } else {
                local_frame = f % section_frames;
                f / section_frames
            };
            block = i32le(mdl, section_base + section * 8)?;
            index = i32le(mdl, section_base + section * 8 + 4)?;
        }
        if block < 0 {
            bail!("unavailable animation block");
        }
        let (data, start) = if block == 0 {
            (mdl, usize::try_from(at as i64 + index as i64)?)
        } else {
            let block = block as usize;
            if block >= offset(mdl, 352)? {
                bail!("ANI block outside table");
            }
            let record = offset(mdl, 356)? + block * 8;
            let start = offset(mdl, record)?;
            let end = offset(mdl, record + 4)?;
            let data = bytes(
                ani.context("ANI companion missing")?,
                start,
                end.checked_sub(start).context("reversed ANI block")?,
            )?;
            (data, usize::try_from(index)?)
        };
        frames.push(frame(data, start, bones, local_frame)?);
    }
    let _ = vfs;
    Ok(Clip {
        events: Vec::new(),
        fps,
        looping,
        frames,
    })
}
pub fn load(vfs: &Vfs, path: &str, wanted: &BTreeSet<String>) -> Result<Rig> {
    let data = vfs.read(path)?.context("MDL missing for animation")?;
    let base = bones(&data)?;
    let mut rig = Rig {
        bones: base.iter().map(|b| b.bone.clone()).collect(),
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    load_sequences(vfs, path, &data, wanted, &mut rig, &mut seen, 0)?;
    Ok(rig)
}
fn sequence_events(
    data: &[u8],
    sequence: usize,
) -> Result<(Vec<modkit_core::animation::ClipEvent>, usize)> {
    let count = offset(data, sequence + 24)?;
    if count > 4096 {
        bail!("sequence event limit");
    }
    if count == 0 {
        return Ok((Vec::new(), 0));
    }
    let base = relative(data, sequence, sequence + 28)?;
    bytes(data, base, count * 80)?;
    let mut events = Vec::new();
    let mut discarded = 0;
    for i in 0..count {
        let at = base + i * 80;
        let cycle = f32::from_le_bytes(bytes(data, at, 4)?.try_into()?);
        // Some shipped unused one-frame sequences contain NaN event cycles. Never schedule those.
        if !cycle.is_finite() || !(0. ..=1.).contains(&cycle) {
            discarded += 1;
            continue;
        }
        let options = bytes(data, at + 12, 64)?;
        let end = options.iter().position(|b| *b == 0).unwrap_or(64);
        let name = if i32le(data, at + 76)? != 0 {
            string(data, relative(data, at, at + 76)?)?
        } else {
            String::new()
        };
        events.push(modkit_core::animation::ClipEvent {
            cycle,
            id: i32le(data, at + 4)?,
            flags: u32le(data, at + 8)?,
            name,
            options: std::str::from_utf8(&options[..end])?.into(),
        });
    }
    Ok((events, discarded))
}
fn load_sequences(
    vfs: &Vfs,
    path: &str,
    data: &[u8],
    wanted: &BTreeSet<String>,
    rig: &mut Rig,
    seen: &mut BTreeSet<String>,
    depth: usize,
) -> Result<()> {
    if depth > 8 || !seen.insert(path.to_lowercase()) {
        return Ok(());
    }
    let source_bones = bones(data)?;
    let name = string(data, offset(data, 348)?)?.replace('\\', "/");
    let ani = if offset(data, 352)? > 0 {
        vfs.read(&name)?
    } else {
        None
    };
    let sequences = offset(data, 188)?;
    if sequences > 4096 {
        bail!("sequence table too large");
    }
    let base = offset(data, 192)?;
    for id in 0..sequences {
        let at = base + id * 212;
        bytes(data, at, 212)?;
        let name = string(data, relative(data, at, at + 4)?)?.to_lowercase();
        if !wanted.contains(&name) || rig.clips.contains_key(&name) {
            continue;
        }
        if rig.clips.len() >= 64 {
            rig.warnings
                .push("sequence load budget of 64 reached".into());
            break;
        }
        // Select the central sample for blend grids; pose-parameter blends are not evaluated yet.
        let blends = offset(data, at + 56)?.max(1);
        let table = relative(data, at, at + 60)?;
        let animation = usize::try_from(i16le(data, table + (blends / 2) * 2)?)?;
        match clip(
            vfs,
            data,
            ani.as_deref(),
            &source_bones,
            animation,
            u32le(data, at + 12)? & 1 != 0,
        ) {
            Ok(mut clip) => {
                let (events, discarded) = sequence_events(data, at)?;
                clip.events = events;
                if discarded > 0 {
                    rig.warnings.push(format!(
                        "{path}:{name}: discarded {discarded} invalid event cycles"
                    ));
                }
                let remap = rig
                    .bones
                    .iter()
                    .map(|b| {
                        source_bones
                            .iter()
                            .position(|s| s.bone.name.eq_ignore_ascii_case(&b.name))
                    })
                    .collect::<Vec<_>>();
                for frame in &mut clip.frames {
                    *frame = rig
                        .bones
                        .iter()
                        .zip(&remap)
                        .map(|(b, id)| {
                            id.and_then(|id| frame.get(id))
                                .cloned()
                                .unwrap_or_else(|| b.bind.clone())
                        })
                        .collect();
                }
                rig.clips.insert(name, clip);
            }
            Err(e) => rig.warnings.push(format!("{path}:{name}: {e:#}")),
        }
    }
    let includes = offset(data, 336)?;
    if includes > 64 {
        bail!("included model table exceeds limit");
    }
    let base = offset(data, 340)?;
    for id in 0..includes {
        let at = base + id * 8;
        let include = string(data, relative(data, at, at + 4)?)?
            .replace('\\', "/")
            .to_lowercase();
        if let Some(bytes) = vfs.read(&include)? {
            if let Err(e) = load_sequences(vfs, &include, &bytes, wanted, rig, seen, depth + 1) {
                rig.warnings.push(format!("{include}: {e:#}"));
            }
        } else {
            rig.warnings
                .push(format!("included model missing: {include}"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event_data() -> Vec<u8> {
        let mut data = vec![0; 400];
        // The sequence begins at 32; its event table is relative to that sequence.
        data[56..60].copy_from_slice(&2i32.to_le_bytes());
        data[60..64].copy_from_slice(&96i32.to_le_bytes());
        data[128..132].copy_from_slice(&0.25f32.to_le_bytes());
        data[132..136].copy_from_slice(&5004i32.to_le_bytes());
        data[136..140].copy_from_slice(&1024u32.to_le_bytes());
        data[140..144].copy_from_slice(b"test");
        data[204..208].copy_from_slice(&192i32.to_le_bytes());
        data[320..336].copy_from_slice(b"AE_CL_PLAYSOUND\0");
        data[208..212].copy_from_slice(&f32::NAN.to_le_bytes());
        data
    }
    #[test]
    fn sequence_relative_events_read_options_and_skip_invalid_cycle() {
        let (events, invalid) = sequence_events(&event_data(), 32).unwrap();
        assert_eq!(invalid, 1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].cycle, 0.25);
        assert_eq!(events[0].options, "test");
        assert_eq!(events[0].name, "AE_CL_PLAYSOUND");
    }
    #[test]
    fn truncated_or_outside_event_records_are_rejected() {
        let mut data = event_data();
        assert!(sequence_events(&data[..220], 32).is_err());
        data[204..208].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(sequence_events(&data, 32).is_err());
    }
    #[test]
    fn signed_rle_repeats_and_crosses_spans() {
        let bytes = [2, 4, 0xff, 0xff, 2, 0, 1, 2, 0xfd, 0xff];
        assert_eq!(rle(&bytes, 0, 0).unwrap(), -1.);
        assert_eq!(rle(&bytes, 0, 3).unwrap(), 2.);
        assert_eq!(rle(&bytes, 0, 5).unwrap(), -3.);
        assert!(rle(&[0, 0], 0, 0).is_err());
    }
    #[test]
    fn compressed_identity_quaternion() {
        let q = quaternion(&[0, 128, 0, 128, 0, 64], 0, false).unwrap();
        assert!(q.dot(Quat::IDENTITY) > 0.99999);
    }
}
