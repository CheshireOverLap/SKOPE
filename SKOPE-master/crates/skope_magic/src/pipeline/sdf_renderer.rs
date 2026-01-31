//! Magic Circle SDF Renderer
//!
//! SDF 기반 마법진 렌더러

#[cfg(feature = "gpu")]
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "gpu")]
use wgpu;

use bevy_ecs::prelude::*;

use crate::components::{CircleTransform, MagicCircle};
use crate::data::{MagicCircleRegistry, NodeDef};

// ========== GPU Data Structures ==========

/// Circle Uniform (GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "gpu", derive(Pod, Zeroable))]
pub struct CircleUniform {
    /// World position
    pub world_position: [f32; 3],
    /// Scale
    pub scale: f32,
    /// Normal direction
    pub normal: [f32; 3],
    /// Base rotation
    pub base_rotation: f32,
    /// Color tint (RGBA)
    pub color: [f32; 4],
    /// Opacity
    pub opacity: f32,
    /// Time (for animations)
    pub time: f32,
    /// Spawn progress (0.0 ~ 1.0)
    pub spawn_progress: f32,
    /// Activate progress (0.0 ~ 1.0)
    pub activate_progress: f32,
}

impl Default for CircleUniform {
    fn default() -> Self {
        Self {
            world_position: [0.0, 0.0, 0.0],
            scale: 1.0,
            normal: [0.0, 1.0, 0.0],
            base_rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            opacity: 1.0,
            time: 0.0,
            spawn_progress: 1.0,
            activate_progress: 0.0,
        }
    }
}

/// Layer Data (GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "gpu", derive(Pod, Zeroable))]
pub struct LayerGpuData {
    pub rotation: f32,
    pub radius_min: f32,
    pub radius_max: f32,
    pub glow_intensity: f32,
    pub pulse: f32,
    pub segments: u32,
    pub layer_type: u32,
    pub _pad: u32,
}

impl Default for LayerGpuData {
    fn default() -> Self {
        Self {
            rotation: 0.0,
            radius_min: 0.0,
            radius_max: 1.0,
            glow_intensity: 1.0,
            pulse: 0.0,
            segments: 0,
            layer_type: 0,
            _pad: 0,
        }
    }
}

/// Node Data (GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "gpu", derive(Pod, Zeroable))]
pub struct NodeGpuData {
    pub position: [f32; 2],
    pub element: u32,
    pub activation: f32,
    pub size: f32,
    pub _pad: [f32; 3],
}

impl Default for NodeGpuData {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            element: 0,
            activation: 0.0,
            size: 1.0,
            _pad: [0.0, 0.0, 0.0],
        }
    }
}

/// Connection Data (GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "gpu", derive(Pod, Zeroable))]
pub struct ConnectionGpuData {
    pub from_pos: [f32; 2],
    pub to_pos: [f32; 2],
    pub flow_progress: f32,
    pub _pad: [f32; 3],
}

impl Default for ConnectionGpuData {
    fn default() -> Self {
        Self {
            from_pos: [0.0, 0.0],
            to_pos: [0.0, 0.0],
            flow_progress: 0.0,
            _pad: [0.0, 0.0, 0.0],
        }
    }
}

/// Counts Uniform (GPU)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "gpu", derive(Pod, Zeroable))]
pub struct CountsUniform {
    pub layer_count: u32,
    pub node_count: u32,
    pub connection_count: u32,
    pub _pad: u32,
}

// ========== Render Data ==========

/// 단일 마법진 렌더 인스턴스
pub struct CircleRenderInstance {
    pub uniform: CircleUniform,
    pub layers: Vec<LayerGpuData>,
    pub nodes: Vec<NodeGpuData>,
    pub connections: Vec<ConnectionGpuData>,
}

/// 마법진 렌더 데이터 (프레임마다 수집)
#[derive(Resource, Default)]
pub struct MagicCircleRenderData {
    pub circles: Vec<CircleRenderInstance>,
}

