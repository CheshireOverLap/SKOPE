//! Editor Dock Layout
//!
//! egui 기반 도킹 시스템 (SidePanel, TopBottomPanel 사용)
//! 언리얼/유니티 스타일 레이아웃 + AI Assistant 패널

use egui::{self, Context, TextureId, Ui, Color32, Rect, Sense, Vec2};
use super::i18n::{Language, TextKey, Translations};

/// 뷰포트 종횡비 프리셋
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioPreset {
    /// 자유 비율 (패널 크기에 맞춤)
    #[default]
    Free,
    /// 16:9 (1920x1080, 2560x1440, 3840x2160)
    Ratio16x9,
    /// 16:10 (1920x1200, 2560x1600)
    Ratio16x10,
    /// 21:9 (2560x1080, 3440x1440)
    Ratio21x9,
    /// 4:3 (1024x768, 1280x960)
    Ratio4x3,
    /// 1:1 (정사각형)
    Ratio1x1,
    /// 세로 9:16 (모바일)
    Ratio9x16,
    /// 커스텀 비율
    Custom(u32, u32),
}

impl AspectRatioPreset {
    /// 종횡비 값 반환 (width / height)
    pub fn ratio(&self) -> Option<f32> {
        match self {
            AspectRatioPreset::Free => None,
            AspectRatioPreset::Ratio16x9 => Some(16.0 / 9.0),
            AspectRatioPreset::Ratio16x10 => Some(16.0 / 10.0),
            AspectRatioPreset::Ratio21x9 => Some(21.0 / 9.0),
            AspectRatioPreset::Ratio4x3 => Some(4.0 / 3.0),
            AspectRatioPreset::Ratio1x1 => Some(1.0),
            AspectRatioPreset::Ratio9x16 => Some(9.0 / 16.0),
            AspectRatioPreset::Custom(w, h) => Some(*w as f32 / *h as f32),
        }
    }

    /// 표시 이름
    pub fn display_name(&self) -> &'static str {
        match self {
            AspectRatioPreset::Free => "Free",
            AspectRatioPreset::Ratio16x9 => "16:9",
            AspectRatioPreset::Ratio16x10 => "16:10",
            AspectRatioPreset::Ratio21x9 => "21:9",
            AspectRatioPreset::Ratio4x3 => "4:3",
            AspectRatioPreset::Ratio1x1 => "1:1",
            AspectRatioPreset::Ratio9x16 => "9:16",
            AspectRatioPreset::Custom(_w, _h) => "Custom",
        }
    }

    /// 모든 기본 프리셋
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

/// 에디터 탭 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorTab {
    /// 게임 뷰포트 (씬 렌더링)
    Viewport,
    /// 씬 계층 구조
    Hierarchy,
    /// 인스펙터 (선택된 오브젝트 속성)
    Inspector,
    /// 에셋 브라우저
    AssetBrowser,
    /// 콘솔 (로그)
    Console,
    /// AI 어시스턴트
    AiAssistant,
}

impl EditorTab {
    /// 탭 이름
    pub fn title(&self) -> &'static str {
        match self {
            EditorTab::Viewport => "Viewport",
            EditorTab::Hierarchy => "Hierarchy",
            EditorTab::Inspector => "Inspector",
            EditorTab::AssetBrowser => "Assets",
            EditorTab::Console => "Console",
            EditorTab::AiAssistant => "AI Assistant",
        }
    }
}

/// 뷰 모드 (렌더링 방식)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// 일반 렌더링 (라이팅 + 텍스처)
    #[default]
    Lit,
    /// 라이팅 없는 렌더링
    Unlit,
    /// 와이어프레임
    Wireframe,
    /// Albedo만 표시
    Albedo,
    /// 노말 맵 시각화
    Normals,
    /// 뎁스 버퍼 시각화
    Depth,
}

impl ViewMode {
    /// 표시 이름
    pub fn display_name(&self) -> &'static str {
        match self {
            ViewMode::Lit => "Lit",
            ViewMode::Unlit => "Unlit",
            ViewMode::Wireframe => "Wireframe",
            ViewMode::Albedo => "Albedo",
            ViewMode::Normals => "Normals",
            ViewMode::Depth => "Depth",
        }
    }

    /// 모든 뷰 모드
    pub fn all() -> &'static [ViewMode] {
        &[
            ViewMode::Lit,
            ViewMode::Unlit,
            ViewMode::Wireframe,
            ViewMode::Albedo,
            ViewMode::Normals,
            ViewMode::Depth,
        ]
    }
}

/// 기즈모 모드 (변환 도구)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    /// 선택만 (기즈모 없음)
    #[default]
    Select,
    /// 이동
    Translate,
    /// 회전
    Rotate,
    /// 스케일
    Scale,
}

impl GizmoMode {
    /// 표시 이름
    pub fn display_name(&self) -> &'static str {
        match self {
            GizmoMode::Select => "Select",
            GizmoMode::Translate => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        }
    }

    /// 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            GizmoMode::Select => "🔍",
            GizmoMode::Translate => "↔",
            GizmoMode::Rotate => "↻",
            GizmoMode::Scale => "⤢",
        }
    }
}

/// 뷰포트 오버레이 정보 (외부에서 설정)
#[derive(Default, Clone)]
pub struct ViewportOverlayInfo {
    /// FPS
    pub fps: f32,
    /// 프레임 타임 (ms)
    pub frame_time_ms: f32,
    /// 드로우 콜 수
    pub draw_calls: u32,
    /// 삼각형 수
    pub triangles: u32,
    /// 카메라 위치
    pub camera_position: [f32; 3],
    /// 카메라 회전 (yaw, pitch)
    pub camera_rotation: [f32; 2],
    /// 카메라 타입 (Perspective/Orthographic)
    pub camera_type: &'static str,
    /// 그리드 표시 여부
    pub grid_visible: bool,
    /// 스냅 활성화 여부
    pub snap_enabled: bool,
    /// 선택된 엔티티 수
    pub selected_count: usize,
}

/// 뷰포트 렌더 타겟 정보
#[derive(Default)]
pub struct ViewportRenderTarget {
    /// egui 텍스처 ID (렌더링된 씬)
    pub texture_id: Option<TextureId>,
    /// 뷰포트 크기 (실제 렌더링 해상도)
    pub size: (u32, u32),
    /// 마우스가 뷰포트 위에 있는지
    pub hovered: bool,
    /// 뷰포트가 포커스 상태인지
    pub focused: bool,
    /// 뷰포트 영역 (screen coordinates, 레터박스 제외)
    pub rect: Option<Rect>,
    /// 패널 전체 영역 (레터박스 포함)
    pub panel_rect: Option<Rect>,
    /// 종횡비 프리셋
    pub aspect_ratio: AspectRatioPreset,
    /// 해상도 스케일 (0.25 ~ 2.0)
    pub resolution_scale: f32,
    /// 현재 뷰 모드
    pub view_mode: ViewMode,
    /// 현재 기즈모 모드
    pub gizmo_mode: GizmoMode,
    /// 오버레이 표시 여부
    pub show_overlay: bool,
    /// 오버레이 정보
    pub overlay_info: ViewportOverlayInfo,
    /// 단축키 도움말 표시 여부
    pub show_help: bool,
}

