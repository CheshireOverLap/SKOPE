// SKOPE Engine - Color Grading System
// LUT 및 수학적 색상 조정

use bytemuck::{Pod, Zeroable};

/// Color Grading 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ColorGradingParams {
    // === 기본 조정 ===
    /// 전체 밝기 (0.5 = -1 stop, 2.0 = +1 stop)
    pub brightness: f32,
    /// 대비 (1.0 = 기본)
    pub contrast: f32,
    /// 채도 (1.0 = 기본)
    pub saturation: f32,
    /// Hue 시프트 (도, -180 ~ 180)
    pub hue_shift: f32,

    // === Lift/Gamma/Gain (그림자/중간톤/하이라이트) ===
    /// Lift (그림자 색조)
    pub lift: [f32; 3],
    pub _pad0: f32,
    /// Gamma (중간톤 색조)
    pub gamma: [f32; 3],
    pub _pad1: f32,
    /// Gain (하이라이트 색조)
    pub gain: [f32; 3],
    pub _pad2: f32,

    // === Split Toning ===
    /// 그림자 틴트 색상
    pub shadow_tint: [f32; 3],
    pub shadow_tint_strength: f32,
    /// 하이라이트 틴트 색상
    pub highlight_tint: [f32; 3],
    pub highlight_tint_strength: f32,

    // === 색온도 ===
    /// 색온도 (Kelvin, 6500 = 기본)
    pub temperature: f32,
    /// 색조 (Green-Magenta)
    pub tint: f32,

    // === LUT ===
    /// LUT 강도 (0 = LUT 없음, 1 = 100%)
    pub lut_intensity: f32,
    /// LUT 사이즈 (보통 32 또는 64)
    pub lut_size: f32,
}

impl Default for ColorGradingParams {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.1,  // SKOPE: 약간 높은 채도
            hue_shift: 0.0,

            lift: [0.0, 0.0, 0.0],
            _pad0: 0.0,
            gamma: [1.0, 1.0, 1.0],
            _pad1: 0.0,
            gain: [1.0, 1.0, 1.0],
            _pad2: 0.0,

            shadow_tint: [0.0, 0.0, 0.1],  // 약간 푸른 그림자
            shadow_tint_strength: 0.15,
            highlight_tint: [1.0, 0.95, 0.9],  // 약간 따뜻한 하이라이트
            highlight_tint_strength: 0.1,

            temperature: 6500.0,
            tint: 0.0,

            lut_intensity: 0.0,  // LUT 비활성화 (기본)
            lut_size: 32.0,
        }
    }
}

impl ColorGradingParams {
    /// 낮 야외 씬 (따뜻하고 밝음)
    pub fn daylight() -> Self {
        Self {
            brightness: 1.05,
            saturation: 1.15,
            temperature: 6800.0,  // 약간 따뜻하게
            ..Default::default()
        }
    }

    /// 밤 씬 (차갑고 어두움)
    pub fn night() -> Self {
        Self {
            brightness: 0.9,
            saturation: 0.95,
            temperature: 5500.0,  // 차갑게
            shadow_tint: [0.0, 0.05, 0.15],
            shadow_tint_strength: 0.25,
            ..Default::default()
        }
    }

    /// 감성적인 씬 (보라 기운)
    pub fn emotional() -> Self {
        Self {
            contrast: 1.1,
            saturation: 1.05,
            lift: [0.02, 0.01, 0.03],  // 그림자에 보라 기운
            ..Default::default()
        }
    }

    /// 액션 씬 (높은 대비)
    pub fn action() -> Self {
        Self {
            contrast: 1.15,
            saturation: 1.2,
            ..Default::default()
        }
    }

    /// 따뜻한 톤
    pub fn warm() -> Self {
        Self {
            temperature: 7000.0,
            highlight_tint: [1.0, 0.9, 0.8],
            highlight_tint_strength: 0.15,
            ..Default::default()
        }
    }

    /// 차가운 톤
    pub fn cool() -> Self {
        Self {
            temperature: 5500.0,
            shadow_tint: [0.0, 0.1, 0.2],
            shadow_tint_strength: 0.2,
            ..Default::default()
        }
    }
}

/// Color Grading 파이프라인
pub struct ColorGradingPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    /// 3D LUT 텍스처 (옵션)
    pub lut_texture: Option<wgpu::Texture>,
    pub lut_view: Option<wgpu::TextureView>,

    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
}

impl ColorGradingPipeline {
    pub fn new(device: &wgpu::Device, screen_size: (u32, u32)) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Color Grading Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Color Grading Params Buffer"),
            size: std::mem::size_of::<ColorGradingParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Color Grading Bind Group Layout"),
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
                // LUT texture (3D)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
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
            label: Some("Color Grading Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/color_grading.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Color Grading Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Color Grading Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Default identity LUT (32x32x32)
        let lut_size = 32;
        let (lut_texture, lut_view) = Self::create_identity_lut(device, lut_size);

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Color Grading Output Texture"),
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
            lut_texture: Some(lut_texture),
            lut_view: Some(lut_view),
            output_texture,
            output_view,
            screen_size,
        }
    }

    fn create_identity_lut(device: &wgpu::Device, size: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Identity LUT"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Identity LUT 데이터 생성 (CPU)
    pub fn generate_identity_lut_data(size: u32) -> Vec<u8> {
        let mut data = Vec::with_capacity((size * size * size * 4) as usize);

        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push((r * 255 / (size - 1)) as u8);
                    data.push((g * 255 / (size - 1)) as u8);
                    data.push((b * 255 / (size - 1)) as u8);
                    data.push(255);
                }
            }
        }

        data
    }

    pub fn upload_lut(&self, queue: &wgpu::Queue, lut_data: &[u8], size: u32) {
        if let Some(ref lut_texture) = self.lut_texture {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: lut_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                lut_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: size,
                },
            );
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &ColorGradingParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Color Grading Output Texture"),
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
