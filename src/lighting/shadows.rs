// SKOPE Engine - Shadow System
// Cascaded Shadow Maps (CSM) + PCSS Soft Shadows

use glam::{Vec3, Vec4, Mat4};
use bytemuck::{Pod, Zeroable};

/// CSM 설정
#[derive(Debug, Clone, Copy)]
pub struct CascadedShadowConfig {
    pub cascade_count: u32,
    pub shadow_map_size: u32,
    pub max_distance: f32,
    pub cascade_split_lambda: f32,  // Logarithmic/Linear blend
    pub depth_bias: f32,
    pub normal_bias: f32,
    pub pcf_radius: f32,
    pub pcss_enabled: bool,
    pub pcss_light_size: f32,
    pub pcss_blocker_search_samples: u32,
    pub pcss_pcf_samples: u32,
}

impl Default for CascadedShadowConfig {
    fn default() -> Self {
        Self {
            cascade_count: 4,
            shadow_map_size: 2048,
            max_distance: 100.0,
            cascade_split_lambda: 0.5,  // 로그/선형 혼합
            depth_bias: 0.001,
            normal_bias: 0.02,
            pcf_radius: 1.5,
            pcss_enabled: true,
            pcss_light_size: 0.02,  // 태양 각도 기반
            pcss_blocker_search_samples: 16,
            pcss_pcf_samples: 32,
        }
    }
}

/// 캐스케이드 정보
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CascadeData {
    pub view_proj: [[f32; 4]; 4],
    pub split_depth: f32,
    pub texel_size: f32,
    pub _pad: [f32; 2],
}

impl CascadeData {
    pub fn new(view_proj: Mat4, split_depth: f32, texel_size: f32) -> Self {
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            split_depth,
            texel_size,
            _pad: [0.0; 2],
        }
    }
}

/// GPU용 섀도우 유니폼
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowUniforms {
    pub cascades: [CascadeData; 4],
    pub cascade_count: u32,
    pub depth_bias: f32,
    pub normal_bias: f32,
    pub pcf_radius: f32,
    pub pcss_enabled: u32,
    pub pcss_light_size: f32,
    pub _pad: [f32; 2],
}

