// SKOPE Engine — Deep Shadow Maps for Hair Strands
//
// Deep Shadow Maps (DSM) store opacity as a function of depth,
// allowing volumetric, soft shadows through semi-transparent hair.
// Each pixel has up to MAX_LAYERS depth-opacity samples.
//
// Pipeline:
// 1. Clear DSM counters
// 2. Build DSM: rasterize hair strands into depth-opacity layers
// 3. Sort layers per pixel (by depth)
// 4. Lookup: query transmittance at given depth for shadow evaluation
//
// Reference: UE5 HairStrandsDeepShadow.usf, Pixar Deep Shadow Maps (2000)

use bytemuck::{Pod, Zeroable};

/// Maximum depth-opacity layers per DSM pixel
pub const MAX_LAYERS_PER_PIXEL: u32 = 32;

/// DSM tile size (per light)
pub const DSM_TILE_SIZE: u32 = 512;

/// Deep shadow map parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DeepShadowParams {
    pub light_view_proj: [[f32; 4]; 4],
    pub light_direction: [f32; 3],
    pub shadow_bias: f32,
    pub atlas_offset: [u32; 2],
    pub atlas_size: u32,
    pub max_layers: u32,
    pub hair_opacity: f32,
    pub softness: f32,
    pub _pad: [f32; 2],
}

impl Default for DeepShadowParams {
    fn default() -> Self {
        Self {
            light_view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            light_direction: [0.0, -1.0, 0.0],
            shadow_bias: 0.001,
            atlas_offset: [0, 0],
            atlas_size: DSM_TILE_SIZE,
            max_layers: MAX_LAYERS_PER_PIXEL,
            hair_opacity: 0.5,
            softness: 1.0,
            _pad: [0.0; 2],
        }
    }
}

/// Sort DSM params (for per-pixel depth sorting pass)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DsmSortParams {
    pub atlas_size: u32,
    pub max_layers: u32,
    pub _pad: [u32; 2],
}

/// DSM lookup parameters (for shadow sampling in hair shading)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DsmLookupParams {
    pub light_view_proj: [[f32; 4]; 4],
    pub atlas_offset: [u32; 2],
    pub atlas_size: u32,
    pub shadow_bias: f32,
    pub softness: f32,
    pub _pad: [f32; 3],
}

/// Deep Shadow Map system
#[cfg(feature = "gpu")]
pub struct DeepShadowMap {
    // Build pass
    build_pipeline: wgpu::ComputePipeline,
    build_layout: wgpu::BindGroupLayout,

    // Sort pass (bitonic sort per pixel)
    sort_pipeline: wgpu::ComputePipeline,
    sort_layout: wgpu::BindGroupLayout,

    // GPU Buffers
    params_buffer: wgpu::Buffer,
    sort_params_buffer: wgpu::Buffer,

    /// DSM layer storage (depth + opacity per layer per pixel)
    /// Size: atlas_size^2 * MAX_LAYERS * 8 bytes
    pub layers_buffer: wgpu::Buffer,

    /// Per-pixel layer count (atomic u32)
    /// Size: atlas_size^2 * 4 bytes
    pub count_buffer: wgpu::Buffer,

    /// Current configuration
    pub atlas_size: u32,
}

