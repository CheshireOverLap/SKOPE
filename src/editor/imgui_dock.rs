//! ImGui Docking Layout for SKOPE Editor
//!
//! ╔══════════════════════════════════════════════════════════════════════════════╗
//! ║  ⚠️  WARNING: DO NOT MODIFY THE NESTED DOCKING STRUCTURE! ⚠️                 ║
//! ╠══════════════════════════════════════════════════════════════════════════════╣
//! ║  이 파일의 도킹 구조는 수많은 시행착오 끝에 완성된 UE5 스타일 레이아웃입니다.   ║
//! ║  "플랫 구조로 바꾸면 간단해질 것 같은데?" → 절대 안 됩니다!                    ║
//! ║                                                                              ║
//! ║  현재 구조:                                                                   ║
//! ║  ┌─────────────────────────────────────────────────────────────────────────┐ ║
//! ║  │ Main DockSpace                                                          │ ║
//! ║  │ ├── [Center] "Map: Untitled" (Document Tab - 외부 탭)                   │ ║
//! ║  │ │   └── Inner DockSpace (LevelEditorDS)                                 │ ║
//! ║  │ │       ├── [Left] Hierarchy                                            │ ║
//! ║  │ │       ├── [Center] ##Viewport (NO_TAB_BAR! 탭 숨김)                   │ ║
//! ║  │ │       └── [Right] Inspector                                           │ ║
//! ║  │ └── [Bottom] Content Browser                                            │ ║
//! ║  └─────────────────────────────────────────────────────────────────────────┘ ║
//! ║                                                                              ║
//! ║  핵심 규칙:                                                                   ║
//! ║  1. "Map: Untitled" = 외부 문서 탭 (항상 표시)                                ║
//! ║  2. Hierarchy/Inspector = Inner DockSpace 안에 존재                          ║
//! ║  3. ##Viewport = NO_TAB_BAR 플래그로 탭 헤더 숨김                             ║
//! ║                                                                              ║
//! ║  수정하려면 반드시 이 주석과 아래 문서를 먼저 읽으세요!                         ║
//! ╚══════════════════════════════════════════════════════════════════════════════╝
//!
//! UE5 스타일 2계층 탭 시스템:
//!
//! ## 핵심 제약 조건 (UE5 Style Constraints)
//!
//! ### 1. NO_SPLIT - 중앙 노드 쪼개기 금지
//! - 중앙 Document 영역은 **탭 스태킹(Tab Stacking)만** 허용
//! - 화면을 좌우로 쪼개서 Viewport + Blueprint 동시 표시 불가
//! - 두 개를 동시에 보려면 탭을 Undock하여 별도 창으로 분리
//!
//! ### 2. Context Switching - 탭 전환 시 UI 변경
//! - 활성화된 탭에 따라 툴바와 사이드 패널 내용 자동 변경
//! - LevelEditor 모드: 이동/회전/스케일 툴, Outliner
//! - BlueprintEditor 모드: 컴파일/저장 버튼, 노드 팔레트
//!
//! ## 용어 정의 (Terminology)
//!
//! ### 1. Document Tabs (문서 탭)
//! - **위치**: 중앙 상단
//! - **예시**: Level Viewport, Blueprint, Material Editor
//! - **동작**: 웹브라우저 탭처럼 동작
//!   - Viewport: 고정 (NO_MOVE) - 뜯어낼 수 없음
//!   - Blueprint 등: 자유 - 듀얼 모니터로 뜯어낼 수 있음
//!
//! ### 2. Tool Panels (도구 패널)
//! - **위치**: 사이드바/하단
//! - **예시**: World Outliner, Details, Content Browser
//! - **동작**: 항상 그 자리에 있거나 접혀 있음, 닫아도 다시 열 수 있음

use bevy_ecs::prelude::*;
use dear_imgui_rs::{
    Ui, WindowFlags, Condition, TextureId, StyleVar, StyleColor,
    DockBuilder, DockNodeFlags, SplitDirection, Id, WindowClass,
    sys as imgui_sys,  // [UE5 Layout] raw API for NoTabBar flag
};

use super::imgui_hierarchy::HierarchyAction;
use super::imgui_inspector::InspectorAction;
use super::imgui_asset_browser::AssetBrowserAction;
use super::imgui_toolbar::{ImGuiToolbar, ToolbarAction};
use super::icons::IconManager;

/// UI 액션 결과
#[derive(Debug, Clone)]
pub enum DockAction {
    None,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    Inspector(InspectorAction),
    Hierarchy(HierarchyAction),
    AssetBrowser(AssetBrowserAction),
    Toolbar(ToolbarAction),
}

/// 에디터 모드 (Context Switching)
///
/// 활성화된 Document Tab에 따라 툴바와 사이드 패널 내용이 변경됩니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// 레벨 에디터 모드 (기본)
    /// - 툴바: 이동/회전/스케일 기즈모, Play 버튼
    /// - 패널: Outliner, Details
    #[default]
    LevelEditor,
    /// 블루프린트 에디터 모드
    /// - 툴바: 컴파일, 저장, 디버그
    /// - 패널: Components, My Blueprint, Details
    BlueprintEditor,
    /// 머티리얼 에디터 모드
    /// - 툴바: Apply, 프리뷰 설정
    /// - 패널: Palette, Details
    MaterialEditor,
    /// 애니메이션 에디터 모드
    /// - 툴바: 재생 컨트롤, 키프레임
    /// - 패널: Timeline, Asset Browser
    AnimationEditor,
}

