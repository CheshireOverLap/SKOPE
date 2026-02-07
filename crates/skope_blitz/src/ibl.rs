// SKOPE Engine - Image Based Lighting (IBL)
// Split-Sum Approximation: Prefiltered Environment + BRDF LUT

use glam::Vec3;
use std::f32::consts::PI;
use wgpu::util::DeviceExt;

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
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
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

    pub fn prefiltered_view(&self) -> &wgpu::TextureView {
        &self.prefiltered_view
    }

    pub fn irradiance_view(&self) -> &wgpu::TextureView {
        &self.irradiance_view
    }

    pub fn brdf_lut_view(&self) -> &wgpu::TextureView {
        &self.brdf_lut_view
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
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

    /// Load HDR environment map from file path
    #[cfg(feature = "gpu")]
    pub fn load_hdr(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hdr_path: &std::path::Path,
        cube_size: u32,
    ) -> Result<Self, String> {
        use image::ImageReader;

        let img = ImageReader::open(hdr_path)
            .map_err(|e| format!("Failed to open HDR file {:?}: {}", hdr_path, e))?
            .decode()
            .map_err(|e| format!("Failed to decode HDR file {:?}: {}", hdr_path, e))?;

        let width = img.width();
        let height = img.height();

        // Convert to Rgb32F for HDR data
        let rgb32f = img.into_rgb32f();
        let hdr_data: &[f32] = bytemuck::cast_slice(rgb32f.as_raw());

        log::info!("[IBL] Loaded HDR {:?} ({}x{}), converting to {}x{} cubemap",
            hdr_path, width, height, cube_size, cube_size);

        let ibl = Self::from_equirectangular(device, queue, hdr_data, width, height, cube_size);
        Ok(ibl)
    }

    /// Get prefiltered cubemap texture (for external prefilter dispatch)
    pub fn prefiltered_texture(&self) -> &wgpu::Texture {
        &self.prefiltered_cube
    }

    /// Get irradiance cubemap texture (for external prefilter dispatch)
    pub fn irradiance_texture(&self) -> &wgpu::Texture {
        &self.irradiance_cube
    }

    /// Get cube size
    pub fn cube_size(&self) -> u32 {
        self.cube_size
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
            immediate_size: 0,
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

    /// Dispatch prefilter compute passes for specular mip chain + irradiance convolution
    pub fn prefilter(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ibl: &IBLEnvironment,
    ) {
        let cube_size = ibl.cube_size();
        let mip_levels = ibl.mip_levels();
        let sampler = ibl.sampler();

        // Create cube view for input (mip 0 as source)
        let input_view = ibl.prefiltered_texture().create_view(&wgpu::TextureViewDescriptor {
            label: Some("IBL Prefilter Input View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("IBL Prefilter Encoder"),
        });

        // Specular prefilter: one dispatch per mip level (mip 1 .. mip_levels-1)
        // mip 0 is the original environment map (already uploaded)
        for mip in 1..mip_levels {
            let mip_size = (cube_size >> mip).max(1);
            let roughness = mip as f32 / (mip_levels - 1) as f32;

            // Create per-mip output view (2D array of 6 faces for this mip level)
            let output_view = ibl.prefiltered_texture().create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("IBL Prefilter Output Mip {}", mip)),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                base_mip_level: mip,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(6),
                ..Default::default()
            });

            // Params uniform: roughness + mip info
            #[repr(C)]
            #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
            struct PrefilterParams {
                roughness: f32,
                resolution: f32,
                num_samples: u32,
                _pad: u32,
            }

            let params = PrefilterParams {
                roughness,
                resolution: mip_size as f32,
                num_samples: 1024,
                _pad: 0,
            };

            let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("IBL Prefilter Params Mip {}", mip)),
                contents: bytemuck::cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("IBL Prefilter BG Mip {}", mip)),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&output_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(&format!("IBL Prefilter Pass Mip {}", mip)),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.prefilter_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                // Dispatch: each workgroup handles 8x8 texels, 6 faces in z
                let groups_x = (mip_size + 7) / 8;
                let groups_y = (mip_size + 7) / 8;
                pass.dispatch_workgroups(groups_x, groups_y, 6);
            }
        }

        // Irradiance convolution: dispatch into irradiance cubemap
        {
            let irr_view = ibl.irradiance_texture().create_view(&wgpu::TextureViewDescriptor {
                label: Some("IBL Irradiance Output View"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(6),
                ..Default::default()
            });

            #[repr(C)]
            #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
            struct IrradianceParams {
                roughness: f32,
                resolution: f32,
                num_samples: u32,
                _pad: u32,
            }

            let params = IrradianceParams {
                roughness: 1.0, // not used for irradiance, but keep struct compatible
                resolution: 64.0,
                num_samples: 2048,
                _pad: 0,
            };

            let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("IBL Irradiance Params"),
                contents: bytemuck::cast_slice(&[params]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("IBL Irradiance BG"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&irr_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("IBL Irradiance Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.irradiance_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                let groups_x = (64 + 7) / 8;
                let groups_y = (64 + 7) / 8;
                pass.dispatch_workgroups(groups_x, groups_y, 6);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        log::info!("[IBL] Prefilter dispatch complete ({} mip levels + irradiance)", mip_levels);
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
