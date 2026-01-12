//! Free Docking System using egui_dock
//!
//! 자유로운 패널 드래그 앤 드롭을 지원하는 도킹 시스템

use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer, SurfaceIndex, AllowedSplits};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::style::{OverlayType, TabAddAlign};
// egui_dock의 egui 재사용
use egui_dock::egui::{self, Context, Ui, Color32, TextureId, Rect, Sense};
use super::i18n::Translations;
use crate::paths;
use skope_debug_ui::DebugView;

/// 에디터 탭 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    /// Scene 뷰 (에디터 카메라, 기즈모 표시)
    Scene,
    /// Game 뷰 (게임 카메라, 실제 게임 화면)
    Game,
    /// 씬 계층 구조
    Hierarchy,
    /// 인스펙터 (선택된 오브젝트 속성)
    Inspector,
    /// 에셋 브라우저
    Assets,
    /// 콘솔 (로그)
    Console,
    /// AI 어시스턴트 - Chat
    AiChat,
    /// AI 어시스턴트 - Memory
    AiMemory,
    /// AI 어시스턴트 - Todos
    AiTodos,
    /// UI 에디터 (Game UI 편집)
    UiEditor,
    /// 애니메이션 타임라인
    Animation,
    /// 마법진 시스템 에디터
    MagicSystem,
}

impl Tab {
    /// 탭 이름
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Scene => "Scene",
            Tab::Game => "Game",
            Tab::Hierarchy => "Hierarchy",
            Tab::Inspector => "Inspector",
            Tab::Assets => "Assets",
            Tab::Console => "Console",
            Tab::AiChat => "AI Chat",
            Tab::AiMemory => "AI Memory",
            Tab::AiTodos => "AI Todos",
            Tab::UiEditor => "UI Editor",
            Tab::Animation => "Animation",
            Tab::MagicSystem => "Magic System",
        }
    }

    /// 탭 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            Tab::Scene => "🎬",
            Tab::Game => "🎮",
            Tab::Hierarchy => "🗂",
            Tab::Inspector => "🔧",
            Tab::Assets => "📁",
            Tab::Console => "📋",
            Tab::AiChat => "◈",
            Tab::AiMemory => "💾",
            Tab::AiTodos => "✓",
            Tab::UiEditor => "🎨",
            Tab::Animation => "⏱",
            Tab::MagicSystem => "✧",
        }
    }

    /// 모든 탭 목록
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Scene,
            Tab::Game,
            Tab::Hierarchy,
            Tab::Inspector,
            Tab::Assets,
            Tab::Console,
            Tab::AiChat,
            Tab::AiMemory,
            Tab::AiTodos,
            Tab::UiEditor,
            Tab::Animation,
            Tab::MagicSystem,
        ]
    }
}

/// 뷰포트 종횡비 프리셋
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioPreset {
    #[default]
    Free,
    Ratio16x9,
    Ratio16x10,
    Ratio21x9,
    Ratio4x3,
    Ratio1x1,
    Ratio9x16,
}

impl AspectRatioPreset {
    pub fn ratio(&self) -> Option<f32> {
        match self {
            AspectRatioPreset::Free => None,
            AspectRatioPreset::Ratio16x9 => Some(16.0 / 9.0),
            AspectRatioPreset::Ratio16x10 => Some(16.0 / 10.0),
            AspectRatioPreset::Ratio21x9 => Some(21.0 / 9.0),
            AspectRatioPreset::Ratio4x3 => Some(4.0 / 3.0),
            AspectRatioPreset::Ratio1x1 => Some(1.0),
            AspectRatioPreset::Ratio9x16 => Some(9.0 / 16.0),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AspectRatioPreset::Free => "Free",
            AspectRatioPreset::Ratio16x9 => "16:9",
            AspectRatioPreset::Ratio16x10 => "16:10",
            AspectRatioPreset::Ratio21x9 => "21:9",
            AspectRatioPreset::Ratio4x3 => "4:3",
            AspectRatioPreset::Ratio1x1 => "1:1",
            AspectRatioPreset::Ratio9x16 => "9:16",
        }
    }

    pub fn presets() -> &'static [AspectRatioPreset] {
        &[
            AspectRatioPreset::Free,
            AspectRatioPreset::Ratio16x9,
            AspectRatioPreset::Ratio16x10,
            AspectRatioPreset::Ratio21x9,
            AspectRatioPreset::Ratio4x3,
            AspectRatioPreset::Ratio1x1,
            AspectRatioPreset::Ratio9x16,
        ]
    }
}

/// 기즈모 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Select,
    Move,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub fn icon(&self) -> &'static str {
        match self {
            GizmoMode::Select => "◇",
            GizmoMode::Move => "✥",
            GizmoMode::Rotate => "↻",
            GizmoMode::Scale => "⬡",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GizmoMode::Select => "Select",
            GizmoMode::Move => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            GizmoMode::Select => "Q",
            GizmoMode::Move => "W",
            GizmoMode::Rotate => "E",
            GizmoMode::Scale => "R",
        }
    }

    /// gizmo::GizmoMode (SceneViewer용)로 변환 - 이제 동일한 variant 이름 사용
    pub fn to_scene_viewer_mode(&self) -> super::gizmo::GizmoMode {
        match self {
            GizmoMode::Select => super::gizmo::GizmoMode::Select,
            GizmoMode::Move => super::gizmo::GizmoMode::Move,
            GizmoMode::Rotate => super::gizmo::GizmoMode::Rotate,
            GizmoMode::Scale => super::gizmo::GizmoMode::Scale,
        }
    }

    /// gizmo::GizmoMode로부터 변환
    pub fn from_scene_viewer_mode(mode: super::gizmo::GizmoMode) -> Self {
        match mode {
            super::gizmo::GizmoMode::Select => GizmoMode::Select,
            super::gizmo::GizmoMode::Move => GizmoMode::Move,
            super::gizmo::GizmoMode::Rotate => GizmoMode::Rotate,
            super::gizmo::GizmoMode::Scale => GizmoMode::Scale,
        }
    }
}

// ============ Scene 뷰 렌더 모드 ============

/// Scene 뷰 렌더링 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneRenderMode {
    #[default]
    Shaded,
    Wireframe,
    ShadedWireframe,
    Unlit,
}

impl SceneRenderMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            SceneRenderMode::Shaded => "Shaded",
            SceneRenderMode::Wireframe => "Wireframe",
            SceneRenderMode::ShadedWireframe => "Shaded Wireframe",
            SceneRenderMode::Unlit => "Unlit",
        }
    }

    pub fn all() -> &'static [SceneRenderMode] {
        &[
            SceneRenderMode::Shaded,
            SceneRenderMode::Wireframe,
            SceneRenderMode::ShadedWireframe,
            SceneRenderMode::Unlit,
        ]
    }
}

/// Scene 뷰 옵션 (상단 툴바용)
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
}

impl Default for SceneViewOptions {
    fn default() -> Self {
        Self {
            show_grid: false, // 기본 OFF - Z-fighting 방지, 필요시 토글
            show_gizmos: true,
            render_mode: SceneRenderMode::Shaded,
            is_2d_mode: false,
            show_skybox: true,
            show_fog: true,
            show_lighting: true,
            show_audio: true,
            show_effects: true,
        }
    }
}

// ============ Game 뷰 해상도 프리셋 ============

/// Game 뷰 해상도 프리셋
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameResolutionPreset {
    #[default]
    FreeAspect,
    Res1920x1080,
    Res1280x720,
    Res800x600,
    Res1024x768,
    Res640x480,
}

