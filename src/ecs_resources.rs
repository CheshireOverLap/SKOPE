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
    /// Mesh index → material index mapping (for glTF meshes with materials)
    pub mesh_to_material: HashMap<usize, usize>,
}

impl MeshAssets {
    /// Register a mesh with a name
    pub fn register(&mut self, name: &str, mesh: MeshGpuData) -> usize {
        let index = self.meshes.len();
        self.meshes.push(mesh);
        self.name_to_index.insert(name.to_string(), index);
        log::info!("[MeshAssets] Registered '{}' at index {}", name, index);
        index
    }

    /// Register a mesh with a name and associated material index
    pub fn register_with_material(&mut self, name: &str, mesh: MeshGpuData, material_index: usize) -> usize {
        let index = self.meshes.len();
        self.meshes.push(mesh);
        self.name_to_index.insert(name.to_string(), index);
        self.mesh_to_material.insert(index, material_index);
        log::info!("[MeshAssets] Registered '{}' at index {} with material {}", name, index, material_index);
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
        log::info!("[SkinnedMeshAssets] Registered '{}' at index {}", name, index);
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

// ============ Play State Resource ============

/// 게임 플레이 상태 (ECS에서 접근 가능)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    #[default]
    Edit,
    Playing,
    Paused,
}

impl PlayState {
    pub fn is_playing(&self) -> bool {
        matches!(self, PlayState::Playing)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, PlayState::Paused)
    }

    pub fn is_edit(&self) -> bool {
        matches!(self, PlayState::Edit)
    }

    pub fn is_running(&self) -> bool {
        // Playing 또는 Paused (Edit 아님)
        !self.is_edit()
    }
}

/// 게임 플레이 상태 리소스
#[derive(Resource, Default)]
pub struct GamePlayState {
    pub state: PlayState,
    pub step_requested: bool,
}

impl GamePlayState {
    /// 게임 로직을 실행해야 하는지 (Playing 또는 Step 요청 시)
    pub fn should_run_gameplay(&self) -> bool {
        self.state.is_playing() || self.step_requested
    }

    /// Step 완료 후 플래그 리셋
    pub fn clear_step(&mut self) {
        self.step_requested = false;
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
    pub manager: skope_lighting::LightManager,
}

// ============ Hair Resource ============

/// Hybrid Hair Renderer wrapper for ECS
#[derive(Resource)]
pub struct HairRendererRes {
    pub renderer: skope_hair::HybridHairRenderer,
}

// ============ Render Extracted Data ============

use glam::{Mat4, Vec3};

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
            sun_direction: Vec3::new(-0.5, -1.0, -0.3).normalize(),
            sun_color: Vec3::new(1.0, 0.98, 0.95),
            sun_intensity: 3.0,
            ambient_color: Vec3::new(0.03, 0.03, 0.05),
        }
    }
}

/// 프레임별 렌더링 데이터 (Preparation Systems가 채움)
#[derive(Resource, Default)]
pub struct RenderExtractedData {
    /// 카메라 데이터
    pub camera: Option<ExtractedCamera>,
    /// 렌더링할 메시 인스턴스들
    pub mesh_instances: Vec<ExtractedMeshInstance>,
    /// 스킨드 메시 인스턴스들
    pub skinned_instances: Vec<ExtractedSkinnedInstance>,
    /// 라이팅 파라미터
    pub lighting: ExtractedLighting,
}

impl RenderExtractedData {
    /// 프레임 시작 시 데이터 클리어
    pub fn clear(&mut self) {
        self.camera = None;
        self.mesh_instances.clear();
        self.skinned_instances.clear();
        self.lighting = ExtractedLighting::default();
    }
}

/// Hair 렌더링용 추출 데이터
#[derive(Resource, Default)]
pub struct HairExtractedData {
    pub elapsed_time: f32,
    pub view_proj: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
    pub camera_pos: Vec3,
}
