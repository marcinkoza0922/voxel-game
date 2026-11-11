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

fn fxaa_luma(uv: vec2<f32>) -> f32 {
    let rgb = textureSampleLevel(current, linear_sampler, uv, 0.).rgb;
    return dot(sqrt(clamp(rgb, vec3(0.), vec3(1.))), vec3(0.2126, 0.7152, 0.0722));
}

@fragment fn fxaa(v: Out) -> @location(0) vec4<f32> {
    let texel = 1. / vec2<f32>(textureDimensions(current));
    let x = vec2(texel.x, 0.);
    let y = vec2(0., texel.y);
    let m = fxaa_luma(v.uv);
    let n = fxaa_luma(v.uv - y);
    let s = fxaa_luma(v.uv + y);
    let w = fxaa_luma(v.uv - x);
    let e = fxaa_luma(v.uv + x);
    let high = max(m, max(max(n, s), max(w, e)));
    let low = min(m, min(min(n, s), min(w, e)));
    let contrast = high - low;
    if contrast < max(0.0312, high * 0.125) {
        return vec4(textureSampleLevel(current, linear_sampler, v.uv, 0.).rgb, 1.);
    }

    let nw = fxaa_luma(v.uv - x - y);
    let ne = fxaa_luma(v.uv + x - y);
    let sw = fxaa_luma(v.uv - x + y);
    let se = fxaa_luma(v.uv + x + y);
    let horizontal = 2. * abs(n + s - 2. * m) + abs(nw + sw - 2. * w) + abs(ne + se - 2. * e);
    let vertical = 2. * abs(w + e - 2. * m) + abs(nw + ne - 2. * n) + abs(sw + se - 2. * s);
    let along_x = horizontal >= vertical;
    let along = select(y, x, along_x);
    let across = select(x, y, along_x);
    let negative = select(w, n, along_x);
    let positive = select(e, s, along_x);
    let negative_steeper = abs(negative - m) >= abs(positive - m);
    let neighbor = select(positive, negative, negative_steeper);
    let normal = select(across, -across, negative_steeper);
    let edge_luma = (m + neighbor) * 0.5;
    let gradient_threshold = abs(m - neighbor) * 0.25;
    let edge_center = v.uv + normal * 0.5;

    // Search along both sides of the edge for a change in contrast.
    var distances = vec2(12.);
    var end_delta = vec2(0.);
    for (var side = 0u; side < 2u; side++) {
        let direction = select(along, -along, side == 0u);
        for (var step = 1.; step <= 8.; step += 1.) {
            let sample_uv = clamp(edge_center + direction * step, texel * 0.5, 1. - texel * 0.5);
            let delta = fxaa_luma(sample_uv) - edge_luma;
            if abs(delta) >= gradient_threshold {
                distances[side] = step;
                end_delta[side] = delta;
                break;
            }
        }
    }
    let use_negative = distances.x < distances.y;
    let nearest_delta = select(end_delta.y, end_delta.x, use_negative);
    let nearest_distance = min(distances.x, distances.y);
    var edge_blend = 0.;
    if nearest_delta != 0. && ((nearest_delta < 0.) != (m < edge_luma)) {
        edge_blend = 0.5 - nearest_distance / (distances.x + distances.y);
    }

    let average = (2. * (n + s + w + e) + nw + ne + sw + se) / 12.;
    let pixel_blend = smoothstep(0., 1., abs(average - m) / contrast);
    let offset = max(edge_blend, pixel_blend * pixel_blend * 0.75);
    return vec4(textureSampleLevel(current, linear_sampler, v.uv + normal * offset, 0.).rgb, 1.);
}