impl GameResolutionPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            GameResolutionPreset::FreeAspect => "Free Aspect",
            GameResolutionPreset::Res1920x1080 => "1920x1080 (Full HD)",
            GameResolutionPreset::Res1280x720 => "1280x720 (HD)",
            GameResolutionPreset::Res800x600 => "800x600",
            GameResolutionPreset::Res1024x768 => "1024x768",
            GameResolutionPreset::Res640x480 => "640x480 (VGA)",
        }
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        match self {
            GameResolutionPreset::FreeAspect => None,
            GameResolutionPreset::Res1920x1080 => Some((1920, 1080)),
            GameResolutionPreset::Res1280x720 => Some((1280, 720)),
            GameResolutionPreset::Res800x600 => Some((800, 600)),
            GameResolutionPreset::Res1024x768 => Some((1024, 768)),
            GameResolutionPreset::Res640x480 => Some((640, 480)),
        }
    }

    pub fn all() -> &'static [GameResolutionPreset] {
        &[
            GameResolutionPreset::FreeAspect,
            GameResolutionPreset::Res1920x1080,
            GameResolutionPreset::Res1280x720,
            GameResolutionPreset::Res1024x768,
            GameResolutionPreset::Res800x600,
            GameResolutionPreset::Res640x480,
        ]
    }
}

/// Game 뷰 옵션 (상단 툴바용)
#[derive(Debug, Clone)]
pub struct GameViewOptions {
    pub display_index: usize,
    pub resolution: GameResolutionPreset,
    pub scale: f32,
    pub maximize_on_play: bool,
    pub mute_audio: bool,
    pub show_stats: bool,
    pub show_gizmos: bool,
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
        }
    }
}

// ============ 에디터 플레이 상태 ============

/// 에디터 플레이 상태 (Play/Pause/Edit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorPlayState {
    #[default]
    Edit,
    Playing,
    Paused,
}

// ============ 메뉴 액션 ============

/// File 메뉴 등에서 발생하는 액션
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// 새 씬 생성
    NewScene,
    /// 씬 열기
    OpenScene,
    /// 저장
    SaveScene,
    /// 다른 이름으로 저장
    SaveSceneAs,
    /// 에디터 종료
    Quit,
    // === GameObject 메뉴 ===
    /// 빈 오브젝트 생성
    CreateEmpty,
    /// 3D 오브젝트 생성 (메시 이름 포함)
    Create3DObject(String),
    /// 라이트 생성 (라이트 타입)
    CreateLight(String),
    /// 카메라 생성
    CreateCamera,
}

impl EditorPlayState {
    pub fn is_playing(&self) -> bool {
        matches!(self, EditorPlayState::Playing | EditorPlayState::Paused)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, EditorPlayState::Paused)
    }
}

/// 뷰포트 상태
pub struct ViewportState {
    pub texture_id: Option<TextureId>,
    pub size: (u32, u32),
    pub aspect_ratio: AspectRatioPreset,
    pub resolution_scale: f32,
    pub gizmo_mode: GizmoMode,
    pub show_overlay: bool,
    pub show_help: bool,
    pub hovered: bool,
    pub focused: bool,
    pub rect: Option<Rect>,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            texture_id: None,
            size: (1280, 720),
            aspect_ratio: AspectRatioPreset::Free,
            resolution_scale: 1.0,
            gizmo_mode: GizmoMode::Select,
            show_overlay: true,
            show_help: false,
            hovered: false,
            focused: false,
            rect: None,
        }
    }
}

/// 프리 도킹 레이아웃 시스템
pub struct FreeDockLayout {
    /// 도킹 상태 (egui_dock)
    pub dock_state: DockState<Tab>,
    /// 뷰포트 상태
    pub viewport: ViewportState,
    /// 에디터 플레이 상태 (Edit/Playing/Paused)
    pub play_state: EditorPlayState,
    /// Scene 뷰 옵션
    pub scene_options: SceneViewOptions,
    /// Game 뷰 옵션
    pub game_options: GameViewOptions,
    /// 다국어 번역
    pub translations: Translations,
    /// 뷰포트 영역 (기즈모 배치용)
    pub viewport_rect: Option<Rect>,
    /// 드롭된 에셋 (경로, 스크린 좌표)
    pub dropped_asset: Option<(String, egui::Pos2)>,
    /// 드래그 호버 상태
    pub drag_hover_viewport: bool,
    /// 카메라 view matrix (좌표축 기즈모용)
    pub camera_view_matrix: [[f32; 4]; 4],
    /// SKOPE 로고 텍스처
    logo_texture: Option<egui::TextureHandle>,
    /// Game 뷰포트 텍스처 ID (게임 카메라 렌더링)
    pub game_viewport_texture_id: Option<TextureId>,
    /// Game 뷰포트 크기
    pub game_viewport_size: (u32, u32),
    /// 게임 카메라 존재 여부 (렌더링 시 업데이트)
    pub has_game_camera: bool,
    /// 대기 중인 메뉴 액션 (main.rs에서 처리)
    pub pending_menu_action: Option<MenuAction>,
    /// 현재 열린 씬 경로
    pub current_scene_path: Option<std::path::PathBuf>,
    /// 씬 변경 여부 (저장 안 된 변경사항)
    pub scene_dirty: bool,
    /// 카메라 이동 속도 (m/s)
    pub camera_fly_speed: f32,
    /// 속도 UI 표시 여부
    pub show_speed_ui: bool,
    /// 에디터 아이콘 매니저
    pub icon_manager: super::icons::IconManager,
    /// 디버그 뷰 모드 (메뉴바에서 선택)
    pub debug_view: DebugView,
}

impl FreeDockLayout {
    /// 플레이 중인지 확인 (호환성용)
    pub fn is_playing(&self) -> bool {
        self.play_state.is_playing()
    }
}

impl FreeDockLayout {
    /// 새 도킹 레이아웃 생성 (기본 레이아웃)
    pub fn new() -> Self {
        // Unity/Unreal 스타일 레이아웃:
        // Inspector만 전체 높이
        // Assets/Console은 Hierarchy+Viewport 밑에
        //
        // +------------+---------------------------+------------+
        // | Hierarchy  |         Viewport          | Inspector  |
        // |            |                           | (전체높이) |
        // +------------+---------------------------+            |
        // |        Assets / Console                |            |
        // +----------------------------------------+------------+

        // Scene과 Game 탭을 함께 (탭으로 전환 가능)
        let mut dock_state = DockState::new(vec![Tab::Scene, Tab::Game]);

        // 1. 우측에 Inspector 추가 (전체 높이, 20%)
        let [left_area, _inspector] = dock_state.main_surface_mut()
            .split_right(NodeIndex::root(), 0.80, vec![Tab::Inspector]);

        // 2. 좌측 영역을 상하로 분할 (위: Hierarchy+Scene/Game, 아래: Assets/Console)
        let [top_area, _bottom] = dock_state.main_surface_mut()
            .split_below(left_area, 0.72, vec![Tab::Assets, Tab::Console]);

        // 3. 상단 영역을 좌우로 분할 (Hierarchy | Scene/Game)
        let [_hierarchy, _viewport] = dock_state.main_surface_mut()
            .split_left(top_area, 0.22, vec![Tab::Hierarchy]);

        Self {
            dock_state,
            viewport: ViewportState::default(),
            play_state: EditorPlayState::Edit,
            scene_options: SceneViewOptions::default(),
            game_options: GameViewOptions::default(),
            translations: Translations::new(),
            viewport_rect: None,
            dropped_asset: None,
            drag_hover_viewport: false,
            camera_view_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            logo_texture: None,
            game_viewport_texture_id: None,
            game_viewport_size: (1280, 720),
            has_game_camera: false,
            pending_menu_action: None,
            current_scene_path: None,
            scene_dirty: false,
            camera_fly_speed: 5.0,
            show_speed_ui: false,
            icon_manager: super::icons::IconManager::new(),
            debug_view: DebugView::None,
        }
    }

    /// Game 뷰포트 텍스처 설정
    pub fn set_game_viewport_texture(&mut self, texture_id: TextureId, size: (u32, u32)) {
        self.game_viewport_texture_id = Some(texture_id);
        self.game_viewport_size = size;
    }

    /// 특정 탭으로 포커스 (Scene/Game 탭 전환용)
    pub fn focus_tab(&mut self, target: Tab) {
        // 탭을 찾아서 활성화
        // find_tab은 Option<(SurfaceIndex, NodeIndex, TabIndex)> 반환
        if let Some(location) = self.dock_state.find_tab(&target) {
            self.dock_state.set_active_tab(location);
        }
    }