impl EditorMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            EditorMode::LevelEditor => "Level Editor",
            EditorMode::BlueprintEditor => "Blueprint Editor",
            EditorMode::MaterialEditor => "Material Editor",
            EditorMode::AnimationEditor => "Animation Editor",
        }
    }
}

/// 도킹 ID 상수
mod dock_ids {
    use dear_imgui_rs::Id;

    /// 메인 DockSpace ID (최상위)
    pub const MAIN_DOCKSPACE: u32 = 0x534B4F50; // "SKOP"

    /// ========================================
    /// 중첩 DockSpace IDs (Asset Editor별)
    /// ========================================

    /// Level Editor 내부 DockSpace
    pub const LEVEL_EDITOR_DS: u32 = 0x4C564C45; // "LVLE"

    /// Blueprint Editor 내부 DockSpace
    pub const BLUEPRINT_EDITOR_DS: u32 = 0x424C5545; // "BLUE"

    /// Material Editor 내부 DockSpace
    pub const MATERIAL_EDITOR_DS: u32 = 0x4D415445; // "MATE"

    /// ========================================
    /// WindowClass IDs (VIP 출입 통제 시스템)
    /// ========================================
    /// 서로 다른 ClassId를 가진 윈도우는 도킹 불가!
    ///
    /// Level Editor 클래스 ID
    pub const LEVEL_EDITOR_CLASS: u32 = 1337;  // Level Editor VIP 출입증
    /// Blueprint Editor 클래스 ID
    pub const BLUEPRINT_EDITOR_CLASS: u32 = 1338;
    /// Material Editor 클래스 ID
    pub const MATERIAL_EDITOR_CLASS: u32 = 1339;

    pub fn main_dockspace() -> Id { Id::from(MAIN_DOCKSPACE) }
    pub fn level_editor_ds() -> Id { Id::from(LEVEL_EDITOR_DS) }
    pub fn blueprint_editor_ds() -> Id { Id::from(BLUEPRINT_EDITOR_DS) }
    pub fn material_editor_ds() -> Id { Id::from(MATERIAL_EDITOR_DS) }
}

/// 패널 표시 상태
#[derive(Debug, Clone)]
pub struct PanelVisibility {
    /// World Outliner (Hierarchy) 패널
    pub hierarchy: bool,
    /// Details (Inspector) 패널
    pub inspector: bool,
    /// Content Browser 패널
    pub content_browser: bool,
    /// Output Log 패널
    pub output_log: bool,
    /// AI Assistant 패널
    pub ai_assistant: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            hierarchy: true,
            inspector: true,
            content_browser: true,
            output_log: false,  // 기본적으로 숨김
            ai_assistant: false,
        }
    }
}

/// 뷰포트 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportMode {
    /// 씬 편집 모드 (기본)
    #[default]
    Edit,
    /// 게임 플레이 모드 (Selected Viewport에서 실행)
    Play,
}

/// Play 해상도 옵션
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayResolution {
    /// 뷰포트 크기에 맞춤
    Viewport,
    /// 1920x1080
    FHD,
    /// 1280x720
    HD,
    /// 2560x1440
    QHD,
    /// 3840x2160
    UHD,
    /// 커스텀
    Custom(u32, u32),
}

impl Default for PlayResolution {
    fn default() -> Self {
        Self::Viewport
    }
}

impl PlayResolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Viewport => "Viewport Size",
            Self::FHD => "1920x1080 (FHD)",
            Self::HD => "1280x720 (HD)",
            Self::QHD => "2560x1440 (QHD)",
            Self::UHD => "3840x2160 (4K)",
            Self::Custom(_, _) => "Custom",
        }
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        match self {
            Self::Viewport => None,
            Self::FHD => Some((1920, 1080)),
            Self::HD => Some((1280, 720)),
            Self::QHD => Some((2560, 1440)),
            Self::UHD => Some((3840, 2160)),
            Self::Custom(w, h) => Some((*w, *h)),
        }
    }
}

/// 뷰포트 상태
#[derive(Debug, Clone)]
pub struct ViewportState {
    pub name: String,
    pub texture_id: Option<u64>,
    pub focused: bool,
    pub hovered: bool,
    pub size: (u32, u32),
    pub pos: (f32, f32),
    /// 현재 모드 (Edit/Play)
    pub mode: ViewportMode,
    /// Play 해상도 설정
    pub play_resolution: PlayResolution,
}

impl ViewportState {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            texture_id: None,
            focused: false,
            hovered: false,
            size: (1280, 720),
            pos: (0.0, 0.0),
            mode: ViewportMode::Edit,
            play_resolution: PlayResolution::default(),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.mode == ViewportMode::Play
    }
}

// 호환성을 위한 타입 별칭
pub type ViewportTab = ViewportState;

/// 도킹 레이아웃 상태
pub struct ImGuiDockLayout {
    /// 메인 레이아웃 초기화 완료 여부
    layout_initialized: bool,

    /// Level Editor 내부 레이아웃 초기화 여부
    level_editor_layout_initialized: bool,

    /// Blueprint Editor 내부 레이아웃 초기화 여부
    blueprint_editor_layout_initialized: bool,

    /// 중앙 노드 ID (NO_SPLIT 플래그 적용용)
    center_node_id: Option<Id>,

    /// 현재 에디터 모드 (Context Switching)
    /// 활성화된 Document Tab에 따라 자동 변경됨
    pub current_mode: EditorMode,

    /// 메인 뷰포트 (씬 편집 + 게임 플레이 통합)
    pub viewport: ViewportState,

    /// 패널 표시 상태
    pub panel_visibility: PanelVisibility,

