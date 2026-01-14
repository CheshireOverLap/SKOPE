// SKOPE Engine - DDGI Pipeline
//
// Orchestrates the DDGI ray tracing and update passes.

use wgpu;
use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Mat4};
use super::{DdgiSystem, DdgiConfig, ProbeGrid};

/// DDGI Parameters (matches WGSL struct)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdgiParams {
    pub view_pos: [f32; 3],
    pub frame_index: u32,

    pub irradiance_hysteresis: f32,
    pub visibility_hysteresis: f32,
    pub max_ray_distance: f32,
    pub normal_bias: f32,

    pub irradiance_atlas_size: [u32; 2],
    pub visibility_atlas_size: [u32; 2],

    pub cascade_count: u32,
    pub active_cascade: u32,
    pub screen_size: [u32; 2],

    pub _pad: [u32; 2],
}

/// Camera uniform for DDGI ray tracing
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdgiCameraUniform {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub _pad: f32,
}

/// Ray result structure (matches WGSL)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RayResult {
    pub radiance: [f32; 3],
    pub distance: f32,
    pub normal: [f32; 3],
    pub hit: u32,
}

/// DDGI Probe Grid Parameters for material evaluation (matches material_eval.wgsl)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdgiProbeGridParams {
    // Cascade 0 (Near)
    pub origin_0: [f32; 3],
    pub spacing_0: f32,
    pub grid_size_0: [u32; 3],
    pub atlas_offset_0: u32,

    // Cascade 1 (Medium)
    pub origin_1: [f32; 3],
    pub spacing_1: f32,
    pub grid_size_1: [u32; 3],
    pub atlas_offset_1: u32,

    // Cascade 2 (Far)
    pub origin_2: [f32; 3],
    pub spacing_2: f32,
    pub grid_size_2: [u32; 3],
    pub atlas_offset_2: u32,

    // Atlas sizes
    pub irradiance_atlas_size: [f32; 2],
    pub visibility_atlas_size: [f32; 2],

    // Settings
    pub max_ray_distance: f32,
    pub normal_bias: f32,
    pub gi_intensity: f32,
    pub enabled: u32,
}

/// DDGI Pipeline - handles ray tracing and probe updates
pub struct DdgiPipeline {
    // Compute pipelines
    pub ray_trace_pipeline: wgpu::ComputePipeline,
    pub irradiance_update_pipeline: wgpu::ComputePipeline,
    pub visibility_update_pipeline: wgpu::ComputePipeline,

    // Bind group layouts
    pub ray_trace_layout_0: wgpu::BindGroupLayout,
    pub ray_trace_layout_1: wgpu::BindGroupLayout,
    pub update_layout_0: wgpu::BindGroupLayout,
    pub update_layout_1: wgpu::BindGroupLayout,

    // Uniform buffers
    pub params_buffer: wgpu::Buffer,
    pub camera_buffer: wgpu::Buffer,

    // Material eval params buffer (for indirect diffuse sampling)
    pub material_eval_params_buffer: wgpu::Buffer,

    // Ray results buffer
    pub ray_results_buffer: wgpu::Buffer,

    // Samplers
    pub linear_sampler: wgpu::Sampler,

    // Frame counter
    pub frame_index: u32,
}

impl DdgiPipeline {
    /// Maximum rays per dispatch
    const MAX_RAYS: u32 = 128 * 32 * 32 * 16;  // 128 rays * max probes in near cascade

    pub fn new(device: &wgpu::Device, _ddgi: &DdgiSystem) -> Self {
        // Create bind group layouts
        let ray_trace_layout_0 = Self::create_ray_trace_layout_0(device);
        let ray_trace_layout_1 = Self::create_ray_trace_layout_1(device);
        let update_layout_0 = Self::create_update_layout_0(device);
        let update_layout_1 = Self::create_update_layout_1(device);

        // Create uniform buffers
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DDGI Params Buffer"),
            size: std::mem::size_of::<DdgiParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DDGI Camera Buffer"),
            size: std::mem::size_of::<DdgiCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Material eval params buffer (for indirect diffuse in material shader)
        let material_eval_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DDGI Material Eval Params"),
            size: std::mem::size_of::<DdgiProbeGridParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Ray results buffer
        let ray_results_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DDGI Ray Results"),
            size: (std::mem::size_of::<RayResult>() * Self::MAX_RAYS as usize) as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Sampler
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DDGI Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create compute pipelines
        let ray_trace_pipeline = Self::create_ray_trace_pipeline(
            device,
            &ray_trace_layout_0,
            &ray_trace_layout_1,
        );

        let irradiance_update_pipeline = Self::create_irradiance_update_pipeline(
            device,
            &update_layout_0,
            &update_layout_1,
        );

        let visibility_update_pipeline = Self::create_visibility_update_pipeline(
            device,
            &update_layout_0,
            &update_layout_1,
        );

        log::info!("[DDGI Pipeline] Initialized");

        Self {
            ray_trace_pipeline,
            irradiance_update_pipeline,
            visibility_update_pipeline,
            ray_trace_layout_0,
            ray_trace_layout_1,
            update_layout_0,
            update_layout_1,
            params_buffer,
            camera_buffer,
            material_eval_params_buffer,
            ray_results_buffer,
            linear_sampler,
            frame_index: 0,
        }
    }

