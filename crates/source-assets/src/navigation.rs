//! Bounded PC AIN37 node-graph reader. Coordinates remain in Source space.
//! Layout reference: Valve's pinned Source SDK network save/load implementation:
//! https://github.com/ValveSoftware/source-sdk-2013/blob/b8cfb12c0e083a2ef5b2f9f9b50f3902fa034474/src/game/server/ai_networkmanager.cpp
//! This decodes navigation data; it does not select routes or simulate NPCs.
use crate::{bytes, f32le, i16le, i32le, u16le, u32le, vec3};
use anyhow::{bail, Context, Result};
use glam::Vec3;

pub const HULL_COUNT: usize = 10;
/// Native PC graph node limit. Link/file limits below are decoder budgets.
const MAX_NODES: usize = 1500;
const MAX_LINKS: usize = 1_000_000;
const MAX_FILE: usize = 16 * 1024 * 1024;
const NODE_BYTES: usize = 61;
const LINK_BYTES: usize = 14;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub origin: Vec3,
    pub yaw: f32,
    /// Source hull order: human, small centered, wide human, tiny, wide short,
    /// medium, tiny centered, large, large centered, medium tall.
    pub hull_z_offsets: [f32; HULL_COUNT],
    /// Kept raw so unknown kinds and flags are not silently reclassified.
    pub node_type: u8,
    pub info: u16,
    pub zone: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub source: usize,
    pub destination: usize,
    /// One raw movement mask per hull, in the same order as node Z offsets.
    pub accepted_move_types: [u8; HULL_COUNT],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub version: u32,
    pub map_revision: u32,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    /// One signed editor/Hammer node ID per node, including negative sentinels.
    pub hammer_ids: Vec<i32>,
    /// Bytes after the known PC AIN37 tables. They are preserved for reporting,
    /// not interpreted as an extension or used to establish graph validity.
    pub trailing_bytes: Vec<u8>,
}

fn count(data: &[u8], at: usize, maximum: usize, label: &str) -> Result<usize> {
    let value = i32le(data, at)?;
    let value = usize::try_from(value).with_context(|| format!("negative AIN {label} count"))?;
    if value > maximum {
        bail!("AIN {label} count {value} exceeds limit {maximum}");
    }
    Ok(value)
}

fn table_end(data: &[u8], start: usize, count: usize, stride: usize) -> Result<usize> {
    let size = count
        .checked_mul(stride)
        .context("AIN table size overflow")?;
    bytes(data, start, size)?;
    start.checked_add(size).context("AIN table end overflow")
}

