// SKOPE Engine - Lumen Reflections
//
// Screen-trace based reflections with radiance cache fallback.
// Replaces traditional SSR with a more robust system that handles
// off-screen information via the radiance cache.
//
// Pipeline:
// 1. HZB screen trace for near-field reflections (high quality)
// 2. Radiance cache lookup for off-screen/far-field
// 3. Roughness-based branching (low roughness = precise, high = diffuse)
// 4. Spatiotemporal denoising for stability
//
// Inspired by UE5 Lumen Reflections (LumenReflections.cpp)

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};

/// Reflection parameters.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ReflectionParams {
    /// View matrix.
    pub view: [[f32; 4]; 4],
    /// Projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Inverse view-projection.
    pub inv_view_proj: [[f32; 4]; 4],
    /// Camera world position.
    pub camera_pos: [f32; 3],
    /// Maximum screen trace distance (NDC units).
    pub max_trace_distance: f32,
    /// Screen dimensions.
    pub screen_width: u32,
    pub screen_height: u32,
    /// Frame index for temporal jitter.
    pub frame_index: u32,
    /// Roughness threshold: below this, use precise screen trace.
    pub roughness_threshold: f32,
    /// Maximum HZB mip for tracing.
    pub max_hzb_mip: u32,
    /// Number of trace steps.
    pub max_steps: u32,
    /// Radiance cache grid size per axis.
    pub grid_size: u32,
    /// Radiance cache probe spacing in world units.
    pub probe_spacing: f32,
    /// Radiance cache grid origin (world-space center).
    pub cache_origin: [f32; 3],
    pub max_reflection_bounces: u32,
    pub max_refraction_bounces: u32,
    pub current_bounce: u32,
    pub enable_hit_lighting: u32,
    pub _pad: u32,
}

/// Lumen reflections pipeline.
///
/// Uses HZB-accelerated screen-space tracing with radiance cache fallback.
#[cfg(feature = "gpu")]
pub struct LumenReflectionsPipeline {
    // Screen trace pass
    trace_pipeline: wgpu::ComputePipeline,
    trace_layout: wgpu::BindGroupLayout,

    // Temporal filter pass
    temporal_pipeline: wgpu::ComputePipeline,
    temporal_layout: wgpu::BindGroupLayout,

    // Buffers
    params_buffer: wgpu::Buffer,

    // Output textures
    pub trace_output: wgpu::Texture,
    pub trace_output_view: wgpu::TextureView,
    pub filtered_output: wgpu::Texture,
    pub filtered_output_view: wgpu::TextureView,

    // History for temporal filtering
    pub history: wgpu::Texture,
    pub history_view: wgpu::TextureView,

    // Multi-bounce tile dispatch
    classify_pipeline: wgpu::ComputePipeline,
    classify_layout: wgpu::BindGroupLayout,
    compact_pipeline: wgpu::ComputePipeline,
    compact_layout: wgpu::BindGroupLayout,
    bounce_trace_pipeline: wgpu::ComputePipeline,
    bounce_trace_layout: wgpu::BindGroupLayout,

    // Tile buffers
    tile_classify_buffer: wgpu::Buffer,
    tile_indirect_args: wgpu::Buffer,
    compact_ray_buffer: wgpu::Buffer,
    compact_ray_count: wgpu::Buffer,

    // Bounce ping-pong textures
    bounce_radiance_a: wgpu::Texture,
    bounce_radiance_a_view: wgpu::TextureView,
    bounce_radiance_b: wgpu::Texture,
    bounce_radiance_b_view: wgpu::TextureView,
    bounce_hit_data_a: wgpu::Texture,
    bounce_hit_data_a_view: wgpu::TextureView,
    bounce_hit_data_b: wgpu::Texture,
    bounce_hit_data_b_view: wgpu::TextureView,

    // Spatial bilateral filter (after temporal)
    spatial_pipeline: wgpu::ComputePipeline,
    spatial_layout: wgpu::BindGroupLayout,
    pub spatial_output: wgpu::Texture,
    pub spatial_output_view: wgpu::TextureView,

    pub max_bounces: u32,

