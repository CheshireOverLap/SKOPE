// SKOPE Engine - Film Effects
// Film Grain, Vignette, Chromatic Aberration

use bytemuck::{Pod, Zeroable};

/// Film Effects 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FilmEffectsParams {
    // === Film Grain ===
    /// 그레인 강도
    pub grain_intensity: f32,
    /// 그레인 크기
    pub grain_size: f32,
    /// 컬러 그레인 (0=흑백, 1=컬러)
    pub grain_colored: f32,
    /// 휘도 연동 (어두운 영역에 더 많이)
    pub grain_luminance_linked: f32,

    // === Vignette ===
    /// 비네트 강도
    pub vignette_intensity: f32,
    /// 비네트 둥글기
    pub vignette_roundness: f32,
    /// 비네트 부드러움
    pub vignette_smoothness: f32,
    pub _pad0: f32,

    /// 비네트 색상 (보통 검은색)
    pub vignette_color: [f32; 3],
    pub _pad1: f32,

    // === Chromatic Aberration ===
    /// 색수차 강도
    pub chromatic_intensity: f32,

    /// 현재 시간 (그레인 애니메이션용)
    pub time: f32,

    pub _pad2: [f32; 2],
}

impl Default for FilmEffectsParams {
    fn default() -> Self {
        Self {
            grain_intensity: 0.05,  // SKOPE: 약하게
            grain_size: 1.5,
            grain_colored: 0.3,
            grain_luminance_linked: 0.5,

            vignette_intensity: 0.2,  // SKOPE: 은은하게
            vignette_roundness: 1.0,
            vignette_smoothness: 0.4,
            _pad0: 0.0,

            vignette_color: [0.0, 0.0, 0.0],
            _pad1: 0.0,

            chromatic_intensity: 0.0,  // SKOPE: 비활성화
            time: 0.0,
            _pad2: [0.0; 2],
        }
    }
}

impl FilmEffectsParams {
    /// 시네마틱 스타일
    pub fn cinematic() -> Self {
        Self {
            grain_intensity: 0.08,
            grain_size: 1.2,
            grain_colored: 0.2,
            vignette_intensity: 0.35,
            vignette_smoothness: 0.35,
            ..Default::default()
        }
    }

    /// 클린 스타일 (효과 최소화)
    pub fn clean() -> Self {
        Self {
            grain_intensity: 0.02,
            vignette_intensity: 0.1,
            ..Default::default()
        }
    }

    /// 레트로 스타일
    pub fn retro() -> Self {
        Self {
            grain_intensity: 0.15,
            grain_size: 2.0,
            grain_colored: 0.0,  // 흑백 그레인
            vignette_intensity: 0.4,
            chromatic_intensity: 0.3,
            ..Default::default()
        }
    }

    /// 밤 씬용
    pub fn night_scene() -> Self {
        Self {
            grain_intensity: 0.1,
            grain_luminance_linked: 0.7,  // 어두운 부분에 더 많이
            vignette_intensity: 0.35,
            ..Default::default()
        }
    }
}

/// Film Effects 파이프라인
pub struct FilmEffectsPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
}

impl FilmEffectsPipeline {
    pub fn new(device: &wgpu::Device, screen_size: (u32, u32)) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Film Effects Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Film Effects Params Buffer"),
            size: std::mem::size_of::<FilmEffectsParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Film Effects Bind Group Layout"),
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
                        format: wgpu::TextureFormat::Rgba8Unorm,
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Film Effects Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/film_effects.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Film Effects Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Film Effects Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Film Effects Output Texture"),
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

    pub fn update_params(&self, queue: &wgpu::Queue, params: &FilmEffectsParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn update_time(&self, queue: &wgpu::Queue, time: f32) {
        // time 필드의 오프셋 계산
        let time_offset = std::mem::size_of::<f32>() * 13; // 14번째 f32 필드
        queue.write_buffer(&self.params_buffer, time_offset as u64, bytemuck::cast_slice(&[time]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Film Effects Output Texture"),
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

    /// Film Effects 실행
    /// Vignette + Film Grain 적용
    pub fn execute(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Film Effects Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Film Effects Pass"),
                timestamp_writes: None,
            });

            let dispatch_x = self.screen_size.0.div_ceil(8);
            let dispatch_y = self.screen_size.1.div_ceil(8);

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }
    }
}
