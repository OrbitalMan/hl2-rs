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
    if data.len() > 64 * 1024 * 1024
        || bytes(data, 0, 4)? != b"IDST"
        || !(44..=49).contains(&i32le(data, 4)?)
        || usize::try_from(i32le(data, 76)?)? != data.len()
    {
        bail!("invalid studio eyeball header");
    }
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