pub struct CascadedShadowMap {
    config: CascadedShadowConfig,
    shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    cascade_views: Vec<wgpu::TextureView>,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    depth_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl CascadedShadowMap {
    pub fn new(device: &wgpu::Device, config: CascadedShadowConfig) -> Self {
        // Shadow map texture array
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CSM Shadow Map"),
            size: wgpu::Extent3d {
                width: config.shadow_map_size,
                height: config.shadow_map_size,
                depth_or_array_layers: config.cascade_count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("CSM Shadow View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        // 각 캐스케이드별 뷰
        let cascade_views: Vec<_> = (0..config.cascade_count)
            .map(|i| {
                shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(&format!("Cascade {} View", i)),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        // Comparison sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniforms"),
            size: std::mem::size_of::<ShadowUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Bind Group Layout"),
            entries: &[
                // Shadow map
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Depth-only pipeline
        let depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow_depth.wgsl").into()),
        });

        let depth_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..64,  // Mat4 for object transform
            }],
        });

        let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Depth Pipeline"),
            layout: Some(&depth_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &depth_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 12,  // position only
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: Default::default(),
            },
            fragment: None,  // Depth only
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),  // Front-face culling for shadows
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            config,
            shadow_texture,
            shadow_view,
            cascade_views,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            depth_pipeline,
            sampler,
        }
    }

    /// 캐스케이드 분할 계산 (Logarithmic + Linear)
    pub fn calculate_cascade_splits(&self, near: f32, far: f32) -> Vec<f32> {
        let mut splits = Vec::with_capacity(self.config.cascade_count as usize + 1);
        splits.push(near);

        let range = far.min(self.config.max_distance);
        let ratio = range / near;
        let lambda = self.config.cascade_split_lambda;

        for i in 1..=self.config.cascade_count {
            let p = i as f32 / self.config.cascade_count as f32;

            // 로그 분할
            let log_split = near * ratio.powf(p);

            // 선형 분할
            let linear_split = near + (range - near) * p;

            // 혼합
            let split = lambda * log_split + (1.0 - lambda) * linear_split;
            splits.push(split);
        }

        splits
    }

    /// 캐스케이드별 Light View-Projection 매트릭스 계산
    pub fn calculate_cascade_matrices(
        &self,
        camera_view: Mat4,
        camera_proj: Mat4,
        light_dir: Vec3,
        near: f32,
        far: f32,
    ) -> Vec<CascadeData> {
        let splits = self.calculate_cascade_splits(near, far);
        let inv_view_proj = (camera_proj * camera_view).inverse();

        let mut cascades = Vec::with_capacity(self.config.cascade_count as usize);

        for i in 0..self.config.cascade_count as usize {
            let near_split = splits[i];
            let far_split = splits[i + 1];

            // Frustum corners in world space
            let corners = frustum_corners_world(inv_view_proj, near_split, far_split, near, far);

            // Frustum center
            let center: Vec3 = corners.iter().sum::<Vec3>() / 8.0;

            // Light view matrix
            let light_view = Mat4::look_at_rh(
                center - light_dir * 50.0,  // Light 위치 (충분히 멀리)
                center,
                Vec3::Y,
            );

            // Frustum을 light space로 변환
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);

            for corner in &corners {
                let light_space = light_view.transform_point3(*corner);
                min = min.min(light_space);
                max = max.max(light_space);
            }

            // Texel snapping (jitter 방지)
            let texel_size = (max.x - min.x) / self.config.shadow_map_size as f32;
            min.x = (min.x / texel_size).floor() * texel_size;
            min.y = (min.y / texel_size).floor() * texel_size;
            max.x = (max.x / texel_size).ceil() * texel_size;
            max.y = (max.y / texel_size).ceil() * texel_size;

            // Orthographic projection
            let light_proj = Mat4::orthographic_rh(
                min.x, max.x,
                min.y, max.y,
                -max.z - 100.0, -min.z + 100.0,
            );

            cascades.push(CascadeData::new(
                light_proj * light_view,
                far_split,
                texel_size,
            ));
        }

        cascades
    }

    /// 유니폼 업데이트
    pub fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        cascades: &[CascadeData],
    ) {
        let mut cascade_array = [CascadeData::new(Mat4::IDENTITY, 0.0, 0.0); 4];

        for (i, cascade) in cascades.iter().enumerate().take(4) {
            cascade_array[i] = *cascade;
        }

        let uniforms = ShadowUniforms {
            cascades: cascade_array,
            cascade_count: self.config.cascade_count,
            depth_bias: self.config.depth_bias,
            normal_bias: self.config.normal_bias,
            pcf_radius: self.config.pcf_radius,
            pcss_enabled: if self.config.pcss_enabled { 1 } else { 0 },
            pcss_light_size: self.config.pcss_light_size,
            _pad: [0.0; 2],
        };

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );
    }

    pub fn shadow_view(&self) -> &wgpu::TextureView {
        &self.shadow_view
    }

    pub fn cascade_view(&self, index: usize) -> Option<&wgpu::TextureView> {
        self.cascade_views.get(index)
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn depth_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.depth_pipeline
    }

    pub fn config(&self) -> &CascadedShadowConfig {
        &self.config
    }
}