    /// Game 탭이 존재하는지 확인
    pub fn has_game_tab(&self) -> bool {
        self.dock_state.find_tab(&Tab::Game).is_some()
    }

    /// Game View 렌더링이 필요한지 확인 (조건부 렌더링용)
    /// - Game 탭이 있으면: 항상 렌더링 (최적화는 나중에)
    /// - Game 탭 없음: 렌더링 안 함
    pub fn should_render_game_view(&self, _frame_count: u64) -> bool {
        self.has_game_tab() && self.game_viewport_texture_id.is_some()
    }

    /// SKOPE 로고 텍스처 로드 (처음 한 번만 호출)
    fn load_logo_texture(&mut self, ctx: &Context) {
        if self.logo_texture.is_some() {
            return;
        }

        let logo_path = std::path::Path::new(paths::engine::ICONS).join("skope_logo.png");
        if !logo_path.exists() {
            return;
        }

        if let Ok(img) = image::open(logo_path) {
            // 24x24로 리사이즈 (툴바용)
            let resized = img.resize(24, 24, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8();
            let (width, height) = rgba.dimensions();

            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                rgba.as_raw(),
            );

            self.logo_texture = Some(ctx.load_texture(
                "skope_logo",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    /// 카메라 view matrix 설정
    pub fn set_camera_view_matrix(&mut self, view: [[f32; 4]; 4]) {
        self.camera_view_matrix = view;
    }

    /// 카메라 속도 정보 설정
    pub fn set_camera_speed_info(&mut self, speed: f32, show_ui: bool) {
        self.camera_fly_speed = speed;
        self.show_speed_ui = show_ui;
    }

    /// 기즈모 모드 가져오기
    pub fn gizmo_mode(&self) -> GizmoMode {
        self.viewport.gizmo_mode
    }

    /// 기즈모 모드 설정
    pub fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        self.viewport.gizmo_mode = mode;
    }

    /// 뷰포트 텍스처 ID 설정
    pub fn set_viewport_texture(&mut self, texture_id: TextureId) {
        self.viewport.texture_id = Some(texture_id);
    }

    /// 뷰포트 크기 가져오기
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport.size
    }

    /// 뷰포트 영역 가져오기
    pub fn get_viewport_rect(&self) -> Option<Rect> {
        self.viewport_rect
    }

    /// 특정 위치가 뷰포트 안에 있는지 확인
    pub fn is_pos_in_viewport(&self, x: f32, y: f32) -> bool {
        if let Some(rect) = self.viewport_rect {
            rect.contains(egui::pos2(x, y))
        } else {
            false
        }
    }

    /// 레이아웃 리셋 (기본 레이아웃으로)
    pub fn reset_layout(&mut self) {
        *self = Self::new();
    }

    /// 빈 상태 표시용 헬퍼 (콤팩트)
    pub fn empty_state_compact(ui: &mut Ui, message: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new(message)
                    .size(11.0)
                    .color(Color32::from_rgb(100, 105, 115))
            );
        });
    }

    /// 탭 열기 (없으면 추가)
    pub fn open_tab(&mut self, tab: Tab) {
        // 이미 열려있는지 확인
        let found = self.dock_state.iter_all_tabs()
            .any(|(_, t)| *t == tab);

        if !found {
            // 새 탭 추가
            self.dock_state.main_surface_mut().push_to_focused_leaf(tab);
        }
    }

    /// AI 패널 토글 (Inspector 옆에 3개 탭 모두 표시/숨김)
    pub fn open_ai_panel(&mut self) {
        // AI 탭이 이미 열려있는지 확인
        let has_ai_chat = self.dock_state.find_tab(&Tab::AiChat).is_some();
        let has_ai_memory = self.dock_state.find_tab(&Tab::AiMemory).is_some();
        let has_ai_todos = self.dock_state.find_tab(&Tab::AiTodos).is_some();

        // 하나라도 열려있으면 모두 닫기
        if has_ai_chat || has_ai_memory || has_ai_todos {
            if let Some(loc) = self.dock_state.find_tab(&Tab::AiChat) {
                self.dock_state.remove_tab(loc);
            }
            if let Some(loc) = self.dock_state.find_tab(&Tab::AiMemory) {
                self.dock_state.remove_tab(loc);
            }
            if let Some(loc) = self.dock_state.find_tab(&Tab::AiTodos) {
                self.dock_state.remove_tab(loc);
            }
            log::info!("[AI Panel] Closed AI panel");
            return;
        }

        // Inspector 탭 위치 찾기
        if let Some((_, node_idx, _)) = self.dock_state.find_tab(&Tab::Inspector) {
            // Inspector 옆에 AI 패널 추가 (오른쪽으로 분할)
            let ai_tabs = vec![Tab::AiChat, Tab::AiMemory, Tab::AiTodos];

            // Inspector 노드를 오른쪽으로 분할하여 AI 탭들 추가
            self.dock_state.main_surface_mut().split_right(node_idx, 0.5, ai_tabs);
        } else {
            // Inspector가 없으면 그냥 탭으로 추가
            self.dock_state.main_surface_mut().push_to_focused_leaf(Tab::AiChat);
            self.dock_state.main_surface_mut().push_to_focused_leaf(Tab::AiMemory);
            self.dock_state.main_surface_mut().push_to_focused_leaf(Tab::AiTodos);
        }

        log::info!("[AI Panel] Opened AI panel with 3 tabs");
    }

    /// 다크 테마 스타일 적용
    fn apply_style(&self, ctx: &Context) {
        let mut style = (*ctx.style()).clone();

        style.visuals.dark_mode = true;
        style.visuals.panel_fill = Color32::from_rgb(30, 30, 34);
        style.visuals.window_fill = Color32::from_rgb(35, 35, 40);
        style.visuals.extreme_bg_color = Color32::from_rgb(22, 22, 26);
        style.visuals.faint_bg_color = Color32::from_rgb(40, 40, 45);

        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(45, 45, 50);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 50, 56);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 60, 70);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 90, 130);

        style.visuals.selection.bg_fill = Color32::from_rgb(50, 80, 140);

        ctx.set_style(style);
    }

    /// egui_dock 스타일 생성
    fn dock_style(&self, ctx: &Context) -> Style {
        let mut style = Style::from_egui(ctx.style().as_ref());

        // ============ 탭 바 ============
        style.tab_bar.bg_fill = Color32::from_rgb(35, 38, 45);
        style.tab_bar.height = 28.0;

        // ============ 탭 ============
        style.tab.tab_body.bg_fill = Color32::from_rgb(30, 30, 34);
        style.tab.active.bg_fill = Color32::from_rgb(50, 55, 68);
        style.tab.inactive.bg_fill = Color32::from_rgb(38, 40, 48);
        style.tab.hovered.bg_fill = Color32::from_rgb(55, 60, 75);
        style.tab.focused.bg_fill = Color32::from_rgb(60, 70, 95);

        // ============ 분할선 (더 굵고 반응적) ============
        style.separator.width = 4.0;
        style.separator.extra_interact_width = 4.0;
        style.separator.color_idle = Color32::from_rgb(40, 42, 50);
        style.separator.color_hovered = Color32::from_rgb(80, 140, 220);
        style.separator.color_dragged = Color32::from_rgb(100, 170, 255);

        // ============ 오버레이 (드래그 미리보기) ============
        style.overlay.overlay_type = OverlayType::HighlightedAreas;
        // 드롭 영역 색상 (언리얼/유니티 스타일 파란색)
        style.overlay.selection_stroke_width = 2.0;
        // 드래그 시 오버레이 색상
        style.overlay.button_color = Color32::from_rgba_unmultiplied(60, 120, 200, 180);
        style.overlay.button_border_stroke = egui::Stroke::new(1.5, Color32::from_rgb(100, 170, 255));

        // ============ 버튼 ============
        style.buttons.close_tab_bg_fill = Color32::TRANSPARENT;
        style.buttons.close_tab_color = Color32::from_rgb(130, 130, 140);
        style.buttons.close_tab_active_color = Color32::from_rgb(240, 80, 80);
        style.buttons.add_tab_align = TabAddAlign::Right;
        style.buttons.add_tab_bg_fill = Color32::TRANSPARENT;
        style.buttons.add_tab_color = Color32::from_rgb(120, 180, 120);
        style.buttons.add_tab_active_color = Color32::from_rgb(140, 220, 140);

        // ============ 전체 영역 ============
        style.dock_area_padding = Some(egui::Margin::same(2));
        style.main_surface_border_stroke = egui::Stroke::new(1.0, Color32::from_rgb(50, 52, 60));
        style.main_surface_border_rounding = egui::CornerRadius::ZERO;

        style
    }

    /// 상단 메뉴바 렌더링 (Unity 스타일 - 2단 분리)
    fn toolbar_ui(&mut self, ctx: &Context) {
        // 로고 텍스처 로드 (처음 한 번만)
        self.load_logo_texture(ctx);

        // 아이콘 로드 (처음 한 번만)
        self.icon_manager.load(ctx);

        // ============ 1단: 메뉴바 ============
        egui::TopBottomPanel::top("menubar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                // 메뉴 버튼 스타일: 배경 없이 텍스트만
                let menu_style = |text: &str| {
                    egui::RichText::new(text).size(12.0)
                };

                ui.horizontal_centered(|ui| {
                    ui.add_space(6.0);
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 20);
                    ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 30);

                    // File 메뉴
                    let mut action: Option<MenuAction> = None;
                    ui.menu_button(menu_style("File"), |ui| {
                        if ui.button("New Scene").clicked() {
                            action = Some(MenuAction::NewScene);
                            ui.close();
                        }
                        if ui.button("Open Scene...       Ctrl+O").clicked() {
                            action = Some(MenuAction::OpenScene);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Save                Ctrl+S").clicked() {
                            action = Some(MenuAction::SaveScene);
                            ui.close();
                        }
                        if ui.button("Save As...       Ctrl+Shift+S").clicked() {
                            action = Some(MenuAction::SaveSceneAs);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Build Settings...").clicked() {
                            log::info!("[Menu] Build Settings clicked");
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            action = Some(MenuAction::Quit);
                            ui.close();
                        }
                    });

                    // Edit 메뉴
                    ui.menu_button(menu_style("Edit"), |ui| {
                        if ui.button("Undo          Ctrl+Z").clicked() {
                            log::info!("[Menu] Undo clicked");
                            ui.close();
                        }
                        if ui.button("Redo          Ctrl+Y").clicked() {
                            log::info!("[Menu] Redo clicked");
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Cut           Ctrl+X").clicked() { ui.close(); }
                        if ui.button("Copy          Ctrl+C").clicked() { ui.close(); }
                        if ui.button("Paste         Ctrl+V").clicked() { ui.close(); }
                        if ui.button("Duplicate     Ctrl+D").clicked() { ui.close(); }
                        if ui.button("Delete        Del").clicked() { ui.close(); }
                        ui.separator();
                        if ui.button("Preferences...").clicked() {
                            log::info!("[Menu] Preferences clicked");
                            ui.close();
                        }
                    });

                    // Assets 메뉴
                    ui.menu_button(menu_style("Assets"), |ui| {
                        ui.menu_button("Create", |ui| {
                            // 폴더 아이콘
                            let folder_clicked = ui.horizontal(|ui| {
                                if let Some(tex) = self.icon_manager.get("folder") {
                                    ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                                }
                                ui.button("Folder").clicked()
                            }).inner;
                            if folder_clicked { ui.close(); }
                            if ui.button("Material").clicked() { ui.close(); }
                            if ui.button("Script").clicked() { ui.close(); }
                            if ui.button("Shader").clicked() { ui.close(); }
                            if ui.button("Prefab").clicked() { ui.close(); }
                        });
                        ui.separator();
                        // Import 아이콘
                        let import_clicked = ui.horizontal(|ui| {
                            if let Some(tex) = self.icon_manager.get("asset_3d") {
                                ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                            }
                            ui.button("Import New Asset...").clicked()
                        }).inner;
                        if import_clicked {
                            log::info!("[Menu] Import Asset clicked");
                            ui.close();
                        }
                        if ui.button("Refresh          Ctrl+R").clicked() {
                            log::info!("[Menu] Refresh clicked");
                            ui.close();
                        }
                    });

                    // GameObject 메뉴
                    ui.menu_button(menu_style("GameObject"), |ui| {
                        if ui.button("Create Empty").clicked() {
                            action = Some(MenuAction::CreateEmpty);
                            ui.close();
                        }
                        ui.separator();
                        // 3D Object 서브메뉴 (아이콘 포함)
                        ui.horizontal(|ui| {
                            if let Some(tex) = self.icon_manager.get("asset_3d") {
                                ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                            }
                            ui.menu_button("3D Object", |ui| {
                                if ui.button("Cube").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Cube".to_string()));
                                    ui.close();
                                }
                                if ui.button("Sphere").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Sphere".to_string()));
                                    ui.close();
                                }
                                if ui.button("Cylinder").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Cylinder".to_string()));
                                    ui.close();
                                }
                                if ui.button("Plane").clicked() {
                                    action = Some(MenuAction::Create3DObject("#Plane".to_string()));
                                    ui.close();
                                }
                            });
                        });
                        ui.menu_button("Light", |ui| {
                            if ui.button("Directional Light").clicked() {
                                action = Some(MenuAction::CreateLight("Directional".to_string()));
                                ui.close();
                            }
                            if ui.button("Point Light").clicked() {
                                action = Some(MenuAction::CreateLight("Point".to_string()));
                                ui.close();
                            }
                            if ui.button("Spot Light").clicked() {
                                action = Some(MenuAction::CreateLight("Spot".to_string()));
                                ui.close();
                            }
                        });
                        ui.menu_button("Audio", |ui| {
                            if ui.button("Audio Source").clicked() {
                                log::info!("[Menu] Audio Source - not yet implemented");
                                ui.close();
                            }
                            if ui.button("Audio Listener").clicked() {
                                log::info!("[Menu] Audio Listener - not yet implemented");
                                ui.close();
                            }
                        });
                        if ui.button("Camera").clicked() {
                            action = Some(MenuAction::CreateCamera);
                            ui.close();
                        }
                    });

                    // Window 메뉴
                    ui.menu_button(menu_style("Window"), |ui| {
                        ui.label(egui::RichText::new("Panels").size(10.0).color(Color32::GRAY));
                        ui.separator();
                        for tab in Tab::all() {
                            let clicked = ui.horizontal(|ui| {
                                // SVG 아이콘 또는 이모지 폴백
                                if let Some(tex) = self.icon_manager.get_for_tab(tab) {
                                    ui.image((tex.id(), egui::vec2(14.0, 14.0)));
                                } else {
                                    ui.label(tab.icon());
                                }
                                ui.button(tab.title()).clicked()
                            }).inner;
                            if clicked {
                                self.open_tab(*tab);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button("↺ Reset Layout").clicked() {
                            self.reset_layout();
                            ui.close();
                        }
                    });

                    // Debug 메뉴
                    ui.menu_button(menu_style("Debug"), |ui| {
                        ui.radio_value(&mut self.debug_view, DebugView::None, "None (Full Render)");
                        ui.separator();

                        ui.menu_button("G-Buffer", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::Albedo, "Albedo");
                            ui.radio_value(&mut self.debug_view, DebugView::Normal, "Normal");
                            ui.radio_value(&mut self.debug_view, DebugView::Depth, "Depth");
                            ui.radio_value(&mut self.debug_view, DebugView::Metallic, "Metallic");
                            ui.radio_value(&mut self.debug_view, DebugView::Roughness, "Roughness");
                        });

                        ui.menu_button("Lighting", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::LightingRaw, "Lighting Raw");
                            ui.radio_value(&mut self.debug_view, DebugView::LightingLog, "Lighting Log");
                            ui.radio_value(&mut self.debug_view, DebugView::LightingScaled, "Lighting Scaled");
                            ui.radio_value(&mut self.debug_view, DebugView::SimpleLambert, "Simple Lambert");
                            ui.radio_value(&mut self.debug_view, DebugView::SpecularOnly, "Specular Only");
                            ui.radio_value(&mut self.debug_view, DebugView::SpecularLog, "Specular Log");
                        });

                        ui.menu_button("V-Buffer", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::Barycentric, "Barycentric");
                            ui.radio_value(&mut self.debug_view, DebugView::TriangleId, "Triangle ID");
                            ui.radio_value(&mut self.debug_view, DebugView::VBufferCheck, "VBuffer Check");
                            ui.radio_value(&mut self.debug_view, DebugView::UvCoords, "UV Coords");
                            ui.radio_value(&mut self.debug_view, DebugView::TextureOnly, "Texture Only");
                            ui.radio_value(&mut self.debug_view, DebugView::UvChecker, "UV Checker");
                        });

                        ui.menu_button("World Space UV", |ui| {
                            ui.radio_value(&mut self.debug_view, DebugView::WorldUvDebug, "★ World UV fract (115)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldMatrixPos, "WorldMatrix Pos (116)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldMatrixScale, "WorldMatrix Scale (117)");
                            ui.radio_value(&mut self.debug_view, DebugView::LocalPosition, "Local Position (119)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldPosDiff, "★ WorldPos Diff (120)");
                            ui.radio_value(&mut self.debug_view, DebugView::WorldPosRaw, "★ WorldPos Raw (121)");
                        });

                        ui.separator();
                        ui.radio_value(&mut self.debug_view, DebugView::Wireframe, "Wireframe");
                    });

                    // Help 메뉴
                    ui.menu_button(menu_style("Help"), |ui| {
                        if ui.button("Documentation").clicked() {
                            log::info!("[Menu] Documentation clicked");
                            ui.close();
                        }
                        if ui.button("Report a Bug...").clicked() { ui.close(); }
                        ui.separator();
                        if ui.button("About SKOPE").clicked() {
                            log::info!("[Menu] About clicked");
                            ui.close();
                        }
                    });

                    // 모든 메뉴에서 발생한 액션 저장
                    if let Some(a) = action {
                        self.pending_menu_action = Some(a);
                    }
                });
            });

        // ============ 2단: 툴바 ============
        egui::TopBottomPanel::top("toolbar")
            .exact_height(32.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);

                    // 로고 이미지 + SKOPE 텍스트
                    if let Some(logo) = &self.logo_texture {
                        ui.image((logo.id(), egui::vec2(22.0, 22.0)));
                    }
                    ui.label(egui::RichText::new("SKOPE").strong().size(14.0));

                    // 중앙 정렬을 위해 좌측 공백
                    let total_width = ui.available_width();
                    let center_width = 100.0;
                    let left_space = (total_width - center_width) / 2.0;

                    ui.add_space(left_space.max(10.0));

                    // Play 버튼
                    let is_playing = self.play_state.is_playing();
                    let play_bg = if is_playing {
                        Color32::from_rgb(50, 80, 50)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let play_color = if is_playing {
                        Color32::from_rgb(100, 255, 100)
                    } else {
                        Color32::from_rgb(180, 180, 180)
                    };

                    if ui.add(egui::Button::new(
                        egui::RichText::new("▶").size(16.0).color(play_color)
                    ).fill(play_bg).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text("Play (Ctrl+P)")
                    .clicked() {
                        if is_playing {
                            // Stop → Edit 모드로
                            self.play_state = EditorPlayState::Edit;
                            // Scene 탭으로 돌아감
                            self.focus_tab(Tab::Scene);
                        } else {
                            // Play 시작
                            self.play_state = EditorPlayState::Playing;
                            // Game 탭으로 자동 전환
                            self.focus_tab(Tab::Game);
                        }
                    }

                    // Pause 버튼
                    let is_paused = self.play_state.is_paused();
                    let pause_bg = if is_paused {
                        Color32::from_rgb(80, 80, 50)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let pause_color = if is_paused {
                        Color32::from_rgb(255, 255, 100)
                    } else if is_playing {
                        Color32::from_rgb(180, 180, 180)
                    } else {
                        Color32::from_rgb(100, 100, 100)
                    };

                    let pause_enabled = is_playing || is_paused;
                    if ui.add_enabled(pause_enabled, egui::Button::new(
                        egui::RichText::new("⏸").size(16.0).color(pause_color)
                    ).fill(pause_bg).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text("Pause")
                    .clicked() {
                        self.play_state = if is_paused {
                            EditorPlayState::Playing
                        } else {
                            EditorPlayState::Paused
                        };
                    }

                    // Step 버튼
                    let step_color = if is_paused {
                        Color32::from_rgb(180, 180, 180)
                    } else {
                        Color32::from_rgb(100, 100, 100)
                    };

                    if ui.add_enabled(is_paused, egui::Button::new(
                        egui::RichText::new("⏭").size(16.0).color(step_color)
                    ).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text("Step (single frame)")
                    .clicked() {
                        log::info!("[Play] Step frame");
                    }

                    // 우측: AI 버튼 + Layout 드롭다운
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);

                        // AI 패널 버튼
                        let ai_button = egui::Button::new(
                            egui::RichText::new("🤖 AI").size(12.0)
                        ).min_size(egui::vec2(50.0, 24.0));

                        if ui.add(ai_button)
                            .on_hover_text("Open AI Panel (Chat, Memory, Todos)")
                            .clicked()
                        {
                            self.open_ai_panel();
                        }

                        ui.add_space(8.0);

                        egui::ComboBox::from_id_salt("layout_combo")
                            .selected_text("Default")
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(true, "Default").clicked() {
                                    self.reset_layout();
                                }
                                if ui.selectable_label(false, "2 by 3").clicked() {
                                    log::info!("[Layout] 2 by 3 selected");
                                }
                                if ui.selectable_label(false, "4 Split").clicked() {
                                    log::info!("[Layout] 4 Split selected");
                                }
                                if ui.selectable_label(false, "Wide").clicked() {
                                    log::info!("[Layout] Wide selected");
                                }
                            });
                    });
                });
            });
    }

    /// 메인 UI 렌더링
    pub fn show<'a>(
        &mut self,
        ctx: &Context,
        mut hierarchy_fn: impl FnMut(&mut Ui) + 'a,
        mut inspector_fn: impl FnMut(&mut Ui) + 'a,
        mut console_fn: impl FnMut(&mut Ui) + 'a,
        mut assets_fn: impl FnMut(&mut Ui) + 'a,
        mut ai_panel_fn: impl FnMut(&mut Ui, AiTabKind) + 'a,
        mut ui_editor_fn: impl FnMut(&mut Ui) + 'a,
        mut animation_fn: impl FnMut(&mut Ui) + 'a,
        mut magic_system_fn: impl FnMut(&mut Ui) + 'a,
    ) {
        // 스타일 적용
        self.apply_style(ctx);

        // 상단 툴바
        self.toolbar_ui(ctx);

        // 도킹 스타일
        let dock_style = self.dock_style(ctx);

        // 탭 뷰어 생성
        let tab_ctx = TabContext {
            viewport: &mut self.viewport,
            viewport_rect: &mut self.viewport_rect,
            dropped_asset: &mut self.dropped_asset,
            drag_hover_viewport: &mut self.drag_hover_viewport,
            is_playing: self.play_state.is_playing(),
            camera_view_matrix: self.camera_view_matrix,
            scene_options: &mut self.scene_options,
            game_options: &mut self.game_options,
            game_viewport_texture_id: self.game_viewport_texture_id,
            game_viewport_size: self.game_viewport_size,
            has_game_camera: self.has_game_camera,
            camera_fly_speed: self.camera_fly_speed,
            show_speed_ui: self.show_speed_ui,
            icon_manager: &self.icon_manager,
        };

        let mut tab_viewer = EditorTabViewer {
            ctx: tab_ctx,
            hierarchy_fn: Some(&mut hierarchy_fn),
            inspector_fn: Some(&mut inspector_fn),
            console_fn: Some(&mut console_fn),
            assets_fn: Some(&mut assets_fn),
            ai_panel_fn: Some(&mut ai_panel_fn),
            ui_editor_fn: Some(&mut ui_editor_fn),
            animation_fn: Some(&mut animation_fn),
            magic_system_fn: Some(&mut magic_system_fn),
        };

        // 도킹 영역 렌더링
        DockArea::new(&mut self.dock_state)
            .style(dock_style)
            // 탭 기능
            .show_close_buttons(true)
            .show_add_buttons(true)  // 탭 추가 버튼 표시
            .draggable_tabs(true)
            .tab_context_menus(true)  // 우클릭 메뉴
            // 패널 접기 버튼 숨김
            .show_leaf_collapse_buttons(false)
            // 분할 방향 허용
            .allowed_splits(AllowedSplits::All)
            .show(ctx, &mut tab_viewer);
    }
}

