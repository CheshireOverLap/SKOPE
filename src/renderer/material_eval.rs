// SKOPE Engine - Material Evaluation System
// V-Buffer Material Evaluation via Compute Shader
//
// Bind Groups (4):
// Group 0: V-Buffer (triangle_id, barycentric, depth, sampler)
// Group 1: Geometry (vertices, indices, mesh_infos)
// Group 2: Materials + Lighting + Bindless Textures + Clustered + Shadows + Tier3 + IBL (bindings 0-25)
//   - 0-3: materials, sampler, lighting, bindless_textures (binding_array)
//   - 4-7: clustered lighting (cluster_params, light_grid, light_indices, lights) [Phase 14]
//   - 8-10: shadows (shadow_map, shadow_sampler, shadow_uniforms) [Phase 16]
//   - 11-13: DDGI (irradiance, visibility, params)
//   - 14-16: VSM (page_table, physical_pool, vsm_params) [Tier 3]
//   - 17-18: MegaLights (denoised_output, megalights_params) [Tier 3]
//   - 19-21: DBuffer Decals (albedo, normal, roughness)
//   - 22-25: IBL Environment (prefiltered, irradiance, BRDF LUT, sampler)
// Group 3: Output (HDR storage texture)

#![allow(dead_code)]

pub mod types;

pub use types::*;

use super::vbuffer::VBuffer;
use std::collections::HashMap;

/// Material buffer 최대 슬롯 수
pub const MAX_MATERIALS: usize = 512;

/// Mesh info buffer 최대 엔트리 수
pub const MAX_MESH_INFOS: usize = 256;

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

    // Tier 3: VSM dummy resources
    dummy_vsm_page_table: wgpu::Texture,
    dummy_vsm_page_table_view: wgpu::TextureView,
    dummy_vsm_physical_pool: wgpu::Texture,
    dummy_vsm_physical_pool_view: wgpu::TextureView,
    dummy_vsm_params: wgpu::Buffer,

    // Tier 3: MegaLights dummy resources
    dummy_megalights_output: wgpu::Texture,
    dummy_megalights_output_view: wgpu::TextureView,
    dummy_megalights_params: wgpu::Buffer,

    // Active resource tracking: each set_* method updates its category,
    // then rebuild_group2() uses whatever is currently active.
    // This prevents set_clustered/set_ddgi/set_csm from clobbering each other.
    active_cluster_params: wgpu::Buffer,
    active_light_grid: wgpu::Buffer,
    active_light_indices: wgpu::Buffer,
    active_lights: wgpu::Buffer,
    active_shadow_view: wgpu::TextureView,
    active_shadow_sampler: wgpu::Sampler,
    active_shadow_uniforms: wgpu::Buffer,
    active_ddgi_irradiance_view: wgpu::TextureView,
    active_ddgi_visibility_view: wgpu::TextureView,
    active_ddgi_params: wgpu::Buffer,
    // Tier 3 active resources
    active_vsm_page_table_view: wgpu::TextureView,
    active_vsm_physical_pool_view: wgpu::TextureView,
    active_vsm_params: wgpu::Buffer,
    active_megalights_output_view: wgpu::TextureView,
    active_megalights_params: wgpu::Buffer,

    // DBuffer decal dummy + active resources
    dummy_dbuffer_albedo: wgpu::Texture,
    dummy_dbuffer_albedo_view: wgpu::TextureView,
    dummy_dbuffer_normal: wgpu::Texture,
    dummy_dbuffer_normal_view: wgpu::TextureView,
    dummy_dbuffer_roughness: wgpu::Texture,
    dummy_dbuffer_roughness_view: wgpu::TextureView,
    active_dbuffer_albedo_view: wgpu::TextureView,
    active_dbuffer_normal_view: wgpu::TextureView,
    active_dbuffer_roughness_view: wgpu::TextureView,

    // Nanite geometry dummy buffers (for when Nanite is not active)
    dummy_nanite_vertices: wgpu::Buffer,
    dummy_nanite_triangles: wgpu::Buffer,
    dummy_nanite_meshlets: wgpu::Buffer,
    dummy_nanite_instances: wgpu::Buffer,
    dummy_nanite_visible_clusters: wgpu::Buffer,

    // IBL dummy + active resources (Group 2, bindings 22-25)
    dummy_ibl_cube: wgpu::Texture,
    dummy_ibl_cube_view: wgpu::TextureView,
    dummy_brdf_lut: wgpu::Texture,
    dummy_brdf_lut_view: wgpu::TextureView,
    dummy_ibl_sampler: wgpu::Sampler,
    active_ibl_prefiltered_view: wgpu::TextureView,
    active_ibl_irradiance_view: wgpu::TextureView,
    active_ibl_brdf_lut_view: wgpu::TextureView,
    active_ibl_sampler: wgpu::Sampler,

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
    // Albedo G-Buffer for GI composite (rgba8unorm: albedo.rgb, metallic)
    pub albedo_texture: wgpu::Texture,
    pub albedo_view: wgpu::TextureView,
    // Shading model mask for SSS (r8unorm: 1.0 = Skin)
    pub shading_model_mask_texture: wgpu::Texture,
    pub shading_model_mask_view: wgpu::TextureView,
    pub output_bind_group: wgpu::BindGroup,

    // Size
    pub width: u32,
    pub height: u32,

    // Bindless texture allocation cursor (incremental registration)
    pub bindless_next_slot: u32,

    // Deferred Group 2 rebuild flag
    dirty_group2: bool,
}

