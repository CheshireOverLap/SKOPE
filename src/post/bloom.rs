// SKOPE Engine - Bloom System
// Dual Kawase Blur 방식

use bytemuck::{Pod, Zeroable};

/// Bloom 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BloomParams {
    /// 블룸 추출 임계값 (이 밝기 이상만)
    pub threshold: f32,
    /// Soft threshold (부드러운 전환)
    pub soft_threshold: f32,
    /// 블룸 강도
    pub intensity: f32,
    /// 다운샘플 패스 수
    pub downsample_passes: u32,

    /// 블룸 색조 (약간 따뜻하게 등)
    pub tint: [f32; 3],
    /// 업샘플 시 블렌드 강도
    pub upsample_blend: f32,

    /// 캐릭터 블룸 억제 (Shading Model 기반)
    pub character_bloom_suppress: f32,
    pub _pad: [f32; 3],
}

impl Default for BloomParams {
    fn default() -> Self {
        Self {
            threshold: 0.9,
            soft_threshold: 0.3,
            intensity: 0.3,
            downsample_passes: 6,
            tint: [1.0, 0.98, 0.95],  // 약간 따뜻하게
            upsample_blend: 0.7,
            character_bloom_suppress: 0.5,
            _pad: [0.0; 3],
        }
    }
}

impl BloomParams {
    /// 강한 블룸 (밝은 씬)
    pub fn bright_scene() -> Self {
        Self {
            threshold: 1.0,
            intensity: 0.2,
            ..Default::default()
        }
    }

    /// 분위기 있는 블룸 (밤 씬)
    pub fn moody() -> Self {
        Self {
            threshold: 0.7,
            intensity: 0.4,
            tint: [0.9, 0.95, 1.0],  // 차가운 블룸
            ..Default::default()
        }
    }

    /// 최소 블룸
    pub fn minimal() -> Self {
        Self {
            threshold: 1.2,
            intensity: 0.15,
            ..Default::default()
        }
    }
}

/// Bloom 파이프라인
pub struct BloomPipeline {
    /// 밝기 추출 파이프라인
    pub threshold_pipeline: wgpu::ComputePipeline,
    /// Dual Kawase 다운샘플
    pub downsample_pipeline: wgpu::ComputePipeline,
    /// Dual Kawase 업샘플
    pub upsample_pipeline: wgpu::ComputePipeline,
    /// 최종 합성
    pub composite_pipeline: wgpu::ComputePipeline,

    /// MIP 체인 텍스처
    pub mip_chain: Vec<wgpu::Texture>,
    pub mip_views: Vec<wgpu::TextureView>,

    /// 파라미터 버퍼
    pub params_buffer: wgpu::Buffer,

    /// Bind group layout
    pub threshold_bind_group_layout: wgpu::BindGroupLayout,
    pub downsample_bind_group_layout: wgpu::BindGroupLayout,
    pub upsample_bind_group_layout: wgpu::BindGroupLayout,

    /// 샘플러
    pub sampler: wgpu::Sampler,

    /// 출력 텍스처
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// 스크린 사이즈
    pub screen_size: (u32, u32),
}

impl BloomPipeline {
    pub fn new(
        device: &wgpu::Device,
        screen_size: (u32, u32),
    ) -> Self {
        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Params Buffer"),
            size: std::mem::size_of::<BloomParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layouts
        let threshold_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Threshold Bind Group Layout"),
            entries: &[
                // HDR input
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Shading model (for character suppression)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Output
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
                // Params
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

        let downsample_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Downsample Bind Group Layout"),
            entries: &[
                // Input texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Output texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let upsample_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Upsample Bind Group Layout"),
            entries: &[
                // Input texture (current mip)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Blend texture (previous mip)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Output texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Blend factor
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

        // Create pipelines
        let threshold_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Threshold Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/bloom_threshold.wgsl").into()),
        });

        let downsample_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Downsample Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/bloom_downsample.wgsl").into()),
        });

        let upsample_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Upsample Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/bloom_upsample.wgsl").into()),
        });

        let threshold_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Threshold Pipeline Layout"),
            bind_group_layouts: &[&threshold_bind_group_layout],
            push_constant_ranges: &[],
        });

        let threshold_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bloom Threshold Pipeline"),
            layout: Some(&threshold_pipeline_layout),
            module: &threshold_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let downsample_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Downsample Pipeline Layout"),
            bind_group_layouts: &[&downsample_bind_group_layout],
            push_constant_ranges: &[],
        });

        let downsample_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bloom Downsample Pipeline"),
            layout: Some(&downsample_pipeline_layout),
            module: &downsample_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let upsample_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Upsample Pipeline Layout"),
            bind_group_layouts: &[&upsample_bind_group_layout],
            push_constant_ranges: &[],
        });

        let upsample_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bloom Upsample Pipeline"),
            layout: Some(&upsample_pipeline_layout),
            module: &upsample_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Composite pipeline (same as upsample for now)
        let composite_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Bloom Composite Pipeline"),
            layout: Some(&upsample_pipeline_layout),
            module: &upsample_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Create MIP chain textures
        let (mip_chain, mip_views) = Self::create_mip_chain(device, screen_size, 6);

        // Output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Output Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            threshold_pipeline,
            downsample_pipeline,
            upsample_pipeline,
            composite_pipeline,
            mip_chain,
            mip_views,
            params_buffer,
            threshold_bind_group_layout,
            downsample_bind_group_layout,
            upsample_bind_group_layout,
            sampler,
            output_texture,
            output_view,
            screen_size,
        }
    }

    fn create_mip_chain(
        device: &wgpu::Device,
        base_size: (u32, u32),
        levels: u32,
    ) -> (Vec<wgpu::Texture>, Vec<wgpu::TextureView>) {
        let mut textures = Vec::new();
        let mut views = Vec::new();

        let mut width = base_size.0;
        let mut height = base_size.1;

        for i in 0..levels {
            width = (width / 2).max(1);
            height = (height / 2).max(1);

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Bloom MIP {}", i)),
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
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            textures.push(texture);
            views.push(view);
        }

        (textures, views)
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &BloomParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        // Recreate MIP chain
        let (mip_chain, mip_views) = Self::create_mip_chain(device, new_size, 6);
        self.mip_chain = mip_chain;
        self.mip_views = mip_views;

        // Recreate output
        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Output Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
}
