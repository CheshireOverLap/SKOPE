// SKOPE Engine - Screen-Space SSS Blur
// Phase 13.2: Separable Gaussian Blur for Skin SSS

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// SSS 블러 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SssBlurParams {
    /// 블러 방향 (1,0 = horizontal, 0,1 = vertical)
    pub direction: [f32; 2],
    /// 블러 폭 (픽셀 단위)
    pub blur_width: f32,
    /// 블러 강도 (0.0 ~ 1.0)
    pub blur_strength: f32,
    /// 깊이 차이 임계값 (bilateral filtering용)
    pub depth_threshold: f32,
    /// 표면 따라가기 (0 or 1)
    pub follow_surface: u32,
    /// 화면 크기
    pub screen_size: [f32; 2],
}

impl Default for SssBlurParams {
    fn default() -> Self {
        Self {
            direction: [1.0, 0.0],
            blur_width: 5.0,
            blur_strength: 0.5,
            depth_threshold: 0.01,
            follow_surface: 1,
            screen_size: [1280.0, 720.0],
        }
    }
}

/// SSS 블러 설정 (UI 노출용)
#[derive(Clone, Debug)]
pub struct SssBlurConfig {
    /// SSS 블러 활성화
    pub enabled: bool,
    /// 블러 폭 (픽셀)
    pub width: f32,
    /// 블러 강도 (0.0 ~ 1.0)
    pub strength: f32,
    /// 표면 따라가기
    pub follow_surface: bool,
    /// 깊이 임계값
    pub depth_threshold: f32,
}

impl Default for SssBlurConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 5.0,
            strength: 0.5,
            follow_surface: true,
            depth_threshold: 0.01,
        }
    }
}

/// Screen-Space SSS Blur Pass
pub struct SssBlurPass {
    /// Horizontal blur pipeline
    horizontal_pipeline: wgpu::ComputePipeline,
    /// Vertical blur pipeline
    vertical_pipeline: wgpu::ComputePipeline,
    /// Params uniform buffer
    params_buffer: wgpu::Buffer,
    /// Bind group layout
    bind_group_layout: wgpu::BindGroupLayout,
    /// Intermediate texture (horizontal pass output)
    intermediate_texture: Option<wgpu::Texture>,
    intermediate_view: Option<wgpu::TextureView>,
    /// Screen size for resize detection
    screen_size: (u32, u32),
}

impl SssBlurPass {
    /// 새 SSS 블러 패스 생성
    pub fn new(device: &wgpu::Device) -> Self {
        // Shader 로드
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSS Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sss_blur.wgsl").into()),
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSS Blur Bind Group Layout"),
            entries: &[
                // input_tex
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
                // depth_tex
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // gbuffer_rt1
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
                // output_tex
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
                // params
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
                // tex_sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSS Blur Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Horizontal pipeline
        let horizontal_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSS Blur Horizontal Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Vertical pipeline (same shader, different params)
        let vertical_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSS Blur Vertical Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Params buffer
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSS Blur Params Buffer"),
            contents: bytemuck::cast_slice(&[SssBlurParams::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            horizontal_pipeline,
            vertical_pipeline,
            params_buffer,
            bind_group_layout,
            intermediate_texture: None,
            intermediate_view: None,
            screen_size: (0, 0),
        }
    }

    /// 화면 크기 변경 시 중간 텍스처 재생성
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.screen_size == (width, height) {
            return;
        }

        self.screen_size = (width, height);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSS Blur Intermediate Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        self.intermediate_view = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        self.intermediate_texture = Some(texture);
    }

    /// SSS 블러 실행
    pub fn execute(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        gbuffer_rt1_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        config: &SssBlurConfig,
    ) {
        if !config.enabled {
            return;
        }

        let Some(intermediate_view) = &self.intermediate_view else {
            return;
        };

        let (width, height) = self.screen_size;

        // Horizontal pass params
        let h_params = SssBlurParams {
            direction: [1.0, 0.0],
            blur_width: config.width,
            blur_strength: config.strength,
            depth_threshold: config.depth_threshold,
            follow_surface: if config.follow_surface { 1 } else { 0 },
            screen_size: [width as f32, height as f32],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[h_params]));

        // Horizontal pass bind group
        let h_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSS Blur Horizontal Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(gbuffer_rt1_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(intermediate_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        // Horizontal pass
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSS Blur Horizontal Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.horizontal_pipeline);
            pass.set_bind_group(0, &h_bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }

        // Vertical pass params
        let v_params = SssBlurParams {
            direction: [0.0, 1.0],
            blur_width: config.width,
            blur_strength: config.strength,
            depth_threshold: config.depth_threshold,
            follow_surface: if config.follow_surface { 1 } else { 0 },
            screen_size: [width as f32, height as f32],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[v_params]));

        // Vertical pass bind group
        let v_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSS Blur Vertical Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(intermediate_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(gbuffer_rt1_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        // Vertical pass
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSS Blur Vertical Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.vertical_pipeline);
            pass.set_bind_group(0, &v_bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
    }
}