/// Frustum corners 계산
fn frustum_corners_world(
    inv_view_proj: Mat4,
    near_split: f32,
    far_split: f32,
    camera_near: f32,
    camera_far: f32,
) -> [Vec3; 8] {
    // NDC에서 near_split과 far_split에 해당하는 z값 계산
    // Reverse-Z를 사용하지 않는 경우
    let near_ndc = 2.0 * (near_split - camera_near) / (camera_far - camera_near) - 1.0;
    let far_ndc = 2.0 * (far_split - camera_near) / (camera_far - camera_near) - 1.0;

    let ndc_corners = [
        // Near plane
        Vec4::new(-1.0, -1.0, near_ndc, 1.0),
        Vec4::new( 1.0, -1.0, near_ndc, 1.0),
        Vec4::new( 1.0,  1.0, near_ndc, 1.0),
        Vec4::new(-1.0,  1.0, near_ndc, 1.0),
        // Far plane
        Vec4::new(-1.0, -1.0, far_ndc, 1.0),
        Vec4::new( 1.0, -1.0, far_ndc, 1.0),
        Vec4::new( 1.0,  1.0, far_ndc, 1.0),
        Vec4::new(-1.0,  1.0, far_ndc, 1.0),
    ];

    let mut world_corners = [Vec3::ZERO; 8];
    for (i, ndc) in ndc_corners.iter().enumerate() {
        let world = inv_view_proj * *ndc;
        world_corners[i] = world.truncate() / world.w;
    }

    world_corners
}

/// Point Light Shadow (Cubemap)
pub struct PointLightShadow {
    shadow_cubemap: wgpu::Texture,
    face_views: [wgpu::TextureView; 6],
    shadow_view: wgpu::TextureView,
    size: u32,
}

impl PointLightShadow {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let shadow_cubemap = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Point Light Shadow Cubemap"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let face_views = [
            create_cubemap_face_view(&shadow_cubemap, 0),
            create_cubemap_face_view(&shadow_cubemap, 1),
            create_cubemap_face_view(&shadow_cubemap, 2),
            create_cubemap_face_view(&shadow_cubemap, 3),
            create_cubemap_face_view(&shadow_cubemap, 4),
            create_cubemap_face_view(&shadow_cubemap, 5),
        ];

        let shadow_view = shadow_cubemap.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Point Light Shadow Cube View"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        Self {
            shadow_cubemap,
            face_views,
            shadow_view,
            size,
        }
    }

    /// 각 면의 View-Projection 매트릭스
    pub fn face_matrices(position: Vec3, near: f32, far: f32) -> [Mat4; 6] {
        let proj = Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_2,  // 90 degrees
            1.0,
            near,
            far,
        );

        let views = [
            Mat4::look_at_rh(position, position + Vec3::X, -Vec3::Y),   // +X
            Mat4::look_at_rh(position, position - Vec3::X, -Vec3::Y),   // -X
            Mat4::look_at_rh(position, position + Vec3::Y, Vec3::Z),    // +Y
            Mat4::look_at_rh(position, position - Vec3::Y, -Vec3::Z),   // -Y
            Mat4::look_at_rh(position, position + Vec3::Z, -Vec3::Y),   // +Z
            Mat4::look_at_rh(position, position - Vec3::Z, -Vec3::Y),   // -Z
        ];

        [
            proj * views[0],
            proj * views[1],
            proj * views[2],
            proj * views[3],
            proj * views[4],
            proj * views[5],
        ]
    }

    pub fn face_view(&self, face: usize) -> &wgpu::TextureView {
        &self.face_views[face]
    }

    pub fn shadow_view(&self) -> &wgpu::TextureView {
        &self.shadow_view
    }
}

fn create_cubemap_face_view(texture: &wgpu::Texture, face: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(&format!("Cubemap Face {} View", face)),
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: face,
        array_layer_count: Some(1),
        ..Default::default()
    })
}

/// Spot Light Shadow
pub struct SpotLightShadow {
    shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    view_proj: Mat4,
    size: u32,
}

impl SpotLightShadow {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Spot Light Shadow"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            shadow_texture,
            shadow_view,
            view_proj: Mat4::IDENTITY,
            size,
        }
    }

    pub fn calculate_matrix(
        &mut self,
        position: Vec3,
        direction: Vec3,
        outer_angle: f32,
        near: f32,
        far: f32,
    ) {
        let fov = outer_angle * 2.0;
        let proj = Mat4::perspective_rh(fov, 1.0, near, far);
        let up = if direction.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
        let view = Mat4::look_at_rh(position, position + direction, up);
        self.view_proj = proj * view;
    }

    pub fn shadow_view(&self) -> &wgpu::TextureView {
        &self.shadow_view
    }

    pub fn view_proj(&self) -> Mat4 {
        self.view_proj
    }
}
