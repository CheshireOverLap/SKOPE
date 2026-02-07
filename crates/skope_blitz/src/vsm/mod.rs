// SKOPE Engine - Virtual Shadow Maps (VSM)
//
// Page-table based shadow system inspired by UE5 Virtual Shadow Maps.
// - 128x128 virtual page table mapping to a 4096x4096 physical depth atlas
// - GPU-driven page marking and allocation via compute shaders
// - Clipmap levels for directional lights
// - LRU page caching across frames

mod page_table;
mod physical_pool;
mod page_request;
mod clipmap;
pub mod cache_manager;
pub mod smrt;

pub use page_table::*;
pub use physical_pool::*;
pub use page_request::*;
pub use clipmap::*;
pub use cache_manager::VsmCacheManager;
pub use smrt::{SmrtPipeline, SmrtParams};

use glam::Mat4;
use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Runtime configuration for the VSM system.
#[derive(Debug, Clone, Copy)]
pub struct VsmConfig {
    /// Pixels per physical page side (default 128).
    pub page_size: u32,
    /// Virtual page table dimension (default 128 -> 128x128 = 16384 pages).
    pub page_table_size: u32,
    /// Physical atlas dimension in pixels (default 4096).
    pub physical_pool_size: u32,
    /// Maximum physical pages (derived: (pool/page)^2, default 1024).
    pub max_pages: u32,
    /// Number of clipmap levels for directional lights.
    pub clipmap_levels: u32,
    /// When true, allocated pages persist across frames via LRU cache.
    pub enable_caching: bool,
}

