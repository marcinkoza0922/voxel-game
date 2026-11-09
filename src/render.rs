use crate::{
    game::Game,
    ui::{Ui, UiVertex},
    world::{Block, SIZE, VIEW_RADIUS, World},
};
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3};
use std::{collections::HashMap, sync::Arc};
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    tile: [f32; 3],
    sky_access: f32,
}
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 5] =
    wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x3,4=>Float32];
impl Vertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: 52,
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
    inverse_view: [[f32; 4]; 4],
    celestial: [f32; 4],
}
struct Mesh {
    buffer: wgpu::Buffer,
    count: u32,
}
pub struct Renderer {
    surface: Option<wgpu::Surface<'static>>,
    window: Option<Arc<Window>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    shadow_bind: wgpu::BindGroup,
    depth: wgpu::TextureView,
    shadow: wgpu::TextureView,
    meshes: HashMap<IVec3, Mesh>,
    pub ui: Ui,
    pub width: u32,
    pub height: u32,
}
// Phase zero is sunrise, 0.25 noon, 0.5 sunset, 0.75 midnight.
fn sky_cycle(time: f32) -> (Vec3, f32) {
    let angle = time.rem_euclid(1.) * std::f32::consts::TAU;
    let direction = Vec3::new(angle.cos(), angle.sin(), angle.sin() * 0.35).normalize();
    let t = ((direction.y + 0.08) / 0.28).clamp(0., 1.);
    (direction, t * t * (3. - 2. * t))
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
        Self::initialize(Some(window), size.width, size.height).await
    }
    pub async fn headless(width: u32, height: u32) -> Result<Self, String> {
        Self::initialize(None, width, height).await
    }
    async fn initialize(
        window: Option<Arc<Window>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = window
            .as_ref()
            .map(|w| instance.create_surface(w.clone()))
            .transpose()
            .map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface.as_ref(),
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
        let caps = surface.as_ref().map(|s| s.get_capabilities(&adapter));
        let format = caps
            .as_ref()
            .and_then(|c| c.formats.iter().copied().find(|f| f.is_srgb()))
            .unwrap_or_else(|| {
                caps.as_ref()
                    .map_or(wgpu::TextureFormat::Rgba8UnormSrgb, |c| c.formats[0])
            });
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: width.max(1),
            height: height.max(1),
            // Strict VSync: AutoVsync can select relaxed FIFO, which permits
            // immediate presentation when rendering misses a refresh.
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .as_ref()
                .map_or(wgpu::CompositeAlphaMode::Auto, |c| c.alpha_modes[0]),
            view_formats: vec![],
        };
        if let Some(surface) = &surface {
            surface.configure(&device, &config);
        }
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
        let tiles = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("32px pixel material tiles"),
            size: wgpu::Extent3d {
                width: crate::art::TILE_SIZE,
                height: crate::art::TILE_SIZE,
                depth_or_array_layers: (crate::art::BLOCKS.len() * 3) as u32,
            },
            mip_level_count: 6,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, pixels) in crate::art::mipmaps().iter().enumerate() {
            let size = crate::art::TILE_SIZE >> level;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    mip_level: level as u32,
                    ..tiles.as_image_copy()
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: (crate::art::BLOCKS.len() * 3) as u32,
                },
            );
        }
        let tiles_view = tiles.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let tile_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Crisp pixel sampler"),
            mipmap_filter: wgpu::FilterMode::Linear,
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&tiles_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&tile_sampler),
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
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Procedural skybox"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("sky_vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                ..ds.clone()
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("sky_fs"),
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
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.,
                    clamp: 0.,
                },
                ..ds
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shadow_fs"),
                compilation_options: Default::default(),
                targets: &[],
            }),
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
            window,
            device,
            queue,
            width: config.width,
            height: config.height,
            config,
            pipeline,
            shadow_pipeline,
            sky_pipeline,
            ui_pipeline,
            uniform,
            bind,
            shadow_bind,
            depth,
            shadow,
            meshes: HashMap::new(),
            ui: Ui::new(),
        })
    }
    pub fn finish(self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        drop(self);
    }
    pub fn reset_world(&mut self) {
        self.meshes.clear();
        self.ui = crate::ui::Ui::new();
    }
    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.width = w;
        self.height = h;
        self.config.width = w;
        self.config.height = h;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.depth = depth(&self.device, w, h, "Main depth");
    }
    pub fn update_world(&mut self, w: &mut World, pos: Vec3) {
        for k in w.stream(pos, VIEW_RADIUS) {
            self.meshes.remove(&k);
        }
        for (k, data) in std::mem::take(&mut w.meshes) {
            if !w.loaded(k) {
                continue;
            }
            if data.is_empty() {
                self.meshes.remove(&k);
            } else {
                self.meshes.insert(
                    k,
                    Mesh {
                        buffer: self
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Chunk mesh"),
                                contents: bytemuck::cast_slice(&data),
                                usage: wgpu::BufferUsages::VERTEX,
                            }),
                        count: data.len() as u32,
                    },
                );
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
        let (celestial, daylight) = sky_cycle(g.time);
        // Moonlight takes over at night; both lights follow the visible orbit.
        let sun = if celestial.y >= 0. {
            celestial
        } else {
            -celestial
        };
        let strength = daylight + (1. - daylight) * 0.025 * sun.y.max(0.).sqrt();
        let sky = [daylight, 0., 0., 1.];
        let eye = g.player.eye();
        let camera_view = Mat4::perspective_rh(
            75_f32.to_radians(),
            self.width as f32 / self.height as f32,
            0.05,
            1024.,
        ) * Mat4::look_at_rh(eye, eye + g.player.direction(), Vec3::Y);
        let center = eye.floor();
        let light_view = Mat4::orthographic_rh(-80., 80., -80., 80., 0.1, 250.)
            * Mat4::look_at_rh(center + sun * 120., center, Vec3::Y);
        let mut lights = [[0.; 4]; 16];
        let mut torches: Vec<_> = g
            .torch_life
            .keys()
            .map(|p| {
                let mount = g.world.torch_mounts.get(p).copied().unwrap_or([0, -1, 0]);
                let wall = Vec3::new(mount[0] as f32, 0., mount[2] as f32);
                Vec3::from_array(p.map(|n| n as f32))
                    + Vec3::new(0.5, 0.04, 0.5)
                    + wall * 0.43
                    + (Vec3::Y - wall * 0.42).normalize() * 0.73
            })
            .collect();
        torches.sort_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        let held = g.player.inventory.selected() == crate::items::Item::Torch
            && g.player.inventory.selected_count() > 0;
        let mut n = 0;
        if held {
            lights[0] = [eye.x, eye.y, eye.z, 0.8];
            n = 1;
        }
        for p in torches.into_iter().take(16 - n) {
            lights[n] = [p.x, p.y, p.z, 0.8];
            n += 1;
        }
        let camera = Camera {
            view: camera_view.to_cols_array_2d(),
            light_view: light_view.to_cols_array_2d(),
            eye: [eye.x, eye.y, eye.z, 1.],
            sun: [sun.x, sun.y, sun.z, strength],
            sky,
            lights,
            inverse_view: camera_view.inverse().to_cols_array_2d(),
            celestial: [celestial.x, celestial.y, celestial.z, 0.],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&camera));
        let mut dynamic = vec![];
        for (_, m) in g.entities.iter() {
            let p = m.pos;
            let creature_start = dynamic.len();
            let guardian = matches!(m.kind, crate::ecs::MobKind::Guardian(_));
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.27, 0.65, -0.19),
                Vec3::new(0.54, 0.73, 0.38),
                if guardian {
                    [0.30, 0.43, 0.56]
                } else {
                    [0.27, 0.33, 0.24]
                },
            );
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.23, 1.38, -0.23),
                Vec3::splat(0.46),
                if guardian {
                    [0.60, 0.78, 0.87]
                } else {
                    [0.48, 0.49, 0.33]
                },
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
                    if guardian {
                        [0.42, 0.54, 0.64]
                    } else {
                        [0.36, 0.41, 0.29]
                    },
                );
            }
            // Recessed face, paired eyes, brow, belt and distinct armor silhouette.
            let eye_color = if guardian {
                [0.40, 0.88, 0.96]
            } else {
                [0.93, 0.65, 0.25]
            };
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.18, 1.49, -0.245),
                Vec3::new(0.36, 0.13, 0.025),
                [0.13, 0.17, 0.18],
            );
            for x in [-0.14, 0.07] {
                box_mesh(
                    &mut dynamic,
                    p + Vec3::new(x, 1.53, -0.265),
                    Vec3::new(0.07, 0.045, 0.025),
                    eye_color,
                );
            }
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.25, 1.64, -0.26),
                Vec3::new(0.50, 0.065, 0.10),
                if guardian {
                    [0.30, 0.40, 0.48]
                } else {
                    [0.31, 0.34, 0.23]
                },
            );
            textured_box(
                &mut dynamic,
                p + Vec3::new(-0.28, 0.70, -0.205),
                Vec3::new(0.56, 0.10, 0.41),
                Block::Log,
            );
            box_mesh(
                &mut dynamic,
                p + Vec3::new(-0.055, 0.71, -0.23),
                Vec3::new(0.11, 0.08, 0.04),
                [0.66, 0.51, 0.28],
            );
            if guardian {
                for x in [-0.44, 0.24] {
                    textured_box(
                        &mut dynamic,
                        p + Vec3::new(x, 1.18, -0.20),
                        Vec3::new(0.20, 0.19, 0.39),
                        Block::RuinStone,
                    );
                }
                box_mesh(
                    &mut dynamic,
                    p + Vec3::new(-0.06, 1.05, -0.23),
                    Vec3::new(0.12, 0.18, 0.055),
                    eye_color,
                );
            } else {
                textured_box(
                    &mut dynamic,
                    p + Vec3::new(-0.22, 0.91, -0.205),
                    Vec3::new(0.18, 0.25, 0.035),
                    Block::Leaves,
                );
            }
            let toward = (Vec3::from_array(g.player.pos) - p)
                .with_y(0.)
                .normalize_or_zero();
            let rotation = glam::Quat::from_rotation_y((-toward.x).atan2(-toward.z));
            for vertex in &mut dynamic[creature_start..] {
                vertex.pos = (p + rotation * (Vec3::from_array(vertex.pos) - p)).to_array();
                vertex.normal = (rotation * Vec3::from_array(vertex.normal)).to_array();
            }
        }
        let mut icon = Ui::new();
        icon.width = 16.;
        icon.height = 16.;
        let right = Vec3::new(g.player.yaw.cos(), 0., g.player.yaw.sin());
        let normal = right.cross(Vec3::Y);
        for drop in &g.drops {
            let pos = Vec3::from_array(drop.pos);
            if pos.distance_squared(eye) > 80. * 80. {
                continue;
            }
            icon.vertices.clear();
            icon.icon(0., 0., drop.item, 1.);
            let center = pos + Vec3::Y * (0.14 + 0.04 * (g.feedback.clock * 2.5 + pos.x).sin());
            for v in &icon.vertices {
                dynamic.push(Vertex {
                    sky_access: 1.,
                    pos: (center + right * v.pos[0] * 0.3 + Vec3::Y * v.pos[1] * 0.3).to_array(),
                    normal: normal.to_array(),
                    color: [v.color[0], v.color[1], v.color[2]],
                    tile: [0., 0., -1.],
                });
            }
        }
        for particle in &g.feedback.particles {
            let size = 0.045 * (particle.life / 0.15).min(1.);
            box_mesh(
                &mut dynamic,
                particle.pos,
                Vec3::splat(size),
                particle.color,
            );
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
        let mut held_mesh = vec![];
        if !pack && !paused {
            held_model(&mut held_mesh, g);
        }
        let held_buffer = (!held_mesh.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Held item"),
                    contents: bytemuck::cast_slice(&held_mesh),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        apply_sky_access(&g.world, &mut dynamic);
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
        let frame = if let Some(surface) = &self.surface {
            match surface.get_current_texture() {
                Ok(frame) => Some(frame),
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    surface.configure(&self.device, &self.config);
                    return Ok(());
                }
                Err(wgpu::SurfaceError::Timeout) => return Ok(()),
                Err(e) => return Err(e.to_string()),
            }
        } else {
            None
        };
        let offscreen = frame.is_none().then(|| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Headless framebuffer"),
                size: wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: self.config.usage,
                view_formats: &[],
            })
        });
        let texture = frame
            .as_ref()
            .map(|f| &f.texture)
            .or(offscreen.as_ref())
            .unwrap();
        let view = texture.create_view(&Default::default());
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
            for (k, m) in &self.meshes {
                if !chunk_visible(light_view, *k) {
                    continue;
                }
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
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipeline);
            for (k, m) in &self.meshes {
                if !chunk_visible(camera_view, *k) {
                    continue;
                }
                pass.set_vertex_buffer(0, m.buffer.slice(..));
                pass.draw(0..m.count, 0..1);
            }
            if let Some(b) = &dynamic_buffer {
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..dynamic.len() as u32, 0..1);
            }
        }
        if let Some(buffer) = &held_buffer {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Held item pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
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
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..held_mesh.len() as u32, 0..1);
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
                texture.as_image_copy(),
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
        if let Some(window) = &self.window {
            window.pre_present_notify();
        }
        if let Some(frame) = frame {
            frame.present();
        }
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
// Conservative clip-space AABB test, using wgpu's 0..w depth range.
fn chunk_visible(view: Mat4, key: IVec3) -> bool {
    let base = (key * SIZE).as_vec3();
    let corners = std::array::from_fn::<_, 8, _>(|i| {
        let offset = Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32);
        view * (base + offset * SIZE as f32).extend(1.)
    });
    !(0..6).any(|plane| {
        corners.iter().all(|p| match plane {
            0 => p.x < -p.w,
            1 => p.x > p.w,
            2 => p.y < -p.w,
            3 => p.y > p.w,
            4 => p.z < 0.,
            _ => p.z > p.w,
        })
    })
}
#[cfg(test)]
pub fn mesh_chunk(w: &World, k: IVec3) -> Vec<Vertex> {
    let mut mesh = mesh_halo(w.seed, k, &w.halo(k), &w.torch_mounts);
    apply_sky_access(w, &mut mesh);
    mesh
}
/// Maximum reach of indirect skylight into an entrance (in blocks).
pub const SKY_REACH: i32 = 12;

