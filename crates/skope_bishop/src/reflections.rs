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
    pub _pad: [u32; 2],
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

        // Textures
        let create_tex = |label: &str| -> (wgpu::Texture, wgpu::TextureView) {
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

        let (trace_output, trace_output_view) = create_tex("Lumen Reflections Trace Output");
        let (filtered_output, filtered_output_view) = create_tex("Lumen Reflections Filtered");
        let (history, history_view) = create_tex("Lumen Reflections History");

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

    /// Copy filtered output to history for next frame.
    pub fn swap_history(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.filtered_output,
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

    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.filtered_output_view
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}
