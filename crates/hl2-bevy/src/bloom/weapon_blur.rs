//! Shared Advanced motion reconstruction for world and weapon cameras.
use bevy::{
    core_pipeline::{
        FullscreenShader,
        core_3d::main_opaque_pass_3d,
        schedule::{Core3d, Core3dSystems},
    },
    ecs::{
        component::Component,
        resource::Resource,
        schedule::IntoScheduleConfigs,
        system::{Commands, Res},
    },
    prelude::{App, AssetServer, Handle, Plugin, Shader},
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        diagnostic::RecordDiagnostics,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::{
            BindGroup, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, FilterMode, FragmentState, MultisampleState,
            PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            ShaderType, TextureFormat, TextureSampleType, TextureView, UniformBuffer,
            binding_types::{sampler, texture_2d, texture_2d_multisampled, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::CachedTexture,
        view::ViewTarget,
    },
};

mod prepare;

#[derive(Component, Clone, Copy, PartialEq, ExtractComponent)]
pub(super) struct WeaponBlur {
    pub max: f32,
    pub world: bool,
}

pub(super) struct WeaponBlurPlugin;
impl Plugin for WeaponBlurPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<WeaponBlur>::default());
        let Some(render) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render
            .add_systems(RenderStartup, init)
            .add_systems(
                Render,
                (
                    prepare::textures.in_set(RenderSystems::PrepareResources),
                    prepare::groups.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(
                Core3d,
                snapshot
                    .in_set(Core3dSystems::MainPass)
                    .before(main_opaque_pass_3d),
            )
            .add_systems(
                Core3d,
                reconstruct
                    .in_set(Core3dSystems::PostProcess)
                    .before(super::source_bloom),
            );
    }
}

#[derive(Resource)]
struct Pipelines {
    raw_layouts: [BindGroupLayoutDescriptor; 2],
    reduce_layout: BindGroupLayoutDescriptor,
    composite_layouts: [BindGroupLayoutDescriptor; 2],
    fullscreen: FullscreenShader,
    shader: Handle<Shader>,
    sampler: Sampler,
}

#[derive(Clone, Copy, Default, ShaderType)]
struct Data {
    params: bevy::math::Vec4,
    reprojection: bevy::math::Mat4,
}

fn motion_binding(
    msaa: bool,
    kind: TextureSampleType,
) -> bevy::render::render_resource::BindGroupLayoutEntryBuilder {
    if msaa {
        texture_2d_multisampled(kind)
    } else {
        texture_2d(kind)
    }
}

fn init(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    assets: Res<AssetServer>,
) {
    let float = TextureSampleType::Float { filterable: false };
    let filtered = TextureSampleType::Float { filterable: true };
    let raw_layouts = [false, true].map(|msaa| {
        BindGroupLayoutDescriptor::new(
            "weapon_motion_reduce_raw",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    motion_binding(msaa, float),
                    motion_binding(msaa, TextureSampleType::Depth),
                    uniform_buffer::<Data>(false),
                ),
            ),
        )
    });
    let reduce_layout = BindGroupLayoutDescriptor::new(
        "weapon_motion_reduce",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(float), uniform_buffer::<Data>(false)),
        ),
    );
    let composite_layouts = [false, true].map(|msaa| {
        BindGroupLayoutDescriptor::new(
            "weapon_motion_composite",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(filtered),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<Data>(false),
                    texture_2d(filtered),
                    motion_binding(msaa, float),
                    motion_binding(msaa, TextureSampleType::Depth),
                    texture_2d(float),
                ),
            ),
        )
    });
    commands.insert_resource(Pipelines {
        raw_layouts,
        reduce_layout,
        composite_layouts,
        fullscreen: fullscreen.clone(),
        shader: assets.load("shaders/weapon_blur.wgsl"),
        sampler: device.create_sampler(&SamplerDescriptor {
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            ..Default::default()
        }),
    });
}

fn pipeline(
    p: &Pipelines,
    cache: &PipelineCache,
    entry: &'static str,
    format: TextureFormat,
    samples: u32,
    world: bool,
) -> CachedRenderPipelineId {
    let msaa = usize::from(samples > 1);
    let layout = match entry {
        "reduce_raw" => &p.raw_layouts[msaa],
        "composite" => &p.composite_layouts[msaa],
        _ => &p.reduce_layout,
    };
    cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some(format!("weapon_blur_{entry}").into()),
        layout: vec![layout.clone()],
        vertex: p.fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: p.shader.clone(),
            shader_defs: {
                let mut defs = Vec::new();
                if world {
                    defs.push("WORLD_RECONSTRUCTION".into());
                }
                if msaa == 1 {
                    defs.push("MULTISAMPLED".into());
                }
                if entry == "reduce_raw" {
                    defs.push("REDUCE_RAW".into());
                } else if entry != "composite" {
                    defs.push("REDUCE_FIELD".into());
                }
                defs
            },
            entry_point: Some(entry.into()),
            targets: vec![Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        multisample: MultisampleState {
            count: if entry == "composite" { samples } else { 1 },
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Component)]
struct Textures {
    background: CachedTexture,
    frame: CachedTexture,
    reduced: [CachedTexture; 3],
    world: bool,
}

#[derive(Component)]
struct Prepared {
    pipelines: [CachedRenderPipelineId; 4],
    groups: [BindGroup; 4],
    _buffer: UniformBuffer<Data>,
}

fn snapshot(view: ViewQuery<(&ViewTarget, &Textures, &WeaponBlur)>, mut ctx: RenderContext) {
    let (target, textures, blur) = view.into_inner();
    if blur.max == 0. || blur.world {
        return;
    }
    ctx.command_encoder().copy_texture_to_texture(
        target.main_texture().as_image_copy(),
        textures.background.texture.as_image_copy(),
        textures.background.texture.size(),
    );
}

fn draw(
    ctx: &mut RenderContext,
    cache: &PipelineCache,
    pipeline: CachedRenderPipelineId,
    group: &BindGroup,
    target: &TextureView,
    label: &'static str,
) {
    let Some(pipeline) = cache.get_render_pipeline(pipeline) else {
        return;
    };
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: Default::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, label);
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

fn reconstruct(
    view: ViewQuery<(&ViewTarget, &Textures, &Prepared, &WeaponBlur)>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, textures, prepared, blur) = view.into_inner();
    if blur.max == 0.
        || prepared
            .pipelines
            .iter()
            .any(|id| cache.get_render_pipeline(*id).is_none())
    {
        return;
    }
    ctx.command_encoder().copy_texture_to_texture(
        target.main_texture().as_image_copy(),
        textures.frame.texture.as_image_copy(),
        textures.frame.texture.size(),
    );
    for i in 0..3 {
        draw(
            &mut ctx,
            &cache,
            prepared.pipelines[i],
            &prepared.groups[i],
            &textures.reduced[i].default_view,
            [
                "weapon_reduce_4",
                "weapon_reduce_32",
                "weapon_motion_dilate",
            ][i],
        );
    }
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("weapon_motion_reconstruction"),
        color_attachments: &[Some(target.get_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(
        &mut pass,
        if blur.world {
            "world_motion_blur"
        } else {
            "viewmodel_animation_blur"
        },
    );
    pass.set_render_pipeline(
        cache
            .get_render_pipeline(prepared.pipelines[3])
            .expect("checked pipelines"),
    );
    pass.set_bind_group(0, &prepared.groups[3], &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}
