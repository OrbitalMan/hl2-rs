// Source image-space motion blur (materialsystem/stdshaders/motion_blur_ps2x.fxc): the frame is
// averaged along one blur vector per pixel, made of the global (turning) vector, the falling
// vector (a zoom, damped in the middle of the screen) and the roll vector.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
// values: yaw, pitch, falling and roll blur; params.x the longest vector, params.y the samples.
struct Blur {
    values: vec4<f32>,
    params: vec4<f32>,
}
@group(0) @binding(2) var<uniform> blur: Blur;

#ifdef ANIMATION
#ifdef ANIMATION_MSAA
@group(0) @binding(3) var vectors: texture_multisampled_2d<f32>;
#else
@group(0) @binding(3) var vectors: texture_2d<f32>;
#endif
fn velocity(uv: vec2<f32>) -> vec2<f32> {
    let size = vec2<i32>(textureDimensions(vectors));
    let pixel = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2(0), size - vec2(1));
#ifdef ANIMATION_MSAA
    var sum = vec2(0.0);
    let count = textureNumSamples(vectors);
    for (var i = 0u; i < count; i++) { sum += textureLoad(vectors, pixel, i32(i)).xy; }
    return sum / f32(count);
#else
    return textureLoad(vectors, pixel, 0).xy;
#endif
}
#endif
@fragment
fn motion_blur(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
#ifdef ANIMATION
    let base = textureSampleLevel(source, linear_sampler, in.uv, 0.0);
    let motion = velocity(in.uv) * blur.params.z;
    let pixels = motion * vec2<f32>(textureDimensions(source));
    if dot(pixels, pixels) < 1.0 { return base; }
    let vector = motion * min(1.0, blur.params.x / max(length(motion), 0.00001));
    var sum = base;
    var weight = 1.0;
    for (var i = 0; i < 7; i++) {
        if i == 3 { continue; } // The centre sample is already in sum.
        let offset = vector * (f32(i) / 6.0 - 0.5);
        let uv = in.uv + offset;
        if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) { continue; }
        // Reject unrelated foreground/background velocities rather than smearing them
        // into the weapon. Stationary background pixels return unchanged above.
        let sample_motion = velocity(uv) * blur.params.z;
        if length(sample_motion - motion) > max(length(motion) * 0.5, 0.001) { continue; }
        sum += textureSampleLevel(source, linear_sampler, uv, 0.0);
        weight += 1.0;
    }
    return sum / weight;
#else
    var falling = in.uv * 2.0 - 1.0;
    // cross(float3(falling, 0), float3(0, 0, 1)).xy
    var roll = vec2(falling.y, -falling.x);
    let global = vec2(blur.values.x, -blur.values.y);
    // Keeps the samples on screen by pointing the vector in.
    let falling_intensity = -abs(blur.values.z);
    falling = falling * dot(falling, falling) * falling_intensity;
    roll = roll * blur.values.w;
    var vector = global + falling + roll;
    if length(vector) > blur.params.x {
        vector = normalize(vector) * blur.params.x;
    }
    let count = i32(blur.params.y);
    let step = vector / f32(count - 1);
    var color = vec3(0.0);
    for (var i = 0; i < count; i++) {
        color += textureSampleLevel(source, linear_sampler, in.uv + step * f32(i), 0.0).rgb;
    }
    let alpha = textureSampleLevel(source, linear_sampler, in.uv, 0.0).a;
    return vec4(color / f32(count), alpha);
#endif
}
