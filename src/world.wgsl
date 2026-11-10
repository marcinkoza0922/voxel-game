struct Camera {
    view: mat4x4<f32>,
    light_view: mat4x4<f32>,
    eye: vec4<f32>,
    sun: vec4<f32>,
    sky: vec4<f32>,
    lights: array<vec4<f32>,16>,
    inverse_view: mat4x4<f32>,
    celestial: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(0) @binding(3) var tiles: texture_2d_array<f32>;
@group(0) @binding(4) var tile_sampler: sampler;
// Shared atmosphere keeps distance fog continuous with the skybox.
fn atmosphere(ray:vec3<f32>)->vec3<f32> {
    let day=camera.sky.x;
    let altitude=sqrt(max(ray.y,0.0));
    let horizon=mix(vec3(0.025,0.035,0.075),vec3(0.56,0.70,0.82),day);
    let zenith=mix(vec3(0.004,0.008,0.025),vec3(0.10,0.32,0.65),day);
    let dusk=exp(-abs(camera.celestial.y)*12.0);
    let glow=pow(max(dot(ray,camera.celestial.xyz),0.0),8.0);
    return mix(horizon,zenith,altitude)+vec3(0.65,0.19,0.055)*dusk*glow*(1.0-altitude);
}
struct SkyOut {
    @builtin(position) clip:vec4<f32>,
    @location(0) ndc:vec2<f32>,
};
@vertex fn sky_vs(@builtin(vertex_index) index:u32)->SkyOut {
    let positions=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
    var o:SkyOut;
    o.ndc=positions[index];o.clip=vec4(o.ndc,1.0,1.0);return o;
}
@fragment fn sky_fs(v:SkyOut)->@location(0) vec4<f32> {
    let world=camera.inverse_view*vec4(v.ndc,1.0,1.0);
    let ray=normalize(world.xyz/world.w-camera.eye.xyz);
    var color=atmosphere(ray);
    // Fixed spherical cells keep stars attached to the sky as the player moves.
    let uv=vec2(atan2(ray.z,ray.x),asin(clamp(ray.y,-1.0,1.0)))*180.0;
    let cell=floor(uv);
    let hash=fract(sin(dot(cell,vec2(127.1,311.7)))*43758.5453);
    let star=(1.0-smoothstep(0.04,0.20,length(fract(uv)-vec2(0.5))))*step(0.985,hash);
    color+=vec3(0.7,0.8,1.0)*star*(1.0-camera.sky.x)*smoothstep(0.0,0.15,ray.y);
    let sun= smoothstep(0.99965,0.99985,dot(ray,camera.celestial.xyz));
    let moon=smoothstep(0.9994,0.99965,dot(ray,-camera.celestial.xyz));
    let above=smoothstep(-0.02,0.01,ray.y);
    color=mix(color,vec3(1.0,0.88,0.56),sun*above);
    color=mix(color,vec3(0.65,0.75,0.9),moon*above);
    // Unmeshed cave space must fade to darkness, including celestial bodies.
    return vec4(color*camera.sky.z,1.0);
}
struct In { @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>, @location(3) tile: vec3<f32>, @location(4) sky_access:f32 };
struct Out { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>, @location(3) shadow: vec4<f32>, @location(4) tile: vec3<f32>, @location(5) sky_access:f32 };
@vertex fn vs(v:In)->Out {
    var o:Out; o.clip=camera.view*vec4(v.pos,1.0);o.world=v.pos;o.normal=v.normal;o.color=v.color;o.tile=v.tile;o.sky_access=v.sky_access;o.shadow=camera.light_view*vec4(v.pos,1.0);return o;
}
fn leaf_gap(world:vec3<f32>, normal:vec3<f32>, layer:f32)->bool {
    if((layer<14.5 || layer>17.5) && (layer > -3.5 || layer < -4.5)) {return false;}
    let axis=abs(normal);
    var uv=world.xy;
    if(axis.x>0.5) {uv=world.zy;}
    if(axis.y>0.5) {uv=world.xz;}
    let cell=floor(uv*select(16.0,32.0,layer < -3.5));
    return fract(sin(dot(cell,vec2(127.1,311.7)))*43758.5453)<select(0.28,0.36,layer < -3.5);
}
fn shade(v:Out)->vec4<f32> {
    let p=v.shadow.xyz/v.shadow.w;
    let uv=vec2(p.x*0.5+0.5,0.5-p.y*0.5);
    // All fragments execute comparisons to satisfy uniform derivative rules.
    var shadow=0.0;
    for(var x=-1;x<=1;x=x+1) {for(var y=-1;y<=1;y=y+1) {
        shadow+=textureSampleCompare(shadow_map,shadow_sampler,uv+vec2(f32(x),f32(y))/2048.0,p.z-0.0015)/9.0;
    }}
    if(any(uv<vec2(0.0)) || any(uv>vec2(1.0)) || p.z>1.0 || p.z<0.0) {shadow=1.0;}
    let foliage=(v.tile.z>=14.5 && v.tile.z<=17.5) || (v.tile.z < -2.5 && v.tile.z > -4.5);
    let facing=dot(v.normal,camera.sun.xyz);
    let diffuse=(max(facing,0.0)+select(0.0,max(-facing,0.0)*0.55,foliage))*camera.sun.w*shadow;
    let day=camera.sky.x;
    let ambient=mix(vec3(0.0005,0.0008,0.0015),vec3(0.12,0.16,0.21),day);
    let tint=mix(vec3(0.40,0.55,0.85),vec3(0.92,0.84,0.68),day);
    let sky_access=clamp(v.sky_access,0.0,1.0);
    var light=(ambient+tint*diffuse)*sky_access;
    for(var i=0;i<16;i=i+1) {
        let l=camera.lights[i];let d=distance(v.world,l.xyz);let falloff=max(1.0-d/9.0,0.0);
        light+=vec3(1.0,0.49,0.16)*falloff*falloff*l.w;
    }
    let tile_color=textureSample(tiles,tile_sampler,v.tile.xy,i32(round(max(v.tile.z,0.0)))).rgb;
    let surface=select(vec3(1.0),tile_color,v.tile.z>=0.0);
    var color=select(v.color*surface*light,v.color,v.tile.z < -1.5 && v.tile.z > -2.5);
    if(v.tile.z < -4.5) {
        let ripple=0.92+0.08*sin(v.world.x*7.0+v.world.z*5.0+camera.sky.y*900.0);
        color*=ripple;
        if(v.tile.z < -5.5) {color=v.color*ripple;}
    }
    let fog=smoothstep(112.0,160.0,distance(v.world.xz,camera.eye.xz));
    if(leaf_gap(v.world,v.normal,v.tile.z)) {discard;}
    return vec4(mix(color,atmosphere(normalize(v.world-camera.eye.xyz))*min(sky_access,camera.sky.z),fog),1.0);
}
@fragment fn fs(v:Out)->@location(0) vec4<f32> {
    let color=shade(v);
    if(v.tile.z < -4.5) {discard;}
    return color;
}
@fragment fn fluid_fs(v:Out)->@location(0) vec4<f32> {
    let color=shade(v);
    if(v.tile.z > -4.5) {discard;}
    return vec4(color.rgb,select(0.62,1.0,v.tile.z < -5.5));
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
    if(v.layer < -4.5) {discard;}
    if(leaf_gap(v.world,v.normal,v.layer)) {discard;}
}