impl MaterialEvalPipeline {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Self {
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
                // binding 2: depth (R32Float from merged resolve, or Depth32Float from standard vbuffer)
                // Changed from Depth to Float for merged resolve compatibility
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // Group 1: Geometry (Standard + Nanite)
        let geometry_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Geometry Layout"),
            entries: &[
                // binding 0: standard vertices
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
                // binding 1: standard indices
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
                // binding 2: mesh_infos
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
                // binding 3: nanite_vertices (NaniteFullVertex array)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: nanite_triangles (u32 array)
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
                // binding 5: nanite_meshlets (Meshlet array)
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
                // binding 6: nanite_instances (NaniteInstance array)
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
                // binding 7: visible_clusters (VisibleCluster array)
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
                // ===== Tier 3: Virtual Shadow Maps (bindings 14-16) =====
                // binding 14: vsm_page_table (R32Uint texture)
                wgpu::BindGroupLayoutEntry {
                    binding: 14,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 15: vsm_physical_pool (depth texture)
                wgpu::BindGroupLayoutEntry {
                    binding: 15,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 16: vsm_params (storage buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 16,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // ===== Tier 3: MegaLights (bindings 17-18) =====
                // binding 17: megalights_output (denoised lighting texture)
                wgpu::BindGroupLayoutEntry {
                    binding: 17,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 18: megalights_params (storage buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 18,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // ===== DBuffer Decals (bindings 19-21) =====
                // binding 19: dbuffer_albedo (Rgba8Unorm)
                wgpu::BindGroupLayoutEntry {
                    binding: 19,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 20: dbuffer_normal (Rgba8Snorm)
                wgpu::BindGroupLayoutEntry {
                    binding: 20,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 21: dbuffer_roughness (Rgba8Unorm)
                wgpu::BindGroupLayoutEntry {
                    binding: 21,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // ===== IBL Environment (bindings 22-25) =====
                // binding 22: prefiltered specular cubemap
                wgpu::BindGroupLayoutEntry {
                    binding: 22,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 23: irradiance cubemap
                wgpu::BindGroupLayoutEntry {
                    binding: 23,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 24: BRDF LUT
                wgpu::BindGroupLayoutEntry {
                    binding: 24,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 25: IBL sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 25,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                // binding 2: Albedo G-Buffer for GI composite (albedo.rgb, metallic)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 3: Shading model mask for SSS (1.0 = Skin)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
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
            size: std::mem::size_of::<GpuMaterial>() as u64 * MAX_MATERIALS as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Phase 14: Dummy clustered lighting buffers (will be replaced when clustered lighting is updated)
        let dummy_cluster_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Cluster Params"),
            size: 32, // grid_size[3] + tile_size + screen[2] + near + far = 32 bytes
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

        // Tier 3: VSM dummy resources
        let dummy_vsm_page_table = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy VSM Page Table"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_vsm_page_table_view = dummy_vsm_page_table.create_view(&Default::default());

        let dummy_vsm_physical_pool = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy VSM Physical Pool"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let dummy_vsm_physical_pool_view = dummy_vsm_physical_pool.create_view(&Default::default());

        // VsmParams: 96 bytes (Mat4 + 8 u32s)
        let dummy_vsm_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy VSM Params"),
            size: 96,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Tier 3: MegaLights dummy resources
        let dummy_megalights_output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy MegaLights Output"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_megalights_output_view = dummy_megalights_output.create_view(&Default::default());

        // MegaLightsParams: 112 bytes (mat4x4 + 10 u32/f32 + 2 pad)
        let dummy_megalights_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy MegaLights Params"),
            size: 112,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // DBuffer decal dummy textures (1x1 transparent)
        let dummy_dbuffer_albedo = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy DBuffer Albedo"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_dbuffer_albedo_view = dummy_dbuffer_albedo.create_view(&Default::default());

        let dummy_dbuffer_normal = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy DBuffer Normal"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Snorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_dbuffer_normal_view = dummy_dbuffer_normal.create_view(&Default::default());

        let dummy_dbuffer_roughness = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy DBuffer Roughness"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let dummy_dbuffer_roughness_view = dummy_dbuffer_roughness.create_view(&Default::default());

        // Active DBuffer views (initially pointing to dummies)
        let active_dbuffer_albedo_view = dummy_dbuffer_albedo_view.clone();
        let active_dbuffer_normal_view = dummy_dbuffer_normal_view.clone();
        let active_dbuffer_roughness_view = dummy_dbuffer_roughness_view.clone();

        // Nanite geometry dummy buffers
        let dummy_nanite_vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Nanite Vertices"),
            size: 80, // One NaniteFullVertex (80 bytes: pos+pad+normal+pad+tangent+uv+uv1+color)
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let dummy_nanite_triangles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Nanite Triangles"),
            size: 4, // One u32
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let dummy_nanite_meshlets = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Nanite Meshlets"),
            size: 64, // One Meshlet (64 bytes)
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let dummy_nanite_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Nanite Instances"),
            size: 144, // One NaniteInstance (128+16 bytes)
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let dummy_nanite_visible_clusters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Nanite Visible Clusters"),
            size: 16, // One VisibleCluster (4 x u32 = 16 bytes)
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Material sampler
        // Important: address_mode set to Repeat for UV > 1.0 texture wrapping
        // Default ClampToEdge would clamp UV to 1.0 causing texture stretching
        let material_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MaterialEval Material Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
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

        // Dummy IBL textures (1x1 cube + 1x1 BRDF LUT) for Group 2 bindings 22-25
        // IMPORTANT: Add COPY_DST to zero-fill (uninitialized textures may contain NaN)
        let dummy_ibl_cube = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy IBL Cube"),
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        });
        // Zero-fill all 6 faces (1x1 Rgba16Float = 8 bytes per face)
        for face in 0..6u32 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &dummy_ibl_cube,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: face },
                    aspect: wgpu::TextureAspect::All,
                },
                &[0u8; 8], // 4 x f16 = 8 bytes, all zero
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(8),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
        }
        let dummy_ibl_cube_view = dummy_ibl_cube.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let dummy_brdf_lut = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy BRDF LUT"),
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        });
        // Zero-fill BRDF LUT (1x1 Rg16Float = 4 bytes)
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &dummy_brdf_lut,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[0u8; 4], // 2 x f16 = 4 bytes, all zero
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let dummy_brdf_lut_view = dummy_brdf_lut.create_view(&Default::default());
        let dummy_ibl_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("IBL Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

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
                // Tier 3: VSM (bindings 14-16)
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&dummy_vsm_page_table_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: wgpu::BindingResource::TextureView(&dummy_vsm_physical_pool_view),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: dummy_vsm_params.as_entire_binding(),
                },
                // Tier 3: MegaLights (bindings 17-18)
                wgpu::BindGroupEntry {
                    binding: 17,
                    resource: wgpu::BindingResource::TextureView(&dummy_megalights_output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: dummy_megalights_params.as_entire_binding(),
                },
                // DBuffer decals (bindings 19-21)
                wgpu::BindGroupEntry {
                    binding: 19,
                    resource: wgpu::BindingResource::TextureView(&dummy_dbuffer_albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 20,
                    resource: wgpu::BindingResource::TextureView(&dummy_dbuffer_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 21,
                    resource: wgpu::BindingResource::TextureView(&dummy_dbuffer_roughness_view),
                },
                // IBL (bindings 22-25)
                wgpu::BindGroupEntry {
                    binding: 22,
                    resource: wgpu::BindingResource::TextureView(&dummy_ibl_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 23,
                    resource: wgpu::BindingResource::TextureView(&dummy_ibl_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 24,
                    resource: wgpu::BindingResource::TextureView(&dummy_brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 25,
                    resource: wgpu::BindingResource::Sampler(&dummy_ibl_sampler),
                },
            ],
        });

        // Output textures
        let (output_texture, output_view) = Self::create_output_texture(device, width, height, "MaterialEval HDR Output");
        let (normal_roughness_texture, normal_roughness_view) = Self::create_output_texture(device, width, height, "MaterialEval Normal/Roughness G-Buffer");
        let (albedo_texture, albedo_view) = Self::create_albedo_texture(device, width, height);
        let (shading_model_mask_texture, shading_model_mask_view) = Self::create_r32f_texture(device, width, height, "MaterialEval Shading Model Mask");

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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shading_model_mask_view),
                },
            ],
        });

        // IBL active resources (initialized as clones of dummies)
        let active_ibl_prefiltered_view = dummy_ibl_cube_view.clone();
        let active_ibl_irradiance_view = dummy_ibl_cube_view.clone();
        let active_ibl_brdf_lut_view = dummy_brdf_lut_view.clone();
        let active_ibl_sampler = dummy_ibl_sampler.clone();

        // Shader (preprocessed by build script with #include)
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Material Evaluation Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!(concat!(env!("OUT_DIR"), "/shaders/material_eval.wgsl")).into()),
        });

