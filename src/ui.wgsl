struct Out { @builtin(position) pos:vec4<f32>, @location(0) color:vec4<f32> };
@vertex fn vs(@location(0) pos:vec2<f32>,@location(1) color:vec4<f32>)->Out {var o:Out;o.pos=vec4(pos,0.0,1.0);o.color=color;return o;}
@fragment fn fs(v:Out)->@location(0) vec4<f32> {return v.color;}
