//! ReSTIR Screen Probe Gather
//!
//! Reservoir-based spatiotemporal importance resampling for screen probe gathering.
//! Replaces direct 8-direction tracing with importance-weighted reservoir sampling
//! for better quality at the same ray budget.
//!
//! Pipeline:
//! 1. Initial Trace — cast rays, build initial reservoirs
//! 2. Temporal Resample — combine with previous frame reservoirs
//! 3. Spatial Resample — share samples between nearby pixels
//! 4. Upsample — reconstruct full-resolution from half-res reservoirs
//! 5. Bilateral Filter — edge-aware noise reduction

#[cfg(feature = "gpu")]
use crate::types::ReSTIRParams;

/// Reservoir texture set for one ping-pong buffer.
///
/// Each reservoir stores per-pixel ray direction, traced radiance,
/// hit distance, hit normal, and importance weights at half resolution.
#[cfg(feature = "gpu")]
pub struct ReservoirTextures {
    pub ray_direction: wgpu::Texture,
    pub ray_direction_view: wgpu::TextureView,
    pub trace_radiance: wgpu::Texture,
    pub trace_radiance_view: wgpu::TextureView,
    pub hit_distance: wgpu::Texture,
    pub hit_distance_view: wgpu::TextureView,
    pub hit_normal: wgpu::Texture,
    pub hit_normal_view: wgpu::TextureView,
    pub weights: wgpu::Texture,
    pub weights_view: wgpu::TextureView,
}

#[cfg(feature = "gpu")]
impl ReservoirTextures {
    fn new(device: &wgpu::Device, width: u32, height: u32, label_prefix: &str) -> Self {
        let create_tex = |label: &str, format: wgpu::TextureFormat| -> (wgpu::Texture, wgpu::TextureView) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
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
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };

        let (ray_direction, ray_direction_view) =
            create_tex(&format!("{label_prefix} Ray Direction"), wgpu::TextureFormat::Rgba16Float);
        let (trace_radiance, trace_radiance_view) =
            create_tex(&format!("{label_prefix} Trace Radiance"), wgpu::TextureFormat::Rgba16Float);
        let (hit_distance, hit_distance_view) =
            create_tex(&format!("{label_prefix} Hit Distance"), wgpu::TextureFormat::R16Float);
        let (hit_normal, hit_normal_view) =
            create_tex(&format!("{label_prefix} Hit Normal"), wgpu::TextureFormat::Rgba8Snorm);
        let (weights, weights_view) =
            create_tex(&format!("{label_prefix} Weights"), wgpu::TextureFormat::Rgba16Float);

        Self {
            ray_direction,
            ray_direction_view,
            trace_radiance,
            trace_radiance_view,
            hit_distance,
            hit_distance_view,
            hit_normal,
            hit_normal_view,
            weights,
            weights_view,
        }
    }
}

/// ReSTIR pipeline for screen probe gathering.
///
/// Manages the full 5-pass pipeline: initial trace, temporal resample,
/// spatial resample, upsample, and bilateral filter. Uses double-buffered
/// reservoirs (ping-pong) for temporal resampling across frames.
#[cfg(feature = "gpu")]
pub struct ReSTIRPipeline {
    // Reservoir double-buffer (ping-pong)
    pub reservoir_a: ReservoirTextures,
    pub reservoir_b: ReservoirTextures,

