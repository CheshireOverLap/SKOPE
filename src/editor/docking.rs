//! Free Docking System using egui_dock
//!
//! 자유로운 패널 드래그 앤 드롭을 지원하는 도킹 시스템

use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer, SurfaceIndex};
use egui_dock::tab_viewer::OnCloseResponse;
// egui_dock의 egui 재사용
use egui_dock::egui::{self, Context, Ui, Color32, TextureId, Rect, Sense};
use super::i18n::Translations;

/// 에디터 탭 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    /// 게임 뷰포트 (씬 렌더링)
    Viewport,
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
}

impl Tab {
    /// 탭 이름
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Viewport => "Viewport",
            Tab::Hierarchy => "Hierarchy",
            Tab::Inspector => "Inspector",
            Tab::Assets => "Assets",
            Tab::Console => "Console",
            Tab::AiChat => "AI Chat",
            Tab::AiMemory => "AI Memory",
            Tab::AiTodos => "AI Todos",
        }
    }

    /// 탭 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            Tab::Viewport => "🎮",
            Tab::Hierarchy => "🗂",
            Tab::Inspector => "🔧",
            Tab::Assets => "📁",
            Tab::Console => "📋",
            Tab::AiChat => "◈",
            Tab::AiMemory => "💾",
            Tab::AiTodos => "✓",
        }
    }

    /// 모든 탭 목록
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Viewport,
            Tab::Hierarchy,
            Tab::Inspector,
            Tab::Assets,
            Tab::Console,
            Tab::AiChat,
            Tab::AiMemory,
            Tab::AiTodos,
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
    Translate,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub fn icon(&self) -> &'static str {
        match self {
            GizmoMode::Select => "◇",
            GizmoMode::Translate => "✥",
            GizmoMode::Rotate => "↻",
            GizmoMode::Scale => "⬡",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GizmoMode::Select => "Select",
            GizmoMode::Translate => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        }
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
    /// 에디터 모드 (Edit/Play)
    pub is_playing: bool,
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
}

impl FreeDockLayout {
    /// 새 도킹 레이아웃 생성 (기본 레이아웃)
    pub fn new() -> Self {
        // 기본 레이아웃 구성:
        // +-------------------+-------------------+
        // |     Hierarchy     |     Viewport      |     Inspector    |
        // +-------------------+-------------------+-------------------+
        // |            Console / Assets           |     AI Chat      |
        // +---------------------------------------+-------------------+

        let mut dock_state = DockState::new(vec![Tab::Viewport]);

        // 메인 서피스의 루트 노드 인덱스
        let _surface = SurfaceIndex::main();

        // 왼쪽에 Hierarchy 추가 (20%)
        let [_hierarchy, _center] = dock_state.main_surface_mut()
            .split_left(NodeIndex::root(), 0.18, vec![Tab::Hierarchy]);

        // 오른쪽에 Inspector 추가 (22%)
        let [_center2, _inspector] = dock_state.main_surface_mut()
            .split_right(NodeIndex::root(), 0.78, vec![Tab::Inspector]);

        // 하단에 Assets/Console 추가 (25%) - Assets가 기본 선택
        let [_top, _bottom] = dock_state.main_surface_mut()
            .split_below(NodeIndex::root(), 0.75, vec![Tab::Assets, Tab::Console]);

        // AI Chat을 Inspector 아래에 추가
        // dock_state.main_surface_mut()
        //     .split_below(_inspector, 0.6, vec![Tab::AiChat]);

        Self {
            dock_state,
            viewport: ViewportState::default(),
            is_playing: false,
            translations: Translations::new(),
            viewport_rect: None,
            dropped_asset: None,
            drag_hover_viewport: false,
            camera_view_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            logo_texture: None,
        }
    }

    /// SKOPE 로고 텍스처 로드 (처음 한 번만 호출)
    fn load_logo_texture(&mut self, ctx: &Context) {
        if self.logo_texture.is_some() {
            return;
        }

        let logo_path = std::path::Path::new("assets/icons/skope_logo.png");
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

        // 탭 바 색상
        style.tab_bar.bg_fill = Color32::from_rgb(35, 38, 45);
        style.tab_bar.height = 26.0;

        // 탭 색상
        style.tab.tab_body.bg_fill = Color32::from_rgb(30, 30, 34);
        style.tab.active.bg_fill = Color32::from_rgb(45, 48, 58);
        style.tab.inactive.bg_fill = Color32::from_rgb(35, 38, 45);
        style.tab.hovered.bg_fill = Color32::from_rgb(50, 55, 65);
        style.tab.focused.bg_fill = Color32::from_rgb(55, 65, 85);

        // 분할선
        style.separator.width = 2.0;
        style.separator.color_idle = Color32::from_rgb(45, 48, 55);
        style.separator.color_hovered = Color32::from_rgb(80, 130, 200);
        style.separator.color_dragged = Color32::from_rgb(100, 160, 240);

        // 버튼 색상
        style.buttons.close_tab_bg_fill = Color32::TRANSPARENT;
        style.buttons.close_tab_color = Color32::from_rgb(150, 150, 160);
        style.buttons.close_tab_active_color = Color32::from_rgb(220, 100, 100);

        style
    }

