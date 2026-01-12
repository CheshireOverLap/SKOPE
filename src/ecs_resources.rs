// ECS Resources for SKOPE Engine
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};
use winit::keyboard::KeyCode;

// skope_gltf 크레이트 (gltf_loader alias)
use skope_gltf;

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

/// Fox 스킨드 메시 전용 머티리얼 (임시)
#[derive(Resource)]
pub struct FoxMaterialRes {
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
}

// ============ Environment Resources ============

/// 앰비언트 라이트 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AmbientLight {
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for AmbientLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            intensity: 0.15,
        }
    }
}

/// 스카이 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SkySettings {
    /// 그라데이션 색상
    Gradient {
        top: [f32; 3],
        bottom: [f32; 3],
    },
    /// HDRI 환경맵
    Hdri {
        path: String,
        intensity: f32,
    },
    /// 절차적 하늘
    Procedural {
        sun_size: f32,
        atmosphere: bool,
    },
    /// 단색
    SolidColor([f32; 3]),
}

impl Default for SkySettings {
    fn default() -> Self {
        Self::Gradient {
            top: [0.05, 0.15, 0.4],
            bottom: [0.15, 0.25, 0.45],
        }
    }
}

/// 안개 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FogSettings {
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
    pub density: f32,
}

impl Default for FogSettings {
    fn default() -> Self {
        Self {
            color: [0.5, 0.6, 0.7],
            start: 20.0,
            end: 100.0,
            density: 0.02,
        }
    }
}

/// 씬 환경 리소스
#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Environment {
    pub ambient: AmbientLight,
    pub sky: SkySettings,
    pub fog: Option<FogSettings>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            ambient: AmbientLight::default(),
            sky: SkySettings::default(),
            fog: None,
        }
    }
}

impl Environment {
    /// 밝은 실외 환경
    pub fn outdoor() -> Self {
        Self {
            ambient: AmbientLight {
                color: [0.9, 0.95, 1.0],
                intensity: 0.2,
            },
            sky: SkySettings::Gradient {
                top: [0.3, 0.5, 0.9],
                bottom: [0.7, 0.8, 0.9],
            },
            fog: None,
        }
    }

    /// 실내 환경
    pub fn indoor() -> Self {
        Self {
            ambient: AmbientLight {
                color: [1.0, 0.95, 0.9],
                intensity: 0.1,
            },
            sky: SkySettings::SolidColor([0.1, 0.1, 0.1]),
            fog: None,
        }
    }

    /// 안개 추가
    pub fn with_fog(mut self, fog: FogSettings) -> Self {
        self.fog = Some(fog);
        self
    }
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

/// 독립 머티리얼 이름 → GPU 머티리얼 인덱스 매핑
#[derive(Resource, Default)]
pub struct StandaloneMaterialMap {
    /// 머티리얼 이름 → GPU 머티리얼 인덱스
    pub name_to_index: HashMap<String, u32>,
    /// 머티리얼 파일 경로 → GPU 머티리얼 인덱스
    pub path_to_index: HashMap<String, u32>,
}

impl StandaloneMaterialMap {
    /// 이름으로 GPU 머티리얼 인덱스 조회
    pub fn get_by_name(&self, name: &str) -> Option<u32> {
        self.name_to_index.get(name).copied()
    }

    /// 경로로 GPU 머티리얼 인덱스 조회
    pub fn get_by_path(&self, path: &str) -> Option<u32> {
        self.path_to_index.get(path).copied()
    }
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

// ============ Skinned Model Registry (여러 스켈레탈 모델 지원) ============

/// 스킨드 모델 머티리얼 바인드 그룹
pub struct SkinnedMaterialBindGroups {
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
}

/// 스킨드 모델 데이터 (로드된 스켈레탈 모델 정보)
pub struct SkinnedModelData {
    pub name: String,
    /// GPU 메시 데이터 인덱스들 (SkinnedMeshAssets)
    pub mesh_indices: Vec<usize>,
    /// 스킨 인덱스 (SkinAssets)
    pub skin_index: usize,
    /// 애니메이션 클립들
    pub animations: Vec<skope_gltf::Animation>,
    /// 노드 계층 구조
    pub nodes: Vec<skope_gltf::SceneNode>,
    /// 스킨 데이터 (조인트 정보 포함)
    pub skin: skope_gltf::Skin,
    /// 머티리얼 바인드 그룹들
    pub material_bind_groups: Vec<SkinnedMaterialBindGroups>,
}

/// 스킨드 모델 레지스트리 (여러 스켈레탈 모델 관리)
#[derive(Resource, Default)]
pub struct SkinnedModelRegistry {
    pub models: HashMap<String, SkinnedModelData>,
}

impl SkinnedModelRegistry {
    /// 모델 등록
    pub fn register(&mut self, data: SkinnedModelData) {
        log::info!("[SkinnedModelRegistry] Registered '{}' with {} meshes, {} animations",
            data.name, data.mesh_indices.len(), data.animations.len());
        self.models.insert(data.name.clone(), data);
    }

    /// 모델 조회
    pub fn get(&self, name: &str) -> Option<&SkinnedModelData> {
        self.models.get(name)
    }

    /// 등록된 모델 이름 목록
    pub fn model_names(&self) -> Vec<&str> {
        self.models.keys().map(|s| s.as_str()).collect()
    }
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
    pub previous_state: PlayState,
    pub step_requested: bool,
    /// 플레이어가 스폰되었는지 여부
    pub player_spawned: bool,
    /// 스폰된 플레이어 엔티티
    pub player_entity: Option<bevy_ecs::entity::Entity>,
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

    /// 상태 전환 감지 (Edit → Playing)
    pub fn just_started_playing(&self) -> bool {
        self.previous_state.is_edit() && self.state.is_playing()
    }

    /// 상태 전환 감지 (Playing → Edit)
    pub fn just_stopped_playing(&self) -> bool {
        self.previous_state.is_playing() && self.state.is_edit()
    }

    /// 상태 업데이트 (이전 상태 저장)
    pub fn update_state(&mut self, new_state: PlayState) {
        self.previous_state = self.state;
        self.state = new_state;
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
