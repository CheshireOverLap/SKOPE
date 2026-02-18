//! Clipmap Radiance Cache
//!
//! Multi-level clipmap structure for world-space radiance caching.
//! Camera-near regions use high-resolution probe grids, distant regions
//! use progressively coarser grids. Replaces the uniform 64^3 grid.
//!
//! Each clipmap level is a 32^3 grid of probes, with the cell size
//! doubling at each successive level. This gives fine detail near
//! the camera and broad coverage far away, matching UE5 Lumen's
//! radiance cache clipmap design.

use crate::types::LumenConfig;
#[cfg(feature = "gpu")]
use crate::types::{ClipmapLevelParams, ClipmapUpdateParams, RadianceCacheProbe};

/// Maximum number of clipmap levels.
pub const MAX_CLIPMAPS: u32 = 6;
/// Grid resolution per axis per clipmap level.
pub const CLIPMAP_RESOLUTION: u32 = 32;
/// Per-probe octahedral texel resolution.
pub const PROBE_RESOLUTION: u32 = 8;
/// Tile size for trace dispatch.
pub const TRACE_TILE_SIZE: u32 = 8;

/// Per-level CPU state for the clipmap radiance cache.
#[derive(Clone, Debug)]
pub struct ClipmapLevel {
    /// World-space cell size (probe spacing) for this level.
    pub cell_size: f32,
    /// World-space corner (minimum corner) of this level's grid.
    pub corner_world: [f32; 3],
    /// Number of probes scheduled for tracing this frame.
    pub probes_this_frame: u32,
}

/// Clipmap radiance cache (CPU state).
///
/// Contains 6 levels of 32^3 probe grids. Level 0 has the finest
/// resolution centered on the camera; each subsequent level doubles
/// the cell size, covering a larger volume at coarser resolution.
pub struct RadianceCache {
    /// Number of active clipmap levels.
    pub num_clipmaps: u32,
    /// Grid resolution per axis per level.
    pub clipmap_resolution: u32,
    /// Per-probe octahedral texel resolution.
    pub probe_resolution: u32,
    /// World-space extent of the innermost (level 0) clipmap.
    pub base_extent: f32,
    /// Scale factor between successive levels (default 2.0).
    pub distribution_base: f32,
    /// Per-level state.
    pub levels: Vec<ClipmapLevel>,
    /// Total number of probes across all levels.
    pub total_probes: u32,
    /// Maximum probes to trace per frame (budget).
    pub trace_budget_per_frame: u32,
    /// Grid size for backward compatibility with renderer (= CLIPMAP_RESOLUTION).
    pub grid_size: u32,
    /// Probe spacing for backward compatibility (= level 0 cell_size).
    pub probe_spacing: f32,
    /// Origin for backward compatibility (camera-snapped).
    pub origin: [f32; 3],
    /// Current round-robin update offset.
    update_offset: u32,
}

impl RadianceCache {
    /// Create a new clipmap radiance cache from the given configuration.
    ///
    /// Builds `MAX_CLIPMAPS` levels. Level 0 cell_size is derived from
    /// `config.radiance_cache_probe_spacing`. Each subsequent level
    /// doubles the cell size.
    pub fn new(config: &LumenConfig) -> Self {
        let num_clipmaps = MAX_CLIPMAPS;
        let clipmap_resolution = CLIPMAP_RESOLUTION;
        let probe_resolution = PROBE_RESOLUTION;
        let distribution_base = 2.0_f32;

        // Base extent: the world-space extent of the innermost clipmap
        let base_extent = config.radiance_cache_probe_spacing * clipmap_resolution as f32;
        let base_cell_size = base_extent / clipmap_resolution as f32;

        let probes_per_level = clipmap_resolution * clipmap_resolution * clipmap_resolution;
        let total_probes = num_clipmaps * probes_per_level;
        let trace_budget_per_frame = (total_probes / 16).max(64);

        let mut levels = Vec::with_capacity(num_clipmaps as usize);
        for i in 0..num_clipmaps {
            let cell_size = base_cell_size * distribution_base.powi(i as i32);
            levels.push(ClipmapLevel {
                cell_size,
                corner_world: [0.0, 0.0, 0.0],
                probes_this_frame: 0,
            });
        }

        Self {
            num_clipmaps,
            clipmap_resolution,
            probe_resolution,
            base_extent,
            distribution_base,
            levels,
            total_probes,
            trace_budget_per_frame,
            // Backward compat fields
            grid_size: clipmap_resolution,
            probe_spacing: base_cell_size,
            origin: [0.0, 0.0, 0.0],
            update_offset: 0,
        }
    }

