//! BSP lump 45 info_overlay records (SDK bspfile.h doverlay_t, 352 bytes). Projection onto
//! the listed faces and rendering are separate steps.
use crate::{f32le, i16le, i32le, records, u16le, vec3};
use anyhow::{bail, Result};
use glam::{Vec2, Vec3};

pub const RECORD_SIZE: usize = 352;
const FACE_SLOTS: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    pub id: i32,
    pub texinfo: i16,
    /// OVERLAY_RENDER_ORDER (top two bits of the face count field).
    pub render_order: u16,
    pub faces: Vec<i32>,
    pub u: [f32; 2],
    pub v: [f32; 2],
    /// Corner positions in the overlay plane (VBSP stores the U basis in their z).
    pub uv_points: [Vec2; 4],
    pub origin: Vec3,
    pub basis_normal: Vec3,
    /// SDK vbsp overlay.cpp: BasisU packed into vecUVPoints[0..3].z.
    pub basis_u: Vec3,
    /// vecUVPoints[3].z = 1 when the V axis is flipped.
    pub flip_v: bool,
}

pub fn read(lump: &[u8]) -> Result<Vec<Overlay>> {
    records(lump, RECORD_SIZE)?
        .map(|r| {
            let packed = u16le(r, 6)?;
            let count = usize::from(packed & 0x3fff);
            if count > FACE_SLOTS {
                bail!("overlay face count {count} exceeds {FACE_SLOTS}");
            }
            let faces = (0..count)
                .map(|i| i32le(r, 8 + 4 * i))
                .collect::<Result<Vec<_>>>()?;
            let at = 8 + 4 * FACE_SLOTS;
            let points = [
                vec3(r, at + 16)?,
                vec3(r, at + 28)?,
                vec3(r, at + 40)?,
                vec3(r, at + 52)?,
            ];
            Ok(Overlay {
                id: i32le(r, 0)?,
                texinfo: i16le(r, 4)?,
                render_order: packed >> 14,
                faces,
                u: [f32le(r, at)?, f32le(r, at + 4)?],
                v: [f32le(r, at + 8)?, f32le(r, at + 12)?],
                uv_points: points.map(|p| Vec2::new(p.x, p.y)),
                origin: vec3(r, at + 64)?,
                basis_normal: vec3(r, at + 76)?,
                basis_u: Vec3::new(points[0].z, points[1].z, points[2].z),
                flip_v: points[3].z == 1.,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_doverlay_layout() {
        let mut r = vec![0u8; RECORD_SIZE];
        r[0..4].copy_from_slice(&7i32.to_le_bytes());
        r[4..6].copy_from_slice(&3i16.to_le_bytes());
        r[6..8].copy_from_slice(&(2u16 | (1 << 14)).to_le_bytes());
        r[8..12].copy_from_slice(&11i32.to_le_bytes());
        r[12..16].copy_from_slice(&12i32.to_le_bytes());
        let at = 8 + 4 * FACE_SLOTS;
        r[at + 4..at + 8].copy_from_slice(&1f32.to_le_bytes());
        r[at + 76 + 8..at + 88].copy_from_slice(&1f32.to_le_bytes());
        r[at + 16 + 8..at + 28].copy_from_slice(&1f32.to_le_bytes());
        r[at + 52 + 8..at + 64].copy_from_slice(&1f32.to_le_bytes());
        let o = &read(&r).unwrap()[0];
        assert_eq!((o.id, o.texinfo, o.render_order), (7, 3, 1));
        assert_eq!(o.faces, vec![11, 12]);
        assert_eq!(o.u, [0., 1.]);
        assert_eq!(o.basis_normal, Vec3::Z);
        assert_eq!(o.basis_u, Vec3::X);
        assert!(o.flip_v);
        r[6..8].copy_from_slice(&65u16.to_le_bytes());
        assert!(read(&r).is_err());
        assert!(read(&r[..100]).is_err());
    }
    #[test]
    #[ignore = "requires owned HL2 installation"]
    fn owned_trainstation_overlays_reference_valid_faces() {
        let mut vfs = crate::vpk::Vfs::mount(std::path::Path::new(
            &std::env::var("HL2_ROOT").expect("set HL2_ROOT"),
        ))
        .unwrap();
        let bytes = vfs.read("maps/d1_trainstation_02.bsp").unwrap().unwrap();
        let bsp = crate::bsp::Bsp::parse(&bytes).unwrap();
        vfs.mount_pak(bsp.lump(40)).unwrap();
        let overlays = read(bsp.lump(45)).unwrap();
        let faces = bsp.lump(7).len().max(bsp.lump(58).len()) / 56;
        let texinfos = bsp.lump(6).len() / 72;
        assert!(!overlays.is_empty());
        for o in &overlays {
            assert!(!o.faces.is_empty() && o.faces.iter().all(|&f| (f as usize) < faces));
            assert!((o.texinfo as usize) < texinfos);
            assert!((o.basis_normal.length() - 1.).abs() < 1e-3);
            assert!((o.basis_u.length() - 1.).abs() < 1e-3);
            assert!(o.basis_u.dot(o.basis_normal).abs() < 1e-3);
        }
        eprintln!("{} overlays", overlays.len());
    }
}
