//! Bevy presentation of the shared ordered Source HUD canvas.
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::RenderLayers,
    },
    image::{ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, MeshVertexBufferLayoutRef},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d,
        PrimitiveTopology, RenderPipelineDescriptor, SpecializedMeshPipelineError,
        TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey},
    window::PrimaryWindow,
};
use hl2_ui::canvas::{FilterMode, Quad, Texture2D};
use std::collections::HashMap;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(HudMaterialKey)]
pub struct HudMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
    additive: bool,
}
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct HudMaterialKey {
    additive: bool,
}
impl From<&HudMaterial> for HudMaterialKey {
    fn from(material: &HudMaterial) -> Self {
        Self {
            additive: material.additive,
        }
    }
}
impl Material2d for HudMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/hud.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.additive {
            for target in descriptor
                .fragment
                .as_mut()
                .into_iter()
                .flat_map(|f| &mut f.targets)
                .flatten()
            {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::SrcAlpha,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent::OVER,
                });
            }
        }
        Ok(())
    }
}
struct GpuTexture {
    // Retain CPU ownership so pointer identities cannot be reused for different glyphs.
    _cpu: Texture2D,
    normal: Handle<HudMaterial>,
    additive: Handle<HudMaterial>,
}
#[derive(Resource)]
pub struct Hud {
    pub source: hl2_ui::hud::WeaponHud,
    textures: HashMap<usize, GpuTexture>,
    white: Option<(Handle<HudMaterial>, Handle<HudMaterial>)>,
    pool: Vec<(Entity, Handle<Mesh>)>,
    pub quads: usize,
}
impl Hud {
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"quads":self.quads,"pooled_meshes":self.pool.len(),"owned_textures":self.textures.len(),"viewport":[self.source.canvas.width(),self.source.canvas.height()]})
    }
    pub fn new(source: hl2_ui::hud::WeaponHud) -> Self {
        Self {
            source,
            textures: HashMap::new(),
            white: None,
            pool: vec![],
            quads: 0,
        }
    }
    fn material(
        &mut self,
        quad: &Quad,
        images: &mut Assets<Image>,
        materials: &mut Assets<HudMaterial>,
    ) -> Handle<HudMaterial> {
        fn upload(
            texture: &Texture2D,
            images: &mut Assets<Image>,
            materials: &mut Assets<HudMaterial>,
        ) -> (Handle<HudMaterial>, Handle<HudMaterial>) {
            let mut image = Image::new(
                Extent3d {
                    width: u32::from(texture.0.width),
                    height: u32::from(texture.0.height),
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                texture.0.rgba.clone(),
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::RENDER_WORLD,
            );
            image.sampler = ImageSampler::Descriptor(match texture.filter() {
                FilterMode::Nearest => ImageSamplerDescriptor::nearest(),
                FilterMode::Linear => ImageSamplerDescriptor::linear(),
            });
            let handle = images.add(image);
            (
                materials.add(HudMaterial {
                    texture: handle.clone(),
                    additive: false,
                }),
                materials.add(HudMaterial {
                    texture: handle,
                    additive: true,
                }),
            )
        }
        let handles = if let Some(cpu) = &quad.texture {
            let texture = self.textures.entry(cpu.id()).or_insert_with(|| {
                let (normal, additive) = upload(cpu, images, materials);
                GpuTexture {
                    _cpu: cpu.clone(),
                    normal,
                    additive,
                }
            });
            (&texture.normal, &texture.additive)
        } else {
            let handles = self.white.get_or_insert_with(|| {
                upload(&Texture2D::from_rgba8(1, 1, &[255; 4]), images, materials)
            });
            (&handles.0, &handles.1)
        };
        if quad.additive {
            handles.1.clone()
        } else {
            handles.0.clone()
        }
    }
}
#[derive(Component)]
pub struct HudDraw;
type DrawQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut MeshMaterial2d<HudMaterial>,
        &'static mut Transform,
        &'static mut Visibility,
        Option<&'static mut Aabb>,
    ),
    With<HudDraw>,
