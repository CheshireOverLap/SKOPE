// SKOPE Engine - Image Based Lighting (IBL)
// Split-Sum Approximation: Prefiltered Environment + BRDF LUT

use glam::Vec3;
use std::f32::consts::PI;

/// IBL 환경맵 시스템
pub struct IBLEnvironment {
    // Prefiltered specular cubemap (mip chain = roughness)
    prefiltered_cube: wgpu::Texture,
    prefiltered_view: wgpu::TextureView,
    cube_size: u32,
    mip_levels: u32,

    // Diffuse irradiance cubemap
    irradiance_cube: wgpu::Texture,
    irradiance_view: wgpu::TextureView,

    // BRDF LUT
    brdf_lut: wgpu::Texture,
    brdf_lut_view: wgpu::TextureView,

    // Sampler
    sampler: wgpu::Sampler,

    // Bind group
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // Environment intensity
    intensity: f32,
    rotation: f32,
}

impl IBLEnvironment {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, cube_size: u32) -> Self {
        let mip_levels = (cube_size as f32).log2() as u32;

        // Prefiltered specular cubemap
        let prefiltered_cube = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("IBL Prefiltered Cubemap"),
            size: wgpu::Extent3d {
                width: cube_size,
                height: cube_size,
                depth_or_array_layers: 6,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let prefiltered_view = prefiltered_cube.create_view(&wgpu::TextureViewDescriptor {
            label: Some("IBL Prefiltered View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        // Irradiance cubemap (low resolution)
        let irradiance_size = 64;
        let irradiance_cube = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("IBL Irradiance Cubemap"),
            size: wgpu::Extent3d {
                width: irradiance_size,
                height: irradiance_size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let irradiance_view = irradiance_cube.create_view(&wgpu::TextureViewDescriptor {
            label: Some("IBL Irradiance View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        // BRDF LUT (512x512)
        let brdf_lut = crate::brdf::create_brdf_lut_texture(device, queue, 512);
        let brdf_lut_view = brdf_lut.create_view(&wgpu::TextureViewDescriptor::default());

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("IBL Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("IBL Bind Group Layout"),
            entries: &[
                // Prefiltered specular
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // Irradiance
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // BRDF LUT
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("IBL Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&prefiltered_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&irradiance_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&brdf_lut_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            prefiltered_cube,
            prefiltered_view,
            cube_size,
            mip_levels,
            irradiance_cube,
            irradiance_view,
            brdf_lut,
            brdf_lut_view,
            sampler,
            bind_group_layout,
            bind_group,
            intensity: 1.0,
            rotation: 0.0,
        }
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity;
    }

    pub fn set_rotation(&mut self, rotation_radians: f32) {
        self.rotation = rotation_radians;
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn intensity(&self) -> f32 {
        self.intensity
    }

    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    pub fn mip_levels(&self) -> u32 {
        self.mip_levels
    }

    /// HDR 이퀴렉탱귤러 맵에서 큐브맵 생성
    pub fn from_equirectangular(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hdr_data: &[f32],
        width: u32,
        height: u32,
        cube_size: u32,
    ) -> Self {
        let ibl = Self::new(device, queue, cube_size);

        // Equirectangular to cubemap conversion (CPU fallback)
        // 실제로는 GPU compute shader로 처리
        let cubemap_data = equirect_to_cubemap_cpu(hdr_data, width, height, cube_size);

        for (face, face_data) in cubemap_data.iter().enumerate() {
            let byte_data: Vec<u8> = face_data.iter().flat_map(|v| {
                let r = half::f16::from_f32(v.x).to_bits().to_le_bytes();
                let g = half::f16::from_f32(v.y).to_bits().to_le_bytes();
                let b = half::f16::from_f32(v.z).to_bits().to_le_bytes();
                let a = half::f16::from_f32(1.0).to_bits().to_le_bytes();
                [r[0], r[1], g[0], g[1], b[0], b[1], a[0], a[1]]
            }).collect();

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &ibl.prefiltered_cube,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &byte_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(cube_size * 8),
                    rows_per_image: Some(cube_size),
                },
                wgpu::Extent3d {
                    width: cube_size,
                    height: cube_size,
                    depth_or_array_layers: 1,
                },
            );
        }

        // Generate prefiltered mips and irradiance
        // (실제로는 compute shader로 처리)

        ibl
    }
}

/// Equirectangular to Cubemap (CPU fallback)
fn equirect_to_cubemap_cpu(
    hdr_data: &[f32],
    width: u32,
    height: u32,
    cube_size: u32,
) -> [Vec<Vec3>; 6] {
    let mut faces: [Vec<Vec3>; 6] = [
        vec![Vec3::ZERO; (cube_size * cube_size) as usize],
        vec![Vec3::ZERO; (cube_size * cube_size) as usize],
        vec![Vec3::ZERO; (cube_size * cube_size) as usize],
        vec![Vec3::ZERO; (cube_size * cube_size) as usize],
        vec![Vec3::ZERO; (cube_size * cube_size) as usize],
        vec![Vec3::ZERO; (cube_size * cube_size) as usize],
    ];

    let face_directions: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::NEG_Y, Vec3::NEG_Z),   // +X
        (Vec3::NEG_X, Vec3::NEG_Y, Vec3::Z),   // -X
        (Vec3::Y, Vec3::Z, Vec3::X),           // +Y
        (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X),   // -Y
        (Vec3::Z, Vec3::NEG_Y, Vec3::X),       // +Z
        (Vec3::NEG_Z, Vec3::NEG_Y, Vec3::NEG_X), // -Z
    ];

    for (face_idx, face_data) in faces.iter_mut().enumerate() {
        let (forward, up, right) = face_directions[face_idx];

        for y in 0..cube_size {
            for x in 0..cube_size {
                let u = (x as f32 + 0.5) / cube_size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / cube_size as f32 * 2.0 - 1.0;

                let dir = (forward + right * u + up * v).normalize();

                // Direction to equirectangular UV
                let phi = dir.z.atan2(dir.x);
                let theta = dir.y.asin();

                let eq_u = (phi / PI * 0.5 + 0.5).fract();
                let eq_v = (theta / PI + 0.5).clamp(0.0, 1.0);

                let px = (eq_u * width as f32) as usize % width as usize;
                let py = (eq_v * height as f32) as usize % height as usize;

                let idx = (py * width as usize + px) * 3;
                if idx + 2 < hdr_data.len() {
                    face_data[(y * cube_size + x) as usize] = Vec3::new(
                        hdr_data[idx],
                        hdr_data[idx + 1],
                        hdr_data[idx + 2],
                    );
                }
            }
        }
    }

    faces
}

/// IBL Prefilter 컴퓨트 파이프라인
pub struct IBLPrefilter {
    prefilter_pipeline: wgpu::ComputePipeline,
    irradiance_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl IBLPrefilter {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("IBL Prefilter Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ibl_prefilter.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("IBL Prefilter Bind Group Layout"),
            entries: &[
                // Input cubemap
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // Output mip
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("IBL Prefilter Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let prefilter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("IBL Prefilter Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("prefilter_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let irradiance_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("IBL Irradiance Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("irradiance_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            prefilter_pipeline,
            irradiance_pipeline,
            bind_group_layout,
        }
    }
}

/// GPU용 IBL 파라미터
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IBLParams {
    pub intensity: f32,
    pub rotation: f32,
    pub max_mip_level: f32,
    pub _pad: f32,
}
