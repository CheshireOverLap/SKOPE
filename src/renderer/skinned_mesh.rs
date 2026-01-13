// Skinned Mesh Renderer Module
// Phase 11: GPU Skinning Support

#![allow(dead_code)]

use wgpu::util::DeviceExt;
use glam::Mat4;
use crate::gltf_loader::{SkinnedVertex, SkinnedMesh, Skin};
use crate::ecs_resources::SkinnedMeshGpuData;

/// 최대 본 개수 (셰이더와 일치해야 함)
pub const MAX_JOINTS: usize = 128;

/// 본 매트릭스 버퍼 (GPU용)
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct JointMatricesUniform {
    pub matrices: [[f32; 16]; MAX_JOINTS],  // 128 * mat4x4
}

impl Default for JointMatricesUniform {
    fn default() -> Self {
        let identity: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        Self {
            matrices: [identity; MAX_JOINTS],
        }
    }
}

impl JointMatricesUniform {
    /// glam::Mat4 배열에서 업데이트
    pub fn from_matrices(mats: &[Mat4]) -> Self {
        let mut uniform = Self::default();
        for (i, mat) in mats.iter().enumerate() {
            if i >= MAX_JOINTS {
                break;
            }
            uniform.matrices[i] = mat.to_cols_array();
        }
        uniform
    }
}

/// 스킨드 렌더 파이프라인 리소스
pub struct SkinnedRenderPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub skinned_uniform_bind_group_layout: wgpu::BindGroupLayout,
}

/// 스킨드 메시 인스턴스 (렌더링용)
pub struct SkinnedMeshRenderData {
    pub gpu_data: SkinnedMeshGpuData,
    pub joint_buffer: wgpu::Buffer,
    pub joint_bind_group: wgpu::BindGroup,
    pub joint_count: usize,
}

/// 스킨드 렌더 파이프라인 생성
pub fn create_skinned_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    material_bind_group_layout: &wgpu::BindGroupLayout,
) -> SkinnedRenderPipeline {
    // 스킨드 셰이더 로드
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Skinned Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/forward_skinned.wgsl").into()),
    });

    // 스킨드용 유니폼 바인드 그룹 레이아웃 (MVP + Joint Matrices)
    let skinned_uniform_bind_group_layout = device.create_bind_group_layout(
        &wgpu::BindGroupLayoutDescriptor {
            label: Some("Skinned Uniform Bind Group Layout"),
            entries: &[
                // Binding 0: MVP uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Joint matrices
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        },
    );

    // 파이프라인 레이아웃
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Skinned Pipeline Layout"),
        bind_group_layouts: &[
            &skinned_uniform_bind_group_layout,
            texture_bind_group_layout,
            material_bind_group_layout,
        ],
        push_constant_ranges: &[],
    });

    // 렌더 파이프라인
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Skinned Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[SkinnedVertex::desc()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,  // 양면 렌더링 (디버그용)
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    log::info!("Created skinned render pipeline");

    SkinnedRenderPipeline {
        pipeline,
        skinned_uniform_bind_group_layout,
    }
}

/// 스킨드 메시를 GPU에 업로드
pub fn upload_skinned_mesh(
    device: &wgpu::Device,
    skinned_mesh: &SkinnedMesh,
    skin: &Skin,
    uniform_buffer: &wgpu::Buffer,
    skinned_bind_group_layout: &wgpu::BindGroupLayout,
) -> SkinnedMeshRenderData {
    // Vertex buffer
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinned Vertex Buffer"),
        contents: bytemuck::cast_slice(&skinned_mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Index buffer
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Skinned Index Buffer"),
        contents: bytemuck::cast_slice(&skinned_mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Joint matrices buffer (초기값: identity)
    let joint_uniform = JointMatricesUniform::default();
    let joint_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Joint Matrices Buffer"),
        contents: bytemuck::cast_slice(&[joint_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // Skinned bind group (MVP + Joints)
    let joint_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Skinned Uniform Bind Group"),
        layout: skinned_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: joint_buffer.as_entire_binding(),
            },
        ],
    });

    let gpu_data = SkinnedMeshGpuData {
        vertex_buffer,
        index_buffer,
        num_indices: skinned_mesh.indices.len() as u32,
        skin_index: skinned_mesh.skin_index,
    };

    log::info!("Uploaded skinned mesh: {} verts, {} indices, {} joints",
        skinned_mesh.vertices.len(),
        skinned_mesh.indices.len(),
        skin.joints.len());

    SkinnedMeshRenderData {
        gpu_data,
        joint_buffer,
        joint_bind_group,
        joint_count: skin.joints.len(),
    }
}

/// 본 변환 행렬 계산 (T-pose에서 현재 포즈로)
/// inverse_bind_matrix * global_transform
pub fn compute_joint_matrices(
    skin: &Skin,
    node_global_transforms: &[Mat4],
) -> Vec<Mat4> {
    skin.joints.iter().map(|joint| {
        // 역 바인드 행렬 (glTF에서 로드)
        let inverse_bind = Mat4::from_cols_array_2d(&joint.inverse_bind_matrix);

        // 현재 노드의 글로벌 변환 (애니메이션 적용된 상태)
        let global_transform = node_global_transforms
            .get(joint.node_index)
            .copied()
            .unwrap_or(Mat4::IDENTITY);

        // 최종 본 매트릭스 = global_transform * inverse_bind_matrix
        global_transform * inverse_bind
    }).collect()
}

/// 본 매트릭스 GPU 버퍼 업데이트
pub fn update_joint_matrices(
    queue: &wgpu::Queue,
    joint_buffer: &wgpu::Buffer,
    matrices: &[Mat4],
) {
    let uniform = JointMatricesUniform::from_matrices(matrices);
    queue.write_buffer(joint_buffer, 0, bytemuck::cast_slice(&[uniform]));
}

/// 스킨드 메시 렌더링 (forward pass)
pub fn render_skinned_mesh<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    pipeline: &'a wgpu::RenderPipeline,
    gpu_data: &'a SkinnedMeshGpuData,
    joint_bind_group: &'a wgpu::BindGroup,
    texture_bind_group: &'a wgpu::BindGroup,
    material_bind_group: &'a wgpu::BindGroup,
) {
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, joint_bind_group, &[]);      // MVP + Joints
    render_pass.set_bind_group(1, texture_bind_group, &[]);    // Textures
    render_pass.set_bind_group(2, material_bind_group, &[]);   // Material
    render_pass.set_vertex_buffer(0, gpu_data.vertex_buffer.slice(..));
    render_pass.set_index_buffer(gpu_data.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.draw_indexed(0..gpu_data.num_indices, 0, 0..1);
}
