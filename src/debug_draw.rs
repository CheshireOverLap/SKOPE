// Debug Drawing System
// 디버그 라인, 포인트, 구 등을 렌더링
#![allow(dead_code)]

use glam::{Vec3, Vec4, Mat4};
use bevy_ecs::prelude::*;

/// 디버그 드로우 프리미티브
#[derive(Debug, Clone)]
pub enum DebugPrimitive {
    /// 라인 (시작점, 끝점, 색상)
    Line { start: Vec3, end: Vec3, color: Vec4 },
    /// 점 (위치, 색상, 크기)
    Point { position: Vec3, color: Vec4, size: f32 },
    /// 구 (중심, 반지름, 색상, 세그먼트 수)
    Sphere { center: Vec3, radius: f32, color: Vec4, segments: u32 },
    /// AABB 박스 (min, max, 색상)
    Box { min: Vec3, max: Vec3, color: Vec4 },
    /// 축 기즈모 (위치, 크기)
    Axis { position: Vec3, size: f32 },
}

/// 디버그 드로우 버퍼 (ECS Resource)
#[derive(Resource, Default)]
pub struct DebugDrawBuffer {
    /// 현재 프레임의 드로우 명령들
    primitives: Vec<DebugPrimitive>,
    /// 지속 드로우 (시간이 지나면 사라짐)
    persistent: Vec<(DebugPrimitive, f32)>, // (primitive, remaining_time)
}

impl DebugDrawBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 라인 추가
    pub fn line(&mut self, start: Vec3, end: Vec3, color: Vec4) {
        self.primitives.push(DebugPrimitive::Line { start, end, color });
    }

    /// 점 추가
    pub fn point(&mut self, position: Vec3, color: Vec4, size: f32) {
        self.primitives.push(DebugPrimitive::Point { position, color, size });
    }

    /// 구 추가
    pub fn sphere(&mut self, center: Vec3, radius: f32, color: Vec4) {
        self.primitives.push(DebugPrimitive::Sphere {
            center,
            radius,
            color,
            segments: 16,
        });
    }

    /// 박스 추가
    pub fn aabb(&mut self, min: Vec3, max: Vec3, color: Vec4) {
        self.primitives.push(DebugPrimitive::Box { min, max, color });
    }

    /// 축 기즈모 추가
    pub fn axis(&mut self, position: Vec3, size: f32) {
        self.primitives.push(DebugPrimitive::Axis { position, size });
    }

    /// 지속 라인 추가 (duration 초 동안 표시)
    pub fn line_persistent(&mut self, start: Vec3, end: Vec3, color: Vec4, duration: f32) {
        self.persistent.push((
            DebugPrimitive::Line { start, end, color },
            duration,
        ));
    }

    /// 프레임 시작 시 호출 - 일회성 프리미티브 클리어
    pub fn clear_frame(&mut self) {
        self.primitives.clear();
    }

    /// 지속 드로우 업데이트 (delta_time 만큼 시간 감소)
    pub fn update_persistent(&mut self, delta_time: f32) {
        self.persistent.retain_mut(|(_, time)| {
            *time -= delta_time;
            *time > 0.0
        });
    }

    /// 모든 프리미티브 가져오기 (일회성 + 지속)
    pub fn all_primitives(&self) -> impl Iterator<Item = &DebugPrimitive> {
        self.primitives.iter().chain(self.persistent.iter().map(|(p, _)| p))
    }

    /// 프리미티브 개수
    pub fn primitive_count(&self) -> usize {
        self.primitives.len() + self.persistent.len()
    }
}

