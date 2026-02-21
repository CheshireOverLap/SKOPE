// SKOPE Engine - Volumetric Fog/Lighting
//
// Froxel-based volumetric effects for realistic light scattering.
// Uses a 3D texture to store and accumulate atmospheric lighting.
//
// Pipeline:
// 1. Inject: Sample lights into froxel volume
// 2. Scatter: Ray march and accumulate in-scattering
// 3. Apply: Composite volumetrics with scene

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4, Mat4};

/// Volumetric Parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VolumetricParams {
    /// View matrix
    pub view: [[f32; 4]; 4],
    /// Inverse view matrix
    pub inv_view: [[f32; 4]; 4],
    /// Projection matrix
    pub proj: [[f32; 4]; 4],
    /// Inverse projection matrix
    pub inv_proj: [[f32; 4]; 4],
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Near plane distance
    pub near_plane: f32,
    /// Far plane distance
    pub far_plane: f32,
    /// Fog density (base scattering coefficient)
    pub fog_density: f32,
    /// Fog height falloff
    pub fog_height_falloff: f32,
    /// Fog base height (world Y)
    pub fog_base_height: f32,
    /// Scattering anisotropy (Henyey-Greenstein g parameter, -1 to 1)
    pub anisotropy: f32,
    /// Light intensity multiplier
    pub light_intensity: f32,
    /// Ambient light contribution
    pub ambient_intensity: f32,
    /// Temporal blend factor
    pub temporal_blend: f32,
    /// Frame index for jittering
    pub frame_index: u32,
    /// Primary light direction (sun)
    pub sun_direction: [f32; 3],
    pub _pad1: f32,
    /// Primary light color
    pub sun_color: [f32; 3],
    pub _pad2: f32,
    /// Ambient sky color
    pub ambient_color: [f32; 3],
    pub _pad3: f32,
}

impl Default for VolumetricParams {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view: Mat4::IDENTITY.to_cols_array_2d(),
            proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_proj: Mat4::IDENTITY.to_cols_array_2d(),
            screen_size: [1920.0, 1080.0],
            near_plane: 0.1,
            far_plane: 500.0,
            fog_density: 0.02,
            fog_height_falloff: 0.2,
            fog_base_height: 0.0,
            anisotropy: 0.5,
            light_intensity: 1.0,
            ambient_intensity: 0.1,
            temporal_blend: 0.9,
            frame_index: 0,
            sun_direction: [0.5, -0.7, 0.5],
            _pad1: 0.0,
            sun_color: [1.0, 0.95, 0.8],
            _pad2: 0.0,
            ambient_color: [0.3, 0.4, 0.5],
            _pad3: 0.0,
        }
    }
}

/// Froxel grid dimensions
pub const FROXEL_WIDTH: u32 = 160;   // Screen width / 8
pub const FROXEL_HEIGHT: u32 = 90;   // Screen height / 8
pub const FROXEL_DEPTH: u32 = 128;   // Depth slices (exponential distribution)

/// Volumetric Fog Pipeline
#[allow(dead_code)]
pub struct VolumetricPipeline {
    /// Inject lighting pipeline
    pub inject_pipeline: wgpu::ComputePipeline,
    /// Scatter/accumulate pipeline
    pub scatter_pipeline: wgpu::ComputePipeline,
    /// Apply to scene pipeline
    pub apply_pipeline: wgpu::ComputePipeline,

    /// Bind group layouts
    pub inject_layout: wgpu::BindGroupLayout,
    pub scatter_layout: wgpu::BindGroupLayout,
    pub apply_layout: wgpu::BindGroupLayout,

    /// Parameters buffer
    pub params_buffer: wgpu::Buffer,

    /// Froxel volume (RGBA16Float - RGB = inscatter, A = transmittance)
    pub froxel_texture: wgpu::Texture,
    pub froxel_view: wgpu::TextureView,

    /// Scattered lighting volume (after accumulation)
    pub scatter_texture: wgpu::Texture,
    pub scatter_view: wgpu::TextureView,

    /// History buffer for temporal filtering
    pub history_texture: wgpu::Texture,
    pub history_view: wgpu::TextureView,

    /// Final integrated result (2D - for applying)
    pub integrated_texture: wgpu::Texture,
    pub integrated_view: wgpu::TextureView,

    /// Samplers
    pub linear_sampler: wgpu::Sampler,
    pub point_sampler: wgpu::Sampler,

    /// Dimensions
    pub width: u32,
    pub height: u32,

    /// Frame counter
    pub frame_index: u32,
}