impl Default for FreeDockLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// 탭 뷰어 구현을 위한 컨텍스트
pub struct TabContext<'a> {
    pub viewport: &'a mut ViewportState,
    pub viewport_rect: &'a mut Option<Rect>,
    pub dropped_asset: &'a mut Option<(String, egui::Pos2)>,
    pub drag_hover_viewport: &'a mut bool,
    pub is_playing: bool,
    /// 카메라 view matrix (좌표축 기즈모용)
    pub camera_view_matrix: [[f32; 4]; 4],
    /// Scene 뷰 옵션
    pub scene_options: &'a mut SceneViewOptions,
    /// Game 뷰 옵션
    pub game_options: &'a mut GameViewOptions,
    /// Game 뷰포트 텍스처 ID
    pub game_viewport_texture_id: Option<TextureId>,
    /// Game 뷰포트 크기
    pub game_viewport_size: (u32, u32),
    /// 게임 카메라 존재 여부
    pub has_game_camera: bool,
    /// 카메라 이동 속도 (m/s)
    pub camera_fly_speed: f32,
    /// 속도 UI 표시 여부
    pub show_speed_ui: bool,
    /// 아이콘 매니저 (탭 아이콘용)
    pub icon_manager: &'a super::icons::IconManager,
}

/// AI 탭 종류 (통합 콜백용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTabKind {
    Chat,
    Memory,
    Todos,
}