impl ViewportRenderTarget {
    pub fn new() -> Self {
        Self {
            texture_id: None,
            size: (1920, 1080),
            hovered: false,
            focused: false,
            rect: None,
            panel_rect: None,
            aspect_ratio: AspectRatioPreset::Free,
            resolution_scale: 1.0,
            view_mode: ViewMode::Lit,
            gizmo_mode: GizmoMode::Select,
            show_overlay: true,  // 기본적으로 오버레이 표시
            overlay_info: ViewportOverlayInfo::default(),
            show_help: false,  // 도움말은 기본적으로 숨김
        }
    }

    /// 패널 크기에서 실제 뷰포트 크기 계산 (종횡비 유지)
    pub fn calculate_viewport_rect(&self, panel_size: Vec2) -> (Rect, Vec2) {
        let target_ratio = self.aspect_ratio.ratio();

        match target_ratio {
            None => {
                // Free 모드: 패널 전체 사용
                let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), panel_size);
                (rect, panel_size)
            }
            Some(ratio) => {
                let panel_ratio = panel_size.x / panel_size.y;

                let (viewport_w, viewport_h) = if panel_ratio > ratio {
                    // 패널이 더 넓음 → 좌우 레터박스
                    let h = panel_size.y;
                    let w = h * ratio;
                    (w, h)
                } else {
                    // 패널이 더 높음 → 상하 레터박스
                    let w = panel_size.x;
                    let h = w / ratio;
                    (w, h)
                };

                // 중앙 정렬
                let offset_x = (panel_size.x - viewport_w) / 2.0;
                let offset_y = (panel_size.y - viewport_h) / 2.0;

                let rect = Rect::from_min_size(
                    egui::pos2(offset_x, offset_y),
                    egui::vec2(viewport_w, viewport_h),
                );

                (rect, egui::vec2(viewport_w, viewport_h))
            }
        }
    }

    /// 렌더링 해상도 계산 (스케일 적용)
    pub fn render_resolution(&self, viewport_size: Vec2) -> (u32, u32) {
        let w = (viewport_size.x * self.resolution_scale).max(1.0) as u32;
        let h = (viewport_size.y * self.resolution_scale).max(1.0) as u32;
        (w, h)
    }
}

/// 패널 크기 설정
#[derive(Clone)]
pub struct PanelSizes {
    /// 좌측 패널 (Hierarchy) 너비
    pub left_panel_width: f32,
    /// 우측 패널 (Inspector) 너비
    pub right_panel_width: f32,
    /// 하단 패널 (Console/Assets) 높이
    pub bottom_panel_height: f32,
    /// AI 패널 너비
    pub ai_panel_width: f32,
}

impl Default for PanelSizes {
    fn default() -> Self {
        Self {
            left_panel_width: 220.0,   // Hierarchy
            right_panel_width: 280.0,  // Inspector
            bottom_panel_height: 180.0, // Console/Assets
            ai_panel_width: 320.0,     // AI Assistant
        }
    }
}

/// 하단 패널에서 활성화된 탭
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomPanelTab {
    #[default]
    Console,
    Assets,
}

/// AI 패널 탭
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiPanelTab {
    #[default]
    Chat,
    Memory,
    Todos,
}

/// 에디터 도킹 레이아웃
pub struct DockLayout {
    /// 패널 크기
    pub sizes: PanelSizes,
    /// 뷰포트 렌더 타겟
    pub viewport: ViewportRenderTarget,
    /// 하단 패널 활성 탭
    pub bottom_tab: BottomPanelTab,
    /// 에디터 모드 (Edit/Play)
    pub is_playing: bool,
    /// AI 패널 표시 여부
    pub ai_panel_visible: bool,
    /// AI 패널 분리 상태 (별도 윈도우)
    pub ai_panel_detached: bool,
    /// AI 패널 현재 탭
    pub ai_panel_tab: AiPanelTab,
    /// 다국어 번역 시스템
    pub translations: Translations,
    /// 뷰포트 영역 (XYZ 기즈모 배치용)
    viewport_rect: Option<Rect>,
    /// 뷰포트에 드롭된 에셋 (경로, 스크린 좌표)
    pub dropped_asset: Option<(String, egui::Pos2)>,
    /// 드래그 중인 에셋이 뷰포트 위에 있는지
    pub drag_hover_viewport: bool,
}

impl DockLayout {
    /// 새 도킹 레이아웃 생성
    pub fn new() -> Self {
        Self {
            sizes: PanelSizes::default(),
            viewport: ViewportRenderTarget::new(),
            bottom_tab: BottomPanelTab::Console,
            is_playing: false,
            ai_panel_visible: false,  // 기본으로 숨김 (툴바에서 토글)
            ai_panel_detached: false,
            ai_panel_tab: AiPanelTab::Chat,
            translations: Translations::new(),
            viewport_rect: None,
            dropped_asset: None,
            drag_hover_viewport: false,
        }
    }

    /// 뷰포트 영역 가져오기 (XYZ 기즈모 배치용)
    pub fn get_viewport_rect(&self) -> Option<Rect> {
        self.viewport_rect
    }

    /// 현재 언어 가져오기
    pub fn language(&self) -> Language {
        self.translations.language()
    }

    /// 언어 설정
    pub fn set_language(&mut self, lang: Language) {
        self.translations.set_language(lang);
    }

