// SKOPE Engine - Screen Space Ambient Occlusion (SSAO)
// 화면 공간 환경광 차폐

use bytemuck::{Pod, Zeroable};

/// SSAO 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SSAOParams {
    /// AO 반경
    pub radius: f32,
    /// AO 강도
    pub intensity: f32,
    /// 바이어스 (셀프 섀도잉 방지)
    pub bias: f32,
    /// 샘플 수
    pub sample_count: u32,

    /// 블러 패스 수
    pub blur_passes: u32,
    /// 블러 샤프니스 (edge-aware)
    pub blur_sharpness: f32,

    /// 카메라 near plane
    pub near_plane: f32,
    /// 카메라 far plane
    pub far_plane: f32,
}

impl Default for SSAOParams {
    fn default() -> Self {
        Self {
            radius: 0.5,
            intensity: 1.0,
            bias: 0.025,
            sample_count: 16,
            blur_passes: 2,
            blur_sharpness: 4.0,
            near_plane: 0.1,
            far_plane: 100.0,
        }
    }
}

impl SSAOParams {
    /// 약한 SSAO (성능 우선)
    pub fn low_quality() -> Self {
        Self {
            sample_count: 8,
            blur_passes: 1,
            intensity: 0.8,
            ..Default::default()
        }
    }

    /// 강한 SSAO (품질 우선)
    pub fn high_quality() -> Self {
        Self {
            sample_count: 32,
            blur_passes: 3,
            intensity: 1.2,
            ..Default::default()
        }
    }

    /// 미세한 디테일용
    pub fn fine_detail() -> Self {
        Self {
            radius: 0.2,
            sample_count: 24,
            ..Default::default()
        }
    }
}

/// SSAO 파이프라인
pub struct SSAOPipeline {
    pub ssao_pipeline: wgpu::ComputePipeline,
    pub blur_pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub blur_bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,

    /// 노이즈 텍스처 (샘플링 방향)
    pub noise_texture: wgpu::Texture,
    pub noise_view: wgpu::TextureView,

    /// SSAO 결과 (블러 전)
    pub raw_ao_texture: wgpu::Texture,
    pub raw_ao_view: wgpu::TextureView,

    /// 블러된 최종 결과
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
}

impl SSAOPipeline {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, screen_size: (u32, u32)) -> Self {
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSAO Params Buffer"),
            size: std::mem::size_of::<SSAOParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Noise texture (4x4 random vectors)
        let (noise_texture, noise_view) = Self::create_noise_texture(device, queue);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSAO Bind Group Layout"),
            entries: &[
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal texture
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
                // Noise texture
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
                // Output texture (R32Float - R8Unorm은 storage 미지원)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
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

        let blur_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSAO Blur Bind Group Layout"),
            entries: &[
                // Input AO
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
                // Depth (for edge-aware blur)
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
                // Output (R32Float - R8Unorm은 storage 미지원)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSAO Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ssao.wgsl").into()),
        });

        let ssao_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let ssao_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSAO Pipeline"),
            layout: Some(&ssao_pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Blur Pipeline Layout"),
            bind_group_layouts: &[&blur_bind_group_layout],
            push_constant_ranges: &[],
        });

        let blur_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SSAO Blur Pipeline"),
            layout: Some(&blur_pipeline_layout),
            module: &shader,
            entry_point: Some("blur_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Raw AO texture
        let raw_ao_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Raw AO Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R8Unorm은 storage 미지원
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let raw_ao_view = raw_ao_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Output texture
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Output Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R8Unorm은 storage 미지원
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            ssao_pipeline,
            blur_pipeline,
            bind_group_layout,
            blur_bind_group_layout,
            params_buffer,
            noise_texture,
            noise_view,
            raw_ao_texture,
            raw_ao_view,
            output_texture,
            output_view,
            screen_size,
        }
    }

    fn create_noise_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
        let noise_size = 4;
        let mut noise_data = Vec::with_capacity((noise_size * noise_size * 4) as usize);

        // Generate random tangent-space vectors
        use std::f32::consts::PI;
        for i in 0..(noise_size * noise_size) {
            let angle = (i as f32 / (noise_size * noise_size) as f32) * 2.0 * PI;
            let x = angle.cos();
            let y = angle.sin();
            noise_data.push(((x * 0.5 + 0.5) * 255.0) as u8);
            noise_data.push(((y * 0.5 + 0.5) * 255.0) as u8);
            noise_data.push(0u8);  // z = 0 for tangent space
            noise_data.push(255u8);
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Noise Texture"),
            size: wgpu::Extent3d {
                width: noise_size,
                height: noise_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
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
            &noise_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(noise_size * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: noise_size,
                height: noise_size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &SSAOParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.raw_ao_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Raw AO Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R8Unorm은 storage 미지원
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.raw_ao_view = self.raw_ao_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSAO Output Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,  // R8Unorm은 storage 미지원
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }

    /// SSAO 효과 실행
    ///
    /// depth_view: 깊이 텍스처
    /// normal_view: 월드 노말 텍스처
    pub fn execute(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
    ) {
        // SSAO 패스 바인드 그룹
        let ssao_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.raw_ao_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        // SSAO 계산
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSAO Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.ssao_pipeline);
            compute_pass.set_bind_group(0, &ssao_bind_group, &[]);

            let workgroups_x = (self.screen_size.0 + 7) / 8;
            let workgroups_y = (self.screen_size.1 + 7) / 8;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        // 블러 패스 바인드 그룹
        let blur_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Blur Bind Group"),
            layout: &self.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.raw_ao_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        // Edge-aware 블러
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SSAO Blur Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.blur_pipeline);
            compute_pass.set_bind_group(0, &blur_bind_group, &[]);

            let workgroups_x = (self.screen_size.0 + 7) / 8;
            let workgroups_y = (self.screen_size.1 + 7) / 8;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }
    }
}
