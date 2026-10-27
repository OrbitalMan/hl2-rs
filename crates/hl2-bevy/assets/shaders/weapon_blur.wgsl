// Advanced-only silhouette reconstruction. Reductions choose dominant motion; the gather
// integrates weapon coverage over the preserved world rather than clamping to its outline.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Data { params: vec4<f32>, reprojection: mat4x4<f32> }
fn camera_motion(uv: vec2<f32>, z: f32, data: Data) -> vec2<f32> {
    let clip = vec4(uv * vec2(2.0,-2.0) + vec2(-1.0,1.0),z,1.0);
    let previous = data.reprojection * clip;
    if abs(previous.w) < 1e-8 { return vec2(0.0); }
    return (clip.xy - previous.xy / previous.w) * vec2(0.5,-0.5);
}
#ifdef REDUCE_RAW
#ifdef MULTISAMPLED
@group(0) @binding(0) var raw: texture_multisampled_2d<f32>;
@group(0) @binding(1) var raw_depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var raw: texture_2d<f32>;
@group(0) @binding(1) var raw_depth: texture_depth_2d;
#endif
@group(0) @binding(2) var<uniform> data: Data;
fn relative_motion(v: vec2<f32>, p: vec2<i32>, z: f32) -> vec2<f32> {
#ifdef WORLD_RECONSTRUCTION
    if z <= 0.0 { return vec2(0.0); }
    return v - camera_motion((vec2<f32>(p)+0.5)/data.params.zw,z,data);
#else
    return v;
#endif
}
fn raw_motion(p: vec2<i32>) -> vec2<f32> {
#ifdef MULTISAMPLED
    var best = vec2(0.0);
    for (var i = 0u; i < textureNumSamples(raw); i++) {
        let v = relative_motion(textureLoad(raw, p, i32(i)).xy,p,textureLoad(raw_depth,p,i32(i)));
        if dot(v,v) > dot(best,best) { best = v; }
    }
    return best;
#else
    return relative_motion(textureLoad(raw, p, 0).xy,p,textureLoad(raw_depth,p,0));
#endif
}
@fragment
fn reduce_raw(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let start = vec2<i32>(in.position.xy) * 4;
    let size = vec2<i32>(textureDimensions(raw));
    var best = vec2(0.0);
    for (var y = 0; y < 4; y++) { for (var x = 0; x < 4; x++) {
        let p = start + vec2(x,y);
        if any(p >= size) { continue; }
        let v = raw_motion(p);
        if dot(v,v) > dot(best,best) { best = v; }
    }}
    return vec4(best, 0.0, 0.0);
}
#else
#ifdef REDUCE_FIELD
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var<uniform> data: Data;
@fragment
fn reduce_tiles(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let start = vec2<i32>(in.position.xy) * 8;
    let size = vec2<i32>(textureDimensions(field));
    var best = vec2(0.0);
    for (var y = 0; y < 8; y++) { for (var x = 0; x < 8; x++) {
        let p = start + vec2(x,y);
        if any(p >= size) { continue; }
        let v = textureLoad(field,p,0).xy;
        if dot(v,v) > dot(best,best) { best = v; }
    }}
    return vec4(best,0.0,0.0);
}
@fragment
fn dilate(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<i32>(in.position.xy);
    let size = vec2<i32>(textureDimensions(field));
    let radius = i32(ceil(data.params.x * max(data.params.z, data.params.w) * 0.5 / 32.0)) + 1;
    var best = vec2(0.0);
    for (var y = -radius; y <= radius; y++) { for (var x = -radius; x <= radius; x++) {
        let p = clamp(center + vec2(x,y),vec2(0),size-vec2(1));
        let v = textureLoad(field,p,0).xy;
        if dot(v,v) > dot(best,best) { best = v; }
    }}
    return vec4(best,0.0,0.0);
}
#else
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> data: Data;
@group(0) @binding(3) var background: texture_2d<f32>;
#ifdef MULTISAMPLED
@group(0) @binding(4) var motion: texture_multisampled_2d<f32>;
@group(0) @binding(5) var depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(4) var motion: texture_2d<f32>;
@group(0) @binding(5) var depth: texture_depth_2d;
#endif
@group(0) @binding(6) var tiles: texture_2d<f32>;
fn weapon(uv: vec2<f32>) -> vec3<f32> {
    let p = clamp(vec2<i32>(uv * data.params.zw),vec2(0),vec2<i32>(data.params.zw)-vec2(1));
#ifdef MULTISAMPLED
    var velocity = vec2(0.0);
    var coverage = 0.0;
    let count = textureNumSamples(motion);
    for (var i = 0u; i < count; i++) {
        if textureLoad(depth,p,i32(i)) > 0.0 {
            velocity += textureLoad(motion,p,i32(i)).xy;
            coverage += 1.0;
        }
    }
    return vec3(velocity / max(coverage,1.0),coverage / f32(count));
#else
    return vec3(textureLoad(motion,p,0).xy, select(0.0,1.0,textureLoad(depth,p,0)>0.0));
#endif
}
#ifndef WORLD_RECONSTRUCTION
@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let base = textureSampleLevel(frame,linear_sampler,in.uv,0.0);
    let own = weapon(in.uv);
    let tile_size = vec2<i32>(textureDimensions(tiles));
    let tile = clamp(vec2<i32>(in.uv * vec2<f32>(tile_size)),vec2(0),tile_size-vec2(1));
    let dominant = textureLoad(tiles,tile,0).xy;
    // A covered pixel follows its own surface, even when a nearby part moves faster.
    let velocity = select(dominant,own.xy,own.z > 0.0) * data.params.y;
    let pixel_motion = length(velocity * data.params.zw);
    if pixel_motion < 1.0 || data.params.x == 0.0 { return base; }
    let vector = velocity * min(1.0,data.params.x / max(length(velocity),0.00001));
    // Odd tap counts always include the current pixel: thin/slow surfaces cannot
    // lose all coverage merely because even taps straddle their narrow footprint.
    let count = i32(clamp(ceil(length(vector * data.params.zw)),16.0,96.0)) | 1;
    let world = textureSampleLevel(background,linear_sampler,in.uv,0.0);
    var sum = vec3(0.0);
    var coverage = 0.0;
    for (var i = 0; i < count; i++) {
        let offset = vector * ((f32(i)+0.5)/f32(count)-0.5);
        let uv = in.uv + offset;
        if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) { continue; }
        let sample = weapon(uv);
        let exposure = sample.xy * data.params.y;
        let distance = length(offset * data.params.zw);
        let reach = length(exposure * data.params.zw) * 0.5;
        if sample.z == 0.0 || reach < distance { continue; }
        // Reject motion perpendicular to this gather ray, including unrelated hand detail.
        let alignment = abs(dot(normalize(exposure+vec2(1e-12)),normalize(vector)));
        let weight = sample.z * smoothstep(0.5,0.9,alignment);
        sum += textureSampleLevel(frame,linear_sampler,uv,0.0).rgb * weight;
        coverage += weight;
    }
    let alpha = min(coverage/f32(count),1.0);
    if alpha == 0.0 { return select(world,base,own.z < 0.5); }
    let reconstructed = vec4(sum/f32(count) + world.rgb * (1.0-alpha),base.a);
    return mix(base, reconstructed, smoothstep(1.0,3.0,pixel_motion));
}
#else
// Read all surfaces, including sky/background motion. Reversed depth is larger nearer us.
fn surface(uv: vec2<f32>) -> vec3<f32> {
    let p = clamp(vec2<i32>(uv*data.params.zw),vec2(0),vec2<i32>(data.params.zw)-1);
#ifdef MULTISAMPLED
    var velocity = vec2(0.0);
    var z = 0.0;
    let n = textureNumSamples(motion);
    for (var i = 0u; i < n; i++) {
        let sample_z = textureLoad(depth,p,i32(i));
        if sample_z > 0.0 {
            velocity += textureLoad(motion,p,i32(i)).xy-camera_motion(uv,sample_z,data);
        }
        z = max(z,sample_z);
    }
    if z == 0.0 { return vec3(camera_motion(uv,z,data),z); }
    return vec3(velocity/f32(n)+camera_motion(uv,z,data),z);
#else
    let z = textureLoad(depth,p,0);
    if z == 0.0 { return vec3(camera_motion(uv,z,data),z); }
    return vec3(textureLoad(motion,p,0).xy,z);
#endif
}
fn limited(v: vec2<f32>) -> vec2<f32> {
    let exposed = v * data.params.y;
    return exposed * min(1.0,data.params.x/max(length(exposed),1e-8));
}
fn taps(vector: vec2<f32>) -> i32 {
    // Subpixel spacing for thin bright lights; odd counts retain the exact center.
    return i32(clamp(ceil(length(vector*data.params.zw)*1.5),16.0,256.0)) | 1;
}
fn camera_gather(uv: vec2<f32>, vector: vec2<f32>) -> vec3<f32> {
    let count = taps(vector);
    var sum = vec3(0.0);
    for (var i = 0; i < count; i++) {
        let offset = vector * (f32(i)/f32(count-1)-0.5);
        sum += textureSampleLevel(frame,linear_sampler,clamp(uv+offset,vec2(0.0),vec2(1.0)),0.0).rgb;
    }
    return sum/f32(count);
}
@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let base = textureSampleLevel(frame,linear_sampler,in.uv,0.0);
    if data.params.x == 0.0 { return base; }
    let own = surface(in.uv);
    let camera = camera_motion(in.uv,own.z,data);
    let relative = own.xy-camera;
    let tile_size = vec2<i32>(textureDimensions(tiles));
    let tile = clamp(vec2<i32>(in.uv*vec2<f32>(tile_size)),vec2(0),tile_size-1);
    let dominant = textureLoad(tiles,tile,0).xy;
    let camera_vector = limited(camera);
    let camera_pixels = length(camera_vector*data.params.zw);
    let object_pixels = length(dominant*data.params.zw);
    if object_pixels < 1.0 {
        if camera_pixels < 1.0 { return base; }
        return vec4(mix(base.rgb,camera_gather(in.uv,camera_vector),smoothstep(1.0,3.0,camera_pixels)),base.a);
    }
    let moving = length(relative*data.params.zw) >= 1.0;
    let vector = limited(select(camera+dominant,own.xy,moving));
    let pixels = length(vector*data.params.zw);
    if pixels < 1.0 { return base; }
    let count = taps(vector);
    var foreground = vec3(0.0);
    var coverage = 0.0;
    var behind = vec3(0.0);
    var background_weight = 0.0;
    for (var i = 0; i < count; i++) {
        let offset = vector * (f32(i)/f32(count-1)-0.5);
        let uv = in.uv+offset;
        if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) { continue; }
        let sample = surface(uv);
        let object = sample.xy-camera_motion(uv,sample.z,data);
        let color = textureSampleLevel(frame,linear_sampler,uv,0.0).rgb;
        if length(object*data.params.zw) < 1.0 {
            if !moving || sample.z <= own.z*1.01 { behind += color; background_weight += 1.0; }
            continue;
        }
        // A foreground wall must not receive streaks from an object behind it.
        if sample.z < own.z*0.99 && !moving { continue; }
        let exposure = limited(sample.xy);
        if length(offset*data.params.zw) > length(exposure*data.params.zw)*0.5+0.5 { continue; }
        let alignment = abs(dot(normalize(exposure+vec2(1e-12)),normalize(vector)));
        let weight = smoothstep(0.5,0.9,alignment);
        foreground += color*weight;
        coverage += weight;
    }
    var backdrop = base.rgb;
    if moving && background_weight > 0.0 { backdrop = behind/background_weight; }
    if !moving && camera_pixels >= 1.0 { backdrop = camera_gather(in.uv,camera_vector); }
    let alpha = min(coverage/f32(count),1.0);
    let reconstructed = foreground/f32(count)+backdrop*(1.0-alpha);
    return vec4(mix(base.rgb,reconstructed,smoothstep(1.0,3.0,pixels)),base.a);
}

#endif
#endif
#endif
