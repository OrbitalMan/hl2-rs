//! Baked-lightmap material, independent of Bevy's PBR lighting model.
use crate::{
    Status,
    assets::{LoadedMap, MaterialData, visible_entity},
    source_to_bevy,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::{Aabb, MeshAabb},
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
    pub(crate) tint: Vec4,
    // x = alpha cutoff, y = ignore base alpha, z = additive output.
    #[uniform(3)]
    pub(crate) parameters: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub(crate) base: Handle<Image>,
    #[texture(4)]
    #[sampler(5)]
    pub(crate) lightmap: Handle<Image>,
    #[texture(6)]
    #[sampler(7)]
    pub(crate) iris: Handle<Image>,
    #[uniform(8)]
    pub(crate) secondary_uv: Mat3,
    pub(crate) alpha: AlphaMode,
    pub(crate) two_sided: bool,
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
pub(crate) fn image(width: u16, height: u16, rgba: Vec<u8>, repeat: bool) -> Image {
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
    skin: Vec<(glam::Vec3, Option<modkit_core::animation::Weights>)>,
}
impl Batch {
    fn append(&mut self, surface: &Surface, transform: Mat4, material: Option<&MaterialData>) {
        let offset = self.positions.len() as u32;
        let rows = material
            .map(|m| m.uv_transform)
            .unwrap_or([[1., 0., 0.], [0., 1., 0.]]);
        for vertex in &surface.vertices {
            self.skin.push((vertex.position, vertex.skin.clone()));
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
            if self.skin.iter().any(|(_, weights)| weights.is_some()) {
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD
            } else {
                RenderAssetUsages::RENDER_WORLD
            },
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
#[derive(Component)]
pub struct SourceEntity(pub usize);
#[derive(Component)]
pub struct EyeMesh {
    pub key: String,
    pub eye: source_assets::eyes::Eyeball,
    pub scale: f32,
}
fn sample_key(clip: &modkit_core::animation::Clip, time: f32) -> u32 {
    if clip.frames.len() <= 1 {
        0
    } else if !clip.looping && time * clip.fps >= (clip.frames.len() - 1) as f32 {
        u32::MAX
    } else {
        time.to_bits()
    }
}
#[derive(Component)]
pub struct AnimatedMesh {
    sampled: Option<(String, u32)>,
    gpu: Option<crate::gpu_skinning::Skeleton>,
    entity: Option<usize>,
    weapon: Option<String>,
    key: String,
    scale: f32,
    bind: Vec<(glam::Vec3, Option<modkit_core::animation::Weights>)>,
}
/// Conjugate rotation by the same basis change used for vertex positions.
pub(crate) fn entity_transform(origin: glam::Vec3, rotation: glam::Quat) -> Transform {
    let basis = Mat3::from_cols(
        source_to_bevy(Vec3::X),
        source_to_bevy(Vec3::Y),
        source_to_bevy(Vec3::Z),
    );
    Transform::from_translation(source_to_bevy(Vec3::from_array(origin.to_array()))).with_rotation(
        Quat::from_mat3(
            &(basis * Mat3::from_quat(Quat::from_array(rotation.to_array())) * basis.transpose()),
        ),
    )
}
pub fn present_entities(
    game: Res<crate::gameplay::Gameplay>,
    sim: Res<crate::movement::Simulation>,
    mut entities: Query<
        (&SourceEntity, &mut Transform, &mut Visibility),
        Without<crate::gpu_skinning::Joint>,
    >,
    mut joints: Query<&mut Transform, (With<crate::gpu_skinning::Joint>, Without<SourceEntity>)>,
    mut animations: Query<(&mut AnimatedMesh, &Mesh3d, Option<&mut Aabb>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    performance: Option<Res<crate::performance::Performance>>,
) {
    let _timing = crate::performance::scope(performance.as_deref(), "animation");
    for (owner, mut transform, mut visibility) in &mut entities {
        let state = &game.scene.states[owner.0];
        let (origin, rotation) = sim
            .physics
            .entity_pose(owner.0)
            .unwrap_or((state.origin, state.rotation));
        let pose = entity_transform(origin, rotation);
        if *transform != pose {
            *transform = pose;
        }
        let visible = if state.visible && !state.killed {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != visible {
            *visibility = visible;
        }
    }
    let mut updated_joints = std::collections::BTreeSet::new();
    let mut poses: BTreeMap<(String, String, u32), Vec<glam::Mat4>> = BTreeMap::new();
    for (mut animation, handle, aabb) in &mut animations {
        let Some(rig) = game.world.rigs.get(&animation.key) else {
            continue;
        };
        let (clip, time) = if let Some(id) = animation.entity {
            let state = &game.scene.states[id];
            if !state.visible || state.killed {
                continue;
            }
            (state.animation.as_str(), game.scene.animation_time(id))
        } else {
            if animation.weapon.as_deref() != Some(game.inventory.active.as_str()) {
                continue;
            }
            let elapsed = (game.scene.time - game.inventory.animation_at).max(0.) as f32;
            if rig
                .clips
                .get(&game.inventory.animation)
                .is_some_and(|c| elapsed < c.duration())
            {
                (game.inventory.animation.as_str(), elapsed)
            } else {
                (game.inventory.idle_animation(), game.scene.time as f32)
            }
        };
        let Some(definition) = rig.clips.get(clip) else {
            continue;
        };
        let key = sample_key(definition, time);
        if animation
            .sampled
            .as_ref()
            .is_some_and(|(name, previous)| name == clip && *previous == key)
        {
            continue;
        }
        let sampled = (clip.to_owned(), key);
        let matrices = poses
            .entry((animation.key.clone(), clip.to_owned(), key))
            .or_insert_with(|| rig.matrices(clip, time));
        if let Some(skeleton) = &animation.gpu {
            if updated_joints.insert(skeleton.joints[0]) {
                for ((joint, matrix), bone) in
                    skeleton.joints.iter().zip(matrices.iter()).zip(&rig.bones)
                {
                    if let Ok(mut transform) = joints.get_mut(*joint) {
                        let pose = Transform::from_matrix(crate::gpu_skinning::convert(
                            *matrix * bone.inverse_bind.inverse(),
                            animation.scale,
                        ));
                        if *transform != pose {
                            *transform = pose;
                        }
                    }
                }
            }
            animation.sampled = Some(sampled);
            continue;
        }
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            let positions: Vec<_> = animation
                .bind
                .iter()
                .map(|(bind, weights)| {
                    let p = weights
                        .as_ref()
                        .map_or(*bind, |w| modkit_core::animation::skin(*bind, w, matrices));
                    source_to_bevy(Vec3::from_array((p * animation.scale).to_array())).to_array()
                })
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            if let (Some(mut aabb), Some(bounds)) = (aabb, mesh.compute_aabb()) {
                *aabb = bounds;
            }
            animation.sampled = Some(sampled);
        }
    }
}
#[derive(Component)]
pub struct WeaponMesh(String);
pub fn present_weapons(
    game: Res<crate::gameplay::Gameplay>,
    mut draws: Query<(&WeaponMesh, &mut Visibility)>,
) {
    for (weapon, mut visibility) in &mut draws {
        *visibility = if weapon.0 == game.inventory.active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    World,
    SkyWorld,
    Entity(usize),
    Weapon(String),
}
impl Owner {
    fn entity(&self) -> Option<usize> {
        if let Self::Entity(id) = self {
            Some(*id)
        } else {
            None
        }
    }
}

pub fn spawn_map(
    loaded: &LoadedMap,
    commands: &mut Commands,
    (meshes, inverse_binds): (
        &mut Assets<Mesh>,
        &mut Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
    ),
    materials: &mut Assets<SourceMaterial>,
    images: &mut Assets<Image>,
    status: &Status,
    (camera_target, cpu_skinning): (&Handle<Image>, bool),
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
            let handle = if material.camera {
                camera_target.clone()
            } else {
                match (&material.base, &material.base_path) {
                    (Some(base), Some(path)) => texture_handles
                        .entry(path.clone())
                        .or_insert_with(|| {
                            images.add(image(base.width, base.height, base.rgba.clone(), true))
                        })
                        .clone(),
                    _ => missing.clone(),
                }
            };
            (name.clone(), handle)
        })
        .collect();
    let irises: BTreeMap<_, _> = loaded
        .materials
        .iter()
        .filter_map(|(name, m)| {
            m.iris
                .as_ref()
                .zip(m.iris_path.as_ref())
                .map(|(image, path)| {
                    let handle = texture_handles
                        .entry(path.clone())
                        .or_insert_with(|| {
                            images.add(self::image(
                                image.width,
                                image.height,
                                image.rgba.clone(),
                                false,
                            ))
                        })
                        .clone();
                    (name.clone(), handle)
                })
        })
        .collect();
    let overlays: BTreeMap<_, _> = loaded
        .materials
        .iter()
        .filter_map(|(name, m)| {
            m.camera_overlay
                .as_ref()
                .zip(m.camera_overlay_path.as_ref())
                .map(|(overlay, path)| {
                    let handle = texture_handles
                        .entry(path.clone())
                        .or_insert_with(|| {
                            images.add(image(
                                overlay.width,
                                overlay.height,
                                overlay.rgba.clone(),
                                true,
                            ))
                        })
                        .clone();
                    (name.clone(), handle)
                })
        })
        .collect();
    let lightmaps: Vec<_> = world
        .lightmaps
        .iter()
        .map(|lm| images.add(image(lm.width, lm.height, lm.rgba.clone(), false)))
        .collect();
    let mut batches: BTreeMap<(Owner, String, Option<usize>, usize), Batch> = BTreeMap::new();
    let skipped = 0usize;
    let mut transparent_id = 0usize;
    let mut append = |surface: &Surface, transform: Mat4, owner: Owner| {
        let owner = if surface.background && owner == Owner::World {
            Owner::SkyWorld
        } else {
            owner
        };
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
            .entry((owner, surface.material.clone(), surface.lightmap, draw_id))
            .or_default()
            .append(surface, transform, loaded.materials.get(&surface.material));
    };
    // BSP::world includes displacements here; terrain is the collision copy.
    for surface in &world.surfaces {
        append(surface, Mat4::IDENTITY, Owner::World);
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
            for surface in &brush.surfaces {
                append(surface, Mat4::IDENTITY, Owner::Entity(id));
            }
        }
    }
    for instance in &world.model_instances {
        // append_models already baked entity=None static props into surfaces.
        if instance.entity.is_none_or(|id| !visible_entity(world, id)) {
            continue;
        }
        if let Some(surfaces) = world.model_assets.get(&instance.asset_key()) {
            let transform = Mat4::from_scale(Vec3::splat(instance.scale));
            for surface in surfaces {
                append(surface, transform, Owner::Entity(instance.entity.unwrap()));
            }
        }
    }
    for (name, weapon) in &loaded.gameplay.weapons {
        if let Some(surfaces) = world
            .model_assets
            .get(&format!("{}#0", weapon.viewmodel.to_lowercase()))
        {
            for surface in surfaces {
                append(surface, Mat4::IDENTITY, Owner::Weapon(name.clone()));
            }
        }
    }
    let mut stats = status.0.lock().expect("status lock");
    stats.skipped_background_surfaces = skipped;
    stats.materials = batches.len();
    let mut skeletons = BTreeMap::<Owner, crate::gpu_skinning::Skeleton>::new();
    for ((owner, name, lm, _), mut batch) in batches {
        let fallback = MaterialData::default();
        let definition = loaded.materials.get(&name).unwrap_or(&fallback);
        let material = materials.add(make_material(
            definition,
            bases.get(&name).unwrap_or(&missing).clone(),
            lm.and_then(|index| lightmaps.get(index))
                .unwrap_or(&white)
                .clone(),
            irises.get(&name).or_else(|| overlays.get(&name)).cloned(),
            &white,
        ));
        stats.meshes += 1;
        stats.triangles += batch.indices.len() / 3;
        let mut animation = if batch.skin.iter().any(|(_, w)| w.is_some()) {
            match &owner {
                Owner::Entity(id) => world
                    .model_instances
                    .iter()
                    .find(|i| i.entity == Some(*id))
                    .map(|instance| AnimatedMesh {
                        sampled: None,
                        gpu: None,
                        entity: Some(*id),
                        weapon: None,
                        key: instance.asset_key(),
                        scale: instance.scale,
                        bind: std::mem::take(&mut batch.skin),
                    }),
                Owner::Weapon(name) => Some(AnimatedMesh {
                    sampled: None,
                    gpu: None,
                    entity: None,
                    weapon: Some(name.clone()),
                    key: format!(
                        "{}#0",
                        loaded.gameplay.weapons[name].viewmodel.to_lowercase()
                    ),
                    scale: 1.,
                    bind: std::mem::take(&mut batch.skin),
                }),
                Owner::World | Owner::SkyWorld => None,
            }
        } else {
            None
        };
        if !cpu_skinning
            && let Some(animation) = &mut animation
            && let Some(rig) = world
                .rigs
                .get(&animation.key)
                .filter(|rig| !rig.bones.is_empty() && rig.bones.len() < 256)
        {
            let skeleton = skeletons.entry(owner.clone()).or_insert_with(|| {
                let mut root = commands.spawn((
                    crate::campaign::MapOwned,
                    Transform::IDENTITY,
                    Visibility::Inherited,
                ));
                match &owner {
                    Owner::Entity(id) => {
                        root.insert(SourceEntity(*id));
                    }
                    Owner::Weapon(name) => {
                        root.insert(WeaponMesh(name.clone()));
                    }
                    _ => unreachable!("only owned model meshes animate"),
                }
                let root = root.id();
                crate::gpu_skinning::Skeleton::spawn(
                    commands,
                    inverse_binds,
                    rig,
                    animation.scale,
                    root,
                )
            });
            animation.gpu = Some(skeleton.clone());
        }
        // CPU assets retain bounds and eye projection data; GPU skinning keeps their positions immutable.
        let animated = animation.is_some() || irises.contains_key(&name);
        let mut mesh = batch.mesh();
        if let Some(animation) = &animation
            && let Some(skeleton) = &animation.gpu
        {
            skeleton.attributes(&mut mesh, &animation.bind);
        }
        if animated {
            mesh.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
        }
        let transform = owner.entity().map_or(Transform::IDENTITY, |id| {
            let state = &loaded.gameplay.scene.states[id];
            entity_transform(state.origin, state.rotation)
        });
        let mut draw = commands.spawn((
            crate::campaign::MapOwned,
            Name::new(name.clone()),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            transform,
        ));
        if matches!(owner, Owner::SkyWorld)
            || owner
                .entity()
                .is_some_and(|id| world.background_entities.contains(&id))
        {
            draw.insert(bevy::camera::visibility::RenderLayers::layer(4));
        }
        if definition.camera {
            draw.insert(crate::monitors::MonitorMaterial {
                animation: definition.camera_animation.clone(),
                color: definition.tint,
                color2: definition.camera_color2,
            });
            // Avoid sampling a render attachment while drawing into that same attachment.
            draw.insert(bevy::camera::visibility::RenderLayers::layer(5));
        }
        if let Some(id) = owner.entity() {
            draw.insert(SourceEntity(id));
            if let Some(instance) = world.model_instances.iter().find(|i| i.entity == Some(id))
                && let Some(eye) = loaded.eyes.get(&(instance.asset_key(), name.clone()))
            {
                draw.insert(EyeMesh {
                    key: instance.asset_key(),
                    eye: eye.clone(),
                    scale: instance.scale,
                });
            }
            let state = &loaded.gameplay.scene.states[id];
            if !state.visible || state.killed {
                draw.insert(Visibility::Hidden);
            }
        }
        if let Owner::Weapon(name) = owner {
            draw.insert((
                WeaponMesh(name),
                bevy::camera::visibility::RenderLayers::layer(1),
                Visibility::Hidden,
            ));
        }
        if let Some(animation) = animation {
            if let Some(skeleton) = &animation.gpu {
                stats.gpu_skinned_meshes += 1;
                draw.insert((
                    skeleton.component(),
                    bevy::camera::visibility::DynamicSkinnedMeshBounds,
                ));
            }
            if animation.gpu.is_none() {
                stats.cpu_skinned_meshes += 1;
            }
            draw.insert(animation);
        }
    }
    stats.skin_joints = skeletons.values().map(|s| s.joints.len()).sum();
}
pub(crate) fn mesh_from_surface(surface: &Surface, definition: &MaterialData) -> Mesh {
    let mut batch = Batch::default();
    batch.append(surface, Mat4::IDENTITY, Some(definition));
    batch.mesh()
}
pub(crate) fn make_material(
    definition: &MaterialData,
    base: Handle<Image>,
    lightmap: Handle<Image>,
    iris: Option<Handle<Image>>,
    white: &Handle<Image>,
) -> SourceMaterial {
    let alpha = alpha_mode(definition);
    SourceMaterial {
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
            if definition.camera {
                if definition.camera_vertex_color {
                    -2.
                } else {
                    -1.
                }
            } else {
                f32::from(iris.is_some())
            },
        ),
        iris: iris.unwrap_or_else(|| white.clone()),
        secondary_uv: Mat3::IDENTITY,
        base,
        lightmap: if definition.unlit {
            white.clone()
        } else {
            lightmap
        },
        alpha,
        two_sided: definition.two_sided,
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
    fn animation_keys_keep_live_samples_and_stop_reuploading_constant_poses() {
        use modkit_core::animation::Clip;
        let mut clip = Clip {
            fps: 30.,
            looping: false,
            frames: vec![vec![]; 31],
            events: vec![],
        };
        assert_ne!(sample_key(&clip, 0.2), sample_key(&clip, 0.3));
        assert_eq!(sample_key(&clip, 1.), sample_key(&clip, 50.));
        clip.looping = true;
        assert_ne!(sample_key(&clip, 1.), sample_key(&clip, 50.));
        clip.frames.truncate(1);
        assert_eq!(sample_key(&clip, 0.), sample_key(&clip, 50.));
    }
    #[test]
    fn entity_pose_conversion_preserves_scaled_local_points_at_arbitrary_angles() {
        let origin = glam::Vec3::new(10., -300., 45.);
        let rotation = hl2_simulation::physics::angles(glam::Vec3::new(17., 90., -12.));
        let local = glam::Vec3::new(40., 4., 16.) * 1.3;
        let converted = entity_transform(origin, rotation)
            .transform_point(source_to_bevy(Vec3::from_array(local.to_array())));
        let expected = source_to_bevy(Vec3::from_array((origin + rotation * local).to_array()));
        assert!(converted.distance(expected) < 0.0001);
    }
    #[test]
    fn animation_system_updates_mesh_bounds_pose_and_scripted_visibility() {
        use bevy::ecs::system::RunSystemOnce;
        use modkit_core::animation::{Bone, Clip, Pose, Rig, Weights};
        let key = "synthetic#0".to_owned();
        let world = modkit_core::World {
            entities: vec![modkit_core::Entity {
                properties: vec![
                    ("classname".into(), "prop_dynamic".into()),
                    ("DefaultAnim".into(), "move".into()),
                    ("origin".into(), "100 50 10".into()),
                    ("angles".into(), "0 90 0".into()),
                ],
            }],
            rigs: BTreeMap::from([(
                key.clone(),
                Rig {
                    bones: vec![Bone {
                        name: "root".into(),
                        parent: None,
                        bind: Pose {
                            position: glam::Vec3::ZERO,
                            rotation: glam::Quat::IDENTITY,
                        },
                        inverse_bind: glam::Mat4::IDENTITY,
                    }],
                    clips: BTreeMap::from([(
                        "move".into(),
                        Clip {
                            fps: 1.,
                            looping: false,
                            events: vec![],
                            frames: vec![
                                vec![Pose {
                                    position: glam::Vec3::ZERO,
                                    rotation: glam::Quat::IDENTITY,
                                }],
                                vec![Pose {
                                    position: glam::Vec3::Z * 8.,
                                    rotation: glam::Quat::IDENTITY,
                                }],
                            ],
                        },
                    )]),
                    warnings: vec![],
                },
            )]),
            ..default()
        };
        let mut game = crate::gameplay::Gameplay::synthetic(world);
        game.scene.time = 0.5;
        let sim = crate::movement::Simulation::new(
            &game.world,
            glam::Vec3::Z * 128.,
            0.,
            0.,
            false,
            vec![],
        );
        let mut meshes = Assets::<Mesh>::default();
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[1., 0., 0.]]);
        let handle = meshes.add(mesh);
        let mut app = App::new();
        app.insert_resource(game)
            .insert_resource(sim)
            .insert_resource(meshes);
        let entity = app
            .world_mut()
            .spawn((
                crate::campaign::MapOwned,
                SourceEntity(0),
                Transform::IDENTITY,
                Visibility::Inherited,
                AnimatedMesh {
                    sampled: None,
                    gpu: None,
                    entity: Some(0),
                    weapon: None,
                    key,
                    scale: 2.,
                    bind: vec![(
                        glam::Vec3::X,
                        Some(Weights {
                            bones: [0; 3],
                            weights: [1., 0., 0.],
                        }),
                    )],
                },
                Mesh3d(handle.clone()),
                Aabb::default(),
            ))
            .id();
        app.world_mut().run_system_once(present_entities).unwrap();
        let meshes = app.world().resource::<Assets<Mesh>>();
        let mesh = meshes.get(&handle).unwrap();
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap(),
            &[[2., 8., 0.]]
        );
        assert!(
            Vec3::from(app.world().get::<Aabb>(entity).unwrap().center)
                .distance(Vec3::new(2., 8., 0.))
                < 0.0001
        );
        let point = app
            .world()
            .get::<Transform>(entity)
            .unwrap()
            .transform_point(Vec3::new(2., 8., 0.));
        assert!(point.distance(source_to_bevy(Vec3::new(100., 52., 18.))) < 0.0001);
        app.world_mut()
            .resource_mut::<crate::gameplay::Gameplay>()
            .scene
            .states[0]
            .visible = false;
        app.world_mut().run_system_once(present_entities).unwrap();
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
    }
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
