//! Move Gizmo 구현
//!
//! XYZ 축 화살표 + XY/YZ/ZX 평면 핸들

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
    vertex_count: u32,
    index_start: u32,
    index_count: u32,
}

/// Move Gizmo
pub struct MoveGizmo {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // 메시 범위 (축별)
    axis_ranges: [AxisMeshRange; 6], // X, Y, Z, XY, YZ, ZX

    // 상태
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: f32, // 화면 크기 고정용

    // 인터랙션
    pub hovered_axis: GizmoAxis,
    pub active_axis: GizmoAxis,
    drag_start_pos: Vec3,
}

impl MoveGizmo {
    /// 새 MoveGizmo 생성
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        // 메시 생성
        let (vertices, indices, axis_ranges) = Self::create_mesh();

        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gizmo Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gizmo.wgsl").into()),
        });

        // 바인드 그룹 레이아웃
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Gizmo Bind Group Layout"),
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
            label: Some("Gizmo Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gizmo Pipeline"),
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
                    write_mask: wgpu::ColorWrites::COLOR,  // RGB만 쓰기, Alpha 채널 보존
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
                depth_write_enabled: false, // Gizmo는 항상 위에 표시
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
            label: Some("Gizmo Vertex Buffer"),
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
            label: Some("Gizmo Index Buffer"),
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
            label: Some("Gizmo Uniform Buffer"),
            size: std::mem::size_of::<GizmoUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 바인드 그룹
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Gizmo Bind Group"),
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
            drag_start_pos: Vec3::ZERO,
        }
    }

    /// 메시 생성 (화살표 + 평면)
    fn create_mesh() -> (Vec<GizmoVertex>, Vec<u32>, [AxisMeshRange; 6]) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut ranges = [
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
        ];

        // X축 화살표 (빨강)
        ranges[0] = Self::add_arrow(&mut vertices, &mut indices, Vec3::X, Vec3::Y);

        // Y축 화살표 (초록)
        ranges[1] = Self::add_arrow(&mut vertices, &mut indices, Vec3::Y, Vec3::X);

        // Z축 화살표 (파랑)
        ranges[2] = Self::add_arrow(&mut vertices, &mut indices, Vec3::Z, Vec3::Y);

        // XY 평면 (파랑)
        ranges[3] = Self::add_plane(&mut vertices, &mut indices, Vec3::X, Vec3::Y, Vec3::Z);

        // YZ 평면 (빨강)
        ranges[4] = Self::add_plane(&mut vertices, &mut indices, Vec3::Y, Vec3::Z, Vec3::X);

        // ZX 평면 (초록)
        ranges[5] = Self::add_plane(&mut vertices, &mut indices, Vec3::Z, Vec3::X, Vec3::Y);

        (vertices, indices, ranges)
    }

    /// 화살표 메시 추가
    fn add_arrow(
        vertices: &mut Vec<GizmoVertex>,
        indices: &mut Vec<u32>,
        direction: Vec3,
        up: Vec3,
    ) -> AxisMeshRange {
        let vertex_start = vertices.len() as u32;
        let index_start = indices.len() as u32;

        let shaft_length = 0.8;
        let shaft_radius = 0.02;
        let cone_length = 0.2;
        let cone_radius = 0.06;
        let segments = 12;

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

        // 원뿔 (cone) 베이스
        let cone_base_start = vertices.len() as u32;
        let cone_base_center = direction * shaft_length;

        // 베이스 중심
        vertices.push(GizmoVertex {
            position: cone_base_center.to_array(),
            normal: (-direction).to_array(),
        });

        // 베이스 둘레
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            let pos = cone_base_center + (right * cos_a + up * sin_a) * cone_radius;
            vertices.push(GizmoVertex {
                position: pos.to_array(),
                normal: (-direction).to_array(),
            });
        }

        // 베이스 인덱스
        for i in 0..segments {
            indices.push(cone_base_start);
            indices.push(cone_base_start + 1 + i);
            indices.push(cone_base_start + 2 + i);
        }

        // 원뿔 측면
        let cone_side_start = vertices.len() as u32;
        let cone_tip = direction * (shaft_length + cone_length);

        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            let base_pos = cone_base_center + (right * cos_a + up * sin_a) * cone_radius;
            let edge = (base_pos - cone_tip).normalize();
            let normal = edge.cross(right * (-sin_a) + up * cos_a).normalize();

            vertices.push(GizmoVertex {
                position: base_pos.to_array(),
                normal: normal.to_array(),
            });
        }

        // 원뿔 끝점
        let cone_tip_idx = vertices.len() as u32;
        vertices.push(GizmoVertex {
            position: cone_tip.to_array(),
            normal: direction.to_array(),
        });

        // 원뿔 측면 인덱스
        for i in 0..segments {
            indices.push(cone_side_start + i);
            indices.push(cone_tip_idx);
            indices.push(cone_side_start + i + 1);
        }

        let vertex_count = vertices.len() as u32 - vertex_start;
        let index_count = indices.len() as u32 - index_start;

        AxisMeshRange {
            vertex_start,
            vertex_count,
            index_start,
            index_count,
        }
    }

    /// 평면 핸들 메시 추가
    fn add_plane(
        vertices: &mut Vec<GizmoVertex>,
        indices: &mut Vec<u32>,
        axis1: Vec3,
        axis2: Vec3,
        normal: Vec3,
    ) -> AxisMeshRange {
        let vertex_start = vertices.len() as u32;
        let index_start = indices.len() as u32;

        let offset = 0.2; // 원점에서 얼마나 떨어져 있는지
        let size = 0.15;  // 사각형 크기

        // 사각형 4개 꼭짓점
        let corners = [
            axis1 * offset + axis2 * offset,
            axis1 * (offset + size) + axis2 * offset,
            axis1 * (offset + size) + axis2 * (offset + size),
            axis1 * offset + axis2 * (offset + size),
        ];

        for corner in &corners {
            vertices.push(GizmoVertex {
                position: corner.to_array(),
                normal: normal.to_array(),
            });
        }

        // 양면 렌더링을 위한 뒷면
        for corner in &corners {
            vertices.push(GizmoVertex {
                position: corner.to_array(),
                normal: (-normal).to_array(),
            });
        }

        // 앞면 인덱스
        indices.push(vertex_start);
        indices.push(vertex_start + 1);
        indices.push(vertex_start + 2);
        indices.push(vertex_start);
        indices.push(vertex_start + 2);
        indices.push(vertex_start + 3);

        // 뒷면 인덱스
        indices.push(vertex_start + 4);
        indices.push(vertex_start + 6);
        indices.push(vertex_start + 5);
        indices.push(vertex_start + 4);
        indices.push(vertex_start + 7);
        indices.push(vertex_start + 6);

        let vertex_count = vertices.len() as u32 - vertex_start;
        let index_count = indices.len() as u32 - index_start;

        AxisMeshRange {
            vertex_start,
            vertex_count,
            index_start,
            index_count,
        }
    }

    /// Gizmo 위치 설정
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// 화면 크기 기반 스케일 업데이트
    pub fn update_scale(&mut self, camera: &EditorCamera, screen_size: (u32, u32)) {
        // 카메라와의 거리에 따라 Gizmo 크기 조정 (화면에서 일정 크기 유지)
        let camera_pos = camera.position;
        let distance = (self.position - camera_pos).length();

        // 화면 높이 기준 스케일
        let fov_factor = (camera.settings.fov / 2.0).tan();
        let screen_factor = screen_size.1 as f32 / 720.0; // 720p 기준

        self.scale = distance * fov_factor * 0.15 / screen_factor;
    }

    /// Ray-Gizmo 충돌 검사
    pub fn pick(&self, ray: &Ray) -> GizmoAxis {
        let inv_scale = 1.0 / self.scale;
        let local_origin = (ray.origin - self.position) * inv_scale;
        let local_dir = ray.direction;

        let mut closest_axis = GizmoAxis::None;
        let mut closest_t = f32::MAX;

        // 각 축 화살표 검사 (원기둥 + 원뿔을 단순 박스로 근사)
        let axes = [
            (GizmoAxis::X, Vec3::X),
            (GizmoAxis::Y, Vec3::Y),
            (GizmoAxis::Z, Vec3::Z),
        ];

        for (axis, dir) in axes {
            // 화살표를 AABB로 근사
            let half_extent = 0.06;
            let length = 1.0;

            let (min, max) = if dir.x > 0.5 {
                (Vec3::new(0.0, -half_extent, -half_extent), Vec3::new(length, half_extent, half_extent))
            } else if dir.y > 0.5 {
                (Vec3::new(-half_extent, 0.0, -half_extent), Vec3::new(half_extent, length, half_extent))
            } else {
                (Vec3::new(-half_extent, -half_extent, 0.0), Vec3::new(half_extent, half_extent, length))
            };

            if let Some(t) = Self::ray_aabb_intersection(local_origin, local_dir, min, max) {
                if t < closest_t && t > 0.0 {
                    closest_t = t;
                    closest_axis = axis;
                }
            }
        }

        // 평면 핸들 검사
        let planes = [
            (GizmoAxis::XY, Vec3::X, Vec3::Y, Vec3::Z),
            (GizmoAxis::YZ, Vec3::Y, Vec3::Z, Vec3::X),
            (GizmoAxis::ZX, Vec3::Z, Vec3::X, Vec3::Y),
        ];

        for (axis, axis1, axis2, normal) in planes {
            let offset = 0.2;
            let size = 0.15;

            // 평면과의 교차점 계산
            let denom = local_dir.dot(normal);
            if denom.abs() > 0.001 {
                let plane_point = (axis1 + axis2) * (offset + size / 2.0);
                let t = (plane_point - local_origin).dot(normal) / denom;

                if t > 0.0 && t < closest_t {
                    let hit_point = local_origin + local_dir * t;

                    // 사각형 범위 내인지 확인
                    let u = hit_point.dot(axis1);
                    let v = hit_point.dot(axis2);

                    if u >= offset && u <= offset + size && v >= offset && v <= offset + size {
                        closest_t = t;
                        closest_axis = axis;
                    }
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

    /// 드래그 오프셋 계산
    pub fn calculate_drag_offset(
        &self,
        camera: &EditorCamera,
        prev_pos: glam::Vec2,
        curr_pos: glam::Vec2,
        screen_size: glam::Vec2,
    ) -> Vec3 {
        let prev_ray = camera.screen_to_ray(prev_pos, screen_size);
        let curr_ray = camera.screen_to_ray(curr_pos, screen_size);

        match self.active_axis {
            GizmoAxis::X | GizmoAxis::Y | GizmoAxis::Z => {
                // 축 제약 드래그
                let axis_dir = self.active_axis.direction();

                // 뷰 방향과 축의 교차 평면 찾기
                let view_dir = camera.forward();
                let plane_normal = view_dir.cross(axis_dir).cross(axis_dir).normalize();

                // 이전/현재 ray와 평면의 교차점
                let prev_t = self.ray_plane_intersection(&prev_ray, plane_normal, self.position);
                let curr_t = self.ray_plane_intersection(&curr_ray, plane_normal, self.position);

                if let (Some(prev_t), Some(curr_t)) = (prev_t, curr_t) {
                    let prev_hit = prev_ray.origin + prev_ray.direction * prev_t;
                    let curr_hit = curr_ray.origin + curr_ray.direction * curr_t;
                    let delta = curr_hit - prev_hit;

                    // 축 방향으로 투영
                    axis_dir * delta.dot(axis_dir)
                } else {
                    Vec3::ZERO
                }
            }
            GizmoAxis::XY | GizmoAxis::YZ | GizmoAxis::ZX => {
                // 평면 제약 드래그
                let plane_normal = self.active_axis.plane_normal();

                let prev_t = self.ray_plane_intersection(&prev_ray, plane_normal, self.position);
                let curr_t = self.ray_plane_intersection(&curr_ray, plane_normal, self.position);

                if let (Some(prev_t), Some(curr_t)) = (prev_t, curr_t) {
                    let prev_hit = prev_ray.origin + prev_ray.direction * prev_t;
                    let curr_hit = curr_ray.origin + curr_ray.direction * curr_t;
                    curr_hit - prev_hit
                } else {
                    Vec3::ZERO
                }
            }
            GizmoAxis::None => Vec3::ZERO,
        }
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
    pub fn begin_drag(&mut self, axis: GizmoAxis) {
        self.active_axis = axis;
        self.drag_start_pos = self.position;
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

        // 각 축별로 렌더링
        let axes = [
            (GizmoAxis::X, 0),
            (GizmoAxis::Y, 1),
            (GizmoAxis::Z, 2),
            (GizmoAxis::XY, 3),
            (GizmoAxis::YZ, 4),
            (GizmoAxis::ZX, 5),
        ];

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Gizmo Render Pass"),
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

        for (axis, range_idx) in axes {
            let range = &self.axis_ranges[range_idx];
            let is_hovered = self.hovered_axis == axis || self.active_axis == axis;

            let uniforms = GizmoUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                model: model.to_cols_array_2d(),
                color: axis.color(),
                hover_color: axis.hover_color(),
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