impl MagicCircleRenderData {
    pub fn clear(&mut self) {
        self.circles.clear();
    }

    pub fn has_data(&self) -> bool {
        !self.circles.is_empty()
    }
}

// ========== Renderer ==========

#[cfg(feature = "gpu")]
pub struct MagicCircleRenderer {
    pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    camera_bind_group_layout: wgpu::BindGroupLayout,
    circle_bind_group_layout: wgpu::BindGroupLayout,

    // 버퍼 (단일 마법진용, 여러 개일 경우 재사용)
    circle_uniform_buffer: wgpu::Buffer,
    layer_buffer: wgpu::Buffer,
    node_buffer: wgpu::Buffer,
    connection_buffer: wgpu::Buffer,
    counts_buffer: wgpu::Buffer,

    // 최대 용량
    max_layers: usize,
    max_nodes: usize,
    max_connections: usize,
}

#[cfg(feature = "gpu")]
impl MagicCircleRenderer {
    const MAX_LAYERS: usize = 16;
    const MAX_NODES: usize = 32;
    const MAX_CONNECTIONS: usize = 64;

    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // 셰이더 로드
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Magic Circle Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/magic_circle.wgsl").into(),
            ),
        });

        // Circle 바인드 그룹 레이아웃
        let circle_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Magic Circle BGL"),
                entries: &[
                    // binding 0: CircleUniform
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
                    // binding 1: Layers (storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 2: Nodes (storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 3: Connections (storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 4: Counts
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Magic Circle Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &circle_bind_group_layout],
            push_constant_ranges: &[],
        });

        // 렌더 파이프라인
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Magic Circle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One, // Additive
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Max,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
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
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // 깊이 쓰기 비활성 (투명)
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 버퍼 생성
        let circle_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Uniform"),
            size: std::mem::size_of::<CircleUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layer_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Layers"),
            size: (std::mem::size_of::<LayerGpuData>() * Self::MAX_LAYERS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let node_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Nodes"),
            size: (std::mem::size_of::<NodeGpuData>() * Self::MAX_NODES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let connection_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Connections"),
            size: (std::mem::size_of::<ConnectionGpuData>() * Self::MAX_CONNECTIONS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let counts_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Counts"),
            size: std::mem::size_of::<CountsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            camera_bind_group_layout: camera_bind_group_layout.clone(),
            circle_bind_group_layout,
            circle_uniform_buffer,
            layer_buffer,
            node_buffer,
            connection_buffer,
            counts_buffer,
            max_layers: Self::MAX_LAYERS,
            max_nodes: Self::MAX_NODES,
            max_connections: Self::MAX_CONNECTIONS,
        }
    }

    /// 바인드 그룹 레이아웃 반환
    pub fn circle_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.circle_bind_group_layout
    }

    /// 마법진 렌더링
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        camera_bind_group: &'a wgpu::BindGroup,
        render_data: &MagicCircleRenderData,
    ) {
        if render_data.circles.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);

        for instance in &render_data.circles {
            // 버퍼 업데이트
            queue.write_buffer(
                &self.circle_uniform_buffer,
                0,
                bytemuck::bytes_of(&instance.uniform),
            );

            // 레이어 데이터
            let layer_count = instance.layers.len().min(self.max_layers);
            if layer_count > 0 {
                queue.write_buffer(
                    &self.layer_buffer,
                    0,
                    bytemuck::cast_slice(&instance.layers[..layer_count]),
                );
            }

            // 노드 데이터
            let node_count = instance.nodes.len().min(self.max_nodes);
            if node_count > 0 {
                queue.write_buffer(
                    &self.node_buffer,
                    0,
                    bytemuck::cast_slice(&instance.nodes[..node_count]),
                );
            }

            // 연결 데이터
            let connection_count = instance.connections.len().min(self.max_connections);
            if connection_count > 0 {
                queue.write_buffer(
                    &self.connection_buffer,
                    0,
                    bytemuck::cast_slice(&instance.connections[..connection_count]),
                );
            }

            // Counts
            let counts = CountsUniform {
                layer_count: layer_count as u32,
                node_count: node_count as u32,
                connection_count: connection_count as u32,
                _pad: 0,
            };
            queue.write_buffer(&self.counts_buffer, 0, bytemuck::bytes_of(&counts));

            // 바인드 그룹 생성 (매 프레임 재생성 - TODO: 캐싱)
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Magic Circle Instance BG"),
                layout: &self.circle_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.circle_uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.layer_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.node_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.connection_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.counts_buffer.as_entire_binding(),
                    },
                ],
            });

            render_pass.set_bind_group(1, &bind_group, &[]);

            // 6 vertices for fullscreen quad
            render_pass.draw(0..6, 0..1);
        }
    }
}

