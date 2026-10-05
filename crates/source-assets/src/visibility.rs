//! BSP leaf/PVS lookup used to separate the miniature 3D sky world from the playable world.
use crate::{bytes, i16le, i32le, index, records, u16le, vec3};
use anyhow::{bail, Context, Result};
use glam::Vec3;
use std::collections::BTreeSet;

pub fn cluster(lumps: &[Vec<u8>], leaf_version: u32, point: Vec3) -> Result<i16> {
    i16le(leaf(lumps, leaf_version, point)?, 4)
}

/// Test all leaves overlapped by an axis-aligned entity bound, rather than only its origin.
/// Area/areaportal connectivity is a separate engine query and is not evaluated here.
pub fn bounds_in_pvs(
    lumps: &[Vec<u8>],
    leaf_version: u32,
    eye: Vec3,
    mins: Vec3,
    maxs: Vec3,
) -> Result<bool> {
    if !mins.is_finite() || !maxs.is_finite() || mins.cmpgt(maxs).any() {
        bail!("invalid BSP visibility bounds");
    }
    let visible = pvs(
        lumps.get(4).context("BSP missing visibility")?,
        cluster(lumps, leaf_version, eye)?,
    )?;
    let nodes = lumps.get(5).context("BSP missing nodes")?;
    let planes = lumps.get(1).context("BSP missing planes")?;
    let leaves = lumps.get(10).context("BSP missing leaves")?;
    let leaf_size = if leaf_version == 0 { 56 } else { 32 };
    let center = (mins + maxs) * 0.5;
    let extent = (maxs - mins) * 0.5;
    if !center.is_finite() || !extent.is_finite() {
        bail!("BSP visibility bounds overflow");
    }
    let root = i32le(
        bytes(lumps.get(14).context("BSP missing models")?, 0, 48)?,
        36,
    )?;
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(node) = pending.pop() {
        if node < 0 {
            let id = index(
                node.checked_neg().context("leaf overflow")? - 1,
                leaves.len() / leaf_size,
            )?;
            if visible.contains(&i16le(bytes(leaves, id * leaf_size, leaf_size)?, 4)?) {
                return Ok(true);
            }
            continue;
        }
        if !visited.insert(node) {
            bail!("cyclic BSP visibility tree");
        }
        let record = bytes(nodes, index(node, nodes.len() / 32)? * 32, 32)?;
        let plane = bytes(
            planes,
            index(i32le(record, 0)?, planes.len() / 20)? * 20,
            20,
        )?;
        let normal = vec3(plane, 0)?;
        let distance = normal.dot(center) - crate::f32le(plane, 12)?;
        let radius = normal.abs().dot(extent);
        if distance + radius >= 0. {
            pending.push(i32le(record, 4)?);
        }
        if distance - radius < 0. {
            pending.push(i32le(record, 8)?);
        }
    }
    Ok(false)
}

/// Find the containing world leaf without allocating a copy of the node/leaf tables.
pub(crate) fn leaf(lumps: &[Vec<u8>], leaf_version: u32, point: Vec3) -> Result<&[u8]> {
    if !point.is_finite() {
        bail!("non-finite BSP lookup point");
    }
    let leaf_size = match leaf_version {
        0 => 56,
        1 => 32,
        _ => bail!("unsupported BSP leaf version {leaf_version}"),
    };
    let nodes = lumps.get(5).context("BSP missing nodes")?;
    let planes = lumps.get(1).context("BSP missing planes")?;
    let leaves = lumps.get(10).context("BSP missing leaves")?;
    // Validate complete records before traversing. Empty tables are allowed only when unused.
    let _ = records(nodes, 32)?;
    let _ = records(planes, 20)?;
    let _ = records(leaves, leaf_size)?;
    let mut node = i32le(
        bytes(lumps.get(14).context("BSP missing models")?, 0, 48)?,
        36,
    )?;
    for _ in 0..=nodes.len() / 32 {
        if node < 0 {
            let id = index(
                node.checked_neg().context("leaf overflow")? - 1,
                leaves.len() / leaf_size,
            )?;
            return bytes(leaves, id * leaf_size, leaf_size);
        }
        let record = bytes(nodes, index(node, nodes.len() / 32)? * 32, 32)?;
        let plane = bytes(
            planes,
            index(i32le(record, 0)?, planes.len() / 20)? * 20,
            20,
        )?;
        let back = vec3(plane, 0)?.dot(point) < crate::f32le(plane, 12)?;
        node = i32le(record, if back { 8 } else { 4 })?;
    }
    bail!("cyclic BSP node tree")
}

