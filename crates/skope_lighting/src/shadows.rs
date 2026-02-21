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
    pub cascade_split_lambda: f32,  // Logarithmic/Linear blend (기존 방식)
    /// UE5 기하급수 분포 지수 (Some이면 UE5 방식 사용, None이면 기존 lambda 방식)
    pub distribution_exponent: Option<f32>,
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
            distribution_exponent: None, // 기본: 기존 lambda 방식
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

/// UE5 ComputeAccumulatedScale 기하급수 캐스케이드 분할.
///
/// UE5에서는 로그/선형 혼합 대신 기하급수 분포를 사용:
/// `accumulated_scale[i] = (1 - exponent^i) / (1 - exponent^N)`
/// `split[i] = near + (far - near) * accumulated_scale[i]`
///
/// `exponent` 값이 클수록 가까운 캐스케이드에 더 많은 해상도가 배분됩니다.
pub fn compute_cascade_splits_ue5(
    exponent: f32,
    cascade_count: u32,
    near: f32,
    far: f32,
) -> Vec<f32> {
    let n = cascade_count as usize;
    let mut splits = Vec::with_capacity(n + 1);
    splits.push(near);

    let range = far - near;

    if (exponent - 1.0).abs() < 1e-6 {
        // Linear fallback when exponent ≈ 1
        for i in 1..=n {
            let t = i as f32 / n as f32;
            splits.push(near + range * t);
        }
    } else {
        // Geometric distribution
        let denom = 1.0 - exponent.powi(n as i32);
        for i in 1..=n {
            let accumulated = (1.0 - exponent.powi(i as i32)) / denom;
            splits.push(near + range * accumulated);
        }
    }

    splits
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

/// Model uniform for shadow depth pass (replaces push constants)
/// Size: 96 bytes (WGSL vec3 has 16-byte alignment)
/// Layout:
///   model (mat4x4<f32>): offset 0, size 64
///   cascade_index (u32): offset 64, size 4
///   _pad1 ([u32;3]): offset 68, size 12 (align vec3 to 16-byte boundary)
///   _pad2 ([u32;3]): offset 80, size 12 (vec3<u32> in WGSL)
///   _pad3 (u32): offset 92, size 4 (align struct size to 16)
///   Total: 96 bytes
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowModelUniform {
    pub model: [[f32; 4]; 4],  // 64 bytes, offset 0
    pub cascade_index: u32,     // 4 bytes, offset 64
    pub _pad1: [u32; 3],        // 12 bytes, offset 68
    pub _pad2: [u32; 3],        // 12 bytes, offset 80 (vec3<u32>)
    pub _pad3: u32,             // 4 bytes, offset 92
}

impl ShadowModelUniform {
    pub fn new(model: Mat4, cascade_index: u32) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            cascade_index,
            _pad1: [0; 3],
            _pad2: [0; 3],
            _pad3: 0,
        }
    }
}

pub struct CascadedShadowMap {
    config: CascadedShadowConfig,
    shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    cascade_views: Vec<wgpu::TextureView>,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,  // For lighting pass (sampling shadows)
    // Depth rendering resources
    depth_pipeline: wgpu::RenderPipeline,
    depth_bind_group_layout: wgpu::BindGroupLayout,  // For depth pass (uniforms only)
    depth_bind_group: wgpu::BindGroup,               // For depth pass (uniforms only)
    model_buffer: wgpu::Buffer,
    model_bind_group_layout: wgpu::BindGroupLayout,
    model_bind_group: wgpu::BindGroup,
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

        // Uniform buffer (also STORAGE for material_eval compute shader binding)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Uniforms"),
            size: std::mem::size_of::<ShadowUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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