/// 디버그 라인 버텍스
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl DebugVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x4,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// 디버그 드로우 렌더러
pub struct DebugDrawRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    max_vertices: usize,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl DebugDrawRenderer {
    const MAX_VERTICES: usize = 65536;

    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Debug Draw Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/debug_draw.wgsl").into()),
        });

        // 유니폼 버퍼 (view_proj 행렬)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Debug Uniform Buffer"),
            size: 64, // Mat4
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Debug Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Debug Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Debug Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Draw Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[DebugVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // 디버그는 깊이 쓰기 안 함
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 버텍스 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Debug Vertex Buffer"),
            size: (Self::MAX_VERTICES * std::mem::size_of::<DebugVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_count: 0,
            max_vertices: Self::MAX_VERTICES,
            uniform_buffer,
            uniform_bind_group,
        }
    }

    /// 프리미티브를 버텍스로 변환하여 버퍼 업데이트
    pub fn update(&mut self, queue: &wgpu::Queue, buffer: &DebugDrawBuffer, view_proj: Mat4) {
        // view_proj 업데이트 (Mat4를 배열로 변환)
        let view_proj_arr: [[f32; 4]; 4] = view_proj.to_cols_array_2d();
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&view_proj_arr));

        // 프리미티브를 라인 버텍스로 변환
        let mut vertices: Vec<DebugVertex> = Vec::new();

        for primitive in buffer.all_primitives() {
            match primitive {
                DebugPrimitive::Line { start, end, color } => {
                    vertices.push(DebugVertex {
                        position: [start.x, start.y, start.z],
                        color: [color.x, color.y, color.z, color.w],
                    });
                    vertices.push(DebugVertex {
                        position: [end.x, end.y, end.z],
                        color: [color.x, color.y, color.z, color.w],
                    });
                }
                DebugPrimitive::Point { position, color, .. } => {
                    // 점은 작은 십자 모양으로 표시
                    let size = 0.05;
                    let c = [color.x, color.y, color.z, color.w];
                    let p = [position.x, position.y, position.z];

                    vertices.push(DebugVertex { position: [p[0] - size, p[1], p[2]], color: c });
                    vertices.push(DebugVertex { position: [p[0] + size, p[1], p[2]], color: c });
                    vertices.push(DebugVertex { position: [p[0], p[1] - size, p[2]], color: c });
                    vertices.push(DebugVertex { position: [p[0], p[1] + size, p[2]], color: c });
                    vertices.push(DebugVertex { position: [p[0], p[1], p[2] - size], color: c });
                    vertices.push(DebugVertex { position: [p[0], p[1], p[2] + size], color: c });
                }
                DebugPrimitive::Sphere { center, radius, color, segments } => {
                    self.generate_sphere_lines(&mut vertices, *center, *radius, *color, *segments);
                }
                DebugPrimitive::Box { min, max, color } => {
                    self.generate_box_lines(&mut vertices, *min, *max, *color);
                }
                DebugPrimitive::Axis { position, size } => {
                    let p = *position;
                    let s = *size;
                    // X축 (빨강)
                    vertices.push(DebugVertex { position: [p.x, p.y, p.z], color: [1.0, 0.0, 0.0, 1.0] });
                    vertices.push(DebugVertex { position: [p.x + s, p.y, p.z], color: [1.0, 0.0, 0.0, 1.0] });
                    // Y축 (초록)
                    vertices.push(DebugVertex { position: [p.x, p.y, p.z], color: [0.0, 1.0, 0.0, 1.0] });
                    vertices.push(DebugVertex { position: [p.x, p.y + s, p.z], color: [0.0, 1.0, 0.0, 1.0] });
                    // Z축 (파랑)
                    vertices.push(DebugVertex { position: [p.x, p.y, p.z], color: [0.0, 0.0, 1.0, 1.0] });
                    vertices.push(DebugVertex { position: [p.x, p.y, p.z + s], color: [0.0, 0.0, 1.0, 1.0] });
                }
            }
        }

        // 최대 버텍스 수 제한
        if vertices.len() > self.max_vertices {
            vertices.truncate(self.max_vertices);
        }

        self.vertex_count = vertices.len() as u32;

        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
    }

    fn generate_sphere_lines(&self, vertices: &mut Vec<DebugVertex>, center: Vec3, radius: f32, color: Vec4, segments: u32) {
        let c = [color.x, color.y, color.z, color.w];

        // 3개의 원 (XY, XZ, YZ 평면)
        for plane in 0..3 {
            for i in 0..segments {
                let a1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                let a2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                let (p1, p2) = match plane {
                    0 => (
                        Vec3::new(center.x + radius * a1.cos(), center.y + radius * a1.sin(), center.z),
                        Vec3::new(center.x + radius * a2.cos(), center.y + radius * a2.sin(), center.z),
                    ),
                    1 => (
                        Vec3::new(center.x + radius * a1.cos(), center.y, center.z + radius * a1.sin()),
                        Vec3::new(center.x + radius * a2.cos(), center.y, center.z + radius * a2.sin()),
                    ),
                    _ => (
                        Vec3::new(center.x, center.y + radius * a1.cos(), center.z + radius * a1.sin()),
                        Vec3::new(center.x, center.y + radius * a2.cos(), center.z + radius * a2.sin()),
                    ),
                };

                vertices.push(DebugVertex { position: [p1.x, p1.y, p1.z], color: c });
                vertices.push(DebugVertex { position: [p2.x, p2.y, p2.z], color: c });
            }
        }
    }

    fn generate_box_lines(&self, vertices: &mut Vec<DebugVertex>, min: Vec3, max: Vec3, color: Vec4) {
        let c = [color.x, color.y, color.z, color.w];

        // 8개 꼭지점
        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];

        // 12개 엣지
        let edges = [
            (0, 1), (1, 2), (2, 3), (3, 0), // 앞면
            (4, 5), (5, 6), (6, 7), (7, 4), // 뒷면
            (0, 4), (1, 5), (2, 6), (3, 7), // 연결
        ];

        for (i, j) in edges {
            vertices.push(DebugVertex { position: [corners[i].x, corners[i].y, corners[i].z], color: c });
            vertices.push(DebugVertex { position: [corners[j].x, corners[j].y, corners[j].z], color: c });
        }
    }

    /// 디버그 드로우 렌더링
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.vertex_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_draw_buffer() {
        let mut buffer = DebugDrawBuffer::new();
        assert_eq!(buffer.primitive_count(), 0);

        buffer.line(Vec3::ZERO, Vec3::ONE, Vec4::ONE);
        assert_eq!(buffer.primitive_count(), 1);

        buffer.clear_frame();
        assert_eq!(buffer.primitive_count(), 0);
    }

    #[test]
    fn test_persistent_primitives() {
        let mut buffer = DebugDrawBuffer::new();

        buffer.line_persistent(Vec3::ZERO, Vec3::ONE, Vec4::ONE, 1.0);
        assert_eq!(buffer.primitive_count(), 1);

        buffer.update_persistent(0.5);
        assert_eq!(buffer.primitive_count(), 1);

        buffer.update_persistent(0.6);
        assert_eq!(buffer.primitive_count(), 0);
    }
}
