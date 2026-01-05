//! Scale Gizmo 구현
//!
//! XYZ 축 큐브 핸들 + 중앙 균일 스케일

use super::GizmoAxis;
use crate::editor::scene_viewer::{EditorCamera, Ray};
use glam::{Mat4, Quat, Vec3};

/// Gizmo 버텍스
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GizmoVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

impl GizmoVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position
        1 => Float32x3,  // normal
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GizmoVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Gizmo 유니폼
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GizmoUniforms {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
    hover_color: [f32; 4],
    is_hovered: u32,
    _padding: [u32; 3],
}

/// 축별 메시 범위
struct AxisMeshRange {
    vertex_start: u32,
    #[allow(dead_code)]
    vertex_count: u32,
    index_start: u32,
    index_count: u32,
}

/// Scale Gizmo
pub struct ScaleGizmo {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // 메시 범위 (X, Y, Z, Center)
    axis_ranges: [AxisMeshRange; 4],

    // 상태
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: f32,

    // 인터랙션
    pub hovered_axis: GizmoAxis,
    pub active_axis: GizmoAxis,
    drag_start_scale: Vec3,
}

impl ScaleGizmo {
    const SHAFT_LENGTH: f32 = 0.7;
    const SHAFT_RADIUS: f32 = 0.015;
    const CUBE_SIZE: f32 = 0.08;
    const CENTER_SIZE: f32 = 0.12;