    width: u32,
    height: u32,
}

#[cfg(feature = "gpu")]
impl LumenReflectionsPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Trace layout: params + depth + normal/roughness + HZB + HDR color + radiance cache + output
        let trace_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Reflections Trace Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: normal/roughness
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
                // binding 3: HZB
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
                // binding 4: HDR color (for hit color lookup)
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
                // binding 5: radiance cache probes (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: trace output (storage, write)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        // Temporal filter layout
        let temporal_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Reflections Temporal Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: current trace output
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
                // binding 2: history
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
                // binding 3: velocity
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
                // binding 4: filtered output (write)
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
                // binding 5: depth texture (for disocclusion detection)
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
            ],
        });

        // Shaders
        let trace_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Reflections Trace Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_reflections_trace.wgsl").into(),
            ),
        });

        let temporal_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Reflections Temporal Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_reflections_temporal.wgsl").into(),
            ),
        });

        // Pipelines
        let trace_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Reflections Trace Layout"),
            bind_group_layouts: &[&trace_layout],
            immediate_size: 0,
        });
        let trace_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Reflections Trace"),
            layout: Some(&trace_pipe_layout),
            module: &trace_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let temporal_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Reflections Temporal Layout"),
            bind_group_layouts: &[&temporal_layout],
            immediate_size: 0,
        });
        let temporal_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Reflections Temporal"),
            layout: Some(&temporal_pipe_layout),
            module: &temporal_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Reflections Params"),
            size: std::mem::size_of::<ReflectionParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Textures (COPY_SRC | COPY_DST for swap_history copies)
        let create_tex = |label: &str| -> (wgpu::Texture, wgpu::TextureView) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };

        let (trace_output, trace_output_view) = create_tex("Lumen Reflections Trace Output");
        let (filtered_output, filtered_output_view) = create_tex("Lumen Reflections Filtered");
        let (history, history_view) = create_tex("Lumen Reflections History");

        // ----- Multi-bounce tile dispatch -----

        let classify_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Reflections Classify Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_reflections_classify.wgsl").into(),
            ),
        });
        let compact_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Reflections Compact Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_reflections_compact.wgsl").into(),
            ),
        });
        let bounce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Reflections Bounce Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_reflections_bounce.wgsl").into(),
            ),
        });

        // Classify bind group layout
        let classify_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Reflections Classify Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: normal_roughness
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
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: tile_classify_buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Compact bind group layout
        let compact_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Reflections Compact Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: tile_classify_buffer (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: compact_ray_buffer (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: compact_ray_count (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Bounce trace bind group layout
        let bounce_trace_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Reflections Bounce Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: compact_ray_buffer (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: compact_ray_count (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: depth
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: hzb
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
                // binding 5: hdr_color
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
                // binding 6: radiance_cache (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 7: bounce_radiance_in (texture)
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
                // binding 8: bounce_radiance_out (storage texture, write)
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 9: bounce_hit_data_out (storage texture, write)
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 10: tile_data (storage, read) — used to early-exit TILE_SKIP tiles
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 11: normal_roughness texture (for actual surface normals)
                wgpu::BindGroupLayoutEntry {
                    binding: 11,
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

        // Classify pipeline
        let classify_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Reflections Classify Layout"),
            bind_group_layouts: &[&classify_layout],
            immediate_size: 0,
        });
        let classify_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Reflections Classify"),
            layout: Some(&classify_pipe_layout),
            module: &classify_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Compact pipeline
        let compact_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Reflections Compact Layout"),
            bind_group_layouts: &[&compact_layout],
            immediate_size: 0,
        });
        let compact_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Reflections Compact"),
            layout: Some(&compact_pipe_layout),
            module: &compact_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Bounce trace pipeline
        let bounce_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Reflections Bounce Layout"),
            bind_group_layouts: &[&bounce_trace_layout],
            immediate_size: 0,
        });
        let bounce_trace_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Reflections Bounce"),
            layout: Some(&bounce_pipe_layout),
            module: &bounce_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Tile buffers
        let tiles_x = (width + 7) / 8;
        let tiles_y = (height + 7) / 8;
        let total_tiles = tiles_x * tiles_y;

        let tile_classify_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Reflections Tile Classify"),
            size: (total_tiles * 4) as u64,  // u32 per tile (roughness bucket)
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tile_indirect_args = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Reflections Tile Indirect Args"),
            size: 12,  // 3 u32s for indirect dispatch
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compact_ray_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Reflections Compact Rays"),
            size: (width * height * 16) as u64,  // vec4 per ray
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compact_ray_count = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Reflections Ray Count"),
            size: 16,  // atomic counter + pad
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bounce ping-pong textures
        let create_bounce_tex = |label: &str| -> (wgpu::Texture, wgpu::TextureView) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };

        let (bounce_radiance_a, bounce_radiance_a_view) = create_bounce_tex("Bounce Radiance A");
        let (bounce_radiance_b, bounce_radiance_b_view) = create_bounce_tex("Bounce Radiance B");
        let (bounce_hit_data_a, bounce_hit_data_a_view) = create_bounce_tex("Bounce Hit Data A");
        let (bounce_hit_data_b, bounce_hit_data_b_view) = create_bounce_tex("Bounce Hit Data B");

        // Spatial bilateral filter pipeline (after temporal)
        let spatial_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Reflections Spatial Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_reflections_spatial.wgsl").into(),
            ),
        });
        let spatial_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Reflections Spatial Layout"),
            entries: &[
                // binding 0: params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: input (temporal output)
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
                // binding 4: output (spatial filtered)
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
        let spatial_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Reflections Spatial Layout"),
            bind_group_layouts: &[&spatial_layout],
            immediate_size: 0,
        });
        let spatial_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Reflections Spatial"),
            layout: Some(&spatial_pipe_layout),
            module: &spatial_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let (spatial_output, spatial_output_view) = create_tex("Lumen Reflections Spatial Output");

        Self {
            trace_pipeline,
            trace_layout,
            temporal_pipeline,
            temporal_layout,
            params_buffer,
            trace_output,
            trace_output_view,
            filtered_output,
            filtered_output_view,
            history,
            history_view,
            classify_pipeline,
            classify_layout,
            compact_pipeline,
            compact_layout,
            bounce_trace_pipeline,
            bounce_trace_layout,
            tile_classify_buffer,
            tile_indirect_args,
            compact_ray_buffer,
            compact_ray_count,
            bounce_radiance_a,
            bounce_radiance_a_view,
            bounce_radiance_b,
            bounce_radiance_b_view,
            bounce_hit_data_a,
            bounce_hit_data_a_view,
            bounce_hit_data_b,
            bounce_hit_data_b_view,
            spatial_pipeline,
            spatial_layout,
            spatial_output,
            spatial_output_view,
            max_bounces: 3,
            width,
            height,
        }
    }

    /// Dispatch the reflection trace pass.
    pub fn trace(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &ReflectionParams,
        depth_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        hzb_view: &wgpu::TextureView,
        hdr_view: &wgpu::TextureView,
        radiance_cache_buffer: &wgpu::Buffer,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lumen Reflections Trace BG"),
            layout: &self.trace_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(hzb_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(hdr_view) },
                wgpu::BindGroupEntry { binding: 5, resource: radiance_cache_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.trace_output_view) },
            ],
        });

        let dx = self.width.div_ceil(8);
        let dy = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Lumen Reflections Trace"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.trace_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(dx, dy, 1);
    }

    /// Dispatch the temporal filter pass and swap history.
    pub fn temporal_filter(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &ReflectionParams,
        velocity_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lumen Reflections Temporal BG"),
            layout: &self.temporal_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.trace_output_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.history_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(velocity_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.filtered_output_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(depth_view) },
            ],
        });

        let dx = self.width.div_ceil(8);
        let dy = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Lumen Reflections Temporal"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.temporal_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(dx, dy, 1);
    }

    /// Dispatch spatial bilateral filter after temporal accumulation.
    ///
    /// Reads the temporally filtered output, applies roughness-adaptive
    /// bilateral filtering, and writes to `spatial_output`.
    pub fn spatial_filter(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lumen Reflections Spatial BG"),
            layout: &self.spatial_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.filtered_output_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.spatial_output_view) },
            ],
        });

        let dx = self.width.div_ceil(8);
        let dy = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Lumen Reflections Spatial"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.spatial_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(dx, dy, 1);
    }

    /// Copy spatial output to history for next frame's temporal filter.
    pub fn swap_history(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.spatial_output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.history,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Classify 8x8 tiles by roughness for ray dispatch.
    pub fn classify_tiles(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        normal_roughness_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lumen Reflections Classify BG"),
            layout: &self.classify_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 3, resource: self.tile_classify_buffer.as_entire_binding() },
            ],
        });
        let tiles_x = self.width.div_ceil(8);
        let tiles_y = self.height.div_ceil(8);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Lumen Reflections Tile Classify"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.classify_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(tiles_x, tiles_y, 1);
    }

    /// Pack active reflection rays into contiguous buffer.
    pub fn compact_traces(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // Reset ray count
        queue.write_buffer(&self.compact_ray_count, 0, &[0u8; 16]);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lumen Reflections Compact BG"),
            layout: &self.compact_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.tile_classify_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.compact_ray_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.compact_ray_count.as_entire_binding() },
            ],
        });
        let tiles_x = self.width.div_ceil(8);
        let tiles_y = self.height.div_ceil(8);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Lumen Reflections Compact"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compact_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(tiles_x, tiles_y, 1);
    }

    /// Trace a single bounce: HZB screen trace + radiance cache fallback.
    pub fn bounce_trace(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        _params: &ReflectionParams,
        depth_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        hzb_view: &wgpu::TextureView,
        hdr_view: &wgpu::TextureView,
        radiance_cache_buffer: &wgpu::Buffer,
        bounce_index: u32,
    ) {
        let (rad_in_view, rad_out_view, hit_out_view) = if bounce_index % 2 == 0 {
            (&self.bounce_radiance_a_view, &self.bounce_radiance_b_view, &self.bounce_hit_data_b_view)
        } else {
            (&self.bounce_radiance_b_view, &self.bounce_radiance_a_view, &self.bounce_hit_data_a_view)
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lumen Reflections Bounce BG"),
            layout: &self.bounce_trace_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.compact_ray_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.compact_ray_count.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(hzb_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(hdr_view) },
                wgpu::BindGroupEntry { binding: 6, resource: radiance_cache_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(rad_in_view) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(rad_out_view) },
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(hit_out_view) },
                wgpu::BindGroupEntry { binding: 10, resource: self.tile_classify_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
            ],
        });

        let dx = self.width.div_ceil(8);
        let dy = self.height.div_ceil(8);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(&format!("Lumen Reflections Bounce {}", bounce_index)),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.bounce_trace_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(dx, dy, 1);
    }

    /// Dispatch multi-bounce reflections: classify → compact → bounce loop → temporal
    pub fn trace_multi_bounce(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &ReflectionParams,
        depth_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
        hzb_view: &wgpu::TextureView,
        hdr_view: &wgpu::TextureView,
        radiance_cache_buffer: &wgpu::Buffer,
    ) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));

        // 1. Tile classification
        self.classify_tiles(device, encoder, normal_roughness_view, depth_view);

        // NOTE: compact_traces skipped — bounce shader uses tile_data for early exit
        // instead of compacted ray buffer. Saves a full-screen compute dispatch.

        // 2. Multi-bounce loop
        for bounce in 0..self.max_bounces.min(params.max_reflection_bounces) {
            let mut bounce_params = *params;
            bounce_params.current_bounce = bounce;
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&bounce_params));

            self.bounce_trace(
                device, encoder, &bounce_params,
                depth_view, normal_roughness_view, hzb_view, hdr_view,
                radiance_cache_buffer, bounce,
            );
        }

        // 4. Copy final bounce result to trace_output for temporal filter
        let final_view = if (self.max_bounces.min(params.max_reflection_bounces)) % 2 == 0 {
            &self.bounce_radiance_a
        } else {
            &self.bounce_radiance_b
        };
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: final_view,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.trace_output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
    }

    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.spatial_output_view
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}