struct SkyAccess<'a> {
    world: &'a World,
    roofs: HashMap<(i32, i32), i32>,
    opaque: HashMap<IVec3, bool>,
    exposure: HashMap<IVec3, f32>,
}
impl SkyAccess<'_> {
    fn open_sky(&mut self, cell: IVec3) -> bool {
        let w = self.world;
        let roof = self.roofs.entry((cell.x, cell.z)).or_insert_with(|| {
            let top = w
                .edits
                .keys()
                .filter(|p| p[0] == cell.x && p[2] == cell.z)
                .map(|p| p[1])
                .max()
                .unwrap_or(crate::world::MIN_Y)
                .max(w.height(cell.x, cell.z) + 16);
            (crate::world::MIN_Y..=top.min(crate::world::MAX_Y))
                .rev()
                .find(|y| {
                    let b = w.get(IVec3::new(cell.x, *y, cell.z));
                    b.solid() && b != Block::Leaves
                })
                .unwrap_or(crate::world::MIN_Y)
        });
        cell.y > *roof
    }
    fn blocked(&mut self, cell: IVec3) -> bool {
        *self.opaque.entry(cell).or_insert_with(|| {
            let b = self.world.get(cell);
            b.solid() && b != Block::Leaves
        })
    }
    fn sample(&mut self, cell: IVec3) -> f32 {
        if let Some(value) = self.exposure.get(&cell) {
            return *value;
        }
        let mut value: f32 = 0.;
        if self.open_sky(cell) {
            value = 1.;
        } else {
            // Trace toward nearby openings, stopping at walls. The limited reach
            // preserves complete darkness in deep caves and enclosed rooms.
            for (x, z) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ] {
                for rise in [0., 0.5] {
                    let direction = Vec3::new(x as f32, rise, z as f32).normalize();
                    for step in 1..SKY_REACH * 2 {
                        let distance = step as f32 * 0.5;
                        let p = (cell.as_vec3() + Vec3::splat(0.5) + direction * distance)
                            .floor()
                            .as_ivec3();
                        if self.blocked(p) {
                            break;
                        }
                        if self.open_sky(p) {
                            let falloff = 1. - distance / SKY_REACH as f32;
                            value = value.max(0.65 * falloff * falloff);
                            break;
                        }
                    }
                }
            }
        }
        self.exposure.insert(cell, value);
        value
    }
}
pub fn apply_sky_access(w: &World, mesh: &mut [Vertex]) {
    let mut sky = SkyAccess {
        world: w,
        roofs: HashMap::new(),
        opaque: HashMap::new(),
        exposure: HashMap::new(),
    };
    for vertex in mesh {
        let sample = Vec3::from_array(vertex.pos) + Vec3::from_array(vertex.normal) * 0.02;
        vertex.sky_access = sky.sample(sample.floor().as_ivec3());
    }
}
pub fn mesh_halo(
    seed: u64,
    k: IVec3,
    halo: &[Block],
    mounts: &std::collections::BTreeMap<[i32; 3], [i32; 3]>,
) -> Vec<Vertex> {
    let base = k * SIZE;
    let idx = |p: IVec3| ((p.x + 1) + 18 * ((p.z + 1) + 18 * (p.y + 1))) as usize;
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
                    let mount = mounts
                        .get(&(base + p).to_array())
                        .copied()
                        .unwrap_or([0, -1, 0]);
                    torch_model(&mut out, (base + p).as_vec3(), IVec3::from_array(mount));
                    continue;
                }
                if block.plant() {
                    vegetation(&mut out, seed, base + p - IVec3::Y, block);
                    continue;
                }
                for (normal, u, v) in FACES {
                    let neighbor = get(p + normal);
                    if neighbor.solid() && neighbor != Block::Leaves {
                        continue;
                    }
                    let mut vertices = [Vertex {
                        sky_access: 1.,
                        pos: [0.; 3],
                        normal: normal.as_vec3().to_array(),
                        color: [1.; 3],
                        tile: [0.; 3],
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
                        let face = if block == Block::Grass
                            && normal.y == 0
                            && (!get(p - IVec3::Y).solid() || get(p + normal - IVec3::Y).solid())
                        {
                            // A one-block step: grass wraps the whole side.
                            1
                        } else if normal.y > 0 {
                            1
                        } else if normal.y < 0 {
                            2
                        } else {
                            0
                        };
                        vertices[i].tile = [
                            (*a as f32 + 1.) * 0.5,
                            (1. - *b as f32) * 0.5,
                            crate::art::layer(block, face) as f32,
                        ];
                        let variation = 0.96
                            + (crate::world::hash(seed, base.x + x, base.y + y, base.z + z) % 9)
                                as f32
                                * 0.01;
                        vertices[i].color = [((1. - ao as f32 * 0.16) * variation); 3];
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
// Plant geometry lives in its own voxel; seed and support coordinates determine its shape.
fn vegetation(out: &mut Vec<Vertex>, seed: u64, support: IVec3, block: Block) {
    let h = crate::world::vegetation_hash(seed, support);
    let origin = support.as_vec3() + Vec3::Y;
    if block == Block::TallGrass {
        for i in 0..3 {
            let bits = h.rotate_right(i * 17);
            let center = origin
                + Vec3::new(
                    0.24 + (bits & 255) as f32 / 500.,
                    0.,
                    0.24 + ((bits >> 8) & 255) as f32 / 500.,
                );
            let height = crate::world::grass_height(h, i);
            let angle = ((bits >> 24) & 255) as f32 * std::f32::consts::TAU / 255.;
            let direction = Vec3::new(angle.cos(), 0., angle.sin());
            for blade in 0..3 {
                let side = glam::Quat::from_rotation_y(blade as f32 * 2.094) * direction;
                let tip = (center + Vec3::Y * height + side * height * 0.3).clamp(
                    origin + Vec3::new(0.02, 0., 0.02),
                    origin + Vec3::new(0.98, 0.95, 0.98),
                );
                let normal = side.cross(Vec3::Y).normalize().to_array();
                for (point, color) in [
                    (center - side * 0.045, [0.16, 0.30, 0.07]),
                    (center + side * 0.045, [0.22, 0.39, 0.09]),
                    (tip, [0.43, 0.59, 0.16]),
                ] {
                    out.push(Vertex {
                        sky_access: 1.,
                        pos: point.to_array(),
                        normal,
                        color,
                        tile: [0., 0., -3.],
                    });
                }
            }
        }
        return;
    }
    let root = origin + Vec3::new(0.5, 0., 0.5);
    if block == Block::Shrub {
        // Overlapping asymmetric leaf clusters around short woody branches.
        let count = 4 + (h % 7) as u32;
        let spread = 0.65 + ((h >> 12) & 255) as f32 / 730.;
        let stature = 0.65 + ((h >> 20) & 255) as f32 / 730.;
        let rotation = ((h >> 32) & 255) as f32 * std::f32::consts::TAU / 255.;
        let warmth = ((h >> 48) & 255) as f32 / 255.;
        for i in 0..count {
            let bits = h.rotate_right(i * 9);
            let angle = rotation + i as f32 * 2.4;
            let radius = if i == 0 {
                0.
            } else {
                0.18 + (bits & 31) as f32 / 300.
            };
            let center = root
                + Vec3::new(
                    angle.cos() * radius * spread,
                    (0.19 + ((bits >> 8) & 255) as f32 / 1100.) * stature,
                    angle.sin() * radius * spread,
                );
            plant_stem(out, root, center, 0.022, [0.32, 0.25, 0.12]);
            let size = Vec3::new(0.27 * spread, 0.24 * stature, 0.27 * spread)
                * (0.65 + ((bits >> 24) & 63) as f32 / 120.);
            let start = out.len();
            box_mesh(
                out,
                center - size * 0.5,
                size,
                [
                    0.13 + warmth * 0.11 + i as f32 * 0.006,
                    0.29 + warmth * 0.10 + i as f32 * 0.012,
                    0.07 + (1. - warmth) * 0.07,
                ],
            );
            for vertex in &mut out[start..] {
                vertex.tile[2] = -4.; // Shrub leaves: cutout and transmitted sunlight.
            }
        }
        return;
    }
    let Some(flower) = block.flower_index() else {
        return;
    };
    let height = 0.35 + ((h >> 40) & 255) as f32 / 640.;
    let angle = (h & 255) as f32 * std::f32::consts::TAU / 255.;
    let lean = Vec3::new(angle.cos(), 0., angle.sin()) * (0.06 + ((h >> 8) & 255) as f32 / 1800.);
    let middle = root + Vec3::Y * height * 0.5 + lean * 0.25;
    let head = root + Vec3::Y * height + lean;
    plant_stem(out, root, middle, 0.024, [0.17, 0.38, 0.08]);
    plant_stem(out, middle, head, 0.019, [0.22, 0.43, 0.10]);
    for side in [-1., 1.] {
        let start = root.lerp(head, 0.3 + (side + 1.) * 0.12);
        let direction = Vec3::new(angle.sin(), 0.45, -angle.cos()) * side;
        plant_stem(
            out,
            start,
            start + direction * 0.17,
            0.045,
            [0.22, 0.43, 0.10],
        );
    }
    let color = crate::world::FLOWER_COLORS[flower];
    let petals = 4 + ((h >> 28) % 3) as usize;
    let tilt = glam::Quat::from_rotation_z(lean.x * 2.) * glam::Quat::from_rotation_x(-lean.z * 2.);
    for i in 0..petals {
        let bits = h.rotate_right(i as u32 * 7);
        let theta = angle + i as f32 * std::f32::consts::TAU / petals as f32;
        let length = 0.085 + (bits & 31) as f32 / 650.;
        let local = Vec3::new(
            theta.cos() * length,
            ((bits >> 8) & 31) as f32 / 950. - 0.025,
            theta.sin() * length,
        );
        let start = out.len();
        let size = Vec3::new(0.085 + length * 0.25, 0.035, 0.09);
        let shade = 0.88 + ((bits >> 16) & 31) as f32 / 260.;
        box_mesh(
            out,
            local - size * 0.5,
            size,
            color.map(|c| (c * shade).min(1.)),
        );
        for vertex in &mut out[start..] {
            vertex.pos = (head + tilt * Vec3::from_array(vertex.pos)).to_array();
            vertex.normal = (tilt * Vec3::from_array(vertex.normal)).to_array();
        }
    }
    box_mesh(
        out,
        head - Vec3::splat(0.035),
        Vec3::splat(0.07),
        [1., 0.64, 0.08],
    );
}
// Rotate a narrow box between endpoints for curved stems, branches and leaves.
fn plant_stem(out: &mut Vec<Vertex>, start: Vec3, end: Vec3, width: f32, color: [f32; 3]) {
    let delta = end - start;
    let rotation = glam::Quat::from_rotation_arc(Vec3::Y, delta.normalize());
    let first = out.len();
    box_mesh(
        out,
        Vec3::new(-width * 0.5, 0., -width * 0.5),
        Vec3::new(width, delta.length(), width),
        color,
    );
    for vertex in &mut out[first..] {
        let mut position = start + rotation * Vec3::from_array(vertex.pos);
        position.y = position.y.max(start.y.min(end.y));
        vertex.pos = position.to_array();
        vertex.normal = (rotation * Vec3::from_array(vertex.normal)).to_array();
    }
}
// Shared held/placed model. Wall mounts lean away from the supporting face.
fn torch_model(out: &mut Vec<Vertex>, origin: Vec3, mount: IVec3) {
    let start = out.len();
    textured_box(
        out,
        Vec3::new(0.445, 0.04, 0.445),
        Vec3::new(0.11, 0.55, 0.11),
        Block::Torch,
    );
    textured_box(
        out,
        Vec3::new(0.415, 0.49, 0.415),
        Vec3::new(0.17, 0.15, 0.17),
        Block::Torch,
    );
    // Iron binding and layered, asymmetric flame silhouette.
    box_mesh(
        out,
        Vec3::new(0.405, 0.48, 0.405),
        Vec3::new(0.19, 0.035, 0.19),
        [0.20, 0.17, 0.13],
    );
    let flame_start = out.len();
    box_mesh(
        out,
        Vec3::new(0.41, 0.64, 0.42),
        Vec3::new(0.18, 0.13, 0.16),
        [1., 0.35, 0.055],
    );
    box_mesh(
        out,
        Vec3::new(0.44, 0.74, 0.445),
        Vec3::new(0.115, 0.13, 0.11),
        [1., 0.65, 0.12],
    );
    box_mesh(
        out,
        Vec3::new(0.475, 0.85, 0.46),
        Vec3::new(0.055, 0.10, 0.055),
        [1., 0.83, 0.32],
    );
    box_mesh(
        out,
        Vec3::new(0.46, 0.65, 0.405),
        Vec3::new(0.07, 0.13, 0.025),
        [1., 0.87, 0.42],
    );
    for vertex in &mut out[flame_start..] {
        vertex.tile[2] = -2.;
    }
    let wall = Vec3::new(mount.x as f32, 0., mount.z as f32);
    let rotation = glam::Quat::from_rotation_arc(Vec3::Y, (Vec3::Y - wall * 0.42).normalize());
    for vertex in &mut out[start..] {
        let local = Vec3::from_array(vertex.pos) - Vec3::new(0.5, 0.04, 0.5);
        vertex.pos =
            (origin + Vec3::new(0.5, 0.04, 0.5) + wall * 0.43 + rotation * local).to_array();
        vertex.normal = (rotation * Vec3::from_array(vertex.normal)).to_array();
    }
}

fn textured_box(out: &mut Vec<Vertex>, pos: Vec3, size: Vec3, block: Block) {
    let start = out.len();
    box_mesh(out, pos, size, [1.; 3]);
    for vertex in &mut out[start..] {
        let n = Vec3::from_array(vertex.normal);
        let local = (Vec3::from_array(vertex.pos) - pos) / size;
        let uv = if n.y.abs() > 0.5 {
            [local.x, local.z]
        } else if n.x.abs() > 0.5 {
            [local.z, 1. - local.y]
        } else {
            [local.x, 1. - local.y]
        };
        let face = if n.y > 0.5 {
            1
        } else if n.y < -0.5 {
            2
        } else {
            0
        };
        vertex.tile = [uv[0], uv[1], crate::art::layer(block, face) as f32];
    }
}
fn box_mesh(out: &mut Vec<Vertex>, pos: Vec3, size: Vec3, color: [f32; 3]) {
    for (n, u, v) in FACES {
        let verts = CORNERS.map(|(a, b)| {
            let p = Vec3::splat(0.5)
                + n.as_vec3() * 0.5
                + u.as_vec3() * a as f32 * 0.5
                + v.as_vec3() * b as f32 * 0.5;
            Vertex {
                sky_access: 1.,
                pos: (pos + p * size).to_array(),
                normal: n.as_vec3().to_array(),
                color,
                tile: [0., 0., -1.],
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
    fn frustum_culls_behind_camera_and_keeps_intersections() {
        let view = Mat4::perspective_rh(75_f32.to_radians(), 1., 0.05, 1024.);
        assert!(chunk_visible(view, IVec3::new(0, 0, -2)));
        assert!(chunk_visible(view, IVec3::new(0, 0, -1)));
        assert!(!chunk_visible(view, IVec3::new(0, 0, 1)));
        assert!(!chunk_visible(view, IVec3::new(20, 0, -2)));
        assert!(!chunk_visible(view, IVec3::new(0, 0, -70)));
        let light = Mat4::orthographic_rh(-80., 80., -80., 80., 0.1, 250.);
        assert!(chunk_visible(light, IVec3::new(0, 0, -2)));
        assert!(!chunk_visible(light, IVec3::new(20, 0, -2)));
    }
    #[test]
    fn vegetation_is_repeatable_varied_and_rooted() {
        let mut first = Vec::new();
        let mut second = Vec::new();
        for x in 0..512 {
            let p = IVec3::new(x, 100, 0);
            let block = crate::world::vegetation_block(314159, p);
            if block == Block::Air {
                continue;
            }
            vegetation(&mut first, 314159, p, block);
            vegetation(&mut second, 314159, p, block);
        }
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&first),
            bytemuck::cast_slice::<_, u8>(&second)
        );
        assert!(first.iter().all(|v| v.pos[1] >= 101. && v.pos[1] < 102.));
        let flower_colors = first
            .iter()
            .filter(|v| v.tile[2] == -1. && v.color[0] > 0.6)
            .map(|v| v.color.map(f32::to_bits))
            .collect::<std::collections::HashSet<_>>();
        assert!(flower_colors.len() >= 4);
        let mut w = World::new(314159);
        let p = IVec3::new(0, 100, 0);
        w.set(p, Block::Grass);
        assert!(mesh_chunk(&w, crate::world::chunk(p)).len() >= 36);
        w.set(p, Block::Air);
        assert!(mesh_chunk(&w, crate::world::chunk(p)).is_empty());
    }
    #[test]
    fn grassy_dirt_wraps_steps_but_keeps_cliff_sides_bare() {
        let mut w = World::new(314159);
        let p = IVec3::new(-1, 111, 0);
        w.set(p - IVec3::Y, Block::Dirt);
        w.set(p, Block::Grass);
        let side_layers = |w: &World| {
            mesh_chunk(w, crate::world::chunk(p))
                .chunks_exact(3)
                .filter(|triangle| {
                    triangle.iter().all(|v| {
                        v.normal == Vec3::X.to_array()
                            && v.pos[0] == p.x as f32 + 1.
                            && v.pos[1] >= p.y as f32
                    })
                })
                .flat_map(|triangle| triangle.iter().map(|v| v.tile[2] as u32))
                .collect::<Vec<_>>()
        };
        let cliff = side_layers(&w);
        assert_eq!(cliff.len(), 6);
        assert!(
            cliff
                .iter()
                .all(|&l| l == crate::art::layer(Block::Grass, 0))
        );
        w.set(p + IVec3::X - IVec3::Y, Block::Dirt);
        let step = side_layers(&w);
        assert_eq!(step.len(), 6);
        assert!(
            step.iter()
                .all(|&l| l == crate::art::layer(Block::Grass, 1))
        );
        w.set(p + IVec3::X - IVec3::Y, Block::Air);
        assert_eq!(side_layers(&w), cliff);
    }
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

fn held_model(out: &mut Vec<Vertex>, g: &Game) {
    use crate::items::Item;
    let item = g.player.inventory.selected();
    let equipped = g.player.inventory.selected_count() > 0;
    let swing = (g.feedback.swing / 0.32 * std::f32::consts::PI).sin();
    let moving = g.velocity.x * g.velocity.x + g.velocity.z * g.velocity.z > 0.1 && g.grounded;
    let bob = if moving {
        (g.feedback.clock * 11.).sin() * 0.014
    } else {
        0.
    };
    let origin = Vec3::new(
        0.32 - swing * 0.18,
        -0.30 + bob + swing * 0.06,
        -0.65 + swing * 0.12,
    );
    box_mesh(
        out,
        Vec3::new(-0.065, -0.22, -0.06),
        Vec3::new(0.13, 0.25, 0.12),
        [0.65, 0.43, 0.29],
    );
    if equipped
        && (item.pick() > 0 || item.axe() > 0 || matches!(item, Item::Sword | Item::IronSword))
    {
        let material = match item {
            Item::WoodAxe | Item::WoodPick => [0.55, 0.35, 0.17],
            Item::StoneAxe | Item::StonePick => [0.53, 0.55, 0.57],
            Item::IronAxe | Item::IronPick | Item::IronSword => [0.72, 0.77, 0.81],
            _ => [0.77, 0.43, 0.24],
        };
        textured_box(
            out,
            Vec3::new(-0.025, -0.10, -0.025),
            Vec3::new(0.05, 0.48, 0.05),
            Block::Log,
        );
        for y in [-0.06, -0.01, 0.04, 0.09] {
            box_mesh(
                out,
                Vec3::new(-0.031, y, -0.031),
                Vec3::new(0.062, 0.018, 0.062),
                [0.57, 0.42, 0.25],
            );
        }
        box_mesh(
            out,
            Vec3::new(-0.035, 0.24, -0.035),
            Vec3::new(0.07, 0.055, 0.07),
            [0.24, 0.23, 0.21],
        );
        if item.pick() > 0 {
            box_mesh(
                out,
                Vec3::new(-0.20, 0.29, -0.04),
                Vec3::new(0.40, 0.07, 0.08),
                material,
            );
            box_mesh(
                out,
                Vec3::new(-0.23, 0.23, -0.035),
                Vec3::new(0.07, 0.09, 0.07),
                material,
            );
        } else if item.axe() > 0 {
            box_mesh(
                out,
                Vec3::new(-0.13, 0.20, -0.045),
                Vec3::new(0.14, 0.15, 0.09),
                material,
            );
            box_mesh(
                out,
                Vec3::new(-0.20, 0.17, -0.025),
                Vec3::new(0.07, 0.21, 0.05),
                material,
            );
        } else {
            box_mesh(
                out,
                Vec3::new(-0.045, 0.10, -0.025),
                Vec3::new(0.09, 0.48, 0.05),
                material,
            );
            box_mesh(
                out,
                Vec3::new(-0.13, 0.08, -0.04),
                Vec3::new(0.26, 0.05, 0.08),
                material,
            );
        }
        let edge = material.map(|c| (c * 1.25_f32).min(1.));
        if item.axe() > 0 {
            box_mesh(
                out,
                Vec3::new(-0.207, 0.18, -0.029),
                Vec3::new(0.022, 0.19, 0.058),
                edge,
            );
        } else if item.pick() > 0 {
            box_mesh(
                out,
                Vec3::new(-0.18, 0.345, -0.044),
                Vec3::new(0.36, 0.018, 0.088),
                edge,
            );
        } else {
            box_mesh(
                out,
                Vec3::new(-0.045, 0.14, -0.031),
                Vec3::new(0.018, 0.42, 0.012),
                edge,
            );
            box_mesh(
                out,
                Vec3::new(-0.035, -0.11, -0.035),
                Vec3::splat(0.07),
                material,
            );
        }
    } else if equipped && item == Item::Torch {
        let start = out.len();
        torch_model(out, Vec3::ZERO, IVec3::NEG_Y);
        for vertex in &mut out[start..] {
            vertex.pos =
                ((Vec3::from_array(vertex.pos) - Vec3::new(0.5, 0.1, 0.5)) * 0.55).to_array();
        }
    } else if equipped && item.flower_index().is_some() {
        let start = out.len();
        vegetation(out, 0, IVec3::ZERO, item.block().unwrap());
        for v in &mut out[start..] {
            v.pos = ((Vec3::from_array(v.pos) - Vec3::new(0.5, 1., 0.5)) * 0.65).to_array();
        }
    } else if equipped {
        textured_box(
            out,
            Vec3::new(-0.09, 0.02, -0.10),
            Vec3::splat(0.18),
            item.block().unwrap_or(Block::Planks),
        );
    }
    let forward = g.player.direction();
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward);
    let basis = Mat4::from_cols(
        right.extend(0.),
        up.extend(0.),
        (-forward).extend(0.),
        g.player.eye().extend(1.),
    );
    let transform =
        basis * Mat4::from_translation(origin) * Mat4::from_rotation_z(-0.3 + swing * 0.95);
    for v in out {
        v.pos = transform
            .transform_point3(Vec3::from_array(v.pos))
            .to_array();
        v.normal = transform
            .transform_vector3(Vec3::from_array(v.normal))
            .to_array();
    }
}

#[cfg(test)]
mod sky_tests {
    use super::*;

    #[test]
    fn orbit_and_daylight_repeat_and_track_horizon() {
        for phase in [0., 0.1, 0.25, 0.5, 0.75] {
            let (sun, light) = sky_cycle(phase);
            let (next, next_light) = sky_cycle(phase + 4.);
            assert!(sun.distance(next) < 0.00001);
            assert!((light - next_light).abs() < 0.00001);
            assert!((sun.length() - 1.).abs() < 0.00001);
        }
        assert_eq!(sky_cycle(0.25).1, 1.);
        assert_eq!(sky_cycle(0.75).1, 0.);
        assert!(sky_cycle(0.).0.y.abs() < 0.00001);
        assert!(sky_cycle(0.5).0.y.abs() < 0.00001);
    }
    #[test]
    fn sky_access_blocks_caves_and_tracks_roof_edits() {
        let mut world = World::new(42);
        let y = world.height(0, 0) + 30;
        let mut vertices = [Vertex {
            pos: [0.5, y as f32, 0.5],
            normal: [0., 1., 0.],
            color: [1.; 3],
            tile: [0.; 3],
            sky_access: 1.,
        }];
        apply_sky_access(&world, &mut vertices);
        assert_eq!(vertices[0].sky_access, 1.);
        world.set(IVec3::new(0, y + 40, 0), Block::Stone);
        apply_sky_access(&world, &mut vertices);
        assert!(vertices[0].sky_access > 0. && vertices[0].sky_access < 1.);
        world.set(IVec3::new(0, y + 40, 0), Block::Air);
        apply_sky_access(&world, &mut vertices);
        assert_eq!(vertices[0].sky_access, 1.);
        vertices[0].pos[1] = (world.height(0, 0) - 20) as f32;
        apply_sky_access(&world, &mut vertices);
        assert_eq!(vertices[0].sky_access, 0.);
    }
    #[test]
    fn entrance_light_fades_with_distance_and_stops_at_walls() {
        let mut world = World::new(42);
        let y = world.height(0, 0) + 60;
        for x in 0..=28 {
            for z in -14..=14 {
                world.set(IVec3::new(x, y + 2, z), Block::Stone);
            }
        }
        let mut sky = SkyAccess {
            world: &world,
            roofs: HashMap::new(),
            opaque: HashMap::new(),
            exposure: HashMap::new(),
        };
        let near = sky.sample(IVec3::new(1, y, 0));
        let middle = sky.sample(IVec3::new(5, y, 0));
        let deep = sky.sample(IVec3::new(14, y, 0));
        assert!(near > middle && middle > 0.);
        assert_eq!(deep, 0.);
        for z in -14..=14 {
            for dy in -1..=2 {
                world.set(IVec3::new(0, y + dy, z), Block::Stone);
            }
        }
        let mut sky = SkyAccess {
            world: &world,
            roofs: HashMap::new(),
            opaque: HashMap::new(),
            exposure: HashMap::new(),
        };
        assert_eq!(sky.sample(IVec3::new(1, y, 0)), 0.);
    }
}