        // Model uniform buffer (for depth pass - replaces push constants)
        let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Model Uniform"),
            size: std::mem::size_of::<ShadowModelUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Model bind group layout (Group 1 for depth pass)
        let model_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Model Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Model Bind Group"),
            layout: &model_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_buffer.as_entire_binding(),
                },
            ],
        });

        // Depth-only pipeline (using uniform buffer instead of push constants)
        let depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow_depth.wgsl").into()),
        });

        // Depth pass bind group layout - ONLY uniforms, no shadow texture
        // This avoids texture usage conflicts (can't sample and write simultaneously)
        let depth_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Depth Bind Group Layout"),
            entries: &[
                // Uniforms only (binding 2 to match shader)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Depth Bind Group"),
            layout: &depth_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let depth_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[&depth_bind_group_layout, &model_bind_group_layout],
            immediate_size: 0,  // No push constants needed
        });

        let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Depth Pipeline"),
            layout: Some(&depth_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &depth_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 64,  // GpuVertex stride (pos + pad + normal + pad + tangent + uv + pad)
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,  // Position is at offset 0
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
            multiview_mask: None,
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
            depth_bind_group_layout,
            depth_bind_group,
            model_buffer,
            model_bind_group_layout,
            model_bind_group,
            sampler,
        }
    }

    /// 캐스케이드 분할 계산.
    /// `distribution_exponent`가 설정되어 있으면 UE5 기하급수 방식,
    /// 아니면 기존 Logarithmic + Linear 혼합 방식 사용.
    pub fn calculate_cascade_splits(&self, near: f32, far: f32) -> Vec<f32> {
        let range = far.min(self.config.max_distance);

        if let Some(exponent) = self.config.distribution_exponent {
            compute_cascade_splits_ue5(exponent, self.config.cascade_count, near, range)
        } else {
            // Legacy: Logarithmic + Linear blend
            let mut splits = Vec::with_capacity(self.config.cascade_count as usize + 1);
            splits.push(near);

            let ratio = range / near;
            let lambda = self.config.cascade_split_lambda;

            for i in 1..=self.config.cascade_count {
                let p = i as f32 / self.config.cascade_count as f32;
                let log_split = near * ratio.powf(p);
                let linear_split = near + (range - near) * p;
                let split = lambda * log_split + (1.0 - lambda) * linear_split;
                splits.push(split);
            }

            splits
        }
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

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn uniform_buffer(&self) -> &wgpu::Buffer {
        &self.uniform_buffer
    }

    /// Render shadow maps for all cascades using uniform buffers
    ///
    /// This renders each mesh to each cascade's shadow map.
    /// Uses staging buffer approach: collect all draw data, then render.
    ///
    /// Note: For simplicity, this assumes identity model matrices for all shadow casters.
    /// For proper per-mesh transforms, use render_shadows_with_staging().
    pub fn render_shadows_simple(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        meshes: &[(&wgpu::Buffer, &wgpu::Buffer, u32)],  // (vertex_buffer, index_buffer, index_count)
    ) {
        for cascade_idx in 0..self.config.cascade_count as usize {
            // Update model uniform for this cascade (identity matrix)
            let model_uniform = ShadowModelUniform::new(Mat4::IDENTITY, cascade_idx as u32);
            queue.write_buffer(&self.model_buffer, 0, bytemuck::bytes_of(&model_uniform));

            let cascade_view = &self.cascade_views[cascade_idx];

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("Shadow Pass Cascade {}", cascade_idx)),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: cascade_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.depth_pipeline);
            pass.set_bind_group(0, &self.depth_bind_group, &[]);  // Uniforms only, no shadow texture
            pass.set_bind_group(1, &self.model_bind_group, &[]);

            for (vertex_buffer, index_buffer, index_count) in meshes {
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*index_count, 0, 0..1);
            }
        }
    }

    /// Render shadow maps with per-mesh transforms
    ///
    /// This uses multiple render passes - one per (cascade, mesh) pair.
    /// Less efficient but supports arbitrary model matrices.
    pub fn render_shadows(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        meshes: &[(Mat4, &wgpu::Buffer, &wgpu::Buffer, u32)],  // (model_matrix, vertex_buffer, index_buffer, index_count)
    ) {
        for cascade_idx in 0..self.config.cascade_count as usize {
            let cascade_view = &self.cascade_views[cascade_idx];

            for (mesh_idx, (model_matrix, vertex_buffer, index_buffer, index_count)) in meshes.iter().enumerate() {
                // Update model uniform before starting render pass
                let model_uniform = ShadowModelUniform::new(*model_matrix, cascade_idx as u32);
                queue.write_buffer(&self.model_buffer, 0, bytemuck::bytes_of(&model_uniform));

                // Start render pass for this mesh
                let load_op = if mesh_idx == 0 {
                    wgpu::LoadOp::Clear(1.0)
                } else {
                    wgpu::LoadOp::Load
                };

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("Shadow Pass C{} M{}", cascade_idx, mesh_idx)),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: cascade_view,
                        depth_ops: Some(wgpu::Operations {
                            load: load_op,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });

                pass.set_pipeline(&self.depth_pipeline);
                pass.set_bind_group(0, &self.depth_bind_group, &[]);  // Uniforms only, no shadow texture
                pass.set_bind_group(1, &self.model_bind_group, &[]);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*index_count, 0, 0..1);
            }
        }
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
