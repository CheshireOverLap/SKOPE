//! Surface Cache Pipeline
//!
//! Caches off-screen geometry lighting info as Card-based atlas textures.
//! Used as fallback by radiance cache and reflections for off-screen GI.
//!
//! Each mesh is decomposed into oriented rectangles (SurfaceCards) that
//! parameterise a patch of surface. Cards are rendered into atlas pages
//! (128x128 texels each) storing albedo, normal, emissive and depth.
//! Pages are updated incrementally across frames to stay within budget.

#[cfg(feature = "gpu")]
use crate::types::{SurfaceCard, SurfaceCachePage, SurfaceCacheConfig};

/// Parameters for the surface cache capture compute shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CaptureParams {
    pub update_start: u32,
    pub update_count: u32,
    pub atlas_resolution: u32,
    pub page_size: u32,
    pub frame_index: u32,
    pub _pad: [u32; 3],
}

/// GPU pipeline for capturing card data into the surface cache atlas.
#[cfg(feature = "gpu")]
pub struct SurfaceCachePipeline {
    pub config: SurfaceCacheConfig,

    // Card and page management
    pub card_buffer: wgpu::Buffer,
    pub page_table: wgpu::Buffer,
    pub free_page_list: wgpu::Buffer,
    pub card_count: u32,
    pub allocated_pages: u32,

    // Atlas textures (2048x2048)
    pub albedo_atlas: wgpu::Texture,
    pub albedo_atlas_view: wgpu::TextureView,
    pub normal_atlas: wgpu::Texture,
    pub normal_atlas_view: wgpu::TextureView,
    pub emissive_atlas: wgpu::Texture,
    pub emissive_atlas_view: wgpu::TextureView,
    pub depth_atlas: wgpu::Texture,
    pub depth_atlas_view: wgpu::TextureView,

    // Compute pipelines
    pub capture_pipeline: wgpu::ComputePipeline,
    pub capture_layout: wgpu::BindGroupLayout,

    // Bind group layouts (reused each frame)
    pub params_layout: wgpu::BindGroupLayout,
    pub atlas_layout: wgpu::BindGroupLayout,
    pub sdf_layout: wgpu::BindGroupLayout,

    // Params
    pub params_buffer: wgpu::Buffer,

    // Dirty tracking
    dirty_pages: Vec<u32>,
    dirty_page_buffer: wgpu::Buffer,
    frame_index: u32,

    // CPU-side card origin cache for distance-based scheduling
    card_origins: Vec<[f32; 3]>,
}

#[cfg(feature = "gpu")]
impl SurfaceCachePipeline {
    /// Create all GPU resources for the surface cache.
    pub fn new(device: &wgpu::Device, config: SurfaceCacheConfig) -> Self {
        let res = config.atlas_resolution;

        // ------------------------------------------------------------------
        // Buffers
        // ------------------------------------------------------------------

        let card_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Surface Cache Cards"),
            size: std::mem::size_of::<SurfaceCard>() as u64 * config.max_cards as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let page_table = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Surface Cache Page Table"),
            size: std::mem::size_of::<SurfaceCachePage>() as u64 * config.max_pages as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let free_page_list = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Surface Cache Free Page List"),
            size: std::mem::size_of::<u32>() as u64 * config.max_pages as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Surface Cache Capture Params"),
            size: std::mem::size_of::<CaptureParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Buffer to hold the indices of dirty pages selected for update each frame.
        // Sized to the per-frame budget so the GPU knows exactly which pages to process.
        let dirty_page_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Surface Cache Dirty Page Indices"),
            size: std::mem::size_of::<u32>() as u64 * config.update_budget_per_frame as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ------------------------------------------------------------------
        // Atlas textures
        // ------------------------------------------------------------------

        let create_atlas = |label: &str, format: wgpu::TextureFormat| -> (wgpu::Texture, wgpu::TextureView) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: res,
                    height: res,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };

        let (albedo_atlas, albedo_atlas_view) =
            create_atlas("Surface Cache Albedo Atlas", wgpu::TextureFormat::Rgba8Unorm);
        let (normal_atlas, normal_atlas_view) =
            create_atlas("Surface Cache Normal Atlas", wgpu::TextureFormat::Rgba8Snorm);
        let (emissive_atlas, emissive_atlas_view) =
            create_atlas("Surface Cache Emissive Atlas", wgpu::TextureFormat::Rgba16Float);
        let (depth_atlas, depth_atlas_view) =
            create_atlas("Surface Cache Depth Atlas", wgpu::TextureFormat::R32Float);

        // ------------------------------------------------------------------
        // Bind group layouts
        // ------------------------------------------------------------------

        // G0: Params uniform
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Surface Cache Params Layout"),
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

        // G1: Card buffer (read) + Page table (read)
        let card_page_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Surface Cache Card/Page Layout"),
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

        // G2: Atlas textures (4 storage textures, write-only)
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Surface Cache Atlas Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Snorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
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

        // G3: SDF volume for geometry queries
        let sdf_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Surface Cache SDF Layout"),
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
                // SDF volume (R32Float is non-filterable)
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // ------------------------------------------------------------------
        // Capture pipeline (card -> atlas)
        // ------------------------------------------------------------------