    // Pipelines
    pub initial_trace_pipeline: wgpu::ComputePipeline,
    pub initial_trace_layout_g0: wgpu::BindGroupLayout,
    pub initial_trace_layout_g1: wgpu::BindGroupLayout,
    pub temporal_resample_pipeline: wgpu::ComputePipeline,
    pub temporal_resample_layout_g0: wgpu::BindGroupLayout,
    pub temporal_resample_layout_g1: wgpu::BindGroupLayout,
    pub spatial_resample_pipeline: wgpu::ComputePipeline,
    pub spatial_resample_layout_g0: wgpu::BindGroupLayout,
    pub spatial_resample_layout_g1: wgpu::BindGroupLayout,
    pub upsample_pipeline: wgpu::ComputePipeline,
    pub upsample_layout_g0: wgpu::BindGroupLayout,
    pub upsample_layout_g1: wgpu::BindGroupLayout,
    pub bilateral_filter_pipeline: wgpu::ComputePipeline,
    pub bilateral_filter_layout_g0: wgpu::BindGroupLayout,
    pub bilateral_filter_layout_g1: wgpu::BindGroupLayout,

    pub params_buffer: wgpu::Buffer,
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    // Intermediate texture for bilateral filter input (upsample writes here)
    pub intermediate_texture: wgpu::Texture,
    pub intermediate_view: wgpu::TextureView,

    reservoir_width: u32,
    reservoir_height: u32,
    width: u32,
    height: u32,
    frame_parity: u32,
}

