// SKOPE Engine - Bloom System
// COD:AW Style Physical Bloom
// Reference: Call of Duty: Advanced Warfare (SIGGRAPH 2014)
//
// Features:
// - 13-tap Karis filter for downsample (prevents fireflies)
// - 9-tap tent filter for upsample
// - Physical bloom: threshold=0, intensity=0.04
// - 7 MIP levels for wide bloom coverage

use bytemuck::{Pod, Zeroable};

/// Bloom 파라미터
/// COD:AW 스타일 Physical Bloom
/// WGSL std140 정렬 규칙에 맞춤 (총 64바이트)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BloomParams {
    /// 블룸 추출 임계값 (0.0 = 물리 기반, 모든 밝기 기여)
    pub threshold: f32,              // offset 0
    /// Soft threshold (부드러운 전환, threshold > 0일 때만 유효)
    pub soft_threshold: f32,         // offset 4
    /// 블룸 강도 (0.04 = 물리 기반, 현실적)
    pub intensity: f32,              // offset 8
    /// 다운샘플 패스 수 (6-8 권장)
    pub downsample_passes: u32,      // offset 12

    /// 블룸 색조 (약간 따뜻하게 등)
    pub tint: [f32; 3],              // offset 16-27 (vec3 aligned to 16)
    /// 블룸 반경 배율 (1.0 = 기본)
    pub radius: f32,                 // offset 28

    /// 캐릭터 블룸 억제 (Shading Model 기반)
    pub character_bloom_suppress: f32, // offset 32
    pub _pad0: [f32; 3],             // offset 36-47 (padding before _pad vec3)

    pub _pad1: [f32; 3],             // offset 48-59 (vec3 aligned to 16)
    pub _pad2: f32,                  // offset 60-63 (final padding to 64)
}

impl Default for BloomParams {
    fn default() -> Self {
        Self {
            // Physical bloom defaults (COD:AW style)
            threshold: 0.0,           // 물리 기반: 임계값 없음
            soft_threshold: 0.5,      // 필요시 soft falloff
            intensity: 0.04,          // 물리 기반: 낮은 강도
            downsample_passes: 7,     // 7 MIP levels
            tint: [1.0, 1.0, 1.0],    // 중립 색조
            radius: 1.0,              // 기본 반경
            character_bloom_suppress: 0.3,
            _pad0: [0.0; 3],
            _pad1: [0.0; 3],
            _pad2: 0.0,
        }
    }
}

/// Upsample 패스용 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BloomUpsampleParams {
    pub blend_factor: f32,
    pub radius: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl BloomParams {
    /// 물리 기반 블룸 (기본값)
    pub fn physical() -> Self {
        Self::default()
    }

    /// 예술적 블룸 (임계값 사용)
    pub fn artistic() -> Self {
        Self {
            threshold: 0.8,
            soft_threshold: 0.3,
            intensity: 0.15,
            tint: [1.0, 0.98, 0.95],  // 약간 따뜻하게
            ..Default::default()
        }
    }

    /// 강한 블룸 (밝은 씬)
    pub fn bright_scene() -> Self {
        Self {
            threshold: 0.0,
            intensity: 0.06,
            radius: 1.2,
            ..Default::default()
        }
    }

    /// 분위기 있는 블룸 (밤 씬)
    pub fn moody() -> Self {
        Self {
            threshold: 0.0,
            intensity: 0.08,
            tint: [0.9, 0.95, 1.0],  // 차가운 블룸
            radius: 1.5,
            ..Default::default()
        }
    }

    /// 최소 블룸
    pub fn minimal() -> Self {
        Self {
            threshold: 0.0,
            intensity: 0.02,
            radius: 0.8,
            ..Default::default()
        }
    }

    /// 드라마틱 블룸 (영화 스타일)
    pub fn cinematic() -> Self {
        Self {
            threshold: 0.0,
            intensity: 0.05,
            tint: [1.0, 0.95, 0.9],  // 따뜻한 필름 느낌
            radius: 1.3,
            character_bloom_suppress: 0.5,
            ..Default::default()
        }
    }
}

