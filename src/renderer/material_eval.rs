// SKOPE Engine - Material Evaluation System
// V-Buffer Material Evaluation via Compute Shader
//
// Bind Groups (4 - wgpu limit):
// Group 0: V-Buffer (triangle_id, barycentric, depth, sampler)
// Group 1: Geometry (vertices, indices, mesh_infos)
// Group 2: Materials + Lighting + Bindless Textures + Clustered + Shadows (bindings 0-13)
//   - 0-3: materials, sampler, lighting, bindless_textures (binding_array)
//   - 4-7: clustered lighting (cluster_params, light_grid, light_indices, lights) [Phase 14]
//   - 8-10: shadows (shadow_map, shadow_sampler, shadow_uniforms) [Phase 16]
//   - 11-13: DDGI (irradiance, visibility, params)
// Group 3: Output (HDR storage texture)

#![allow(dead_code)]

pub mod types;

pub use types::*;

use super::vbuffer::VBuffer;
use std::collections::HashMap;

/// Mapping from texture array layer indices to bindless heap slots
#[derive(Debug, Clone, Default)]
pub struct BindlessHandleMaps {
    /// Albedo layer index -> bindless slot
    pub albedo: HashMap<u32, u32>,
    /// Normal layer index -> bindless slot
    pub normal: HashMap<u32, u32>,
    /// MetallicRoughness layer index -> bindless slot
    pub metallic_roughness: HashMap<u32, u32>,
}

/// Material Evaluation Pipeline (4 Bind Groups, Phase 14 + Bindless)
/// Group 2 now includes bindless textures (binding 3) + clustered lighting (bindings 4-7)
pub struct MaterialEvalPipeline {
    pub pipeline: wgpu::ComputePipeline,

    // Bind group layouts (4)
    pub vbuffer_layout: wgpu::BindGroupLayout,      // Group 0
    pub geometry_layout: wgpu::BindGroupLayout,     // Group 1
    pub material_lighting_layout: wgpu::BindGroupLayout, // Group 2: Materials + Lighting + Bindless + Clustered
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

    // Bindless texture system
    /// Placeholder texture (1x1 magenta) for empty bindless slots
    pub placeholder_texture: wgpu::Texture,
    pub placeholder_view: wgpu::TextureView,
    /// All bindless texture views (indexed by handle)
    /// Slot 0 is always placeholder
    pub bindless_texture_views: Vec<wgpu::TextureView>,

    // Bind group for materials + lighting (Group 2)
    pub material_lighting_bind_group: wgpu::BindGroup,

    // HDR output
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    // Normal/Roughness G-Buffer for SSR (rgba16float: normal.xyz, roughness)
    pub normal_roughness_texture: wgpu::Texture,
    pub normal_roughness_view: wgpu::TextureView,
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

