// SKOPE Engine - Depth of Field
// 피사계 심도 효과

use bytemuck::{Pod, Zeroable};

/// DOF 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DOFParams {
    /// 초점 거리 (미터)
    pub focus_distance: f32,
    /// 피사계 심도 범위 (초점 앞뒤로)
    pub focus_range: f32,
    /// 최대 블러 크기 (픽셀)
    pub max_blur_size: f32,
    /// Bokeh 강도 (밝은 점 부각)
    pub bokeh_intensity: f32,

    /// 조리개 형태 (0=원형, 1=육각형)
    pub aperture_shape: u32,
    /// 조리개 블레이드 수 (육각형일 때)
    pub aperture_blades: u32,

    /// 카메라 near plane
    pub near_plane: f32,
    /// 카메라 far plane
    pub far_plane: f32,
}

impl Default for DOFParams {
    fn default() -> Self {
        Self {
            focus_distance: 3.0,
            focus_range: 2.0,
            max_blur_size: 8.0,
            bokeh_intensity: 0.5,
            aperture_shape: 0,
            aperture_blades: 6,
            near_plane: 0.1,
            far_plane: 100.0,
        }
    }
}

impl DOFParams {
    /// 얕은 심도 (인물 촬영)
    pub fn shallow() -> Self {
        Self {
            focus_distance: 2.0,
            focus_range: 1.0,
            max_blur_size: 12.0,
            bokeh_intensity: 0.7,
            ..Default::default()
        }
    }

    /// 깊은 심도 (풍경)
    pub fn deep() -> Self {
        Self {
            focus_distance: 10.0,
            focus_range: 20.0,
            max_blur_size: 4.0,
            bokeh_intensity: 0.3,
            ..Default::default()
        }
    }

    /// 시네마틱 (강한 보케)
    pub fn cinematic() -> Self {
        Self {
            focus_distance: 2.5,
            focus_range: 0.8,
            max_blur_size: 16.0,
            bokeh_intensity: 1.0,
            aperture_shape: 1,  // 육각형
            ..Default::default()
        }
    }
}

/// DOF 파이프라인
pub struct DOFPipeline {
    pub coc_pipeline: wgpu::ComputePipeline,
    pub blur_pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    /// Circle of Confusion 텍스처
    pub coc_texture: wgpu::Texture,
    pub coc_view: wgpu::TextureView,

    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
}

impl DOFPipeline {
    pub fn new(device: &wgpu::Device, screen_size: (u32, u32)) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DOF Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DOF Params Buffer"),
            size: std::mem::size_of::<DOFParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DOF Bind Group Layout"),
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
                // Depth texture
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
            label: Some("DOF Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/dof.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DOF Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let coc_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DOF CoC Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let blur_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DOF Blur Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // CoC texture
        let coc_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DOF CoC Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R16Float은 storage 미지원
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let coc_view = coc_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DOF Output Texture"),
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
            coc_pipeline,
            blur_pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
            coc_texture,
            coc_view,
            output_texture,
            output_view,
            screen_size,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &DOFParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.coc_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DOF CoC Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R16Float은 storage 미지원
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.coc_view = self.coc_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DOF Output Texture"),
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
