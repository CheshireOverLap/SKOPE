// SKOPE Engine — Distance Field System
//
// Provides SDF-based rendering effects:
// 1. Global Distance Field (GDF) — composite SDF volume for the scene
// 2. DF Soft Shadows — sphere-traced penumbra shadows
// 3. DF Ambient Occlusion — large-scale AO from distant geometry
//
// The GDF is a 3D texture (R16Float) centered on the camera,
// updated incrementally as the camera moves. Mesh SDFs are
// composited into the GDF using a min() operation.
//
// Reference: UE5 GlobalDistanceField.cpp, DistanceFieldShadowing.usf

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// GDF volume configuration
#[derive(Debug, Clone, Copy)]
pub struct GDFConfig {
    /// Resolution per axis (e.g., 128 = 128^3 volume)
    pub resolution: u32,
    /// World-space extent of the volume (cube side length)
    pub extent: f32,
}

impl Default for GDFConfig {
    fn default() -> Self {
        Self {
            resolution: 128,
            extent: 200.0, // 200 world units (meters)
        }
    }
}

/// DF shadow parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DFShadowParams {
    pub inv_view_proj: [[f32; 4]; 4],
    pub light_direction: [f32; 3],
    pub light_angle: f32,
    pub camera_pos: [f32; 3],
    pub max_trace_dist: f32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub volume_origin: [f32; 2],
    pub volume_origin_y: f32,
    pub volume_extent: f32,
    pub volume_resolution: u32,
    pub _pad: u32,
}

/// SDF voxelization parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VoxelizeParams {
    pub volume_origin: [f32; 3],
    pub voxel_size: f32,
    pub resolution: u32,
    pub instance_count: u32,
    pub _pad: [u32; 2],
}

/// DF AO parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DFAOParams {
    pub inv_view_proj: [[f32; 4]; 4],
    pub screen_width: u32,
    pub screen_height: u32,
    pub volume_origin: [f32; 2],
    pub volume_origin_y: f32,
    pub volume_extent: f32,
    pub volume_resolution: u32,
    pub max_distance: f32,
    pub ao_strength: f32,
    pub num_steps: u32,
    pub _pad: [u32; 2],
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

pub struct DistanceFieldSystem {
    // Global Distance Field volume
    pub gdf_volume: wgpu::Texture,
    pub gdf_view: wgpu::TextureView,
    pub gdf_sampler: wgpu::Sampler,

    // SDF Voxelization
    voxelize_pipeline: wgpu::ComputePipeline,
    voxelize_layout: wgpu::BindGroupLayout,
    voxelize_params_buffer: wgpu::Buffer,

    // DF Shadows
    shadow_pipeline: wgpu::ComputePipeline,
    shadow_layout: wgpu::BindGroupLayout,
    shadow_params_buffer: wgpu::Buffer,
    pub shadow_output: wgpu::Texture,
    pub shadow_output_view: wgpu::TextureView,

    // DF AO
    ao_pipeline: wgpu::ComputePipeline,
    ao_layout: wgpu::BindGroupLayout,
    ao_params_buffer: wgpu::Buffer,
    pub ao_output: wgpu::Texture,
    pub ao_output_view: wgpu::TextureView,

    // Config
    pub config: GDFConfig,
    screen_width: u32,
    screen_height: u32,

    // Volume tracking
    pub volume_origin: [f32; 3],
}

