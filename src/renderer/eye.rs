// SKOPE Engine - Eye Forward Rendering Pipeline
//
// Forward Pass로 눈을 렌더링하는 파이프라인
// 각막 굴절, 홍채 Parallax, 스타일라이즈드 하이라이트 지원
//
// Reference: SKOPE Engine Rendering Pipeline v1.1 Design Doc Section 6.1

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};

use skope_shading::EyeShadeParams;

/// GPU용 Eye 파라미터 (WGSL 정렬에 맞춤)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuEyeParams {
    // Geometry (16 bytes)
    pub cornea_curvature: f32,
    pub pupil_size: f32,
    pub pupil_depth: f32,
    pub iris_size: f32,

    // Colors (32 bytes)
    pub iris_color: [f32; 3],
    pub _pad0: f32,
    pub limbal_ring_color: [f32; 3],
    pub limbal_ring_intensity: f32,

    // Reflection (16 bytes)
    pub cornea_specular: f32,
    pub cornea_ior: f32,
    pub wetness: f32,
    pub caustics_intensity: f32,

    // Stylized (16 bytes)
    pub highlight_size: f32,
    pub highlight_offset: [f32; 2],
    pub see_through_alpha: f32,
}

impl From<&EyeShadeParams> for GpuEyeParams {
    fn from(params: &EyeShadeParams) -> Self {
        Self {
            cornea_curvature: params.cornea_curvature,
            pupil_size: params.pupil_size,
            pupil_depth: params.pupil_depth,
            iris_size: params.iris_size,
            iris_color: params.iris_color,
            _pad0: 0.0,
            limbal_ring_color: params.limbal_ring_color,
            limbal_ring_intensity: params.limbal_ring_intensity,
            cornea_specular: params.cornea_specular,
            cornea_ior: params.cornea_ior,
            wetness: params.wetness,
            caustics_intensity: params.caustics_intensity,
            highlight_size: params.highlight_size,
            highlight_offset: params.highlight_offset,
            see_through_alpha: params.see_through_alpha,
        }
    }
}

impl Default for GpuEyeParams {
    fn default() -> Self {
        Self::from(&EyeShadeParams::default())
    }
}

/// Eye 렌더링 인스턴스
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EyeInstance {
    pub model_matrix: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
}

/// Eye Forward Rendering Pipeline
pub struct EyePipeline {
    // Pipelines
    pub pipeline: wgpu::RenderPipeline,
    pub pipeline_simple: wgpu::RenderPipeline,

    // Bind group layouts
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,

    // Uniform buffers
    pub eye_params_buffer: wgpu::Buffer,

    // Default textures
    pub default_iris_texture: wgpu::Texture,
    pub default_iris_view: wgpu::TextureView,
    pub default_sclera_texture: wgpu::Texture,
    pub default_sclera_view: wgpu::TextureView,

    // Sampler
    pub sampler: wgpu::Sampler,

    // Quality setting
    pub use_simple_shader: bool,
}

impl EyePipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        // Shader
        let shader_source = include_str!("../shaders/eye_forward.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Eye Forward Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Uniform bind group layout (Group 0)
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Eye Uniform Bind Group Layout"),
            entries: &[
                // Camera uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Light params
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Eye params
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Model uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Texture bind group layout (Group 1)
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Eye Texture Bind Group Layout"),
            entries: &[
                // Iris texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sclera texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal map
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Environment map (cubemap)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Eye Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 64, // position(12) + normal(12) + tangent(16) + uv(8) + padding(16)
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // Normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 16,
                    shader_location: 1,
                },
                // Tangent
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 2,
                },
                // UV
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 48,
                    shader_location: 3,
                },
            ],
        };

        // Main pipeline (high quality)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Eye Forward Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Simple pipeline (low quality)
        let pipeline_simple = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Eye Forward Pipeline (Simple)"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main_simple"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Eye params buffer
        let eye_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Eye Params Buffer"),
            size: std::mem::size_of::<GpuEyeParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Default textures
        let (default_iris_texture, default_iris_view) = Self::create_default_iris_texture(device, queue);
        let (default_sclera_texture, default_sclera_view) = Self::create_default_sclera_texture(device, queue);

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Eye Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            anisotropy_clamp: 4,
            ..Default::default()
        });

        Self {
            pipeline,
            pipeline_simple,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            eye_params_buffer,
            default_iris_texture,
            default_iris_view,
            default_sclera_texture,
            default_sclera_view,
            sampler,
            use_simple_shader: false,
        }
    }

    /// 기본 홍채 텍스처 생성 (방사형 패턴)
    fn create_default_iris_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
        let size = 256u32;
        let mut data = vec![0u8; (size * size * 4) as usize];

        for y in 0..size {
            for x in 0..size {
                let idx = ((y * size + x) * 4) as usize;

                // Radial pattern
                let fx = (x as f32 / size as f32) * 2.0 - 1.0;
                let fy = (y as f32 / size as f32) * 2.0 - 1.0;
                let dist = (fx * fx + fy * fy).sqrt();
                let angle = fy.atan2(fx);

                // Radial fibers
                let fiber = ((angle * 20.0).sin() * 0.3 + 0.7).clamp(0.0, 1.0);

                // Radial gradient
                let gradient = (1.0 - dist * 1.2).clamp(0.0, 1.0);

                let intensity = (fiber * gradient * 255.0) as u8;

                data[idx] = intensity;     // R
                data[idx + 1] = intensity; // G
                data[idx + 2] = intensity; // B
                data[idx + 3] = 255;       // A
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Iris Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// 기본 공막(흰자) 텍스처 생성
    fn create_default_sclera_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
        let size = 64u32;
        let mut data = vec![0u8; (size * size * 4) as usize];

        for y in 0..size {
            for x in 0..size {
                let idx = ((y * size + x) * 4) as usize;

                // Slight off-white with subtle variation
                let base = 240u8;
                let variation = ((x as f32 * 0.1 + y as f32 * 0.15).sin() * 5.0) as i32;
                let value = (base as i32 + variation).clamp(230, 255) as u8;

                data[idx] = value;         // R
                data[idx + 1] = value;     // G
                data[idx + 2] = (value as f32 * 0.98) as u8; // B (slightly warmer)
                data[idx + 3] = 255;       // A
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Sclera Texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Eye 파라미터 업데이트
    pub fn update_params(&self, queue: &wgpu::Queue, params: &EyeShadeParams) {
        let gpu_params = GpuEyeParams::from(params);
        queue.write_buffer(&self.eye_params_buffer, 0, bytemuck::bytes_of(&gpu_params));
    }

    /// 품질 설정
    pub fn set_quality(&mut self, use_simple: bool) {
        self.use_simple_shader = use_simple;
    }

    /// 현재 사용할 파이프라인 반환
    pub fn current_pipeline(&self) -> &wgpu::RenderPipeline {
        if self.use_simple_shader {
            &self.pipeline_simple
        } else {
            &self.pipeline
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_eye_params_size() {
        // 80 bytes expected (aligned)
        assert_eq!(std::mem::size_of::<GpuEyeParams>(), 80);
    }

    #[test]
    fn test_eye_params_conversion() {
        let params = EyeShadeParams::blue();
        let gpu_params = GpuEyeParams::from(&params);

        assert_eq!(gpu_params.iris_color, params.iris_color);
        assert_eq!(gpu_params.cornea_ior, params.cornea_ior);
    }
}
