// SKOPE Engine - Temporal Anti-Aliasing (TAA)
// 시간적 안티앨리어싱

use bytemuck::{Pod, Zeroable};

/// TAA 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TAAParams {
    /// History 가중치 (높을수록 고스팅, 낮을수록 플리커)
    pub history_weight: f32,
    /// 클램핑 방식 (0=AABB, 1=Variance)
    pub clamp_mode: u32,
    /// 샤프닝 강도
    pub sharpness: f32,
    /// 모션 벡터 스케일
    pub motion_scale: f32,

    /// 서브픽셀 지터 활성화
    pub jitter_enabled: u32,
    /// 지터 시퀀스 (Halton 등)
    pub jitter_sequence: u32,
    /// 현재 프레임 지터
    pub current_jitter: [f32; 2],
}

impl Default for TAAParams {
    fn default() -> Self {
        Self {
            history_weight: 0.9,
            clamp_mode: 1,  // Variance clipping
            sharpness: 0.25,
            motion_scale: 1.0,
            jitter_enabled: 1,
            jitter_sequence: 0,  // Halton 2,3
            current_jitter: [0.0; 2],
        }
    }
}

impl TAAParams {
    /// 고품질 설정
    pub fn high_quality() -> Self {
        Self {
            history_weight: 0.95,
            sharpness: 0.2,
            ..Default::default()
        }
    }

    /// 낮은 고스팅 (빠른 움직임용)
    pub fn low_ghosting() -> Self {
        Self {
            history_weight: 0.8,
            sharpness: 0.3,
            ..Default::default()
        }
    }
}

/// Halton 시퀀스 생성
pub fn halton_sequence(index: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    let mut i = index;

    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }

    r
}

/// 프레임 인덱스에 따른 지터 값 반환
pub fn get_jitter(frame_index: u32, sequence_length: u32) -> [f32; 2] {
    let idx = frame_index % sequence_length;
    [
        halton_sequence(idx + 1, 2) - 0.5,
        halton_sequence(idx + 1, 3) - 0.5,
    ]
}

/// TAA 파이프라인
pub struct TAAPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    /// 현재 프레임 출력
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    /// 히스토리 버퍼
    pub history_texture: wgpu::Texture,
    pub history_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
    pub frame_index: u32,
}

impl TAAPipeline {
    pub fn new(device: &wgpu::Device, screen_size: (u32, u32)) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TAA Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TAA Params Buffer"),
            size: std::mem::size_of::<TAAParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TAA Bind Group Layout"),
            entries: &[
                // Current frame
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
                // History
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
                // Velocity
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
                // Depth
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
                // Output
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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
            label: Some("TAA Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/taa.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TAA Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("TAA Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA Output Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // History texture
        let history_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA History Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let history_view = history_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
            output_texture,
            output_view,
            history_texture,
            history_view,
            screen_size,
            frame_index: 0,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &TAAParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn get_current_jitter(&self) -> [f32; 2] {
        get_jitter(self.frame_index, 16)
    }

    pub fn increment_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA Output Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.history_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA History Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.history_view = self.history_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
}
