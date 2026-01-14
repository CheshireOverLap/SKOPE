// SKOPE Engine - Material Evaluation System
// V-Buffer Material Evaluation via Compute Shader
//
// Bind Groups (4 - wgpu limit):
// Group 0: V-Buffer (triangle_id, barycentric, depth, sampler)
// Group 1: Geometry (vertices, indices, mesh_infos)
// Group 2: Materials + Lighting + Textures + Clustered + Shadows (bindings 0-12)
//   - 0-5: materials, sampler, lighting, albedo/normal/MR texture arrays
//   - 6-9: clustered lighting (cluster_params, light_grid, light_indices, lights) [Phase 14]
//   - 10-12: shadows (shadow_map, shadow_sampler, shadow_uniforms) [Phase 16]
// Group 3: Output (HDR storage texture)

#![allow(dead_code)]

mod types;

pub use types::*;

use super::vbuffer::VBuffer;

/// Material Evaluation Pipeline (4 Bind Groups, Phase 14)
/// Group 2 now includes clustered lighting (bindings 6-9) due to wgpu 4 bind group limit
pub struct MaterialEvalPipeline {
    pub pipeline: wgpu::ComputePipeline,

    // Bind group layouts (4)
    pub vbuffer_layout: wgpu::BindGroupLayout,      // Group 0
    pub geometry_layout: wgpu::BindGroupLayout,     // Group 1
    pub material_lighting_layout: wgpu::BindGroupLayout, // Group 2: Materials + Lighting + Clustered
    pub output_layout: wgpu::BindGroupLayout,       // Group 3

    // Buffers
    pub lighting_buffer: wgpu::Buffer,
    pub mesh_info_buffer: wgpu::Buffer,
    pub material_buffer: wgpu::Buffer,

    // Phase 14: Dummy clustered lighting buffers
    dummy_cluster_params: wgpu::Buffer,
    dummy_light_grid: wgpu::Buffer,
    dummy_light_indices: wgpu::Buffer,
    dummy_lights: wgpu::Buffer,

    // Phase 16: Dummy shadow resources
    dummy_shadow_texture: wgpu::Texture,
    dummy_shadow_view: wgpu::TextureView,
    dummy_shadow_sampler: wgpu::Sampler,
    dummy_shadow_uniforms: wgpu::Buffer,

    // DDGI: Dummy resources (replaced when DDGI is enabled)
    dummy_ddgi_irradiance: wgpu::Texture,
    dummy_ddgi_irradiance_view: wgpu::TextureView,
    dummy_ddgi_visibility: wgpu::Texture,
    dummy_ddgi_visibility_view: wgpu::TextureView,
    dummy_ddgi_params: wgpu::Buffer,

    // Material sampler
    pub material_sampler: wgpu::Sampler,

    // Default fallback textures (1x1 white/normal/metallic)
    pub default_albedo: wgpu::Texture,
    pub default_albedo_view: wgpu::TextureView,
    pub default_normal: wgpu::Texture,
    pub default_normal_view: wgpu::TextureView,
    pub default_metallic_roughness: wgpu::Texture,
    pub default_metallic_roughness_view: wgpu::TextureView,

    // Bind group for materials + lighting (Group 2)
    pub material_lighting_bind_group: wgpu::BindGroup,

    // HDR output
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    pub output_bind_group: wgpu::BindGroup,

    // Size
    pub width: u32,
    pub height: u32,
}