impl VolumetricPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Create bind group layouts
        let inject_layout = Self::create_inject_layout(device);
        let scatter_layout = Self::create_scatter_layout(device);
        let apply_layout = Self::create_apply_layout(device);

        // Parameters buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Volumetric Params"),
            size: std::mem::size_of::<VolumetricParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create 3D textures for froxel volumes
        let froxel_texture = Self::create_froxel_texture(device, "Froxel Volume");
        let froxel_view = froxel_texture.create_view(&Default::default());

        let scatter_texture = Self::create_froxel_texture(device, "Scatter Volume");
        let scatter_view = scatter_texture.create_view(&Default::default());

        let history_texture = Self::create_froxel_texture(device, "History Volume");
        let history_view = history_texture.create_view(&Default::default());

        // 2D integrated result
        let integrated_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volumetric Integrated"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let integrated_view = integrated_texture.create_view(&Default::default());

        // Samplers
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Volumetric Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Volumetric Point Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create pipelines
        let inject_pipeline = Self::create_inject_pipeline(device, &inject_layout);
        let scatter_pipeline = Self::create_scatter_pipeline(device, &scatter_layout);
        let apply_pipeline = Self::create_apply_pipeline(device, &apply_layout);

        log::info!("[Volumetric] Initialized {}x{}, froxel {}x{}x{}",
            width, height, FROXEL_WIDTH, FROXEL_HEIGHT, FROXEL_DEPTH);

        Self {
            inject_pipeline,
            scatter_pipeline,
            apply_pipeline,
            inject_layout,
            scatter_layout,
            apply_layout,
            params_buffer,
            froxel_texture,
            froxel_view,
            scatter_texture,
            scatter_view,
            history_texture,
            history_view,
            integrated_texture,
            integrated_view,
            linear_sampler,
            point_sampler,
            width,
            height,
            frame_index: 0,
        }
    }

    fn create_froxel_texture(device: &wgpu::Device, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: FROXEL_WIDTH,
                height: FROXEL_HEIGHT,
                depth_or_array_layers: FROXEL_DEPTH,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_inject_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Volumetric Inject Layout"),
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
                // binding 1: cascaded shadow map array
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: depth buffer
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
                // binding 3: linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 4: output froxel volume
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
                // binding 5: shadow uniforms (cascade matrices, bias, etc.)
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
                // binding 6: local light buffer (point/spot lights)
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
                // binding 7: light count buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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

    fn create_scatter_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Volumetric Scatter Layout"),
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
                // binding 1: input froxel volume
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: history volume
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 4: output scatter volume
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_apply_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Volumetric Apply Layout"),
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
                // binding 1: scatter volume
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: depth buffer
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
                // binding 3: scene color (to composite)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 5: output
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_inject_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Volumetric Inject Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/volumetric_inject.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Volumetric Inject Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Volumetric Inject Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_scatter_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Volumetric Scatter Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/volumetric_scatter.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Volumetric Scatter Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Volumetric Scatter Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    fn create_apply_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Volumetric Apply Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/volumetric_apply.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Volumetric Apply Pipeline Layout"),
            bind_group_layouts: &[layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Volumetric Apply Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Render volumetric fog
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        shadow_map_view: &wgpu::TextureView,
        shadow_uniforms_buffer: &wgpu::Buffer,
        light_buffer: &wgpu::Buffer,
        light_count_buffer: &wgpu::Buffer,
        scene_color_view: &wgpu::TextureView,
        view: Mat4,
        proj: Mat4,
        sun_direction: Vec3,
        sun_color: Vec3,
    ) {
        // Update params
        let params = VolumetricParams {
            view: view.to_cols_array_2d(),
            inv_view: view.inverse().to_cols_array_2d(),
            proj: proj.to_cols_array_2d(),
            inv_proj: proj.inverse().to_cols_array_2d(),
            screen_size: [self.width as f32, self.height as f32],
            frame_index: self.frame_index,
            sun_direction: sun_direction.normalize().to_array(),
            sun_color: sun_color.to_array(),
            ..Default::default()
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Pass 1: Inject lighting into froxels
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Volumetric Inject Bind Group"),
                layout: &self.inject_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(shadow_map_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&self.froxel_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: shadow_uniforms_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: light_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: light_count_buffer.as_entire_binding(),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Volumetric Inject Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.inject_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                FROXEL_WIDTH.div_ceil(8),
                FROXEL_HEIGHT.div_ceil(8),
                FROXEL_DEPTH,
            );
        }

        // Pass 2: Accumulate scattering with temporal blending
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Volumetric Scatter Bind Group"),
                layout: &self.scatter_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.froxel_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.history_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&self.scatter_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Volumetric Scatter Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.scatter_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                FROXEL_WIDTH.div_ceil(8),
                FROXEL_HEIGHT.div_ceil(8),
                1,  // Process all depth slices per thread
            );
        }

        // Copy per-slice inject data to history for next frame's temporal blending.
        // IMPORTANT: We copy froxel (per-slice) data, NOT scatter (cumulative) data,
        // because temporal blending must operate on per-slice quantities.
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.froxel_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.history_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: FROXEL_WIDTH,
                height: FROXEL_HEIGHT,
                depth_or_array_layers: FROXEL_DEPTH,
            },
        );

        // Pass 3: Apply volumetrics to scene
        {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Volumetric Apply Bind Group"),
                layout: &self.apply_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.scatter_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(scene_color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&self.integrated_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Volumetric Apply Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.apply_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                self.width.div_ceil(8),
                self.height.div_ceil(8),
                1,
            );
        }

        self.frame_index += 1;
    }

    /// Resize buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;

        self.integrated_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volumetric Integrated"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.integrated_view = self.integrated_texture.create_view(&Default::default());
    }
}