    /// Camera PIP (Picture-in-Picture) 텍스처 ID
    /// 카메라 엔티티 선택 시 미리보기용
    pub camera_pip_texture_id: Option<u64>,
    /// Camera PIP 표시 여부
    pub show_camera_pip: bool,

    // 호환성을 위한 기존 필드들 (deprecated - viewport 참조)
    /// 뷰포트 크기 (width, height)
    pub viewport_size: (u32, u32),
    /// 뷰포트 위치 (x, y)
    pub viewport_pos: (f32, f32),
    /// Scene 뷰포트 텍스처 ID (호환용 - viewport.texture_id와 동일)
    pub scene_viewport_texture_id: Option<u64>,
    /// Game 뷰포트 텍스처 ID (deprecated - 더 이상 사용 안함)
    #[deprecated(note = "Game tab removed - use viewport with Play mode")]
    pub game_viewport_texture_id: Option<u64>,
    /// 뷰포트 포커스 여부 (카메라 조작용)
    pub viewport_focused: bool,
    /// 뷰포트 호버 여부
    pub viewport_hovered: bool,
}

impl ImGuiDockLayout {
    #[allow(deprecated)]
    pub fn new() -> Self {
        Self {
            layout_initialized: false,
            level_editor_layout_initialized: false,
            blueprint_editor_layout_initialized: false,
            center_node_id: None,
            current_mode: EditorMode::default(),
            viewport: ViewportState::new("Viewport"),
            panel_visibility: PanelVisibility::default(),
            camera_pip_texture_id: None,
            show_camera_pip: false,
            // 호환성 필드
            viewport_size: (1280, 720),
            viewport_pos: (0.0, 0.0),
            scene_viewport_texture_id: None,
            game_viewport_texture_id: None,
            viewport_focused: false,
            viewport_hovered: false,
        }
    }

    /// 현재 에디터 모드 가져오기
    pub fn get_current_mode(&self) -> EditorMode {
        self.current_mode
    }

    /// 에디터 모드 설정 (수동 전환용)
    pub fn set_editor_mode(&mut self, mode: EditorMode) {
        if self.current_mode != mode {
            log::info!("[EditorMode] Switched to: {}", mode.display_name());
            self.current_mode = mode;
        }
    }

    /// 뷰포트 텍스처 설정
    pub fn set_viewport_texture(&mut self, texture_id: u64) {
        self.viewport.texture_id = Some(texture_id);
        self.scene_viewport_texture_id = Some(texture_id);
    }

    /// 호환용 (deprecated)
    pub fn set_scene_viewport_texture(&mut self, texture_id: u64) {
        self.set_viewport_texture(texture_id);
    }

    /// 호환용 (deprecated - Game 탭 삭제됨)
    #[allow(deprecated)]
    pub fn set_game_viewport_texture(&mut self, _texture_id: u64) {
        // Game 탭이 삭제되어 더 이상 사용하지 않음
        // 호환성을 위해 함수는 유지하되 아무 동작 안함
    }

    /// Camera PIP 텍스처 설정
    pub fn set_camera_pip_texture(&mut self, texture_id: u64) {
        self.camera_pip_texture_id = Some(texture_id);
    }

    /// Play 모드 시작 (Selected Viewport)
    pub fn start_play(&mut self) {
        self.viewport.mode = ViewportMode::Play;
        log::info!("[Viewport] Play mode started (Selected Viewport)");
    }

    /// Play 모드 중지
    pub fn stop_play(&mut self) {
        self.viewport.mode = ViewportMode::Edit;
        log::info!("[Viewport] Returned to Edit mode");
    }

    /// Play 해상도 설정
    pub fn set_play_resolution(&mut self, resolution: PlayResolution) {
        self.viewport.play_resolution = resolution;
        log::info!("[Viewport] Play resolution set to: {}", resolution.as_str());
    }

    /// 현재 Play 중인지 확인
    pub fn is_playing(&self) -> bool {
        self.viewport.is_playing()
    }