impl DistanceFieldSystem {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, config: GDFConfig) -> Self {
        let gdf_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GDF Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // GDF volume (R32Float for signed distances; R16Float doesn't support STORAGE_BINDING)
        let gdf_volume = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Global Distance Field Volume"),
            size: wgpu::Extent3d {
                width: config.resolution,
                height: config.resolution,
                depth_or_array_layers: config.resolution,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let gdf_view = gdf_volume.create_view(&wgpu::TextureViewDescriptor::default());

        // Voxelize layout + pipeline
        let voxelize_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SDF Voxelize Layout"),
            entries: &[
                bgl_uniform(0),                    // VoxelizeParams
                bgl_storage_read(1),               // GpuInstance array
                bgl_storage_tex_3d(2, wgpu::TextureFormat::R32Float), // SDF output
            ],
        });

        let voxelize_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Voxelize Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sdf_voxelize.wgsl").into()),
        });

        let voxelize_pipeline = create_compute_pipeline(device, "SDF Voxelize", &voxelize_shader, &voxelize_layout);

        let voxelize_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Voxelize Params"),
            size: std::mem::size_of::<VoxelizeParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shadow layout + pipeline
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DF Shadow Layout"),
            entries: &[
                bgl_uniform(0),       // params
                bgl_depth_tex(1),     // depth
                bgl_3d_tex(2),        // GDF volume
                bgl_sampler(3),       // GDF sampler
                bgl_storage_tex_2d(4, wgpu::TextureFormat::R32Float), // shadow output
            ],
        });

        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DF Shadow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/df_shadows.wgsl").into()),
        });

        let shadow_pipeline = create_compute_pipeline(device, "DF Shadow", &shadow_shader, &shadow_layout);

        let shadow_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DF Shadow Params"),
            size: std::mem::size_of::<DFShadowParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_output = create_r8_texture(device, width, height, "DF Shadow Output");
        let shadow_output_view = shadow_output.create_view(&wgpu::TextureViewDescriptor::default());

        // AO layout + pipeline
        let ao_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DF AO Layout"),
            entries: &[
                bgl_uniform(0),       // params
                bgl_depth_tex(1),     // depth
                bgl_float_tex(2),     // normals
                bgl_3d_tex(3),        // GDF volume
                bgl_sampler(4),       // GDF sampler
                bgl_storage_tex_2d(5, wgpu::TextureFormat::R32Float), // AO output
            ],
        });

        let ao_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DF AO Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/df_ao.wgsl").into()),
        });

        let ao_pipeline = create_compute_pipeline(device, "DF AO", &ao_shader, &ao_layout);

        let ao_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DF AO Params"),
            size: std::mem::size_of::<DFAOParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ao_output = create_r8_texture(device, width, height, "DF AO Output");
        let ao_output_view = ao_output.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            gdf_volume,
            gdf_view,
            gdf_sampler,
            voxelize_pipeline,
            voxelize_layout,
            voxelize_params_buffer,
            shadow_pipeline,
            shadow_layout,
            shadow_params_buffer,
            shadow_output,
            shadow_output_view,
            ao_pipeline,
            ao_layout,
            ao_params_buffer,
            ao_output,
            ao_output_view,
            config,
            screen_width: width,
            screen_height: height,
            volume_origin: [0.0; 3],
        }
    }

    /// Update volume origin (center on camera position)
    pub fn update_volume_origin(&mut self, camera_pos: [f32; 3]) {
        let half = self.config.extent * 0.5;
        self.volume_origin = [
            camera_pos[0] - half,
            camera_pos[1] - half,
            camera_pos[2] - half,
        ];
    }

    /// Voxelize the scene into the GDF using instance bounding spheres
    pub fn voxelize(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        gpu_scene_buffer: &wgpu::Buffer,
        instance_count: u32,
    ) {
        if instance_count == 0 {
            return;
        }

        let voxel_size = self.config.extent / self.config.resolution as f32;
        let params = VoxelizeParams {
            volume_origin: self.volume_origin,
            voxel_size,
            resolution: self.config.resolution,
            instance_count,
            _pad: [0; 2],
        };
        queue.write_buffer(&self.voxelize_params_buffer, 0, bytemuck::bytes_of(&params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SDF Voxelize Bind Group"),
            layout: &self.voxelize_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.voxelize_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu_scene_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.gdf_view),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SDF Voxelization"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.voxelize_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            // workgroup_size(4,4,4) → dispatches = resolution/4
            let dispatches = (self.config.resolution + 3) / 4;
            pass.dispatch_workgroups(dispatches, dispatches, dispatches);
        }
    }

    /// Trace soft shadows through the GDF
    pub fn trace_shadows(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &DFShadowParams,
        depth_view: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.shadow_params_buffer, 0, bytemuck::bytes_of(params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DF Shadow Bind Group"),
            layout: &self.shadow_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.shadow_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.gdf_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.gdf_sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.shadow_output_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DF Soft Shadows"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (self.screen_width + 7) / 8,
                (self.screen_height + 7) / 8,
                1,
            );
        }
    }

    /// Compute distance field ambient occlusion
    pub fn compute_ao(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &DFAOParams,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.ao_params_buffer, 0, bytemuck::bytes_of(params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DF AO Bind Group"),
            layout: &self.ao_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.ao_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(normal_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.gdf_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.gdf_sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.ao_output_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DF Ambient Occlusion"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.ao_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (self.screen_width + 7) / 8,
                (self.screen_height + 7) / 8,
                1,
            );
        }
    }

    /// Resize output textures
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.screen_width == width && self.screen_height == height {
            return;
        }
        self.screen_width = width;
        self.screen_height = height;

        self.shadow_output = create_r8_texture(device, width, height, "DF Shadow Output");
        self.shadow_output_view = self.shadow_output.create_view(&wgpu::TextureViewDescriptor::default());

        self.ao_output = create_r8_texture(device, width, height, "DF AO Output");
        self.ao_output_view = self.ao_output.create_view(&wgpu::TextureViewDescriptor::default());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_compute_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{} Pipeline Layout", label)),
        bind_group_layouts: &[layout],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(&format!("{} Pipeline", label)),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_r8_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    })
}

fn bgl_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_depth_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_float_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_3d_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn bgl_storage_read(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_storage_tex_3d(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D3,
        },
        count: None,
    }
}

fn bgl_storage_tex_2d(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}