>;
type HudAssets<'w> = (
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<HudMaterial>>,
    ResMut<'w, Assets<Mesh>>,
);
pub fn present(
    mut commands: Commands,
    mut hud: ResMut<Hud>,
    mut game: ResMut<crate::gameplay::Gameplay>,
    windows: Query<&Window, With<PrimaryWindow>>,
    (mut images, mut materials, mut meshes): HudAssets,
    mut draws: DrawQuery,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.width();
    let height = window.height();
    hud.source.canvas.resize(width, height);
    hud.source.draw_status(
        &game.inventory,
        &game.weapons,
        &game.selection,
        game.scene.time,
    );
    hud.source.draw_selection(
        &game.selection,
        &game.inventory,
        &game.weapons,
        game.scene.time,
    );
    hud.source.draw_crosshair(&game.inventory);
    game.scene
        .sounds
        .extend(hud.source.drain_sounds().into_iter().map(Into::into));
    let quads = hud.source.canvas.drain();
    hud.quads = quads.len();
    for (i, quad) in quads.iter().enumerate() {
        let material = hud.material(quad, &mut images, &mut materials);
        let mesh = quad_mesh(quad, Vec2::new(width, height));
        if let Some((entity, handle)) = hud.pool.get(i) {
            if let Some(mut old) = meshes.get_mut(handle) {
                *old = mesh;
            }
            if let Ok((mut old_material, mut transform, mut visibility, aabb)) =
                draws.get_mut(*entity)
            {
                old_material.0 = material;
                transform.translation.z = i as f32 * 0.01;
                *visibility = Visibility::Inherited;
                if let (Some(mut aabb), Some(mesh)) = (aabb, meshes.get(handle))
                    && let Some(bounds) = mesh.compute_aabb()
                {
                    *aabb = bounds;
                }
            }
        } else {
            let mesh = meshes.add(mesh);
            let entity = commands
                .spawn((
                    HudDraw,
                    Mesh2d(mesh.clone()),
                    MeshMaterial2d(material),
                    Transform::from_xyz(0., 0., i as f32 * 0.01),
                    RenderLayers::layer(2),
                ))
                .id();
            hud.pool.push((entity, mesh));
        }
    }
    for (entity, _) in hud.pool.iter().skip(quads.len()) {
        if let Ok((_, _, mut visibility, _)) = draws.get_mut(*entity) {
            *visibility = Visibility::Hidden;
        }
    }
}
fn quad_mesh(quad: &Quad, viewport: Vec2) -> Mesh {
    let r = quad.destination;
    let positions = [
        (r.x, r.y),
        (r.x, r.y + r.h),
        (r.x + r.w, r.y + r.h),
        (r.x + r.w, r.y),
    ]
    .map(|(x, y)| [x - viewport.x * 0.5, viewport.y * 0.5 - y, 0.]);
    let (tw, th) = quad
        .texture
        .as_ref()
        .map_or((1., 1.), |t| (t.width(), t.height()));
    let s = quad.source;
    let uvs = [
        (s.x, s.y),
        (s.x, s.y + s.h),
        (s.x + s.w, s.y + s.h),
        (s.x + s.w, s.y),
    ]
    .map(|(x, y)| [x / tw, y / th]);
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![quad.color.to_array(); 4]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use hl2_ui::canvas::{Color, Rect};
    #[test]
    fn atlas_crop_remains_pixel_aligned_in_camera_coordinates() {
        let quad = Quad {
            texture: Some(Texture2D::from_rgba8(128, 128, &vec![255; 128 * 128 * 4])),
            source: Rect::new(0.5, 48.5, 23., 23.),
            destination: Rect::new(948., 528., 24., 24.),
            color: Color::from_rgba(255, 255, 255, 255),
            additive: true,
        };
        let mesh = quad_mesh(&quad, Vec2::new(1920., 1080.));
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("HUD positions");
        };
        assert_eq!(
            positions,
            &[
                [-12., 12., 0.],
                [-12., -12., 0.],
                [12., -12., 0.],
                [12., 12., 0.]
            ]
        );
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("HUD UVs");
        };
        assert_eq!(uvs[0], [0.5 / 128., 48.5 / 128.]);
        assert_eq!(uvs[2], [23.5 / 128., 71.5 / 128.]);
    }
}
