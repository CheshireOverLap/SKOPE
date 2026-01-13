// SKOPE Engine - Shadow Atlas System
//
// Combines multiple local light shadows into a single texture atlas
// for efficient sampling and reduced texture switches.
//
// Features:
// - Dynamic tile allocation based on screen-space importance
// - Variable tile sizes (256, 512, 1024, 2048)
// - Point lights use 6 tiles (cubemap faces)
// - Spot lights use 1 tile
// - LRU-style eviction for inactive lights
//
// Reference: "Practical Techniques for Dynamic Shadow Maps" (GDC 2015)

use glam::{Vec3, Mat4};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

/// Shadow Atlas Configuration
#[derive(Debug, Clone, Copy)]
pub struct ShadowAtlasConfig {
    /// Total atlas size (e.g., 4096x4096)
    pub atlas_size: u32,
    /// Minimum tile size (smallest shadow map)
    pub min_tile_size: u32,
    /// Maximum tile size (largest shadow map)
    pub max_tile_size: u32,
    /// Maximum number of shadow-casting lights
    pub max_lights: u32,
    /// Depth bias for shadow mapping
    pub depth_bias: f32,
    /// Normal bias for shadow mapping
    pub normal_bias: f32,
}

impl Default for ShadowAtlasConfig {
    fn default() -> Self {
        Self {
            atlas_size: 4096,
            min_tile_size: 256,
            max_tile_size: 2048,
            max_lights: 32,
            depth_bias: 0.001,
            normal_bias: 0.02,
        }
    }
}

/// Tile allocation result
#[derive(Debug, Clone, Copy)]
pub struct TileAllocation {
    /// X offset in atlas (pixels)
    pub x: u32,
    /// Y offset in atlas (pixels)
    pub y: u32,
    /// Tile size (width = height)
    pub size: u32,
    /// Tile index for shader lookup
    pub tile_index: u32,
}

/// Light Shadow Info (GPU-side)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShadowLightData {
    /// Light view-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// Atlas UV offset and scale: (u_offset, v_offset, u_scale, v_scale)
    pub atlas_uv: [f32; 4],
    /// Light position (for point light distance calculation)
    pub position: [f32; 4],
    /// Near/far planes, bias, light type
    pub params: [f32; 4],
}

impl ShadowLightData {
    pub fn new(
        view_proj: Mat4,
        tile: &TileAllocation,
        atlas_size: u32,
        position: Vec3,
        near: f32,
        far: f32,
        light_type: u32,
    ) -> Self {
        let atlas_size_f = atlas_size as f32;
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            atlas_uv: [
                tile.x as f32 / atlas_size_f,
                tile.y as f32 / atlas_size_f,
                tile.size as f32 / atlas_size_f,
                tile.size as f32 / atlas_size_f,
            ],
            position: [position.x, position.y, position.z, 1.0],
            params: [near, far, 0.001, light_type as f32],
        }
    }
}

/// Point Light Cubemap Shadow Info
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PointShadowData {
    /// 6 face view-projection matrices
    pub face_view_proj: [[[f32; 4]; 4]; 6],
    /// 6 atlas UV regions (one per face)
    pub face_atlas_uv: [[f32; 4]; 6],
    /// Light position
    pub position: [f32; 4],
    /// Near, far, bias, radius
    pub params: [f32; 4],
}

/// Shadow Atlas Tile Allocator
struct TileAllocator {
    atlas_size: u32,
    min_tile_size: u32,
    /// Occupancy grid (for each min_tile_size block)
    occupancy: Vec<bool>,
    grid_size: u32,
}

impl TileAllocator {
    fn new(atlas_size: u32, min_tile_size: u32) -> Self {
        let grid_size = atlas_size / min_tile_size;
        let total_cells = (grid_size * grid_size) as usize;
        Self {
            atlas_size,
            min_tile_size,
            occupancy: vec![false; total_cells],
            grid_size,
        }
    }

    fn allocate(&mut self, size: u32) -> Option<TileAllocation> {
        let tiles_needed = size / self.min_tile_size;

        // Simple first-fit allocation
        for gy in 0..(self.grid_size - tiles_needed + 1) {
            for gx in 0..(self.grid_size - tiles_needed + 1) {
                if self.can_allocate(gx, gy, tiles_needed) {
                    self.mark_occupied(gx, gy, tiles_needed);
                    return Some(TileAllocation {
                        x: gx * self.min_tile_size,
                        y: gy * self.min_tile_size,
                        size,
                        tile_index: gy * self.grid_size + gx,
                    });
                }
            }
        }
        None
    }