    fn create_ray_trace_layout_0(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Ray Trace Layout 0"),
            entries: &[
                // DdgiParams
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
                // ProbeGridUniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // CameraUniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Ray results (read-write)
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
        })
    }

    fn create_ray_trace_layout_1(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Ray Trace Layout 1"),
            entries: &[
                // HZB texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // HZB sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Scene color
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
                // Scene depth
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
                // Scene normal
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
        })
    }

    fn create_update_layout_0(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Update Layout 0"),
            entries: &[
                // DdgiParams
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
                // ProbeGridUniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Ray results (read)
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
            ],
        })
    }

    fn create_update_layout_1(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Update Layout 1"),
            entries: &[
                // Atlas texture (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_ray_trace_pipeline(
        device: &wgpu::Device,
        layout_0: &wgpu::BindGroupLayout,
        layout_1: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DDGI Ray Trace Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/ddgi/ray_trace.wgsl").into()
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DDGI Ray Trace Pipeline Layout"),
            bind_group_layouts: &[layout_0, layout_1],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DDGI Ray Trace Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_irradiance_update_pipeline(
        device: &wgpu::Device,
        layout_0: &wgpu::BindGroupLayout,
        _layout_1: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DDGI Irradiance Update Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/ddgi/irradiance_update.wgsl").into()
            ),
        });

        // Need different layout for irradiance atlas
        let irradiance_layout_1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Irradiance Update Layout 1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DDGI Irradiance Update Pipeline Layout"),
            bind_group_layouts: &[layout_0, &irradiance_layout_1],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DDGI Irradiance Update Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_visibility_update_pipeline(
        device: &wgpu::Device,
        layout_0: &wgpu::BindGroupLayout,
        _layout_1: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DDGI Visibility Update Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/ddgi/visibility_update.wgsl").into()
            ),
        });

        // Visibility uses RG16Float format
        let visibility_layout_1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Visibility Update Layout 1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,  // Rg16Float doesn't support STORAGE_BINDING on all GPUs
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DDGI Visibility Update Pipeline Layout"),
            bind_group_layouts: &[layout_0, &visibility_layout_1],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DDGI Visibility Update Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Update DDGI system for one frame
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        ddgi: &mut DdgiSystem,
        camera_pos: Vec3,
        view: Mat4,
        proj: Mat4,
        screen_size: (u32, u32),
        hzb_view: &wgpu::TextureView,
        scene_color_view: &wgpu::TextureView,
        scene_depth_view: &wgpu::TextureView,
        scene_normal_view: &wgpu::TextureView,
    ) {
        // Update DDGI center
        ddgi.update_center(camera_pos);

        // Upload camera uniform
        let view_proj = proj * view;
        let camera_uniform = DdgiCameraUniform {
            view: view.to_cols_array_2d(),
            proj: proj.to_cols_array_2d(),
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            position: camera_pos.to_array(),
            _pad: 0.0,
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        // Update params
        let params = DdgiParams {
            view_pos: camera_pos.to_array(),
            frame_index: self.frame_index,
            irradiance_hysteresis: ddgi.config.irradiance_hysteresis,
            visibility_hysteresis: ddgi.config.visibility_hysteresis,
            max_ray_distance: ddgi.config.max_ray_distance,
            normal_bias: 0.1,
            irradiance_atlas_size: [
                ddgi.irradiance_atlas.width(),
                ddgi.irradiance_atlas.height(),
            ],
            visibility_atlas_size: [
                ddgi.visibility_atlas.width(),
                ddgi.visibility_atlas.height(),
            ],
            cascade_count: 3,
            active_cascade: 0,  // Process near cascade this frame
            screen_size: [screen_size.0, screen_size.1],
            _pad: [0, 0],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Update material eval params buffer (for indirect diffuse sampling in material shader)
        let material_eval_params = DdgiProbeGridParams {
            // Cascade 0 (Near)
            origin_0: ddgi.cascades[0].origin.to_array(),
            spacing_0: ddgi.cascades[0].config.spacing,
            grid_size_0: [
                ddgi.cascades[0].config.grid_size.x as u32,
                ddgi.cascades[0].config.grid_size.y as u32,
                ddgi.cascades[0].config.grid_size.z as u32,
            ],
            atlas_offset_0: 0,

            // Cascade 1 (Medium)
            origin_1: ddgi.cascades[1].origin.to_array(),
            spacing_1: ddgi.cascades[1].config.spacing,
            grid_size_1: [
                ddgi.cascades[1].config.grid_size.x as u32,
                ddgi.cascades[1].config.grid_size.y as u32,
                ddgi.cascades[1].config.grid_size.z as u32,
            ],
            atlas_offset_1: ddgi.cascades[0].total_probes(),

            // Cascade 2 (Far)
            origin_2: ddgi.cascades[2].origin.to_array(),
            spacing_2: ddgi.cascades[2].config.spacing,
            grid_size_2: [
                ddgi.cascades[2].config.grid_size.x as u32,
                ddgi.cascades[2].config.grid_size.y as u32,
                ddgi.cascades[2].config.grid_size.z as u32,
            ],
            atlas_offset_2: ddgi.cascades[0].total_probes() + ddgi.cascades[1].total_probes(),

            // Atlas sizes
            irradiance_atlas_size: [
                ddgi.irradiance_atlas.width() as f32,
                ddgi.irradiance_atlas.height() as f32,
            ],
            visibility_atlas_size: [
                ddgi.visibility_atlas.width() as f32,
                ddgi.visibility_atlas.height() as f32,
            ],

            // Settings
            max_ray_distance: ddgi.config.max_ray_distance,
            normal_bias: 0.1,
            gi_intensity: 1.0,
            enabled: 1,
        };
        queue.write_buffer(&self.material_eval_params_buffer, 0, bytemuck::bytes_of(&material_eval_params));

        // Process one cascade per frame (round-robin)
        let cascade_idx = (self.frame_index % 3) as usize;
        let cascade = &mut ddgi.cascades[cascade_idx];
        cascade.upload_uniform(queue);

        let total_probes = cascade.total_probes();
        let rays_per_probe = 128u32;
        let total_rays = total_probes * rays_per_probe;

        // Create bind groups
        let ray_trace_bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DDGI Ray Trace BG 0"),
            layout: &self.ray_trace_layout_0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cascade.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.ray_results_buffer.as_entire_binding(),
                },
            ],
        });

        let ray_trace_bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DDGI Ray Trace BG 1"),
            layout: &self.ray_trace_layout_1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hzb_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(scene_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(scene_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(scene_normal_view),
                },
            ],
        });

        // Ray trace pass
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DDGI Ray Trace"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.ray_trace_pipeline);
            pass.set_bind_group(0, &ray_trace_bind_group_0, &[]);
            pass.set_bind_group(1, &ray_trace_bind_group_1, &[]);

            let workgroups = total_rays.div_ceil(64);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Update bind groups for irradiance/visibility update
        let update_bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DDGI Update BG 0"),
            layout: &self.update_layout_0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cascade.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.ray_results_buffer.as_entire_binding(),
                },
            ],
        });

        // Irradiance update layout
        let irradiance_layout_1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Irradiance BG Layout 1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let irradiance_bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DDGI Irradiance BG 1"),
            layout: &irradiance_layout_1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ddgi.irradiance_view),
                },
            ],
        });

        // Irradiance update pass
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DDGI Irradiance Update"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.irradiance_update_pipeline);
            pass.set_bind_group(0, &update_bind_group_0, &[]);
            pass.set_bind_group(1, &irradiance_bind_group_1, &[]);

            // Dispatch: 8x8 threads per probe, one probe per Z layer
            // Workgroup size is (8, 8, 1), dispatching total_probes in Z
            pass.dispatch_workgroups(1, 1, total_probes);
        }

        // Visibility update layout
        let visibility_layout_1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DDGI Visibility BG Layout 1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba16Float,  // Rg16Float doesn't support STORAGE_BINDING on all GPUs
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let visibility_bind_group_1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DDGI Visibility BG 1"),
            layout: &visibility_layout_1,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ddgi.visibility_view),
                },
            ],
        });

        // Visibility update pass
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DDGI Visibility Update"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.visibility_update_pipeline);
            pass.set_bind_group(0, &update_bind_group_0, &[]);
            pass.set_bind_group(1, &visibility_bind_group_1, &[]);

            // Dispatch: 16x16 threads per probe (2 workgroups), one probe per Z
            pass.dispatch_workgroups(2, 2, total_probes);
        }

        self.frame_index += 1;
    }
}