/// Bloom 파이프라인
/// COD:AW Style Physical Bloom with Mip Chain
pub struct BloomPipeline {
    /// 밝기 추출 파이프라인
    pub threshold_pipeline: wgpu::ComputePipeline,
    /// 13-tap Karis 다운샘플
    pub downsample_pipeline: wgpu::ComputePipeline,
    /// 9-tap Tent 업샘플
    pub upsample_pipeline: wgpu::ComputePipeline,
    /// 최종 합성
    pub composite_pipeline: wgpu::ComputePipeline,

    /// MIP 체인 텍스처 (7 levels for COD:AW style)
    pub mip_chain: Vec<wgpu::Texture>,
    pub mip_views: Vec<wgpu::TextureView>,

    /// Upsample용 별도 텍스처 (Ping-Pong 패턴)
    /// 같은 텍스처를 읽기/쓰기하는 충돌 방지
    pub upsample_chain: Vec<wgpu::Texture>,
    pub upsample_views: Vec<wgpu::TextureView>,

    /// Bloom 파라미터 버퍼
    pub params_buffer: wgpu::Buffer,
    /// Upsample 파라미터 버퍼
    pub upsample_params_buffer: wgpu::Buffer,

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

    /// MIP 레벨 수
    pub mip_levels: u32,
}

impl BloomPipeline {
    /// COD:AW 스타일 기본 MIP 레벨 수
    pub const DEFAULT_MIP_LEVELS: u32 = 7;

    pub fn new(
        device: &wgpu::Device,
        screen_size: (u32, u32),
    ) -> Self {
        Self::with_mip_levels(device, screen_size, Self::DEFAULT_MIP_LEVELS)
    }