    /// 새 ScaleGizmo 생성
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        // 메시 생성
        let (vertices, indices, axis_ranges) = Self::create_mesh();

        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Scale Gizmo Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gizmo.wgsl").into()),
        });

        // 바인드 그룹 레이아웃
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scale Gizmo Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Scale Gizmo Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Scale Gizmo Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[GizmoVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 버텍스 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scale Gizmo Vertex Buffer"),
            size: (vertices.len() * std::mem::size_of::<GizmoVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        vertex_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytemuck::cast_slice(&vertices));
        vertex_buffer.unmap();

        // 인덱스 버퍼
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scale Gizmo Index Buffer"),
            size: (indices.len() * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        index_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytemuck::cast_slice(&indices));
        index_buffer.unmap();

        // 유니폼 버퍼
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scale Gizmo Uniform Buffer"),
            size: std::mem::size_of::<GizmoUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 바인드 그룹
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scale Gizmo Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            axis_ranges,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: 1.0,
            hovered_axis: GizmoAxis::None,
            active_axis: GizmoAxis::None,
            drag_start_scale: Vec3::ONE,
        }
    }

    /// 메시 생성 (라인 + 큐브 핸들)
    fn create_mesh() -> (Vec<GizmoVertex>, Vec<u32>, [AxisMeshRange; 4]) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut ranges = [
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
        ];

        // X축 (샤프트 + 큐브)
        ranges[0] = Self::add_axis_with_cube(&mut vertices, &mut indices, Vec3::X, Vec3::Y);

        // Y축
        ranges[1] = Self::add_axis_with_cube(&mut vertices, &mut indices, Vec3::Y, Vec3::X);

        // Z축
        ranges[2] = Self::add_axis_with_cube(&mut vertices, &mut indices, Vec3::Z, Vec3::Y);

        // 중앙 큐브 (균일 스케일)
        ranges[3] = Self::add_center_cube(&mut vertices, &mut indices);

        (vertices, indices, ranges)
    }

    /// 축 + 끝 큐브 메시 추가
    fn add_axis_with_cube(
        vertices: &mut Vec<GizmoVertex>,
        indices: &mut Vec<u32>,
        direction: Vec3,
        up: Vec3,
    ) -> AxisMeshRange {
        let vertex_start = vertices.len() as u32;
        let index_start = indices.len() as u32;

        let shaft_length = Self::SHAFT_LENGTH;
        let shaft_radius = Self::SHAFT_RADIUS;
        let cube_size = Self::CUBE_SIZE;
        let segments = 8;

        let right = direction.cross(up).normalize();
        let up = right.cross(direction).normalize();

        // 원기둥 (shaft)
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            let normal = right * cos_a + up * sin_a;

            // 시작점
            let pos_start = normal * shaft_radius;
            vertices.push(GizmoVertex {
                position: pos_start.to_array(),
                normal: normal.to_array(),
            });

            // 끝점
            let pos_end = direction * shaft_length + normal * shaft_radius;
            vertices.push(GizmoVertex {
                position: pos_end.to_array(),
                normal: normal.to_array(),
            });
        }

        // 원기둥 인덱스
        for i in 0..segments {
            let base = vertex_start + i * 2;
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);

            indices.push(base + 1);
            indices.push(base + 3);
            indices.push(base + 2);
        }

        // 끝 큐브
        let cube_center = direction * (shaft_length + cube_size / 2.0);
        Self::add_cube(vertices, indices, cube_center, cube_size);

        let vertex_count = vertices.len() as u32 - vertex_start;
        let index_count = indices.len() as u32 - index_start;

        AxisMeshRange {
            vertex_start,
            vertex_count,
            index_start,
            index_count,
        }
    }

    /// 중앙 큐브 추가
    fn add_center_cube(
        vertices: &mut Vec<GizmoVertex>,
        indices: &mut Vec<u32>,
    ) -> AxisMeshRange {
        let vertex_start = vertices.len() as u32;
        let index_start = indices.len() as u32;

        Self::add_cube(vertices, indices, Vec3::ZERO, Self::CENTER_SIZE);

        let vertex_count = vertices.len() as u32 - vertex_start;
        let index_count = indices.len() as u32 - index_start;

        AxisMeshRange {
            vertex_start,
            vertex_count,
            index_start,
            index_count,
        }
    }

    /// 큐브 메시 추가
    fn add_cube(
        vertices: &mut Vec<GizmoVertex>,
        indices: &mut Vec<u32>,
        center: Vec3,
        size: f32,
    ) {
        let half = size / 2.0;
        let v_start = vertices.len() as u32;

        // 6면 x 4정점 = 24정점
        let faces = [
            // +X
            ([half, -half, -half], [half, half, -half], [half, half, half], [half, -half, half], Vec3::X),
            // -X
            ([-half, -half, half], [-half, half, half], [-half, half, -half], [-half, -half, -half], Vec3::NEG_X),
            // +Y
            ([-half, half, -half], [-half, half, half], [half, half, half], [half, half, -half], Vec3::Y),
            // -Y
            ([-half, -half, half], [-half, -half, -half], [half, -half, -half], [half, -half, half], Vec3::NEG_Y),
            // +Z
            ([-half, -half, half], [half, -half, half], [half, half, half], [-half, half, half], Vec3::Z),
            // -Z
            ([half, -half, -half], [-half, -half, -half], [-half, half, -half], [half, half, -half], Vec3::NEG_Z),
        ];

        for (v0, v1, v2, v3, normal) in faces {
            let base = vertices.len() as u32;

            vertices.push(GizmoVertex {
                position: [center.x + v0[0], center.y + v0[1], center.z + v0[2]],
                normal: normal.to_array(),
            });
            vertices.push(GizmoVertex {
                position: [center.x + v1[0], center.y + v1[1], center.z + v1[2]],
                normal: normal.to_array(),
            });
            vertices.push(GizmoVertex {
                position: [center.x + v2[0], center.y + v2[1], center.z + v2[2]],
                normal: normal.to_array(),
            });
            vertices.push(GizmoVertex {
                position: [center.x + v3[0], center.y + v3[1], center.z + v3[2]],
                normal: normal.to_array(),
            });

            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        let _ = v_start; // suppress warning
    }

    /// Gizmo 위치 설정
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// 화면 크기 기반 스케일 업데이트
    pub fn update_scale(&mut self, camera: &EditorCamera, screen_size: (u32, u32)) {
        let camera_pos = camera.position;
        let distance = (self.position - camera_pos).length();
        let fov_factor = (camera.settings.fov / 2.0).tan();
        let screen_factor = screen_size.1 as f32 / 720.0;
        self.scale = distance * fov_factor * 0.15 / screen_factor;
    }

    /// Ray-Gizmo 충돌 검사
    pub fn pick(&self, ray: &Ray) -> GizmoAxis {
        let inv_scale = 1.0 / self.scale;
        let local_origin = (ray.origin - self.position) * inv_scale;
        let local_dir = ray.direction;

        let mut closest_axis = GizmoAxis::None;
        let mut closest_t = f32::MAX;

        // 중앙 큐브 검사 (우선)
        let center_half = Self::CENTER_SIZE / 2.0;
        if let Some(t) = Self::ray_aabb_intersection(
            local_origin,
            local_dir,
            Vec3::splat(-center_half),
            Vec3::splat(center_half),
        ) {
            if t > 0.0 && t < closest_t {
                closest_t = t;
                closest_axis = GizmoAxis::None; // 균일 스케일은 None으로 표시 (나중에 별도 처리)
            }
        }

        // 각 축 검사
        let axes = [
            (GizmoAxis::X, Vec3::X),
            (GizmoAxis::Y, Vec3::Y),
            (GizmoAxis::Z, Vec3::Z),
        ];

        for (axis, dir) in axes {
            // 큐브 핸들 위치
            let cube_center = dir * (Self::SHAFT_LENGTH + Self::CUBE_SIZE / 2.0);
            let cube_half = Self::CUBE_SIZE / 2.0;

            if let Some(t) = Self::ray_aabb_intersection(
                local_origin,
                local_dir,
                cube_center - Vec3::splat(cube_half),
                cube_center + Vec3::splat(cube_half),
            ) {
                if t > 0.0 && t < closest_t {
                    closest_t = t;
                    closest_axis = axis;
                }
            }

            // 샤프트도 검사
            let shaft_half = 0.04;
            let (min, max) = if dir.x > 0.5 {
                (Vec3::new(0.0, -shaft_half, -shaft_half), Vec3::new(Self::SHAFT_LENGTH, shaft_half, shaft_half))
            } else if dir.y > 0.5 {
                (Vec3::new(-shaft_half, 0.0, -shaft_half), Vec3::new(shaft_half, Self::SHAFT_LENGTH, shaft_half))
            } else {
                (Vec3::new(-shaft_half, -shaft_half, 0.0), Vec3::new(shaft_half, shaft_half, Self::SHAFT_LENGTH))
            };

            if let Some(t) = Self::ray_aabb_intersection(local_origin, local_dir, min, max) {
                if t > 0.0 && t < closest_t {
                    closest_t = t;
                    closest_axis = axis;
                }
            }
        }

        closest_axis
    }

    /// Ray-AABB 교차 검사
    fn ray_aabb_intersection(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
        let inv_dir = Vec3::new(
            if dir.x.abs() > 0.0001 { 1.0 / dir.x } else { f32::MAX },
            if dir.y.abs() > 0.0001 { 1.0 / dir.y } else { f32::MAX },
            if dir.z.abs() > 0.0001 { 1.0 / dir.z } else { f32::MAX },
        );

        let t1 = (min - origin) * inv_dir;
        let t2 = (max - origin) * inv_dir;

        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        let tmin_max = tmin.x.max(tmin.y).max(tmin.z);
        let tmax_min = tmax.x.min(tmax.y).min(tmax.z);

        if tmin_max <= tmax_min && tmax_min >= 0.0 {
            Some(if tmin_max > 0.0 { tmin_max } else { tmax_min })
        } else {
            None
        }
    }

    /// 스케일 팩터 계산
    pub fn calculate_scale_factor(
        &self,
        camera: &EditorCamera,
        prev_pos: glam::Vec2,
        curr_pos: glam::Vec2,
        screen_size: glam::Vec2,
    ) -> Vec3 {
        let axis_dir = self.active_axis.direction();

        let prev_ray = camera.screen_to_ray(prev_pos, screen_size);
        let curr_ray = camera.screen_to_ray(curr_pos, screen_size);

        // 뷰 방향과 축의 교차 평면 찾기
        let view_dir = camera.forward();

        if axis_dir == Vec3::ZERO {
            // 균일 스케일: 마우스 Y 이동에 따라
            let delta_y = (curr_pos.y - prev_pos.y) / screen_size.y;
            let scale_factor = 1.0 + delta_y * 2.0;
            return Vec3::splat(scale_factor);
        }

        let plane_normal = view_dir.cross(axis_dir).cross(axis_dir).normalize();

        // 이전/현재 ray와 평면의 교차점
        let prev_t = self.ray_plane_intersection(&prev_ray, plane_normal, self.position);
        let curr_t = self.ray_plane_intersection(&curr_ray, plane_normal, self.position);

        if let (Some(prev_t), Some(curr_t)) = (prev_t, curr_t) {
            let prev_hit = prev_ray.origin + prev_ray.direction * prev_t;
            let curr_hit = curr_ray.origin + curr_ray.direction * curr_t;

            // 중심에서의 거리 변화로 스케일 계산
            let prev_dist = (prev_hit - self.position).dot(axis_dir);
            let curr_dist = (curr_hit - self.position).dot(axis_dir);

            if prev_dist.abs() > 0.01 {
                let scale_ratio = curr_dist / prev_dist;
                // 축 방향으로만 스케일
                return Vec3::ONE + axis_dir * (scale_ratio - 1.0);
            }
        }

        Vec3::ONE
    }

    /// Ray-Plane 교차점
    fn ray_plane_intersection(&self, ray: &Ray, plane_normal: Vec3, plane_point: Vec3) -> Option<f32> {
        let denom = ray.direction.dot(plane_normal);
        if denom.abs() > 0.0001 {
            let t = (plane_point - ray.origin).dot(plane_normal) / denom;
            if t > 0.0 {
                return Some(t);
            }
        }
        None
    }

    /// 드래그 시작
    pub fn begin_drag(&mut self, axis: GizmoAxis, current_scale: Vec3) {
        self.active_axis = axis;
        self.drag_start_scale = current_scale;
    }

    /// 드래그 종료
    pub fn end_drag(&mut self) {
        self.active_axis = GizmoAxis::None;
    }

    /// Gizmo 렌더링
    pub fn render(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        camera: &EditorCamera,
        aspect: f32,
    ) {
        let view_proj = camera.view_projection_matrix(aspect);
        let model = Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            self.rotation,
            self.position,
        );

        // X, Y, Z + Center
        let axes_colors: [(GizmoAxis, usize, [f32; 4], [f32; 4]); 4] = [
            (GizmoAxis::X, 0, GizmoAxis::X.color(), GizmoAxis::X.hover_color()),
            (GizmoAxis::Y, 1, GizmoAxis::Y.color(), GizmoAxis::Y.hover_color()),
            (GizmoAxis::Z, 2, GizmoAxis::Z.color(), GizmoAxis::Z.hover_color()),
            (GizmoAxis::None, 3, [0.9, 0.9, 0.9, 1.0], [1.0, 1.0, 1.0, 1.0]), // 중앙 흰색
        ];

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Scale Gizmo Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for (axis, range_idx, color, hover_color) in axes_colors {
            let range = &self.axis_ranges[range_idx];
            let is_hovered = self.hovered_axis == axis || self.active_axis == axis;

            let uniforms = GizmoUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                model: model.to_cols_array_2d(),
                color,
                hover_color,
                is_hovered: if is_hovered { 1 } else { 0 },
                _padding: [0; 3],
            };

            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw_indexed(
                range.index_start..(range.index_start + range.index_count),
                range.vertex_start as i32,
                0..1,
            );
        }
    }
}