impl Graph {
    /// Requires the caller's BSP revision. Shipped-game revision exceptions in
    /// the native loader are deliberately not guessed by this bounded reader.
    /// Only PC packed records are decoded. The header has no platform marker;
    /// callers must supply PC graph files, not Xbox padded-node variants.
    pub fn parse(data: &[u8], expected_bsp_revision: u32) -> Result<Self> {
        if data.len() > MAX_FILE {
            bail!("AIN file exceeds reader budget");
        }
        let version = u32le(data, 0)?;
        if version != 37 {
            bail!("unsupported PC AIN version {version}");
        }
        let map_revision = u32le(data, 4)?;
        if map_revision != expected_bsp_revision {
            bail!("AIN/BSP revision mismatch: graph {map_revision}, BSP {expected_bsp_revision}");
        }
        let node_count = count(data, 8, MAX_NODES, "node")?;
        let links_count_at = table_end(data, 12, node_count, NODE_BYTES)?;
        let link_count = count(data, links_count_at, MAX_LINKS, "link")?;
        let links_start = links_count_at
            .checked_add(4)
            .context("AIN link offset overflow")?;
        let hammer_start = table_end(data, links_start, link_count, LINK_BYTES)?;
        let parsed_end = table_end(data, hammer_start, node_count, 4)?;

        // Validate complete table ranges before reserving space from file counts.
        let mut nodes = Vec::with_capacity(node_count);
        for node in 0..node_count {
            let at = 12 + node * NODE_BYTES;
            let mut hull_z_offsets = [0.; HULL_COUNT];
            for (hull, offset) in hull_z_offsets.iter_mut().enumerate() {
                *offset = f32le(data, at + 16 + hull * 4)?;
            }
            nodes.push(Node {
                origin: vec3(data, at)?,
                yaw: f32le(data, at + 12)?,
                hull_z_offsets,
                node_type: bytes(data, at + 56, 1)?[0],
                info: u16le(data, at + 57)?,
                zone: i16le(data, at + 59)?,
            });
        }
        let mut links = Vec::with_capacity(link_count);
        for link in 0..link_count {
            let at = links_start + link * LINK_BYTES;
            let endpoint = |field| -> Result<usize> {
                let value = i16le(data, field)?;
                let value = usize::try_from(value).context("negative AIN link endpoint")?;
                if value >= node_count {
                    bail!("AIN link {link} endpoint {value} exceeds node count {node_count}");
                }
                Ok(value)
            };
            links.push(Link {
                source: endpoint(at)?,
                destination: endpoint(at + 2)?,
                accepted_move_types: bytes(data, at + 4, HULL_COUNT)?.try_into()?,
            });
        }
        let hammer_ids = (0..node_count)
            .map(|node| i32le(data, hammer_start + node * 4))
            .collect::<Result<_>>()?;
        Ok(Self {
            version,
            map_revision,
            nodes,
            links,
            hammer_ids,
            trailing_bytes: data[parsed_end..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Entirely synthetic; no installed graph bytes or game-specific coordinates.
    fn fixture() -> Vec<u8> {
        let mut data = Vec::new();
        for value in [37i32, 42, 2] {
            data.extend(value.to_le_bytes());
        }
        for (position, yaw, kind, info, zone) in [
            ([12.5f32, -2.25, 6.], -540f32, 2u8, 0x1234u16, 7i16),
            ([-99., 0., 0.5], 450., 255, 0xabcd, -123),
        ] {
            for value in position.into_iter().chain([yaw]) {
                data.extend(value.to_le_bytes());
            }
            for hull in 0..HULL_COUNT {
                data.extend((hull as f32 * -0.25).to_le_bytes());
            }
            data.push(kind);
            data.extend(info.to_le_bytes());
            data.extend(zone.to_le_bytes());
        }
        data.extend(2i32.to_le_bytes());
        for (source, destination, masks) in [
            (0i16, 1i16, [1u8, 2, 4, 8, 16, 32, 64, 128, 255, 0]),
            (1, 0, [10, 9, 8, 7, 6, 5, 4, 3, 2, 1]),
        ] {
            data.extend(source.to_le_bytes());
            data.extend(destination.to_le_bytes());
            data.extend(masks);
        }
        for id in [-1i32, 314] {
            data.extend(id.to_le_bytes());
        }
        data
    }

    #[test]
    fn mixed_hulls_and_raw_metadata_are_preserved() {
        let graph = Graph::parse(&fixture(), 42).unwrap();
        assert_eq!((graph.version, graph.map_revision), (37, 42));
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes[0].origin, Vec3::new(12.5, -2.25, 6.));
        assert_eq!(graph.nodes[1].origin, Vec3::new(-99., 0., 0.5));
        assert_eq!(graph.nodes[0].yaw, -540.);
        assert_eq!(graph.nodes[1].yaw, 450.);
        assert_eq!(
            graph.nodes[0].hull_z_offsets,
            std::array::from_fn(|i| i as f32 * -0.25)
        );
        assert_eq!(
            (
                graph.nodes[0].node_type,
                graph.nodes[0].info,
                graph.nodes[0].zone
            ),
            (2, 0x1234, 7)
        );
        assert_eq!(
            (
                graph.nodes[1].node_type,
                graph.nodes[1].info,
                graph.nodes[1].zone
            ),
            (255, 0xabcd, -123)
        );
        assert_eq!(
            graph.links,
            [
                Link {
                    source: 0,
                    destination: 1,
                    accepted_move_types: [1, 2, 4, 8, 16, 32, 64, 128, 255, 0]
                },
                Link {
                    source: 1,
                    destination: 0,
                    accepted_move_types: [10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
                },
            ]
        );
        assert_eq!(graph.hammer_ids, [-1, 314]);
        assert!(graph.trailing_bytes.is_empty());
    }

    #[test]
    fn every_truncation_of_known_tables_is_rejected() {
        let data = fixture();
        for end in 0..data.len() {
            assert!(Graph::parse(&data[..end], 42).is_err(), "cut at {end}");
        }
    }

    #[test]
    fn negative_and_excessive_counts_are_rejected_before_allocation() {
        for (at, maximum) in [(8, MAX_NODES), (12 + 2 * NODE_BYTES, MAX_LINKS)] {
            for value in [-1, i32::MIN, maximum as i32 + 1, i32::MAX] {
                let mut data = fixture();
                data[at..at + 4].copy_from_slice(&value.to_le_bytes());
                assert!(Graph::parse(&data, 42).is_err(), "count {value} at {at}");
            }
        }
    }

    #[test]
    fn every_link_endpoint_must_reference_an_existing_node() {
        let links_start = 12 + 2 * NODE_BYTES + 4;
        for link in 0..2 {
            for field in [0, 2] {
                let at = links_start + link * LINK_BYTES + field;
                for value in [-1i16, i16::MIN, 2, i16::MAX] {
                    let mut data = fixture();
                    data[at..at + 2].copy_from_slice(&value.to_le_bytes());
                    assert!(Graph::parse(&data, 42).is_err(), "endpoint {value} at {at}");
                }
            }
        }
    }

    #[test]
    fn all_positions_yaws_and_hull_offsets_must_be_finite() {
        for node in 0..2 {
            for field in 0..14 {
                let at = 12 + node * NODE_BYTES + field * 4;
                for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                    let mut data = fixture();
                    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
                    assert!(Graph::parse(&data, 42).is_err(), "float at {at}");
                }
            }
        }
    }

    #[test]
    fn version_revision_and_file_budget_are_guarded() {
        assert!(Graph::parse(&fixture(), 41).is_err());
        for version in [0u32, 36, 38, u32::MAX] {
            let mut data = fixture();
            data[..4].copy_from_slice(&version.to_le_bytes());
            assert!(Graph::parse(&data, 42).is_err());
        }
        let mut oversized = fixture();
        oversized.resize(MAX_FILE + 1, 0);
        assert!(Graph::parse(&oversized, 42).is_err());
    }

    #[test]
    fn empty_graph_and_uninterpreted_tail_are_explicit() {
        let mut empty = Vec::new();
        for value in [37i32, 42, 0, 0] {
            empty.extend(value.to_le_bytes());
        }
        let graph = Graph::parse(&empty, 42).unwrap();
        assert!(graph.nodes.is_empty() && graph.links.is_empty() && graph.hammer_ids.is_empty());
        let mut data = fixture();
        data.extend([0xde, 0xad, 0xbe, 0xef]);
        let graph = Graph::parse(&data, 42).unwrap();
        assert_eq!(graph.trailing_bytes, [0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(graph.hammer_ids, [-1, 314]);
    }
}
