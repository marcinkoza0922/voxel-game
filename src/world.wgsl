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
struct In { @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32> };
struct Out { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>, @location(3) shadow: vec4<f32> };
@vertex fn vs(v:In)->Out {
    var o:Out; o.clip=camera.view*vec4(v.pos,1.0);o.world=v.pos;o.normal=v.normal;o.color=v.color;o.shadow=camera.light_view*vec4(v.pos,1.0);return o;
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
    let diffuse=max(dot(v.normal,camera.sun.xyz),0.0)*camera.sun.w*shadow;
    var light=vec3(0.15,0.19,0.25)+vec3(0.92,0.84,0.68)*diffuse;
    for(var i=0;i<16;i=i+1) {
        let l=camera.lights[i];let d=distance(v.world,l.xyz);let falloff=max(1.0-d/9.0,0.0);
        light+=vec3(1.0,0.49,0.16)*falloff*falloff*l.w;
    }
    let color=v.color*light;
    let fog=smoothstep(112.0,160.0,distance(v.world.xz,camera.eye.xz));
    return vec4(mix(color,camera.sky.xyz,fog),1.0);
}
@vertex fn shadow_vs(v:In)->@builtin(position) vec4<f32> {return camera.light_view*vec4(v.pos,1.0);}