        let capture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Surface Cache Capture Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_surface_cache_capture.wgsl").into(),
            ),
        });

        let capture_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Surface Cache Capture Pipeline Layout"),
            bind_group_layouts: &[&params_layout, &card_page_layout, &atlas_layout, &sdf_layout],
            immediate_size: 0,
        });

        let capture_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Surface Cache Capture"),
            layout: Some(&capture_pipe_layout),
            module: &capture_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });


        Self {
            config,
            card_buffer,
            page_table,
            free_page_list,
            card_count: 0,
            allocated_pages: 0,
            albedo_atlas,
            albedo_atlas_view,
            normal_atlas,
            normal_atlas_view,
            emissive_atlas,
            emissive_atlas_view,
            depth_atlas,
            depth_atlas_view,
            capture_pipeline,
            capture_layout: card_page_layout,
            params_layout,
            atlas_layout,
            sdf_layout,
            params_buffer,
            dirty_pages: Vec::new(),
            dirty_page_buffer,
            frame_index: 0,
            card_origins: Vec::new(),
        }
    }

    /// Upload card data and allocate pages from the free list.
    ///
    /// Cards arriving here already have atlas offsets set by the mesh
    /// decomposition step. This method writes them into the GPU buffer
    /// and allocates corresponding pages.
    pub fn register_mesh_cards(&mut self, queue: &wgpu::Queue, cards: &[SurfaceCard]) {
        if cards.is_empty() {
            return;
        }

        let max = self.config.max_cards as usize;
        let start = self.card_count as usize;
        let count = cards.len().min(max.saturating_sub(start));
        if count == 0 {
            log::warn!("Surface cache card buffer full ({} cards)", max);
            return;
        }

        let slice = &cards[..count];

        queue.write_buffer(
            &self.card_buffer,
            (start * std::mem::size_of::<SurfaceCard>()) as u64,
            bytemuck::cast_slice(slice),
        );

        // Allocate one page per card (simple 1:1 mapping for now).
        let pages_per_axis = self.config.atlas_resolution / self.config.page_size;
        let mut pages = Vec::with_capacity(count);

        for (i, _card) in slice.iter().enumerate() {
            let page_idx = self.allocated_pages;
            if page_idx >= self.config.max_pages {
                log::warn!("Surface cache page table full ({} pages)", self.config.max_pages);
                break;
            }

            let page_x = page_idx % pages_per_axis;
            let page_y = page_idx / pages_per_axis;

            pages.push(SurfaceCachePage {
                card_index: (start + i) as u32,
                atlas_x: page_x * self.config.page_size,
                atlas_y: page_y * self.config.page_size,
                last_update_frame: 0,
            });

            self.dirty_pages.push(page_idx);
            self.allocated_pages += 1;
        }

        if !pages.is_empty() {
            let offset = start * std::mem::size_of::<SurfaceCachePage>();
            queue.write_buffer(
                &self.page_table,
                offset as u64,
                bytemuck::cast_slice(&pages),
            );
        }

        // Cache card origins for distance-based scheduling
        for card in slice {
            self.card_origins.push(card.origin);
        }

        self.card_count += count as u32;
    }

    /// Select dirty pages to update this frame within the per-frame budget.
    ///
    /// Pages closer to `camera_pos` are prioritised; stale pages are
    /// also bumped in priority so distant pages still eventually refresh.
    pub fn schedule_updates(&mut self, camera_pos: [f32; 3], frame_index: u32) -> Vec<u32> {
        self.frame_index = frame_index;

        if self.dirty_pages.is_empty() {
            return Vec::new();
        }

        let budget = self.config.update_budget_per_frame as usize;

        // Sort dirty pages by distance to camera (closer = higher priority).
        // Uses CPU-side card origin cache (1:1 page-to-card mapping).
        let cam = camera_pos;
        let origins = &self.card_origins;
        self.dirty_pages.sort_unstable_by(|&a, &b| {
            let dist_a = if (a as usize) < origins.len() {
                let o = origins[a as usize];
                let dx = o[0] - cam[0];
                let dy = o[1] - cam[1];
                let dz = o[2] - cam[2];
                dx * dx + dy * dy + dz * dz
            } else {
                f32::MAX
            };
            let dist_b = if (b as usize) < origins.len() {
                let o = origins[b as usize];
                let dx = o[0] - cam[0];
                let dy = o[1] - cam[1];
                let dz = o[2] - cam[2];
                dx * dx + dy * dy + dz * dz
            } else {
                f32::MAX
            };
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let count = self.dirty_pages.len().min(budget);
        let selected: Vec<u32> = self.dirty_pages.drain(..count).collect();

        selected
    }

    /// Dispatch the capture compute shader to write card data into atlas pages.
    ///
    /// `page_indices` contains the (potentially non-contiguous) page indices to
    /// update this frame.  They are uploaded to `dirty_page_buffer` so the GPU
    /// shader can look up the actual page for each workgroup invocation.
    pub fn capture_cards(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        page_indices: &[u32],
        sdf_params_buf: &wgpu::Buffer,
        sdf_view: &wgpu::TextureView,
        sdf_sampler: &wgpu::Sampler,
    ) {
        if page_indices.is_empty() {
            return;
        }

        let update_count = page_indices.len() as u32;

        // Upload dirty page indices so the GPU shader can index into them.
        queue.write_buffer(
            &self.dirty_page_buffer,
            0,
            bytemuck::cast_slice(page_indices),
        );

        let params = CaptureParams {
            update_start: 0,
            update_count,
            atlas_resolution: self.config.atlas_resolution,
            page_size: self.config.page_size,
            frame_index: self.frame_index,
            _pad: [0; 3],
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // G0: params
        let params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Surface Cache Capture Params BG"),
            layout: &self.params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.params_buffer.as_entire_binding(),
            }],
        });

        // G1: cards + pages
        let card_page_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Surface Cache Card/Page BG"),
            layout: &self.capture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.card_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.page_table.as_entire_binding(),
                },
            ],
        });

        // G2: atlas textures
        let atlas_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Surface Cache Atlas BG"),
            layout: &self.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.albedo_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.normal_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.emissive_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.depth_atlas_view),
                },
            ],
        });

        // G3: SDF volume
        let sdf_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Surface Cache SDF BG"),
            layout: &self.sdf_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sdf_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sdf_sampler),
                },
            ],
        });

        // Each workgroup handles one page (8x8 threads looping over 128x128 texels).
        // Dispatch one workgroup per page being updated.
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Surface Cache Capture"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.capture_pipeline);
        pass.set_bind_group(0, &params_bg, &[]);
        pass.set_bind_group(1, &card_page_bg, &[]);
        pass.set_bind_group(2, &atlas_bg, &[]);
        pass.set_bind_group(3, &sdf_bg, &[]);
        pass.dispatch_workgroups(update_count, 1, 1);
    }

    /// Return references to the atlas texture views (albedo, normal, emissive, depth).
    pub fn atlas_views(
        &self,
    ) -> (
        &wgpu::TextureView,
        &wgpu::TextureView,
        &wgpu::TextureView,
        &wgpu::TextureView,
    ) {
        (
            &self.albedo_atlas_view,
            &self.normal_atlas_view,
            &self.emissive_atlas_view,
            &self.depth_atlas_view,
        )
    }
}
