struct Camera {
    view: mat4x4<f32>,
    light_view: mat4x4<f32>,
    eye: vec4<f32>,
    sun: vec4<f32>,
    sky: vec4<f32>,
    lights: array<vec4<f32>,16>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(0) @binding(3) var tiles: texture_2d_array<f32>;
@group(0) @binding(4) var tile_sampler: sampler;
struct In { @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>, @location(3) tile: vec3<f32> };
struct Out { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>, @location(3) shadow: vec4<f32>, @location(4) tile: vec3<f32> };
@vertex fn vs(v:In)->Out {
    var o:Out; o.clip=camera.view*vec4(v.pos,1.0);o.world=v.pos;o.normal=v.normal;o.color=v.color;o.tile=v.tile;o.shadow=camera.light_view*vec4(v.pos,1.0);return o;
}
fn leaf_gap(world:vec3<f32>, normal:vec3<f32>, layer:f32)->bool {
    if(layer<14.5 || layer>17.5) {return false;}
    let axis=abs(normal);
    var uv=world.xy;
    if(axis.x>0.5) {uv=world.zy;}
    if(axis.y>0.5) {uv=world.xz;}
    let cell=floor(uv*16.0);
    return fract(sin(dot(cell,vec2(127.1,311.7)))*43758.5453)<0.28;
}
@fragment fn fs(v:Out)->@location(0) vec4<f32> {
    let p=v.shadow.xyz/v.shadow.w;
    let uv=vec2(p.x*0.5+0.5,0.5-p.y*0.5);
    // All fragments execute comparisons to satisfy uniform derivative rules.
    var shadow=0.0;
    for(var x=-1;x<=1;x=x+1) {for(var y=-1;y<=1;y=y+1) {
        shadow+=textureSampleCompare(shadow_map,shadow_sampler,uv+vec2(f32(x),f32(y))/2048.0,p.z-0.0015)/9.0;
    }}
    if(any(uv<vec2(0.0)) || any(uv>vec2(1.0)) || p.z>1.0 || p.z<0.0) {shadow=1.0;}
    let foliage=(v.tile.z>=14.5 && v.tile.z<=17.5) || v.tile.z < -2.5;
    let facing=dot(v.normal,camera.sun.xyz);
    let diffuse=(max(facing,0.0)+select(0.0,max(-facing,0.0)*0.55,foliage))*camera.sun.w*shadow;
    var light=vec3(0.15,0.19,0.25)+vec3(0.92,0.84,0.68)*diffuse;
    for(var i=0;i<16;i=i+1) {
        let l=camera.lights[i];let d=distance(v.world,l.xyz);let falloff=max(1.0-d/9.0,0.0);
        light+=vec3(1.0,0.49,0.16)*falloff*falloff*l.w;
    }
    let tile_color=textureSample(tiles,tile_sampler,v.tile.xy,i32(round(max(v.tile.z,0.0)))).rgb;
    let surface=select(vec3(1.0),tile_color,v.tile.z>=0.0);
    let color=select(v.color*surface*light,v.color,v.tile.z < -1.5 && v.tile.z > -2.5);
    let fog=smoothstep(112.0,160.0,distance(v.world.xz,camera.eye.xz));
    if(leaf_gap(v.world,v.normal,v.tile.z)) {discard;}
    return vec4(mix(color,camera.sky.xyz,fog),1.0);
}
struct ShadowOut {
    @builtin(position) clip:vec4<f32>,
    @location(0) world:vec3<f32>,
    @location(1) normal:vec3<f32>,
    @location(2) layer:f32,
};
@vertex fn shadow_vs(v:In)->ShadowOut {
    var o:ShadowOut;
    o.clip=camera.light_view*vec4(v.pos,1.0);
    o.world=v.pos;o.normal=v.normal;o.layer=v.tile.z;
    return o;
}
@fragment fn shadow_fs(v:ShadowOut) {
    if(leaf_gap(v.world,v.normal,v.layer)) {discard;}
}