    /// 메인 DockSpace 초기 레이아웃 설정 (한 번만 실행)
    /// ════════════════════════════════════════════════════════════════════════════
    /// ⚠️  CRITICAL: DO NOT FLATTEN THIS STRUCTURE! ⚠️
    /// ════════════════════════════════════════════════════════════════════════════
    ///
    /// 이 함수는 UE5 스타일 중첩 도킹 구조를 설정합니다.
    /// "플랫 구조가 더 간단해 보인다"고 생각할 수 있지만, 그렇게 하면:
    /// - "Map: Untitled" 탭 안에 Hierarchy/Inspector가 들어가지 않음
    /// - 마트료시카(중첩 탭) 문제가 다시 발생함
    /// - UE5 스타일 레이아웃이 완전히 깨짐
    ///
    /// ========================================
    /// [UE5 Layout] 중첩 도킹 구조 (Nested Docking Architecture)
    /// ========================================
    ///
    /// ```text
    /// Main DockSpace (Global)
    /// ├── [Center] "Map: Untitled" (Document Tab)
    /// │   └── Inner DockSpace (LevelEditorDS)
    /// │       ├── [Left] Hierarchy (World Outliner)
    /// │       ├── [Center] ##Viewport (NO_TAB_BAR! 툴바+3D 일체형)
    /// │       └── [Right] Inspector (Details)
    /// └── [Bottom] Content Browser, Output Log, AI Assistant
    /// ```
    ///
    /// 핵심 포인트:
    /// - "Map: Untitled"은 외부 문서 탭 (항상 표시)
    /// - Hierarchy/Inspector는 내부 DockSpace 안에 존재
    /// - Viewport 중앙 노드에 NO_TAB_BAR 적용 → 탭 헤더 숨김
    fn setup_initial_layout(&mut self, ui: &Ui, dockspace_id: Id) {
        if self.layout_initialized {
            return;
        }

        // 기존 노드가 있으면 레이아웃 설정 건너뛰기 (imgui.ini에서 복원됨)
        if DockBuilder::node_exists(ui, dockspace_id) {
            self.layout_initialized = true;
            log::info!("[DockLayout] Restored from imgui.ini");

            // [UE5 Layout] 복원된 경우에도 Map 탭 고정 플래그 적용
            // (imgui.ini에는 이 플래그가 저장되지 않을 수 있음)
            self.ensure_map_tab_locked(dockspace_id);
            return;
        }

        log::info!("[DockLayout] Setting up Nested Docking layout (UE5 Style)...");

        // ========================================
        // [UE5 Layout] Main DockSpace 설정
        // ========================================
        // - PASSTHRU_CENTRAL_NODE: 중앙 노드가 투명하게 통과
        // - 주의: AUTO_HIDE_TAB_BAR 사용 금지! 문서 탭은 항상 보여야 함
        DockBuilder::add_node(
            dockspace_id,
            DockNodeFlags::PASSTHRU_CENTRAL_NODE,
        );

        let display_size = ui.io().display_size();
        DockBuilder::set_node_size(dockspace_id, display_size);

        // ========================================
        // Main DockSpace 레이아웃:
        // ┌─────────────────────────────────────────┐
        // │          Map: Untitled (중앙)           │
        // │   (Document Tab - 내부에 Inner DS)      │
        // ├─────────────────────────────────────────┤
        // │         Content Browser (하단 25%)      │
        // └─────────────────────────────────────────┘
        // ========================================

        // 하단 25% 분리 (Content Browser 영역)
        let (bottom_panel, center) = DockBuilder::split_node(
            dockspace_id,
            SplitDirection::Down,
            0.25,
        );

        // [UE5 Layout] "Map: Untitled" = 문서 탭 (외부 컨테이너)
        // 이 윈도우 안에 Inner DockSpace가 생성됨
        DockBuilder::dock_window("Map: Untitled", center);

        // 하단: 공유 패널들 (Content Browser 등)
        DockBuilder::dock_window("Content Browser", bottom_panel);
        DockBuilder::dock_window("Output Log", bottom_panel);
        DockBuilder::dock_window("AI Assistant", bottom_panel);

        DockBuilder::finish(dockspace_id);

        // [UE5 Layout] Map 탭이 드래그로 떨어져 나가지 않도록 고정
        self.apply_no_undocking_to_map_node(center);

        // ========================================
        // [UE5 Layout] Inner DockSpace 설정 (LevelEditorDS)
        // ========================================
        // "Map: Untitled" 안에 들어갈 내부 레이아웃
        let inner_ds_id = dock_ids::level_editor_ds();

        DockBuilder::add_node(
            inner_ds_id,
            DockNodeFlags::PASSTHRU_CENTRAL_NODE
                | DockNodeFlags::NO_DOCKING_OVER_CENTRAL_NODE,
        );

        DockBuilder::set_node_size(inner_ds_id, display_size);

        // Inner DockSpace 레이아웃:
        // ┌──────────┬────────────────────┬──────────┐
        // │ Hierarchy│     Viewport       │Inspector │
        // │  (20%)   │   (NO_TAB_BAR!)    │  (25%)   │
        // └──────────┴────────────────────┴──────────┘

        // 좌측 20% (Hierarchy)
        let (left_panel, center_right) = DockBuilder::split_node(
            inner_ds_id,
            SplitDirection::Left,
            0.20,
        );

        // 우측 25% (Inspector)
        let (right_panel, inner_center) = DockBuilder::split_node(
            center_right,
            SplitDirection::Right,
            0.25,
        );

        // 중앙 노드 ID 저장 (NO_TAB_BAR 적용용)
        self.center_node_id = Some(inner_center);

        // 윈도우 도킹
        DockBuilder::dock_window("Hierarchy", left_panel);
        DockBuilder::dock_window("##Viewport", inner_center);  // ## prefix = 타이틀 숨김
        DockBuilder::dock_window("Inspector", right_panel);

        DockBuilder::finish(inner_ds_id);

        // ========================================
        // [UE5 Layout] 중앙 노드에 NO_TAB_BAR 적용
        // ========================================
        // Viewport 탭 헤더를 숨겨서 툴바+3D가 일체형으로 보이게 함
        self.apply_no_tab_bar_to_center(inner_center);

        self.layout_initialized = true;
        self.level_editor_layout_initialized = true;
        log::info!("[DockLayout] Nested layout setup complete (UE5 Style)");
    }

    /// ════════════════════════════════════════════════════════════════════════════
    /// ⚠️  CRITICAL: This function is essential for UE5-style layout! ⚠️
    /// ════════════════════════════════════════════════════════════════════════════
    ///
    /// [UE5 Layout] 중앙 노드에 NO_TAB_BAR 플래그 적용
    ///
    /// ImGui private flag (4096)를 사용하여 탭 바를 완전히 숨김
    /// 이렇게 하면 Viewport 탭 헤더가 사라지고 툴바+3D가 일체형으로 보임
    ///
    /// 이 함수를 제거하면 "##Viewport" 탭이 보이면서 마트료시카 문제 재발!
    fn apply_no_tab_bar_to_center(&self, node_id: Id) {
        unsafe {
            let node_ptr = imgui_sys::igDockBuilderGetNode(node_id.into());
            if !node_ptr.is_null() {
                // ImGuiDockNodeFlags_NoTabBar = 4096 (private flag)
                const NO_TAB_BAR: i32 = 4096;
                imgui_sys::ImGuiDockNode_SetLocalFlags(node_ptr, NO_TAB_BAR);
                log::info!("[DockLayout] Applied NO_TAB_BAR to center node");
            }
        }
    }

