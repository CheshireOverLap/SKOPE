// SKOPE Engine — Variable Rate Shading (VRS)
//
// Generates a per-tile shading rate map based on scene analysis:
// - Motion vectors: fast-moving areas can use coarser shading
// - Luminance variance: uniform areas don't need full-rate
// - Depth discontinuities: edges always get full-rate
// - Specular highlights: glossy surfaces stay full-rate
//
// The VRS map is a uint8 texture where each texel represents one tile.
// Tile size is hardware-dependent (typically 8x8 or 16x16).
//
// Reference: UE5 VariableRateShading.usf, DX12 VRS Tier 2

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Shading rate per tile
#[repr(u32)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingRate {
    /// 1x1 — Full rate
    Full = 0,
    /// 1x2 or 2x1 — Half rate
    Half = 1,
    /// 2x2 — Quarter rate
    Quarter = 2,
    /// 2x4 or 4x2 — Eighth rate
    Eighth = 3,
    /// 4x4 — Sixteenth rate
    Sixteenth = 4,
}

/// VRS classification parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VrsParams {
    pub screen_width: u32,
    pub screen_height: u32,
    pub tile_size: u32,
    pub motion_threshold: f32,
    pub variance_threshold: f32,
    pub edge_sensitivity: f32,
    pub max_rate: u32,
    pub _pad: u32,
}

impl Default for VrsParams {
    fn default() -> Self {
        Self {
            screen_width: 1920,
            screen_height: 1080,
            tile_size: 16,
            motion_threshold: 0.01,
            variance_threshold: 0.005,
            edge_sensitivity: 0.1,
            max_rate: 2, // Quarter rate max by default
            _pad: 0,
        }
    }
}

/// VRS statistics (for profiling)
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct VrsStats {
    pub total_tiles: u32,
    pub full_rate_tiles: u32,
    pub half_rate_tiles: u32,
    pub quarter_rate_tiles: u32,
    pub coarser_tiles: u32,
    /// Estimated shading rate reduction (1.0 = no savings, 0.25 = 4x savings)
    pub avg_shading_rate: f32,
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

pub struct VrsPipeline {
    classify_pipeline: wgpu::ComputePipeline,
    classify_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,

    /// VRS map: each texel = one tile's shading rate (R8Uint)
    pub vrs_map: wgpu::Texture,
    pub vrs_map_view: wgpu::TextureView,

    pub tiles_x: u32,
    pub tiles_y: u32,
    pub tile_size: u32,
    screen_width: u32,
    screen_height: u32,
}

impl VrsPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, tile_size: u32) -> Self {
        let classify_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VRS Classify Layout"),
            entries: &[
                // binding 0: params
                bgl_uniform(0),
                // binding 1: color texture
                bgl_float_tex(1),
                // binding 2: motion vectors
                bgl_float_tex(2),
                // binding 3: depth
                bgl_depth_tex(3),
                // binding 4: VRS map output
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VRS Classify Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/vrs_classify.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VRS Classify Pipeline Layout"),
            bind_group_layouts: &[&classify_layout],
            immediate_size: 0,
        });

        let classify_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VRS Classify Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VRS Params"),
            size: std::mem::size_of::<VrsParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tiles_x = (width + tile_size - 1) / tile_size;
        let tiles_y = (height + tile_size - 1) / tile_size;

        let vrs_map = create_vrs_texture(device, tiles_x, tiles_y);
        let vrs_map_view = vrs_map.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            classify_pipeline,
            classify_layout,
            params_buffer,
            vrs_map,
            vrs_map_view,
            tiles_x,
            tiles_y,
            tile_size,
            screen_width: width,
            screen_height: height,
        }
    }

    /// Classify tiles and generate VRS map
    pub fn classify(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        motion_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        max_rate: u32,
    ) {
        let params = VrsParams {
            screen_width: self.screen_width,
            screen_height: self.screen_height,
            tile_size: self.tile_size,
            motion_threshold: 0.01,
            variance_threshold: 0.005,
            edge_sensitivity: 0.1,
            max_rate,
            _pad: 0,
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VRS Classify Bind Group"),
            layout: &self.classify_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(color_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(motion_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.vrs_map_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("VRS Classify"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.classify_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (self.tiles_x + 7) / 8,
                (self.tiles_y + 7) / 8,
                1,
            );
        }
    }

    /// Resize VRS map when resolution changes
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.screen_width == width && self.screen_height == height {
            return;
        }
        self.screen_width = width;
        self.screen_height = height;
        self.tiles_x = (width + self.tile_size - 1) / self.tile_size;
        self.tiles_y = (height + self.tile_size - 1) / self.tile_size;

        self.vrs_map = create_vrs_texture(device, self.tiles_x, self.tiles_y);
        self.vrs_map_view = self.vrs_map.create_view(&wgpu::TextureViewDescriptor::default());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_vrs_texture(device: &wgpu::Device, tiles_x: u32, tiles_y: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("VRS Map"),
        size: wgpu::Extent3d {
            width: tiles_x,
            height: tiles_y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
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

fn bgl_float_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
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
