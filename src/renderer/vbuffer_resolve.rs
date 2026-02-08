// SKOPE Engine - V-Buffer Resolve Pipeline
//
// Merges Nanite (Gambit) V-Buffer output with standard V-Buffer.
// The resolve is a compute pass that compares depths and writes
// the closer fragment per pixel to the merged output.
//
// The merged triangle_id uses bit 31 to distinguish Nanite entries:
//   bit 31 = 0 → Standard mesh (mesh_index[16] | primitive_index[16])
//   bit 31 = 1 → Nanite mesh (cluster_id[20] | triangle_id[7] | material_id[5])

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};

/// Flag bit set in merged triangle_id to mark Nanite geometry.
pub const NANITE_FLAG: u32 = 0x8000_0000;

/// Resolve parameters uniform.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ResolveParams {
    pub width: u32,
    pub height: u32,
    pub enable_nanite: u32,
    pub _pad: u32,
}

/// V-Buffer resolve pipeline: merges Nanite + Standard V-Buffers.
pub struct VBufferResolvePipeline {
    pipeline: wgpu::ComputePipeline,
    std_layout: wgpu::BindGroupLayout,    // Group 0: Standard V-Buffer (read)
    nanite_layout: wgpu::BindGroupLayout, // Group 1: Nanite V-Buffer (read)
    output_layout: wgpu::BindGroupLayout, // Group 2: Merged output (write)
    params_buffer: wgpu::Buffer,

    // Merged output textures
    pub merged_triangle_id: wgpu::Texture,
    pub merged_triangle_id_view: wgpu::TextureView,
    pub merged_barycentrics: wgpu::Texture,
    pub merged_barycentrics_view: wgpu::TextureView,
    pub merged_depth: wgpu::Texture,
    pub merged_depth_view: wgpu::TextureView,        // R32Float (for Float-typed passes: HZB, Lumen, Outline)

    // Depth32Float converted depth (for downstream TextureSampleType::Depth passes)
    pub merged_depth_d32: wgpu::Texture,
    pub merged_depth_d32_view: wgpu::TextureView,
    depth_convert_pipeline: wgpu::RenderPipeline,
    depth_convert_layout: wgpu::BindGroupLayout,

    width: u32,
    height: u32,
}

