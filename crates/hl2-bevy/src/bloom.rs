//! Source HDR bloom (SDK viewpostprocess.cpp Generate8BitBloomTexture + Engine_Post): a
//! gamma-space shaped quarter-size downsample, a 13-tap Gaussian blur in X and Y (Y scaled
//! by the bloom amount) and an additive composite before the HUD.
use bevy::{
    core_pipeline::{
        FullscreenShader,
        schedule::{Core3d, Core3dSystems},
        tonemapping::tonemapping,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::{CachedTexture, TextureCache},
        view::{ExtractedView, ViewTarget},
    },
};

/// Bloom amount on the last 3D camera of the main view, and the image that receives the
/// pre-bloom frame for auto exposure.
#[derive(Component, Clone, ExtractComponent)]
pub struct SourceBloom {
    pub amount: f32,
    pub presample: Option<Handle<Image>>,
}

pub struct SourceBloomPlugin;
impl Plugin for SourceBloomPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<SourceBloom>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pipelines)
            .add_systems(
                Render,
                (
                    prepare_pipelines.in_set(RenderSystems::Prepare),
                    prepare_textures.in_set(RenderSystems::PrepareResources),
                ),
            )
            // After the main pass (PostProcess set), before tonemapping: ordered only
            // `.before(tonemapping)`, the executor could run it before the viewmodel camera's
            // main pass, and the post-process swap then discarded the weapon on some frames.
            .add_systems(
                Core3d,
                source_bloom
                    .in_set(Core3dSystems::PostProcess)
                    .before(tonemapping),
            );
    }
}

const SMALL_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

#[derive(Resource)]
struct Pipelines {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shape: CachedRenderPipelineId,
    blur: CachedRenderPipelineId,
    presample: CachedRenderPipelineId,
    fullscreen: FullscreenShader,
    shader: Handle<Shader>,
}

#[derive(ShaderType, Clone, Copy)]
struct PassData {
    texel_direction: Vec4,
    params: Vec4,
}

fn descriptor(
    pipelines_layout: &BindGroupLayoutDescriptor,
    fullscreen: &FullscreenShader,
    shader: &Handle<Shader>,
    entry: &'static str,
    format: TextureFormat,
) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: Some(format!("source_bloom_{entry}").into()),
        layout: vec![pipelines_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: shader.clone(),
            entry_point: Some(entry.into()),
            targets: vec![Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    }
}

fn init_pipelines(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    assets: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "source_bloom_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<PassData>(false),
            ),
        ),
    );
    let sampler = device.create_sampler(&SamplerDescriptor {
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        ..default()
    });
    let shader = assets.load("shaders/bloom.wgsl");
    let fullscreen = fullscreen.clone();
    let shape = cache.queue_render_pipeline(descriptor(
        &layout,
        &fullscreen,
        &shader,
        "shape",
        SMALL_FORMAT,
    ));
    let blur = cache.queue_render_pipeline(descriptor(
        &layout,
        &fullscreen,
        &shader,
        "blur",
        SMALL_FORMAT,
    ));
    let presample = cache.queue_render_pipeline(descriptor(
        &layout,
        &fullscreen,
        &shader,
        "presample",
        SMALL_FORMAT,
    ));
    commands.insert_resource(Pipelines {
        layout,
        sampler,
        shape,
        blur,
        presample,
        fullscreen,
        shader,
    });
}

/// The composite pipeline depends on the view's target format.
#[derive(Component)]
struct CompositePipeline(CachedRenderPipelineId);

fn prepare_pipelines(
    mut commands: Commands,
    pipelines: Res<Pipelines>,
    cache: Res<PipelineCache>,
    mut known: Local<std::collections::HashMap<TextureFormat, CachedRenderPipelineId>>,
    views: Query<(Entity, &ExtractedView), With<SourceBloom>>,
) {
    for (entity, view) in &views {
        let id = *known.entry(view.target_format).or_insert_with(|| {
            cache.queue_render_pipeline(descriptor(
                &pipelines.layout,
                &pipelines.fullscreen,
                &pipelines.shader,
                "composite",
                view.target_format,
            ))
        });
        commands.entity(entity).insert(CompositePipeline(id));
    }
}

#[derive(Component)]
struct SmallTextures([CachedTexture; 2]);

