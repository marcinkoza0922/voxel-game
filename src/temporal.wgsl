struct Params { inverse: mat4x4<f32>, previous: mat4x4<f32>, eye_delta: vec4<f32>, flags: vec4<f32> };
@group(0) @binding(0) var current: texture_2d<f32>;
@group(0) @binding(1) var history: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_depth_2d;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var<uniform> params: Params;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) index: u32) -> Out {
    let p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    var o: Out; o.position = vec4(p[index],0.,1.); o.uv = p[index]*vec2(0.5,-0.5)+0.5; return o;
}
@fragment fn resolve(v: Out) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(current));
    // Resolve to a fixed output grid; scene samples carry projection jitter.
    let scene_uv = v.uv + params.flags.yz;
    let pixel = clamp(vec2<i32>(scene_uv*vec2<f32>(size)),vec2(0),size-1);
    let d = textureLoad(depth,pixel,0);
    let now = textureSampleLevel(current,linear_sampler,scene_uv,0.).rgb;
    var world = params.inverse*vec4(scene_uv*vec2(2.,-2.)+vec2(-1.,1.),d,1.);
    world /= world.w;
    // Sky reprojection ignores camera translation.
    if d >= 0.999999 { world = vec4(world.xyz+params.eye_delta.xyz,1.); }
    let previous = params.previous*world;
    let stationary = params.flags.w > 0.5;
    // At rest, output pixels map exactly to themselves. Reconstructing this
    // identity through world coordinates introduces float error and edge depth
    // rejection when different jitter samples land on opposite silhouettes.
    let uv = select(previous.xy/previous.w*vec2(0.5,-0.5)+0.5,v.uv,stationary);
    let old = textureSampleLevel(history,linear_sampler,uv,0.);
    var low = now; var high = now;
    for (var y = -1; y <= 1; y++) { for (var x = -1; x <= 1; x++) {
        let c = textureLoad(current,clamp(pixel+vec2(x,y),vec2(0),size-1),0).rgb;
        low = min(low,c); high = max(high,c);
    }}
    let accepted = params.flags.x > 0.5 && previous.w > 0. && all(uv > vec2(0.)) && all(uv < vec2(1.)) && (stationary || abs(old.a-previous.z/previous.w) < 0.001);
    // Neighborhood clipping rejects stale silhouettes and moving-object trails.
    // A fixed view can retain subpixel edge coverage across the jitter cycle.
    // Keep tighter rejection while moving to avoid trails and disocclusion ghosts.
    let margin = select(0.,0.03,stationary);
    let clipped = clamp(old.rgb,low-vec3(margin),high+vec3(margin));
    let difference = length(now-clipped);
    let retained = select(clamp(0.9-difference*2.,0.1,0.9),0.95,stationary);
    let weight = select(0.,retained,accepted);
    return vec4(mix(now,clipped,weight),d);
}
@fragment fn present(v: Out) -> @location(0) vec4<f32> {
    return vec4(textureSampleLevel(current,linear_sampler,v.uv,0.).rgb,1.);
}

@fragment fn velocity(v: Out) -> @location(0) vec2<f32> {
    // An unchanged camera has exactly zero motion, independently of jitter.
    // Avoid inverse-projection error being accumulated as apparent movement.
    if params.flags.w > 0.5 { return vec2(0.); }
    let size = vec2<i32>(textureDimensions(depth));
    let d = textureLoad(depth,clamp(vec2<i32>(v.uv*vec2<f32>(size)),vec2(0),size-1),0);
    var world = params.inverse*vec4(v.uv*vec2(2.,-2.)+vec2(-1.,1.),d,1.);
    world /= world.w;
    if d >= 0.999999 { world = vec4(world.xyz+params.eye_delta.xyz,1.); }
    let previous = params.previous*world;
    // Previous projection is unjittered; remove current geometry jitter from
    // the current UV as well. SDK motion must not contain projection jitter.
    return previous.xy/previous.w*vec2(0.5,-0.5)+0.5-v.uv+params.flags.yz;
}

@fragment fn depth_copy(v: Out) -> @location(0) f32 {
    let size = vec2<i32>(textureDimensions(depth));
    return textureLoad(depth,clamp(vec2<i32>(v.uv*vec2<f32>(size)),vec2(0),size-1),0);
}