    pub fn with_mip_levels(
        device: &wgpu::Device,
        screen_size: (u32, u32),
        mip_levels: u32,
    ) -> Self {
        // Sampler (linear filtering for smooth bloom)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Bloom params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Params Buffer"),
            size: std::mem::size_of::<BloomParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upsample params buffer
        let upsample_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Upsample Params Buffer"),
            size: std::mem::size_of::<BloomUpsampleParams>() as u64,
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

        // Create MIP chain textures (COD:AW uses 7-8 levels)
        let (mip_chain, mip_views) = Self::create_mip_chain(device, screen_size, mip_levels);

        // Create Upsample chain (Ping-Pong to avoid read/write conflict)
        let (upsample_chain, upsample_views) = Self::create_upsample_chain(device, screen_size, mip_levels);

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
            upsample_chain,
            upsample_views,
            params_buffer,
            upsample_params_buffer,
            threshold_bind_group_layout,
            downsample_bind_group_layout,
            upsample_bind_group_layout,
            sampler,
            output_texture,
            output_view,
            screen_size,
            mip_levels,
        }
    }

    /// Upsample 파라미터 업데이트
    pub fn update_upsample_params(&self, queue: &wgpu::Queue, params: &BloomUpsampleParams) {
        queue.write_buffer(&self.upsample_params_buffer, 0, bytemuck::cast_slice(&[*params]));
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

    /// Upsample용 Ping-Pong 텍스처 체인 생성
    /// mip_levels - 1개 (최종은 output_view로 직접 출력)
    fn create_upsample_chain(
        device: &wgpu::Device,
        base_size: (u32, u32),
        levels: u32,
    ) -> (Vec<wgpu::Texture>, Vec<wgpu::TextureView>) {
        let mut textures = Vec::new();
        let mut views = Vec::new();

        // mip[1] ~ mip[levels-1]에 대응하는 텍스처 생성
        // (mip[0]은 output_view로 바로 출력하므로 불필요)
        for i in 1..levels {
            let width = (base_size.0 >> (i + 1)).max(1);
            let height = (base_size.1 >> (i + 1)).max(1);

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Bloom Upsample {}", i)),
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

        // Recreate MIP chain with same level count
        let (mip_chain, mip_views) = Self::create_mip_chain(device, new_size, self.mip_levels);
        self.mip_chain = mip_chain;
        self.mip_views = mip_views;

        // Recreate Upsample chain (Ping-Pong)
        let (upsample_chain, upsample_views) = Self::create_upsample_chain(device, new_size, self.mip_levels);
        self.upsample_chain = upsample_chain;
        self.upsample_views = upsample_views;

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

    /// MIP 체인 레벨 수 반환
    pub fn mip_level_count(&self) -> u32 {
        self.mip_levels
    }

    /// Bloom 효과 실행
    /// hdr_input: Material Eval의 HDR 출력 (Rgba16Float)
    /// shading_model: 캐릭터 억제용 텍스처 (없으면 기본 검정 텍스처 사용)
    pub fn execute(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hdr_input: &wgpu::TextureView,
        shading_model: &wgpu::TextureView,
    ) {
        if self.mip_views.is_empty() {
            return;
        }

        // === 1. Threshold Pass ===
        // HDR → mip_views[0] (첫 번째 다운샘플)
        let threshold_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Threshold Bind Group"),
            layout: &self.threshold_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(shading_model),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.mip_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Bloom Threshold Pass"),
                timestamp_writes: None,
            });

            let width = (self.screen_size.0 / 2).max(1);
            let height = (self.screen_size.1 / 2).max(1);
            let dispatch_x = (width + 7) / 8;
            let dispatch_y = (height + 7) / 8;

            pass.set_pipeline(&self.threshold_pipeline);
            pass.set_bind_group(0, &threshold_bind_group, &[]);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        // === 2. Downsample Chain ===
        // mip[0] → mip[1] → ... → mip[n-1]
        for i in 0..(self.mip_levels - 1) as usize {
            let input_view = &self.mip_views[i];
            let output_view = &self.mip_views[i + 1];

            let downsample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Bloom Downsample {} Bind Group", i)),
                layout: &self.downsample_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(output_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(&format!("Bloom Downsample Pass {}", i)),
                    timestamp_writes: None,
                });

                // 각 레벨의 크기 계산
                let level_width = (self.screen_size.0 >> (i + 2)).max(1);
                let level_height = (self.screen_size.1 >> (i + 2)).max(1);
                let dispatch_x = (level_width + 7) / 8;
                let dispatch_y = (level_height + 7) / 8;

                pass.set_pipeline(&self.downsample_pipeline);
                pass.set_bind_group(0, &downsample_bind_group, &[]);
                pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
            }
        }

        // === 3. Upsample Chain (Ping-Pong) ===
        // Ping-Pong 패턴으로 읽기/쓰기 충돌 방지
        // 첫 iteration: mip[n-1] → upsample[n-3]
        // 이후: upsample[k] → upsample[k-1]
        // 마지막: upsample[0] → output_view
        for i in (0..(self.mip_levels - 1) as usize).rev() {
            // 입력: 첫 iteration은 downsample 결과(mip), 이후는 이전 upsample 결과
            let input_view = if i == (self.mip_levels - 2) as usize {
                &self.mip_views[i + 1]  // 첫 iteration: 가장 작은 MIP
            } else {
                &self.upsample_views[i]  // 이후: 이전 upsample 출력
            };

            // 블렌드: 항상 원본 MIP에서 읽기 (downsample 데이터)
            let blend_view = &self.mip_views[i];

            // 출력: 마지막은 output_view, 나머지는 upsample 텍스처
            let output_view = if i == 0 {
                &self.output_view  // 최종 출력 (전체 화면)
            } else {
                &self.upsample_views[i - 1]  // upsample 텍스처에 쓰기
            };

            let upsample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Bloom Upsample {} Bind Group", i)),
                layout: &self.upsample_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(blend_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(output_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.upsample_params_buffer.as_entire_binding(),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(&format!("Bloom Upsample Pass {}", i)),
                    timestamp_writes: None,
                });

                // 출력 레벨의 크기 계산
                let level_width = if i == 0 {
                    self.screen_size.0
                } else {
                    (self.screen_size.0 >> (i + 1)).max(1)
                };
                let level_height = if i == 0 {
                    self.screen_size.1
                } else {
                    (self.screen_size.1 >> (i + 1)).max(1)
                };
                let dispatch_x = (level_width + 7) / 8;
                let dispatch_y = (level_height + 7) / 8;

                pass.set_pipeline(&self.upsample_pipeline);
                pass.set_bind_group(0, &upsample_bind_group, &[]);
                pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
            }
        }
    }
}
