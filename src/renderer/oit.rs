// SKOPE Engine - Order-Independent Transparency (OIT)
//
// V-Bufferized OIT using Per-Pixel Linked Lists
// For structural transparency: glass, water, magic barriers
//
// Reference: "Real-Time Rendering of Transparent Objects" (McGuire, Mara 2017)

use wgpu;
use bytemuck::{Pod, Zeroable};

/// Maximum nodes per pixel (overflow handled by discarding furthest)
pub const MAX_NODES_PER_PIXEL: u32 = 8;

/// Maximum total nodes (based on resolution and average fragment count)
/// For 1080p with 4 average fragments: 1920 * 1080 * 4 = ~8.3M nodes
pub const DEFAULT_MAX_NODES: u32 = 8 * 1024 * 1024;

/// OIT Node stored in the linked list
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OitNode {
    /// Packed color (RGBA8)
    pub color: u32,

    /// Depth value (0-1, near to far)
    pub depth: f32,

    /// Index of next node in the list (0xFFFFFFFF = end)
    pub next: u32,

    /// Triangle ID for V-Buffer lookup
    pub triangle_id: u32,
}

/// OIT Uniforms
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OitParams {
    pub screen_width: u32,
    pub screen_height: u32,
    pub max_nodes: u32,
    pub _pad: u32,
}

/// OIT Pipeline for V-Bufferized transparency
pub struct OitPipeline {
    /// Per-pixel head buffer (stores first node index, 0xFFFFFFFF = empty)
    pub head_buffer: wgpu::Buffer,
    pub head_view: wgpu::TextureView,

    /// Linked list node buffer
    pub node_buffer: wgpu::Buffer,

    /// Atomic counter for node allocation
    pub counter_buffer: wgpu::Buffer,

    /// Params uniform buffer
    pub params_buffer: wgpu::Buffer,

    /// Build pipeline (adds fragments to linked list)
    pub build_pipeline: wgpu::RenderPipeline,
    pub build_bind_group_layout: wgpu::BindGroupLayout,

    /// Resolve pipeline (sorts and composites)
    pub resolve_pipeline: wgpu::ComputePipeline,
    pub resolve_bind_group_layout: wgpu::BindGroupLayout,

    width: u32,
    height: u32,
    max_nodes: u32,
}

impl OitPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let pixel_count = width * height;
        let max_nodes = (pixel_count * 4).min(DEFAULT_MAX_NODES);

        // Head buffer: 1 u32 per pixel
        // Using a texture for better cache coherency
        let head_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("OIT Head Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let head_view = head_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Head buffer (also keep as buffer for atomic operations)
        let head_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OIT Head Buffer"),
            size: (pixel_count * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Node buffer: OitNode per node
        let node_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OIT Node Buffer"),
            size: (max_nodes as u64) * std::mem::size_of::<OitNode>() as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Atomic counter
        let counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OIT Counter Buffer"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("OIT Params Buffer"),
            size: std::mem::size_of::<OitParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Build bind group layout
        let build_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OIT Build Bind Group Layout"),
            entries: &[
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Head buffer (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Node buffer (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Counter buffer (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Resolve bind group layout
        let resolve_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OIT Resolve Bind Group Layout"),
            entries: &[
                // Params
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
                // Head buffer (read-only)
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
                // Node buffer (read-only)
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
                // Output texture (write)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Background texture (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Create pipelines
        let build_pipeline = Self::create_build_pipeline(device, &build_bind_group_layout);
        let resolve_pipeline = Self::create_resolve_pipeline(device, &resolve_bind_group_layout);

        log::info!(
            "[OIT] Initialized: {}x{}, max {} nodes ({:.1} MB)",
            width, height, max_nodes,
            (max_nodes as f64 * std::mem::size_of::<OitNode>() as f64) / (1024.0 * 1024.0)
        );

        Self {
            head_buffer,
            head_view,
            node_buffer,
            counter_buffer,
            params_buffer,
            build_pipeline,
            build_bind_group_layout,
            resolve_pipeline,
            resolve_bind_group_layout,
            width,
            height,
            max_nodes,
        }
    }

    fn create_build_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("OIT Build Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/oit_build.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("OIT Build Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("OIT Build Pipeline"),
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
                targets: &[],  // No color output, just writes to storage
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,  // Don't write depth for transparent
                depth_compare: wgpu::CompareFunction::Less,  // But do test against opaque
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn create_resolve_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("OIT Resolve Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/oit_resolve.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("OIT Resolve Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("OIT Resolve Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Clear buffers before rendering transparent objects
    pub fn clear(&self, queue: &wgpu::Queue) {
        // Clear head buffer to 0xFFFFFFFF (null pointer)
        let clear_data = vec![0xFFFFFFFFu32; (self.width * self.height) as usize];
        queue.write_buffer(&self.head_buffer, 0, bytemuck::cast_slice(&clear_data));

        // Reset counter to 0
        queue.write_buffer(&self.counter_buffer, 0, bytemuck::bytes_of(&0u32));

        // Update params
        let params = OitParams {
            screen_width: self.width,
            screen_height: self.height,
            max_nodes: self.max_nodes,
            _pad: 0,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Resize OIT buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        *self = Self::new(device, width, height);
    }

    /// Create build bind group for rendering
    pub fn create_build_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OIT Build Bind Group"),
            layout: &self.build_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.head_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.node_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.counter_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create resolve bind group for compositing
    pub fn create_resolve_bind_group(
        &self,
        device: &wgpu::Device,
        output_view: &wgpu::TextureView,
        background_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OIT Resolve Bind Group"),
            layout: &self.resolve_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.head_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.node_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(background_view),
                },
            ],
        })
    }

    /// Resolve (sort and composite) transparent fragments
    pub fn resolve(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        background_view: &wgpu::TextureView,
    ) {
        let bind_group = self.create_resolve_bind_group(device, output_view, background_view);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("OIT Resolve Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.resolve_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);

        let workgroups_x = (self.width + 7) / 8;
        let workgroups_y = (self.height + 7) / 8;
        pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }
}