        // Group 2: Materials + Lighting + Bindless Textures + Clustered Lighting (Phase 14)
        // Note: bindings 3-5 (3x D2Array) consolidated into binding 3 (1x binding_array)
        // All subsequent bindings shifted by -2
        let material_lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Material+Lighting+Bindless+Clustered Layout"),
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
                // binding 2: lighting storage (changed from uniform for binding_array compatibility)
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
                // binding 3: bindless textures (binding_array<texture_2d>)
                // Requires TEXTURE_BINDING_ARRAY + SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: Some(std::num::NonZeroU32::new(crate::texture::bindless::MAX_BINDLESS_TEXTURES).unwrap()),
                },
                // ===== Phase 14: Clustered Lighting (bindings 4-7, shifted -2) =====
                // binding 4: cluster_params (storage, changed from uniform for binding_array compatibility)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 5: light_grid (storage, read-only)
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
                // binding 6: light_indices (storage, read-only)
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
                // binding 7: lights (storage, read-only)
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
                // ===== Phase 16: Cascaded Shadow Maps (bindings 8-10, shifted -2) =====
                // binding 8: shadow_map (depth texture array)
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 9: shadow_sampler (placeholder, textureLoad doesn't need sampler)
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 10: shadow_uniforms (storage, changed from uniform for binding_array compatibility)
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
                // ===== DDGI Global Illumination (bindings 11-13, shifted -2) =====
                // binding 11: ddgi_irradiance_atlas (texture_2d)
                wgpu::BindGroupLayoutEntry {
                    binding: 11,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 12: ddgi_visibility_atlas (texture_2d)
                wgpu::BindGroupLayoutEntry {
                    binding: 12,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 13: ddgi_params (storage, changed from uniform for binding_array compatibility)
                wgpu::BindGroupLayoutEntry {
                    binding: 13,
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

        // Group 3: Output HDR + Normal/Roughness G-Buffer
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Output Layout"),
            entries: &[
                // binding 0: HDR color output
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
                // binding 1: Normal/Roughness G-Buffer for SSR (normal.xyz, roughness)
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
            ],
        });

        // Create buffers
        let lighting_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval Lighting Buffer"),
            size: std::mem::size_of::<MaterialEvalLighting>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, // Changed for binding_array compatibility
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, // Changed for binding_array compatibility
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, // Changed for binding_array compatibility
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, // Changed for binding_array compatibility
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

        // Bindless: Create placeholder texture (1x1 magenta for debugging)
        let placeholder_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bindless Placeholder Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let placeholder_view = placeholder_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Initialize bindless texture views array with placeholders
        // All MAX_BINDLESS_TEXTURES slots filled with placeholder initially
        let bindless_texture_views: Vec<wgpu::TextureView> = (0..crate::texture::bindless::MAX_BINDLESS_TEXTURES)
            .map(|_| placeholder_texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();

        // Collect references for bind group creation
        let bindless_view_refs: Vec<&wgpu::TextureView> = bindless_texture_views.iter().collect();

        // Material + Lighting + Bindless + Clustered bind group (Group 2) - Phase 14 + Bindless
        let material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+Bindless+Clustered Bind Group"),
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
                // binding 3: bindless textures (binding_array)
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureViewArray(&bindless_view_refs),
                },
                // Phase 14: Clustered lighting (bindings 4-7, shifted -2)
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: dummy_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: dummy_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: dummy_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: dummy_lights.as_entire_binding(),
                },
                // Phase 16: Shadow maps (bindings 8-10, shifted -2)
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: dummy_shadow_uniforms.as_entire_binding(),
                },
                // DDGI Global Illumination (bindings 11-13, shifted -2)
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&dummy_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&dummy_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: dummy_ddgi_params.as_entire_binding(),
                },
            ],
        });

        // Output textures
        let (output_texture, output_view) = Self::create_output_texture(device, width, height, "MaterialEval HDR Output");
        let (normal_roughness_texture, normal_roughness_view) = Self::create_output_texture(device, width, height, "MaterialEval Normal/Roughness G-Buffer");

        // Output bind group (Group 3)
        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Output Bind Group"),
            layout: &output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal_roughness_view),
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
            placeholder_texture,
            placeholder_view,
            bindless_texture_views,
            material_lighting_bind_group,
            output_texture,
            output_view,
            normal_roughness_texture,
            normal_roughness_view,
            output_bind_group,
            width,
            height,
        }
    }

    /// Initialize placeholder texture and lighting buffer with default values
    pub fn init_default_textures(&self, queue: &wgpu::Queue) {
        // Initialize lighting buffer with default values
        let default_lighting = MaterialEvalLighting::default();
        queue.write_buffer(&self.lighting_buffer, 0, bytemuck::cast_slice(&[default_lighting]));
        log::info!("[MaterialEval] Initialized lighting buffer with debug_mode: {}", default_lighting.debug_mode);

        // Initialize placeholder texture (magenta for debugging missing textures)
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.placeholder_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 0, 255, 255], // Magenta
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );

        log::info!("[MaterialEval] Initialized bindless placeholder texture");
    }

    /// Register a texture view at a specific bindless slot
    /// Returns the slot index (handle) for use in GpuMaterial
    pub fn register_bindless_texture(&mut self, device: &wgpu::Device, slot: u32, view: wgpu::TextureView) {
        if slot as usize >= self.bindless_texture_views.len() {
            log::error!("[MaterialEval] Bindless slot {} out of range", slot);
            return;
        }
        self.bindless_texture_views[slot as usize] = view;
        self.rebuild_bindless_bind_group(device);
    }

    /// Register texture views from TextureArrayManager (batch operation)
    /// Takes BindlessTextureViews and returns handle mappings for each type.
    /// Returns: (albedo_slot_map, normal_slot_map, mr_slot_map)
    /// Each map: layer_index -> bindless_slot
    pub fn register_texture_array_views(
        &mut self,
        device: &wgpu::Device,
        views: crate::renderer::texture_array::BindlessTextureViews,
    ) -> BindlessHandleMaps {
        let mut albedo_map: HashMap<u32, u32> = HashMap::new();
        let mut normal_map: HashMap<u32, u32> = HashMap::new();
        let mut mr_map: HashMap<u32, u32> = HashMap::new();

        let mut next_slot: u32 = 0;

        // Register albedo textures
        for (layer_idx, view) in views.albedo_views {
            if (next_slot as usize) < self.bindless_texture_views.len() {
                self.bindless_texture_views[next_slot as usize] = view;
                albedo_map.insert(layer_idx, next_slot);
                next_slot += 1;
            } else {
                log::error!("[MaterialEval] Bindless heap full at slot {}", next_slot);
                break;
            }
        }
        let albedo_count = albedo_map.len();

        // Register normal textures
        for (layer_idx, view) in views.normal_views {
            if (next_slot as usize) < self.bindless_texture_views.len() {
                self.bindless_texture_views[next_slot as usize] = view;
                normal_map.insert(layer_idx, next_slot);
                next_slot += 1;
            } else {
                log::error!("[MaterialEval] Bindless heap full at slot {}", next_slot);
                break;
            }
        }
        let normal_count = normal_map.len();

        // Register metallic-roughness textures
        for (layer_idx, view) in views.mr_views {
            if (next_slot as usize) < self.bindless_texture_views.len() {
                self.bindless_texture_views[next_slot as usize] = view;
                mr_map.insert(layer_idx, next_slot);
                next_slot += 1;
            } else {
                log::error!("[MaterialEval] Bindless heap full at slot {}", next_slot);
                break;
            }
        }
        let mr_count = mr_map.len();

        log::info!(
            "[MaterialEval] Registered {} bindless textures (albedo: {}, normal: {}, mr: {})",
            next_slot, albedo_count, normal_count, mr_count
        );

        // Rebuild bind group once after all registrations
        self.rebuild_bindless_bind_group(device);

        BindlessHandleMaps {
            albedo: albedo_map,
            normal: normal_map,
            metallic_roughness: mr_map,
        }
    }

    /// Rebuild the material_lighting_bind_group after texture changes
    pub fn rebuild_bindless_bind_group(&mut self, device: &wgpu::Device) {
        let bindless_view_refs: Vec<&wgpu::TextureView> = self.bindless_texture_views.iter().collect();

        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+Bindless Bind Group (Rebuilt)"),
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
                    resource: wgpu::BindingResource::TextureViewArray(&bindless_view_refs),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.dummy_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.dummy_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.dummy_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.dummy_lights.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: self.dummy_shadow_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: self.dummy_ddgi_params.as_entire_binding(),
                },
            ],
        });
    }

    /// [DEPRECATED] Legacy D2Array texture binding - replaced by bindless system
    /// This is a no-op stub for backward compatibility during migration.
    /// Use register_bindless_texture() instead.
    #[deprecated(note = "Use register_bindless_texture() for bindless textures")]
    pub fn set_texture_arrays(
        &mut self,
        _device: &wgpu::Device,
        _albedo_array_view: &wgpu::TextureView,
        _normal_array_view: &wgpu::TextureView,
        _metallic_roughness_array_view: &wgpu::TextureView,
    ) {
        log::warn!("[MaterialEval] set_texture_arrays() is deprecated. Bindless textures are now used.");
        // No-op: bindless system is already initialized with placeholders
    }

    fn create_output_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
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

        (self.output_texture, self.output_view) = Self::create_output_texture(device, width, height, "MaterialEval HDR Output");
        (self.normal_roughness_texture, self.normal_roughness_view) = Self::create_output_texture(device, width, height, "MaterialEval Normal/Roughness G-Buffer");

        self.output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Output Bind Group"),
            layout: &self.output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.normal_roughness_view),
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

    /// Update bind group with clustered lighting buffers (Phase 14 + Bindless)
    /// Uses bindless texture array (binding 3)
    pub fn set_clustered_lighting_buffers(
        &mut self,
        device: &wgpu::Device,
        cluster_params: &wgpu::Buffer,
        light_grid: &wgpu::Buffer,
        light_indices: &wgpu::Buffer,
        lights: &wgpu::Buffer,
        _texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        // Note: texture_views parameter is ignored - bindless textures are used instead
        let bindless_view_refs: Vec<&wgpu::TextureView> = self.bindless_texture_views.iter().collect();

        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+Bindless+Clustered Bind Group"),
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
                // binding 3: bindless textures
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureViewArray(&bindless_view_refs),
                },
                // bindings 4-7: clustered lighting (shifted -2)
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: lights.as_entire_binding(),
                },
                // bindings 8-10: shadow maps (shifted -2)
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: self.dummy_shadow_uniforms.as_entire_binding(),
                },
                // bindings 11-13: DDGI (shifted -2)
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: self.dummy_ddgi_params.as_entire_binding(),
                },
            ],
        });
    }

    /// Update bind group with DDGI textures (Bindless version)
    /// Called when DDGI is enabled and textures are ready
    pub fn set_ddgi_textures(
        &mut self,
        device: &wgpu::Device,
        irradiance_view: &wgpu::TextureView,
        visibility_view: &wgpu::TextureView,
        ddgi_params_buffer: &wgpu::Buffer,
        _texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        // Note: texture_views parameter is ignored - bindless textures are used instead
        let bindless_view_refs: Vec<&wgpu::TextureView> = self.bindless_texture_views.iter().collect();

        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting+Bindless+DDGI Bind Group"),
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
                // binding 3: bindless textures
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureViewArray(&bindless_view_refs),
                },
                // bindings 4-7: clustered lighting (dummy)
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.dummy_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.dummy_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.dummy_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.dummy_lights.as_entire_binding(),
                },
                // bindings 8-10: shadow maps (dummy)
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.dummy_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: self.dummy_shadow_uniforms.as_entire_binding(),
                },
                // bindings 11-13: DDGI (real textures)
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
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
