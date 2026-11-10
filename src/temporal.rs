//! World-only temporal resolve and resolution-independent presentation.
use glam::Mat4;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    inverse: [[f32; 4]; 4],
    previous: [[f32; 4]; 4],
    eye_delta: [f32; 4],
    flags: [f32; 4],
}
pub struct Temporal {
    pub scene: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    history: [wgpu::TextureView; 2],
    resolve: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    #[cfg(feature = "native-upscalers")]
    velocity: wgpu::RenderPipeline,
    #[cfg(feature = "native-upscalers")]
    depth_copy: wgpu::RenderPipeline,
    #[cfg(feature = "native-upscalers")]
    previous_jitter: glam::Vec2,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    previous: Mat4,
    previous_eye: glam::Vec3,
    pub valid: bool,
    pub frame: u32,
    pub size: (u32, u32),
}
pub fn color(
    device: &wgpu::Device,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
impl Temporal {
    pub fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        output: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Self {
        let scene = color(
            device,
            size,
            wgpu::TextureFormat::Rgba16Float,
            "Scene color",
        );
        let depth = super::render::depth(device, size.0, size.1, "Scene depth");
        let history = std::array::from_fn(|_| {
            color(
                device,
                output,
                wgpu::TextureFormat::Rgba16Float,
                "Temporal history",
            )
        });
        let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Temporal layout"),
            entries: &[
                texture(0, wgpu::TextureSampleType::Float { filterable: true }),
                texture(1, wgpu::TextureSampleType::Float { filterable: true }),
                texture(2, wgpu::TextureSampleType::Depth),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Temporal params"),
            contents: bytemuck::bytes_of(&Params::zero()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Temporal shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("temporal.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |entry, target| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let resolve = pipeline("resolve", wgpu::TextureFormat::Rgba16Float);
        let present = pipeline("present", format);
        #[cfg(feature = "native-upscalers")]
        let velocity = pipeline("velocity", wgpu::TextureFormat::Rg16Float);
        #[cfg(feature = "native-upscalers")]
        let depth_copy = pipeline("depth_copy", wgpu::TextureFormat::R32Float);
        Self {
            scene,
            #[cfg(feature = "native-upscalers")]
            velocity,
            #[cfg(feature = "native-upscalers")]
            depth_copy,
            #[cfg(feature = "native-upscalers")]
            previous_jitter: glam::Vec2::ZERO,
            depth,
            history,
            resolve,
            present,
            layout,
            sampler,
            uniform,
            previous: Mat4::IDENTITY,
            previous_eye: glam::Vec3::ZERO,
            valid: false,
            frame: 0,
            size,
        }
    }
    pub fn jitter(&self, enabled: bool) -> glam::Vec2 {
        if !enabled {
            return glam::Vec2::ZERO;
        }
        let halton = |mut n: u32, base: u32| {
            let mut result = 0.;
            let mut f = 1.;
            while n > 0 {
                f /= base as f32;
                result += f * (n % base) as f32;
                n /= base;
            }
            result - 0.5
        };
        let n = self.frame % 16 + 1;
        glam::Vec2::new(
            2. * halton(n, 2) / self.size.0 as f32,
            2. * halton(n, 3) / self.size.1 as f32,
        )
    }
    fn pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        source: &wgpu::TextureView,
        history: &wgpu::TextureView,
        pipeline: &wgpu::RenderPipeline,
    ) {
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Resolve bindings"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(history),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Resolve"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
    #[cfg(feature = "native-upscalers")]
    #[allow(clippy::too_many_arguments)]
    pub fn native(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        camera: Mat4,
        eye: glam::Vec3,
        native: &mut crate::native_upscale::Native,
        dt: f32,
    ) -> Result<(), String> {
        let delta = self.previous_eye - eye;
        let jitter = self.jitter(true);
        let correction = (jitter - self.previous_jitter) * glam::Vec2::new(0.5, -0.5);
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Params {
                inverse: camera.inverse().to_cols_array_2d(),
                previous: self.previous.to_cols_array_2d(),
                eye_delta: [delta.x, delta.y, delta.z, 0.],
                flags: [0., correction.x, correction.y, 0.],
            }),
        );
        self.pass(
            device,
            encoder,
            &native.linear_depth,
            &self.scene,
            &self.history[0],
            &self.depth_copy,
        );
        self.pass(
            device,
            encoder,
            &native.motion,
            &self.scene,
            &self.history[0],
            &self.velocity,
        );
        let pixel_jitter =
            jitter * glam::Vec2::new(self.size.0 as f32 * 0.5, -(self.size.1 as f32) * 0.5);
        native.render(
            encoder,
            &self.scene,
            &self.depth,
            pixel_jitter,
            !self.valid || delta.length_squared() > 16.,
            dt,
        )?;
        self.pass(
            device,
            encoder,
            output,
            &native.output,
            &self.history[0],
            &self.present,
        );
        self.previous = camera;
        self.previous_eye = eye;
        self.previous_jitter = jitter;
        self.valid = true;
        self.frame = self.frame.wrapping_add(1);
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        stable_camera: Mat4,
        eye: glam::Vec3,
        enabled: bool,
    ) {
        let jitter = self.jitter(enabled);
        let camera = Mat4::from_translation(jitter.extend(0.)) * stable_camera;
        let delta = self.previous_eye - eye;
        let valid = enabled && self.valid && delta.length_squared() < 16.;
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Params {
                inverse: camera.inverse().to_cols_array_2d(),
                previous: self.previous.to_cols_array_2d(),
                eye_delta: [delta.x, delta.y, delta.z, 0.],
                flags: [
                    valid as u32 as f32,
                    jitter.x * 0.5,
                    -jitter.y * 0.5,
                    stationary_history(valid, stable_camera, self.previous, eye, self.previous_eye)
                        as u32 as f32,
                ],
            }),
        );
        let index = (self.frame % 2) as usize;
        for (target, source, history, pipeline) in [
            (
                &self.history[index],
                &self.scene,
                &self.history[1 - index],
                &self.resolve,
            ),
            (
                output,
                &self.history[index],
                &self.history[1 - index],
                &self.present,
            ),
        ] {
            self.pass(device, encoder, target, source, history, pipeline);
        }
        self.previous = stable_camera;
        self.previous_eye = eye;
        self.valid = true;
        self.frame = self.frame.wrapping_add(1);
    }
}
impl Params {
    fn zero() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

// Only an identical view has identity screen reprojection. Any motion must use
// depth-tested reprojection, including subpixel motion and history resets.
fn stationary_history(
    valid: bool,
    camera: Mat4,
    previous: Mat4,
    eye: glam::Vec3,
    previous_eye: glam::Vec3,
) -> bool {
    valid && camera == previous && eye == previous_eye
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stationary_history_requires_identical_view_and_valid_history() {
        let eye = glam::Vec3::new(220., 31., -233.);
        let camera = Mat4::perspective_rh(75_f32.to_radians(), 1.6, 0.05, 1024.)
            * Mat4::look_at_rh(eye, eye + glam::Vec3::Z, glam::Vec3::Y);
        assert!(stationary_history(true, camera, camera, eye, eye));
        assert!(!stationary_history(false, camera, camera, eye, eye));
        assert!(!stationary_history(
            true,
            camera,
            camera,
            eye + glam::Vec3::X * 0.0001,
            eye
        ));
        assert!(!stationary_history(
            true,
            Mat4::from_rotation_y(0.00001) * camera,
            camera,
            eye,
            eye
        ));
        assert!(!stationary_history(
            true,
            Mat4::from_scale(glam::Vec3::new(1.001, 1., 1.)) * camera,
            camera,
            eye,
            eye
        ));
    }
}