// ========== Extract System ==========

/// MagicCircle → MagicCircleRenderData 추출 시스템
pub fn magic_circle_extract_system(
    circles: Query<(&MagicCircle, &CircleTransform)>,
    registry: Res<MagicCircleRegistry>,
    mut render_data: ResMut<MagicCircleRenderData>,
) {
    render_data.clear();

    for (circle, transform) in circles.iter() {
        // 페이드 완료된 마법진은 스킵
        if circle.should_despawn() {
            continue;
        }

        let Some(def) = registry.get(&circle.definition_id) else {
            continue;
        };

        // CircleUniform 생성
        let uniform = CircleUniform {
            world_position: transform.position,
            scale: circle.display_scale(),
            normal: transform.normal,
            base_rotation: transform.base_rotation,
            color: circle.color,
            opacity: circle.opacity,
            time: circle.time,
            spawn_progress: circle.spawn_progress,
            activate_progress: circle.activate_progress,
        };

        // 레이어 데이터 생성
        let layers: Vec<LayerGpuData> = def
            .layers
            .iter()
            .zip(circle.layer_states.iter())
            .map(|(layer_def, state)| LayerGpuData {
                rotation: state.current_rotation,
                radius_min: layer_def.radius_min,
                radius_max: layer_def.radius_max,
                glow_intensity: layer_def.glow_intensity,
                pulse: state.pulse_intensity(layer_def.pulse_amplitude),
                segments: layer_def.segments,
                layer_type: layer_def.layer_type.shader_index(),
                _pad: 0,
            })
            .collect();

        // 노드 데이터 생성
        let nodes: Vec<NodeGpuData> = def
            .nodes
            .iter()
            .zip(circle.node_activations.iter())
            .map(|(node_def, &activation): (&NodeDef, &f32)| {
                let (x, y) = node_def.cartesian();
                NodeGpuData {
                    position: [x, y],
                    element: node_def.element.shader_index(),
                    activation,
                    size: node_def.size,
                    _pad: [0.0, 0.0, 0.0],
                }
            })
            .collect();

        // 연결 데이터 생성
        let connections: Vec<ConnectionGpuData> = def
            .connections
            .iter()
            .zip(circle.flow_progress.iter())
            .filter_map(|(conn, &progress)| {
                let from_node = def.nodes.get(conn.from)?;
                let to_node = def.nodes.get(conn.to)?;
                let (fx, fy) = from_node.cartesian();
                let (tx, ty) = to_node.cartesian();
                Some(ConnectionGpuData {
                    from_pos: [fx, fy],
                    to_pos: [tx, ty],
                    flow_progress: progress,
                    _pad: [0.0, 0.0, 0.0],
                })
            })
            .collect();

        render_data.circles.push(CircleRenderInstance {
            uniform,
            layers,
            nodes,
            connections,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_size() {
        assert_eq!(std::mem::size_of::<CircleUniform>(), 64); // 16 floats
        assert_eq!(std::mem::size_of::<LayerGpuData>(), 32);
        assert_eq!(std::mem::size_of::<NodeGpuData>(), 32);
        assert_eq!(std::mem::size_of::<ConnectionGpuData>(), 32);
        assert_eq!(std::mem::size_of::<CountsUniform>(), 16);
    }
}