impl VBufferResolvePipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Group 0: Standard V-Buffer input (read)
        let std_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VBuffer Resolve G0: Standard"),
            entries: &[
                // binding 0: triangle_id (R32Uint texture)
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
                // binding 1: barycentrics (RG16Float texture)
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
                // binding 2: depth (Depth32Float)
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
            ],
        });

        // Group 1: Nanite V-Buffer input (read)
        let nanite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VBuffer Resolve G1: Nanite"),
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
            ],
        });

        // Group 2: Merged output (write) + params
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VBuffer Resolve G2: Output"),
            entries: &[
                // binding 0: merged triangle_id (R32Uint, write)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 1: merged barycentrics (RGBA16Float, write)
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
                // binding 2: merged depth (R32Float, write)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 3: params (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Compute shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VBuffer Resolve Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/vbuffer_resolve.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VBuffer Resolve Pipeline Layout"),
            bind_group_layouts: &[&std_layout, &nanite_layout, &output_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VBuffer Resolve Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VBuffer Resolve Params"),
            size: std::mem::size_of::<ResolveParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create merged output textures
        let (merged_triangle_id, merged_triangle_id_view) =
            Self::create_texture(device, width, height, wgpu::TextureFormat::R32Uint, "Merged Triangle ID");
        let (merged_barycentrics, merged_barycentrics_view) =
            Self::create_texture(device, width, height, wgpu::TextureFormat::Rgba16Float, "Merged Barycentrics");
        let (merged_depth, merged_depth_view) =
            Self::create_texture(device, width, height, wgpu::TextureFormat::R32Float, "Merged Depth");

        // Depth32Float converted texture (for downstream TextureSampleType::Depth passes)
        let (merged_depth_d32, merged_depth_d32_view) =
            Self::create_depth_texture(device, width, height);

        // Depth format convert pipeline (R32Float → Depth32Float)
        let depth_convert_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Depth Format Convert Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/depth_format_convert.wgsl").into(),
            ),
        });

        let depth_convert_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth Convert Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let depth_convert_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Depth Format Convert"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Depth Convert Pipeline Layout"),
                bind_group_layouts: &[&depth_convert_layout],
                immediate_size: 0,
            })),
            vertex: wgpu::VertexState {
                module: &depth_convert_shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &depth_convert_shader,
                entry_point: Some("fs"),
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            std_layout,
            nanite_layout,
            output_layout,
            params_buffer,
            merged_triangle_id,
            merged_triangle_id_view,
            merged_barycentrics,
            merged_barycentrics_view,
            merged_depth,
            merged_depth_view,
            merged_depth_d32,
            merged_depth_d32_view,
            depth_convert_pipeline,
            depth_convert_layout,
            width,
            height,
        }
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Create Depth32Float texture for downstream depth sampling passes.
    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Merged Depth D32"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                 | wgpu::TextureUsages::TEXTURE_BINDING
                 | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Dispatch the resolve pass to merge Standard + Nanite V-Buffers.
    pub fn resolve(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        std_triangle_id_view: &wgpu::TextureView,
        std_barycentrics_view: &wgpu::TextureView,
        std_depth_view: &wgpu::TextureView,
        nanite_triangle_id_view: &wgpu::TextureView,
        nanite_barycentrics_view: &wgpu::TextureView,
        nanite_depth_view: &wgpu::TextureView,
        enable_nanite: bool,
    ) {
        // Upload params
        let params = ResolveParams {
            width: self.width,
            height: self.height,
            enable_nanite: if enable_nanite { 1 } else { 0 },
            _pad: 0,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind groups
        let std_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VBuffer Resolve G0"),
            layout: &self.std_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(std_triangle_id_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(std_barycentrics_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(std_depth_view) },
            ],
        });

        let nanite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VBuffer Resolve G1"),
            layout: &self.nanite_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(nanite_triangle_id_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(nanite_barycentrics_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(nanite_depth_view) },
            ],
        });

        let output_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VBuffer Resolve G2"),
            layout: &self.output_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.merged_triangle_id_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.merged_barycentrics_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.merged_depth_view) },
                wgpu::BindGroupEntry { binding: 3, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        // Dispatch
        let dispatch_x = self.width.div_ceil(8);
        let dispatch_y = self.height.div_ceil(8);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("VBuffer Resolve Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &std_bg, &[]);
        pass.set_bind_group(1, &nanite_bg, &[]);
        pass.set_bind_group(2, &output_bg, &[]);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }

    /// Convert R32Float merged depth to Depth32Float for downstream sampling.
    /// Must be called after `resolve()` and before any pass that needs `TextureSampleType::Depth`.
    pub fn convert_depth_format(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Depth Convert BG"),
            layout: &self.depth_convert_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.merged_depth_view),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Depth Format Convert"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.merged_depth_d32_view,
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
        pass.set_pipeline(&self.depth_convert_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Resize output textures when viewport changes.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let (tid, tidv) = Self::create_texture(device, width, height, wgpu::TextureFormat::R32Uint, "Merged Triangle ID");
        let (bary, baryv) = Self::create_texture(device, width, height, wgpu::TextureFormat::Rgba16Float, "Merged Barycentrics");
        let (depth, depthv) = Self::create_texture(device, width, height, wgpu::TextureFormat::R32Float, "Merged Depth");
        let (d32, d32v) = Self::create_depth_texture(device, width, height);
        self.merged_triangle_id = tid;
        self.merged_triangle_id_view = tidv;
        self.merged_barycentrics = bary;
        self.merged_barycentrics_view = baryv;
        self.merged_depth = depth;
        self.merged_depth_view = depthv;
        self.merged_depth_d32 = d32;
        self.merged_depth_d32_view = d32v;
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
}

/// Check if a merged triangle_id is from Nanite.
pub fn is_nanite_triangle(triangle_id: u32) -> bool {
    triangle_id & NANITE_FLAG != 0
}

/// Strip the Nanite flag from a merged triangle_id.
pub fn strip_nanite_flag(triangle_id: u32) -> u32 {
    triangle_id & !NANITE_FLAG
}
