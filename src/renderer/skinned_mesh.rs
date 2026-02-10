// Skinned Mesh Renderer Module
// Phase 11: GPU Skinning Support

#![allow(dead_code)]

use wgpu::util::DeviceExt;
use glam::Mat4;
use crate::gltf_loader::{SkinnedVertex, SkinnedMesh, Skin};
use crate::ecs_resources::SkinnedMeshGpuData;

/// 최대 본 개수 (셰이더와 일치해야 함)
pub const MAX_JOINTS: usize = 128;

/// 본 매트릭스 버퍼 (GPU용) - 현재 + 이전 프레임 포함
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct JointMatricesUniform {
    pub matrices: [[f32; 16]; MAX_JOINTS],       // 현재 프레임 (128 * mat4x4)
    pub prev_matrices: [[f32; 16]; MAX_JOINTS],  // 이전 프레임 (TAA velocity용)
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
            prev_matrices: [identity; MAX_JOINTS],
        }
    }
}

impl JointMatricesUniform {
    /// glam::Mat4 배열에서 업데이트 (현재 프레임만)
    pub fn from_matrices(mats: &[Mat4]) -> Self {
        let mut uniform = Self::default();
        for (i, mat) in mats.iter().enumerate() {
            if i >= MAX_JOINTS {
                break;
            }
            uniform.matrices[i] = mat.to_cols_array();
            // 첫 프레임에서는 이전 = 현재
            uniform.prev_matrices[i] = mat.to_cols_array();
        }
        uniform
    }

    /// 현재 + 이전 프레임 매트릭스 설정
    pub fn from_matrices_with_prev(current: &[Mat4], prev: &[Mat4]) -> Self {
        let mut uniform = Self::default();
        for (i, mat) in current.iter().enumerate() {
            if i >= MAX_JOINTS {
                break;
            }
            uniform.matrices[i] = mat.to_cols_array();
        }
        for (i, mat) in prev.iter().enumerate() {
            if i >= MAX_JOINTS {
                break;
            }
            uniform.prev_matrices[i] = mat.to_cols_array();
        }
        uniform
    }
}

/// 스킨드 메시 인스턴스 (렌더링용)
pub struct SkinnedMeshRenderData {
    pub gpu_data: SkinnedMeshGpuData,
    pub joint_buffer: wgpu::Buffer,
    pub joint_bind_group: wgpu::BindGroup,
    pub joint_count: usize,
    /// 이전 프레임 본 매트릭스 (TAA velocity용)
    pub prev_joint_matrices: Vec<Mat4>,
    /// 이전 프레임 모델 매트릭스
    pub prev_model_matrix: Mat4,
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

    // 초기 이전 프레임 데이터 (identity)
    let prev_joint_matrices = vec![Mat4::IDENTITY; skin.joints.len()];

    SkinnedMeshRenderData {
        gpu_data,
        joint_buffer,
        joint_bind_group,
        joint_count: skin.joints.len(),
        prev_joint_matrices,
        prev_model_matrix: Mat4::IDENTITY,
    }
}

