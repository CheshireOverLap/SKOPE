//! Mesh Factory — GPU mesh creation from Model data
//!
//! Extracts mesh upload logic from loader.rs into a reusable factory pattern.
//! UE5.7 Interchange Factory 패턴 참고.

use wgpu::util::DeviceExt;

use crate::ecs_resources::{MeshAssets, MeshGpuData};
use crate::gltf_loader::Model;
use crate::renderer::GpuVertex;

/// Mesh GPU 리소스 생성 팩토리
pub struct MeshFactory;

impl MeshFactory {
    /// Model의 메시 → GPU 버퍼 생성 + MeshAssets 등록
    ///
    /// Returns: (전체 인덱스 맵, 새로 등록된 수)
    /// indices[glTF_mesh_idx] == MeshAssets_index 보장 (직접 매핑)
    pub fn create_from_model(
        device: &wgpu::Device,
        model: &Model,
        mesh_assets: &mut MeshAssets,
        model_name: &str,
    ) -> (Vec<usize>, usize) {
        let mut indices = Vec::new();
        let mut newly_registered = 0;

        for (mesh_idx, mesh) in model.meshes.iter().enumerate() {
            let mesh_name = if model.meshes.len() == 1 {
                model_name.to_string()
            } else {
                format!("{}_{}", model_name, mesh_idx)
            };

            // 이미 등록된 경우 → 기존 인덱스 반환 (UE5.7 NodeContainer 캐시 패턴)
            if let Some(existing_idx) = mesh_assets.get_index(&mesh_name) {
                log::debug!("[MeshFactory] '{}' already at index {}", mesh_name, existing_idx);
                indices.push(existing_idx);
                continue;
            }

            let gpu_mesh = Self::upload_mesh(
                device,
                mesh,
                &format!("{} {}", model_name, mesh_idx),
            );

            let idx = mesh_assets.register_with_material(&mesh_name, gpu_mesh, 0);
            indices.push(idx);
            newly_registered += 1;
        }

        (indices, newly_registered)
    }

    /// 단일 메시 GPU 업로드
    fn upload_mesh(
        device: &wgpu::Device,
        mesh: &crate::gltf_loader::Mesh,
        label_prefix: &str,
    ) -> MeshGpuData {
        let gpu_vertices: Vec<GpuVertex> = mesh.vertices
            .iter()
            .map(GpuVertex::from_vertex)
            .collect();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Vertex Buffer", label_prefix)),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Index Buffer", label_prefix)),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        MeshGpuData {
            vertex_buffer,
            index_buffer,
            num_indices: mesh.indices.len() as u32,
        }
    }

    /// LOD 메시 그룹 생성 (MSFT_lod 또는 수동 LOD)
    ///
    /// lod_meshes: [LOD0_mesh, LOD1_mesh, LOD2_mesh, ...]
    #[allow(dead_code)]
    pub fn create_lod_mesh(
        device: &wgpu::Device,
        lod_meshes: &[&crate::gltf_loader::Mesh],
        mesh_assets: &mut MeshAssets,
        model_name: &str,
    ) -> crate::renderer::LodMesh {
        use crate::renderer::{LodMesh, LodLevel, BoundingSphere};
        use glam::Vec3;

        let mut levels = Vec::new();
        let lod0_index_count = lod_meshes.first().map(|m| m.indices.len() as u32).unwrap_or(0);

        for (lod_idx, mesh) in lod_meshes.iter().enumerate() {
            let mesh_name = format!("{}__lod{}", model_name, lod_idx);

            let gpu_mesh = Self::upload_mesh(device, mesh, &mesh_name);
            let vertex_count = mesh.vertices.len() as u32;
            let index_count = mesh.indices.len() as u32;
            let mesh_idx = mesh_assets.register_with_material(&mesh_name, gpu_mesh, 0);

            let reduction_ratio = if lod0_index_count > 0 {
                index_count as f32 / lod0_index_count as f32
            } else {
                1.0
            };

            levels.push(LodLevel {
                mesh_idx,
                vertex_count,
                index_count,
                reduction_ratio,
            });
        }

        // Compute bounding sphere from LOD0 (highest detail)
        let bounds = if let Some(lod0) = lod_meshes.first() {
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            for v in &lod0.vertices {
                let p = Vec3::from_array(v.position);
                min = min.min(p);
                max = max.max(p);
            }
            BoundingSphere::from_aabb(min, max)
        } else {
            BoundingSphere::new(Vec3::ZERO, 1.0)
        };

        LodMesh { levels, bounds }
    }
}
