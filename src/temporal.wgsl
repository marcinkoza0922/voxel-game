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
    let pixel = clamp(vec2<i32>(v.uv*vec2<f32>(size)),vec2(0),size-1);
    let d = textureLoad(depth,pixel,0);
    let now = textureSampleLevel(current,linear_sampler,v.uv,0.).rgb;
    var world = params.inverse*vec4(v.uv*vec2(2.,-2.)+vec2(-1.,1.),d,1.);
    world /= world.w;
    // Sky reprojection ignores camera translation.
    if d >= 0.999999 { world = vec4(world.xyz+params.eye_delta.xyz,1.); }
    let previous = params.previous*world;
    let uv = previous.xy/previous.w*vec2(0.5,-0.5)+0.5;
    let old = textureSampleLevel(history,linear_sampler,uv,0.);
    var low = now; var high = now;
    for (var y = -1; y <= 1; y++) { for (var x = -1; x <= 1; x++) {
        let c = textureLoad(current,clamp(pixel+vec2(x,y),vec2(0),size-1),0).rgb;
        low = min(low,c); high = max(high,c);
    }}
    let accepted = params.flags.x > 0.5 && previous.w > 0. && all(uv > vec2(0.)) && all(uv < vec2(1.)) && abs(old.a-previous.z/previous.w) < 0.001;
    // Neighborhood clipping rejects stale silhouettes and moving-object trails.
    let clipped = clamp(old.rgb,low,high);
    let difference = length(now-clipped);
    let weight = select(0.,clamp(0.9-difference*2.,0.1,0.9),accepted);
    return vec4(mix(now,clipped,weight),d);
}
@fragment fn present(v: Out) -> @location(0) vec4<f32> {
    return vec4(textureSampleLevel(current,linear_sampler,v.uv,0.).rgb,1.);
}

@fragment fn velocity(v: Out) -> @location(0) vec2<f32> {
    let size = vec2<i32>(textureDimensions(depth));
    let d = textureLoad(depth,clamp(vec2<i32>(v.uv*vec2<f32>(size)),vec2(0),size-1),0);
    var world = params.inverse*vec4(v.uv*vec2(2.,-2.)+vec2(-1.,1.),d,1.);
    world /= world.w;
    if d >= 0.999999 { world = vec4(world.xyz+params.eye_delta.xyz,1.); }
    let previous = params.previous*world;
    return previous.xy/previous.w*vec2(0.5,-0.5)+0.5-v.uv+params.flags.yz;
}

@fragment fn depth_copy(v: Out) -> @location(0) f32 {
    let size = vec2<i32>(textureDimensions(depth));
    return textureLoad(depth,clamp(vec2<i32>(v.uv*vec2<f32>(size)),vec2(0),size-1),0);
}