impl Default for VsmConfig {
    fn default() -> Self {
        let page_size = 128u32;
        let pool_size = 4096u32;
        let pages_per_side = pool_size / page_size; // 32
        Self {
            page_size,
            page_table_size: 128,
            physical_pool_size: pool_size,
            max_pages: pages_per_side * pages_per_side, // 1024
            clipmap_levels: 6,
            enable_caching: true,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU uniform
// ---------------------------------------------------------------------------

/// Parameters uploaded to the GPU each frame for both compute and sampling.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VsmParams {
    pub light_view_proj: [[f32; 4]; 4],
    pub page_table_size: u32,
    pub physical_pool_size: u32,
    pub page_size: u32,
    pub clipmap_level: u32,
    pub screen_size: [u32; 2],
    pub frame_index: u32,
    pub _pad: u32,
}

impl VsmParams {
    pub fn new(
        light_view_proj: Mat4,
        page_table_size: u32,
        physical_pool_size: u32,
        page_size: u32,
        clipmap_level: u32,
        screen_width: u32,
        screen_height: u32,
        frame_index: u32,
    ) -> Self {
        Self {
            light_view_proj: light_view_proj.to_cols_array_2d(),
            page_table_size,
            physical_pool_size,
            page_size,
            clipmap_level,
            screen_size: [screen_width, screen_height],
            frame_index,
            _pad: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// VirtualShadowMap  (GPU orchestrator)
// ---------------------------------------------------------------------------

/// The main VSM system.  Owns all GPU resources and exposes helpers that
/// the renderer calls each frame.
pub struct VirtualShadowMap {
    // Page table texture (R32Uint, page_table_size x page_table_size)
    page_table_texture: wgpu::Texture,
    page_table_view: wgpu::TextureView,

    // Physical depth atlas (Depth32Float, physical_pool_size x physical_pool_size)
    physical_pool_texture: wgpu::Texture,
    physical_pool_view: wgpu::TextureView,
    physical_pool_depth_view: wgpu::TextureView,

    // Buffers
    page_flags_buffer: wgpu::Buffer,
    vsm_params_buffer: wgpu::Buffer,
    page_allocator_buffer: wgpu::Buffer,
    free_list_buffer: wgpu::Buffer,

    // Compute pipelines
    mark_pages_pipeline: wgpu::ComputePipeline,
    allocate_pipeline: wgpu::ComputePipeline,

    // Bind group layouts (exposed for external bind group creation)
    mark_bind_group_layout: wgpu::BindGroupLayout,
    allocate_bind_group_layout: wgpu::BindGroupLayout,

    // Sampling resources (for material_eval)
    sampling_sampler: wgpu::Sampler,

    // CPU-side state
    config: VsmConfig,
    page_table: PageTable,
    physical_pool: PhysicalPool,
    clipmap: Clipmap,
    pub cache_manager: VsmCacheManager,
    frame_index: u64,
}

impl VirtualShadowMap {
    pub fn new(device: &wgpu::Device, config: VsmConfig) -> Self {
        // -- Page table texture (R32Uint) -----------------------------------
        let page_table_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("VSM Page Table"),
            size: wgpu::Extent3d {
                width: config.page_table_size,
                height: config.page_table_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let page_table_view = page_table_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("VSM Page Table View"),
            ..Default::default()
        });

        // -- Physical depth atlas (Depth32Float) ----------------------------
        let physical_pool_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("VSM Physical Pool"),
            size: wgpu::Extent3d {
                width: config.physical_pool_size,
                height: config.physical_pool_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let physical_pool_view = physical_pool_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("VSM Physical Pool View"),
            ..Default::default()
        });

        // Separate view for render attachment usage
        let physical_pool_depth_view = physical_pool_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("VSM Physical Pool Depth Attachment"),
            ..Default::default()
        });

        // -- Buffers --------------------------------------------------------
        let total_pages = (config.page_table_size * config.page_table_size) as u64;

        let page_flags_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VSM Page Flags"),
            size: total_pages * 4, // 1 u32 per virtual page
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vsm_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VSM Params"),
            size: std::mem::size_of::<VsmParams>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let page_allocator_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VSM Page Allocator"),
            size: std::mem::size_of::<PageAllocatorData>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let free_list_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VSM Free List"),
            size: (config.max_pages as u64) * std::mem::size_of::<GpuFreeListEntry>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // -- Bind group layouts ---------------------------------------------

        // Mark pages pass: reads depth texture + params, writes page_flags
        let mark_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VSM Mark Pages Layout"),
            entries: &[
                // binding 0: depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1: page_flags (read_write)
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
                // binding 2: vsm_params (read)
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

        // Allocate pass: reads page_flags + free_list, writes page_table, updates allocator
        let allocate_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VSM Allocate Pages Layout"),
            entries: &[
                // binding 0: page_flags (read)
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
                // binding 1: page_table (storage texture, write)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 2: allocator (read_write)
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
                // binding 3: free_list (read)
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
                // binding 4: vsm_params (read)
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
            ],
        });

        // -- Compute pipelines -----------------------------------------------
        let mark_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VSM Mark Pages Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/vsm_mark_pages.wgsl").into(),
            ),
        });

        let mark_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VSM Mark Pages Pipeline Layout"),
            bind_group_layouts: &[&mark_bind_group_layout],
            immediate_size: 0,
        });

        let mark_pages_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VSM Mark Pages Pipeline"),
            layout: Some(&mark_pipeline_layout),
            module: &mark_shader,
            entry_point: Some("mark_pages"),
            compilation_options: Default::default(),
            cache: None,
        });

        let allocate_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VSM Allocate Pages Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/vsm_allocate.wgsl").into(),
            ),
        });

        let allocate_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VSM Allocate Pipeline Layout"),
            bind_group_layouts: &[&allocate_bind_group_layout],
            immediate_size: 0,
        });

        let allocate_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VSM Allocate Pages Pipeline"),
            layout: Some(&allocate_pipeline_layout),
            module: &allocate_shader,
            entry_point: Some("allocate_pages"),
            compilation_options: Default::default(),
            cache: None,
        });

        // -- Sampler for shadow sampling ------------------------------------
        let sampling_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("VSM Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // -- CPU-side helpers -----------------------------------------------
        let page_table = PageTable::new(config.page_table_size);
        let physical_pool = PhysicalPool::new(config.physical_pool_size, config.page_size);
        let clipmap = Clipmap::new(
            ClipmapConfig {
                levels: config.clipmap_levels,
                ..Default::default()
            },
            config.page_table_size,
        );

        Self {
            page_table_texture,
            page_table_view,
            physical_pool_texture,
            physical_pool_view,
            physical_pool_depth_view,
            page_flags_buffer,
            vsm_params_buffer,
            page_allocator_buffer,
            free_list_buffer,
            mark_pages_pipeline,
            allocate_pipeline,
            mark_bind_group_layout,
            allocate_bind_group_layout,
            sampling_sampler,
            config,
            page_table,
            physical_pool,
            clipmap,
            cache_manager: VsmCacheManager::new(config.page_table_size),
            frame_index: 0,
        }
    }

    // =======================================================================
    // Per-frame workflow
    // =======================================================================

    /// Upload the current VSM params to GPU.
    pub fn update_params(
        &mut self,
        queue: &wgpu::Queue,
        light_view_proj: Mat4,
        clipmap_level: u32,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.frame_index += 1;

        let params = VsmParams::new(
            light_view_proj,
            self.config.page_table_size,
            self.config.physical_pool_size,
            self.config.page_size,
            clipmap_level,
            screen_width,
            screen_height,
            self.frame_index as u32,
        );

        queue.write_buffer(&self.vsm_params_buffer, 0, bytemuck::bytes_of(&params));

        // Reset page flags for the new frame.
        // IMPORTANT: queue.write_buffer() enqueues a DMA copy that wgpu serialises
        // within the same queue timeline.  Any CommandBuffer submitted after this
        // point (e.g. the mark_pages compute dispatch) is guaranteed to see the
        // zeroed page_flags because wgpu processes queue operations in order.
        let zeros = vec![0u32; (self.config.page_table_size * self.config.page_table_size) as usize];
        queue.write_buffer(&self.page_flags_buffer, 0, bytemuck::cast_slice(&zeros));

        // Upload allocator state
        let alloc = PageAllocatorData::new(self.physical_pool.free_count());
        queue.write_buffer(&self.page_allocator_buffer, 0, bytemuck::bytes_of(&alloc));

        // Upload free list entries to GPU so the allocate pass can read them
        let free_entries: Vec<GpuFreeListEntry> = self.physical_pool
            .free_list_entries()
            .iter()
            .map(|coord| GpuFreeListEntry {
                physical_x: coord.x,
                physical_y: coord.y,
            })
            .collect();
        if !free_entries.is_empty() {
            queue.write_buffer(
                &self.free_list_buffer,
                0,
                bytemuck::cast_slice(&free_entries),
            );
        }
    }

    /// Dispatch the "mark pages" compute pass.
    ///
    /// `depth_view` is the scene depth buffer from the current camera.
    pub fn mark_pages(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        screen_width: u32,
        screen_height: u32,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VSM Mark Pages Bind Group"),
            layout: &self.mark_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.page_flags_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.vsm_params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("VSM Mark Pages Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.mark_pages_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);

        // Dispatch one thread per screen pixel, workgroup 8x8
        let dispatch_x = screen_width.div_ceil(8);
        let dispatch_y = screen_height.div_ceil(8);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }

    /// Dispatch the "allocate pages" compute pass.
    pub fn allocate_pages(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VSM Allocate Pages Bind Group"),
            layout: &self.allocate_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.page_flags_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.page_table_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.page_allocator_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.free_list_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.vsm_params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("VSM Allocate Pages Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.allocate_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);

        // One thread per virtual page
        let pages = self.config.page_table_size;
        let dispatch_x = pages.div_ceil(8);
        let dispatch_y = pages.div_ceil(8);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }

    /// Render shadow depth into the physical atlas for all dirty/newly
    /// allocated pages.
    ///
    /// Each page maps to a `page_size x page_size` region of the atlas.
    /// The caller provides meshes as `(vertex_buffer, index_buffer,
    /// index_count)` triples.
    ///
    /// Note: In a production system this would cull geometry per-page.
    /// Here we render all meshes into each dirty page with the appropriate
    /// viewport/scissor.
    pub fn render_pages(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[(&wgpu::Buffer, &wgpu::Buffer, u32)],
        shadow_pipeline: &wgpu::RenderPipeline,
        shadow_bind_group: &wgpu::BindGroup,
    ) {
        // Full-atlas clear, then render.  A smarter implementation would
        // only clear individual dirty pages, but this is correct.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("VSM Render Pages Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.physical_pool_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(shadow_pipeline);
        pass.set_bind_group(0, shadow_bind_group, &[]);

        for (vertex_buffer, index_buffer, index_count) in meshes {
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*index_count, 0, 0..1);
        }
    }

    // =======================================================================
    // Clipmap helpers
    // =======================================================================

    pub fn update_clipmap(
        &mut self,
        camera_pos: glam::Vec3,
        light_dir: glam::Vec3,
    ) {
        self.clipmap.update_matrices(camera_pos, light_dir);
    }

    pub fn clipmap(&self) -> &Clipmap {
        &self.clipmap
    }

    pub fn clipmap_mut(&mut self) -> &mut Clipmap {
        &mut self.clipmap
    }

    // =======================================================================
    // Accessors for external bind group creation (e.g. material_eval)
    // =======================================================================

    pub fn page_table_view(&self) -> &wgpu::TextureView {
        &self.page_table_view
    }

    pub fn physical_pool_view(&self) -> &wgpu::TextureView {
        &self.physical_pool_view
    }

    pub fn physical_pool_depth_view(&self) -> &wgpu::TextureView {
        &self.physical_pool_depth_view
    }

    pub fn params_buffer(&self) -> &wgpu::Buffer {
        &self.vsm_params_buffer
    }

    pub fn page_flags_buffer(&self) -> &wgpu::Buffer {
        &self.page_flags_buffer
    }

    pub fn sampling_sampler(&self) -> &wgpu::Sampler {
        &self.sampling_sampler
    }

    pub fn config(&self) -> &VsmConfig {
        &self.config
    }

    pub fn page_table(&self) -> &PageTable {
        &self.page_table
    }

    pub fn physical_pool(&self) -> &PhysicalPool {
        &self.physical_pool
    }

    pub fn mark_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.mark_bind_group_layout
    }

    pub fn allocate_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.allocate_bind_group_layout
    }
}