    /// Update clipmap origins to track the camera position.
    ///
    /// For each level, the corner is snapped to the level's grid so that
    /// the camera is centered within the level's volume.
    pub fn update_origin(&mut self, camera_pos: [f32; 3]) {
        for level in &mut self.levels {
            let half_extent = level.cell_size * self.clipmap_resolution as f32 * 0.5;
            // Corner = camera - half_extent, snapped to cell_size
            level.corner_world = [
                ((camera_pos[0] - half_extent) / level.cell_size).floor() * level.cell_size,
                ((camera_pos[1] - half_extent) / level.cell_size).floor() * level.cell_size,
                ((camera_pos[2] - half_extent) / level.cell_size).floor() * level.cell_size,
            ];
        }

        // Backward compat: set grid_size and probe_spacing for renderer
        self.grid_size = self.clipmap_resolution;
        self.probe_spacing = self.levels[0].cell_size;
        self.origin = [
            (camera_pos[0] / self.probe_spacing).round() * self.probe_spacing,
            (camera_pos[1] / self.probe_spacing).round() * self.probe_spacing,
            (camera_pos[2] / self.probe_spacing).round() * self.probe_spacing,
        ];
    }

    /// Get the range of probes to update this frame (round-robin).
    ///
    /// Returns `(start, end)` where `start` is inclusive and `end` is exclusive.
    pub fn update_range(&mut self) -> (u32, u32) {
        let start = self.update_offset;
        let end = (start + self.trace_budget_per_frame).min(self.total_probes);
        self.update_offset = if end >= self.total_probes { 0 } else { end };
        (start, end)
    }

    /// Half-extent of the outermost clipmap level.
    pub fn half_extent(&self) -> f32 {
        if let Some(last) = self.levels.last() {
            last.cell_size * self.clipmap_resolution as f32 * 0.5
        } else {
            0.0
        }
    }

    /// Build GPU parameters for all clipmap levels.
    #[cfg(feature = "gpu")]
    pub fn clipmap_params(&self) -> Vec<ClipmapLevelParams> {
        let probes_per_level = self.clipmap_resolution * self.clipmap_resolution * self.clipmap_resolution;
        self.levels
            .iter()
            .enumerate()
            .map(|(i, level)| ClipmapLevelParams {
                corner_world: level.corner_world,
                cell_size: level.cell_size,
                resolution: self.clipmap_resolution,
                level_index: i as u32,
                probe_offset: i as u32 * probes_per_level,
                probe_count: probes_per_level,
            })
            .collect()
    }

    /// Get the world position of a probe by its linear index.
    ///
    /// The index spans all clipmap levels sequentially.
    pub fn probe_world_pos(&self, index: u32) -> [f32; 3] {
        let probes_per_level = self.clipmap_resolution * self.clipmap_resolution * self.clipmap_resolution;
        let level_idx = (index / probes_per_level).min(self.num_clipmaps - 1);
        let local_idx = index % probes_per_level;

        let gs = self.clipmap_resolution;
        let ix = local_idx % gs;
        let iy = (local_idx / gs) % gs;
        let iz = local_idx / (gs * gs);

        let level = &self.levels[level_idx as usize];
        [
            level.corner_world[0] + ix as f32 * level.cell_size,
            level.corner_world[1] + iy as f32 * level.cell_size,
            level.corner_world[2] + iz as f32 * level.cell_size,
        ]
    }
}