    /// [UE5 Layout] Map 탭 노드에 NO_UNDOCKING 플래그 적용
    ///
    /// "Map: Untitled" 탭이 드래그로 떨어져 나가지 않도록 고정
    /// 이 탭은 항상 중앙에 고정되어야 함 (UE5 레벨 에디터처럼)
    fn apply_no_undocking_to_map_node(&self, node_id: Id) {
        unsafe {
            let node_ptr = imgui_sys::igDockBuilderGetNode(node_id.into());
            if !node_ptr.is_null() {
                // ImGuiDockNodeFlags_NoUndocking = 16 (private flag: 1 << 4)
                // 이미 설정된 플래그와 OR 연산
                let current_flags = (*node_ptr).LocalFlags;
                const NO_UNDOCKING: i32 = 16;
                imgui_sys::ImGuiDockNode_SetLocalFlags(node_ptr, current_flags | NO_UNDOCKING);
                log::info!("[DockLayout] Applied NO_UNDOCKING to Map tab node");
            }
        }
    }

    /// [UE5 Layout] Map 탭이 고정되어 있는지 확인하고 필요시 플래그 적용
    ///
    /// imgui.ini에서 레이아웃이 복원된 경우에도 Map 탭을 고정하기 위해 사용
    fn ensure_map_tab_locked(&self, _dockspace_id: Id) {
        self.apply_no_undocking_to_window("Map: Untitled");
    }

    /// [UE5 Layout] 윈도우가 도킹된 노드에 NO_UNDOCKING 플래그 적용
    ///
    /// 해당 윈도우를 찾아서 도킹 노드에 NO_UNDOCKING 적용
    fn apply_no_undocking_to_window(&self, window_name: &str) {
        unsafe {
            // 윈도우 이름으로 윈도우 찾기
            let c_name = std::ffi::CString::new(window_name).unwrap();
            let window_ptr = imgui_sys::igFindWindowByName(c_name.as_ptr());

            if window_ptr.is_null() {
                return;  // 윈도우가 아직 생성되지 않음
            }

            // 윈도우의 도킹 노드 가져오기
            let dock_node_ptr = (*window_ptr).DockNode;
            if dock_node_ptr.is_null() {
                return;  // 도킹되지 않음
            }

            // NO_UNDOCKING 플래그 적용
            const NO_UNDOCKING: i32 = 16;  // ImGuiDockNodeFlags_NoUndocking = 1 << 4
            let current_flags = (*dock_node_ptr).LocalFlags;

            if (current_flags & NO_UNDOCKING) == 0 {
                imgui_sys::ImGuiDockNode_SetLocalFlags(dock_node_ptr, current_flags | NO_UNDOCKING);
                log::debug!("[DockLayout] Applied NO_UNDOCKING to '{}'", window_name);
            }
        }
    }

    /// 메인 렌더링 함수
    /// window_size: (width, height) in logical pixels
    /// content_offset: titlebar height offset (툴바는 뷰포트 내장)
    pub fn render(
        &mut self,
        ui: &Ui,
        _world: &World,
        window_size: (f32, f32),
        content_offset: f32,
        toolbar: &mut ImGuiToolbar,
        icons: &IconManager,
    ) -> DockAction {
        let dockspace_id = dock_ids::main_dockspace();

        // 초기 레이아웃 설정 (한 번만)
        self.setup_initial_layout(ui, dockspace_id);

        // DockSpace 영역 계산
        let dockspace_pos = [0.0, content_offset];
        let dockspace_size = [window_size.0, window_size.1 - content_offset];

        // DockSpace 스타일 설정
        let _p1 = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
        let _c1 = ui.push_style_color(StyleColor::WindowBg, [0.0, 0.0, 0.0, 0.0]);

        // DockSpace 호스트 윈도우
        let window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS
            | WindowFlags::NO_BACKGROUND
            | WindowFlags::NO_DOCKING;

        ui.window("##DockSpaceHost")
            .position(dockspace_pos, Condition::Always)
            .size(dockspace_size, Condition::Always)
            .flags(window_flags)
            .build(|| {
                // DockSpace 생성
                // - PASSTHRU_CENTRAL_NODE: 3D 배경 표시
                // - NO_DOCKING_OVER_CENTRAL_NODE: 중앙 노드(Viewport) 보호
                //
                // NO_DOCKING_SPLIT 제거: 사이드 패널 분할 허용
                // NO_UNDOCKING 제거: 패널 자유 언독 허용
                let dockspace_flags = DockNodeFlags::PASSTHRU_CENTRAL_NODE
                    | DockNodeFlags::NO_DOCKING_OVER_CENTRAL_NODE;

                ui.dock_space_with_class(
                    dockspace_id,
                    [0.0, 0.0],  // 전체 영역 사용
                    dockspace_flags,
                    None,  // window_class
                );
            });

        // ========================================
        // 중첩 도킹 구조 렌더링
        // ========================================

        // 1. Viewport (3D Scene) + Side panels
        let toolbar_action = self.render_level_editor_container(ui, toolbar, icons);

        // 2. 공유 패널 (Content Browser 등)
        self.render_shared_panels(ui);

        // 호환성을 위해 기존 필드 동기화
        self.sync_legacy_fields();

        // 툴바 액션 반환
        if toolbar_action != ToolbarAction::None {
            DockAction::Toolbar(toolbar_action)
        } else {
            DockAction::None
        }
    }