        // Pipeline layout (4 bind groups - all lighting merged into Group 2 including IBL)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MaterialEval Pipeline Layout"),
            bind_group_layouts: &[
                &vbuffer_layout,
                &geometry_layout,
                &material_lighting_layout, // Includes clustered, shadows, DDGI, VSM, MegaLights, DBuffer, IBL
                &output_layout,
            ],
            immediate_size: 0,
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

        // Initialize active resources as clones of dummies
        let active_cluster_params = dummy_cluster_params.clone();
        let active_light_grid = dummy_light_grid.clone();
        let active_light_indices = dummy_light_indices.clone();
        let active_lights = dummy_lights.clone();
        let active_shadow_view = dummy_shadow_view.clone();
        let active_shadow_sampler = dummy_shadow_sampler.clone();
        let active_shadow_uniforms = dummy_shadow_uniforms.clone();
        let active_ddgi_irradiance_view = dummy_ddgi_irradiance_view.clone();
        let active_ddgi_visibility_view = dummy_ddgi_visibility_view.clone();
        let active_ddgi_params = dummy_ddgi_params.clone();
        let active_vsm_page_table_view = dummy_vsm_page_table_view.clone();
        let active_vsm_physical_pool_view = dummy_vsm_physical_pool_view.clone();
        let active_vsm_params = dummy_vsm_params.clone();
        let active_megalights_output_view = dummy_megalights_output_view.clone();
        let active_megalights_params = dummy_megalights_params.clone();

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
            active_cluster_params,
            active_light_grid,
            active_light_indices,
            active_lights,
            active_shadow_view,
            active_shadow_sampler,
            active_shadow_uniforms,
            active_ddgi_irradiance_view,
            active_ddgi_visibility_view,
            active_ddgi_params,
            dummy_vsm_page_table,
            dummy_vsm_page_table_view,
            dummy_vsm_physical_pool,
            dummy_vsm_physical_pool_view,
            dummy_vsm_params,
            dummy_megalights_output,
            dummy_megalights_output_view,
            dummy_megalights_params,
            active_vsm_page_table_view,
            active_vsm_physical_pool_view,
            active_vsm_params,
            active_megalights_output_view,
            active_megalights_params,
            dummy_dbuffer_albedo,
            dummy_dbuffer_albedo_view,
            dummy_dbuffer_normal,
            dummy_dbuffer_normal_view,
            dummy_dbuffer_roughness,
            dummy_dbuffer_roughness_view,
            active_dbuffer_albedo_view,
            active_dbuffer_normal_view,
            active_dbuffer_roughness_view,
            dummy_nanite_vertices,
            dummy_nanite_triangles,
            dummy_nanite_meshlets,
            dummy_nanite_instances,
            dummy_nanite_visible_clusters,
            dummy_ibl_cube,
            dummy_ibl_cube_view,
            dummy_brdf_lut,
            dummy_brdf_lut_view,
            dummy_ibl_sampler,
            active_ibl_prefiltered_view,
            active_ibl_irradiance_view,
            active_ibl_brdf_lut_view,
            active_ibl_sampler,
            material_sampler,
            placeholder_texture,
            placeholder_view,
            bindless_texture_views,
            bindless_next_slot: 0,
            material_lighting_bind_group,
            output_texture,
            output_view,
            normal_roughness_texture,
            normal_roughness_view,
            albedo_texture,
            albedo_view,
            shading_model_mask_texture,
            shading_model_mask_view,
            output_bind_group,
            width,
            height,
            dirty_group2: false,
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

