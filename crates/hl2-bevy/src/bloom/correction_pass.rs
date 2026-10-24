//! Color correction, render half: the lookup volumes are applied by the bloom composite, in the
//! same pass (Engine_Post `PerformColorCorrection` after the bloom add), so no pass is added.
//! The pipelines exist per (target format, lookup count, bloom) and are queued the first time
//! a view needs one; the uniform buffer and bind group are made once per view.
use super::{Pipelines, SourceBloom, color_correction::SourceColorCorrection};
use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        system::{Commands, Local, Query, Res},
    },
    math::Vec4,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, PipelineCache,
            RenderPipelineDescriptor, ShaderStages, ShaderType, TextureFormat, TextureSampleType,
            TextureViewId, UniformBuffer,
            binding_types::{texture_3d, uniform_buffer},
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
        view::ExtractedView,
    },
};
use hl2_simulation::color_correction::MAX_LOOKUPS;
use std::collections::HashMap;

#[derive(ShaderType, Clone, Copy, Default)]
struct CorrectionData {
    weights: Vec4,
    /// The default weight in x.
    default_weight: Vec4,
}

pub(super) fn layout() -> BindGroupLayoutDescriptor {
    let volume = || texture_3d(TextureSampleType::Float { filterable: true });
    BindGroupLayoutDescriptor::new(
        "source_color_correction_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                volume(),
                volume(),
                volume(),
                volume(),
                uniform_buffer::<CorrectionData>(false),
            ),
        ),
    )
}

/// What the composite of one view needs to apply the lookups this frame.
#[derive(Component)]
pub(super) struct Correction {
    pub pipeline: CachedRenderPipelineId,
    pub group: BindGroup,
}

pub(super) type PipelineKey = (TextureFormat, usize, bool);
type Known = HashMap<PipelineKey, CachedRenderPipelineId>;
type Buffers = HashMap<Entity, Prepared>;

pub(super) struct Prepared {
    buffer: UniformBuffer<CorrectionData>,
    group: Option<(Vec<TextureViewId>, BindGroup)>,
}

fn queue_pipeline(
    pipelines: &Pipelines,
    cache: &PipelineCache,
    (format, lookups, bloom): PipelineKey,
) -> CachedRenderPipelineId {
    let mut shader_defs = vec![
        "COLOR_CORRECTION".into(),
        bevy::shader::ShaderDefVal::UInt("LOOKUPS".into(), lookups as u32),
    ];
    if bloom {
        shader_defs.push("BLOOM".into());
    }
    cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("source_color_correction".into()),
        layout: vec![
            pipelines.layout.clone(),
            pipelines.correction_layout.clone(),
        ],
        vertex: pipelines.fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: pipelines.shader.clone(),
            shader_defs,
            entry_point: Some("composite_cc".into()),
            targets: vec![Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        ..Default::default()
    })
}

pub(super) fn prepare(
    mut commands: Commands,
    (pipelines, cache, images): (
        Res<Pipelines>,
        Res<PipelineCache>,
        Res<RenderAssets<GpuImage>>,
    ),
    (device, queue): (Res<RenderDevice>, Res<RenderQueue>),
    (mut known, mut prepared): (Local<Known>, Local<Buffers>),
    views: Query<(Entity, &ExtractedView, &SourceBloom, &SourceColorCorrection)>,
    stale: Query<Entity, (With<Correction>, Without<SourceColorCorrection>)>,
) {
    for entity in &stale {
        commands.entity(entity).remove::<Correction>();
    }
    for (entity, view, bloom, correction) in &views {
        let views: Vec<&GpuImage> = correction
            .lookups
            .iter()
            .filter_map(|handle| images.get(handle))
            .collect();
        if views.is_empty() || views.len() != correction.lookups.len() {
            // Still uploading: the composite runs without the lookups.
            commands.entity(entity).remove::<Correction>();
            continue;
        }
        let state = prepared.entry(entity).or_insert_with(|| Prepared {
            buffer: UniformBuffer::default(),
            group: None,
        });
        let [w0, w1, w2, w3] = correction.mix.weights;
        state.buffer.set(CorrectionData {
            weights: Vec4::new(w0, w1, w2, w3),
            default_weight: Vec4::new(correction.mix.default, 0., 0., 0.),
        });
        state.buffer.write_buffer(&device, &queue);
        let ids: Vec<TextureViewId> = views.iter().map(|image| image.texture_view.id()).collect();
        if state
            .group
            .as_ref()
            .is_none_or(|(known_ids, _)| *known_ids != ids)
        {
            // Slots past the bound lookups repeat the first volume; the shader never reads them.
            let slot = |i: usize| &views[i.min(views.len() - 1)].texture_view;
            let layout = cache.get_bind_group_layout(&pipelines.correction_layout);
            let Some(buffer) = state.buffer.binding() else {
                continue;
            };
            let group = device.create_bind_group(
                Some("source_color_correction_bind_group"),
                &layout,
                &BindGroupEntries::sequential((slot(0), slot(1), slot(2), slot(3), buffer)),
            );
            state.group = Some((ids, group));
        }
        let Some((_, group)) = &state.group else {
            continue;
        };
        let key = (
            view.target_format,
            views.len().min(MAX_LOOKUPS),
            bloom.amount > 0.,
        );
        let pipeline = *known
            .entry(key)
            .or_insert_with(|| queue_pipeline(&pipelines, &cache, key));
        commands.entity(entity).insert(Correction {
            pipeline,
            group: group.clone(),
        });
    }
}
