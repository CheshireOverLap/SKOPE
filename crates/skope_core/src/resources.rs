//! ECS Resources for SKOPE Engine
//!
//! Core resources shared across all engine modules.

use bevy_ecs::prelude::*;
use glam::{Mat4, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use winit::keyboard::KeyCode;

// ============ GPU Resources ============

/// GPU context (Device, Queue)
#[derive(Resource, Clone)]
pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

// ============ Asset Resources ============

/// Mesh GPU data
pub struct MeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

/// Mesh assets (all loaded meshes with name indexing)
#[derive(Resource, Default)]
pub struct MeshAssets {
    pub meshes: Vec<MeshGpuData>,
    pub name_to_index: HashMap<String, usize>,
    pub mesh_to_material: HashMap<usize, usize>,
}

impl MeshAssets {
    /// Register a mesh with a name
    pub fn register(&mut self, name: &str, mesh: MeshGpuData) -> usize {
        let index = self.meshes.len();
        self.meshes.push(mesh);
        self.name_to_index.insert(name.to_string(), index);
        index
    }

    /// Register a mesh with a name and associated material index
    pub fn register_with_material(&mut self, name: &str, mesh: MeshGpuData, material_index: usize) -> usize {
        let index = self.meshes.len();
        self.meshes.push(mesh);
        self.name_to_index.insert(name.to_string(), index);
        self.mesh_to_material.insert(index, material_index);
        index
    }

    /// Get mesh index by name
    pub fn get_index(&self, name: &str) -> Option<usize> {
        self.name_to_index.get(name).copied()
    }

    /// Get material index for a given mesh index
    pub fn get_material_index(&self, mesh_index: usize) -> Option<usize> {
        self.mesh_to_material.get(&mesh_index).copied()
    }

    /// Get mesh by name
    pub fn get(&self, name: &str) -> Option<&MeshGpuData> {
        self.get_index(name).map(|idx| &self.meshes[idx])
    }
}

/// Material GPU data
pub struct MaterialGpuData {
    pub material_bind_group: wgpu::BindGroup,
    pub deferred_bind_group: Option<wgpu::BindGroup>,
}

/// Material assets (all loaded materials)
#[derive(Resource, Default)]
pub struct MaterialAssets {
    pub materials: Vec<MaterialGpuData>,
}

// ============ Skinned Mesh Resources ============

/// 스킨드 메시 GPU 데이터
pub struct SkinnedMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub skin_index: usize,
}

/// 스킨드 메시 에셋들
#[derive(Resource, Default)]
pub struct SkinnedMeshAssets {
    pub meshes: Vec<SkinnedMeshGpuData>,
    pub name_to_index: HashMap<String, usize>,
}

impl SkinnedMeshAssets {
    pub fn register(&mut self, name: &str, mesh: SkinnedMeshGpuData) -> usize {
        let index = self.meshes.len();
        self.meshes.push(mesh);
        self.name_to_index.insert(name.to_string(), index);
        index
    }
}

/// 스킨(스켈레톤) 데이터 (CPU 측)
#[derive(Debug, Clone)]
pub struct SkinData {
    pub name: String,
    pub joint_count: usize,
    pub inverse_bind_matrices: Vec<Mat4>,
}

/// 스킨 에셋들
#[derive(Resource, Default)]
pub struct SkinAssets {
    pub skins: Vec<SkinData>,
}

/// 본 매트릭스 GPU 버퍼 (스켈레톤당 하나)
pub struct JointMatrixBuffer {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

// ============ Input Resources ============

/// Keyboard input state
#[derive(Resource, Default)]
pub struct KeyboardInput {
    pub keys_pressed: HashSet<KeyCode>,
}

/// Mouse input state
#[derive(Resource, Default)]
pub struct MouseInput {
    pub is_pressed: bool,
    pub last_pos: Option<(f64, f64)>,
}

// ============ Time Resource ============

/// Delta time and elapsed time
#[derive(Resource)]
pub struct Time {
    pub delta_seconds: f32,
    pub elapsed_seconds: f64,
    pub frame_count: u64,
    last_update: std::time::Instant,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            delta_seconds: 0.016,
            elapsed_seconds: 0.0,
            frame_count: 0,
            last_update: std::time::Instant::now(),
        }
    }
}

impl Time {
    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_seconds = (now - self.last_update).as_secs_f32();
        self.elapsed_seconds += self.delta_seconds as f64;
        self.frame_count += 1;
        self.last_update = now;
    }
}

// ============ Window Resource ============

/// Window size
#[derive(Resource)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

// ============ Render Extracted Data ============

/// 추출된 카메라 데이터 (렌더링용)
#[derive(Clone, Debug)]
pub struct ExtractedCamera {
    pub position: Vec3,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection: Mat4,
    pub forward: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// 추출된 메시 인스턴스 데이터
#[derive(Clone, Debug)]
pub struct ExtractedMeshInstance {
    pub mesh_index: usize,
    pub material_index: usize,
    pub world_transform: Mat4,
}

/// 추출된 스킨드 메시 인스턴스 데이터
#[derive(Clone, Debug)]
pub struct ExtractedSkinnedInstance {
    pub skinned_mesh_index: usize,
    pub material_index: usize,
    pub world_transform: Mat4,
    pub joint_matrices: Vec<Mat4>,
}

/// 추출된 라이팅 데이터
#[derive(Clone, Debug)]
pub struct ExtractedLighting {
    pub sun_direction: Vec3,
    pub sun_color: Vec3,
    pub sun_intensity: f32,
    pub ambient_color: Vec3,
}

impl Default for ExtractedLighting {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(0.0, -1.0, 0.0),
            sun_color: Vec3::ZERO,
            sun_intensity: 0.0,
            ambient_color: Vec3::new(0.05, 0.05, 0.05),
        }
    }
}

/// 프레임별 렌더링 데이터 (Preparation Systems가 채움)
#[derive(Resource, Default)]
pub struct RenderExtractedData {
    pub camera: Option<ExtractedCamera>,
    pub mesh_instances: Vec<ExtractedMeshInstance>,
    pub skinned_instances: Vec<ExtractedSkinnedInstance>,
    pub lighting: ExtractedLighting,
}

impl RenderExtractedData {
    pub fn clear(&mut self) {
        self.camera = None;
        self.mesh_instances.clear();
        self.skinned_instances.clear();
        self.lighting = ExtractedLighting::default();
    }
}

