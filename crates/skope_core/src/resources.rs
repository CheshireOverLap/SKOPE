//! ECS Resources for SKOPE Engine
//!
//! Core resources shared across all engine modules.

use skope_ecs::prelude::*;
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
///
/// wgpu 28.0: Buffer is internally Arc — Clone is cheap (atomic ref count).
#[derive(Clone)]
pub struct MeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

/// Mesh assets (all loaded meshes with name indexing)
#[derive(Resource, Default, Clone)]
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
///
/// wgpu 28.0: BindGroup is internally Arc — Clone is cheap (atomic ref count).
#[derive(Clone)]
pub struct MaterialGpuData {
    pub material_bind_group: wgpu::BindGroup,
    pub deferred_bind_group: Option<wgpu::BindGroup>,
}

/// Material assets (all loaded materials)
#[derive(Resource, Default, Clone)]
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
/// ECS camera_extract_system이 채움. 셰이크/블렌드 등 모든 후처리가 적용된 최종 상태.
#[derive(Clone, Debug)]
pub struct ExtractedCamera {
    pub position: Vec3,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection: Mat4,
    pub forward: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub near: f32,
    pub far: f32,
    pub fov: f32,
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

/// 추출된 라이팅 데이터 (UE5 확장)
#[derive(Clone, Debug)]
pub struct ExtractedLighting {
    // Core fields
    pub sun_direction: Vec3,
    pub sun_color: Vec3,
    pub sun_intensity: f32,
    pub ambient_color: Vec3,
    // UE5 extension fields
    /// PCSS source radius (from light_source_angle)
    pub sun_source_radius: f32,
    /// Specular contribution scale
    pub sun_specular_scale: f32,
    /// Diffuse contribution scale
    pub sun_diffuse_scale: f32,
    /// Shadow darkness (0=no shadow, 1=full shadow)
    pub sun_shadow_amount: f32,
    /// UE5 cascade distribution exponent
    pub cascade_distribution_exponent: f32,
    /// Dynamic shadow max distance
    pub dynamic_shadow_distance: f32,
    /// Number of shadow cascades
    pub shadow_cascade_count: u32,
}

impl Default for ExtractedLighting {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(0.0, -1.0, 0.0),
            sun_color: Vec3::ZERO,
            sun_intensity: 0.0,
            ambient_color: Vec3::new(0.05, 0.05, 0.05),
            // UE5 defaults
            sun_source_radius: 0.00467, // sin(0.5357 * 0.5 * PI / 180)
            sun_specular_scale: 1.0,
            sun_diffuse_scale: 1.0,
            sun_shadow_amount: 1.0,
            cascade_distribution_exponent: 3.0,
            dynamic_shadow_distance: 200.0,
            shadow_cascade_count: 4,
        }
    }
}

/// 대기-태양 상호작용 데이터 (AtmosphereSunData)
#[derive(Resource, Clone, Debug)]
pub struct AtmosphereSunData {
    /// 정규화된 태양 방향 (from sun toward ground)
    pub sun_direction: Vec3,
    /// 태양 고도각 (도)
    pub sun_elevation: f32,
    /// 태양 방위각 (도)
    pub sun_azimuth: f32,
    /// 대기 투과율 (RGB)
    pub atmosphere_transmittance: Vec3,
    /// 투과율 × base_color × kelvin 적용 최종 색상
    pub effective_sun_color: Vec3,
    /// intensity × transmittance luminance
    pub effective_sun_intensity: f32,
}

impl Default for AtmosphereSunData {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(0.0, -1.0, 0.0),
            sun_elevation: 45.0,
            sun_azimuth: 180.0,
            atmosphere_transmittance: Vec3::ONE,
            effective_sun_color: Vec3::ONE,
            effective_sun_intensity: 1.0,
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

// ============ Camera Shake Resource ============

use crate::components::camera::{CameraShakeInstance, CameraShakeDef, ViewBlendParams};

/// Active camera shakes (managed per-camera or globally)
#[derive(Resource, Default)]
pub struct ActiveCameraShakes {
    pub shakes: Vec<CameraShakeInstance>,
}

impl ActiveCameraShakes {
    /// Add a new shake from a definition with optional scale
    pub fn play(&mut self, def: CameraShakeDef, scale: f32) {
        self.shakes.push(CameraShakeInstance::new(def, scale));
    }

    /// Stop all active shakes
    pub fn stop_all(&mut self) {
        self.shakes.clear();
    }

    /// Remove finished shakes
    pub fn cleanup(&mut self) {
        self.shakes.retain(|s| s.is_playing);
    }
}

// ============ View Target Blend Resource ============

/// Camera view data for blending
#[derive(Clone, Debug)]
pub struct CameraViewState {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

impl Default for CameraViewState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            fov: 45.0_f32.to_radians(),
        }
    }
}

/// View target blend state (UE5 PlayerCameraManager blend)
#[derive(Resource)]
pub struct ViewTargetBlend {
    /// Source camera state (blend from)
    pub from: CameraViewState,
    /// Target camera state (blend to)
    pub to: CameraViewState,
    /// Blend parameters
    pub params: ViewBlendParams,
    /// Time remaining in the blend
    pub time_remaining: f32,
    /// Whether a blend is currently active
    pub is_active: bool,
}

impl Default for ViewTargetBlend {
    fn default() -> Self {
        Self {
            from: CameraViewState::default(),
            to: CameraViewState::default(),
            params: ViewBlendParams::default(),
            time_remaining: 0.0,
            is_active: false,
        }
    }
}

impl ViewTargetBlend {
    /// Start a new blend from current state to target
    pub fn start(&mut self, from: CameraViewState, to: CameraViewState, params: ViewBlendParams) {
        self.from = from;
        self.to = to;
        self.params = params;
        self.time_remaining = params.blend_time;
        self.is_active = true;
    }

    /// Update the blend and return interpolated state, or None if not active
    pub fn update(&mut self, dt: f32) -> Option<CameraViewState> {
        if !self.is_active {
            return None;
        }

        self.time_remaining -= dt;
        if self.time_remaining <= 0.0 {
            self.is_active = false;
            return Some(self.to.clone());
        }

        let elapsed = self.params.blend_time - self.time_remaining;
        let t = (elapsed / self.params.blend_time).clamp(0.0, 1.0);
        let alpha = self.params.evaluate(t);

        Some(CameraViewState {
            position: self.from.position.lerp(self.to.position, alpha),
            yaw: lerp_angle(self.from.yaw, self.to.yaw, alpha),
            pitch: self.from.pitch + (self.to.pitch - self.from.pitch) * alpha,
            fov: self.from.fov + (self.to.fov - self.from.fov) * alpha,
        })
    }
}

/// Lerp between two angles (radians), taking the shortest path
fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut diff = b - a;
    // Normalize to [-PI, PI]
    while diff > std::f32::consts::PI { diff -= std::f32::consts::TAU; }
    while diff < -std::f32::consts::PI { diff += std::f32::consts::TAU; }
    a + diff * t
}

