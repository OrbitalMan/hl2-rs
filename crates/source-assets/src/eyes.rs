//! Bounded PC studio eyeball metadata; reads the same first body models as meshes.
use crate::{bytes, f32le, i32le, vec3, vpk::Vfs};
use anyhow::{bail, Context, Result};
use glam::Vec3;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Eyeball {
    pub surface: usize,
    pub bone: usize,
    pub origin: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub radius: f32,
    pub z_offset: f32,
    pub iris_scale: f32,
}
/// Server actor attention uses the animated "eyes" attachment when available.
#[derive(Clone, Debug)]
pub struct Attachment {
    pub bone: usize,
    pub local: glam::Mat4,
    pub world_align: bool,
}
#[derive(Clone, Debug)]
pub struct Gaze {
    pub view_offset: Vec3,
    pub attachment: Option<Attachment>,
}
pub fn load_gaze(vfs: &Vfs, model: &str) -> Result<Gaze> {
    decode_gaze(&vfs.read(model)?.context("gaze MDL absent")?)
}
fn decode_gaze(data: &[u8]) -> Result<Gaze> {
    validate_header(data)?;
    let view_offset = vec3(data, 80)?;
    let bones = count(data, 156, 256)?;
    let attachments = count(data, 240, 4096)?;
    let base = usize::try_from(i32le(data, 244)?)?;
    bytes(data, base, attachments * 92)?;
    let mut result = Gaze {
        view_offset,
        attachment: None,
    };
    for id in 0..attachments {
        let at = base + id * 92;
        let start = relative(data, at, at)?;
        let tail = data.get(start..).context("attachment name outside MDL")?;
        let end = tail
            .iter()
            .take(1025)
            .position(|b| *b == 0)
            .context("attachment name unterminated or exceeds 1024 bytes")?;
        let name = std::str::from_utf8(&tail[..end])?;
        if !name.eq_ignore_ascii_case("eyes") {
            continue;
        }
        let bone = usize::try_from(i32le(data, at + 8)?)?;
        if bone >= bones {
            bail!("eyes attachment bone outside skeleton");
        }
        let flags = crate::u32le(data, at + 4)?;
        if flags & !0x10000 != 0 {
            bail!("unsupported eyes attachment flags {flags:#x}");
        }
        let mut matrix = [0.; 16];
        matrix[15] = 1.;
        for row in 0..3 {
            for col in 0..4 {
                matrix[col * 4 + row] = f32le(data, at + 12 + (row * 4 + col) * 4)?;
            }
        }
        let local = glam::Mat4::from_cols_array(&matrix);
        let determinant = local.determinant();
        if !determinant.is_finite() || determinant.abs() < 1e-8 {
            bail!("singular eyes attachment transform");
        }
        result.attachment = Some(Attachment {
            bone,
            local,
            world_align: flags & 0x10000 != 0,
        });
        break;
    }
    Ok(result)
}
fn validate_header(data: &[u8]) -> Result<()> {
    if data.len() > 64 * 1024 * 1024
        || bytes(data, 0, 4)? != b"IDST"
        || !(44..=49).contains(&i32le(data, 4)?)
        || usize::try_from(i32le(data, 76)?)? != data.len()
    {
        bail!("invalid studio eye header");
    }
    Ok(())
}
/// Retail StudioRender planar iris basis, at default eyeball-size adjustment.
/// `bone` is the current bone-to-model transform, before entity scale/rotation.
pub fn projection(eye: &Eyeball, bone: glam::Mat4, target: Option<Vec3>) -> [glam::Vec4; 2] {
    let origin = bone.transform_point3(eye.origin);
    let authored_up = bone.transform_vector3(eye.up).normalize();
    let fallback = -bone.transform_vector3(eye.forward).normalize();
    let mut forward = target.map_or(fallback, |p| {
        (p - origin).try_normalize().unwrap_or(fallback)
    });
    let right = forward
        .cross(authored_up)
        .try_normalize()
        .unwrap_or_else(|| fallback.cross(authored_up).normalize());
    forward = (forward + right * (eye.z_offset * 2.)).normalize();
    let right = forward.cross(authored_up).normalize();
    let up = right.cross(forward).normalize();
    let u = -right * eye.iris_scale;
    let v = -up * eye.iris_scale;
    [u.extend(0.5 - u.dot(origin)), v.extend(0.5 - v.dot(origin))]
}
pub fn load(vfs: &Vfs, model: &str) -> Result<Vec<Eyeball>> {
    decode(&vfs.read(model)?.context("eyeball MDL absent")?)
}
fn count(data: &[u8], at: usize, limit: usize) -> Result<usize> {
    let n = usize::try_from(i32le(data, at)?)?;
    if n > limit {
        bail!("studio eyeball table exceeds limit at {at}");
    }
    Ok(n)
}
fn relative(data: &[u8], base: usize, at: usize) -> Result<usize> {
    Ok(usize::try_from(base as i64 + i32le(data, at)? as i64)?)
}
fn decode(data: &[u8]) -> Result<Vec<Eyeball>> {
    validate_header(data)?;
    let bones = count(data, 156, 4096)?;
    let bodies = count(data, 232, 256)?;
    let start = usize::try_from(i32le(data, 236)?)?;
    bytes(data, start, bodies * 16)?;
    let mut result = Vec::new();
    let mut surface = 0;
    for body in 0..bodies {
        let b = start + body * 16;
        if count(data, b + 4, 256)? == 0 {
            continue;
        }
        let m = relative(data, b, b + 12)?;
        bytes(data, m, 148)?;
        let meshes = count(data, m + 72, 4096)?;
        let mesh_start = relative(data, m, m + 76)?;
        bytes(data, mesh_start, meshes * 116)?;
        let eyes = count(data, m + 100, 32)?;
        let eye_start = relative(data, m, m + 104)?;
        bytes(data, eye_start, eyes * 172)?;
        for mesh in 0..meshes {
            let at = mesh_start + mesh * 116;
            if i32le(data, at + 24)? != 1 {
                continue;
            }
            let index = usize::try_from(i32le(data, at + 28)?)?;
            if index >= eyes {
                bail!("mesh eyeball index outside table");
            }
            let at = eye_start + index * 172;
            let eye = Eyeball {
                surface: surface + mesh,
                bone: usize::try_from(i32le(data, at + 4)?)?,
                origin: vec3(data, at + 8)?,
                z_offset: f32le(data, at + 20)?,
                radius: f32le(data, at + 24)?,
                up: vec3(data, at + 28)?,
                forward: vec3(data, at + 40)?,
                iris_scale: f32le(data, at + 60)?,
            };
            if eye.bone >= bones
                || eye.radius <= 0.
                || eye.iris_scale <= 0.
                || eye.up.length_squared() < 0.5
                || eye.forward.length_squared() < 0.5
                || eye.up.cross(eye.forward).length_squared() < 0.25
            {
                bail!("invalid studio eyeball basis/bone/scalars");
            }
            result.push(eye);
            if result.len() > 512 {
                bail!("studio eyeball budget exceeded");
            }
        }
        surface += meshes;
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut d = vec![0; 800];
        d[..4].copy_from_slice(b"IDST");
        for (at, n) in [
            (4, 48i32),
            (76, 800),
            (156, 1),
            (232, 1),
            (236, 240),
            (244, 1),
            (252, 16),
            (328, 1),
            (332, 148),
            (356, 1),
            (360, 264),
            (428, 1),
        ] {
            d[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        // body +240, model +256, mesh +404, eyeball +520.
        for (at, v) in [(544, 0.5f32), (548, 1.), (564, 1.), (580, 1.5)] {
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        d
    }
    fn gaze_fixture() -> Vec<u8> {
        let mut d = vec![0u8; 500];
        d[..4].copy_from_slice(b"IDST");
        for (at, value) in [
            (4, 48i32),
            (76, 500),
            (156, 1),
            (240, 1),
            (244, 300),
            (300, 100),
        ] {
            d[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        d[400..405].copy_from_slice(b"eyes\0");
        for (at, value) in [
            (88, 70f32),
            (312, 1.),
            (332, 1.),
            (352, 1.),
            (324, 4.),
            (340, 5.),
            (356, 6.),
        ] {
            d[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        d
    }
    #[test]
    fn attachment_basis_fallback_and_malformed_records_are_distinguished() {
        let d = gaze_fixture();
        let gaze = decode_gaze(&d).unwrap();
        assert_eq!(gaze.view_offset, Vec3::new(0., 0., 70.));
        let a = gaze.attachment.unwrap();
        assert_eq!(a.bone, 0);
        assert_eq!(a.local.transform_point3(Vec3::ZERO), Vec3::new(4., 5., 6.));
        assert!(!a.world_align);
        for (at, n) in [
            (244, 490u32),
            (308, 1),
            (300, 1000),
            (316, 0x7fc00000),
            (304, 1),
            (312, 0),
        ] {
            let mut bad = d.clone();
            bad[at..at + 4].copy_from_slice(&n.to_le_bytes());
            assert!(decode_gaze(&bad).is_err(), "offset {at}");
        }
        let mut overflow = d.clone();
        for at in [312, 332] {
            overflow[at..at + 4].copy_from_slice(&f32::MAX.to_le_bytes());
        }
        assert!(decode_gaze(&overflow).is_err());
        let mut missing = d.clone();
        missing[400..405].copy_from_slice(b"hand\0");
        assert!(decode_gaze(&missing).unwrap().attachment.is_none());
        missing[240..244].copy_from_slice(&0i32.to_le_bytes());
        assert!(decode_gaze(&missing).unwrap().attachment.is_none());
    }
    #[test]
    #[ignore = "requires owned installed HL2 actor MDLs"]
    fn owned_actor_gaze_attachments_match_bone_skeletons() {
        let vfs = Vfs::mount(&crate::install::discover().unwrap()).unwrap();
        for model in [
            "models/barney.mdl",
            "models/kleiner.mdl",
            "models/humans/group01/male_07.mdl",
            "models/gman_high.mdl",
            "models/police.mdl",
        ] {
            let gaze = load_gaze(&vfs, model).unwrap();
            let attachment = gaze.attachment.expect(model);
            let rig = crate::animation::load(&vfs, model, &["idle_subtle".into()].into()).unwrap();
            assert!(attachment.bone < rig.bones.len(), "{model}");
            let matrix = rig.matrices("idle_subtle", 0.)[attachment.bone]
                * rig.bones[attachment.bone].inverse_bind.inverse()
                * attachment.local;
            assert!(matrix.is_finite());
            assert!(
                (matrix.transform_vector3(Vec3::X).length() - 1.).abs() < 1e-3,
                "{model}"
            );
        }
    }
    #[test]
    fn valid_eye_and_malformed_tables_are_distinguished() {
        let d = fixture();
        let eyes = decode(&d).unwrap();
        assert_eq!(eyes.len(), 1);
        assert_eq!(eyes[0].surface, 0);
        assert_eq!(eyes[0].up, Vec3::X);
        assert_eq!(eyes[0].forward, Vec3::Y);
        for (at, n) in [
            (532, 0x7fc00000u32),
            (524, 1),
            (432, 4),
            (356, 33),
            (360, 10000),
        ] {
            let mut bad = d.clone();
            bad[at..at + 4].copy_from_slice(&n.to_le_bytes());
            assert!(decode(&bad).is_err());
        }
        assert!(decode(&d[..799]).is_err());
    }
    #[test]
    fn planar_basis_centers_eye_tracks_target_and_preserves_bone_transform() {
        let eye = decode(&fixture()).unwrap().remove(0);
        let rows = projection(&eye, glam::Mat4::IDENTITY, None);
        assert_eq!(rows[0].dot(eye.origin.extend(1.)), 0.5);
        assert_eq!(rows[1].dot(eye.origin.extend(1.)), 0.5);
        assert!((rows[0].truncate().length() - eye.iris_scale).abs() < 1e-6);
        assert!(rows[0].truncate().dot(rows[1].truncate()).abs() < 1e-6);
        let bone = glam::Mat4::from_rotation_translation(
            glam::Quat::from_rotation_z(0.7),
            Vec3::new(10., 3., -9.),
        );
        let transformed = projection(&eye, bone, None);
        for p in [Vec3::ZERO, Vec3::Z * 0.3, Vec3::X * 0.4] {
            for i in 0..2 {
                assert!(
                    (rows[i].dot(p.extend(1.))
                        - transformed[i].dot(bone.transform_point3(p).extend(1.)))
                    .abs()
                        < 2e-6
                );
            }
        }
        let tracked = projection(&eye, glam::Mat4::IDENTITY, Some(Vec3::new(0., -20., 5.)));
        assert_ne!(tracked[0], rows[0]);
        assert_eq!(
            projection(&eye, glam::Mat4::IDENTITY, Some(eye.origin)),
            rows
        );
    }

    #[test]
    #[ignore = "requires owned installed HL2"]
    fn owned_eyes_match_mesh_materials_and_bone_projection() {
        let vfs = Vfs::mount(&crate::install::discover().unwrap()).unwrap();
        for model in [
            "models/barney.mdl",
            "models/humans/group01/male_07.mdl",
            "models/gman_high.mdl",
            "models/vortigaunt.mdl",
        ] {
            let eyes = load(&vfs, model).unwrap();
            assert!(eyes.len() >= 2, "{model}");
            let meshes = crate::models::read_model(&vfs, model, 0).unwrap();
            let rig =
                crate::animation::load(&vfs, model, &std::collections::BTreeSet::new()).unwrap();
            for eye in eyes {
                assert!(meshes[eye.surface].material.to_lowercase().contains("eye"));
                let bone = rig.bones[eye.bone].inverse_bind.inverse();
                assert!(projection(&eye, bone, None).iter().all(|r| r.is_finite()));
            }
        }
    }
}
