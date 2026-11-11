//! SMAA 1x using the reference edge, weight, and neighborhood passes.
use wgpu::util::DeviceExt;

use crate::temporal::{color, fullscreen_pass};

struct Pass {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}

impl Pass {
    fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        fullscreen_pass(encoder, target, &self.pipeline, &self.bind);
    }
}

pub struct Smaa {
    edges: wgpu::TextureView,
    weights: wgpu::TextureView,
    pub scaled: Option<wgpu::TextureView>,
    detect: Pass,
    calculate: Pass,
    blend: Pass,
}

impl Smaa {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &wgpu::TextureView,
        size: (u32, u32),
        output: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Self {
        use wgpu::TextureFormat::{Rg8Unorm, Rgba8Unorm, Rgba16Float};
        let edges = color(device, size, Rg8Unorm, "SMAA edges");
        let weights = color(device, size, Rgba8Unorm, "SMAA weights");
        let scaled = (size != output).then(|| color(device, size, Rgba16Float, "SMAA color"));
        let metrics = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SMAA render size"),
            contents: bytemuck::cast_slice(&[
                1. / size.0 as f32,
                1. / size.1 as f32,
                size.0 as f32,
                size.1 as f32,
            ]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let lookup = |label, width, height, format, data: &[u8]| {
            device
                .create_texture_with_data(
                    queue,
                    &wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    data,
                )
                .create_view(&Default::default())
        };
        let area = lookup(
            "SMAA area lookup",
            160,
            560,
            Rg8Unorm,
            include_bytes!("../vendor/smaa/area.bin"),
        );
        let search = lookup(
            "SMAA search lookup",
            64,
            16,
            wgpu::TextureFormat::R8Unorm,
            include_bytes!("../vendor/smaa/search.bin"),
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SMAA 1x"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../vendor/smaa/smaa.wgsl").into()),
        });
        let make_pass = |label,
                         vertex,
                         fragment,
                         format: wgpu::TextureFormat,
                         resources: &[(u32, wgpu::BindingResource<'_>)]| {
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });
            let mut entries = vec![wgpu::BindGroupEntry {
                binding: 1,
                resource: metrics.as_entire_binding(),
            }];
            entries.extend(
                resources
                    .iter()
                    .map(|(binding, resource)| wgpu::BindGroupEntry {
                        binding: *binding,
                        resource: resource.clone(),
                    }),
            );
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &entries,
            });
            Pass { pipeline, bind }
        };
        use wgpu::BindingResource::{Sampler, TextureView};
        let detect = make_pass(
            "SMAA edge detection",
            "edge_detection_vertex_main",
            "luma_edge_detection_fragment_main",
            Rg8Unorm,
            &[(0, TextureView(scene)), (2, Sampler(&sampler))],
        );
        let calculate = make_pass(
            "SMAA blend weights",
            "blending_weight_calculation_vertex_main",
            "blending_weight_calculation_fragment_main",
            Rgba8Unorm,
            &[
                (3, TextureView(&edges)),
                (4, Sampler(&sampler)),
                (5, TextureView(&search)),
                (6, TextureView(&area)),
            ],
        );
        let blend = make_pass(
            "SMAA neighborhood blend",
            "neighborhood_blending_vertex_main",
            "neighborhood_blending_fragment_main",
            if scaled.is_some() {
                Rgba16Float
            } else {
                format
            },
            &[
                (0, TextureView(scene)),
                (7, TextureView(&weights)),
                (8, Sampler(&sampler)),
            ],
        );
        Self {
            edges,
            weights,
            scaled,
            detect,
            calculate,
            blend,
        }
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        self.detect.draw(encoder, &self.edges);
        self.calculate.draw(encoder, &self.weights);
        self.blend
            .draw(encoder, self.scaled.as_ref().unwrap_or(output));
    }
}
