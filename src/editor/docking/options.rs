//! Docking Options
//!
//! View options for Scene and Game views

use super::types::{SceneRenderMode, GameResolutionPreset, AspectRatioPreset};
use crate::editor::pip_overlay::PipOverlay;

// ============================================================================
// Scene Render Flags (bitflags)
// ============================================================================

bitflags::bitflags! {
    /// Scene 뷰 렌더 플래그
    ///
    /// 각 플래그는 Scene 뷰에서 특정 요소의 표시 여부를 제어합니다.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SceneRenderFlags: u32 {
        /// 그리드 표시
        const GRID = 1 << 0;
        /// 기즈모 표시 (Move/Rotate/Scale)
        const GIZMOS = 1 << 1;
        /// 선택 아웃라인 표시
        const SELECTION_OUTLINE = 1 << 2;
        /// 라이트 아이콘 표시
        const LIGHTS = 1 << 3;
        /// 카메라 아이콘 표시
        const CAMERAS = 1 << 4;
        /// 콜라이더 셰이프 표시
        const COLLIDERS = 1 << 5;
        /// 오디오 소스 아이콘 표시
        const AUDIO_SOURCES = 1 << 6;
        /// 스카이박스 표시
        const SKYBOX = 1 << 7;
        /// 안개 효과 표시
        const FOG = 1 << 8;
        /// 파티클/이펙트 표시
        const PARTICLES = 1 << 9;
        /// 와이어프레임 오버레이
        const WIREFRAME = 1 << 10;
        /// 노말 시각화
        const NORMALS = 1 << 11;
        /// UV 체커 패턴
        const UV_CHECKER = 1 << 12;
        /// 본/스켈레톤 표시
        const BONES = 1 << 13;
        /// 내비메시 표시
        const NAVMESH = 1 << 14;
        /// 바운딩 박스 표시
        const BOUNDS = 1 << 15;

        /// 기본 플래그 (Grid 제외 - Z-fighting 방지)
        const DEFAULT = Self::GIZMOS.bits()
            | Self::SELECTION_OUTLINE.bits()
            | Self::LIGHTS.bits()
            | Self::CAMERAS.bits()
            | Self::SKYBOX.bits()
            | Self::FOG.bits()
            | Self::PARTICLES.bits();

        /// 디버그 모드 플래그
        const DEBUG = Self::DEFAULT.bits()
            | Self::GRID.bits()
            | Self::COLLIDERS.bits()
            | Self::BOUNDS.bits();

        /// 모든 플래그
        const ALL = Self::GRID.bits()
            | Self::GIZMOS.bits()
            | Self::SELECTION_OUTLINE.bits()
            | Self::LIGHTS.bits()
            | Self::CAMERAS.bits()
            | Self::COLLIDERS.bits()
            | Self::AUDIO_SOURCES.bits()
            | Self::SKYBOX.bits()
            | Self::FOG.bits()
            | Self::PARTICLES.bits()
            | Self::WIREFRAME.bits()
            | Self::NORMALS.bits()
            | Self::UV_CHECKER.bits()
            | Self::BONES.bits()
            | Self::NAVMESH.bits()
            | Self::BOUNDS.bits();
    }
}

impl Default for SceneRenderFlags {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl SceneRenderFlags {
    /// 플래그 이름 (UI 표시용)
    pub fn flag_names() -> &'static [(Self, &'static str, &'static str)] {
        &[
            (Self::GRID, "Grid", "Show grid overlay"),
            (Self::GIZMOS, "Gizmos", "Show transform gizmos"),
            (Self::SELECTION_OUTLINE, "Selection", "Show selection outline"),
            (Self::LIGHTS, "Lights", "Show light icons"),
            (Self::CAMERAS, "Cameras", "Show camera icons"),
            (Self::COLLIDERS, "Colliders", "Show collider shapes"),
            (Self::AUDIO_SOURCES, "Audio", "Show audio source icons"),
            (Self::SKYBOX, "Skybox", "Show skybox"),
            (Self::FOG, "Fog", "Show fog effect"),
            (Self::PARTICLES, "Particles", "Show particles"),
            (Self::WIREFRAME, "Wireframe", "Show wireframe overlay"),
            (Self::NORMALS, "Normals", "Visualize normals"),
            (Self::UV_CHECKER, "UV Checker", "Show UV checker pattern"),
            (Self::BONES, "Bones", "Show skeleton bones"),
            (Self::NAVMESH, "NavMesh", "Show navigation mesh"),
            (Self::BOUNDS, "Bounds", "Show bounding boxes"),
        ]
    }
}

// ============================================================================
// Letterbox Calculation
// ============================================================================

/// 레터박스 계산 결과
#[derive(Debug, Clone, Copy)]
pub struct LetterboxResult {
    /// 실제 콘텐츠가 렌더링될 영역
    pub content_rect: egui::Rect,
    /// 상단 레터박스 영역 (None이면 없음)
    pub top_bar: Option<egui::Rect>,
    /// 하단 레터박스 영역 (None이면 없음)
    pub bottom_bar: Option<egui::Rect>,
    /// 좌측 레터박스 영역 (None이면 없음)
    pub left_bar: Option<egui::Rect>,
    /// 우측 레터박스 영역 (None이면 없음)
    pub right_bar: Option<egui::Rect>,
}

