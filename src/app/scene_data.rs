//! Scene Render Data — GT에서 수집한 렌더링 스냅샷
//!
//! GT(Game Thread)가 ECS 쿼리로 수집하여 RT(Render Thread)로 전송하는 데이터.
//! 모든 타입은 Send — raw pointer 없이 값으로 복사.

use skope_core as ecs_resources;
use crate::editor::gizmo::{GizmoMode, GizmoAxis};

/// GT에서 ECS 쿼리로 수집한 Send 가능 렌더링 스냅샷
///
/// RT의 RenderState가 이 데이터를 받아 GPU 커맨드를 인코딩.
/// World/Entity 참조 없음 — 모든 데이터가 값으로 복사됨.
#[allow(dead_code)]
pub struct SceneRenderData {
    pub frame_number: u64,
    pub delta_time: f32,

    // 카메라
    pub scene_camera: super::data_types::CameraRenderData,
    pub game_camera: Option<super::data_types::CameraRenderData>,
    pub viewport_size: Option<(u32, u32)>,
    pub game_viewport_size: Option<(u32, u32)>,

    // 메시 인스턴스 (GT에서 frustum cull 완료)
    pub mesh_instances: Vec<MeshInstanceData>,

    // GPU 에셋 (프레임당 clone — wgpu Arc 내부)
    pub mesh_assets: ecs_resources::MeshAssets,
    pub material_assets: ecs_resources::MaterialAssets,

    // 라이팅
    pub extracted_lighting: ecs_resources::ExtractedLighting,
    pub environment: ecs_resources::Environment,
    pub light_buffer: Option<wgpu::Buffer>,
    pub light_count_buffer: Option<wgpu::Buffer>,
    pub total_light_count: u32,
    pub gpu_lights_for_culling: Vec<skope_lighting::GpuLight>,

    // 디버그
    pub debug_view_mode: u32,
    pub debug_params: DebugRenderParams,
    pub debug_draw_primitives: Vec<crate::debug::draw::DebugPrimitive>,

    // 윈도우 크기 (CameraUniform aspect)
    pub window_size: (u32, u32),

    // 오버레이 (Grid + Gizmo)
    pub overlay: Option<OverlayRenderData>,
}

// Safety: SceneRenderData contains only value types and wgpu Send types
// (Buffer, BindGroup are internally Arc — Send+Sync in wgpu 28.0).
// No raw pointers, no Rc, no non-Send types.
unsafe impl Send for SceneRenderData {}

/// GT에서 수집한 오버레이 렌더링 데이터 (Grid + Gizmo)
#[derive(Clone)]
pub struct OverlayRenderData {
    pub show_grid: bool,
    pub gizmo_mode: GizmoMode,
    pub gizmo_position: glam::Vec3,
    pub gizmo_rotation: glam::Quat,
    pub gizmo_scale: f32,
    pub gizmo_hovered_axis: GizmoAxis,
    pub has_selection: bool,
    pub screen_size: (u32, u32),
}

/// Frustum-culled 메시 인스턴스 데이터 (Send)
pub struct MeshInstanceData {
    pub entity_bits: u64,       // Entity::to_bits()
    pub mesh_index: usize,
    pub material_index: usize,
    pub world_transform: glam::Mat4,
}

/// 디버그 렌더링 파라미터 (DebugUi에서 추출)
#[derive(Clone, Copy)]
pub struct DebugRenderParams {
    pub intensity_scale: f32,
    pub d_ggx_max: f32,
    pub specular_max: f32,
    pub roughness_min: f32,
}

/// SharedViewportHandle — GT/RT 간 뷰포트 텍스처 공유 (후속 Step에서 활용)
#[allow(dead_code)]
///
/// RT가 렌더링한 뷰포트 텍스처를 GT의 UI에서 읽어 표시.
/// wgpu::TextureView는 Send+Sync (wgpu 28).
pub struct SharedViewportHandle {
    view: std::sync::Arc<std::sync::RwLock<Option<wgpu::TextureView>>>,
    size: std::sync::Arc<(std::sync::atomic::AtomicU32, std::sync::atomic::AtomicU32)>,
}

impl SharedViewportHandle {
    pub fn new() -> Self {
        Self {
            view: std::sync::Arc::new(std::sync::RwLock::new(None)),
            size: std::sync::Arc::new((
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            )),
        }
    }

    /// RT: 뷰포트 텍스처 업데이트
    pub fn update(&self, new_view: wgpu::TextureView, width: u32, height: u32) {
        *self.view.write().unwrap() = Some(new_view);
        self.size.0.store(width, std::sync::atomic::Ordering::Relaxed);
        self.size.1.store(height, std::sync::atomic::Ordering::Relaxed);
    }

    /// GT: 뷰포트 텍스처 읽기 (UI 표시용)
    pub fn get_view(&self) -> Option<std::sync::RwLockReadGuard<'_, Option<wgpu::TextureView>>> {
        let guard = self.view.read().ok()?;
        if guard.is_some() {
            Some(guard)
        } else {
            None
        }
    }

    /// GT: 뷰포트 크기 읽기
    pub fn size(&self) -> (u32, u32) {
        (
            self.size.0.load(std::sync::atomic::Ordering::Relaxed),
            self.size.1.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// 핸들 복제 (Arc clone)
    pub fn clone_handle(&self) -> Self {
        Self {
            view: std::sync::Arc::clone(&self.view),
            size: std::sync::Arc::clone(&self.size),
        }
    }
}

impl Clone for SharedViewportHandle {
    fn clone(&self) -> Self {
        self.clone_handle()
    }
}