    /// 번역 문자열 가져오기 (단축 메서드)
    pub fn t(&self, key: TextKey) -> &'static str {
        self.translations.get(key)
    }

    /// 메인 도킹 레이아웃 UI 렌더링
    ///
    /// 레이아웃:
    /// ```
    /// +----------+------------------+----------+----------+
    /// | Toolbar  |                  |          |          |
    /// +----------+                  +----------+          |
    /// |          |                  |          |    AI    |
    /// |Hierarchy |    Viewport      | Inspector| Assistant|
    /// |          |                  |          |          |
    /// +----------+------------------+----------+----------+
    /// |  Console  |     Assets                            |
    /// +----------+----------------------------------------+
    /// ```
    pub fn ui(
        &mut self,
        ctx: &Context,
        hierarchy_ui: impl FnOnce(&mut Ui),
        inspector_ui: impl FnOnce(&mut Ui),
        console_ui: impl FnOnce(&mut Ui),
        asset_browser_ui: impl FnOnce(&mut Ui),
        ai_panel_ui: impl FnOnce(&mut Ui, AiPanelTab),
    ) {
        // 에디터 다크 테마 적용
        self.apply_editor_style(ctx);

        // 상단 툴바
        self.toolbar_ui(ctx);

        // 하단 패널 (Console / Assets)
        self.bottom_panel_ui(ctx, console_ui, asset_browser_ui);

        // 좌측 패널 (Hierarchy)
        self.left_panel_ui(ctx, hierarchy_ui);

        // AI 패널 (Inspector 오른쪽, 분리 안됨 + 표시 중일 때)
        if self.ai_panel_visible && !self.ai_panel_detached {
            self.ai_panel_ui(ctx, ai_panel_ui);
        }

        // 우측 패널 (Inspector)
        self.right_panel_ui(ctx, inspector_ui);

        // 중앙 뷰포트 (남은 공간)
        self.viewport_ui(ctx);
    }

    /// 에디터 스타일 적용
    ///
    /// UX 개선: 현대적인 플랫 디자인, 통일된 라운딩, 명확한 색상 대비
    fn apply_editor_style(&self, ctx: &Context) {
        let mut style = (*ctx.style()).clone();

        // ========== 다크 테마 색상 (플랫 디자인) ==========
        // 그라데이션 제거, 단색 사용
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = Color32::from_rgb(30, 30, 34);      // 패널 배경
        style.visuals.window_fill = Color32::from_rgb(35, 35, 40);     // 윈도우 배경
        style.visuals.extreme_bg_color = Color32::from_rgb(22, 22, 26); // 가장 어두운 배경
        style.visuals.faint_bg_color = Color32::from_rgb(40, 40, 45);  // 약간 밝은 배경

        // ========== 위젯 색상 (일관된 대비) ==========
        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(45, 45, 50);
        style.visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(160, 160, 170);

        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 50, 56);
        style.visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(180, 180, 190);

        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 60, 70);
        style.visuals.widgets.hovered.fg_stroke.color = Color32::from_rgb(220, 220, 230);

        style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 90, 130);
        style.visuals.widgets.active.fg_stroke.color = Color32::from_rgb(240, 240, 250);

        // ========== 선택 색상 (명확한 강조) ==========
        style.visuals.selection.bg_fill = Color32::from_rgb(50, 80, 140);
        style.visuals.selection.stroke.color = Color32::from_rgb(80, 130, 200);

        // ========== 라운딩 통일 (4px) ==========
        // WidgetVisuals는 직접 수정 불가, 위젯 스타일은 기본값 사용

        // ========== 간격 조정 ==========
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(6.0, 3.0);
        style.spacing.window_margin = egui::Margin::same(8);

        // ========== 그림자 (활성 창 깊이감) ==========
        // egui 0.33+ 버전에서 Shadow 타입 변경됨
        style.visuals.popup_shadow = egui::Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(80),
        };
        style.visuals.window_shadow = egui::Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(60),
        };

        ctx.set_style(style);
    }

    /// 상단 툴바 UI
    ///
    /// UX 개선: 명확한 그룹 구분, 토글 상태 시각화, 수직 구분선
    fn toolbar_ui(&mut self, ctx: &Context) {
        // 번역 문자열 미리 가져오기 (borrow checker 회피)
        let tooltip_play = self.t(TextKey::TooltipPlay);
        let tooltip_stop = self.t(TextKey::TooltipStop);
        let tooltip_pause = self.t(TextKey::TooltipPause);
        let tooltip_move = self.t(TextKey::TooltipMove);
        let tooltip_rotate = self.t(TextKey::TooltipRotate);
        let tooltip_scale = self.t(TextKey::TooltipScale);
        let tooltip_grid = self.t(TextKey::TooltipGrid);
        let tooltip_snap = self.t(TextKey::TooltipSnap);
        let tooltip_ai = self.t(TextKey::TooltipAiPanel);
        let tooltip_lang = self.t(TextKey::TooltipLanguage);
        let mode_edit = self.t(TextKey::EditMode);
        let mode_play = self.t(TextKey::PlayMode);
        let current_lang = self.translations.language();

        egui::TopBottomPanel::top("toolbar")
            .exact_height(40.0)
            .show(ctx, |ui| {
                // 툴바 배경
                let toolbar_rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(
                    toolbar_rect,
                    0.0,
                    Color32::from_rgb(38, 40, 48),
                );

                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);

                    // ========== 로고 영역 ==========
                    ui.label(egui::RichText::new("SKOPE")
                        .size(15.0)
                        .strong()
                        .color(Color32::from_rgb(100, 170, 240)));

                    Self::toolbar_separator(ui);

                    // ========== 재생 제어 그룹 ==========
                    Self::toolbar_group(ui, |ui| {
                        let play_icon = if self.is_playing { "⏹" } else { "▶" };
                        let play_color = if self.is_playing {
                            Color32::from_rgb(240, 100, 100)
                        } else {
                            Color32::from_rgb(100, 210, 140)
                        };

                        if Self::toolbar_button(ui, play_icon, play_color, self.is_playing)
                            .on_hover_text(if self.is_playing { tooltip_stop } else { tooltip_play })
                            .clicked() {
                            self.is_playing = !self.is_playing;
                        }

                        if Self::toolbar_button(ui, "⏸", Color32::from_rgb(180, 180, 190), false)
                            .on_hover_text(tooltip_pause)
                            .clicked() {
                            // TODO: 일시정지 기능
                        }
                    });

                    Self::toolbar_separator(ui);

                    // ========== 변환 도구 그룹 ==========
                    Self::toolbar_group(ui, |ui| {
                        if Self::toolbar_button(ui, "↔", Color32::from_rgb(140, 200, 255), false)
                            .on_hover_text(tooltip_move)
                            .clicked() {
                            // TODO: 이동 기즈모 활성화
                        }
                        if Self::toolbar_button(ui, "↻", Color32::from_rgb(255, 180, 140), false)
                            .on_hover_text(tooltip_rotate)
                            .clicked() {
                            // TODO: 회전 기즈모 활성화
                        }
                        if Self::toolbar_button(ui, "⤢", Color32::from_rgb(180, 255, 180), false)
                            .on_hover_text(tooltip_scale)
                            .clicked() {
                            // TODO: 스케일 기즈모 활성화
                        }
                    });

                    Self::toolbar_separator(ui);

                    // ========== 뷰 옵션 그룹 ==========
                    Self::toolbar_group(ui, |ui| {
                        let grid_active = true; // TODO: 실제 상태 연결
                        if Self::toolbar_button(ui, "▦", Color32::from_rgb(160, 160, 180), grid_active)
                            .on_hover_text(tooltip_grid)
                            .clicked() {
                            // TODO: 그리드 토글
                        }

                        let snap_active = false; // TODO: 실제 상태 연결
                        if Self::toolbar_button(ui, "⊞", Color32::from_rgb(160, 160, 180), snap_active)
                            .on_hover_text(tooltip_snap)
                            .clicked() {
                            // TODO: 스냅 토글
                        }
                    });

                    // ========== 우측 영역 (언어 + 상태 + 토글) ==========
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);

                        // 언어 선택 콤보박스
                        egui::ComboBox::from_id_salt("language_selector")
                            .selected_text(current_lang.display_name())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                for lang in Language::all() {
                                    let is_selected = *lang == current_lang;
                                    if ui.selectable_label(is_selected, lang.display_name()).clicked() {
                                        self.translations.set_language(*lang);
                                    }
                                }
                            })
                            .response
                            .on_hover_text(tooltip_lang);

                        ui.add_space(8.0);

                        // AI 패널 토글 버튼
                        if Self::toolbar_button(ui, "◈", Color32::from_rgb(100, 170, 240), self.ai_panel_visible)
                            .on_hover_text(tooltip_ai)
                            .clicked() {
                            self.ai_panel_visible = !self.ai_panel_visible;
                        }

                        ui.add_space(12.0);

                        // 현재 모드 표시 (배지 스타일)
                        let mode_text = if self.is_playing { mode_play } else { mode_edit };
                        let (mode_bg, mode_fg) = if self.is_playing {
                            (Color32::from_rgb(60, 100, 70), Color32::from_rgb(140, 230, 160))
                        } else {
                            (Color32::from_rgb(50, 55, 65), Color32::from_rgb(140, 145, 160))
                        };

                        let badge_size = egui::vec2(50.0, 20.0);
                        let (badge_rect, _) = ui.allocate_exact_size(badge_size, Sense::hover());
                        ui.painter().rect_filled(badge_rect, 4.0, mode_bg);
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            mode_text,
                            egui::FontId::proportional(10.0),
                            mode_fg,
                        );
                    });
                });
            });
    }

    /// 툴바 그룹 (버튼들을 감싸는 배경)
    fn toolbar_group(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
        let group_rect = ui.available_rect_before_wrap();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            content(ui);
        });
        // 그룹 배경은 투명하게 (버튼 자체가 배경을 가짐)
        let _ = group_rect;
    }

    /// 툴바 수직 구분선
    fn toolbar_separator(ui: &mut Ui) {
        ui.add_space(10.0);
        let rect = ui.available_rect_before_wrap();
        ui.painter().vline(
            rect.left(),
            (rect.top() + 8.0)..=(rect.bottom() - 8.0),
            egui::Stroke::new(1.0, Color32::from_rgb(60, 62, 70)),
        );
        ui.add_space(10.0);
    }

    /// 툴바 버튼 (통일된 스타일)
    fn toolbar_button(ui: &mut Ui, icon: &str, icon_color: Color32, is_active: bool) -> egui::Response {
        let btn_size = egui::vec2(30.0, 26.0);
        let (rect, response) = ui.allocate_exact_size(btn_size, Sense::click());

        let is_hovered = response.hovered();

        // 배경
        let bg_color = if is_active {
            Color32::from_rgb(55, 75, 105)
        } else if is_hovered {
            Color32::from_rgb(55, 58, 68)
        } else {
            Color32::from_rgb(45, 48, 56)
        };

        ui.painter().rect_filled(rect, 4.0, bg_color);

        // 활성 상태 하단 인디케이터
        if is_active {
            ui.painter().hline(
                (rect.left() + 4.0)..=(rect.right() - 4.0),
                rect.bottom() - 2.0,
                egui::Stroke::new(2.0, icon_color),
            );
        }

        // 테두리 (호버 시)
        if is_hovered {
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, Color32::from_rgb(80, 85, 100)),
                egui::StrokeKind::Inside,
            );
        }

        // 아이콘
        let final_color = if is_active || is_hovered {
            icon_color
        } else {
            icon_color.gamma_multiply(0.7)
        };

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(13.0),
            final_color,
        );

        response
    }

    /// 하단 패널 UI (Console / Assets 탭)
    ///
    /// UX 개선: 탭에 아이콘 + 하단 인디케이터 + 호버 효과
    fn bottom_panel_ui(
        &mut self,
        ctx: &Context,
        console_ui: impl FnOnce(&mut Ui),
        asset_browser_ui: impl FnOnce(&mut Ui),
    ) {
        // 번역 문자열 미리 가져오기
        let console_label = self.t(TextKey::Console);
        let assets_label = self.t(TextKey::Assets);

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .min_height(100.0)
            .max_height(400.0)
            .default_height(self.sizes.bottom_panel_height)
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());

                // 탭 바 배경
                let tab_bar_height = 28.0;
                let tab_bar_rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), tab_bar_height),
                );
                ui.painter().rect_filled(
                    tab_bar_rect,
                    0.0,
                    Color32::from_rgb(35, 38, 45),
                );

                // 탭 헤더
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), tab_bar_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(8.0);

                        // Console 탭
                        let console_selected = self.bottom_tab == BottomPanelTab::Console;
                        if Self::render_tab(ui, "📋", console_label, console_selected, Color32::from_rgb(140, 180, 220)) {
                            self.bottom_tab = BottomPanelTab::Console;
                        }

                        ui.add_space(4.0);

                        // Assets 탭
                        let assets_selected = self.bottom_tab == BottomPanelTab::Assets;
                        if Self::render_tab(ui, "📁", assets_label, assets_selected, Color32::from_rgb(180, 160, 120)) {
                            self.bottom_tab = BottomPanelTab::Assets;
                        }
                    },
                );

                // 구분선
                ui.painter().hline(
                    tab_bar_rect.left()..=tab_bar_rect.right(),
                    tab_bar_rect.bottom(),
                    egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 56)),
                );
                ui.add_space(6.0);

                // 선택된 탭 내용
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.bottom_tab {
                        BottomPanelTab::Console => console_ui(ui),
                        BottomPanelTab::Assets => asset_browser_ui(ui),
                    }
                });
            });
    }

    /// 빈 상태 안내 메시지 렌더링
    ///
    /// 패널에 내용이 없을 때 아이콘 + 메시지를 중앙에 표시
    /// 힌트 텍스트도 선택적으로 표시 가능
    pub fn empty_state(ui: &mut Ui, icon: &str, message: &str, hint: Option<&str>) {
        let available = ui.available_size();
        let center = ui.cursor().min + egui::vec2(available.x / 2.0, available.y / 2.0 - 20.0);

        // 아이콘 (크게)
        ui.painter().text(
            center - egui::vec2(0.0, 20.0),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(32.0),
            Color32::from_rgb(70, 75, 85),
        );

        // 메인 메시지
        ui.painter().text(
            center + egui::vec2(0.0, 16.0),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(13.0),
            Color32::from_rgb(110, 115, 125),
        );

        // 힌트 (있는 경우)
        if let Some(hint_text) = hint {
            ui.painter().text(
                center + egui::vec2(0.0, 38.0),
                egui::Align2::CENTER_CENTER,
                hint_text,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(80, 85, 95),
            );
        }

        // 공간 할당 (UI 레이아웃 유지)
        ui.allocate_space(available);
    }

    /// 빈 상태 안내 (아이콘 없이, 작은 패널용)
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

    /// 탭 버튼 렌더링 (아이콘 + 라벨 + 하단 인디케이터)
    fn render_tab(ui: &mut Ui, icon: &str, label: &str, selected: bool, accent_color: Color32) -> bool {
        let tab_width = 80.0;
        let tab_height = 24.0;

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(tab_width, tab_height),
            Sense::click(),
        );

        let is_hovered = response.hovered();

        // 배경 (호버 시)
        if is_hovered && !selected {
            ui.painter().rect_filled(
                rect,
                4.0,
                Color32::from_rgb(50, 53, 60),
            );
        }

        // 선택 시 배경
        if selected {
            ui.painter().rect_filled(
                rect,
                4.0,
                Color32::from_rgb(45, 48, 55),
            );
        }

        // 아이콘 + 라벨
        let text_color = if selected {
            Color32::from_rgb(230, 230, 240)
        } else if is_hovered {
            Color32::from_rgb(200, 200, 210)
        } else {
            Color32::from_rgb(140, 140, 150)
        };

        let icon_color = if selected { accent_color } else { text_color };

        // 아이콘
        ui.painter().text(
            rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            icon_color,
        );

        // 라벨
        ui.painter().text(
            rect.left_center() + egui::vec2(24.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            text_color,
        );

        // 하단 인디케이터 (선택 시)
        if selected {
            ui.painter().hline(
                (rect.left() + 4.0)..=(rect.right() - 4.0),
                rect.bottom() - 1.0,
                egui::Stroke::new(2.0, accent_color),
            );
        }

        response.clicked()
    }

    /// 좌측 패널 UI (Hierarchy)
    ///
    /// UX 개선: 아이콘 + 헤더 + 언더라인, 빈 상태 안내
    fn left_panel_ui(&mut self, ctx: &Context, hierarchy_ui: impl FnOnce(&mut Ui)) {
        let hierarchy_label = self.t(TextKey::Hierarchy);

        egui::SidePanel::left("hierarchy_panel")
            .resizable(true)
            .min_width(150.0)
            .max_width(400.0)
            .default_width(self.sizes.left_panel_width)
            .show(ctx, |ui| {
                ui.set_min_width(ui.available_width());

                // 패널 헤더 (아이콘 + 라벨 + 언더라인)
                Self::panel_header(ui, "🗂", hierarchy_label, Color32::from_rgb(100, 180, 140));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    hierarchy_ui(ui);
                });
            });
    }

    /// 공통 패널 헤더 렌더링
    fn panel_header(ui: &mut Ui, icon: &str, label: &str, accent_color: Color32) {
        let header_height = 28.0;

        // 헤더 배경
        let header_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            egui::vec2(ui.available_width(), header_height),
        );
        ui.painter().rect_filled(
            header_rect,
            0.0,
            Color32::from_rgb(35, 38, 45),
        );

        // 헤더 내용
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), header_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(icon).size(13.0).color(accent_color));
                ui.add_space(6.0);
                ui.label(egui::RichText::new(label)
                    .size(12.0)
                    .strong()
                    .color(Color32::from_rgb(200, 200, 210)));
            },
        );

        // 하단 강조선
        let line_rect = ui.cursor();
        ui.painter().hline(
            line_rect.min.x..=line_rect.min.x + ui.available_width(),
            line_rect.min.y,
            egui::Stroke::new(2.0, accent_color.gamma_multiply(0.5)),
        );
        ui.add_space(6.0);
    }

    /// AI 패널 UI (Inspector 오른쪽)
    ///
    /// UX 개선: 아이콘 헤더 + 탭 인디케이터 + 호버 효과
    fn ai_panel_ui(&mut self, ctx: &Context, content_ui: impl FnOnce(&mut Ui, AiPanelTab)) {
        // 번역 문자열 미리 가져오기
        let ai_label = self.t(TextKey::AiAssistant);
        let chat_label = self.t(TextKey::Chat);
        let memory_label = self.t(TextKey::Memory);
        let todos_label = self.t(TextKey::Todos);
        let popout_label = self.t(TextKey::PopOut);
        let current_tab = self.ai_panel_tab;

        let accent_color = Color32::from_rgb(100, 160, 230);

        egui::SidePanel::right("ai_panel")
            .resizable(true)
            .min_width(280.0)
            .max_width(500.0)
            .default_width(self.sizes.ai_panel_width)
            .show(ctx, |ui| {
                ui.set_min_width(ui.available_width());

                // 패널 헤더 (아이콘 + 라벨)
                let header_height = 28.0;
                let header_rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), header_height),
                );
                ui.painter().rect_filled(header_rect, 0.0, Color32::from_rgb(35, 38, 45));

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), header_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("◈").size(13.0).color(accent_color));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(ai_label)
                            .size(12.0)
                            .strong()
                            .color(Color32::from_rgb(200, 200, 210)));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            // 분리 버튼
                            if ui.add(egui::Button::new(
                                egui::RichText::new("↗").size(11.0)
                            ).min_size(egui::vec2(22.0, 20.0)))
                            .on_hover_text(popout_label)
                            .clicked() {
                                self.ai_panel_detached = true;
                            }
                        });
                    },
                );

                // 하단 강조선
                ui.painter().hline(
                    header_rect.left()..=header_rect.right(),
                    header_rect.bottom(),
                    egui::Stroke::new(2.0, accent_color.gamma_multiply(0.5)),
                );
                ui.add_space(4.0);

                // 탭 바
                let tab_bar_height = 26.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), tab_bar_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(4.0);

                        // Chat 탭
                        if Self::render_tab(ui, "💬", chat_label, self.ai_panel_tab == AiPanelTab::Chat, accent_color) {
                            self.ai_panel_tab = AiPanelTab::Chat;
                        }

                        // Memory 탭
                        if Self::render_tab(ui, "🧠", memory_label, self.ai_panel_tab == AiPanelTab::Memory, Color32::from_rgb(180, 140, 200)) {
                            self.ai_panel_tab = AiPanelTab::Memory;
                        }

                        // Todos 탭
                        if Self::render_tab(ui, "📝", todos_label, self.ai_panel_tab == AiPanelTab::Todos, Color32::from_rgb(140, 200, 160)) {
                            self.ai_panel_tab = AiPanelTab::Todos;
                        }
                    },
                );

                ui.add_space(6.0);

                // 탭 내용
                egui::ScrollArea::vertical().show(ui, |ui| {
                    content_ui(ui, current_tab);
                });
            });
    }

    /// 우측 패널 UI (Inspector)
    ///
    /// UX 개선: 아이콘 + 헤더 + 언더라인
    fn right_panel_ui(&mut self, ctx: &Context, inspector_ui: impl FnOnce(&mut Ui)) {
        let inspector_label = self.t(TextKey::Inspector);

        egui::SidePanel::right("inspector_panel")
            .resizable(true)
            .min_width(200.0)
            .max_width(400.0)
            .default_width(self.sizes.right_panel_width)
            .show(ctx, |ui| {
                ui.set_min_width(ui.available_width());

                // 패널 헤더 (아이콘 + 라벨 + 언더라인)
                Self::panel_header(ui, "🔧", inspector_label, Color32::from_rgb(200, 160, 100));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    inspector_ui(ui);
                });
            });
    }

    /// 중앙 뷰포트 UI
    fn viewport_ui(&mut self, ctx: &Context) {
        // 번역 문자열 미리 가져오기
        let viewport_label = self.t(TextKey::GameViewport);
        let aspect_label = self.t(TextKey::Aspect);
        let scale_label = self.t(TextKey::Resolution);

        egui::CentralPanel::default().show(ctx, |ui| {
            // ========== 상단 뷰포트 컨트롤 바 ==========
            let control_bar_height = 28.0;
            let control_bar_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                egui::vec2(ui.available_width(), control_bar_height),
            );
            ui.painter().rect_filled(control_bar_rect, 0.0, Color32::from_rgb(35, 38, 45));

            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), control_bar_height),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);

                    // 기즈모 모드 버튼 그룹
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;

                        let gizmo_modes = [
                            (GizmoMode::Select, Color32::from_rgb(180, 180, 200)),
                            (GizmoMode::Translate, Color32::from_rgb(140, 200, 255)),
                            (GizmoMode::Rotate, Color32::from_rgb(255, 180, 140)),
                            (GizmoMode::Scale, Color32::from_rgb(180, 255, 180)),
                        ];

                        for (mode, color) in &gizmo_modes {
                            let is_selected = self.viewport.gizmo_mode == *mode;
                            if Self::toolbar_button(ui, mode.icon(), *color, is_selected)
                                .on_hover_text(mode.display_name())
                                .clicked() {
                                self.viewport.gizmo_mode = *mode;
                            }
                        }
                    });

                    Self::toolbar_separator(ui);

                    // 뷰 모드 선택
                    ui.label(egui::RichText::new("👁").size(11.0).color(Color32::from_rgb(140, 160, 180)));
                    ui.add_space(4.0);
                    egui::ComboBox::from_id_salt("view_mode_combo")
                        .selected_text(self.viewport.view_mode.display_name())
                        .width(75.0)
                        .show_ui(ui, |ui| {
                            for mode in ViewMode::all() {
                                let is_selected = self.viewport.view_mode == *mode;
                                if ui.selectable_label(is_selected, mode.display_name()).clicked() {
                                    self.viewport.view_mode = *mode;
                                }
                            }
                        });

                    Self::toolbar_separator(ui);

                    // 종횡비 선택
                    ui.label(egui::RichText::new(aspect_label).size(10.0).color(Color32::from_rgb(120, 120, 135)));
                    ui.add_space(4.0);
                    egui::ComboBox::from_id_salt("aspect_ratio_combo")
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

                    ui.add_space(8.0);

                    // 해상도 스케일
                    ui.label(egui::RichText::new(scale_label).size(10.0).color(Color32::from_rgb(120, 120, 135)));
                    ui.add_space(4.0);
                    let scale_text = format!("{:.0}%", self.viewport.resolution_scale * 100.0);
                    egui::ComboBox::from_id_salt("resolution_scale_combo")
                        .selected_text(scale_text)
                        .width(50.0)
                        .show_ui(ui, |ui| {
                            for scale in &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
                                let label = format!("{:.0}%", scale * 100.0);
                                if ui.selectable_label(
                                    (self.viewport.resolution_scale - scale).abs() < 0.01,
                                    label
                                ).clicked() {
                                    self.viewport.resolution_scale = *scale as f32;
                                }
                            }
                        });

                    // 우측 영역
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);

                        // 도움말 토글
                        if Self::toolbar_button(ui, "?", Color32::from_rgb(140, 180, 220), self.viewport.show_help)
                            .on_hover_text("Keyboard Shortcuts (H)")
                            .clicked() {
                            self.viewport.show_help = !self.viewport.show_help;
                        }

                        // 오버레이 토글
                        if Self::toolbar_button(ui, "📊", Color32::from_rgb(140, 200, 160), self.viewport.show_overlay)
                            .on_hover_text("Toggle Stats Overlay")
                            .clicked() {
                            self.viewport.show_overlay = !self.viewport.show_overlay;
                        }

                        ui.add_space(8.0);

                        // 현재 해상도 표시 (배지 스타일)
                        let (w, h) = self.viewport.size;
                        let res_text = format!("{}×{}", w, h);
                        let res_size = egui::vec2(70.0, 18.0);
                        let (res_rect, _) = ui.allocate_exact_size(res_size, Sense::hover());
                        ui.painter().rect_filled(res_rect, 3.0, Color32::from_rgb(40, 42, 50));
                        ui.painter().text(
                            res_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            res_text,
                            egui::FontId::monospace(9.0),
                            Color32::from_rgb(120, 125, 140),
                        );
                    });
                },
            );

            // ========== 뷰포트 영역 ==========
            let available_size = ui.available_size();
            let panel_min = ui.cursor().min;

            // 종횡비에 따른 뷰포트 영역 계산
            let (local_viewport_rect, viewport_size) = self.viewport.calculate_viewport_rect(available_size);

            // 렌더링 해상도 계산 (스케일 적용)
            let (render_w, render_h) = self.viewport.render_resolution(viewport_size);
            self.viewport.size = (render_w, render_h);

            // 전체 패널 영역 (레터박스 포함)
            let panel_rect = Rect::from_min_size(panel_min, available_size);
            self.viewport.panel_rect = Some(panel_rect);

            // 실제 뷰포트 영역 (스크린 좌표)
            let viewport_rect = Rect::from_min_size(
                panel_min + local_viewport_rect.min.to_vec2(),
                viewport_size,
            );

            // 뷰포트 영역 저장 (XYZ 기즈모 배치용)
            self.viewport_rect = Some(viewport_rect);

            // 레터박스 배경 (검은색)
            // Sense::hover()를 사용하여 egui가 마우스 이벤트를 소비하지 않도록 함
            // 이렇게 해야 씬 뷰포트에서 카메라 조작이 가능함
            let (full_rect, response) = ui.allocate_exact_size(available_size, Sense::hover());

            // ========== 드래그 앤 드롭 감지 ==========
            // dnd_hover_payload: 드래그 중인 페이로드가 이 응답 위에 있는지 확인
            let is_hovering = response.dnd_hover_payload::<String>().is_some();
            self.drag_hover_viewport = is_hovering;

            // dnd_release_payload: 드롭이 완료되었는지 확인
            if let Some(payload) = response.dnd_release_payload::<String>() {
                let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
                if let Some(pos) = pointer_pos {
                    self.dropped_asset = Some(((*payload).clone(), pos));
                    log::info!("[Viewport] Asset dropped: {} at {:?}", *payload, pos);
                }
            }
            ui.painter().rect_filled(full_rect, 0.0, Color32::from_rgb(15, 15, 18));

            // 뷰포트 텍스처가 있으면 표시
            if let Some(texture_id) = self.viewport.texture_id {
                // 뷰포트 이미지 그리기
                ui.painter().image(
                    texture_id,
                    viewport_rect,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                // 텍스처가 없으면 플레이스홀더 표시
                ui.painter().rect_filled(viewport_rect, 0.0, Color32::from_rgb(25, 25, 30));

                ui.painter().text(
                    viewport_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    viewport_label,
                    egui::FontId::proportional(20.0),
                    Color32::from_rgb(70, 70, 80),
                );

                // 종횡비 표시
                if let Some(_ratio) = self.viewport.aspect_ratio.ratio() {
                    ui.painter().text(
                        viewport_rect.center() + egui::vec2(0.0, 28.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{} ({}×{})", self.viewport.aspect_ratio.display_name(), render_w, render_h),
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(55, 55, 65),
                    );
                }
            }

            // 호버 감지
            self.viewport.hovered = viewport_rect.contains(
                ctx.input(|i| i.pointer.hover_pos().unwrap_or_default())
            );
            self.viewport.rect = Some(viewport_rect);

            // 뷰포트 테두리 (호버/포커스 표시)
            if self.viewport.hovered || self.viewport.focused {
                let border_color = if self.viewport.focused {
                    Color32::from_rgb(90, 140, 200)
                } else {
                    Color32::from_rgb(60, 90, 140)
                };
                ui.painter().rect_stroke(
                    viewport_rect,
                    0.0,
                    egui::Stroke::new(1.0, border_color),
                    egui::StrokeKind::Inside,
                );
            }

            // 드래그 호버 시 드롭 가능 표시
            if self.drag_hover_viewport {
                // 반투명 오버레이
                ui.painter().rect_filled(
                    viewport_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(80, 140, 220, 40),
                );

                // 강조 테두리
                ui.painter().rect_stroke(
                    viewport_rect,
                    0.0,
                    egui::Stroke::new(2.0, Color32::from_rgb(80, 160, 255)),
                    egui::StrokeKind::Inside,
                );

                // 드롭 안내 텍스트
                ui.painter().text(
                    viewport_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop to place object",
                    egui::FontId::proportional(16.0),
                    Color32::from_rgb(180, 200, 255),
                );
            }

            // ========== 오버레이 UI ==========
            if self.viewport.show_overlay {
                let overlay = &self.viewport.overlay_info;
                let padding = 8.0;

                // 좌상단: 성능 정보
                let perf_text = format!(
                    "{:.1} FPS\n{:.2} ms\n{} draws\n{}K tris",
                    overlay.fps,
                    overlay.frame_time_ms,
                    overlay.draw_calls,
                    overlay.triangles / 1000
                );
                let perf_pos = viewport_rect.min + egui::vec2(padding, padding);

                // 배경
                let text_galley = ui.painter().layout_no_wrap(
                    perf_text.clone(),
                    egui::FontId::monospace(10.0),
                    Color32::WHITE,
                );
                let bg_rect = Rect::from_min_size(
                    perf_pos - egui::vec2(4.0, 2.0),
                    text_galley.size() + egui::vec2(8.0, 4.0),
                );
                ui.painter().rect_filled(bg_rect, 4.0, Color32::from_black_alpha(180));

                ui.painter().text(
                    perf_pos,
                    egui::Align2::LEFT_TOP,
                    perf_text,
                    egui::FontId::monospace(10.0),
                    Color32::from_rgb(200, 200, 210),
                );

                // 우상단: 카메라 정보
                let cam_text = format!(
                    "{}\nPos: {:.1}, {:.1}, {:.1}\nYaw: {:.0}° Pitch: {:.0}°",
                    overlay.camera_type,
                    overlay.camera_position[0],
                    overlay.camera_position[1],
                    overlay.camera_position[2],
                    overlay.camera_rotation[0].to_degrees(),
                    overlay.camera_rotation[1].to_degrees(),
                );
                let cam_galley = ui.painter().layout_no_wrap(
                    cam_text.clone(),
                    egui::FontId::monospace(10.0),
                    Color32::WHITE,
                );
                let cam_pos = egui::pos2(
                    viewport_rect.max.x - cam_galley.size().x - padding,
                    viewport_rect.min.y + padding,
                );
                let cam_bg_rect = Rect::from_min_size(
                    cam_pos - egui::vec2(4.0, 2.0),
                    cam_galley.size() + egui::vec2(8.0, 4.0),
                );
                ui.painter().rect_filled(cam_bg_rect, 4.0, Color32::from_black_alpha(180));

                ui.painter().text(
                    cam_pos,
                    egui::Align2::LEFT_TOP,
                    cam_text,
                    egui::FontId::monospace(10.0),
                    Color32::from_rgb(200, 200, 210),
                );

                // 좌하단: 상태 표시
                let mut status_parts: Vec<String> = Vec::new();
                if overlay.grid_visible {
                    status_parts.push("Grid".to_string());
                }
                if overlay.snap_enabled {
                    status_parts.push("Snap".to_string());
                }
                if overlay.selected_count > 0 {
                    status_parts.push(format!("{} selected", overlay.selected_count));
                }

                if !status_parts.is_empty() || self.viewport.gizmo_mode != GizmoMode::Select {
                    let gizmo_text = format!("{} {}",
                        self.viewport.gizmo_mode.icon(),
                        self.viewport.gizmo_mode.display_name()
                    );

                    let status_text = if status_parts.is_empty() {
                        gizmo_text
                    } else {
                        format!("{} | {}", gizmo_text, status_parts.join(" | "))
                    };

                    let status_galley = ui.painter().layout_no_wrap(
                        status_text.clone(),
                        egui::FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                    let status_pos = egui::pos2(
                        viewport_rect.min.x + padding,
                        viewport_rect.max.y - status_galley.size().y - padding,
                    );
                    let status_bg_rect = Rect::from_min_size(
                        status_pos - egui::vec2(4.0, 2.0),
                        status_galley.size() + egui::vec2(8.0, 4.0),
                    );
                    ui.painter().rect_filled(status_bg_rect, 4.0, Color32::from_black_alpha(180));

                    ui.painter().text(
                        status_pos,
                        egui::Align2::LEFT_TOP,
                        status_text,
                        egui::FontId::proportional(10.0),
                        Color32::from_rgb(180, 180, 190),
                    );
                }

                // 우하단: 뷰 모드 표시 (Lit이 아닌 경우)
                if self.viewport.view_mode != ViewMode::Lit {
                    let view_text = self.viewport.view_mode.display_name();
                    let view_galley = ui.painter().layout_no_wrap(
                        view_text.to_string(),
                        egui::FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                    let view_pos = egui::pos2(
                        viewport_rect.max.x - view_galley.size().x - padding,
                        viewport_rect.max.y - view_galley.size().y - padding,
                    );
                    let view_bg_rect = Rect::from_min_size(
                        view_pos - egui::vec2(4.0, 2.0),
                        view_galley.size() + egui::vec2(8.0, 4.0),
                    );
                    ui.painter().rect_filled(view_bg_rect, 4.0, Color32::from_rgba_unmultiplied(100, 60, 60, 200));

                    ui.painter().text(
                        view_pos,
                        egui::Align2::LEFT_TOP,
                        view_text,
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(255, 200, 200),
                    );
                }

                // ========== 단축키 도움말 오버레이 ==========
                if self.viewport.show_help {
                    // 중앙에 반투명 패널로 단축키 표시
                    let help_width = 280.0;
                    let help_height = 320.0;
                    let help_pos = egui::pos2(
                        viewport_rect.center().x - help_width / 2.0,
                        viewport_rect.center().y - help_height / 2.0,
                    );
                    let help_rect = Rect::from_min_size(help_pos, egui::vec2(help_width, help_height));

                    // 배경
                    ui.painter().rect_filled(help_rect, 8.0, Color32::from_rgba_unmultiplied(25, 28, 35, 240));
                    ui.painter().rect_stroke(
                        help_rect,
                        8.0,
                        egui::Stroke::new(1.0, Color32::from_rgb(60, 65, 75)),
                        egui::StrokeKind::Outside,
                    );

                    let text_color = Color32::from_rgb(200, 205, 215);
                    let key_color = Color32::from_rgb(130, 180, 255);
                    let title_color = Color32::from_rgb(255, 255, 255);
                    let section_color = Color32::from_rgb(150, 155, 165);

                    let mut y = help_pos.y + 16.0;
                    let left_x = help_pos.x + 16.0;
                    let right_x = help_pos.x + help_width - 16.0;
                    let line_height = 18.0;

                    // 타이틀
                    ui.painter().text(
                        egui::pos2(help_rect.center().x, y),
                        egui::Align2::CENTER_TOP,
                        "Keyboard Shortcuts",
                        egui::FontId::proportional(14.0),
                        title_color,
                    );
                    y += 28.0;

                    // === 카메라 섹션 ===
                    ui.painter().text(egui::pos2(left_x, y), egui::Align2::LEFT_TOP, "Camera", egui::FontId::proportional(11.0), section_color);
                    y += line_height;

                    let camera_shortcuts = [
                        ("RMB + Drag", "Rotate Camera"),
                        ("MMB + Drag", "Pan Camera"),
                        ("Scroll", "Zoom In/Out"),
                        ("F", "Focus Selected"),
                    ];
                    for (key, desc) in camera_shortcuts {
                        ui.painter().text(egui::pos2(left_x + 8.0, y), egui::Align2::LEFT_TOP, key, egui::FontId::proportional(10.0), key_color);
                        ui.painter().text(egui::pos2(right_x, y), egui::Align2::RIGHT_TOP, desc, egui::FontId::proportional(10.0), text_color);
                        y += line_height;
                    }
                    y += 8.0;

                    // === 트랜스폼 섹션 ===
                    ui.painter().text(egui::pos2(left_x, y), egui::Align2::LEFT_TOP, "Transform", egui::FontId::proportional(11.0), section_color);
                    y += line_height;

                    let transform_shortcuts = [
                        ("Q", "Select Mode"),
                        ("W", "Move (Translate)"),
                        ("E", "Rotate"),
                        ("R", "Scale"),
                    ];
                    for (key, desc) in transform_shortcuts {
                        ui.painter().text(egui::pos2(left_x + 8.0, y), egui::Align2::LEFT_TOP, key, egui::FontId::proportional(10.0), key_color);
                        ui.painter().text(egui::pos2(right_x, y), egui::Align2::RIGHT_TOP, desc, egui::FontId::proportional(10.0), text_color);
                        y += line_height;
                    }
                    y += 8.0;

                    // === 뷰 섹션 ===
                    ui.painter().text(egui::pos2(left_x, y), egui::Align2::LEFT_TOP, "View", egui::FontId::proportional(11.0), section_color);
                    y += line_height;

                    let view_shortcuts = [
                        ("G", "Toggle Grid"),
                        ("H", "Toggle Shortcuts"),
                        ("F5", "Play / Stop"),
                    ];
                    for (key, desc) in view_shortcuts {
                        ui.painter().text(egui::pos2(left_x + 8.0, y), egui::Align2::LEFT_TOP, key, egui::FontId::proportional(10.0), key_color);
                        ui.painter().text(egui::pos2(right_x, y), egui::Align2::RIGHT_TOP, desc, egui::FontId::proportional(10.0), text_color);
                        y += line_height;
                    }
                    let _ = y; // 사용됨 표시

                    // 닫기 힌트
                    ui.painter().text(
                        egui::pos2(help_rect.center().x, help_rect.max.y - 20.0),
                        egui::Align2::CENTER_CENTER,
                        "Press H or ? to close",
                        egui::FontId::proportional(9.0),
                        Color32::from_rgb(100, 105, 115),
                    );
                }
            }
        });
    }

    /// 뷰포트가 호버 상태인지 확인
    pub fn is_viewport_hovered(&self) -> bool {
        self.viewport.hovered
    }

    /// 특정 위치가 뷰포트 영역 내인지 확인 (실시간 체크)
    pub fn is_pos_in_viewport(&self, x: f32, y: f32) -> bool {
        if let Some(rect) = self.viewport.rect {
            rect.contains(egui::pos2(x, y))
        } else {
            false
        }
    }

    /// 뷰포트 크기 가져오기
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport.size
    }

    /// 뷰포트 텍스처 ID 설정
    pub fn set_viewport_texture(&mut self, texture_id: TextureId) {
        self.viewport.texture_id = Some(texture_id);
    }

    /// 플레이 모드인지 확인
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// AI 패널 표시 여부 확인
    pub fn is_ai_panel_visible(&self) -> bool {
        self.ai_panel_visible
    }

    /// AI 패널 토글
    pub fn toggle_ai_panel(&mut self) {
        self.ai_panel_visible = !self.ai_panel_visible;
    }

    /// AI 패널 분리 상태 확인
    pub fn is_ai_panel_detached(&self) -> bool {
        self.ai_panel_detached
    }

    /// AI 패널 분리 상태 설정
    pub fn set_ai_panel_detached(&mut self, detached: bool) {
        self.ai_panel_detached = detached;
    }
}

impl Default for DockLayout {
    fn default() -> Self {
        Self::new()
    }
}
