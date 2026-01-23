//! ImGui Docking Layout for SKOPE Editor
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
    ///
    /// ## 플랫 도킹 구조 (Flat Docking Architecture)
    ///
    /// ```text
    /// Main DockSpace
    /// ├── Hierarchy (좌측 20%)
    /// ├── Level Editor (중앙) - 뷰포트
    /// ├── Inspector (우측 25%)
    /// └── Content Browser (하단 25%)
    /// ```
    fn setup_initial_layout(&mut self, ui: &Ui, dockspace_id: Id) {
        if self.layout_initialized {
            return;
        }

        // 기존 노드가 있으면 레이아웃 설정 건너뛰기 (imgui.ini에서 복원됨)
        if DockBuilder::node_exists(ui, dockspace_id) {
            self.layout_initialized = true;
            log::info!("[DockLayout] Restored from imgui.ini");
            return;
        }

        log::info!("[DockLayout] Setting up Flat Docking layout...");

        // 1. 루트 노드 추가
        // - PASSTHRU_CENTRAL_NODE: 3D 배경 표시
        // - NO_DOCKING_OVER_CENTRAL_NODE: 중앙 노드(Level Editor) 보호
        DockBuilder::add_node(
            dockspace_id,
            DockNodeFlags::PASSTHRU_CENTRAL_NODE
                | DockNodeFlags::NO_DOCKING_OVER_CENTRAL_NODE,
        );

        // 노드 크기 설정
        let display_size = ui.io().display_size();
        DockBuilder::set_node_size(dockspace_id, display_size);

        // 레이아웃:
        // ┌──────────┬────────────────────┬──────────┐
        // │ Hierarchy│   Level Editor     │Inspector │
        // │  (20%)   │     (뷰포트)       │  (25%)   │
        // ├──────────┴────────────────────┴──────────┤
        // │           Content Browser (25%)          │
        // └──────────────────────────────────────────┘

        let mut dock_main = dockspace_id;

        // 2. 하단 25% 자르기 (Content Browser)
        let (bottom_panel, upper_area) = DockBuilder::split_node(
            dock_main,
            SplitDirection::Down,
            0.25,
        );
        dock_main = upper_area;

        // 3. 좌측 20% (Hierarchy)
        let (left_panel, center_right) = DockBuilder::split_node(
            dock_main,
            SplitDirection::Left,
            0.20,
        );
        dock_main = center_right;

        // 4. 우측 25% (Inspector)
        let (right_panel, center) = DockBuilder::split_node(
            dock_main,
            SplitDirection::Right,
            0.25,
        );

        self.center_node_id = Some(center);

        // ========================================
        // 윈도우 도킹
        // ========================================

        // 좌측: Hierarchy
        DockBuilder::dock_window("Hierarchy", left_panel);

        // 중앙: Level Editor (뷰포트)
        DockBuilder::dock_window("Level Editor", center);

        // 우측: Inspector
        DockBuilder::dock_window("Inspector", right_panel);

        // 하단: Content Browser (탭으로 묶음)
        DockBuilder::dock_window("Content Browser", bottom_panel);
        DockBuilder::dock_window("Output Log", bottom_panel);
        DockBuilder::dock_window("AI Assistant", bottom_panel);

        // 레이아웃 완료
        DockBuilder::finish(dockspace_id);

        self.layout_initialized = true;
        log::info!("[DockLayout] Main layout setup complete (Flat)");
    }

    // 플랫 구조에서는 내부 DockSpace 불필요 - 모든 패널이 메인 DockSpace에 직접 도킹

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
                // - NO_DOCKING_OVER_CENTRAL_NODE: 중앙 노드(Level Editor) 보호
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

        // 1. Level Editor 컨테이너 탭 (내부에 Viewport, Hierarchy, Inspector 포함)
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

    /// Level Editor (뷰포트) 렌더링
    ///
    /// 플랫 구조: Level Editor는 뷰포트만 포함
    /// Hierarchy, Inspector는 별도 윈도우로 메인 DockSpace에 도킹
    fn render_level_editor_container(&mut self, ui: &Ui, toolbar: &mut ImGuiToolbar, icons: &IconManager) -> ToolbarAction {
        let mut toolbar_action = ToolbarAction::None;

        // "Level Editor" 뷰포트 윈도우
        let _pad = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));

        ui.window("Level Editor")
            .flags(WindowFlags::NO_SCROLLBAR)
            .build(|| {
                // 포커스 감지 → LevelEditor 모드 전환
                if ui.is_window_focused() || ui.is_window_hovered() {
                    if self.current_mode != EditorMode::LevelEditor {
                        log::info!("[EditorMode] Switched to LevelEditor");
                        self.current_mode = EditorMode::LevelEditor;
                    }
                }

                // 뷰포트 콘텐츠 렌더링
                toolbar_action = self.render_viewport_background(ui, toolbar, icons);
            });

        // 사이드 패널 렌더링 (Hierarchy, Inspector)
        self.render_side_panels_flat(ui);

        toolbar_action
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

    /// 사이드 패널 렌더링 (Hierarchy, Inspector) - 플랫 구조
    ///
    /// WindowClass 없이 렌더링하여 자유로운 도킹 허용
    /// - 어디든 도킹 가능
    /// - 탭으로 묶기 가능
    /// - 언독하여 별도 창으로 분리 가능
    fn render_side_panels_flat(&mut self, ui: &Ui) {
        // ========================================
        // Hierarchy (좌측) - 자유 도킹
        // ========================================
        if self.panel_visibility.hierarchy {
            let mut open = true;

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
        // Inspector (우측) - 자유 도킹
        // ========================================
        if self.panel_visibility.inspector {
            let mut open = true;

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
