#import "shaders/source_uniforms.wgsl"::material
// Source alpha-tested silhouettes in the flashlight's depth map. Bevy's default
// prepass vertex path provides the same posed GPU skinning and UVs as the colour draw.
#import bevy_pbr::prepass_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;

#ifdef MOTION_VECTOR_PREPASS
#import bevy_pbr::{prepass_io::FragmentOutput, prepass_bindings, mesh_view_bindings::view}
#endif
@fragment
fn fragment(mesh: VertexOutput)
#ifdef MOTION_VECTOR_PREPASS
 -> FragmentOutput
#endif
{
    if material.parameters.x > 0.0 {
        let alpha = select(textureSample(base_texture, base_sampler, mesh.uv).a, 1.0, material.parameters.y > 0.5);
        if alpha * material.tint.a < material.parameters.x { discard; }
    }
#ifdef MOTION_VECTOR_PREPASS
    var out: FragmentOutput;
    let current = view.unjittered_clip_from_world * mesh.world_position;
    let previous = prepass_bindings::previous_view_uniforms.clip_from_world * mesh.previous_world_position;
    out.motion_vector = (current.xy / current.w - previous.xy / previous.w) * vec2(0.5, -0.5);
    return out;
#endif
}
