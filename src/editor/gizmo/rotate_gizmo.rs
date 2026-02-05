//! Rotate Gizmo 구현
//!
//! XYZ 축 회전 원호 (토러스 세그먼트)

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

/// Rotate Gizmo
pub struct RotateGizmo {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // 메시 범위 (축별) - X, Y, Z
    axis_ranges: [AxisMeshRange; 3],

    // 상태
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: f32,

    // 인터랙션
    pub hovered_axis: GizmoAxis,
    pub active_axis: GizmoAxis,
    drag_start_angle: f32,
    accumulated_rotation: Quat,
}

impl RotateGizmo {
    const RING_RADIUS: f32 = 0.8;
    const RING_THICKNESS: f32 = 0.03;
    const RING_SEGMENTS: u32 = 48;
    const TUBE_SEGMENTS: u32 = 8;

    /// 새 RotateGizmo 생성
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        // 메시 생성
        let (vertices, indices, axis_ranges) = Self::create_mesh();

        // 셰이더 (MoveGizmo와 같은 셰이더 사용)
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rotate Gizmo Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gizmo.wgsl").into()),
        });

        // 바인드 그룹 레이아웃
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rotate Gizmo Bind Group Layout"),
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
            label: Some("Rotate Gizmo Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rotate Gizmo Pipeline"),
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
                cull_mode: None, // 양면 렌더링
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
            multiview_mask: None,
            cache: None,
        });

        // 버텍스 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rotate Gizmo Vertex Buffer"),
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
            label: Some("Rotate Gizmo Index Buffer"),
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
            label: Some("Rotate Gizmo Uniform Buffer"),
            size: std::mem::size_of::<GizmoUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 바인드 그룹
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rotate Gizmo Bind Group"),
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
            drag_start_angle: 0.0,
            accumulated_rotation: Quat::IDENTITY,
        }
    }

    /// 메시 생성 (원호 토러스 3개)
    fn create_mesh() -> (Vec<GizmoVertex>, Vec<u32>, [AxisMeshRange; 3]) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut ranges = [
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
            AxisMeshRange { vertex_start: 0, vertex_count: 0, index_start: 0, index_count: 0 },
        ];

        // X축 회전 (YZ 평면의 원) - 빨강
        ranges[0] = Self::add_rotation_ring(&mut vertices, &mut indices, Vec3::X);

        // Y축 회전 (XZ 평면의 원) - 초록
        ranges[1] = Self::add_rotation_ring(&mut vertices, &mut indices, Vec3::Y);

        // Z축 회전 (XY 평면의 원) - 파랑
        ranges[2] = Self::add_rotation_ring(&mut vertices, &mut indices, Vec3::Z);

        (vertices, indices, ranges)
    }

    /// 회전 링(토러스) 메시 추가
    fn add_rotation_ring(
        vertices: &mut Vec<GizmoVertex>,
        indices: &mut Vec<u32>,
        axis: Vec3, // 회전축 (원이 이 축을 중심으로)
    ) -> AxisMeshRange {
        let vertex_start = vertices.len() as u32;
        let index_start = indices.len() as u32;

        let radius = Self::RING_RADIUS;
        let thickness = Self::RING_THICKNESS;
        let ring_segments = Self::RING_SEGMENTS;
        let tube_segments = Self::TUBE_SEGMENTS;

        // 회전축에 수직인 두 벡터 찾기
        let (u, v) = if axis.x.abs() > 0.9 {
            (Vec3::Y, Vec3::Z)
        } else if axis.y.abs() > 0.9 {
            (Vec3::Z, Vec3::X)
        } else {
            (Vec3::X, Vec3::Y)
        };

        // 토러스 생성
        for i in 0..=ring_segments {
            let ring_angle = (i as f32 / ring_segments as f32) * std::f32::consts::TAU;
            let ring_cos = ring_angle.cos();
            let ring_sin = ring_angle.sin();

            // 링 위의 중심점
            let ring_center = u * ring_cos * radius + v * ring_sin * radius;
            // 중심에서 바깥 방향
            let ring_normal = ring_center.normalize();
            // 튜브의 up 방향 (축과 같음)
            let tube_up = axis;
            // 튜브의 right 방향
            let tube_right = ring_normal;

            for j in 0..=tube_segments {
                let tube_angle = (j as f32 / tube_segments as f32) * std::f32::consts::TAU;
                let tube_cos = tube_angle.cos();
                let tube_sin = tube_angle.sin();

                let normal = tube_right * tube_cos + tube_up * tube_sin;
                let position = ring_center + normal * thickness;

                vertices.push(GizmoVertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                });
            }
        }

        // 인덱스 생성
        let verts_per_ring = tube_segments + 1;
        for i in 0..ring_segments {
            for j in 0..tube_segments {
                let current = vertex_start + i * verts_per_ring + j;
                let next_ring = vertex_start + (i + 1) * verts_per_ring + j;

                // 두 삼각형으로 사각형 구성
                indices.push(current);
                indices.push(next_ring);
                indices.push(current + 1);

                indices.push(current + 1);
                indices.push(next_ring);
                indices.push(next_ring + 1);
            }
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
        let mut closest_dist = f32::MAX;

        let axes = [
            (GizmoAxis::X, Vec3::X),
            (GizmoAxis::Y, Vec3::Y),
            (GizmoAxis::Z, Vec3::Z),
        ];

        for (axis, axis_dir) in axes {
            // Ray와 원의 최단 거리 계산
            if let Some(dist) = self.ray_ring_distance(local_origin, local_dir, axis_dir) {
                if dist < Self::RING_THICKNESS * 2.0 && dist < closest_dist {
                    closest_dist = dist;
                    closest_axis = axis;
                }
            }
        }

        closest_axis
    }

    /// Ray와 링(원) 사이의 최단 거리
    fn ray_ring_distance(&self, origin: Vec3, dir: Vec3, axis: Vec3) -> Option<f32> {
        // 축에 수직인 평면과 ray의 교차점 계산
        let denom = dir.dot(axis);

        // Ray가 평면과 거의 평행한 경우
        if denom.abs() < 0.001 {
            // 원점이 평면 근처인지 확인
            let dist_to_plane = origin.dot(axis).abs();
            if dist_to_plane > Self::RING_THICKNESS * 3.0 {
                return None;
            }

            // 원과의 최단 거리 근사
            let proj_origin = origin - axis * origin.dot(axis);
            let dist_from_center = proj_origin.length();
            return Some((dist_from_center - Self::RING_RADIUS).abs());
        }

        let t = -origin.dot(axis) / denom;
        if t < 0.0 {
            return None;
        }

        let hit_point = origin + dir * t;
        // 원의 중심(원점)으로부터의 거리
        let dist_from_center = hit_point.length();
        // 링 반지름과의 차이
        Some((dist_from_center - Self::RING_RADIUS).abs())
    }

    /// 회전 각도 계산 (스크린 좌표 기반)
    pub fn calculate_rotation_angle(
        &self,
        camera: &EditorCamera,
        prev_pos: glam::Vec2,
        curr_pos: glam::Vec2,
        screen_size: glam::Vec2,
    ) -> f32 {
        let axis_dir = self.active_axis.direction();
        if axis_dir == Vec3::ZERO {
            return 0.0;
        }

        let prev_ray = camera.screen_to_ray(prev_pos, screen_size);
        let curr_ray = camera.screen_to_ray(curr_pos, screen_size);

        // 회전축에 수직인 평면과 ray들의 교차점 계산
        let prev_hit = self.ray_plane_hit(&prev_ray, axis_dir);
        let curr_hit = self.ray_plane_hit(&curr_ray, axis_dir);

        if let (Some(prev_hit), Some(curr_hit)) = (prev_hit, curr_hit) {
            // 중심에서 두 점으로의 벡터
            let prev_vec = (prev_hit - self.position).normalize();
            let curr_vec = (curr_hit - self.position).normalize();

            // 두 벡터 사이의 각도 계산
            let dot = prev_vec.dot(curr_vec).clamp(-1.0, 1.0);
            let angle = dot.acos();

            // 회전 방향 결정 (cross product)
            let cross = prev_vec.cross(curr_vec);
            let sign = if cross.dot(axis_dir) > 0.0 { 1.0 } else { -1.0 };

            angle * sign
        } else {
            0.0
        }
    }

    /// Ray-Plane 교차점
    fn ray_plane_hit(&self, ray: &Ray, plane_normal: Vec3) -> Option<Vec3> {
        let denom = ray.direction.dot(plane_normal);
        if denom.abs() > 0.0001 {
            let t = (self.position - ray.origin).dot(plane_normal) / denom;
            if t > 0.0 {
                return Some(ray.origin + ray.direction * t);
            }
        }
        None
    }

    /// 드래그 시작
    pub fn begin_drag(&mut self, axis: GizmoAxis) {
        self.active_axis = axis;
        self.drag_start_angle = 0.0;
        self.accumulated_rotation = Quat::IDENTITY;
    }

    /// 드래그 종료
    pub fn end_drag(&mut self) {
        self.active_axis = GizmoAxis::None;
        self.accumulated_rotation = Quat::IDENTITY;
    }

    /// 현재 드래그로 인한 회전 쿼터니언 반환
    pub fn get_drag_rotation(&self, angle: f32) -> Quat {
        let axis_dir = self.active_axis.direction();
        if axis_dir == Vec3::ZERO {
            Quat::IDENTITY
        } else {
            Quat::from_axis_angle(axis_dir, angle)
        }
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

        let axes = [
            (GizmoAxis::X, 0),
            (GizmoAxis::Y, 1),
            (GizmoAxis::Z, 2),
        ];

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Rotate Gizmo Render Pass"),
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
            multiview_mask: None,
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