    fn can_allocate(&self, gx: u32, gy: u32, tiles_needed: u32) -> bool {
        for dy in 0..tiles_needed {
            for dx in 0..tiles_needed {
                let idx = ((gy + dy) * self.grid_size + (gx + dx)) as usize;
                if self.occupancy[idx] {
                    return false;
                }
            }
        }
        true
    }

    fn mark_occupied(&mut self, gx: u32, gy: u32, tiles_needed: u32) {
        for dy in 0..tiles_needed {
            for dx in 0..tiles_needed {
                let idx = ((gy + dy) * self.grid_size + (gx + dx)) as usize;
                self.occupancy[idx] = true;
            }
        }
    }

    fn free(&mut self, tile: &TileAllocation) {
        let gx = tile.x / self.min_tile_size;
        let gy = tile.y / self.min_tile_size;
        let tiles_needed = tile.size / self.min_tile_size;

        for dy in 0..tiles_needed {
            for dx in 0..tiles_needed {
                let idx = ((gy + dy) * self.grid_size + (gx + dx)) as usize;
                self.occupancy[idx] = false;
            }
        }
    }

    fn reset(&mut self) {
        self.occupancy.fill(false);
    }
}

/// Light identifier for tracking allocations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightId {
    Point(u32),
    Spot(u32),
}

/// Per-light allocation tracking
struct LightAllocation {
    tiles: Vec<TileAllocation>,
    frame_last_used: u64,
    importance: f32,
}

/// Shadow Atlas Pipeline
pub struct ShadowAtlas {
    config: ShadowAtlasConfig,
    /// Shadow atlas texture (Depth32Float)
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    /// Allocator for managing tiles
    allocator: TileAllocator,
    /// Per-light allocations
    allocations: HashMap<LightId, LightAllocation>,
    /// Shadow light data buffer (for shader access)
    light_data_buffer: wgpu::Buffer,
    /// Point shadow data buffer
    point_data_buffer: wgpu::Buffer,
    /// Uniform buffer for atlas params
    params_buffer: wgpu::Buffer,
    /// Bind group layout for sampling
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    /// Depth-only render pipeline
    depth_pipeline: wgpu::RenderPipeline,
    depth_bind_group_layout: wgpu::BindGroupLayout,
    /// Model transform buffer
    model_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
    /// Sampler (comparison)
    sampler: wgpu::Sampler,
    /// Current frame number
    frame: u64,
    /// Active spot lights this frame
    active_spot_count: u32,
    /// Active point lights this frame
    active_point_count: u32,
}

/// Atlas Params (GPU uniform)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct AtlasParams {
    atlas_size: u32,
    depth_bias: f32,
    normal_bias: f32,
    spot_count: u32,
    point_count: u32,
    _pad: [u32; 3],
}

/// Model Uniform (per-draw)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ShadowModelUniform {
    model: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
}

