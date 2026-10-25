// Source alpha-tested silhouettes in the flashlight's depth map. Bevy's default
// prepass vertex path provides the same posed GPU skinning and UVs as the colour draw.
#import bevy_pbr::prepass_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> parameters: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) {
    if parameters.x > 0.0 {
        let alpha = select(textureSample(base_texture, base_sampler, mesh.uv).a, 1.0, parameters.y > 0.5);
        if alpha * tint.a < parameters.x { discard; }
    }
}