/// 탭 콜백 타입
pub type HierarchyFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type InspectorFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type ConsoleFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type AssetsFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type AiPanelFn<'a> = &'a mut dyn FnMut(&mut Ui, AiTabKind);
pub type UiEditorFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type AnimationFn<'a> = &'a mut dyn FnMut(&mut Ui);
pub type MagicSystemFn<'a> = &'a mut dyn FnMut(&mut Ui);

/// 탭 뷰어 (콜백 기반)
pub struct EditorTabViewer<'a> {
    pub ctx: TabContext<'a>,
    pub hierarchy_fn: Option<HierarchyFn<'a>>,
    pub inspector_fn: Option<InspectorFn<'a>>,
    pub console_fn: Option<ConsoleFn<'a>>,
    pub assets_fn: Option<AssetsFn<'a>>,
    pub ai_panel_fn: Option<AiPanelFn<'a>>,
    pub ui_editor_fn: Option<UiEditorFn<'a>>,
    pub animation_fn: Option<AnimationFn<'a>>,
    pub magic_system_fn: Option<MagicSystemFn<'a>>,
}

impl<'a> TabViewer for EditorTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("{} {}", tab.icon(), tab.title()).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Scene => {
                self.render_scene_view(ui);
            }
            Tab::Game => {
                self.render_game_view(ui);
            }
            Tab::Hierarchy => {
                if let Some(ref mut f) = self.hierarchy_fn {
                    f(ui);
                } else {
                    ui.label("Hierarchy panel");
                }
            }
            Tab::Inspector => {
                if let Some(ref mut f) = self.inspector_fn {
                    f(ui);
                } else {
                    ui.label("Inspector panel");
                }
            }
            Tab::Assets => {
                if let Some(ref mut f) = self.assets_fn {
                    f(ui);
                } else {
                    ui.label("Assets panel");
                }
            }
            Tab::Console => {
                if let Some(ref mut f) = self.console_fn {
                    f(ui);
                } else {
                    ui.label("Console panel");
                }
            }
            Tab::AiChat => {
                if let Some(ref mut f) = self.ai_panel_fn {
                    f(ui, AiTabKind::Chat);
                } else {
                    ui.label("AI Chat");
                }
            }
            Tab::AiMemory => {
                if let Some(ref mut f) = self.ai_panel_fn {
                    f(ui, AiTabKind::Memory);
                } else {
                    ui.label("AI Memory");
                }
            }
            Tab::AiTodos => {
                if let Some(ref mut f) = self.ai_panel_fn {
                    f(ui, AiTabKind::Todos);
                } else {
                    ui.label("AI Todos");
                }
            }
            Tab::UiEditor => {
                if let Some(ref mut f) = self.ui_editor_fn {
                    f(ui);
                } else {
                    ui.label("UI Editor panel");
                }
            }
            Tab::Animation => {
                if let Some(ref mut f) = self.animation_fn {
                    f(ui);
                } else {
                    ui.label("Animation Timeline");
                }
            }
            Tab::MagicSystem => {
                if let Some(ref mut f) = self.magic_system_fn {
                    f(ui);
                } else {
                    ui.label("Magic System Editor");
                }
            }
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true  // 모든 탭 닫기 가능
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        OnCloseResponse::Close  // 닫기 허용
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        // 탭 버튼 호버 시 아이콘 포함 툴팁 표시
        if let Some(tex) = self.ctx.icon_manager.get_for_tab(tab) {
            response.clone().on_hover_ui(|ui| {
                ui.horizontal(|ui| {
                    ui.image((tex.id(), egui::vec2(16.0, 16.0)));
                    ui.label(tab.title());
                });
            });
        }
    }

    fn add_popup(&mut self, ui: &mut Ui, _surface: SurfaceIndex, _node: NodeIndex) {
        // + 버튼 클릭 시 팝업 메뉴 (아이콘 포함)
        ui.set_min_width(150.0);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

        for tab in Tab::all() {
            ui.horizontal(|ui| {
                // 아이콘 이미지 표시
                if let Some(tex) = self.ctx.icon_manager.get_for_tab(&tab) {
                    ui.image((tex.id(), egui::vec2(16.0, 16.0)));
                } else {
                    // 아이콘 없으면 이모지 폴백
                    ui.label(tab.icon());
                }
                if ui.button(tab.title()).clicked() {
                    // 탭은 외부에서 추가해야 함 (여기서는 신호만)
                    // egui_dock 내부에서 처리됨
                }
            });
        }
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        true  // 모든 탭을 플로팅 윈도우로 분리 가능
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        // Scene/Game 뷰는 배경 클리어 안 함 (자체 렌더링)
        !matches!(tab, Tab::Scene | Tab::Game)
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]  // 스크롤바는 각 탭에서 개별 관리
    }
}

