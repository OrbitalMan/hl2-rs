//! Baked-lightmap material, independent of Bevy's PBR lighting model.
use crate::{
    Status,
    assets::{LoadedMap, MaterialData, visible_entity},
    source_to_bevy,
};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, Extent3d, FrontFace, PrimitiveTopology, RenderPipelineDescriptor,
        SpecializedMeshPipelineError, TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
};
use modkit_core::Surface;
use std::collections::BTreeMap;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(MaterialKey)]
pub struct SourceMaterial {
    #[uniform(0)]
    tint: Vec4,
    // x = alpha cutoff, y = ignore base alpha, z = additive output.
    #[uniform(3)]
    parameters: Vec4,
    #[texture(1)]
    #[sampler(2)]
    base: Handle<Image>,
    #[texture(4)]
    #[sampler(5)]
    lightmap: Handle<Image>,
    alpha: AlphaMode,
    two_sided: bool,
}
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct MaterialKey {
    two_sided: bool,
}
impl From<&SourceMaterial> for MaterialKey {
    fn from(material: &SourceMaterial) -> Self {
        Self {
            two_sided: material.two_sided,
        }
    }
}
impl Material for SourceMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/source.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        self.alpha
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // BSP render triangles are normalized before model append; vmdl already
        // supplies CCW model triangles. Both now use Bevy's CCW front face.
        descriptor.primitive.front_face = FrontFace::Ccw;
        if key.bind_group_data.two_sided {
            descriptor.primitive.cull_mode = None;
        }
        Ok(())
    }
}
fn image(width: u16, height: u16, rgba: Vec<u8>, repeat: bool) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: width.into(),
            height: height.into(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: if repeat {
            ImageAddressMode::Repeat
        } else {
            ImageAddressMode::ClampToEdge
        },
        address_mode_v: if repeat {
            ImageAddressMode::Repeat
        } else {
            ImageAddressMode::ClampToEdge
        },
        ..ImageSamplerDescriptor::linear()
    });
    image
}
#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    light_uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}
impl Batch {
    fn append(&mut self, surface: &Surface, transform: Mat4, material: Option<&MaterialData>) {
        let offset = self.positions.len() as u32;
        let rows = material
            .map(|m| m.uv_transform)
            .unwrap_or([[1., 0., 0.], [0., 1., 0.]]);
        for vertex in &surface.vertices {
            self.positions.push(
                source_to_bevy(
                    transform.transform_point3(Vec3::from_array(vertex.position.to_array())),
                )
                .to_array(),
            );
            let uv = vertex.uv;
            self.uvs.push([
                rows[0][0] * uv.x + rows[0][1] * uv.y + rows[0][2],
                rows[1][0] * uv.x + rows[1][1] * uv.y + rows[1][2],
            ]);
            self.light_uvs.push(vertex.light_uv.to_array());
            self.colors.push(vertex.color.map(|v| f32::from(v) / 255.));
        }
        self.indices
            .extend(surface.indices.iter().map(|i| i + offset));
    }
    fn mesh(self) -> Mesh {
        let count = self.positions.len();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; count]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, self.light_uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}