        let mut next_slot: u32 = self.bindless_next_slot;

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

        let registered_count = next_slot - self.bindless_next_slot;
        log::info!(
            "[MaterialEval] Registered {} bindless textures (albedo: {}, normal: {}, mr: {}) [slots {}..{}]",
            registered_count, albedo_count, normal_count, mr_count,
            self.bindless_next_slot, next_slot
        );

        // Save cursor for incremental registration
        self.bindless_next_slot = next_slot;

        // Rebuild bind group once after all registrations
        self.rebuild_bindless_bind_group(device);

        BindlessHandleMaps {
            albedo: albedo_map,
            normal: normal_map,
            metallic_roughness: mr_map,
        }
    }

    /// Unified Group 2 bind group rebuild using active resources.
    /// All set_* methods update their active_ fields, then call this.
    fn rebuild_group2(&mut self, device: &wgpu::Device) {
        let bindless_view_refs: Vec<&wgpu::TextureView> = self.bindless_texture_views.iter().collect();

        self.material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Group2 Bind Group (Unified)"),
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
                // Clustered lighting (active)
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.active_cluster_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.active_light_grid.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.active_light_indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.active_lights.as_entire_binding(),
                },
                // CSM shadows (active)
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.active_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.active_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: self.active_shadow_uniforms.as_entire_binding(),
                },
                // DDGI (active)
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&self.active_ddgi_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::TextureView(&self.active_ddgi_visibility_view),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: self.active_ddgi_params.as_entire_binding(),
                },
                // VSM (active)
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: wgpu::BindingResource::TextureView(&self.active_vsm_page_table_view),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: wgpu::BindingResource::TextureView(&self.active_vsm_physical_pool_view),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: self.active_vsm_params.as_entire_binding(),
                },
                // MegaLights (active)
                wgpu::BindGroupEntry {
                    binding: 17,
                    resource: wgpu::BindingResource::TextureView(&self.active_megalights_output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: self.active_megalights_params.as_entire_binding(),
                },
                // DBuffer decals (active)
                wgpu::BindGroupEntry {
                    binding: 19,
                    resource: wgpu::BindingResource::TextureView(&self.active_dbuffer_albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 20,
                    resource: wgpu::BindingResource::TextureView(&self.active_dbuffer_normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 21,
                    resource: wgpu::BindingResource::TextureView(&self.active_dbuffer_roughness_view),
                },
                // IBL (active)
                wgpu::BindGroupEntry {
                    binding: 22,
                    resource: wgpu::BindingResource::TextureView(&self.active_ibl_prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 23,
                    resource: wgpu::BindingResource::TextureView(&self.active_ibl_irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 24,
                    resource: wgpu::BindingResource::TextureView(&self.active_ibl_brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 25,
                    resource: wgpu::BindingResource::Sampler(&self.active_ibl_sampler),
                },
            ],
        });
    }

    /// Rebuild the material_lighting_bind_group after texture changes
    pub fn rebuild_bindless_bind_group(&mut self, device: &wgpu::Device) {
        self.rebuild_group2(device);
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
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_albedo_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MaterialEval Albedo G-Buffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_r32f_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
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
            format: wgpu::TextureFormat::R32Float,
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
        (self.albedo_texture, self.albedo_view) = Self::create_albedo_texture(device, width, height);
        (self.shading_model_mask_texture, self.shading_model_mask_view) = Self::create_r32f_texture(device, width, height, "MaterialEval Shading Model Mask");

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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.shading_model_mask_view),
                },
            ],
        });
    }

    pub fn update_lighting(&self, queue: &wgpu::Queue, lighting: &MaterialEvalLighting) {
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n < 10 {
            log::info!("[MaterialEval] update_lighting call #{}: debug_mode={}, size={}",
                n, lighting.debug_mode, std::mem::size_of::<MaterialEvalLighting>());
        }
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

    /// Create V-Buffer Bind Group (Group 0) from merged resolve output.
    /// The merged resolve always runs and produces R32Float depth.
    pub fn create_merged_vbuffer_bind_group(
        &self,
        device: &wgpu::Device,
        resolve: &super::vbuffer_resolve::VBufferResolvePipeline,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Merged VBuffer Bind Group"),
            layout: &self.vbuffer_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&resolve.merged_triangle_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&resolve.merged_barycentrics_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&resolve.merged_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Create Geometry Bind Group (Group 1) with Nanite buffers.
    /// Nanite buffers can be None (dummy buffers used as fallback).
    pub fn create_geometry_bind_group_with_nanite(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
        nanite_vertex_buffer: Option<&wgpu::Buffer>,
        nanite_triangle_buffer: Option<&wgpu::Buffer>,
        nanite_meshlet_buffer: Option<&wgpu::Buffer>,
        nanite_instance_buffer: Option<&wgpu::Buffer>,
        nanite_visible_clusters_buffer: Option<&wgpu::Buffer>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Geometry+Nanite Bind Group"),
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: nanite_vertex_buffer.unwrap_or(&self.dummy_nanite_vertices).as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: nanite_triangle_buffer.unwrap_or(&self.dummy_nanite_triangles).as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: nanite_meshlet_buffer.unwrap_or(&self.dummy_nanite_meshlets).as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: nanite_instance_buffer.unwrap_or(&self.dummy_nanite_instances).as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: nanite_visible_clusters_buffer.unwrap_or(&self.dummy_nanite_visible_clusters).as_entire_binding(),
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

    /// Update clustered lighting buffers (Phase 14)
    /// Only updates clustered lighting category; preserves shadow/DDGI active state.
    pub fn set_clustered_lighting_buffers(
        &mut self,
        device: &wgpu::Device,
        cluster_params: &wgpu::Buffer,
        light_grid: &wgpu::Buffer,
        light_indices: &wgpu::Buffer,
        lights: &wgpu::Buffer,
        _texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        self.active_cluster_params = cluster_params.clone();
        self.active_light_grid = light_grid.clone();
        self.active_light_indices = light_indices.clone();
        self.active_lights = lights.clone();
        self.rebuild_group2(device);
    }

    /// Update CSM shadow resources (Phase 16)
    /// Connects real shadow map texture, sampler, and uniforms buffer.
    pub fn set_csm_resources(
        &mut self,
        _device: &wgpu::Device,
        shadow_view: &wgpu::TextureView,
        shadow_uniforms: &wgpu::Buffer,
    ) {
        self.active_shadow_view = shadow_view.clone();
        self.active_shadow_uniforms = shadow_uniforms.clone();
        // Keep NonFiltering sampler (shader uses textureLoad + manual PCF)
        self.dirty_group2 = true;
    }

    /// Update VSM resources (Tier 3)
    /// Only updates VSM category; preserves other active state.
    pub fn set_vsm_resources(
        &mut self,
        _device: &wgpu::Device,
        page_table_view: &wgpu::TextureView,
        physical_pool_view: &wgpu::TextureView,
        vsm_params: &wgpu::Buffer,
    ) {
        self.active_vsm_page_table_view = page_table_view.clone();
        self.active_vsm_physical_pool_view = physical_pool_view.clone();
        self.active_vsm_params = vsm_params.clone();
        self.dirty_group2 = true;
    }

    /// Update MegaLights resources (Tier 3)
    /// Only updates MegaLights category; preserves other active state.
    pub fn set_megalights_resources(
        &mut self,
        _device: &wgpu::Device,
        output_view: &wgpu::TextureView,
        megalights_params: &wgpu::Buffer,
    ) {
        self.active_megalights_output_view = output_view.clone();
        self.active_megalights_params = megalights_params.clone();
        self.dirty_group2 = true;
    }

    /// Update DBuffer decal resources (from DBufferDecalPipeline)
    /// Only updates DBuffer category; preserves other active state.
    pub fn set_dbuffer_resources(
        &mut self,
        _device: &wgpu::Device,
        albedo_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        roughness_view: &wgpu::TextureView,
    ) {
        self.active_dbuffer_albedo_view = albedo_view.clone();
        self.active_dbuffer_normal_view = normal_view.clone();
        self.active_dbuffer_roughness_view = roughness_view.clone();
        self.dirty_group2 = true;
    }

    /// Update DDGI textures (Bindless version)
    /// Only updates DDGI category; preserves clustered/shadow active state.
    pub fn set_ddgi_textures(
        &mut self,
        _device: &wgpu::Device,
        irradiance_view: &wgpu::TextureView,
        visibility_view: &wgpu::TextureView,
        ddgi_params_buffer: &wgpu::Buffer,
        _texture_views: Option<(&wgpu::TextureView, &wgpu::TextureView, &wgpu::TextureView)>,
    ) {
        self.active_ddgi_irradiance_view = irradiance_view.clone();
        self.active_ddgi_visibility_view = visibility_view.clone();
        self.active_ddgi_params = ddgi_params_buffer.clone();
        self.dirty_group2 = true;
    }

    /// Set IBL resources (replaces dummy with real IBL textures, rebuilds Group 2)
    pub fn set_ibl_resources(
        &mut self,
        _device: &wgpu::Device,
        prefiltered_view: &wgpu::TextureView,
        irradiance_view: &wgpu::TextureView,
        brdf_lut_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) {
        self.active_ibl_prefiltered_view = prefiltered_view.clone();
        self.active_ibl_irradiance_view = irradiance_view.clone();
        self.active_ibl_brdf_lut_view = brdf_lut_view.clone();
        self.active_ibl_sampler = sampler.clone();
        self.dirty_group2 = true;
    }

    /// Flush deferred Group 2 bind group rebuild.
    /// Call once per frame before dispatch() to batch all set_* updates.
    pub fn flush_bind_groups(&mut self, device: &wgpu::Device) {
        if self.dirty_group2 {
            self.rebuild_group2(device);
            self.dirty_group2 = false;
        }
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
            immediate_size: 0,
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
