use crate::{
    game::Game,
    ui::{Ui, UiVertex},
    world::{Block, SIZE, World},
};
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3];
impl Vertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: 36,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Camera {
    view: [[f32; 4]; 4],
    light_view: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    sky: [f32; 4],
    lights: [[f32; 4]; 16],
}
struct Mesh {
    buffer: wgpu::Buffer,
    count: u32,
}
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    shadow_bind: wgpu::BindGroup,
    depth: wgpu::TextureView,
    shadow: wgpu::TextureView,
    meshes: HashMap<IVec3, Mesh>,
    pending: VecDeque<IVec3>,
    queued: HashSet<IVec3>,
    pub ui: Ui,
    pub width: u32,
    pub height: u32,
}
fn depth(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = instance.create_surface(window).map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| e.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Voxel device"),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let depth = depth(&device, config.width, config.height, "Main depth");
        let shadow = crate::render::depth(&device, 2048, 2048, "Sun shadow");
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera"),
            size: std::mem::size_of::<Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow layout"),
            entries: &[entry],
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("World layout"),
            entries: &[
                entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("World bindings"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow bindings"),
            layout: &shadow_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("World shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&shadow_layout],
                push_constant_ranges: &[],
            });
        let ds = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("World pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(ds.clone()),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sun depth pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shadow_vs"),
                compilation_options: Default::default(),
                buffers: &[Vertex::layout()],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.,
                    clamp: 0.,
                },
                ..ds
            }),
            multisample: Default::default(),
            fragment: None,
            multiview: None,
            cache: None,
        });
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GUI shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GUI pipeline"),
            layout: Some(&ui_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x4],
                }],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        Ok(Self {
            surface,
            device,
            queue,
            width: config.width,
            height: config.height,
            config,
            pipeline,
            shadow_pipeline,
            ui_pipeline,
            uniform,
            bind,
            shadow_bind,
            depth,
            shadow,
            meshes: HashMap::new(),
            pending: VecDeque::new(),
            queued: HashSet::new(),
            ui: Ui::new(),
        })
    }
    pub fn finish(self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        drop(self);
    }
    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.width = w;
        self.height = h;
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth = depth(&self.device, w, h, "Main depth");
    }
    pub fn update_world(&mut self, w: &mut World, pos: Vec3) {
        for k in w.stream(pos, 5) {
            self.meshes.remove(&k);
        }
        for k in w.dirty.drain(..) {
            if self.queued.insert(k) {
                self.pending.push_back(k);
            }
        }
        for _ in 0..3 {
            if let Some(k) = self.pending.pop_front() {
                self.queued.remove(&k);
                if !w.loaded(k) {
                    continue;
                }
                let data = mesh_chunk(w, k);
                if data.is_empty() {
                    self.meshes.remove(&k);
                } else {
                    self.meshes.insert(
                        k,
                        Mesh {
                            buffer: self.device.create_buffer_init(
                                &wgpu::util::BufferInitDescriptor {
                                    label: Some("Chunk mesh"),
                                    contents: bytemuck::cast_slice(&data),
                                    usage: wgpu::BufferUsages::VERTEX,
                                },
                            ),
                            count: data.len() as u32,
                        },
                    );
                }
            }
        }
    }
    pub fn draw(
        &mut self,
        g: &Game,
        pack: bool,
        paused: bool,
        fps: f32,
        capture: Option<&std::path::Path>,
    ) -> Result<(), String> {
        let daylight =
            ((g.time.fract() * std::f32::consts::TAU).sin() * 0.8 + 0.25).clamp(0.03, 1.);
        let sun = Vec3::new(0.45, 0.8, 0.3).normalize();
        let sky = [
            0.28 * daylight + 0.018,
            0.45 * daylight + 0.025,
            0.59 * daylight + 0.045,
            1.,
        ];
        let eye = g.player.eye();
        let view = Mat4::perspective_rh(
            75_f32.to_radians(),
            self.width as f32 / self.height as f32,
            0.05,
            240.,
        ) * Mat4::look_at_rh(eye, eye + g.player.direction(), Vec3::Y);
        let center = eye.floor();
        let light_view = Mat4::orthographic_rh(-80., 80., -80., 80., 0.1, 250.)
            * Mat4::look_at_rh(center + sun * 120., center, Vec3::Y);
        let mut lights = [[0.; 4]; 16];
        let mut torches: Vec<_> = g
            .torch_life
            .keys()
            .map(|p| Vec3::from_array(p.map(|n| n as f32)) + Vec3::splat(0.5))
            .collect();
        torches.sort_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        let held = g.player.inventory.selected() == crate::items::Item::Torch
            && g.player.inventory.count(crate::items::Item::Torch) > 0;
        let mut n = 0;
        if held {
            lights[0] = [eye.x, eye.y, eye.z, 1.];
            n = 1;
        }
        for p in torches.into_iter().take(16 - n) {
            lights[n] = [p.x, p.y, p.z, 1.];
            n += 1;
        }
        let camera = Camera {
            view: view.to_cols_array_2d(),
            light_view: light_view.to_cols_array_2d(),
            eye: [eye.x, eye.y, eye.z, 1.],
            sun: [sun.x, sun.y, sun.z, daylight],
            sky,
            lights,
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&camera));
        let mut dynamic = vec![];
        for (_, m) in g.entities.iter() {
            let p = m.pos;
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.27, 0.65, -0.19),
                Vec3::new(0.54, 0.73, 0.38),
                [0.27, 0.33, 0.24],
            );
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.23, 1.38, -0.23),
                Vec3::splat(0.46),
                [0.48, 0.49, 0.33],
            );
            for x in [-0.25, 0.05] {
                box_mesh(
                    &mut dynamic,
                    p + Vec3::new(x, 0., -0.13),
                    Vec3::new(0.20, 0.7, 0.26),
                    [0.25, 0.23, 0.25],
                );
            }
            for x in [-0.40, 0.26] {
                box_mesh(
                    &mut dynamic,
                    p + Vec3::new(x, 0.8, -0.12),
                    Vec3::new(0.14, 0.55, 0.25),
                    [0.36, 0.41, 0.29],
                );
            }
        }
        if let Some((p, _)) = g.target {
            let p = p.as_vec3() - Vec3::splat(0.005);
            let t = 0.012;
            for axis in 0..3 {
                let a = (axis + 1) % 3;
                let b = (axis + 2) % 3;
                for i in [0., 1.] {
                    for j in [0., 1.] {
                        let mut origin = p;
                        origin[a] += i;
                        origin[b] += j;
                        let mut size = Vec3::splat(t);
                        size[axis] = 1.01;
                        box_mesh(&mut dynamic, origin, size, [0.95, 0.83, 0.5]);
                    }
                }
            }
        }
        let dynamic_buffer = (!dynamic.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Creatures and target"),
                    contents: bytemuck::cast_slice(&dynamic),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        self.ui.build(g, pack, paused, self.width, self.height, fps);
        let ui_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GUI geometry"),
                contents: bytemuck::cast_slice(&self.ui.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.shadow_bind, &[]);
            for m in self.meshes.values() {
                pass.set_vertex_buffer(0, m.buffer.slice(..));
                pass.draw(0..m.count, 0..1);
            }
            if let Some(b) = &dynamic_buffer {
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..dynamic.len() as u32, 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("World pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky[0] as f64,
                            g: sky[1] as f64,
                            b: sky[2] as f64,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            for m in self.meshes.values() {
                pass.set_vertex_buffer(0, m.buffer.slice(..));
                pass.draw(0..m.count, 0..1);
            }
            if let Some(b) = &dynamic_buffer {
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..dynamic.len() as u32, 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GUI pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.ui_pipeline);
            pass.set_vertex_buffer(0, ui_buffer.slice(..));
            pass.draw(0..self.ui.vertices.len() as u32, 0..1);
        }
        let readback = capture.map(|_| {
            let stride = (self.width * 4).div_ceil(256) * 256;
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Screenshot readback"),
                size: (stride * self.height) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                frame.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(self.height),
                    },
                },
                wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
            );
            (buffer, stride)
        });
        self.queue.submit(Some(encoder.finish()));
        if let (Some(path), Some((buffer, stride))) = (capture, readback) {
            let (tx, rx) = std::sync::mpsc::channel();
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            rx.recv()
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let data = buffer.slice(..).get_mapped_range();
            let mut ppm = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
            let bgra = matches!(
                self.config.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            );
            for row in data.chunks(stride as usize).take(self.height as usize) {
                for pixel in row[..self.width as usize * 4].as_chunks::<4>().0 {
                    if bgra {
                        ppm.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                    } else {
                        ppm.extend_from_slice(&pixel[..3]);
                    }
                }
            }
            std::fs::write(path, ppm).map_err(|e| e.to_string())?;
            drop(data);
            buffer.unmap();
        }
        frame.present();
        Ok(())
    }
}