#[cfg(feature = "gpu")]
impl ReSTIRPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let reservoir_width = width / 2;
        let reservoir_height = height / 2;

        // ── Reservoir textures (double-buffered) ──────────────────
        let reservoir_a = ReservoirTextures::new(device, reservoir_width, reservoir_height, "ReSTIR A");
        let reservoir_b = ReservoirTextures::new(device, reservoir_width, reservoir_height, "ReSTIR B");

        // ── Output textures ───────────────────────────────────────
        let create_full_res_tex = |label: &str| -> (wgpu::Texture, wgpu::TextureView) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };

        let (output_texture, output_view) = create_full_res_tex("ReSTIR Output");
        let (intermediate_texture, intermediate_view) = create_full_res_tex("ReSTIR Intermediate");

        // ── Params buffer ─────────────────────────────────────────
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ReSTIR Params"),
            size: std::mem::size_of::<ReSTIRParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Initial Trace pipeline ────────────────────────────────
        let (initial_trace_pipeline, initial_trace_layout_g0, initial_trace_layout_g1) =
            Self::create_initial_trace_pipeline(device);

        // ── Temporal Resample pipeline ────────────────────────────
        let (temporal_resample_pipeline, temporal_resample_layout_g0, temporal_resample_layout_g1) =
            Self::create_temporal_resample_pipeline(device);

        // ── Spatial Resample pipeline ─────────────────────────────
        let (spatial_resample_pipeline, spatial_resample_layout_g0, spatial_resample_layout_g1) =
            Self::create_spatial_resample_pipeline(device);

        // ── Upsample pipeline ─────────────────────────────────────
        let (upsample_pipeline, upsample_layout_g0, upsample_layout_g1) =
            Self::create_upsample_pipeline(device);

        // ── Bilateral Filter pipeline ─────────────────────────────
        let (bilateral_filter_pipeline, bilateral_filter_layout_g0, bilateral_filter_layout_g1) =
            Self::create_bilateral_filter_pipeline(device);

        Self {
            reservoir_a,
            reservoir_b,
            initial_trace_pipeline,
            initial_trace_layout_g0,
            initial_trace_layout_g1,
            temporal_resample_pipeline,
            temporal_resample_layout_g0,
            temporal_resample_layout_g1,
            spatial_resample_pipeline,
            spatial_resample_layout_g0,
            spatial_resample_layout_g1,
            upsample_pipeline,
            upsample_layout_g0,
            upsample_layout_g1,
            bilateral_filter_pipeline,
            bilateral_filter_layout_g0,
            bilateral_filter_layout_g1,
            params_buffer,
            output_texture,
            output_view,
            intermediate_texture,
            intermediate_view,
            reservoir_width,
            reservoir_height,
            width,
            height,
            frame_parity: 0,
        }
    }

    // ── Pipeline creation helpers ─────────────────────────────────

    /// Create the initial trace pipeline.
    ///
    /// G0: params(uniform), depth(texture_2d), normal_roughness(texture_2d),
    ///     hzb(texture_2d), hdr_color(texture_2d)
    /// G1: reservoir_out (5 storage textures write)
    fn create_initial_trace_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Initial Trace G0"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ReSTIRParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // binding 1: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: normal_roughness
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: hzb
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: hdr_color
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Initial Trace G1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Snorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ReSTIR Initial Trace Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_restir_initial_trace.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ReSTIR Initial Trace Layout"),
            bind_group_layouts: &[&g0, &g1],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ReSTIR Initial Trace"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1)
    }

    /// Create the temporal resample pipeline.
    ///
    /// G0: params(uniform), current reservoir (5 textures read),
    ///     previous reservoir (3 textures read: ray_dir, radiance, weights),
    ///     depth(texture_2d), velocity(texture_2d)
    /// G1: output reservoir (5 storage textures write)
    fn create_temporal_resample_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Temporal Resample G0"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ReSTIRParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // binding 1: curr_ray_dir
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: curr_radiance
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: curr_hit_dist
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: curr_hit_normal
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 5: curr_weights
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 6: prev_ray_dir
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 7: prev_radiance
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 8: prev_weights
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 9: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 10: velocity
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Temporal Resample G1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Snorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ReSTIR Temporal Resample Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_restir_temporal_resample.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ReSTIR Temporal Resample Layout"),
            bind_group_layouts: &[&g0, &g1],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ReSTIR Temporal Resample"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1)
    }

    /// Create the spatial resample pipeline.
    ///
    /// G0: params(uniform), input reservoir (5 textures read),
    ///     depth(texture_2d), normal_roughness(texture_2d)
    /// G1: output reservoir (5 storage textures write)
    fn create_spatial_resample_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Spatial Resample G0"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ReSTIRParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // binding 1: in_ray_dir
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: in_radiance
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: in_hit_dist
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: in_hit_normal
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 5: in_weights
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 6: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 7: normal_roughness
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Spatial Resample G1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Snorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ReSTIR Spatial Resample Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_restir_spatial_resample.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ReSTIR Spatial Resample Layout"),
            bind_group_layouts: &[&g0, &g1],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ReSTIR Spatial Resample"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1)
    }

    /// Create the upsample pipeline.
    ///
    /// G0: params(uniform), reservoir_radiance(texture_2d), reservoir_weights(texture_2d),
    ///     depth_full(texture_2d), normal_full(texture_2d)
    /// G1: output(storage_2d rgba16float write)
    fn create_upsample_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Upsample G0"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ReSTIRParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // binding 1: reservoir_radiance
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: reservoir_weights
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: depth_full
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: normal_roughness_full
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Upsample G1"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ReSTIR Upsample Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_restir_upsample.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ReSTIR Upsample Layout"),
            bind_group_layouts: &[&g0, &g1],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ReSTIR Upsample"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1)
    }

    /// Create the bilateral filter pipeline.
    ///
    /// G0: params(uniform), input(texture_2d), depth(texture_2d), normal_roughness(texture_2d)
    /// G1: output(storage_2d rgba16float write)
    fn create_bilateral_filter_pipeline(
        device: &wgpu::Device,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Bilateral Filter G0"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ReSTIRParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // binding 1: input
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: normal_roughness
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ReSTIR Bilateral Filter G1"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba16Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ReSTIR Bilateral Filter Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_restir_bilateral_filter.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ReSTIR Bilateral Filter Layout"),
            bind_group_layouts: &[&g0, &g1],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ReSTIR Bilateral Filter"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1)
    }

    // ── Execution ─────────────────────────────────────────────────

    /// Execute the full 5-pass ReSTIR pipeline.
    ///
    /// Writes params to the uniform buffer, runs initial trace, temporal
    /// resample, spatial resample, upsample, and bilateral filter passes.
    /// The final result is available via `output_view()`.
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &ReSTIRParams,
        depth_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        hzb_view: &wgpu::TextureView,
        hdr_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));

        let (src, dst) = if self.frame_parity == 0 {
            (&self.reservoir_a, &self.reservoir_b)
        } else {
            (&self.reservoir_b, &self.reservoir_a)
        };

        let res_dx = (self.reservoir_width + 7) / 8;
        let res_dy = (self.reservoir_height + 7) / 8;
        let full_dx = (self.width + 7) / 8;
        let full_dy = (self.height + 7) / 8;

        // ── Pass 1: Initial Trace → writes to dst reservoir ──────
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Initial Trace BG0"),
                layout: &self.initial_trace_layout_g0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(hzb_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(hdr_view) },
                ],
            });

            // Write initial trace to SRC so that spatial resample (which reads src)
            // can directly consume it without the temporal pass.
            // Temporal resample is disabled because it reads+writes the same reservoir
            // in one dispatch (wgpu resource aliasing violation). Needs triple buffering.
            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Initial Trace BG1"),
                layout: &self.initial_trace_layout_g1,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src.ray_direction_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.trace_radiance_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&src.hit_distance_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&src.hit_normal_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&src.weights_view) },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ReSTIR Initial Trace"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.initial_trace_pipeline);
            pass.set_bind_group(0, &bg0, &[]);
            pass.set_bind_group(1, &bg1, &[]);
            pass.dispatch_workgroups(res_dx, res_dy, 1);
        }

        // ── Pass 2: Temporal Resample — DISABLED ──
        // Temporal resample reads+writes the same reservoir set in one dispatch,
        // causing wgpu resource aliasing violation. Needs triple buffering to fix.
        // Initial trace now writes directly to src, so spatial resample can
        // consume it without the temporal merge step.

        // ── Pass 3: Spatial Resample: src → dst ──────────────────
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Spatial Resample BG0"),
                layout: &self.spatial_resample_layout_g0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&src.ray_direction_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&src.trace_radiance_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&src.hit_distance_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&src.hit_normal_view) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&src.weights_view) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(depth_view) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                ],
            });

            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Spatial Resample BG1"),
                layout: &self.spatial_resample_layout_g1,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&dst.ray_direction_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&dst.trace_radiance_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&dst.hit_distance_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&dst.hit_normal_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&dst.weights_view) },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ReSTIR Spatial Resample"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.spatial_resample_pipeline);
            pass.set_bind_group(0, &bg0, &[]);
            pass.set_bind_group(1, &bg1, &[]);
            pass.dispatch_workgroups(res_dx, res_dy, 1);
        }

        // ── Pass 4: Upsample: dst reservoir → intermediate (full res) ──
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Upsample BG0"),
                layout: &self.upsample_layout_g0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&dst.trace_radiance_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&dst.weights_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                ],
            });

            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Upsample BG1"),
                layout: &self.upsample_layout_g1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.intermediate_view),
                }],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ReSTIR Upsample"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.upsample_pipeline);
            pass.set_bind_group(0, &bg0, &[]);
            pass.set_bind_group(1, &bg1, &[]);
            pass.dispatch_workgroups(full_dx, full_dy, 1);
        }

        // ── Pass 5: Bilateral Filter: intermediate → output ──────
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Bilateral Filter BG0"),
                layout: &self.bilateral_filter_layout_g0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.intermediate_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(depth_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                ],
            });

            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ReSTIR Bilateral Filter BG1"),
                layout: &self.bilateral_filter_layout_g1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                }],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ReSTIR Bilateral Filter"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.bilateral_filter_pipeline);
            pass.set_bind_group(0, &bg0, &[]);
            pass.set_bind_group(1, &bg1, &[]);
            pass.dispatch_workgroups(full_dx, full_dy, 1);
        }

        self.frame_parity = 1 - self.frame_parity;
    }

    /// Get the final output texture view.
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.output_view
    }

    /// Resize all textures for a new viewport size.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}