#[cfg(feature = "gpu")]
impl DeepShadowMap {
    pub fn new(device: &wgpu::Device, atlas_size: u32) -> Self {
        // Build layout
        let build_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DSM Build Layout"),
            entries: &[
                bgl_uniform(0),         // params
                bgl_storage_read(1),    // strand vertices
                bgl_storage_rw(2),      // dsm layers
                bgl_storage_rw(3),      // dsm count
            ],
        });

        let build_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DSM Build Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/hair_deep_shadow.wgsl").into(),
            ),
        });

        let build_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DSM Build Pipeline Layout"),
            bind_group_layouts: &[&build_layout],
            immediate_size: 0,
        });

        let build_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DSM Build Pipeline"),
            layout: Some(&build_pipeline_layout),
            module: &build_shader,
            entry_point: Some("build_dsm"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Sort layout (reuses layers + count)
        let sort_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DSM Sort Layout"),
            entries: &[
                bgl_uniform(0),      // sort params
                bgl_storage_rw(1),   // dsm layers
                bgl_storage_read(2), // dsm count (read-only for sort)
            ],
        });

        // Sort shader - simple insertion sort per pixel (small N)
        let sort_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DSM Sort Shader"),
            source: wgpu::ShaderSource::Wgsl(DSM_SORT_SHADER.into()),
        });

        let sort_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DSM Sort Pipeline Layout"),
            bind_group_layouts: &[&sort_layout],
            immediate_size: 0,
        });

        let sort_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DSM Sort Pipeline"),
            layout: Some(&sort_pipeline_layout),
            module: &sort_shader,
            entry_point: Some("sort_layers"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Buffers
        let total_pixels = (atlas_size * atlas_size) as u64;
        let layers_size = total_pixels * MAX_LAYERS_PER_PIXEL as u64 * 8; // 2 x f32
        let count_size = total_pixels * 4; // u32

        let layers_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DSM Layers Buffer"),
            size: layers_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DSM Count Buffer"),
            size: count_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DSM Params Buffer"),
            size: std::mem::size_of::<DeepShadowParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sort_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DSM Sort Params Buffer"),
            size: std::mem::size_of::<DsmSortParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            build_pipeline,
            build_layout,
            sort_pipeline,
            sort_layout,
            params_buffer,
            sort_params_buffer,
            layers_buffer,
            count_buffer,
            atlas_size,
        }
    }

    /// Build the deep shadow map from hair strand vertices
    pub fn build(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        params: &DeepShadowParams,
        strand_vertices_buffer: &wgpu::Buffer,
        strand_vertex_count: u32,
    ) {
        // Upload params
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));

        // Clear count buffer
        encoder.clear_buffer(&self.count_buffer, 0, None);

        // Build pass
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DSM Build Bind Group"),
            layout: &self.build_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: strand_vertices_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.layers_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.count_buffer.as_entire_binding() },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DSM Build"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.build_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups((strand_vertex_count + 63) / 64, 1, 1);
        }

        // Sort pass
        let sort_params = DsmSortParams {
            atlas_size: self.atlas_size,
            max_layers: MAX_LAYERS_PER_PIXEL,
            _pad: [0; 2],
        };
        queue.write_buffer(&self.sort_params_buffer, 0, bytemuck::bytes_of(&sort_params));

        let sort_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DSM Sort Bind Group"),
            layout: &self.sort_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.sort_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.layers_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.count_buffer.as_entire_binding() },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DSM Sort"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.sort_pipeline);
            pass.set_bind_group(0, &sort_bg, &[]);
            let total_pixels = self.atlas_size * self.atlas_size;
            pass.dispatch_workgroups((total_pixels + 63) / 64, 1, 1);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "gpu")]
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

#[cfg(feature = "gpu")]
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

#[cfg(feature = "gpu")]
fn bgl_storage_rw(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

// Inline sort shader (small enough to embed)
#[cfg(feature = "gpu")]
const DSM_SORT_SHADER: &str = r#"
// DSM Layer Sort — Insertion sort per pixel (N <= 32, well suited)

struct DsmSortParams {
    atlas_size:  u32,
    max_layers:  u32,
    _pad:        vec2<u32>,
};

struct DeepShadowLayer {
    depth:   f32,
    opacity: f32,
};

@group(0) @binding(0) var<uniform> params: DsmSortParams;
@group(0) @binding(1) var<storage, read_write> dsm_layers: array<DeepShadowLayer>;
@group(0) @binding(2) var<storage, read> dsm_count: array<u32>;

const MAX_LAYERS: u32 = 32u;

@compute @workgroup_size(64)
fn sort_layers(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel_idx = gid.x;
    let total_pixels = params.atlas_size * params.atlas_size;
    if pixel_idx >= total_pixels {
        return;
    }

    let count = min(dsm_count[pixel_idx], MAX_LAYERS);
    if count <= 1u {
        return;
    }

    let base = pixel_idx * MAX_LAYERS;

    // Insertion sort (stable, O(N^2) but N <= 32)
    for (var i = 1u; i < count; i = i + 1u) {
        let key = dsm_layers[base + i];
        var j = i;
        while j > 0u && dsm_layers[base + j - 1u].depth > key.depth {
            dsm_layers[base + j] = dsm_layers[base + j - 1u];
            j = j - 1u;
        }
        dsm_layers[base + j] = key;
    }
}
"#;
