// SKOPE Engine - Subsurface Scattering (SSS)
//
// Screen-space subsurface scattering for realistic skin and translucent materials.
// Uses separable blur with a diffusion profile.
//
// Based on "Separable Subsurface Scattering" by Jorge Jimenez et al.
// Provides real-time approximation of light diffusion in skin/wax/milk etc.

use wgpu;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// SSS Parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SssParams {
    /// Projection matrix
    pub proj: [[f32; 4]; 4],
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// SSS width (world-space scattering radius)
    pub sss_width: f32,
    /// Follow surface (how much to follow surface curvature)
    pub follow_surface: f32,
    /// Blur direction (1,0) for horizontal, (0,1) for vertical
    pub direction: [f32; 2],
    /// Strength multiplier
    pub strength: f32,
    /// Translucency (back-lighting amount)
    pub translucency: f32,
}

impl Default for SssParams {
    fn default() -> Self {
        Self {
            proj: Mat4::IDENTITY.to_cols_array_2d(),
            screen_size: [1920.0, 1080.0],
            sss_width: 0.012,  // About 1.2cm in world units
            follow_surface: 0.5,
            direction: [1.0, 0.0],
            strength: 1.0,
            translucency: 0.5,
        }
    }
}

/// Pre-computed SSS kernel (Gaussian sum of three)
/// Based on skin diffusion profile
pub const SSS_KERNEL_SIZE: usize = 25;

/// SSS diffusion profile sample
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SssKernelSample {
    /// Offset from center (in [-1, 1] range)
    pub offset: f32,
    /// Weight for this sample
    pub weight: f32,
    /// Falloff factor for edge detection
    pub falloff: f32,
    /// Padding
    pub _pad: f32,
}

/// SSS Pipeline
pub struct SssPipeline {
    /// Horizontal blur pipeline
    pub blur_h_pipeline: wgpu::ComputePipeline,
    /// Vertical blur pipeline
    pub blur_v_pipeline: wgpu::ComputePipeline,

    /// Bind group layout
    pub layout: wgpu::BindGroupLayout,

    /// Parameters buffer
    pub params_buffer: wgpu::Buffer,

    /// Kernel buffer (pre-computed samples)
    pub kernel_buffer: wgpu::Buffer,

    /// Intermediate texture (after horizontal pass)
    pub temp_texture: wgpu::Texture,
    pub temp_view: wgpu::TextureView,

    /// Output texture
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// Samplers
    pub point_sampler: wgpu::Sampler,
    pub linear_sampler: wgpu::Sampler,

    /// Dimensions
    pub width: u32,
    pub height: u32,
}

impl SssPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Create bind group layout
        let layout = Self::create_layout(device);

        // Parameters buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSS Params"),
            size: std::mem::size_of::<SssParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Pre-compute kernel
        let kernel = Self::compute_kernel();
        let kernel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSS Kernel"),
            size: (std::mem::size_of::<SssKernelSample>() * SSS_KERNEL_SIZE) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create textures
        let temp_texture = Self::create_sss_texture(device, "SSS Temp", width, height);
        let temp_view = temp_texture.create_view(&Default::default());

        let output_texture = Self::create_sss_texture(device, "SSS Output", width, height);
        let output_view = output_texture.create_view(&Default::default());

        // Samplers
        let point_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSS Point Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSS Linear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create pipeline (same shader for both passes, different params)
        let blur_h_pipeline = Self::create_pipeline(device, &layout);
        let blur_v_pipeline = Self::create_pipeline(device, &layout);

        log::info!("[SSS] Initialized {}x{}, kernel size {}", width, height, SSS_KERNEL_SIZE);

        Self {
            blur_h_pipeline,
            blur_v_pipeline,
            layout,
            params_buffer,
            kernel_buffer,
            temp_texture,
            temp_view,
            output_texture,
            output_view,
            point_sampler,
            linear_sampler,
            width,
            height,
        }
    }

    fn create_sss_texture(device: &wgpu::Device, label: &str, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSS Layout"),
            entries: &[
                // binding 0: params
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
                // binding 1: kernel
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
                // binding 2: input color
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: depth buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: SSS mask (alpha = sss amount)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 5: linear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 6: output
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSS Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sss_blur.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSS Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSS Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Compute the SSS kernel based on skin diffusion profile
    fn compute_kernel() -> [SssKernelSample; SSS_KERNEL_SIZE] {
        let mut kernel = [SssKernelSample {
            offset: 0.0,
            weight: 0.0,
            falloff: 0.0,
            _pad: 0.0,
        }; SSS_KERNEL_SIZE];

        // Sum of three Gaussians for skin diffusion profile
        // Variances based on measured skin data
        let gaussians = [
            (0.233, 0.0064),   // Red channel - narrow
            (0.455, 0.0484),   // Green channel - medium
            (0.649, 0.1870),   // Blue channel - wide
        ];

        let range = 3.0;  // Standard deviations to cover
        let mut total_weight = 0.0;

        for (i, sample) in kernel.iter_mut().enumerate() {
            let t = (i as f32) / ((SSS_KERNEL_SIZE - 1) as f32) * 2.0 - 1.0;
            let offset = t * range;

            // Sum of Gaussians
            let mut weight = 0.0;
            for &(amplitude, variance) in &gaussians {
                let g = amplitude * (-offset * offset / (2.0 * variance)).exp();
                weight += g;
            }

            sample.offset = offset;
            sample.weight = weight;
            sample.falloff = 1.0;  // Will be modulated by depth difference
            total_weight += weight;
        }

        // Normalize weights
        for sample in kernel.iter_mut() {
            sample.weight /= total_weight;
        }

        kernel
    }

    /// Render SSS blur
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        sss_mask_view: &wgpu::TextureView,
        proj: Mat4,
    ) {
        // Upload kernel
        let kernel = Self::compute_kernel();
        queue.write_buffer(&self.kernel_buffer, 0, bytemuck::cast_slice(&kernel));

        // Horizontal pass
        {
            let params = SssParams {
                proj: proj.to_cols_array_2d(),
                screen_size: [self.width as f32, self.height as f32],
                direction: [1.0, 0.0],
                ..Default::default()
            };
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSS Horizontal Bind Group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.kernel_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(sss_mask_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&self.temp_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSS Horizontal Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blur_h_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((self.width + 7) / 8, (self.height + 7) / 8, 1);
        }

        // Vertical pass
        {
            let params = SssParams {
                proj: proj.to_cols_array_2d(),
                screen_size: [self.width as f32, self.height as f32],
                direction: [0.0, 1.0],
                ..Default::default()
            };
            queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSS Vertical Bind Group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.kernel_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.temp_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(sss_mask_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&self.output_view),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSS Vertical Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.blur_v_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((self.width + 7) / 8, (self.height + 7) / 8, 1);
        }
    }

    /// Resize buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;

        self.temp_texture = Self::create_sss_texture(device, "SSS Temp", width, height);
        self.temp_view = self.temp_texture.create_view(&Default::default());

        self.output_texture = Self::create_sss_texture(device, "SSS Output", width, height);
        self.output_view = self.output_texture.create_view(&Default::default());

        log::info!("[SSS] Resized to {}x{}", width, height);
    }
}
