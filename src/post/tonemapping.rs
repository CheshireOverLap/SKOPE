// SKOPE Engine - Tonemapping System
// HDR → LDR 변환

use bytemuck::{Pod, Zeroable};

/// Tonemapping 연산자 타입
#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TonemapOperator {
    /// Reinhard (심플)
    Reinhard = 0,
    /// ACES Filmic
    #[default]
    ACES = 1,
    /// Uncharted 2 / Hable
    Uncharted2 = 2,
    /// AgX (Blender 스타일)
    AgX = 3,
    /// 커스텀 커브
    Custom = 4,
}

/// Tonemapping 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TonemapParams {
    /// 톤맵 연산자 타입
    pub operator: u32,
    /// 노출 조정 (EV)
    pub exposure: f32,
    /// 화이트 포인트
    pub white_point: f32,
    /// 채도 보존 강도 (ACES 보정용)
    pub saturation_preserve: f32,

    /// 감마 (보통 2.2)
    pub gamma: f32,

    pub _pad: [f32; 3],
}

impl Default for TonemapParams {
    fn default() -> Self {
        Self {
            operator: TonemapOperator::ACES as u32,
            exposure: 1.0,
            white_point: 4.0,
            saturation_preserve: 0.3,  // SKOPE: 채도 좀 더 보존
            gamma: 2.2,
            _pad: [0.0; 3],
        }
    }
}

impl TonemapParams {
    /// 밝은 야외 씬
    pub fn bright_outdoor() -> Self {
        Self {
            exposure: 1.2,
            white_point: 5.0,
            ..Default::default()
        }
    }

    /// 어두운 실내 씬
    pub fn dark_indoor() -> Self {
        Self {
            exposure: 0.8,
            white_point: 3.0,
            ..Default::default()
        }
    }

    /// 시네마틱 (ACES 강화)
    pub fn cinematic() -> Self {
        Self {
            operator: TonemapOperator::ACES as u32,
            saturation_preserve: 0.2,
            ..Default::default()
        }
    }

    /// 자연스러운 (Reinhard)
    pub fn natural() -> Self {
        Self {
            operator: TonemapOperator::Reinhard as u32,
            ..Default::default()
        }
    }
}

/// Tonemapping 파이프라인
pub struct TonemapPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
}

impl TonemapPipeline {
    pub fn new(device: &wgpu::Device, screen_size: (u32, u32)) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tonemap Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tonemap Params Buffer"),
            size: std::mem::size_of::<TonemapParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tonemap Bind Group Layout"),
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
                // Bloom texture
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
                // LDR output
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
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
                // Params
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tonemapping Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/tonemapping.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Tonemap Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Tonemap Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Tonemap Output Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
            output_texture,
            output_view,
            screen_size,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &TonemapParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Tonemap Output Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
}