/// GPU resources for the clipmap radiance cache.
///
/// Holds textures, buffers, and compute pipelines for the multi-level
/// clipmap probe system. Also maintains a backward-compatible
/// `probe_buffer` for use by the reflections pipeline.
#[cfg(feature = "gpu")]
pub struct RadianceCacheGpu {
    // --- Clipmap resources ---

    /// Indirection texture: 3D R32Uint mapping grid coords to probe atlas indices.
    pub indirection_texture: wgpu::Texture,
    /// View for the indirection texture (storage write).
    pub indirection_view: wgpu::TextureView,
    /// Separate read-only copy of the indirection texture (avoids aliasing
    /// when the same texture is bound as both WriteOnly storage and read
    /// texture in the Allocate pass). Copied from indirection_texture each frame.
    pub indirection_read_texture: wgpu::Texture,
    pub indirection_read_view: wgpu::TextureView,

    /// Probe radiance atlas: 2D Rgba16Float storing octahedral radiance per probe.
    pub radiance_atlas: wgpu::Texture,
    /// View for the radiance atlas.
    pub radiance_atlas_view: wgpu::TextureView,

    /// Probe depth atlas: 2D R16Float storing octahedral depth per probe.
    pub depth_atlas: wgpu::Texture,
    /// View for the depth atlas.
    pub depth_atlas_view: wgpu::TextureView,

    /// Per-probe world offset buffer (vec4 per probe).
    pub probe_world_offset: wgpu::Buffer,
    /// Free list for probe allocation (u32 per probe).
    pub probe_free_list: wgpu::Buffer,
    /// Atomic allocator counter (16 bytes).
    pub probe_allocator: wgpu::Buffer,
    /// Trace tile data buffer (u32 per probe, stores probe indices to trace).
    pub trace_tile_data: wgpu::Buffer,
    /// Trace tile allocator (16 bytes, atomic counter).
    pub trace_tile_allocator: wgpu::Buffer,
    /// Clipmap level parameters buffer.
    pub clipmap_params_buffer: wgpu::Buffer,

    /// Per-probe frame tracking for persistence: last frame each probe was traced.
    /// Probes with recent trace data are reused instead of reallocated.
    pub probe_last_traced: wgpu::Buffer,

    /// Backward compat: probe buffer matching old RadianceCacheProbe layout.
    /// Used by the reflections pipeline.
    pub probe_buffer: wgpu::Buffer,

    // --- Clipmap compute pipelines ---

    /// Mark pipeline: identifies which probes are needed by screen probes.
    pub mark_pipeline: wgpu::ComputePipeline,
    /// Bind group layout for the mark pipeline.
    pub mark_layout_g0: wgpu::BindGroupLayout,
    pub mark_layout_g1: wgpu::BindGroupLayout,
    pub mark_layout_g2: wgpu::BindGroupLayout,

    /// Allocate pipeline: allocates probes from the free list.
    pub allocate_pipeline: wgpu::ComputePipeline,
    /// Bind group layouts for the allocate pipeline.
    pub allocate_layout_g0: wgpu::BindGroupLayout,
    pub allocate_layout_g1: wgpu::BindGroupLayout,
    pub allocate_layout_g2: wgpu::BindGroupLayout,

    /// Trace pipeline: traces rays for dirty probes.
    pub trace_pipeline: wgpu::ComputePipeline,
    /// Bind group layouts for the trace pipeline.
    pub trace_layout_g0: wgpu::BindGroupLayout,
    pub trace_layout_g1: wgpu::BindGroupLayout,
    pub trace_layout_g2: wgpu::BindGroupLayout,
    pub trace_layout_g3: wgpu::BindGroupLayout,

    /// Integrate pipeline: integrates traced radiance into the atlas.
    pub integrate_pipeline: wgpu::ComputePipeline,
    /// Bind group layouts for the integrate pipeline.
    pub integrate_layout_g0: wgpu::BindGroupLayout,
    pub integrate_layout_g1: wgpu::BindGroupLayout,
    pub integrate_layout_g2: wgpu::BindGroupLayout,

