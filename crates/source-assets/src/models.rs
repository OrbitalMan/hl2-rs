//! Static-prop game lumps, model meshes, and installed sequence adapters.
use crate::{bytes, i32le, u16le, u32le, vec3, vpk::Vfs};
use anyhow::{bail, Context, Result};
use glam::{Mat3, Vec2, Vec3};
use modkit_core::{parse_vec3, ModelInstance, Surface, Vertex, World};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub fn static_props(bsp: &[u8], lump: &[u8]) -> Result<Vec<ModelInstance>> {
    if lump.is_empty() {
        return Ok(Vec::new());
    }
    let count = u32le(lump, 0)? as usize;
    bytes(
        lump,
        4,
        count.checked_mul(16).context("game lump overflow")?,
    )?;
    for n in 0..count {
        let record = &lump[4 + n * 16..4 + (n + 1) * 16];
        if u32le(record, 0)? != 0x73707270 {
            continue;
        }
        if u16le(record, 4)? & 1 != 0 {
            bail!("compressed static-prop lump is unsupported");
        }
        let version = u16le(record, 6)?;
        let data = bytes(bsp, u32le(record, 8)? as usize, u32le(record, 12)? as usize)?;
        return parse_static(data, version);
    }
    Ok(Vec::new())
}
fn parse_static(data: &[u8], version: u16) -> Result<Vec<ModelInstance>> {
    let stride = match version {
        4 => 56,
        5 => 60,
        6 => 64,
        _ => bail!("unsupported static-prop version {version}"),
    };
    let names = u32le(data, 0)? as usize;
    let dictionary = bytes(
        data,
        4,
        names.checked_mul(128).context("prop dictionary overflow")?,
    )?;
    let mut offset = 4 + dictionary.len();
    let leaves = u32le(data, offset)? as usize;
    offset += 4;
    offset += bytes(
        data,
        offset,
        leaves.checked_mul(2).context("prop leaf overflow")?,
    )?
    .len();
    let count = u32le(data, offset)? as usize;
    offset += 4;
    let records = bytes(
        data,
        offset,
        count.checked_mul(stride).context("prop count overflow")?,
    )?;
    let mut props = Vec::with_capacity(count);
    for record in records.chunks_exact(stride) {
        let name = bytes(dictionary, u16le(record, 24)? as usize * 128, 128)?;
        let name = std::str::from_utf8(&name[..name.iter().position(|&b| b == 0).unwrap_or(128)])?;
        props.push(ModelInstance {
            background: false,
            model: name.replace('\\', "/"),
            origin: vec3(record, 0)?,
            angles: vec3(record, 12)?,
            skin: usize::try_from(i32le(record, 32)?)?,
            scale: 1.,
            kind: "static_prop".into(),
            solid: record[30] != 0,
            solid_mode: Some(record[30]),
            entity: None,
        });
    }
    Ok(props)
}
pub fn read_model(vfs: &Vfs, path: &str, skin: usize) -> Result<Vec<Surface>> {
    let stem = path.trim_end_matches(".mdl");
    let mdl_bytes = vfs.read(path)?.context("MDL missing")?;
    let vvd_bytes = vfs.read(&format!("{stem}.vvd"))?.context("VVD missing")?;
    let vtx_bytes = vfs
        .read(&format!("{stem}.dx90.vtx"))?
        .context("DX90 VTX missing")?;
    if bytes(&mdl_bytes, 0, 4)? != b"IDST" || bytes(&vvd_bytes, 0, 4)? != b"IDSV" {
        bail!("model magic mismatch");
    }
    if !(44..=49).contains(&u32le(&mdl_bytes, 4)?)
        || u32le(&vvd_bytes, 4)? != 4
        || u32le(&vtx_bytes, 0)? != 7
    {
        bail!("unsupported MDL/VVD/VTX version");
    }
    let checksum = u32le(&mdl_bytes, 8)?;
    if checksum != u32le(&vvd_bytes, 8)? || checksum != u32le(&vtx_bytes, 16)? {
        bail!("model companion checksum mismatch");
    }
    // Geometry loading must not enter vmdl's unimplemented external animation-block path.
    let mut geometry_bytes = mdl_bytes.clone();
    bytes(&geometry_bytes, 180, 4)?;
    geometry_bytes[180..184].copy_from_slice(&0i32.to_le_bytes());
    let mdl = vmdl::Mdl::read(&geometry_bytes)?;
    let skin_reference_count = usize::try_from(i32le(&mdl_bytes, 220)?)?;
    let vvd = vmdl::Vvd::read(&vvd_bytes)?;
    let vtx = vmdl::Vtx::read(&vtx_bytes)?;
    let mut surfaces = Vec::new();
    for (body, topology) in mdl.body_parts.iter().zip(&vtx.body_parts) {
        let Some(model) = body.models.first() else {
            continue;
        };
        let lod = topology
            .models
            .first()
            .and_then(|m| m.lods.first())
            .context("missing model LOD")?;
        for (mesh, topology) in model.meshes.iter().zip(&lod.meshes) {
            let material = usize::try_from(mesh.material)?;
            let texture_index = if skin_reference_count > 0 {
                let index = skin
                    .checked_mul(skin_reference_count)
                    .and_then(|i| i.checked_add(material))
                    .context("skin overflow")?;
                *mdl.skin_table
                    .get(index)
                    .or_else(|| mdl.skin_table.get(material))
                    .context("skin material missing")? as usize
            } else {
                material
            };
            let texture = mdl
                .textures
                .get(texture_index)
                .context("model texture index outside table")?;
            let candidates = mdl
                .texture_paths
                .iter()
                .map(|p| format!("{}{}", p.replace('\\', "/"), texture.name))
                .chain(std::iter::once(texture.name.clone()));
            let mut material_name = texture.name.clone();
            if let Some(resolved) = vfs.resolve_material_name(&texture.name) {
                material_name = resolved;
            }
            for candidate in candidates {
                if vfs.read(&format!("materials/{candidate}.vmt"))?.is_some() {
                    material_name = candidate;
                    break;
                }
            }
            let mut surface = Surface {
                background: false,
                material: material_name,
                lightmap: None,
                vertices: Vec::new(),
                indices: Vec::new(),
            };
            let base = usize::try_from(model.vertex_offset)?
                .checked_add(usize::try_from(mesh.vertex_offset)?)
                .context("model vertex overflow")?;
            for group in &topology.strip_groups {
                for strip in &group.strips {
                    let mut indices: Vec<_> = strip.indices().collect();
                    // vmdl 0.2 expands two extra triangles for a triangle strip.
                    if strip.flags.contains(vmdl::vtx::StripFlags::IS_TRI_STRIP) {
                        indices.truncate(indices.len().saturating_sub(6));
                    }
                    for triangle in indices.as_chunks::<3>().0 {
                        for &index in triangle {
                            let group_index = *group
                                .indices
                                .get(index)
                                .context("strip index out of range")?
                                as usize;
                            let vertex_id = group
                                .vertices
                                .get(group_index)
                                .context("VTX vertex missing")?
                                .original_mesh_vertex_id
                                as usize;
                            let vertex = vvd
                                .vertices
                                .get(base + vertex_id)
                                .context("VVD vertex missing")?;
                            surface.indices.push(surface.vertices.len() as u32);
                            surface.vertices.push(Vertex {
                                position: Vec3::new(
                                    vertex.position.x,
                                    vertex.position.y,
                                    vertex.position.z,
                                ),
                                uv: Vec2::from_array(vertex.texture_coordinates),
                                color: [180, 180, 180, 255],
                                light_uv: Vec2::ZERO,
                                skin: Some({
                                    let mut bones = [0; 3];
                                    let mut weights = [0.; 3];
                                    for (i, w) in vertex.bone_weights.weights().enumerate() {
                                        bones[i] = w.bone_id;
                                        weights[i] = w.weight;
                                    }
                                    modkit_core::animation::Weights { bones, weights }
                                }),
                            });
                        }
                    }
                }
            }
            if !surface.indices.is_empty() {
                surfaces.push(surface);
            }
        }
    }
    if surfaces.is_empty() {
        bail!("model has no default-bodygroup triangles");
    }
    Ok(surfaces)
}
pub fn rotation(angles: Vec3) -> Mat3 {
    Mat3::from_rotation_z(angles.y.to_radians())
        * Mat3::from_rotation_y(angles.x.to_radians())
        * Mat3::from_rotation_x(angles.z.to_radians())
}
#[derive(Default, Debug, Serialize)]
pub struct ModelReport {
    pub instances_loaded: usize,
    pub unique_models: usize,
    pub errors: Vec<String>,
    pub rigs_loaded: usize,
    pub clips_loaded: usize,
    pub animation_warnings: Vec<String>,
    pub collision_models_loaded: usize,
    pub collision_pieces_loaded: usize,
    pub collision_models_missing: usize,
    pub collision_warnings: Vec<String>,
}
pub fn append_models(world: &mut World, vfs: &Vfs) -> ModelReport {
    for (entity_id, entity) in world.entities.iter().enumerate() {
        if !entity.class().starts_with("npc_")
            && !entity.class().starts_with("weapon_")
            && !entity.class().starts_with("item_")
            && !matches!(
                entity.class(),
                "prop_physics"
                    | "prop_physics_multiplayer"
                    | "prop_dynamic"
                    | "prop_dynamic_override"
                    | "prop_door_rotating"
            )
        {
            continue;
        }
        let default_model = match entity.class() {
            "npc_metropolice" => Some("models/police.mdl"),
            "npc_citizen" => Some("models/humans/group01/male_07.mdl"),
            _ => None,
        };
        let script_model = if entity.class().starts_with("weapon_") {
            vfs.read(&format!("scripts/{}.txt", entity.class()))
                .ok()
                .flatten()
                .and_then(|data| {
                    crate::keyvalues::value(&String::from_utf8_lossy(&data), "playermodel")
                        .ok()
                        .flatten()
                })
        } else {
            None
        };
        let item_model = match entity.class() {
            "item_healthkit" => Some("models/items/healthkit.mdl"),
            "item_healthvial" => Some("models/healthvial.mdl"),
            "item_battery" => Some("models/items/battery.mdl"),
            "item_suit" => Some("models/items/hevsuit.mdl"),
            "item_ammo_pistol" | "item_ammo_pistol_large" => Some("models/items/boxsrounds.mdl"),
            _ => None,
        };
        if let Some(model) = entity
            .get("model")
            .filter(|m| m.ends_with(".mdl"))
            .or(script_model.as_deref())
            .or(item_model)
            .or(default_model)
        {
            world.model_instances.push(ModelInstance {
                background: world.background_entities.contains(&entity_id),
                model: model.replace('\\', "/"),
                origin: entity.origin(),
                angles: parse_vec3(entity.get("angles").unwrap_or("0 0 0")).unwrap_or(Vec3::ZERO),
                skin: entity.get("skin").and_then(|s| s.parse().ok()).unwrap_or(0),
                scale: entity
                    .get("modelscale")
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|s| s.is_finite() && *s > 0.)
                    .unwrap_or(1.),
                kind: entity.class().into(),
                solid: !entity.class().starts_with("item_")
                    && !entity.class().starts_with("weapon_")
                    && entity.get("solid").is_none_or(|s| s != "0"),
                solid_mode: None,
                entity: Some(entity_id),
            });
        }
    }
    let mut cache: BTreeMap<(String, usize), Option<Vec<Surface>>> = BTreeMap::new();
    let mut report = ModelReport::default();
    let mut collision_attempted = BTreeSet::new();
    let mut collision_modes_reported = BTreeSet::new();
    let mut batches: BTreeMap<(String, Option<usize>, bool), Surface> = world
        .surfaces
        .drain(..)
        .map(|s| ((s.material.clone(), s.lightmap, s.background), s))
        .collect();
    for instance in &world.model_instances {
        let key = (instance.model.to_lowercase(), instance.skin);
        let model = cache.entry(key.clone()).or_insert_with(|| {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                read_model(vfs, &key.0, key.1)
            })) {
                Ok(Ok(s)) => {
                    report.unique_models += 1;
                    Some(s)
                }
                result => {
                    report.errors.push(format!(
                        "{} skin {}: {}",
                        key.0,
                        key.1,
                        match result {
                            Ok(Err(e)) => format!("{e:#}"),
                            _ => "model reader panicked".into(),
                        }
                    ));
                    None
                }
            }
        });
        let Some(model) = model else {
            continue;
        };
        world
            .model_assets
            .entry(instance.asset_key())
            .or_insert_with(|| model.clone());
        // Only static props with a proved identity-root transform use PHY yet.
        // Animated doors, dynamic props and ragdolls retain explicit fallback.
        if instance.solid
            && instance.kind == "static_prop"
            && !instance.background
            && instance.solid_mode != Some(6)
            && collision_modes_reported.insert((instance.asset_key(), instance.solid_mode))
        {
            report.collision_warnings.push(format!(
                "{}: static-prop solid mode {:?} is unsupported; using render collision fallback",
                instance.model, instance.solid_mode
            ));
        }
        if instance.solid
            && instance.kind == "static_prop"
            && !instance.background
            && instance.solid_mode == Some(6)
            && collision_attempted.insert(instance.asset_key())
        {
            match read_collision(vfs, &instance.model) {
                Ok(Some(pieces)) => {
                    report.collision_models_loaded += 1;
                    report.collision_pieces_loaded += pieces.len();
                    world.model_collision.insert(instance.asset_key(), pieces);
                }
                Ok(None) => report.collision_models_missing += 1,
                Err(error) => report.collision_warnings.push(format!(
                    "{}: {error:#}; using render collision fallback",
                    instance.model
                )),
            }
        }
        report.instances_loaded += 1;
        if instance.entity.is_some() && !world.rigs.contains_key(&instance.asset_key()) {
            let mut wanted = [
                "idle_subtle",
                "idle_baton",
                "walk_all",
                "run_all",
                "idle",
                "idle01",
                "fire",
                "reload",
                "draw",
                "swing",
                "attack",
                "death1",
            ]
            .into_iter()
            .map(String::from)
            .collect::<BTreeSet<_>>();
            for e in &world.entities {
                for key in [
                    "DefaultAnim",
                    "m_iszPlay",
                    "m_iszIdle",
                    "m_iszPostIdle",
                    "animation",
                ] {
                    if let Some(name) = e.get(key) {
                        wanted.insert(name.to_lowercase());
                    }
                }
            }
            match crate::animation::load(vfs, &instance.model, &wanted) {
                Ok(rig) => {
                    report.rigs_loaded += 1;
                    report.clips_loaded += rig.clips.len();
                    report
                        .animation_warnings
                        .extend(rig.warnings.iter().cloned());
                    world.rigs.insert(instance.asset_key(), rig);
                }
                Err(e) => report
                    .animation_warnings
                    .push(format!("{}: {e:#}", instance.model)),
            }
        }
        if instance.entity.is_some() {
            continue;
        }
        let rotate = rotation(instance.angles);
        for surface in model {
            let out = batches
                .entry((
                    surface.material.clone(),
                    surface.lightmap,
                    instance.background,
                ))
                .or_insert_with(|| Surface {
                    background: instance.background,
                    material: surface.material.clone(),
                    lightmap: surface.lightmap,
                    vertices: Vec::new(),
                    indices: Vec::new(),
                });
            let base = out.vertices.len() as u32;
            out.vertices.extend(surface.vertices.iter().map(|v| Vertex {
                position: instance.origin + rotate * (v.position * instance.scale),
                ..v.clone()
            }));
            out.indices.extend(surface.indices.iter().map(|i| base + i));
        }
    }
    world.surfaces = batches.into_values().collect();
    report
}
pub fn read_collision(vfs: &Vfs, model: &str) -> Result<Option<Vec<modkit_core::ConvexPiece>>> {
    let stem = model.trim_end_matches(".mdl");
    let Some(phy) = vfs.read(&format!("{stem}.phy"))? else {
        return Ok(None);
    };
    let mdl = vfs.read(model)?.context("PHY companion MDL missing")?;
    let checksum = crate::phy::identity_root_checksum(&mdl)?;
    Ok(Some(crate::phy::read(&phy, checksum)?.pieces))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_yaw_rotates_x_toward_y() {
        assert!((rotation(Vec3::new(0., 90., 0.)) * Vec3::X - Vec3::Y).length() < 0.0001);
    }
    #[test]
    fn truncated_static_dictionary_is_rejected() {
        assert!(parse_static(&[1, 0, 0, 0], 6).is_err());
    }
    #[test]
    fn static_v6_extracts_dictionary_origin_and_skin() {
        let mut d = vec![0u8; 4 + 128 + 4 + 4 + 64];
        d[0] = 1;
        d[4..4 + 15].copy_from_slice(b"models/test.mdl");
        d[136] = 1;
        d[140..144].copy_from_slice(&42f32.to_le_bytes());
        d[172] = 2;
        let props = parse_static(&d, 6).unwrap();
        assert_eq!(props[0].model, "models/test.mdl");
        assert_eq!(props[0].origin.x, 42.);
        assert_eq!(props[0].skin, 2);
    }
    #[test]
    fn static_prop_solid_modes_are_preserved_in_every_supported_version() {
        for (version, stride) in [(4, 56), (5, 60), (6, 64)] {
            let mut data = vec![0u8; 140 + 3 * stride];
            data[0] = 1;
            data[4..19].copy_from_slice(b"models/test.mdl");
            data[136] = 3;
            for (i, mode) in [0, 2, 6].into_iter().enumerate() {
                data[140 + i * stride + 30] = mode;
            }
            let props = parse_static(&data, version).unwrap();
            assert_eq!(
                props.iter().map(|p| p.solid_mode).collect::<Vec<_>>(),
                vec![Some(0), Some(2), Some(6)]
            );
            assert_eq!(
                props.iter().map(|p| p.solid).collect::<Vec<_>>(),
                vec![false, true, true]
            );
        }
    }
}
