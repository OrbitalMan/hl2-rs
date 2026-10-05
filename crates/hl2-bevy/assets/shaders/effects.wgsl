#import bevy_pbr::forward_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var effect_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var effect_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> parameters: vec4<f32>;
fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(pow((color + vec3(0.055)) / 1.055, vec3(2.4)), color / 12.92, color <= vec3(0.04045));
}
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(effect_texture, effect_sampler, mesh.uv);
    color.a = select(color.a, 1.0, parameters.y > 0.5);
    color *= mesh.color;
    if color.a < parameters.x { discard; }
    return vec4(srgb_to_linear(color.rgb * parameters.z), color.a);
}
