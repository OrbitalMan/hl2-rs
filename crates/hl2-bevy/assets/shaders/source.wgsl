#import bevy_pbr::forward_io::VertexOutput
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> parameters: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var lightmap_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var lightmap_sampler: sampler;
fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(pow((color + vec3(0.055)) / 1.055, vec3(2.4)), color / 12.92, color <= vec3(0.04045));
}
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    var base = textureSample(base_texture, base_sampler, mesh.uv);
    base.a = select(base.a, 1.0, parameters.y > 0.5);
    base *= tint * mesh.color;
    if base.a < parameters.x { discard; }
    let baked = textureSample(lightmap_texture, lightmap_sampler, mesh.uv_b).rgb;
    // LDR atlases are gamma-encoded bytes. Preserve prototype multiplication
    // before converting for the sRGB output target. Source HDR remains absent.
    let rgb = srgb_to_linear(base.rgb * baked);
    // Bevy Add uses a premultiplied pipeline. Alpha zero retains all of the
    // destination while premultiplying RGB implements Source SrcAlpha/One.
    if parameters.z > 0.5 { return vec4(rgb * base.a, 0.0); }
    return vec4(rgb, base.a);
}