impl<'a> EditorTabViewer<'a> {
    /// Scene 뷰 렌더링 (에디터 카메라, 기즈모, 드래그앤드롭)
    fn render_scene_view(&mut self, ui: &mut Ui) {
        // ============ 상단 툴바 ============
        ui.horizontal(|ui| {
            ui.set_height(24.0);
            ui.add_space(4.0);

            // 2D/3D 토글
            let mode_text = if self.ctx.scene_options.is_2d_mode { "2D" } else { "3D" };
            let mode_color = if self.ctx.scene_options.is_2d_mode {
                Color32::from_rgb(100, 200, 255)
            } else {
                Color32::from_rgb(180, 180, 180)
            };
            if ui.add(egui::Button::new(
                egui::RichText::new(mode_text).size(11.0).color(mode_color)
            ).min_size(egui::vec2(28.0, 18.0))).clicked() {
                self.ctx.scene_options.is_2d_mode = !self.ctx.scene_options.is_2d_mode;
            }

            ui.add_space(4.0);

            // 렌더 모드 드롭다운
            egui::ComboBox::from_id_salt("scene_render_mode")
                .selected_text(self.ctx.scene_options.render_mode.display_name())
                .width(100.0)
                .show_ui(ui, |ui| {
                    for mode in SceneRenderMode::all() {
                        let is_selected = std::mem::discriminant(&self.ctx.scene_options.render_mode)
                            == std::mem::discriminant(mode);
                        if ui.selectable_label(is_selected, mode.display_name()).clicked() {
                            self.ctx.scene_options.render_mode = *mode;
                        }
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // 토글 버튼들
            let toggle_button = |ui: &mut Ui, label: &str, enabled: &mut bool, tooltip: &str| {
                let color = if *enabled {
                    Color32::from_rgb(180, 220, 255)
                } else {
                    Color32::from_rgb(100, 100, 110)
                };
                if ui.add(egui::Button::new(
                    egui::RichText::new(label).size(9.0).color(color)
                ).min_size(egui::vec2(20.0, 18.0)))
                .on_hover_text(tooltip)
                .clicked() {
                    *enabled = !*enabled;
                }
            };

            toggle_button(ui, "☀", &mut self.ctx.scene_options.show_lighting, "Lighting");
            toggle_button(ui, "🔊", &mut self.ctx.scene_options.show_audio, "Audio");
            toggle_button(ui, "✨", &mut self.ctx.scene_options.show_effects, "Effects");
            toggle_button(ui, "☁", &mut self.ctx.scene_options.show_skybox, "Skybox");
            toggle_button(ui, "🌫", &mut self.ctx.scene_options.show_fog, "Fog");

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Grid 토글
            let grid_color = if self.ctx.scene_options.show_grid {
                Color32::from_rgb(100, 255, 150)
            } else {
                Color32::from_rgb(100, 100, 110)
            };
            if ui.add(egui::Button::new(
                egui::RichText::new("Grid").size(10.0).color(grid_color)
            ).min_size(egui::vec2(35.0, 18.0))).clicked() {
                self.ctx.scene_options.show_grid = !self.ctx.scene_options.show_grid;
            }

            // Gizmos 토글
            let gizmo_color = if self.ctx.scene_options.show_gizmos {
                Color32::from_rgb(255, 200, 100)
            } else {
                Color32::from_rgb(100, 100, 110)
            };
            if ui.add(egui::Button::new(
                egui::RichText::new("Gizmos").size(10.0).color(gizmo_color)
            ).min_size(egui::vec2(50.0, 18.0))).clicked() {
                self.ctx.scene_options.show_gizmos = !self.ctx.scene_options.show_gizmos;
            }

            // 우측 정렬용 스페이서
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // 카메라 속도 표시 (언리얼 스타일)
                let speed = self.ctx.camera_fly_speed;
                let speed_text = if speed >= 10.0 {
                    format!("{:.0}", speed)
                } else if speed >= 1.0 {
                    format!("{:.1}", speed)
                } else {
                    format!("{:.2}", speed)
                };

                ui.add(egui::Label::new(
                    egui::RichText::new("m/s").size(9.0).color(Color32::from_rgb(120, 120, 130))
                ));
                ui.add_space(2.0);
                ui.add(egui::Label::new(
                    egui::RichText::new(&speed_text).size(11.0).color(Color32::from_rgb(180, 200, 255))
                ));
                ui.add_space(4.0);
                ui.add(egui::Label::new(
                    egui::RichText::new("⚡").size(10.0).color(Color32::from_rgb(255, 200, 100))
                ));
            });
        });

        ui.separator();

        // ============ 뷰포트 영역 ============
        let available_size = ui.available_size();

        // 뷰포트 영역 계산
        let viewport_rect = ui.available_rect_before_wrap();

        // 뷰포트 크기 업데이트
        self.ctx.viewport.size = (available_size.x as u32, available_size.y as u32);
        *self.ctx.viewport_rect = Some(viewport_rect);

        // 드롭 타겟 (드래그 앤 드롭)
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        // 드래그 호버 감지
        let is_hovering = response.dnd_hover_payload::<String>().is_some();
        *self.ctx.drag_hover_viewport = is_hovering;

        // 드롭 감지
        if let Some(payload) = response.dnd_release_payload::<String>() {
            let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
            if let Some(pos) = pointer_pos {
                *self.ctx.dropped_asset = Some(((*payload).clone(), pos));
                log::info!("[Scene] Asset dropped: {} at {:?}", *payload, pos);
            }
        }

        // 배경
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));