fn prepare_textures(
    mut commands: Commands,
    mut cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedCamera), With<SourceBloom>>,
) {
    for (entity, camera) in &views {
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };
        // _rt_SmallFB0/1 are exactly a quarter of the frame.
        let descriptor = |label| TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: (size.x / 4).max(1),
                height: (size.y / 4).max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: SMALL_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let textures = [
            cache.get(&device, descriptor("source_bloom_small0")),
            cache.get(&device, descriptor("source_bloom_small1")),
        ];
        commands.entity(entity).insert(SmallTextures(textures));
    }
}

fn source_bloom(
    view: ViewQuery<(
        &ViewTarget,
        &SourceBloom,
        &SmallTextures,
        &CompositePipeline,
    )>,
    pipelines: Res<Pipelines>,
    cache: Res<PipelineCache>,
    images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    mut ctx: RenderContext,
) {
    let (target, bloom, small, composite) = view.into_inner();
    let device = ctx.render_device().clone();
    let layout = cache.get_bind_group_layout(&pipelines.layout);
    let draw = |ctx: &mut RenderContext,
                pipeline: &RenderPipeline,
                source: &TextureView,
                glow: &TextureView,
                data: &Buffer,
                destination: &TextureView| {
        let bind_group = device.create_bind_group(
            Some("source_bloom_bind_group"),
            &layout,
            &BindGroupEntries::sequential((
                source,
                glow,
                &pipelines.sampler,
                data.as_entire_binding(),
            )),
        );
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("source_bloom"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    };
    let uniform = |label: &str, data: PassData| {
        let mut buffer = encase::UniformBuffer::new(Vec::new());
        buffer.write(&data).expect("bloom uniform");
        device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some(label),
            contents: buffer.as_ref(),
            usage: BufferUsages::UNIFORM,
        })
    };
    let one = Vec4::new(1., 0., 0., 0.);
    if let (Some(pipeline), Some(image)) = (
        cache.get_render_pipeline(pipelines.presample),
        bloom.presample.as_ref().and_then(|h| images.get(h)),
    ) {
        let data = uniform(
            "source_bloom_presample",
            PassData {
                texel_direction: Vec4::ZERO,
                params: one,
            },
        );
        let frame = target.main_texture_view();
        draw(&mut ctx, pipeline, frame, frame, &data, &image.texture_view);
    }
    if bloom.amount <= 0. {
        return;
    }
    let (Some(shape), Some(blur), Some(composite)) = (
        cache.get_render_pipeline(pipelines.shape),
        cache.get_render_pipeline(pipelines.blur),
        cache.get_render_pipeline(composite.0),
    ) else {
        return;
    };
    let small_size = small.0[0].texture.size();
    let texel = Vec2::new(1. / small_size.width as f32, 1. / small_size.height as f32);
    let shape_data = uniform(
        "source_bloom_shape",
        PassData {
            texel_direction: Vec4::ZERO,
            params: one,
        },
    );
    let x_data = uniform(
        "source_bloom_blur_x",
        PassData {
            texel_direction: Vec4::new(texel.x, texel.y, 1., 0.),
            params: one,
        },
    );
    let y_data = uniform(
        "source_bloom_blur_y",
        PassData {
            texel_direction: Vec4::new(texel.x, texel.y, 0., 1.),
            params: Vec4::new(bloom.amount, 0., 0., 0.),
        },
    );
    // Engine_Post BloomFactor is 1 when bloom is enabled.
    let composite_data = uniform(
        "source_bloom_composite",
        PassData {
            texel_direction: Vec4::ZERO,
            params: one,
        },
    );
    let post = target.post_process_write();
    let passes: [(
        &RenderPipeline,
        &TextureView,
        &TextureView,
        &Buffer,
        &TextureView,
    ); 4] = [
        (
            shape,
            post.source,
            post.source,
            &shape_data,
            &small.0[0].default_view,
        ),
        (
            blur,
            &small.0[0].default_view,
            &small.0[0].default_view,
            &x_data,
            &small.0[1].default_view,
        ),
        (
            blur,
            &small.0[1].default_view,
            &small.0[1].default_view,
            &y_data,
            &small.0[0].default_view,
        ),
        (
            composite,
            post.source,
            &small.0[0].default_view,
            &composite_data,
            post.destination,
        ),
    ];
    for (pipeline, source, glow, data, destination) in passes {
        draw(&mut ctx, pipeline, source, glow, data, destination);
    }
}
