// SKOPE Engine - Hierarchical Z-Buffer (HZB) Module
//
// Generates mip chain of depth buffer for:
// - SSR (Screen-Space Reflections) - Hi-Z ray march
// - Contact Shadows - fast occlusion testing
// - DDGI - screen-space ray tracing acceleration

use bytemuck::{Pod, Zeroable};

/// Maximum mip levels supported
pub const MAX_HZB_MIPS: usize = 12;  // Supports up to 4096x4096

/// HZB Parameters (matches WGSL struct)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct HzbParams {
    pub src_mip_size: [u32; 2],
    pub dst_mip_size: [u32; 2],
    pub src_mip_level: u32,
    pub dst_mip_level: u32,
    pub _pad: [u32; 2],
}

/// Hierarchical Z-Buffer Pipeline
pub struct HzbPipeline {
    pub downsample_pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,

    // HZB texture with full mip chain
    pub hzb_texture: wgpu::Texture,
    pub hzb_view: wgpu::TextureView,
    pub mip_views: Vec<wgpu::TextureView>,

    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
}

impl HzbPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Calculate mip count
        let mip_count = Self::calculate_mip_count(width, height);

        // Bind group layout for downsample compute
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HZB Bind Group Layout"),
            entries: &[
                // Params uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<HzbParams>() as u64),
                    },
                    count: None,
                },
                // Source depth (read)
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
                // Destination depth (write)
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
            ],
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HZB Params Buffer"),
            size: std::mem::size_of::<HzbParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // HZB texture with mip chain
        let hzb_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HZB Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Full texture view
        let hzb_view = hzb_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Per-mip views for compute dispatches
        let mip_views: Vec<wgpu::TextureView> = (0..mip_count)
            .map(|mip| {
                hzb_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("HZB Mip {} View", mip)),
                    format: None,
                    dimension: None,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    base_array_layer: 0,
                    array_layer_count: None,
                    usage: None,
                })
            })
            .collect();

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HZB Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/hzb.wgsl").into()),
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HZB Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Compute pipeline
        let downsample_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("HZB Downsample Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("downsample"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            downsample_pipeline,
            bind_group_layout,
            params_buffer,
            hzb_texture,
            hzb_view,
            mip_views,
            width,
            height,
            mip_count,
        }
    }

    /// Calculate number of mip levels
    fn calculate_mip_count(width: u32, height: u32) -> u32 {
        let max_dim = width.max(height) as f32;
        (max_dim.log2().floor() as u32 + 1).min(MAX_HZB_MIPS as u32)
    }

    /// Get mip size
    pub fn mip_size(&self, mip: u32) -> (u32, u32) {
        let w = (self.width >> mip).max(1);
        let h = (self.height >> mip).max(1);
        (w, h)
    }

    /// Create bind group for a specific mip transition
    fn create_downsample_bind_group(
        &self,
        device: &wgpu::Device,
        src_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HZB Downsample Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(dst_view),
                },
            ],
        })
    }

    /// Generate HZB from depth buffer
    ///
    /// # Arguments
    /// * `depth_view` - Source depth buffer view (used as mip 0 input)
    pub fn generate(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
    ) {
        // First pass: copy depth to mip 0 would be ideal, but we'll read directly
        // from depth buffer for mip 0 → mip 1 transition

        for mip in 0..(self.mip_count - 1) {
            let (src_w, src_h) = self.mip_size(mip);
            let (dst_w, dst_h) = self.mip_size(mip + 1);

            // Update params
            let params = HzbParams {
                src_mip_size: [src_w, src_h],
                dst_mip_size: [dst_w, dst_h],
                src_mip_level: mip,
                dst_mip_level: mip + 1,
                _pad: [0, 0],
            };
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

            // Source: for mip 0, use original depth; otherwise use HZB mip
            let src_view = if mip == 0 {
                depth_view
            } else {
                &self.mip_views[mip as usize]
            };

            // Destination: next HZB mip
            let dst_view = &self.mip_views[(mip + 1) as usize];

            // Create bind group
            let bind_group = self.create_downsample_bind_group(device, src_view, dst_view);

            // Dispatch compute
            let workgroups_x = dst_w.div_ceil(8);
            let workgroups_y = dst_h.div_ceil(8);

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&format!("HZB Mip {} → {}", mip, mip + 1)),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.downsample_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
    }

    /// Resize HZB texture
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }

        self.width = width;
        self.height = height;
        self.mip_count = Self::calculate_mip_count(width, height);

        // Recreate texture
        self.hzb_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HZB Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: self.mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.hzb_view = self.hzb_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.mip_views = (0..self.mip_count)
            .map(|mip| {
                self.hzb_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("HZB Mip {} View", mip)),
                    format: None,
                    dimension: None,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    base_array_layer: 0,
                    array_layer_count: None,
                    usage: None,
                })
            })
            .collect();
    }
}