        // Scene 텍스처 표시
        if let Some(texture_id) = self.ctx.viewport.texture_id {
            ui.painter().image(
                texture_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // 플레이스홀더
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Scene View",
                egui::FontId::proportional(18.0),
                Color32::from_rgb(80, 80, 90),
            );
        }

        // ============ 좌측 툴 팔레트 (오버레이) ============
        self.draw_tool_palette(ui, rect);

        // 드래그 호버 시 오버레이
        if *self.ctx.drag_hover_viewport {
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(80, 140, 220, 40),
            );
            ui.painter().rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(2.0, Color32::from_rgb(80, 160, 255)),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop to place object",
                egui::FontId::proportional(16.0),
                Color32::from_rgb(180, 200, 255),
            );
        }

        // 좌표축 기즈모 (블렌더 스타일) - 오른쪽 상단 코너
        self.draw_orientation_gizmo(ui, rect);

        // 호버 상태 업데이트
        self.ctx.viewport.hovered = response.hovered();
        self.ctx.viewport.rect = Some(rect);
    }

    /// 좌측 툴 팔레트 그리기 (Scene 뷰 오버레이)
    fn draw_tool_palette(&mut self, ui: &mut Ui, viewport_rect: Rect) {
        let palette_x = viewport_rect.min.x + 8.0;
        let palette_y = viewport_rect.min.y + 8.0;
        let button_size = 28.0;
        let button_spacing = 2.0;

        // 팔레트 배경
        let palette_rect = Rect::from_min_size(
            egui::pos2(palette_x - 3.0, palette_y - 3.0),
            egui::vec2(button_size + 6.0, (button_size + button_spacing) * 4.0 + 3.0),
        );
        ui.painter().rect_filled(palette_rect, 4.0, Color32::from_rgba_unmultiplied(30, 32, 38, 220));
        ui.painter().rect_stroke(
            palette_rect,
            4.0,
            egui::Stroke::new(1.0, Color32::from_rgb(50, 55, 65)),
            egui::StrokeKind::Outside,
        );

        // 툴 버튼들
        let tools = [
            (GizmoMode::Select, "Q"),
            (GizmoMode::Move, "W"),
            (GizmoMode::Rotate, "E"),
            (GizmoMode::Scale, "R"),
        ];

        let tool_colors = [
            Color32::from_rgb(180, 180, 200),  // Select
            Color32::from_rgb(140, 200, 255),  // Move
            Color32::from_rgb(255, 180, 140),  // Rotate
            Color32::from_rgb(180, 255, 180),  // Scale
        ];

        for (i, ((mode, shortcut), color)) in tools.iter().zip(tool_colors.iter()).enumerate() {
            let btn_rect = Rect::from_min_size(
                egui::pos2(palette_x, palette_y + (button_size + button_spacing) * i as f32),
                egui::vec2(button_size, button_size),
            );

            let is_selected = self.ctx.viewport.gizmo_mode == *mode;
            let bg_color = if is_selected {
                Color32::from_rgb(60, 80, 120)
            } else {
                Color32::from_rgba_unmultiplied(45, 48, 55, 200)
            };

            // 버튼 배경
            ui.painter().rect_filled(btn_rect, 3.0, bg_color);

            // 아이콘 (SVG 이미지 사용)
            if let Some(tex) = self.ctx.icon_manager.get_for_gizmo(mode) {
                let icon_size = 16.0;
                let icon_rect = Rect::from_center_size(btn_rect.center(), egui::vec2(icon_size, icon_size));
                let tint = if is_selected { *color } else { Color32::from_rgb(160, 165, 175) };
                ui.painter().image(
                    tex.id(),
                    icon_rect,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
            } else {
                // 폴백: 텍스트 아이콘
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    mode.icon(),
                    egui::FontId::proportional(14.0),
                    *color,
                );
            }

            // 클릭 감지
            let btn_response = ui.allocate_rect(btn_rect, Sense::click());
            if btn_response.clicked() {
                self.ctx.viewport.gizmo_mode = *mode;
            }

            // 호버 시 툴팁
            btn_response.clone().on_hover_text(format!("{} ({})", mode.display_name(), shortcut));

            // 호버 강조
            if btn_response.hovered() && !is_selected {
                ui.painter().rect_stroke(
                    btn_rect,
                    3.0,
                    egui::Stroke::new(1.0, Color32::from_rgb(100, 140, 200)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// Game 뷰 렌더링 (게임 카메라, 기즈모 없음)
    fn render_game_view(&mut self, ui: &mut Ui) {
        // 최소 크기 체크 - 너무 작으면 렌더링 스킵
        let total_available = ui.available_size();
        if total_available.x < 100.0 || total_available.y < 50.0 {
            return;
        }

        // ============ 상단 툴바 (고정 높이 26px) ============
        let toolbar_height = 26.0;

        ui.allocate_ui_with_layout(
            egui::vec2(total_available.x, toolbar_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(4.0);

                // Display 드롭다운
                let display_text = format!("Display {}", self.ctx.game_options.display_index);
                egui::ComboBox::from_id_salt("game_display")
                    .selected_text(&display_text)
                    .width(75.0)
                    .show_ui(ui, |ui| {
                        for i in 1..=3 {
                            let is_selected = self.ctx.game_options.display_index == i;
                            if ui.selectable_label(is_selected, format!("Display {}", i)).clicked() {
                                self.ctx.game_options.display_index = i;
                            }
                        }
                    });

                // Resolution 드롭다운
                egui::ComboBox::from_id_salt("game_resolution")
                    .selected_text(self.ctx.game_options.resolution.display_name())
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for preset in GameResolutionPreset::all() {
                            let is_selected = std::mem::discriminant(&self.ctx.game_options.resolution)
                                == std::mem::discriminant(preset);
                            if ui.selectable_label(is_selected, preset.display_name()).clicked() {
                                self.ctx.game_options.resolution = *preset;
                            }
                        }
                    });

                // 너비가 충분할 때만 추가 옵션 표시
                if total_available.x > 400.0 {
                    // Scale 슬라이더
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Scale:").size(10.0).color(Color32::from_rgb(140, 140, 150)));
                    ui.add(egui::Slider::new(&mut self.ctx.game_options.scale, 0.25..=2.0)
                        .show_value(true)
                        .suffix("x")
                        .max_decimals(2)
                    ).on_hover_text("Render scale");
                }

                if total_available.x > 550.0 {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 토글 체크박스들
                    ui.checkbox(&mut self.ctx.game_options.maximize_on_play, "")
                        .on_hover_text("Maximize on Play");
                    ui.label(egui::RichText::new("Max").size(9.0).color(Color32::from_rgb(130, 130, 140)));

                    ui.add_space(4.0);
                    ui.checkbox(&mut self.ctx.game_options.mute_audio, "")
                        .on_hover_text("Mute Audio");
                    ui.label(egui::RichText::new("Mute").size(9.0).color(Color32::from_rgb(130, 130, 140)));
                }

                // 우측 토글 (너비 충분할 때만)
                if total_available.x > 650.0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(6.0);

                        // Gizmos 토글
                        let gizmo_color = if self.ctx.game_options.show_gizmos {
                            Color32::from_rgb(100, 200, 255)
                        } else {
                            Color32::from_rgb(100, 100, 110)
                        };
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Gizmos").size(10.0).color(gizmo_color)
                        ).min_size(egui::vec2(50.0, 18.0))).clicked() {
                            self.ctx.game_options.show_gizmos = !self.ctx.game_options.show_gizmos;
                        }

                        // Stats 토글
                        let stats_color = if self.ctx.game_options.show_stats {
                            Color32::from_rgb(100, 255, 150)
                        } else {
                            Color32::from_rgb(100, 100, 110)
                        };
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Stats").size(10.0).color(stats_color)
                        ).min_size(egui::vec2(40.0, 18.0))).clicked() {
                            self.ctx.game_options.show_stats = !self.ctx.game_options.show_stats;
                        }
                    });
                }
            },
        );

        ui.separator();

        // ============ 게임 뷰 영역 ============
        let available_size = ui.available_size();

        // 최소 크기 체크
        if available_size.x < 10.0 || available_size.y < 10.0 {
            return;
        }

        // 영역 할당
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click());

        // 배경 (더 어두운 톤)
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(15, 15, 18));

        // Game 텍스처 표시 (게임 카메라가 있을 때만)
        if self.ctx.has_game_camera {
            if let Some(texture_id) = self.ctx.game_viewport_texture_id {
                // 선택된 해상도에 따른 종횡비 결정
                let target_aspect = if let Some((w, h)) = self.ctx.game_options.resolution.resolution() {
                    // 특정 해상도 선택됨 - 해당 비율 사용
                    w as f32 / h as f32
                } else {
                    // Free Aspect - 텍스처 원본 비율 사용
                    self.ctx.game_viewport_size.0 as f32 / self.ctx.game_viewport_size.1.max(1) as f32
                };

                let view_aspect = rect.width() / rect.height();

                // 선택된 비율에 맞게 표시 영역 계산 (레터박스/필러박스)
                let display_rect = if target_aspect > view_aspect {
                    // 타겟이 더 넓음 - 너비에 맞추고 위아래 검정
                    let w = rect.width();
                    let h = w / target_aspect;
                    let y_offset = (rect.height() - h) / 2.0;
                    Rect::from_min_size(
                        egui::pos2(rect.min.x, rect.min.y + y_offset),
                        egui::vec2(w, h),
                    )
                } else {
                    // 타겟이 더 높음 - 높이에 맞추고 좌우 검정
                    let h = rect.height();
                    let w = h * target_aspect;
                    let x_offset = (rect.width() - w) / 2.0;
                    Rect::from_min_size(
                        egui::pos2(rect.min.x + x_offset, rect.min.y),
                        egui::vec2(w, h),
                    )
                };

                ui.painter().image(
                    texture_id,
                    display_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );

                // 선택된 해상도 표시 (디버그용, 좌하단)
                if let Some((w, h)) = self.ctx.game_options.resolution.resolution() {
                    ui.painter().text(
                        egui::pos2(display_rect.min.x + 5.0, display_rect.max.y - 18.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{}x{}", w, h),
                        egui::FontId::monospace(10.0),
                        Color32::from_rgba_unmultiplied(200, 200, 200, 150),
                    );
                }
            }
        } else {
            // Game 카메라가 없으면 메시지 표시
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Game Camera\nAdd a Camera component to an entity",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(80, 80, 90),
            );
        }

        // Stats 오버레이
        if self.ctx.game_options.show_stats {
            let stats_rect = Rect::from_min_size(
                egui::pos2(rect.max.x - 120.0, rect.min.y + 5.0),
                egui::vec2(115.0, 60.0),
            );
            ui.painter().rect_filled(stats_rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
            ui.painter().text(
                egui::pos2(stats_rect.min.x + 5.0, stats_rect.min.y + 8.0),
                egui::Align2::LEFT_TOP,
                "FPS: 60.0\nDraw Calls: --\nTriangles: --",
                egui::FontId::monospace(10.0),
                Color32::from_rgb(200, 200, 200),
            );
        }

        // Play 모드가 아닐 때 오버레이
        if !self.ctx.is_playing {
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 100),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "▶ Press Play to start",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(150, 150, 160),
            );
        }

        // Game 뷰는 호버 상태 업데이트 안 함 (Scene 뷰만)
        let _ = response;
        self.ctx.viewport.rect = Some(rect);
    }

    /// 뷰포트 코너에 좌표축 기즈모 그리기
    fn draw_orientation_gizmo(&self, ui: &Ui, viewport_rect: Rect) {
        use super::scene_viewer::orientation_gizmo;
        orientation_gizmo::draw(
            ui,
            viewport_rect,
            self.ctx.camera_view_matrix,
            &orientation_gizmo::OrientationGizmoConfig::default(),
        );
    }
}
