// ECS Resources for SKOPE Engine
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};
use winit::keyboard::KeyCode;

// ============ GPU Resources ============

use std::sync::Arc;

/// GPU context (Device, Queue)
#[derive(Resource, Clone)]
pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

/// Surface context (Surface, Config, Depth texture)
#[derive(Resource)]
pub struct SurfaceContext {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::TextureView,
}

/// Render pipeline and bind group layouts
#[derive(Resource)]
pub struct RenderPipelineRes {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
}

/// Skinned mesh render pipeline
#[derive(Resource)]
pub struct SkinnedPipelineRes {
    pub pipeline: wgpu::RenderPipeline,
    pub skinned_uniform_bind_group_layout: wgpu::BindGroupLayout,
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
    /// Name → mesh index mapping (e.g., "Cube" → 0, "models/chair.glb" → 1)
    pub name_to_index: HashMap<String, usize>,
}

impl MeshAssets {
    /// Register a mesh with a name
    pub fn register(&mut self, name: &str, mesh: MeshGpuData) -> usize {
        let index = self.meshes.len();
        self.meshes.push(mesh);
        self.name_to_index.insert(name.to_string(), index);
        println!("[MeshAssets] Registered '{}' at index {}", name, index);
        index
    }

    /// Get mesh index by name
    pub fn get_index(&self, name: &str) -> Option<usize> {
        self.name_to_index.get(name).copied()
    }

    /// Get mesh by name
    pub fn get(&self, name: &str) -> Option<&MeshGpuData> {
        self.get_index(name).map(|idx| &self.meshes[idx])
    }
}

/// Material GPU data
pub struct MaterialGpuData {
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
    // Phase 17: Deferred rendering bind group
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
    pub skin_index: usize,  // 어떤 Skin을 사용하는지
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
        println!("[SkinnedMeshAssets] Registered '{}' at index {}", name, index);
        index
    }
}

/// 스킨(스켈레톤) 데이터 (CPU 측)
#[derive(Debug, Clone)]
pub struct SkinData {
    pub name: String,
    pub joint_count: usize,
    pub inverse_bind_matrices: Vec<glam::Mat4>,  // 각 본의 역 바인드 행렬
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

/// Uniform buffer for MVP matrices
#[derive(Resource)]
pub struct UniformBuffer {
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
            delta_seconds: 0.016,  // 60 FPS 초기값
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

// ============ Lighting Resource ============

/// Light Manager wrapper for ECS
#[derive(Resource)]
pub struct LightManagerRes {
    pub manager: crate::lighting::LightManager,
}

// ============ Hair Resource ============

/// Hybrid Hair Renderer wrapper for ECS
#[derive(Resource)]
pub struct HairRendererRes {
    pub renderer: crate::hair::HybridHairRenderer,
}
