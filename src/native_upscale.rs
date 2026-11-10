//! Native Vulkan SDK adapters; resources remain owned and tracked by wgpu.
use dlss_wgpu::super_resolution::{
    DlssSuperResolution, DlssSuperResolutionExposure, DlssSuperResolutionRenderParameters,
};
use dlss_wgpu::{DlssFeatureFlags, DlssPerfQualityMode, DlssSdk};
use glam::{UVec2, Vec2};
use std::sync::{Arc, Mutex};
use wgpu::hal::api::Vulkan;

pub const PROJECT_ID: uuid::Uuid = uuid::Uuid::from_u128(0x5b7ed90d_c279_42ef_a969_413ad4f861e8);

pub struct Native {
    dlss: Option<DlssSuperResolution>,
    fsr: Option<fsr::Context>,
    sdk: Option<Arc<Mutex<DlssSdk>>>,
    device: wgpu::Device,
    adapter: wgpu::Adapter,
    pub output: wgpu::TextureView,
    pub motion: wgpu::TextureView,
    pub linear_depth: wgpu::TextureView,
    mode: u8,
    quality: crate::settings::UpscaleQuality,
    size: (u32, u32),
    display: (u32, u32),
    pub dlss_status: String,
}
impl Native {
    pub fn new(device: &wgpu::Device, adapter: wgpu::Adapter, supported: bool) -> Self {
        let (sdk, dlss_status) = if supported {
            match DlssSdk::new(PROJECT_ID, device.clone()) {
                Ok(sdk) => (Some(sdk), "Available".into()),
                Err(e) => (None, format!("Unavailable: {e}")),
            }
        } else {
            (None, "Unsupported GPU or NGX runtime".into())
        };
        Self {
            dlss: None,
            fsr: None,
            sdk,
            device: device.clone(),
            adapter,
            output: output(device, (1, 1)),
            motion: crate::temporal::color(
                device,
                (1, 1),
                wgpu::TextureFormat::Rg16Float,
                "Motion vectors",
            ),
            linear_depth: crate::temporal::color(
                device,
                (1, 1),
                wgpu::TextureFormat::R32Float,
                "Native linear depth",
            ),
            mode: 0,
            quality: crate::settings::UpscaleQuality::Quality,
            size: (0, 0),
            display: (0, 0),
            dlss_status,
        }
    }
    pub fn configure(
        &mut self,
        mode: u8,
        quality: crate::settings::UpscaleQuality,
        size: (u32, u32),
        display: (u32, u32),
        queue: &wgpu::Queue,
    ) -> Result<(), String> {
        if mode == self.mode
            && quality == self.quality
            && size == self.size
            && display == self.display
        {
            return Ok(());
        }
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        self.dlss = None;
        self.destroy_fsr();
        self.mode = 0;
        if mode == 0 {
            return Ok(());
        }
        if size.0 > display.0 || size.1 > display.1 {
            return Err("Native upscalers require render resolution <= output resolution".into());
        }
        self.output = output(&self.device, display);
        self.linear_depth = crate::temporal::color(
            &self.device,
            size,
            wgpu::TextureFormat::R32Float,
            "Native depth copy",
        );
        self.motion = crate::temporal::color(
            &self.device,
            size,
            wgpu::TextureFormat::Rg16Float,
            "Motion vectors",
        );
        match mode {
            1 => {
                let sdk = self.sdk.as_ref().ok_or_else(|| self.dlss_status.clone())?;
                use crate::settings::UpscaleQuality;
                let dlss_quality = match quality {
                    UpscaleQuality::Quality => DlssPerfQualityMode::Quality,
                    UpscaleQuality::Balanced => DlssPerfQualityMode::Balanced,
                    UpscaleQuality::Performance => DlssPerfQualityMode::Performance,
                    UpscaleQuality::Native => DlssPerfQualityMode::Dlaa,
                };
                let context = DlssSuperResolution::new(
                    UVec2::new(display.0, display.1),
                    dlss_quality,
                    DlssFeatureFlags::HighDynamicRange
                        | DlssFeatureFlags::LowResolutionMotionVectors
                        | DlssFeatureFlags::AutoExposure,
                    Arc::clone(sdk),
                    &self.device,
                    queue,
                )
                .map_err(|e| e.to_string())?;
                let range = context.render_resolution_range();
                if size.0 < range.start().x
                    || size.1 < range.start().y
                    || size.0 > range.end().x
                    || size.1 > range.end().y
                {
                    return Err(format!(
                        "Render size {:?} outside DLSS range {:?}..={:?}",
                        size,
                        range.start(),
                        range.end()
                    ));
                }
                self.dlss = Some(context);
            }
            2 => {
                // SAFETY: wgpu owns all Vulkan handles. The context is destroyed only
                // after GPU completion and before the cloned wgpu device is dropped.
                let context = unsafe {
                    let hal = self
                        .device
                        .as_hal::<Vulkan>()
                        .ok_or("FSR requires Vulkan")?;
                    let shared = hal.shared_instance();
                    let interface = fsr::vk::get_interface(
                        shared.entry(),
                        shared.raw_instance(),
                        hal.raw_physical_device(),
                    )
                    .map_err(|e| e.to_string())?;
                    let device = fsr::vk::get_device(hal.raw_device().clone());
                    fsr::Context::new(fsr::ContextDescription {
                        interface,
                        device: &device,
                        display_size: [display.0, display.1],
                        max_render_size: [size.0, size.1],
                        flags: fsr::InitializationFlagBits::ENABLE_HIGH_DYNAMIC_RANGE
                            | fsr::InitializationFlagBits::ENABLE_AUTO_EXPOSURE,
                        message_callback: None,
                    })
                    .map_err(|e| e.to_string())?
                };
                self.fsr = Some(context);
            }
            _ => return Err("Unknown upscaler".into()),
        }
        self.size = size;
        self.display = display;
        self.mode = mode;
        self.quality = quality;
        Ok(())
    }
    fn destroy_fsr(&mut self) {
        if let Some(mut context) = self.fsr.take() {
            // SAFETY: configure and Drop wait for GPU completion first.
            if let Err(e) = unsafe { context.destroy() } {
                eprintln!("FSR cleanup: {e}");
            }
        }
    }
    pub fn active(&self) -> bool {
        self.mode != 0
    }
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        _depth: &wgpu::TextureView,
        jitter: Vec2,
        reset: bool,
        dt: f32,
    ) -> Result<(), String> {
        let depth = &self.linear_depth;
        if let Some(dlss) = &mut self.dlss {
            dlss.render(
                DlssSuperResolutionRenderParameters {
                    color,
                    depth,
                    motion_vectors: &self.motion,
                    exposure: DlssSuperResolutionExposure::Automatic,
                    bias: None,
                    dlss_output: &self.output,
                    reset,
                    jitter_offset: jitter,
                    partial_texture_size: Some(UVec2::new(self.size.0, self.size.1)),
                    motion_vector_scale: Some(Vec2::new(self.size.0 as f32, self.size.1 as f32)),
                },
                encoder,
                &self.adapter,
            )
            .map_err(|e| e.to_string())?;
        }
        if let Some(context) = &mut self.fsr {
            let inputs = [color, depth, &self.motion];
            encoder.transition_resources(
                std::iter::empty(),
                inputs
                    .into_iter()
                    .map(|v| wgpu::TextureTransition {
                        texture: v.texture(),
                        selector: None,
                        state: wgpu::TextureUses::RESOURCE,
                    })
                    .chain(std::iter::once(wgpu::TextureTransition {
                        texture: self.output.texture(),
                        selector: None,
                        state: wgpu::TextureUses::STORAGE_READ_WRITE,
                    })),
            );
            // SAFETY: transitions above match the SDK's declared initial layouts.
            // FSR restores external resources to those states after dispatch. The
            // encoder exclusively owns this command buffer for the entire callback.
            unsafe {
                let resource = |ctx: &mut fsr::Context,
                                view: &wgpu::TextureView,
                                format,
                                size,
                                state,
                                name|
                 -> Result<_, String> {
                    let image = {
                        view.texture()
                            .as_hal::<Vulkan>()
                            .ok_or("Missing Vulkan texture")?
                            .raw_handle()
                    };
                    let image_view = {
                        view.as_hal::<Vulkan>()
                            .ok_or("Missing Vulkan view")?
                            .raw_handle()
                    };
                    Ok(fsr::vk::get_texture_resource(
                        ctx, image, image_view, format, size, state, name,
                    ))
                };
                let size = [self.size.0, self.size.1];
                let color = resource(
                    context,
                    color,
                    ash::vk::Format::R16G16B16A16_SFLOAT,
                    size,
                    fsr::ResourceStates::COMPUTE_READ,
                    "Color",
                )?;
                let depth = resource(
                    context,
                    depth,
                    ash::vk::Format::R32_SFLOAT,
                    size,
                    fsr::ResourceStates::COMPUTE_READ,
                    "Depth",
                )?;
                let motion = resource(
                    context,
                    &self.motion,
                    ash::vk::Format::R16G16_SFLOAT,
                    size,
                    fsr::ResourceStates::COMPUTE_READ,
                    "Motion",
                )?;
                let output = resource(
                    context,
                    &self.output,
                    ash::vk::Format::R16G16B16A16_SFLOAT,
                    [self.display.0, self.display.1],
                    fsr::ResourceStates::UNORDERED_ACCESS,
                    "Output",
                )?;
                encoder.as_hal_mut::<Vulkan, _, _>(|raw| {
                    let mut desc = fsr::DispatchDescription::new(
                        raw.unwrap().raw_handle().into(),
                        color,
                        depth,
                        motion,
                        output,
                        dt * 1000.,
                        size,
                    )
                    .camera(0.05, 1024., 75_f32.to_radians());
                    desc.motion_vector_scale = [self.size.0 as f32, self.size.1 as f32];
                    desc.jitter_offset = jitter.to_array();
                    desc.reset = reset;
                    context.dispatch(desc).map_err(|e| e.to_string())
                })?;
            }
        }
        Ok(())
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        self.dlss = None;
        self.destroy_fsr();
    }
}
fn output(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Native upscale output"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