impl MaterialEvalPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Group 0: V-Buffer
        let vbuffer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval VBuffer Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // Group 1: Geometry
        let geometry_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Geometry Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
        });

        // Group 2: Materials + Lighting + Textures + Clustered Lighting (Phase 14)
        let material_lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Material+Lighting+Textures+Clustered Layout"),
            entries: &[
                // binding 0: materials storage buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: material sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: lighting uniform
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
                // binding 3: albedo texture array (D2Array)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: normal texture array (D2Array)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 5: metallic_roughness texture array (D2Array)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // ===== Phase 14: Clustered Lighting (bindings 6-9) =====
                // binding 6: cluster_params (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 7: light_grid (storage, read-only)
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
                // binding 8: light_indices (storage, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 9: lights (storage, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // ===== Phase 16: Cascaded Shadow Maps (bindings 10-12) =====
                // binding 10: shadow_map (depth texture array)
                wgpu::BindGroupLayoutEntry {
                    binding: 10,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 11: shadow_sampler (placeholder, textureLoad doesn't need sampler)
                wgpu::BindGroupLayoutEntry {
                    binding: 11,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 12: shadow_uniforms (uniform buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 12,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // ===== DDGI Global Illumination (bindings 13-15) =====
                // binding 13: ddgi_irradiance_atlas (texture_2d)
                wgpu::BindGroupLayoutEntry {
                    binding: 13,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 14: ddgi_visibility_atlas (texture_2d)
                wgpu::BindGroupLayoutEntry {
                    binding: 14,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 15: ddgi_params (uniform buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 15,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Group 3: Output HDR
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Output Layout"),
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
            ],
        });

        // Create buffers
        let lighting_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval Lighting Buffer"),
            size: std::mem::size_of::<MaterialEvalLighting>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mesh_info_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval MeshInfo Buffer"),
            size: std::mem::size_of::<GpuMeshInfo>() as u64 * 256,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval Material Buffer"),
            size: std::mem::size_of::<GpuMaterial>() as u64 * 128,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Phase 14: Dummy clustered lighting buffers (will be replaced when clustered lighting is updated)
        let dummy_cluster_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Cluster Params"),
            size: 32, // ClusterReadParams size
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy_light_grid = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Light Grid"),
            size: 8, // At least one LightGrid
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy_light_indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Light Indices"),
            size: 4, // At least one u32
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy_lights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Lights"),
            size: 80, // At least one GpuLight (5 * vec4 = 80 bytes)
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Phase 16: Dummy shadow resources
        let dummy_shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy Shadow Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 4, // 4 cascades
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let dummy_shadow_view = dummy_shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Dummy Shadow View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let dummy_shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Dummy Shadow Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        // ShadowUniforms: 4 cascades * 80 bytes + 32 bytes header = 352 bytes
        let dummy_shadow_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Shadow Uniforms"),
            size: 352,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // DDGI: Dummy resources (will be replaced when DDGI is enabled)
        let dummy_ddgi_irradiance = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy DDGI Irradiance"),
            size: wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_ddgi_irradiance_view = dummy_ddgi_irradiance.create_view(&Default::default());

        let dummy_ddgi_visibility = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy DDGI Visibility"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // NOTE: Rg16Float doesn't support STORAGE_BINDING on all GPUs
            // Using Rgba16Float for broader compatibility (dummy texture only)
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_ddgi_visibility_view = dummy_ddgi_visibility.create_view(&Default::default());

        // DdgiProbeGridParams: 128 bytes (3 cascades * 32 + 16 + 16)
        let dummy_ddgi_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy DDGI Params"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Material sampler
        // Important: address_mode set to Repeat for UV > 1.0 texture wrapping
        // Default ClampToEdge would clamp UV to 1.0 causing texture stretching
        let material_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MaterialEval Material Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        // Default 1x1 fallback texture arrays (D2Array with 1 layer)
        let (default_albedo, default_albedo_view) = Self::create_default_texture(
            device, "Albedo", [255, 255, 255, 255], true // White, sRGB
        );
        let (default_normal, default_normal_view) = Self::create_default_texture(
            device, "Normal", [128, 128, 255, 255], false // Flat normal, Linear
        );
        let (default_metallic_roughness, default_metallic_roughness_view) = Self::create_default_texture(
            device, "MetallicRoughness", [0, 128, 0, 255], false // Non-metallic, Linear
        );

        // Material + Lighting + Clustered bind group (Group 2) - Phase 14
        let material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+Clustered Bind Group"),
            layout: &material_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&material_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&default_albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&default_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&default_metallic_roughness_view),
                },
                // Phase 14: Clustered lighting (dummy buffers)
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: dummy_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: dummy_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: dummy_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: dummy_lights.as_entire_binding(),
                },
                // Phase 16: Shadow maps (dummy resources)
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Sampler(&dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: dummy_shadow_uniforms.as_entire_binding(),
                },
                // DDGI Global Illumination (dummy resources)
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&dummy_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&dummy_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: dummy_ddgi_params.as_entire_binding(),
                },
            ],
        });

        // Output texture
        let (output_texture, output_view) = Self::create_output_texture(device, width, height);

        // Output bind group (Group 3)
        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Output Bind Group"),
            layout: &output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&output_view),
                },
            ],
        });

        // Shader (preprocessed by build script with #include)
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Material Evaluation Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shaders/material_eval.wgsl")).into()),
        });

        // Pipeline layout (4 bind groups - clustered lighting merged into Group 2)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MaterialEval Pipeline Layout"),
            bind_group_layouts: &[
                &vbuffer_layout,
                &geometry_layout,
                &material_lighting_layout, // Includes clustered lighting (bindings 6-9)
                &output_layout,
            ],
            push_constant_ranges: &[],
        });

        // Compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MaterialEval Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            pipeline,
            vbuffer_layout,
            geometry_layout,
            material_lighting_layout,
            output_layout,
            lighting_buffer,
            mesh_info_buffer,
            material_buffer,
            dummy_cluster_params,
            dummy_light_grid,
            dummy_light_indices,
            dummy_lights,
            dummy_shadow_texture,
            dummy_shadow_view,
            dummy_shadow_sampler,
            dummy_shadow_uniforms,
            dummy_ddgi_irradiance,
            dummy_ddgi_irradiance_view,
            dummy_ddgi_visibility,
            dummy_ddgi_visibility_view,
            dummy_ddgi_params,
            material_sampler,
            default_albedo,
            default_albedo_view,
            default_normal,
            default_normal_view,
            default_metallic_roughness,
            default_metallic_roughness_view,
            material_lighting_bind_group,
            output_texture,
            output_view,
            output_bind_group,
            width,
            height,
        }
    }

    /// Create a 1x1 default texture array (D2Array with 1 layer)
    fn create_default_texture(
        device: &wgpu::Device,
        name: &str,
        _color: [u8; 4],
        is_srgb: bool,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let format = if is_srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Default {} Texture Array", name)),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1, // 1-layer array
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create D2Array view
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("Default {} Array View", name)),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        (texture, view)
    }

    /// Initialize default textures and lighting buffer with default values
    pub fn init_default_textures(&self, queue: &wgpu::Queue) {
        // Initialize lighting buffer with default values (including debug_mode: 999)
        let default_lighting = MaterialEvalLighting::default();
        queue.write_buffer(&self.lighting_buffer, 0, bytemuck::cast_slice(&[default_lighting]));
        log::info!("[MaterialEval] Initialized lighting buffer with debug_mode: {}", default_lighting.debug_mode);

        // White albedo
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.default_albedo,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );

        // Flat normal (0.5, 0.5, 1.0 in linear = 128, 128, 255 in sRGB-ish)
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.default_normal,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[128u8, 128, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );

        // Metallic=0, Roughness=0.5 (glTF: R=occlusion, G=roughness, B=metallic)
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.default_metallic_roughness,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 128, 0, 255], // AO=1, Roughness=0.5, Metallic=0
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
    }

    /// Update bind group with texture arrays (D2Array views from TextureArrayManager)
    /// All views must be D2Array type
    pub fn set_texture_arrays(
        &mut self,
        device: &wgpu::Device,
        albedo_array_view: &wgpu::TextureView,
        normal_array_view: &wgpu::TextureView,
        metallic_roughness_array_view: &wgpu::TextureView,
    ) {
        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+TextureArrays Bind Group"),
            layout: &self.material_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.material_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(albedo_array_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(normal_array_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(metallic_roughness_array_view),
                },
                // Phase 14: Clustered Lighting bindings (6-9)
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.dummy_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.dummy_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.dummy_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.dummy_lights.as_entire_binding(),
                },
                // Phase 16: Shadow maps (10-12)
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Sampler(&self.dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: self.dummy_shadow_uniforms.as_entire_binding(),
                },
                // DDGI Global Illumination (13-15)
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: self.dummy_ddgi_params.as_entire_binding(),
                },
            ],
        });
    }

    fn create_output_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MaterialEval HDR Output"),
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;

        (self.output_texture, self.output_view) = Self::create_output_texture(device, width, height);

        self.output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Output Bind Group"),
            layout: &self.output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
            ],
        });
    }

    pub fn update_lighting(&self, queue: &wgpu::Queue, lighting: &MaterialEvalLighting) {
        queue.write_buffer(&self.lighting_buffer, 0, bytemuck::cast_slice(&[*lighting]));
    }

    pub fn update_mesh_infos(&self, queue: &wgpu::Queue, mesh_infos: &[GpuMeshInfo]) {
        if !mesh_infos.is_empty() {
            queue.write_buffer(&self.mesh_info_buffer, 0, bytemuck::cast_slice(mesh_infos));
        }
    }

    pub fn update_materials(&self, queue: &wgpu::Queue, materials: &[GpuMaterial]) {
        if !materials.is_empty() {
            queue.write_buffer(&self.material_buffer, 0, bytemuck::cast_slice(materials));
        }
    }

    /// Create V-Buffer Bind Group (Group 0)
    pub fn create_vbuffer_bind_group(&self, device: &wgpu::Device, vbuffer: &VBuffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval VBuffer Bind Group"),
            layout: &self.vbuffer_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&vbuffer.triangle_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&vbuffer.barycentric_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&vbuffer.depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&vbuffer.sampler),
                },
            ],
        })
    }

    /// Create Geometry Bind Group (Group 1)
    pub fn create_geometry_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Geometry Bind Group"),
            layout: &self.geometry_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.mesh_info_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Dispatch compute shader
    /// Phase 14: Clustered lighting is now merged into Group 2
    pub fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        vbuffer_bind_group: &wgpu::BindGroup,
        geometry_bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("MaterialEval Compute Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, vbuffer_bind_group, &[]);
        pass.set_bind_group(1, geometry_bind_group, &[]);
        pass.set_bind_group(2, &self.material_lighting_bind_group, &[]);
        pass.set_bind_group(3, &self.output_bind_group, &[]);

        let dispatch_x = self.width.div_ceil(8);
        let dispatch_y = self.height.div_ceil(8);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }

    /// Update bind group with clustered lighting buffers (Phase 14)
    /// texture_views: Option<(albedo, normal, mr)> - None uses default
    pub fn set_clustered_lighting_buffers(
        &mut self,
        device: &wgpu::Device,
        cluster_params: &wgpu::Buffer,
        light_grid: &wgpu::Buffer,
        light_indices: &wgpu::Buffer,
        lights: &wgpu::Buffer,
        texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        let (albedo_view, normal_view, mr_view) = texture_views.unwrap_or((
            &self.default_albedo_view,
            &self.default_normal_view,
            &self.default_metallic_roughness_view,
        ));

        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+Clustered Bind Group (Updated)"),
            layout: &self.material_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.material_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(mr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: lights.as_entire_binding(),
                },
                // Phase 16: Shadow maps (10-12)
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Sampler(&self.dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: self.dummy_shadow_uniforms.as_entire_binding(),
                },
                // DDGI Global Illumination (13-15)
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: self.dummy_ddgi_params.as_entire_binding(),
                },
            ],
        });
    }

    /// Update bind group with DDGI textures
    /// Called when DDGI is enabled and textures are ready
    pub fn set_ddgi_textures(
        &mut self,
        device: &wgpu::Device,
        irradiance_view: &wgpu::TextureView,
        visibility_view: &wgpu::TextureView,
        ddgi_params_buffer: &wgpu::Buffer,
        texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        let (albedo_view, normal_view, mr_view) = texture_views.unwrap_or((
            &self.default_albedo_view,
            &self.default_normal_view,
            &self.default_metallic_roughness_view,
        ));

        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+DDGI Bind Group"),
            layout: &self.material_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.material_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(mr_view),
                },
                // Clustered lighting (dummy for now)
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.dummy_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.dummy_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.dummy_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.dummy_lights.as_entire_binding(),
                },
                // Shadow maps (dummy)
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::Sampler(&self.dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: self.dummy_shadow_uniforms.as_entire_binding(),
                },
                // DDGI (real textures)
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: ddgi_params_buffer.as_entire_binding(),
                },
            ],
        });
    }

    /// Shader hot reload pipeline rebuild
    #[cfg(debug_assertions)]
    pub fn rebuild_pipeline(&mut self, device: &wgpu::Device, shader_source: &str) -> Result<(), String> {
        // Create new shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Material Evaluation Shader (Hot Reload)"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Rebuild pipeline layout (use existing bind group layouts)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MaterialEval Pipeline Layout (Hot Reload)"),
            bind_group_layouts: &[
                &self.vbuffer_layout,
                &self.geometry_layout,
                &self.material_lighting_layout,
                &self.output_layout,
            ],
            push_constant_ranges: &[],
        });

        // Create new compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MaterialEval Pipeline (Hot Reload)"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Replace existing pipeline
        self.pipeline = pipeline;

        log::info!("[MaterialEval] Pipeline rebuilt successfully");
        Ok(())
    }
}