pub fn pvs(data: &[u8], cluster: i16) -> Result<BTreeSet<i16>> {
    if cluster < 0 || data.is_empty() {
        return Ok(BTreeSet::new());
    }
    let count = usize::try_from(i32le(data, 0)?)?;
    if count > 32768 {
        bail!("PVS cluster limit");
    }
    let id = index(cluster as i32, count)?;
    let offset = i32le(data, 4 + id * 8)?;
    if offset < 0 {
        return Ok((0..count).map(|i| i as i16).collect());
    }
    let mut i = offset as usize;
    let row = decompress(data, &mut i, count.div_ceil(8))?;
    Ok((0..count)
        .filter(|&c| row[c / 8] & (1 << (c % 8)) != 0)
        .map(|c| c as i16)
        .collect())
}
fn decompress(data: &[u8], cursor: &mut usize, length: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(length);
    while out.len() < length {
        let value = *data.get(*cursor).context("truncated PVS")?;
        *cursor += 1;
        if value != 0 {
            out.push(value);
        } else {
            let run = *data.get(*cursor).context("truncated PVS run")? as usize;
            *cursor += 1;
            if run == 0 || out.len() + run > length {
                bail!("invalid PVS zero run");
            }
            out.resize(out.len() + run, 0);
        }
    }
    Ok(out)
}
pub fn faces(
    lumps: &[Vec<u8>],
    leaf_version: u32,
    clusters: &BTreeSet<i16>,
) -> Result<BTreeSet<usize>> {
    let mut result = BTreeSet::new();
    for leaf in records(&lumps[10], if leaf_version == 0 { 56 } else { 32 })? {
        if !clusters.contains(&i16le(leaf, 4)?) {
            continue;
        }
        let first = u16le(leaf, 20)? as usize;
        let count = u16le(leaf, 22)? as usize;
        for face in records(bytes(&lumps[16], first * 2, count * 2)?, 2)? {
            result.insert(u16le(face, 0)? as usize);
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compressed_rows_preserve_cluster_bits_and_reject_bad_runs() {
        let mut cursor = 0;
        assert_eq!(
            decompress(&[5, 0, 2, 128], &mut cursor, 4).unwrap(),
            vec![5, 0, 0, 128]
        );
        assert!(decompress(&[0, 0], &mut 0, 1).is_err());
        assert!(decompress(&[0, 2], &mut 0, 1).is_err());
        assert!(decompress(&[0], &mut 0, 1).is_err());
    }
    #[test]
    fn bounds_visibility_crosses_leaf_planes_and_rejects_bad_bounds_and_cycles() {
        let mut lumps = vec![vec![]; 64];
        lumps[1] = vec![0; 20];
        lumps[1][0..4].copy_from_slice(&1f32.to_le_bytes());
        lumps[5] = vec![0; 32];
        lumps[5][4..8].copy_from_slice(&(-1i32).to_le_bytes());
        lumps[5][8..12].copy_from_slice(&(-2i32).to_le_bytes());
        lumps[10] = vec![0; 64];
        lumps[10][36..38].copy_from_slice(&1i16.to_le_bytes());
        lumps[14] = vec![0; 48];
        lumps[4] = vec![0; 22];
        lumps[4][0..4].copy_from_slice(&2i32.to_le_bytes());
        lumps[4][4..8].copy_from_slice(&20i32.to_le_bytes());
        lumps[4][12..16].copy_from_slice(&21i32.to_le_bytes());
        lumps[4][20] = 1;
        lumps[4][21] = 2;
        let eye = Vec3::X;
        assert!(bounds_in_pvs(&lumps, 1, eye, Vec3::X, Vec3::splat(2.)).unwrap());
        assert!(!bounds_in_pvs(&lumps, 1, eye, Vec3::splat(-2.), -Vec3::X).unwrap());
        assert!(bounds_in_pvs(&lumps, 1, eye, Vec3::splat(-1.), Vec3::splat(1.)).unwrap());
        assert!(bounds_in_pvs(&lumps, 1, eye, Vec3::splat(f32::NAN), Vec3::ZERO).is_err());
        assert!(bounds_in_pvs(&lumps, 1, eye, Vec3::ONE, Vec3::ZERO).is_err());
        lumps[5][4..8].copy_from_slice(&0i32.to_le_bytes());
        assert!(bounds_in_pvs(&lumps, 1, eye, Vec3::ZERO, Vec3::ONE).is_err());
    }
}
