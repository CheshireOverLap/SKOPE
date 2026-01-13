// SKOPE Engine - Temporal Anti-Aliasing (TAA) Module
//
// Features:
// - Halton sequence jitter for camera subpixel offset
// - History buffer management (ping-pong)
// - Variance clipping for ghosting reduction
// - Motion vector integration (Phase 1.3)

use bytemuck::{Pod, Zeroable};

/// Halton sequence for 16 frames (base 2, 3)
const HALTON_SAMPLES: [(f32, f32); 16] = [
    (0.5, 0.333333),
    (0.25, 0.666667),
    (0.75, 0.111111),
    (0.125, 0.444444),
    (0.625, 0.777778),
    (0.375, 0.222222),
    (0.875, 0.555556),
    (0.0625, 0.888889),
    (0.5625, 0.037037),
    (0.3125, 0.370370),
    (0.8125, 0.703704),
    (0.1875, 0.148148),
    (0.6875, 0.481481),
    (0.4375, 0.814815),
    (0.9375, 0.259259),
    (0.03125, 0.592593),
];

/// TAA Parameters (matches WGSL struct)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TaaParams {
    pub screen_size: [f32; 2],
    pub inv_screen_size: [f32; 2],
    pub jitter_offset: [f32; 2],
    pub prev_jitter_offset: [f32; 2],
    pub blend_factor: f32,
    pub variance_clip_gamma: f32,
    pub motion_scale: f32,
    pub frame_index: u32,
}

impl Default for TaaParams {
    fn default() -> Self {
        Self {
            screen_size: [1920.0, 1080.0],
            inv_screen_size: [1.0 / 1920.0, 1.0 / 1080.0],
            jitter_offset: [0.0, 0.0],
            prev_jitter_offset: [0.0, 0.0],
            blend_factor: 0.9,              // 90% history
            variance_clip_gamma: 1.25,      // Moderate clipping
            motion_scale: 0.1,              // Motion sensitivity
            frame_index: 0,
        }
    }
}

/// Temporal Anti-Aliasing Pipeline
pub struct TaaPipeline {
    pub resolve_pipeline: wgpu::RenderPipeline,
    pub copy_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub linear_sampler: wgpu::Sampler,
    pub point_sampler: wgpu::Sampler,

    // Ping-pong history buffers
    pub history_textures: [wgpu::Texture; 2],
    pub history_views: [wgpu::TextureView; 2],
    pub current_history_index: usize,

    // Velocity buffer (will be filled by motion vector pass)
    pub velocity_texture: wgpu::Texture,
    pub velocity_view: wgpu::TextureView,

    // Output texture
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    // State
    pub width: u32,
    pub height: u32,
    pub frame_index: u32,
    pub prev_jitter: [f32; 2],
    pub enabled: bool,
}

impl TaaPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TAA Bind Group Layout"),
            entries: &[
                // Params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<TaaParams>() as u64),
                    },
                    count: None,
                },
                // Current color texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // History color texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Velocity texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Point sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TAA Params Buffer"),
            size: std::mem::size_of::<TaaParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Samplers
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TAA Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TAA Point Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // History textures (ping-pong)
        let history_textures = std::array::from_fn(|i| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("TAA History {}", i)),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        });

        let history_views = std::array::from_fn(|i| {
            history_textures[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        // Velocity texture (RG16Float for motion vectors)
        // Note: STORAGE_BINDING removed - Motion Vector pass will use RENDER_ATTACHMENT
        let velocity_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA Velocity"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let velocity_view = velocity_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TAA Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/taa.wgsl").into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TAA Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Resolve pipeline (main TAA)
        let resolve_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TAA Resolve Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Copy pipeline (for first frame)
        let copy_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TAA Copy Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_copy"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            resolve_pipeline,
            copy_pipeline,
            bind_group_layout,
            params_buffer,
            linear_sampler,
            point_sampler,
            history_textures,
            history_views,
            current_history_index: 0,
            velocity_texture,
            velocity_view,
            output_texture,
            output_view,
            width,
            height,
            frame_index: 0,
            prev_jitter: [0.0, 0.0],
            enabled: true,
        }
    }

    /// Get current frame's jitter offset in NDC space
    pub fn get_jitter(&self) -> [f32; 2] {
        if !self.enabled {
            return [0.0, 0.0];
        }

        let sample = HALTON_SAMPLES[(self.frame_index as usize) % HALTON_SAMPLES.len()];
        [
            (sample.0 - 0.5) * 2.0 / self.width as f32,
            (sample.1 - 0.5) * 2.0 / self.height as f32,
        ]
    }

    /// Apply jitter to projection matrix
    pub fn jitter_projection(&self, proj: glam::Mat4) -> glam::Mat4 {
        if !self.enabled {
            return proj;
        }

        let jitter = self.get_jitter();

        // Add subpixel offset to projection matrix
        // proj[2][0] and proj[2][1] control the NDC offset
        let cols = proj.to_cols_array_2d();
        let mut new_cols = cols;
        new_cols[2][0] += jitter[0];
        new_cols[2][1] += jitter[1];

        glam::Mat4::from_cols_array_2d(&new_cols)
    }

    /// Create bind group for TAA resolve
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        current_color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        // Read from previous history, write to current
        let history_read_index = 1 - self.current_history_index;

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(current_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.history_views[history_read_index]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.velocity_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.point_sampler),
                },
            ],
        })
    }

    /// Execute TAA resolve pass
    pub fn resolve(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        current_color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if !self.enabled {
            return;
        }

        // Update params
        let jitter = self.get_jitter();
        let params = TaaParams {
            screen_size: [self.width as f32, self.height as f32],
            inv_screen_size: [1.0 / self.width as f32, 1.0 / self.height as f32],
            jitter_offset: jitter,
            prev_jitter_offset: self.prev_jitter,
            blend_factor: 0.9,
            variance_clip_gamma: 1.25,
            motion_scale: 0.1,
            frame_index: self.frame_index,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group
        let bind_group = self.create_bind_group(device, current_color_view, depth_view);

        // Get write target (current history becomes next frame's history)
        let history_write_view = &self.history_views[self.current_history_index];

        // Run TAA resolve
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TAA Resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: history_write_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if self.frame_index == 0 {
                // First frame - just copy
                pass.set_pipeline(&self.copy_pipeline);
            } else {
                // Normal TAA resolve
                pass.set_pipeline(&self.resolve_pipeline);
            }

            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // Copy result to output
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.history_textures[self.current_history_index],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        // Swap history buffers
        self.prev_jitter = jitter;
        self.current_history_index = 1 - self.current_history_index;
        self.frame_index += 1;
    }

    /// Resize textures when window size changes
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }

        self.width = width;
        self.height = height;
        self.frame_index = 0; // Reset to first frame

        // Recreate all textures
        for i in 0..2 {
            self.history_textures[i] = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("TAA History {}", i)),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.history_views[i] = self.history_textures[i]
                .create_view(&wgpu::TextureViewDescriptor::default());
        }

        self.velocity_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA Velocity"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.velocity_view = self.velocity_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.output_view = self.output_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        log::info!("[TAA] Resized to {}x{}", width, height);
    }

    /// Toggle TAA on/off
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.frame_index = 0; // Reset on re-enable
        }
    }
}
