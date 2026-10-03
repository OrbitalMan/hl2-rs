//! BSP leaf/PVS lookup used to separate the miniature 3D sky world from the playable world.
use crate::{bytes, i16le, i32le, index, records, u16le, vec3};
use anyhow::{bail, Context, Result};
use glam::Vec3;
use std::collections::BTreeSet;

pub fn cluster(lumps: &[Vec<u8>], leaf_version: u32, point: Vec3) -> Result<i16> {
    i16le(leaf(lumps, leaf_version, point)?, 4)
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
}