const FACES: [(IVec3, IVec3, IVec3); 6] = [
    (IVec3::X, IVec3::Z, IVec3::Y),
    (IVec3::NEG_X, IVec3::NEG_Z, IVec3::Y),
    (IVec3::Y, IVec3::X, IVec3::Z),
    (IVec3::NEG_Y, IVec3::X, IVec3::NEG_Z),
    (IVec3::Z, IVec3::NEG_X, IVec3::Y),
    (IVec3::NEG_Z, IVec3::X, IVec3::Y),
];
const CORNERS: [(i32, i32); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
pub fn mesh_chunk(w: &World, k: IVec3) -> Vec<Vertex> {
    let base = k * SIZE;
    // One-block halo avoids repeated terrain queries during face and AO lookup.
    let mut halo = vec![Block::Air; 18 * 18 * 18];
    let idx = |p: IVec3| ((p.x + 1) + 18 * ((p.z + 1) + 18 * (p.y + 1))) as usize;
    for x in -1..=16 {
        for z in -1..=16 {
            for y in -1..=16 {
                let p = IVec3::new(x, y, z);
                halo[idx(p)] = w.get(base + p);
            }
        }
    }
    let get = |p: IVec3| halo[idx(p)];
    let mut out = vec![];
    for x in 0..SIZE {
        for z in 0..SIZE {
            for y in 0..SIZE {
                let p = IVec3::new(x, y, z);
                let block = get(p);
                if block == Block::Air {
                    continue;
                }
                if block == Block::Torch {
                    box_mesh(
                        &mut out,
                        (base + p).as_vec3() + Vec3::new(0.41, 0., 0.41),
                        Vec3::new(0.18, 0.8, 0.18),
                        block.color(),
                    );
                    continue;
                }
                for (normal, u, v) in FACES {
                    if get(p + normal).solid() {
                        continue;
                    }
                    let mut vertices = [Vertex {
                        pos: [0.; 3],
                        normal: normal.as_vec3().to_array(),
                        color: block.color(),
                    }; 4];
                    let mut aos = [0; 4];
                    for (i, (a, b)) in CORNERS.iter().enumerate() {
                        let side1 = get(p + normal + u * *a).solid() as u8;
                        let side2 = get(p + normal + v * *b).solid() as u8;
                        let corner = get(p + normal + u * *a + v * *b).solid() as u8;
                        let ao = if side1 + side2 == 2 {
                            3
                        } else {
                            side1 + side2 + corner
                        };
                        aos[i] = ao;
                        let pos = (base + p).as_vec3()
                            + Vec3::splat(0.5)
                            + normal.as_vec3() * 0.5
                            + u.as_vec3() * (*a as f32 * 0.5)
                            + v.as_vec3() * (*b as f32 * 0.5);
                        vertices[i].pos = pos.to_array();
                        let mut color = block.color();
                        if block == Block::Grass && normal.y == 0 {
                            color = Block::Dirt.color();
                        }
                        let variation = 0.94
                            + (crate::world::hash(w.seed, base.x + x, base.y + y, base.z + z) % 11)
                                as f32
                                * 0.01;
                        vertices[i].color = color.map(|c| c * (1. - ao as f32 * 0.16) * variation);
                    }
                    let indices = if aos[0] + aos[2] > aos[1] + aos[3] {
                        [0, 1, 3, 1, 2, 3]
                    } else {
                        [0, 1, 2, 0, 2, 3]
                    };
                    for i in indices {
                        out.push(vertices[i]);
                    }
                }
            }
        }
    }
    out
}
fn box_mesh(out: &mut Vec<Vertex>, pos: Vec3, size: Vec3, color: [f32; 3]) {
    for (n, u, v) in FACES {
        let verts = CORNERS.map(|(a, b)| {
            let p = Vec3::splat(0.5)
                + n.as_vec3() * 0.5
                + u.as_vec3() * a as f32 * 0.5
                + v.as_vec3() * b as f32 * 0.5;
            Vertex {
                pos: (pos + p * size).to_array(),
                normal: n.as_vec3().to_array(),
                color,
            }
        });
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(verts[i]);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_sky_has_no_mesh() {
        let w = World::new(1);
        assert!(mesh_chunk(&w, IVec3::new(0, 80, 0)).is_empty());
    }
    #[test]
    fn adjacent_blocks_cull_shared_face() {
        let mut w = World::new(1);
        w.set(IVec3::new(0, 100, 0), Block::Planks);
        let one = mesh_chunk(&w, IVec3::new(0, 6, 0)).len();
        assert_eq!(one, 36);
        w.set(IVec3::new(1, 100, 0), Block::Planks);
        assert_eq!(mesh_chunk(&w, IVec3::new(0, 6, 0)).len(), 60);
    }
}
