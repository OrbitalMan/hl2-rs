//! Cached textures, pipelines and bind groups; no steady-frame allocations.
use super::{Data, Pipelines, Prepared, Textures, WeaponBlur, pipeline};
use bevy::{
    core_pipeline::prepass::{PreviousViewData, ViewPrepassTextures},
    ecs::{
        entity::Entity,
        query::{Or, With, Without},
        system::{Commands, Local, Query, Res},
    },
    math::{Mat4, Vec4},
    prelude::Msaa,
    render::{
        camera::ExtractedCamera,
        render_resource::{
            BindGroupEntries, CachedRenderPipelineId, Extent3d, PipelineCache, TextureDescriptor,
            TextureDimension, TextureFormat, TextureUsages, TextureViewId, UniformBuffer,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::CachedTexture,
        view::ExtractedView,
    },
};
use std::collections::HashMap;

type Stale = (
    Or<(With<Textures>, With<Prepared>, With<Key>)>,
    Without<WeaponBlur>,
);

type TextureViewData<'w> = (
    Entity,
    &'w ExtractedCamera,
    &'w ExtractedView,
    &'w WeaponBlur,
    Option<&'w Textures>,
);

pub(super) fn textures(
    mut commands: Commands,
    device: Res<RenderDevice>,
    views: Query<TextureViewData>,
    stale: Query<Entity, Stale>,
) {
    for entity in &stale {
        commands
            .entity(entity)
            .remove::<(Textures, Prepared, Key)>();
    }
    for (entity, camera, view, blur, current) in &views {
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };
        if current.is_some_and(|t| {
            t.world == blur.world
                && t.frame.texture.width() == size.x
                && t.frame.texture.height() == size.y
                && t.frame.texture.format() == view.target_format
        }) {
            continue;
        }
        let texture = |width, height, format, usage| {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("weapon_motion_texture"),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            });
            let default_view = texture.create_view(&Default::default());
            CachedTexture {
                texture,
                default_view,
            }
        };
        let copied = TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING;
        let frame = texture(size.x, size.y, view.target_format, copied);
        let background = if blur.world {
            frame.clone()
        } else {
            texture(size.x, size.y, view.target_format, copied)
        };
        let reduced = [4, 32, 32].map(|factor| {
            texture(
                size.x.div_ceil(factor),
                size.y.div_ceil(factor),
                TextureFormat::Rgba16Float,
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            )
        });
        commands.entity(entity).insert(Textures {
            background,
            frame,
            reduced,
            world: blur.world,
        });
    }
}

#[derive(bevy::ecs::component::Component, PartialEq)]
pub(super) struct Key {
    textures: [TextureViewId; 7],
    samples: u32,
    world: bool,
}
type View<'w> = (
    Entity,
    &'w WeaponBlur,
    &'w Textures,
    &'w Msaa,
    &'w ExtractedView,
    &'w ViewPrepassTextures,
    Option<&'w PreviousViewData>,
    Option<&'w Key>,
    Option<&'w mut Prepared>,
);

pub(super) fn groups(
    mut commands: Commands,
    (p, cache): (Res<Pipelines>, Res<PipelineCache>),
    (device, queue): (Res<RenderDevice>, Res<RenderQueue>),
    mut known: Local<HashMap<(TextureFormat, u32, bool), [CachedRenderPipelineId; 4]>>,
    mut views: Query<View>,
) {
    for (entity, blur, textures, msaa, view, prepass, previous, old, prepared) in &mut views {
        let (Some(vectors), Some(depth)) = (prepass.motion_vectors_view(), prepass.depth_view())
        else {
            continue;
        };
        let key = Key {
            textures: [
                textures.frame.default_view.id(),
                textures.background.default_view.id(),
                vectors.id(),
                depth.id(),
                textures.reduced[0].default_view.id(),
                textures.reduced[1].default_view.id(),
                textures.reduced[2].default_view.id(),
            ],
            samples: msaa.samples(),
            world: blur.world,
        };
        let data = view_data(blur, textures, view, previous);
        if old == Some(&key)
            && let Some(mut prepared) = prepared
        {
            if prepared._buffer.get().params != data.params
                || prepared._buffer.get().reprojection != data.reprojection
            {
                prepared._buffer.set(data);
                prepared._buffer.write_buffer(&device, &queue);
            }
            continue;
        }
        let pipelines = *known
            .entry((view.target_format, msaa.samples(), blur.world))
            .or_insert_with(|| {
                pipeline_set(&p, &cache, view.target_format, msaa.samples(), blur.world)
            });
        let mut buffer = UniformBuffer::from(data);
        buffer.write_buffer(&device, &queue);
        let binding = buffer.binding().expect("written buffer");
        let raw = cache.get_bind_group_layout(&p.raw_layouts[usize::from(msaa.samples() > 1)]);
        let reduce = cache.get_bind_group_layout(&p.reduce_layout);
        let composite =
            cache.get_bind_group_layout(&p.composite_layouts[usize::from(msaa.samples() > 1)]);
        let groups = [
            device.create_bind_group(
                Some("weapon_reduce_raw"),
                &raw,
                &BindGroupEntries::sequential((vectors, depth, binding.clone())),
            ),
            device.create_bind_group(
                Some("weapon_reduce_tiles"),
                &reduce,
                &BindGroupEntries::sequential((&textures.reduced[0].default_view, binding.clone())),
            ),
            device.create_bind_group(
                Some("weapon_dilate"),
                &reduce,
                &BindGroupEntries::sequential((&textures.reduced[1].default_view, binding.clone())),
            ),
            device.create_bind_group(
                Some("weapon_composite"),
                &composite,
                &BindGroupEntries::sequential((
                    &textures.frame.default_view,
                    &p.sampler,
                    binding,
                    &textures.background.default_view,
                    vectors,
                    depth,
                    &textures.reduced[2].default_view,
                )),
            ),
        ];
        commands.entity(entity).insert((
            key,
            Prepared {
                pipelines,
                groups,
                _buffer: buffer,
            },
        ));
    }
}

fn view_data(
    blur: &WeaponBlur,
    textures: &Textures,
    view: &ExtractedView,
    previous: Option<&PreviousViewData>,
) -> Data {
    let mut reprojection = Mat4::IDENTITY;
    if blur.world {
        let current = view
            .clip_from_world
            .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());
        let prior = previous.map_or(current, |previous| previous.clip_from_world);
        if blur.max > 0. {
            reprojection = prior * current.inverse();
        }
    }
    Data {
        reprojection,
        params: Vec4::new(
            blur.max,
            1.25,
            textures.frame.texture.width() as f32,
            textures.frame.texture.height() as f32,
        ),
    }
}

fn pipeline_set(
    p: &Pipelines,
    cache: &PipelineCache,
    format: TextureFormat,
    samples: u32,
    world: bool,
) -> [CachedRenderPipelineId; 4] {
    [
        pipeline(
            p,
            cache,
            "reduce_raw",
            TextureFormat::Rgba16Float,
            samples,
            world,
        ),
        pipeline(
            p,
            cache,
            "reduce_tiles",
            TextureFormat::Rgba16Float,
            1,
            world,
        ),
        pipeline(p, cache, "dilate", TextureFormat::Rgba16Float, 1, world),
        pipeline(p, cache, "composite", format, samples, world),
    ]
}