    /// Clipmap update parameters uniform buffer.
    pub params_buffer: wgpu::Buffer,
    /// Total probes across all clipmap levels.
    pub total_probes: u32,
}

#[cfg(feature = "gpu")]
impl RadianceCacheGpu {
    /// Create GPU resources for the clipmap radiance cache.
    ///
    /// `total_probes` is the total number of probes across all levels.
    /// `num_clipmaps` is the number of clipmap levels (typically 6).
    ///
    /// For backward compatibility, this also accepts being called with
    /// just `(device, total_probes)` — num_clipmaps defaults to MAX_CLIPMAPS.
    pub fn new(device: &wgpu::Device, total_probes: u32) -> Self {
        Self::new_with_clipmaps(device, total_probes, MAX_CLIPMAPS)
    }

    pub fn new_with_clipmaps(device: &wgpu::Device, total_probes: u32, num_clipmaps: u32) -> Self {
        // --- Indirection texture: 3D R32Uint ---
        // Dimensions: CLIPMAP_RESOLUTION x CLIPMAP_RESOLUTION x (CLIPMAP_RESOLUTION * num_clipmaps)
        let indirection_size = wgpu::Extent3d {
            width: CLIPMAP_RESOLUTION,
            height: CLIPMAP_RESOLUTION,
            depth_or_array_layers: CLIPMAP_RESOLUTION * num_clipmaps,
        };
        let indirection_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Radiance Cache Clipmap Indirection"),
            size: indirection_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let indirection_view = indirection_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Radiance Cache Indirection (write)"),
            ..Default::default()
        });
        // Separate read-only copy to avoid read+write aliasing in wgpu
        let indirection_read_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Radiance Cache Clipmap Indirection (read copy)"),
            size: indirection_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let indirection_read_view = indirection_read_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Radiance Cache Indirection (read)"),
            ..Default::default()
        });

        // --- Radiance atlas: 2D Rgba16Float ---
        // Layout: PROBE_RESOLUTION * 256 wide, PROBE_RESOLUTION * ceil(total_probes / 256) tall
        let atlas_cols = 256u32;
        let atlas_rows = total_probes.div_ceil(atlas_cols);
        let atlas_width = PROBE_RESOLUTION * atlas_cols;
        let atlas_height = PROBE_RESOLUTION * atlas_rows;

        let radiance_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Radiance Cache Radiance Atlas"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let radiance_atlas_view = radiance_atlas.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Depth atlas: 2D R16Float ---
        let depth_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Radiance Cache Depth Atlas"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_atlas_view = depth_atlas.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Buffers ---
        let probe_world_offset = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Probe World Offset"),
            size: 16 * total_probes as u64, // vec4<f32> per probe
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let probe_free_list = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Probe Free List"),
            size: 4 * total_probes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let probe_allocator = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Probe Allocator"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let trace_tile_data = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Trace Tile Data"),
            size: 4 * total_probes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let trace_tile_allocator = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Trace Tile Allocator"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Per-probe last-traced frame (u32 per probe), initialized to 0
        let probe_last_traced = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Probe Last Traced"),
            size: 4 * total_probes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let clipmap_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Clipmap Params"),
            size: std::mem::size_of::<ClipmapLevelParams>() as u64 * MAX_CLIPMAPS as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        // Backward compat probe buffer (RadianceCacheProbe * total_probes)
        let probe_size = std::mem::size_of::<RadianceCacheProbe>() as u64;
        let probe_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Radiance Cache Probes (compat)"),
            size: probe_size * total_probes as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Radiance Cache Clipmap Update Params"),
            size: std::mem::size_of::<ClipmapUpdateParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Compute pipelines ---

        // Mark pipeline
        let (mark_pipeline, mark_layout_g0, mark_layout_g1, mark_layout_g2) =
            Self::create_mark_pipeline(device);

        // Allocate pipeline
        let (allocate_pipeline, allocate_layout_g0, allocate_layout_g1, allocate_layout_g2) =
            Self::create_allocate_pipeline(device);

        // Trace pipeline
        let (trace_pipeline, trace_layout_g0, trace_layout_g1, trace_layout_g2, trace_layout_g3) =
            Self::create_trace_pipeline(device);

        // Integrate pipeline
        let (integrate_pipeline, integrate_layout_g0, integrate_layout_g1, integrate_layout_g2) =
            Self::create_integrate_pipeline(device);

        Self {
            indirection_texture,
            indirection_view,
            indirection_read_texture,
            indirection_read_view,
            radiance_atlas,
            radiance_atlas_view,
            depth_atlas,
            depth_atlas_view,
            probe_world_offset,
            probe_free_list,
            probe_allocator,
            trace_tile_data,
            trace_tile_allocator,
            probe_last_traced,
            clipmap_params_buffer,
            probe_buffer,
            mark_pipeline,
            mark_layout_g0,
            mark_layout_g1,
            mark_layout_g2,
            allocate_pipeline,
            allocate_layout_g0,
            allocate_layout_g1,
            allocate_layout_g2,
            trace_pipeline,
            trace_layout_g0,
            trace_layout_g1,
            trace_layout_g2,
            trace_layout_g3,
            integrate_pipeline,
            integrate_layout_g0,
            integrate_layout_g1,
            integrate_layout_g2,
            params_buffer,
            total_probes,
        }
    }

    /// Create the mark pipeline: identifies which probes need tracing.
    fn create_mark_pipeline(
        device: &wgpu::Device,
    ) -> (
        wgpu::ComputePipeline,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
    ) {
        // G0: ClipmapUpdateParams uniform
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Mark G0"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // G1: indirection_texture (read), probe_world_offset (read)
        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Mark G1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
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
            ],
        });

        // G2: clipmap_levels (read), trace_tile_data (write), trace_tile_allocator (read_write), probe_last_traced (read_write)
        let g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Mark G2"),
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
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Radiance Cache Mark Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_radiance_cache_mark.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RC Mark Pipeline Layout"),
            bind_group_layouts: &[&g0, &g1, &g2],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Radiance Cache Mark"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1, g2)
    }

    /// Create the allocate pipeline: allocates probes from the free list.
    fn create_allocate_pipeline(
        device: &wgpu::Device,
    ) -> (
        wgpu::ComputePipeline,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
    ) {
        // G0: ClipmapUpdateParams uniform + indirection (write) + indirection_read (read)
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Allocate G0"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
                // indirection_read: read existing indirection for persistence check
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // G1: free_list (read_write), allocator (read_write)
        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Allocate G1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
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

        // G2: trace_tiles (read), trace_tile_count (read)
        let g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Allocate G2"),
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
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Radiance Cache Allocate Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_radiance_cache_allocate.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RC Allocate Pipeline Layout"),
            bind_group_layouts: &[&g0, &g1, &g2],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Radiance Cache Allocate"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1, g2)
    }

    /// Create the trace pipeline: traces radiance rays for dirty probes.
    fn create_trace_pipeline(
        device: &wgpu::Device,
    ) -> (
        wgpu::ComputePipeline,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
    ) {
        // G0: ClipmapUpdateParams uniform
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Trace G0"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // G1: trace_tile_data (read), radiance_atlas (write)
        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Trace G1"),
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
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // G2: clipmap_params (read), probe_world_offset (read)
        let g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Trace G2"),
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
            ],
        });

        // G3: SDF params, SDF volume, SDF sampler, prev HDR, HDR sampler
        let g3 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Trace G3"),
            entries: &[
                // binding 0: SDF params uniform
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
                // binding 1: SDF volume texture (R32Float is non-filterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: SDF sampler (non-filtering for R32Float)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 3: prev HDR texture
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
                // binding 4: HDR sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Radiance Cache Trace Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_radiance_cache_trace.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RC Trace Pipeline Layout"),
            bind_group_layouts: &[&g0, &g1, &g2, &g3],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Radiance Cache Trace"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1, g2, g3)
    }

    /// Create the integrate pipeline: integrates trace results into the atlas.
    fn create_integrate_pipeline(
        device: &wgpu::Device,
    ) -> (
        wgpu::ComputePipeline,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
        wgpu::BindGroupLayout,
    ) {
        // G0: ClipmapUpdateParams uniform
        let g0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Integrate G0"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // G1: radiance_atlas (read as texture), depth_atlas (write)
        let g1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Integrate G1"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // G2: probe_buffer (read_write) — backward compat probe buffer
        let g2 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RC Integrate G2"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Radiance Cache Integrate Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_radiance_cache_integrate.wgsl").into(),
            ),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RC Integrate Pipeline Layout"),
            bind_group_layouts: &[&g0, &g1, &g2],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Radiance Cache Integrate"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        (pipeline, g0, g1, g2)
    }

    /// Write CPU-side `RadianceCache` state to GPU uniform/storage buffers.
    ///
    /// Uploads `ClipmapUpdateParams` and per-level `ClipmapLevelParams`.
    pub fn update_gpu_state(
        &self,
        queue: &wgpu::Queue,
        cache: &RadianceCache,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        screen_width: u32,
        screen_height: u32,
        frame_index: u32,
        max_trace_distance: f32,
    ) {
        let screen_probe_spacing = 16u32;
        let screen_probes_x = screen_width.div_ceil(screen_probe_spacing);
        let screen_probes_y = screen_height.div_ceil(screen_probe_spacing);

        let params = ClipmapUpdateParams {
            view_proj,
            camera_pos,
            _align_camera: 0,
            num_clipmaps: cache.num_clipmaps,
            probe_resolution: cache.probe_resolution,
            trace_budget: cache.trace_budget_per_frame,
            frame_index,
            total_probes: cache.total_probes,
            screen_width,
            screen_height,
            screen_probe_spacing,
            screen_probes_x,
            screen_probes_y,
            max_trace_distance,
            _pad: 0,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Upload per-level clipmap params
        let level_params = cache.clipmap_params();
        queue.write_buffer(
            &self.clipmap_params_buffer,
            0,
            bytemuck::cast_slice(&level_params),
        );
    }

    /// Execute all 4 clipmap radiance cache passes:
    ///   1. Mark  — identify which probes need tracing
    ///   2. Allocate — allocate probe slots from the free list
    ///   3. Trace — trace radiance rays for dirty probes
    ///   4. Integrate — integrate results into atlas + legacy probe buffer
    ///
    /// Pre-fill the backward-compat `probe_buffer` with valid world
    /// positions from CPU-side cache data. Call once after creation and
    /// after `RadianceCache::update_origin()` to avoid zeroed positions
    /// on the first frame.
    pub fn initialize_probe_positions(
        &self,
        queue: &wgpu::Queue,
        cache: &RadianceCache,
    ) {
        let total = cache.total_probes;
        let probe_size = std::mem::size_of::<RadianceCacheProbe>();

        // Build a temporary buffer with world positions filled in
        let mut data = vec![0u8; probe_size * total as usize];
        for i in 0..total {
            let pos = cache.probe_world_pos(i);
            let offset = i as usize * probe_size;
            // RadianceCacheProbe layout: position [f32;3] at offset 0
            if offset + 12 <= data.len() {
                data[offset..offset + 4].copy_from_slice(&pos[0].to_le_bytes());
                data[offset + 4..offset + 8].copy_from_slice(&pos[1].to_le_bytes());
                data[offset + 8..offset + 12].copy_from_slice(&pos[2].to_le_bytes());
            }
        }
        queue.write_buffer(&self.probe_buffer, 0, &data);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        cache: &RadianceCache,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        screen_width: u32,
        screen_height: u32,
        frame_index: u32,
        max_trace_distance: f32,
        // SDF resources for trace pass
        sdf_params_buf: &wgpu::Buffer,
        sdf_view: &wgpu::TextureView,
        sdf_sampler: &wgpu::Sampler,
        prev_hdr_view: &wgpu::TextureView,
        hdr_sampler: &wgpu::Sampler,
    ) {
        // Step 0: Upload CPU state to GPU buffers
        self.update_gpu_state(
            queue,
            cache,
            view_proj,
            camera_pos,
            screen_width,
            screen_height,
            frame_index,
            max_trace_distance,
        );

        // Reset the trace tile allocator to 0 before mark pass
        queue.write_buffer(&self.trace_tile_allocator, 0, &[0u8; 16]);

        let total = self.total_probes;

        // ---- Pass 1: Mark ----
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Mark BG0"),
                layout: &self.mark_layout_g0,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                }],
            });
            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Mark BG1"),
                layout: &self.mark_layout_g1,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.indirection_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.probe_world_offset.as_entire_binding(),
                    },
                ],
            });
            let bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Mark BG2"),
                layout: &self.mark_layout_g2,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.clipmap_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.trace_tile_data.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.trace_tile_allocator.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.probe_last_traced.as_entire_binding(),
                    },
                ],
            });

            let workgroups = total.div_ceil(64);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Radiance Cache Mark"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.mark_pipeline);
            pass.set_bind_group(0, Some(&bg0), &[]);
            pass.set_bind_group(1, Some(&bg1), &[]);
            pass.set_bind_group(2, Some(&bg2), &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy indirection texture → read copy (needed because wgpu forbids
        // using the same physical texture as both storage-write and texture-read
        // in a single dispatch).
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.indirection_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.indirection_read_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            self.indirection_texture.size(),
        );

        // ---- Pass 2: Allocate ----
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Allocate BG0"),
                layout: &self.allocate_layout_g0,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.indirection_view),
                    },
                    // indirection_read: separate read-only view to avoid aliasing violation
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.indirection_read_view),
                    },
                ],
            });
            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Allocate BG1"),
                layout: &self.allocate_layout_g1,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.probe_free_list.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.probe_allocator.as_entire_binding(),
                    },
                ],
            });
            let bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Allocate BG2"),
                layout: &self.allocate_layout_g2,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.trace_tile_data.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.trace_tile_allocator.as_entire_binding(),
                    },
                ],
            });

            let workgroups = cache.trace_budget_per_frame.div_ceil(64);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Radiance Cache Allocate"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.allocate_pipeline);
            pass.set_bind_group(0, Some(&bg0), &[]);
            pass.set_bind_group(1, Some(&bg1), &[]);
            pass.set_bind_group(2, Some(&bg2), &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // ---- Pass 3: Trace ----
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Trace BG0"),
                layout: &self.trace_layout_g0,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                }],
            });
            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Trace BG1"),
                layout: &self.trace_layout_g1,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.trace_tile_data.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.radiance_atlas_view),
                    },
                ],
            });
            let bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Trace BG2"),
                layout: &self.trace_layout_g2,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.clipmap_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.probe_world_offset.as_entire_binding(),
                    },
                ],
            });
            let bg3 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Trace BG3"),
                layout: &self.trace_layout_g3,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: sdf_params_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(sdf_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sdf_sampler) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(prev_hdr_view) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(hdr_sampler) },
                ],
            });

            // Dispatch: (trace_budget * probe_resolution / 8, probe_resolution / 8, 1)
            // workgroup_size(8, 8) — each workgroup covers 8x8 texels
            let pr = cache.probe_resolution;
            let dispatch_x = (cache.trace_budget_per_frame * pr).div_ceil(TRACE_TILE_SIZE);
            let dispatch_y = pr.div_ceil(TRACE_TILE_SIZE);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Radiance Cache Trace"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.trace_pipeline);
            pass.set_bind_group(0, Some(&bg0), &[]);
            pass.set_bind_group(1, Some(&bg1), &[]);
            pass.set_bind_group(2, Some(&bg2), &[]);
            pass.set_bind_group(3, Some(&bg3), &[]);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        // ---- Pass 4: Integrate ----
        {
            let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Integrate BG0"),
                layout: &self.integrate_layout_g0,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                }],
            });
            let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Integrate BG1"),
                layout: &self.integrate_layout_g1,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.radiance_atlas_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.depth_atlas_view),
                    },
                ],
            });
            let bg2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("RC Integrate BG2"),
                layout: &self.integrate_layout_g2,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.probe_buffer.as_entire_binding(),
                }],
            });

            let workgroups = total.div_ceil(64);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Radiance Cache Integrate"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.integrate_pipeline);
            pass.set_bind_group(0, Some(&bg0), &[]);
            pass.set_bind_group(1, Some(&bg1), &[]);
            pass.set_bind_group(2, Some(&bg2), &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radiance_cache_clipmap() {
        let config = LumenConfig::default();
        let cache = RadianceCache::new(&config);
        assert_eq!(cache.num_clipmaps, MAX_CLIPMAPS);
        assert_eq!(cache.clipmap_resolution, CLIPMAP_RESOLUTION);
        assert!(cache.total_probes > 0);
        assert_eq!(cache.levels.len(), MAX_CLIPMAPS as usize);
        // 6 levels * 32^3 = 196608
        assert_eq!(cache.total_probes, MAX_CLIPMAPS * CLIPMAP_RESOLUTION * CLIPMAP_RESOLUTION * CLIPMAP_RESOLUTION);
    }

    #[test]
    fn test_clipmap_level_sizes() {
        let config = LumenConfig::default();
        let cache = RadianceCache::new(&config);
        // Each level should be 2x the cell size of the previous
        for i in 1..cache.levels.len() {
            let ratio = cache.levels[i].cell_size / cache.levels[i - 1].cell_size;
            assert!((ratio - 2.0).abs() < 0.01, "Level {} ratio was {}", i, ratio);
        }
    }

    #[test]
    fn test_update_range_wraps() {
        let config = LumenConfig::default();
        let mut cache = RadianceCache::new(&config);
        let (s1, e1) = cache.update_range();
        assert_eq!(s1, 0);
        assert!(e1 <= cache.total_probes);
        for _ in 0..100 {
            cache.update_range();
        }
        // Should have wrapped at least once without panicking
    }

    #[test]
    fn test_update_origin_snaps() {
        let config = LumenConfig::default();
        let mut cache = RadianceCache::new(&config);
        cache.update_origin([10.0, 20.0, 30.0]);

        // All level corners should be set
        for level in &cache.levels {
            assert!(level.corner_world[0] != 0.0 || level.corner_world[1] != 0.0 || level.corner_world[2] != 0.0,
                "Level corners should be updated");
        }

        // Backward compat fields should be set
        assert_eq!(cache.grid_size, CLIPMAP_RESOLUTION);
        assert!(cache.probe_spacing > 0.0);
    }

    #[test]
    fn test_half_extent() {
        let config = LumenConfig::default();
        let cache = RadianceCache::new(&config);
        let he = cache.half_extent();
        // Outermost level cell_size = base_cell_size * 2^5 = 4.0 * 32 = 128
        // half_extent = 128 * 32 * 0.5 = 2048
        assert!(he > 0.0, "Half extent should be positive");
        // It should be larger than inner levels
        let inner_he = cache.levels[0].cell_size * CLIPMAP_RESOLUTION as f32 * 0.5;
        assert!(he > inner_he, "Outer half_extent should be larger than inner");
    }

    #[test]
    fn test_probe_world_pos() {
        let config = LumenConfig::default();
        let mut cache = RadianceCache::new(&config);
        cache.update_origin([0.0, 0.0, 0.0]);

        // First probe of level 0
        let pos0 = cache.probe_world_pos(0);
        // Should be at the corner of level 0
        assert!((pos0[0] - cache.levels[0].corner_world[0]).abs() < 0.001);

        // First probe of level 1
        let probes_per_level = CLIPMAP_RESOLUTION * CLIPMAP_RESOLUTION * CLIPMAP_RESOLUTION;
        let pos1 = cache.probe_world_pos(probes_per_level);
        assert!((pos1[0] - cache.levels[1].corner_world[0]).abs() < 0.001);
    }
}