    /// ========================================
    /// [UE5 Layout] Document Tab 렌더링
    /// ════════════════════════════════════════════════════════════════════════════
    /// ⚠️  CRITICAL: NESTED CONTAINER - DO NOT FLATTEN! ⚠️
    /// ════════════════════════════════════════════════════════════════════════════
    ///
    /// [UE5 Layout] 중첩 컨테이너 렌더링
    ///
    /// 이 함수는 UE5 스타일 레이아웃의 핵심입니다.
    /// "Map: Untitled" 안에 Inner DockSpace를 생성하여
    /// Hierarchy/Viewport/Inspector가 그 안에 도킹됩니다.
    ///
    /// 구조:
    /// - "Map: Untitled" = 외부 문서 탭 (항상 표시)
    /// - 내부 DockSpace (LevelEditorDS) = Hierarchy/Viewport/Inspector 포함
    /// - ##Viewport = NO_TAB_BAR로 탭 헤더 숨김 (setup_initial_layout에서 적용)
    ///
    /// ⚠️  이 함수를 "간단하게" 만들려고 Inner DockSpace를 제거하면:
    /// - Hierarchy/Inspector가 "Map: Untitled" 밖으로 나감
    /// - UE5 스타일 레이아웃이 완전히 깨짐
    fn render_level_editor_container(&mut self, ui: &Ui, toolbar: &mut ImGuiToolbar, icons: &IconManager) -> ToolbarAction {
        let mut toolbar_action = ToolbarAction::None;

        // [UE5 Style] 문서 탭 - 패딩 없이 내부가 꽉 차게
        let _pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));

        // [UE5 Layout] Map 탭을 드래그로 떨어뜨리지 못하도록 NO_UNDOCKING 적용
        self.apply_no_undocking_to_window("Map: Untitled");

        // ========================================
        // [UE5 Layout] "Map: Untitled" = 외부 문서 탭 (Container)
        // ========================================
        // 언리얼에서 맵을 열면 탭에 "Untitled", "MyLevel" 등 맵 이름이 표시됨
        // 이 탭 안에 Inner DockSpace가 생성되어 Hierarchy/Viewport/Inspector 포함
        //
        // ★★★ LOCK DOWN FLAGS + NO_TITLE_BAR ★★★
        // - NO_TITLE_BAR: GlobalHeader에서 커스텀 탭을 그리므로 네이티브 타이틀바 숨김
        // - NO_MOVE: 윈도우 드래그 금지 (탭 헤더 잡고 끄는 것 방지)
        // - NO_COLLAPSE: 더블클릭 접기 금지
        // - NO_BRING_TO_FRONT_ON_FOCUS: 배경 컨테이너로 유지
        // - NO_SCROLLBAR: 스크롤바 숨김
        let map_window_flags = WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_SCROLLBAR
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS;

        ui.window("Map: Untitled")
            .flags(map_window_flags)
            .build(|| {
                // 포커스 감지 → LevelEditor 모드 전환
                if ui.is_window_focused() || ui.is_window_hovered() {
                    if self.current_mode != EditorMode::LevelEditor {
                        log::info!("[EditorMode] Switched to LevelEditor");
                        self.current_mode = EditorMode::LevelEditor;
                    }
                }

                // ========================================
                // [UE5 Layout] Inner DockSpace 생성
                // ========================================
                // "Map: Untitled" 안에 Hierarchy/Viewport/Inspector가 도킹됨
                let inner_ds_id = dock_ids::level_editor_ds();

                // [UE5 Style] Inner DockSpace 플래그:
                // - PASSTHRU_CENTRAL_NODE: 중앙 노드가 투명하게 통과
                // - NO_DOCKING_OVER_CENTRAL_NODE: 뷰포트 위에 도킹 방지
                let inner_flags = DockNodeFlags::PASSTHRU_CENTRAL_NODE
                    | DockNodeFlags::NO_DOCKING_OVER_CENTRAL_NODE;

                ui.dock_space_with_class(
                    inner_ds_id,
                    [0.0, 0.0],  // 전체 영역 사용
                    inner_flags,
                    None,
                );
            });

        // ========================================
        // [UE5 Layout] 내부 패널 렌더링
        // ========================================
        // Hierarchy, Viewport, Inspector가 Inner DockSpace에 도킹
        self.render_inner_panels(ui, toolbar, icons, &mut toolbar_action);

        toolbar_action
    }

    /// [UE5 Layout] Inner DockSpace 내부 패널 렌더링
    ///
    /// - Hierarchy (좌측): World Outliner
    /// - ##Viewport (중앙): 툴바 + 3D Scene (NO_TAB_BAR로 탭 숨김)
    /// - Inspector (우측): Details Panel
    fn render_inner_panels(&mut self, ui: &Ui, toolbar: &mut ImGuiToolbar, icons: &IconManager, toolbar_action: &mut ToolbarAction) {
        // ========================================
        // [UE5 Layout] Hierarchy (World Outliner)
        // ========================================
        if self.panel_visibility.hierarchy {
            let mut open = true;
            ui.window("Hierarchy")
                .opened(&mut open)
                .build(|| {
                    ui.text_colored([0.6, 0.8, 1.0, 1.0], "World Outliner");
                    ui.separator();
                    ui.text("Scene Entities:");
                });
            self.panel_visibility.hierarchy = open;
        }

        // ========================================
        // [UE5 Layout] Viewport (3D Scene)
        // ========================================
        // ## prefix로 타이틀 숨김 + NO_TAB_BAR로 탭 헤더 숨김
        // → 툴바와 3D 화면이 일체형으로 보임
        {
            let _pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));

            // [UE5 Style] ## prefix = ImGui에서 타이틀 숨김
            // NO_TITLE_BAR + NO_SCROLLBAR로 깔끔하게
            ui.window("##Viewport")
                .flags(WindowFlags::NO_TITLE_BAR | WindowFlags::NO_SCROLLBAR)
                .build(|| {
                    *toolbar_action = self.render_viewport_background(ui, toolbar, icons);
                });
        }

        // ========================================
        // [UE5 Layout] Inspector (Details Panel)
        // ========================================
        if self.panel_visibility.inspector {
            let mut open = true;
            ui.window("Inspector")
                .opened(&mut open)
                .build(|| {
                    ui.text_colored([0.6, 0.8, 1.0, 1.0], "Details");
                    ui.separator();
                    ui.text("Select an entity to edit");
                });
            self.panel_visibility.inspector = open;
        }
    }

    /// 뷰포트를 배경으로 직접 렌더링 (윈도우 아님!)
    ///
    /// 컨테이너의 전체 영역에 뷰포트를 그림
    /// DockSpace 오버레이 아래에 위치하여 패널이 닫히면 자동으로 보임
    fn render_viewport_background(&mut self, ui: &Ui, toolbar: &mut ImGuiToolbar, icons: &IconManager) -> ToolbarAction {
        let content_pos = ui.cursor_screen_pos();
        let size = ui.content_region_avail();

        // 최소 크기 체크 (어설션 방지)
        if size[0] < 1.0 || size[1] < 1.0 {
            return ToolbarAction::None;
        }

        // 뷰포트 상태 업데이트
        let is_focused = ui.is_window_focused();
        let is_hovered = ui.is_window_hovered();
        let new_size = (size[0] as u32, size[1] as u32);

        self.viewport.pos = (content_pos[0], content_pos[1]);
        self.viewport.size = new_size;
        self.viewport.focused = is_focused;
        self.viewport.hovered = is_hovered;

        // ★ 툴바 (뷰포트 상단에 직접 렌더링)
        let toolbar_action = toolbar.render_embedded(ui, icons, self.current_mode);

        // ★ 뷰포트 텍스처 표시 (남은 영역)
        let remaining_size = ui.content_region_avail();

        // 남은 영역도 최소 크기 체크
        if remaining_size[0] >= 1.0 && remaining_size[1] >= 1.0 {
            if let Some(tex_id) = self.viewport.texture_id {
                ui.image(TextureId::from(tex_id), [remaining_size[0], remaining_size[1]]);
            } else {
                let msg = if self.viewport.is_playing() {
                    "Game loading..."
                } else {
                    "Viewport loading..."
                };
                ui.text_colored([0.5, 0.5, 0.5, 1.0], msg);
            }

            // Camera PIP
            if self.show_camera_pip {
                if let Some(pip_tex_id) = self.camera_pip_texture_id {
                    self.render_camera_pip(ui, pip_tex_id, remaining_size);
                }
            }
        }

        toolbar_action
    }

    // [UE5 Layout] render_side_panels_flat 제거됨
    // Hierarchy, Inspector는 이제 render_inner_panels()에서 렌더링됨

    /// 사이드 패널 렌더링 (Hierarchy, Inspector) - WindowClass 버전
    ///
    /// 뷰포트는 이제 윈도우가 아니므로 여기서 제외
    #[allow(dead_code)]
    fn render_side_panels(&mut self, ui: &Ui, level_editor_class: &WindowClass) {
        // ========================================
        // Hierarchy (좌측)
        // ========================================
        if self.panel_visibility.hierarchy {
            let mut open = true;

            // VIP 출입증 발급
            ui.set_next_window_class(level_editor_class);

            ui.window("Hierarchy")
                .opened(&mut open)
                .build(|| {
                    ui.text_colored([0.6, 0.8, 1.0, 1.0], "World Outliner");
                    ui.separator();
                    ui.text("Scene Root");
                    ui.text("  └ MainCamera");
                    ui.text("  └ DirectionalLight");
                    ui.text("  └ StaticMesh_Floor");
                });
            self.panel_visibility.hierarchy = open;
        }

        // ========================================
        // Inspector (우측)
        // ========================================
        if self.panel_visibility.inspector {
            let mut open = true;

            // VIP 출입증 발급
            ui.set_next_window_class(level_editor_class);

            ui.window("Inspector")
                .opened(&mut open)
                .build(|| {
                    ui.text_colored([0.6, 0.8, 1.0, 1.0], "Details");
                    ui.separator();
                    ui.text("Select an object to view details");
                });
            self.panel_visibility.inspector = open;
        }
    }

    /// Document Tabs 렌더링 (Viewport + 내장 툴바)
    ///
    /// ## 뷰포트 시스템:
    /// 뷰포트 내부 콘텐츠 렌더링
    fn render_viewport_content(&mut self, ui: &Ui) {
        let content_pos = ui.cursor_screen_pos();
        let size = ui.content_region_avail();

        // 뷰포트 상태 업데이트
        let is_focused = ui.is_window_focused();
        let is_hovered = ui.is_window_hovered();
        let new_size = (size[0].max(1.0) as u32, size[1].max(1.0) as u32);

        self.viewport.pos = (content_pos[0], content_pos[1]);
        self.viewport.size = new_size;
        self.viewport.focused = is_focused;
        self.viewport.hovered = is_hovered;

        // 뷰포트 텍스처 표시
        if let Some(tex_id) = self.viewport.texture_id {
            ui.image(TextureId::from(tex_id), [size[0], size[1]]);
        } else {
            let msg = if self.viewport.is_playing() {
                "Game loading..."
            } else {
                "Viewport loading..."
            };
            ui.text_colored([0.5, 0.5, 0.5, 1.0], msg);
        }

        // Camera PIP (Picture-in-Picture) - 카메라 선택 시 오른쪽 하단에 미리보기
        if self.show_camera_pip {
            if let Some(pip_tex_id) = self.camera_pip_texture_id {
                self.render_camera_pip(ui, pip_tex_id, size);
            }
        }
    }

    /// Camera PIP (Picture-in-Picture) 렌더링
    /// 뷰포트 오른쪽 하단에 작은 미리보기 표시
    fn render_camera_pip(&self, ui: &Ui, texture_id: u64, viewport_size: [f32; 2]) {
        let pip_width = 200.0;
        let pip_height = 112.0;  // 16:9 비율
        let margin = 10.0;

        // 뷰포트 콘텐츠 영역 내 오른쪽 하단에 배치
        let pip_pos = [
            viewport_size[0] - pip_width - margin,
            viewport_size[1] - pip_height - margin,
        ];

        // PIP 배경 (약간 어두운 반투명)
        let cursor_pos = ui.cursor_screen_pos();
        let pip_screen_pos = [cursor_pos[0] + pip_pos[0], cursor_pos[1] + pip_pos[1]];

        // draw_list로 테두리 그리기
        {
            let draw_list = ui.get_window_draw_list();
            // 배경
            draw_list.add_rect(
                pip_screen_pos,
                [pip_screen_pos[0] + pip_width, pip_screen_pos[1] + pip_height],
                [0.0, 0.0, 0.0, 0.7],
            ).filled(true).build();
            // 테두리
            draw_list.add_rect(
                pip_screen_pos,
                [pip_screen_pos[0] + pip_width, pip_screen_pos[1] + pip_height],
                [0.3, 0.6, 1.0, 1.0],  // 파란색 테두리
            ).build();
        }

        // PIP 내용 (카메라 뷰)
        ui.set_cursor_pos(pip_pos);
        ui.image(TextureId::from(texture_id), [pip_width, pip_height]);

        // PIP 라벨
        ui.set_cursor_pos([pip_pos[0], pip_pos[1] - 16.0]);
        ui.text_colored([0.3, 0.6, 1.0, 1.0], "Camera Preview");
    }

    /// 공유 패널 렌더링 (모든 에디터 모드에서 공통으로 사용)
    ///
    /// 메인 DockSpace의 하단에 도킹되는 패널들:
    /// - Content Browser
    /// - Output Log
    /// - AI Assistant
    fn render_shared_panels(&mut self, ui: &Ui) {
        // Content Browser (모든 모드에서 사용)
        if self.panel_visibility.content_browser {
            let mut open = true;
            ui.window("Content Browser")
                .opened(&mut open)
                .build(|| {
                    let mode_name = self.current_mode.display_name();
                    ui.text_colored([0.8, 0.8, 0.8, 1.0], &format!("Content Browser ({})", mode_name));
                    ui.separator();
                    ui.text("Assets/");
                    ui.text("  ├── Materials/");
                    ui.text("  ├── Meshes/");
                    ui.text("  └── Textures/");
                });
            self.panel_visibility.content_browser = open;
        }

        // Output Log (모든 모드에서 사용)
        if self.panel_visibility.output_log {
            let mut open = true;
            ui.window("Output Log")
                .opened(&mut open)
                .build(|| {
                    ui.text("Output Log");
                    ui.separator();
                    ui.text_colored([0.7, 0.7, 0.7, 1.0], "[INFO] Editor initialized");
                    ui.text_colored([0.7, 0.7, 0.7, 1.0], "[INFO] Level loaded");
                });
            self.panel_visibility.output_log = open;
        }

        // AI Assistant (모든 모드에서 사용)
        if self.panel_visibility.ai_assistant {
            let mut open = true;
            ui.window("AI Assistant")
                .opened(&mut open)
                .build(|| {
                    ui.text("AI Assistant");
                    ui.separator();
                    ui.text("Ask me anything about your project...");
                });
            self.panel_visibility.ai_assistant = open;
        }
    }

    /// 기존 호환성 필드 동기화
    fn sync_legacy_fields(&mut self) {
        // 단일 뷰포트 기준으로 기존 필드 동기화
        self.viewport_size = self.viewport.size;
        self.viewport_pos = self.viewport.pos;
        self.viewport_focused = self.viewport.focused;
        self.viewport_hovered = self.viewport.hovered;
        self.scene_viewport_texture_id = self.viewport.texture_id;
    }

    /// 뷰포트 크기 반환 (호환성)
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
    }

    /// 패널 토글
    pub fn toggle_panel(&mut self, panel: &str) {
        match panel {
            "hierarchy" => self.panel_visibility.hierarchy = !self.panel_visibility.hierarchy,
            "inspector" => self.panel_visibility.inspector = !self.panel_visibility.inspector,
            "content_browser" => self.panel_visibility.content_browser = !self.panel_visibility.content_browser,
            "output_log" => self.panel_visibility.output_log = !self.panel_visibility.output_log,
            "ai_assistant" => self.panel_visibility.ai_assistant = !self.panel_visibility.ai_assistant,
            _ => {}
        }
    }

    /// 레이아웃 리셋 (기본 레이아웃으로 복원)
    pub fn reset_layout(&mut self) {
        self.layout_initialized = false;
        self.level_editor_layout_initialized = false;
        self.blueprint_editor_layout_initialized = false;
        log::info!("[DockLayout] Layout reset requested - will reinitialize on next frame");
    }
}

impl Default for ImGuiDockLayout {
    fn default() -> Self {
        Self::new()
    }
}