    /// 상단 툴바 렌더링
    fn toolbar_ui(&mut self, ctx: &Context) {
        // 로고 텍스처 로드 (처음 한 번만)
        self.load_logo_texture(ctx);

        egui::TopBottomPanel::top("toolbar")
            .exact_height(36.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);

                    // 로고 이미지 + 텍스트
                    if let Some(logo) = &self.logo_texture {
                        ui.image((logo.id(), egui::vec2(22.0, 22.0)));
                        ui.add_space(4.0);
                    }
                    ui.label(egui::RichText::new("SKOPE")
                        .size(14.0)
                        .strong()
                        .color(Color32::from_rgb(100, 170, 240)));

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Play/Stop 버튼
                    let (play_icon, play_color) = if self.is_playing {
                        ("⏹", Color32::from_rgb(220, 100, 100))
                    } else {
                        ("▶", Color32::from_rgb(100, 200, 120))
                    };

                    if ui.add(egui::Button::new(
                        egui::RichText::new(play_icon).size(14.0).color(play_color)
                    ).min_size(egui::vec2(28.0, 24.0))).clicked() {
                        self.is_playing = !self.is_playing;
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // 기즈모 모드 버튼
                    let modes = [
                        (GizmoMode::Select, Color32::from_rgb(180, 180, 200)),
                        (GizmoMode::Translate, Color32::from_rgb(140, 200, 255)),
                        (GizmoMode::Rotate, Color32::from_rgb(255, 180, 140)),
                        (GizmoMode::Scale, Color32::from_rgb(180, 255, 180)),
                    ];

                    for (mode, color) in &modes {
                        let is_selected = self.viewport.gizmo_mode == *mode;
                        let bg = if is_selected {
                            Color32::from_rgb(60, 70, 90)
                        } else {
                            Color32::TRANSPARENT
                        };

                        if ui.add(egui::Button::new(
                            egui::RichText::new(mode.icon()).size(13.0).color(*color)
                        ).fill(bg).min_size(egui::vec2(26.0, 24.0)))
                        .on_hover_text(mode.display_name())
                        .clicked() {
                            self.viewport.gizmo_mode = *mode;
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // 종횡비 선택
                    ui.label(egui::RichText::new("Aspect:").size(10.0).color(Color32::from_rgb(120, 120, 135)));
                    egui::ComboBox::from_id_salt("aspect_combo")
                        .selected_text(self.viewport.aspect_ratio.display_name())
                        .width(55.0)
                        .show_ui(ui, |ui| {
                            for preset in AspectRatioPreset::presets() {
                                let is_selected = std::mem::discriminant(&self.viewport.aspect_ratio)
                                    == std::mem::discriminant(preset);
                                if ui.selectable_label(is_selected, preset.display_name()).clicked() {
                                    self.viewport.aspect_ratio = *preset;
                                }
                            }
                        });

                    // 우측 영역
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);

                        // 레이아웃 리셋 버튼
                        if ui.button("↺ Reset Layout").clicked() {
                            self.reset_layout();
                        }

                        ui.add_space(8.0);

                        // 탭 추가 메뉴
                        ui.menu_button("+ Add Tab", |ui| {
                            for tab in Tab::all() {
                                if ui.button(format!("{} {}", tab.icon(), tab.title())).clicked() {
                                    self.open_tab(*tab);
                                    ui.close();
                                }
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
            is_playing: self.is_playing,
            camera_view_matrix: self.camera_view_matrix,
        };

        let mut tab_viewer = EditorTabViewer {
            ctx: tab_ctx,
            hierarchy_fn: Some(&mut hierarchy_fn),
            inspector_fn: Some(&mut inspector_fn),
            console_fn: Some(&mut console_fn),
            assets_fn: Some(&mut assets_fn),
            ai_panel_fn: Some(&mut ai_panel_fn),
        };

        // 도킹 영역 렌더링
        DockArea::new(&mut self.dock_state)
            .style(dock_style)
            .show_close_buttons(true)
            .show_add_buttons(false)
            .draggable_tabs(true)
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

/// 탭 뷰어 (콜백 기반)
pub struct EditorTabViewer<'a> {
    pub ctx: TabContext<'a>,
    pub hierarchy_fn: Option<HierarchyFn<'a>>,
    pub inspector_fn: Option<InspectorFn<'a>>,
    pub console_fn: Option<ConsoleFn<'a>>,
    pub assets_fn: Option<AssetsFn<'a>>,
    pub ai_panel_fn: Option<AiPanelFn<'a>>,
}

impl<'a> TabViewer for EditorTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("{} {}", tab.icon(), tab.title()).into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Viewport => {
                self.render_viewport(ui);
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
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true  // 모든 탭 닫기 가능
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        OnCloseResponse::Close  // 닫기 허용
    }
}

impl<'a> EditorTabViewer<'a> {
    /// 뷰포트 렌더링
    fn render_viewport(&mut self, ui: &mut Ui) {
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
                log::info!("[Viewport] Asset dropped: {} at {:?}", *payload, pos);
            }
        }

        // 배경
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));

        // 뷰포트 텍스처 표시
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
                "Viewport",
                egui::FontId::proportional(18.0),
                Color32::from_rgb(80, 80, 90),
            );
        }

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