/// 레터박스 계산
///
/// 지정된 종횡비로 뷰포트 중앙에 콘텐츠를 배치하고,
/// 남는 영역은 레터박스(검은 바)로 채웁니다.
pub fn calculate_letterbox(available: egui::Rect, target_aspect: f32) -> LetterboxResult {
    let available_aspect = available.width() / available.height();

    if (available_aspect - target_aspect).abs() < 0.001 {
        // 종횡비가 거의 일치하면 레터박스 없음
        return LetterboxResult {
            content_rect: available,
            top_bar: None,
            bottom_bar: None,
            left_bar: None,
            right_bar: None,
        };
    }

    if available_aspect > target_aspect {
        // 컨테이너가 더 넓음 → 좌우에 레터박스
        let new_width = available.height() * target_aspect;
        let offset = (available.width() - new_width) / 2.0;

        let content_rect = egui::Rect::from_min_size(
            egui::pos2(available.min.x + offset, available.min.y),
            egui::vec2(new_width, available.height()),
        );

        let left_bar = egui::Rect::from_min_max(
            available.min,
            egui::pos2(content_rect.min.x, available.max.y),
        );

        let right_bar = egui::Rect::from_min_max(
            egui::pos2(content_rect.max.x, available.min.y),
            available.max,
        );

        LetterboxResult {
            content_rect,
            top_bar: None,
            bottom_bar: None,
            left_bar: Some(left_bar),
            right_bar: Some(right_bar),
        }
    } else {
        // 컨테이너가 더 높음 → 상하에 레터박스
        let new_height = available.width() / target_aspect;
        let offset = (available.height() - new_height) / 2.0;

        let content_rect = egui::Rect::from_min_size(
            egui::pos2(available.min.x, available.min.y + offset),
            egui::vec2(available.width(), new_height),
        );

        let top_bar = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.max.x, content_rect.min.y),
        );

        let bottom_bar = egui::Rect::from_min_max(
            egui::pos2(available.min.x, content_rect.max.y),
            available.max,
        );

        LetterboxResult {
            content_rect,
            top_bar: Some(top_bar),
            bottom_bar: Some(bottom_bar),
            left_bar: None,
            right_bar: None,
        }
    }
}

/// AspectRatioPreset을 사용한 레터박스 계산
pub fn calculate_letterbox_with_preset(
    available: egui::Rect,
    preset: AspectRatioPreset,
) -> Option<LetterboxResult> {
    preset.ratio().map(|ratio| calculate_letterbox(available, ratio))
}

/// Scene view options (for top toolbar)
#[derive(Debug, Clone)]
pub struct SceneViewOptions {
    pub show_grid: bool,
    pub show_gizmos: bool,
    pub render_mode: SceneRenderMode,
    pub is_2d_mode: bool,
    pub show_skybox: bool,
    pub show_fog: bool,
    pub show_lighting: bool,
    pub show_audio: bool,
    pub show_effects: bool,
    /// PiP 오버레이 설정
    pub pip_config: PipOverlay,
}

impl Default for SceneViewOptions {
    fn default() -> Self {
        Self {
            show_grid: false, // Default OFF - prevent Z-fighting, toggle when needed
            show_gizmos: true,
            render_mode: SceneRenderMode::Shaded,
            is_2d_mode: false,
            show_skybox: true,
            show_fog: true,
            show_lighting: true,
            show_audio: true,
            show_effects: true,
            pip_config: PipOverlay::default(),
        }
    }
}

// ============================================================================
// Debug Render Flags (Game View)
// ============================================================================

bitflags::bitflags! {
    /// Game 뷰 디버그 렌더 플래그
    ///
    /// Play 모드에서 디버그 정보 표시를 제어합니다.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DebugRenderFlags: u32 {
        /// FPS/메모리 등 성능 통계
        const STATS = 1 << 0;
        /// 콜라이더 와이어프레임
        const COLLIDERS = 1 << 1;
        /// 내비메시 표시
        const NAV_MESH = 1 << 2;
        /// 오디오 소스 범위
        const AUDIO_SOURCES = 1 << 3;
        /// 바운딩 박스
        const BOUNDS = 1 << 4;
        /// 물리 속도 벡터
        const VELOCITY_VECTORS = 1 << 5;
        /// 레이캐스트/센서 표시
        const RAYCASTS = 1 << 6;
        /// AI 경로/목표 표시
        const AI_DEBUG = 1 << 7;
        /// 그리드 오버레이
        const GRID = 1 << 8;
    }
}

impl DebugRenderFlags {
    /// 플래그 이름 및 설명 (UI 표시용)
    pub fn flag_info() -> &'static [(Self, &'static str, &'static str)] {
        &[
            (Self::STATS, "Stats", "FPS, memory, draw calls"),
            (Self::COLLIDERS, "Colliders", "Collider wireframes"),
            (Self::NAV_MESH, "NavMesh", "Navigation mesh"),
            (Self::AUDIO_SOURCES, "Audio", "Audio source ranges"),
            (Self::BOUNDS, "Bounds", "Bounding boxes"),
            (Self::VELOCITY_VECTORS, "Velocity", "Physics velocity vectors"),
            (Self::RAYCASTS, "Raycasts", "Raycast/sensor visualization"),
            (Self::AI_DEBUG, "AI Debug", "AI paths and targets"),
            (Self::GRID, "Grid", "Grid overlay"),
        ]
    }
}

/// Game view options (for top toolbar)
#[derive(Debug, Clone)]
pub struct GameViewOptions {
    pub display_index: usize,
    pub resolution: GameResolutionPreset,
    pub scale: f32,
    pub maximize_on_play: bool,
    pub mute_audio: bool,
    pub show_stats: bool,
    pub show_gizmos: bool,
    /// 디버그 렌더 플래그
    pub debug_flags: DebugRenderFlags,
}

impl Default for GameViewOptions {
    fn default() -> Self {
        Self {
            display_index: 1,
            resolution: GameResolutionPreset::FreeAspect,
            scale: 1.0,
            maximize_on_play: false,
            mute_audio: false,
            show_stats: false,
            show_gizmos: false,
            debug_flags: DebugRenderFlags::empty(),
        }
    }
}