impl ShadowAtlas {
    pub fn new(device: &wgpu::Device, config: ShadowAtlasConfig) -> Self {
        // Create atlas texture
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Atlas"),
            size: wgpu::Extent3d {
                width: config.atlas_size,
                height: config.atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Comparison sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // Buffers
        let light_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Light Data Buffer"),
            size: (config.max_lights as usize * std::mem::size_of::<ShadowLightData>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Shadow Data Buffer"),
            size: (config.max_lights as usize * std::mem::size_of::<PointShadowData>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Atlas Params"),
            size: std::mem::size_of::<AtlasParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Model Buffer"),
            size: std::mem::size_of::<ShadowModelUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout for sampling
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Atlas Bind Group Layout"),
            entries: &[
                // Atlas texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                // Spot light shadow data
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Point light shadow data
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Atlas Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: point_data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        // Depth pipeline for rendering shadow maps
        let (depth_pipeline, depth_bind_group_layout, model_bind_group) =
            Self::create_depth_pipeline(device, &model_buffer);

        let allocator = TileAllocator::new(config.atlas_size, config.min_tile_size);

        log::info!(
            "[ShadowAtlas] Initialized: {}x{}, max {} lights",
            config.atlas_size, config.atlas_size, config.max_lights
        );

        Self {
            config,
            atlas_texture,
            atlas_view,
            allocator,
            allocations: HashMap::new(),
            light_data_buffer,
            point_data_buffer,
            params_buffer,
            bind_group_layout,
            bind_group,
            depth_pipeline,
            depth_bind_group_layout,
            model_buffer,
            model_bind_group,
            sampler,
            frame: 0,
            active_spot_count: 0,
            active_point_count: 0,
        }
    }

    fn create_depth_pipeline(
        device: &wgpu::Device,
        model_buffer: &wgpu::Buffer,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout, wgpu::BindGroup) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Atlas Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow_atlas_depth.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Atlas Depth Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Atlas Depth Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Atlas Depth Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Atlas Depth Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 48,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        (pipeline, bind_group_layout, bind_group)
    }

    /// Begin a new frame - reset allocations
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.active_spot_count = 0;
        self.active_point_count = 0;
        // Reset allocator for this frame
        self.allocator.reset();
        self.allocations.clear();
    }

    /// Calculate tile size based on screen-space importance
    pub fn calculate_tile_size(&self, importance: f32) -> u32 {
        // importance: 0.0 (far/small) to 1.0 (near/large)
        let t = importance.clamp(0.0, 1.0);
        let size_levels = [
            self.config.min_tile_size,
            self.config.min_tile_size * 2,
            self.config.min_tile_size * 4,
            self.config.max_tile_size,
        ];

        let idx = (t * 3.0).floor() as usize;
        size_levels[idx.min(3)]
    }

    /// Allocate shadow tile for a spot light
    pub fn allocate_spot_light(
        &mut self,
        light_id: u32,
        importance: f32,
    ) -> Option<TileAllocation> {
        let tile_size = self.calculate_tile_size(importance);

        if let Some(tile) = self.allocator.allocate(tile_size) {
            self.allocations.insert(
                LightId::Spot(light_id),
                LightAllocation {
                    tiles: vec![tile],
                    frame_last_used: self.frame,
                    importance,
                },
            );
            Some(tile)
        } else {
            None
        }
    }

    /// Allocate shadow tiles for a point light (6 cubemap faces)
    pub fn allocate_point_light(
        &mut self,
        light_id: u32,
        importance: f32,
    ) -> Option<[TileAllocation; 6]> {
        let tile_size = self.calculate_tile_size(importance);

        let mut tiles = Vec::with_capacity(6);
        for _ in 0..6 {
            if let Some(tile) = self.allocator.allocate(tile_size) {
                tiles.push(tile);
            } else {
                // Failed to allocate all faces, free what we got
                for tile in &tiles {
                    self.allocator.free(tile);
                }
                return None;
            }
        }

        let tiles_array: [TileAllocation; 6] = tiles.try_into().ok()?;
        self.allocations.insert(
            LightId::Point(light_id),
            LightAllocation {
                tiles: tiles_array.to_vec(),
                frame_last_used: self.frame,
                importance,
            },
        );

        Some(tiles_array)
    }

    /// Update GPU buffers with shadow data
    pub fn update_buffers(
        &mut self,
        queue: &wgpu::Queue,
        spot_shadows: &[ShadowLightData],
        point_shadows: &[PointShadowData],
    ) {
        self.active_spot_count = spot_shadows.len() as u32;
        self.active_point_count = point_shadows.len() as u32;

        // Update spot light data
        if !spot_shadows.is_empty() {
            queue.write_buffer(
                &self.light_data_buffer,
                0,
                bytemuck::cast_slice(spot_shadows),
            );
        }

        // Update point light data
        if !point_shadows.is_empty() {
            queue.write_buffer(
                &self.point_data_buffer,
                0,
                bytemuck::cast_slice(point_shadows),
            );
        }

        // Update params
        let params = AtlasParams {
            atlas_size: self.config.atlas_size,
            depth_bias: self.config.depth_bias,
            normal_bias: self.config.normal_bias,
            spot_count: self.active_spot_count,
            point_count: self.active_point_count,
            _pad: [0; 3],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Render shadow map for a single tile
    pub fn render_tile(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        tile: &TileAllocation,
        view_proj: Mat4,
        meshes: &[(Mat4, &wgpu::Buffer, &wgpu::Buffer, u32)],
    ) {
        for (mesh_idx, (model_matrix, vertex_buffer, index_buffer, index_count)) in meshes.iter().enumerate() {
            let uniform = ShadowModelUniform {
                model: model_matrix.to_cols_array_2d(),
                view_proj: view_proj.to_cols_array_2d(),
            };
            queue.write_buffer(&self.model_buffer, 0, bytemuck::bytes_of(&uniform));

            let load_op = if mesh_idx == 0 {
                wgpu::LoadOp::Clear(1.0)
            } else {
                wgpu::LoadOp::Load
            };

            // Create a view for just this tile region
            // Note: wgpu doesn't support render pass scissor easily, so we use the full atlas
            // and rely on proper viewport setup
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Atlas Tile Render"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.atlas_view,
                    depth_ops: Some(wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_viewport(
                tile.x as f32,
                tile.y as f32,
                tile.size as f32,
                tile.size as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(tile.x, tile.y, tile.size, tile.size);
            pass.set_pipeline(&self.depth_pipeline);
            pass.set_bind_group(0, &self.model_bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*index_count, 0, 0..1);
        }
    }

    /// Clear the entire atlas
    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder) {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Atlas Clear"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.atlas_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        // _pass drops here, completing the clear operation
    }

    pub fn atlas_view(&self) -> &wgpu::TextureView {
        &self.atlas_view
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn config(&self) -> &ShadowAtlasConfig {
        &self.config
    }

    pub fn active_spot_count(&self) -> u32 {
        self.active_spot_count
    }

    pub fn active_point_count(&self) -> u32 {
        self.active_point_count
    }
}

/// Calculate spot light view-projection matrix
pub fn spot_light_view_proj(
    position: Vec3,
    direction: Vec3,
    outer_angle: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let fov = outer_angle * 2.0;
    let proj = Mat4::perspective_rh(fov.min(std::f32::consts::PI * 0.99), 1.0, near, far);
    let up = if direction.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let view = Mat4::look_at_rh(position, position + direction, up);
    proj * view
}

/// Calculate point light face view-projection matrices
pub fn point_light_face_matrices(position: Vec3, near: f32, far: f32) -> [Mat4; 6] {
    let proj = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_2,
        1.0,
        near,
        far,
    );

    let views = [
        Mat4::look_at_rh(position, position + Vec3::X, -Vec3::Y),   // +X
        Mat4::look_at_rh(position, position - Vec3::X, -Vec3::Y),   // -X
        Mat4::look_at_rh(position, position + Vec3::Y, Vec3::Z),    // +Y
        Mat4::look_at_rh(position, position - Vec3::Y, -Vec3::Z),   // -Y
        Mat4::look_at_rh(position, position + Vec3::Z, -Vec3::Y),   // +Z
        Mat4::look_at_rh(position, position - Vec3::Z, -Vec3::Y),   // -Z
    ];

    [
        proj * views[0],
        proj * views[1],
        proj * views[2],
        proj * views[3],
        proj * views[4],
        proj * views[5],
    ]
}

/// Calculate screen-space importance for a light
pub fn calculate_light_importance(
    light_pos: Vec3,
    light_radius: f32,
    camera_pos: Vec3,
    screen_height: f32,
    proj_scale: f32,
) -> f32 {
    let distance = (light_pos - camera_pos).length();
    if distance < 0.001 {
        return 1.0;
    }

    let screen_radius = (light_radius / distance) * proj_scale * screen_height * 0.5;
    let coverage = (screen_radius * 2.0) / screen_height;
    coverage.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_allocator() {
        let mut alloc = TileAllocator::new(1024, 256);

        // Should be able to allocate 16 tiles of 256x256
        let mut tiles = Vec::new();
        for _ in 0..16 {
            let tile = alloc.allocate(256);
            assert!(tile.is_some());
            tiles.push(tile.unwrap());
        }

        // 17th should fail
        assert!(alloc.allocate(256).is_none());

        // Free one and try again
        alloc.free(&tiles[0]);
        assert!(alloc.allocate(256).is_some());
    }

    // Note: ShadowAtlas requires GPU device for proper initialization
    // Cannot safely zero-initialize due to wgpu types
}