fn source_transform(origin: Vec3, angles: Vec3, scale: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(scale),
        Quat::from_rotation_z(angles.y.to_radians())
            * Quat::from_rotation_y(angles.x.to_radians())
            * Quat::from_rotation_x(angles.z.to_radians()),
        origin,
    )
}
pub fn spawn_map(
    loaded: &LoadedMap,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SourceMaterial>,
    images: &mut Assets<Image>,
    status: &Status,
) {
    let world = &loaded.world;
    let white = images.add(image(1, 1, vec![255; 4], false));
    let missing = images.add(image(
        2,
        2,
        vec![
            255, 0, 255, 255, 30, 30, 30, 255, 30, 30, 30, 255, 255, 0, 255, 255,
        ],
        true,
    ));
    let mut texture_handles: BTreeMap<String, Handle<Image>> = BTreeMap::new();
    let bases: BTreeMap<_, _> = loaded
        .materials
        .iter()
        .map(|(name, material)| {
            let handle = match (&material.base, &material.base_path) {
                (Some(base), Some(path)) => texture_handles
                    .entry(path.clone())
                    .or_insert_with(|| {
                        images.add(image(base.width, base.height, base.rgba.clone(), true))
                    })
                    .clone(),
                _ => missing.clone(),
            };
            (name.clone(), handle)
        })
        .collect();
    let lightmaps: Vec<_> = world
        .lightmaps
        .iter()
        .map(|lm| images.add(image(lm.width, lm.height, lm.rgba.clone(), false)))
        .collect();
    let mut batches: BTreeMap<(String, Option<usize>, usize), Batch> = BTreeMap::new();
    let mut skipped = 0usize;
    let mut transparent_id = 0usize;
    let mut append = |surface: &Surface, transform: Mat4| {
        if surface.background {
            skipped += 1;
            return;
        }
        if surface.indices.is_empty() {
            return;
        }
        let definition = loaded.materials.get(&surface.material);
        let draw_id = if definition
            .is_some_and(|m| matches!(alpha_mode(m), AlphaMode::Blend | AlphaMode::Add))
        {
            transparent_id += 1;
            transparent_id
        } else {
            0
        };
        batches
            .entry((surface.material.clone(), surface.lightmap, draw_id))
            .or_default()
            .append(surface, transform, loaded.materials.get(&surface.material));
    };
    // BSP::world includes displacements here; terrain is the collision copy.
    for surface in &world.surfaces {
        append(surface, Mat4::IDENTITY);
    }
    for (id, entity) in world.entities.iter().enumerate() {
        if !visible_entity(world, id) {
            continue;
        }
        let brush = entity
            .get("model")
            .and_then(|s| s.strip_prefix('*'))
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|id| world.brush_models.iter().find(|model| model.id == id));
        if let Some(brush) = brush {
            let angles = modkit_core::parse_vec3(entity.get("angles").unwrap_or("0 0 0"))
                .map(|p| Vec3::from_array(p.to_array()))
                .unwrap_or_default();
            let transform =
                source_transform(Vec3::from_array(entity.origin().to_array()), angles, 1.);
            for surface in &brush.surfaces {
                append(surface, transform);
            }
        }
    }
    for instance in &world.model_instances {
        // append_models already baked entity=None static props into surfaces.
        if instance.background || instance.entity.is_none_or(|id| !visible_entity(world, id)) {
            continue;
        }
        if let Some(surfaces) = world.model_assets.get(&instance.asset_key()) {
            let transform = source_transform(
                Vec3::from_array(instance.origin.to_array()),
                Vec3::from_array(instance.angles.to_array()),
                instance.scale,
            );
            for surface in surfaces {
                append(surface, transform);
            }
        }
    }
    let mut stats = status.0.lock().expect("status lock");
    stats.skipped_background_surfaces = skipped;
    stats.materials = batches.len();
    for ((name, lm, _), batch) in batches {
        let fallback = MaterialData::default();
        let definition = loaded.materials.get(&name).unwrap_or(&fallback);
        let alpha = alpha_mode(definition);
        let material = materials.add(SourceMaterial {
            tint: Vec4::new(
                definition.tint[0],
                definition.tint[1],
                definition.tint[2],
                definition.opacity,
            ),
            parameters: Vec4::new(
                definition
                    .alpha_cutoff
                    .unwrap_or(if matches!(alpha, AlphaMode::Opaque) {
                        0.
                    } else {
                        0.001
                    }),
                f32::from(matches!(alpha, AlphaMode::Opaque)),
                f32::from(matches!(alpha, AlphaMode::Add)),
                0.,
            ),
            base: bases.get(&name).unwrap_or(&missing).clone(),
            lightmap: if definition.unlit {
                white.clone()
            } else {
                lm.and_then(|index| lightmaps.get(index))
                    .unwrap_or(&white)
                    .clone()
            },
            alpha,
            two_sided: definition.two_sided,
        });
        stats.meshes += 1;
        stats.triangles += batch.indices.len() / 3;
        commands.spawn((
            Name::new(name),
            Mesh3d(meshes.add(batch.mesh())),
            MeshMaterial3d(material),
            Transform::IDENTITY,
        ));
    }
}
fn alpha_mode(material: &MaterialData) -> AlphaMode {
    if material.additive {
        AlphaMode::Add
    } else if let Some(cutoff) = material.alpha_cutoff {
        AlphaMode::Mask(cutoff)
    } else if material.translucent || material.opacity < 1. {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_test_takes_precedence_over_translucency_and_additive_over_both() {
        let mut material = MaterialData {
            alpha_cutoff: Some(0.5),
            translucent: true,
            ..default()
        };
        assert_eq!(alpha_mode(&material), AlphaMode::Mask(0.5));
        material.additive = true;
        assert_eq!(alpha_mode(&material), AlphaMode::Add);
    }
}
