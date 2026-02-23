//! EditorTheme - 중앙집중식 테마 시스템
//!
//! UE의 FEditorStyle / FSlateStyleSet 패턴 참고.
//! 코드에서는 타입 안전한 구조체 접근, JSON으로 사용자 커스텀 테마 지원.

use crate::core::Color;
use serde::{Serialize, Deserialize};

/// 에디터 테마
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTheme {
    pub name: String,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
    pub spacing: ThemeSpacing,
}

/// 테마 색상
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    // ── 기본 배경 ──
    pub window_bg: Color,
    pub panel_bg: Color,
    pub content_bg: Color,
    pub titlebar_bg: Color,
    pub toolbar_bg: Color,

    // ── 탭 바 ──
    pub tab_bar_bg: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_bg: Color,
    pub tab_hover_bg: Color,

    // ── 텍스트 ──
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_bright: Color,

    // ── 아이콘 ──
    pub icon_tint: Color,

    // ── 액센트 (선택/하이라이트) ──
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_preview: Color,

    // ── 위험/닫기 ──
    pub danger: Color,
    pub danger_hover: Color,
    pub danger_bg: Color,

    // ── 보더/구분선 ──
    pub border: Color,
    pub separator: Color,
    pub shadow: Color,

    // ── 스플리터 ──
    pub splitter_bg: Color,
    pub splitter_hover: Color,
    pub splitter_drag: Color,

    // ── 사이드바 ──
    pub sidebar_bg: Color,
    pub sidebar_button_active: Color,
    pub sidebar_button_hover: Color,
    pub sidebar_button_normal: Color,
    pub sidebar_drawer_bg: Color,
    pub sidebar_drawer_header_bg: Color,
    pub sidebar_drawer_header_text: Color,

    // ── 메뉴 ──
    pub menu_bg: Color,
    pub menu_border: Color,
    pub menu_hover: Color,
    pub menu_text: Color,
    pub menu_divider: Color,

    // ── 나침반 ──
    pub compass_line: Color,
    pub compass_hover: Color,
    pub compass_preview: Color,

    // ── 윈도우 컨트롤 버튼 ──
    pub window_button_bg: Color,
    pub window_button_hover: Color,
    pub window_close_hover: Color,
    pub window_button_icon: Color,

    // ── 드래그 프리뷰 ──
    pub drag_preview_bg: Color,
    pub drag_preview_border: Color,
    pub drag_tab_bar_bg: Color,
    pub drag_title_text: Color,

    // ── 도킹 타겟 ──
    pub dock_target_fill: Color,
    pub dock_target_border: Color,

    // ── 메이저 탭 바 ──
    pub major_tab_bar_bg: Color,
    pub major_tab_active_bg: Color,
    pub major_tab_hover_bg: Color,
    pub major_tab_inactive_bg: Color,
    pub major_tab_inactive_text: Color,
    pub major_tab_accent: Color,

    // ── 로고 ──
    pub logo_tint: Color,

    // ── 컨트롤 공통 ──
    pub control_bg: Color,
    pub control_bg_hover: Color,
    pub control_bg_pressed: Color,
    pub control_bg_disabled: Color,
    pub control_border: Color,
    pub focus_border: Color,
    pub selection_bg: Color,

    // ── 스크롤바 ──
    pub scrollbar_track: Color,
    pub scrollbar_thumb: Color,
    pub scrollbar_thumb_hover: Color,

    // ── 상태 색상 ──
    pub success: Color,
    pub warning: Color,

    // ── 팝업/툴팁 ──
    pub popup_bg: Color,
    pub popup_border: Color,
    pub popup_dim: Color,

    // ── 에디터 확장 (serde(default)로 기존 호환) ──
    #[serde(default = "ThemeColors::default_header_bg")]
    pub header_bg: Color,
    #[serde(default = "ThemeColors::default_viewport_bg")]
    pub viewport_bg: Color,
    #[serde(default = "ThemeColors::default_section_header_bg")]
    pub section_header_bg: Color,
    #[serde(default = "ThemeColors::default_vec3_x")]
    pub vec3_x_color: Color,
    #[serde(default = "ThemeColors::default_vec3_y")]
    pub vec3_y_color: Color,
    #[serde(default = "ThemeColors::default_vec3_z")]
    pub vec3_z_color: Color,
    #[serde(default = "ThemeColors::default_vec3_w")]
    pub vec3_w_color: Color,

    // ── UE5.7 기반 확장 색상 ──

    /// 트리뷰/리스트 교차 행 배경 (UE5 SelectHover)
    #[serde(default = "ThemeColors::default_row_stripe_bg")]
    pub row_stripe_bg: Color,
    /// 검색 필드 배경 (UE5 Input=#0F0F0F)
    #[serde(default = "ThemeColors::default_search_bg")]
    pub search_bg: Color,
    /// 툴바 버튼 그룹 배경
    #[serde(default = "ThemeColors::default_toolbar_group_bg")]
    pub toolbar_group_bg: Color,
    /// 호버 반투명 오버레이 (UE5 Hover=#575757)
    #[serde(default = "ThemeColors::default_hover_overlay")]
    pub hover_overlay: Color,

    // ── 에셋 타입 ──
    #[serde(default = "ThemeColors::default_asset_folder")]
    pub asset_folder: Color,
    #[serde(default = "ThemeColors::default_asset_scene")]
    pub asset_scene: Color,
    #[serde(default = "ThemeColors::default_asset_mesh")]
    pub asset_mesh: Color,
    #[serde(default = "ThemeColors::default_asset_texture")]
    pub asset_texture: Color,
    #[serde(default = "ThemeColors::default_asset_material")]
    pub asset_material: Color,
    #[serde(default = "ThemeColors::default_asset_script")]
    pub asset_script: Color,
    #[serde(default = "ThemeColors::default_asset_audio")]
    pub asset_audio: Color,
    #[serde(default = "ThemeColors::default_asset_prefab")]
    pub asset_prefab: Color,
    #[serde(default = "ThemeColors::default_asset_ui_layout")]
    pub asset_ui_layout: Color,
    #[serde(default = "ThemeColors::default_asset_unknown")]
    pub asset_unknown: Color,
}

/// 테마 폰트 크기
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFonts {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
}

/// 테마 간격/크기
///
/// UE5.7 FStyleDefaults / CoreStyleConstants 참조.
/// 모든 패널의 레이아웃 매직넘버를 이 구조체로 통합.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpacing {
    pub titlebar_height: f32,
    pub toolbar_height: f32,
    pub menu_bar_height: f32,
    pub sidebar_width: f32,
    pub sidebar_drawer_width: f32,
    pub menu_item_height: f32,
    pub menu_width: f32,
    pub close_button_size: f32,
    // 컨트롤 공통
    pub control_height: f32,
    pub small_control_height: f32,
    pub button_padding_h: f32,
    pub button_padding_v: f32,
    pub input_padding: f32,
    pub content_padding: f32,
    pub border_width: f32,
    pub border_radius: f32,
    // 에디터 확장 (serde(default)로 기존 호환)
    #[serde(default = "ThemeSpacing::default_doc_tab_height")]
    pub doc_tab_height: f32,
    #[serde(default = "ThemeSpacing::default_panel_header_height")]
    pub panel_header_height: f32,
    #[serde(default = "ThemeSpacing::default_gap")]
    pub gap: f32,
    #[serde(default = "ThemeSpacing::default_pill_radius")]
    pub pill_radius: f32,
    #[serde(default = "ThemeSpacing::default_toolbar_button_radius")]
    pub toolbar_button_radius: f32,

    // ── UE5.7 CoreStyleConstants 기반 확장 ──

    /// 코너 라디우스 계층 (UE5.7 InputFocusRadius = 4.0)
    #[serde(default = "ThemeSpacing::default_corner_radius_small")]
    pub corner_radius_small: f32,
    #[serde(default = "ThemeSpacing::default_corner_radius_medium")]
    pub corner_radius_medium: f32,
    #[serde(default = "ThemeSpacing::default_corner_radius_large")]
    pub corner_radius_large: f32,

    /// 트리뷰 들여쓰기 (Hierarchy)
    #[serde(default = "ThemeSpacing::default_tree_indent")]
    pub tree_indent: f32,
    /// 아이콘 열 폭 (트리뷰/리스트 아이콘 영역)
    #[serde(default = "ThemeSpacing::default_icon_column_width")]
    pub icon_column_width: f32,

    /// 에셋 브라우저 그리드
    #[serde(default = "ThemeSpacing::default_grid_item_size")]
    pub grid_item_size: f32,
    #[serde(default = "ThemeSpacing::default_grid_icon_size")]
    pub grid_icon_size: f32,

    /// Hierarchy 행 높이 (control_height와 분리 — HTML ref 22px)
    #[serde(default = "ThemeSpacing::default_hierarchy_row_height")]
    pub hierarchy_row_height: f32,

    /// 인스펙터 레이블 폭
    #[serde(default = "ThemeSpacing::default_inspector_label_width")]
    pub inspector_label_width: f32,
    /// Vec3 축 색상 인디케이터 폭
    #[serde(default = "ThemeSpacing::default_vec3_indicator_width")]
    pub vec3_indicator_width: f32,

    /// 사이드바 버튼/드로어 헤더 크기
    #[serde(default = "ThemeSpacing::default_sidebar_button_size")]
    pub sidebar_button_size: f32,
    #[serde(default = "ThemeSpacing::default_sidebar_drawer_header_height")]
    pub sidebar_drawer_header_height: f32,

    /// 툴바 버튼 기본 폭 / 간격
    #[serde(default = "ThemeSpacing::default_toolbar_button_width")]
    pub toolbar_button_width: f32,
    #[serde(default = "ThemeSpacing::default_toolbar_small_button_width")]
    pub toolbar_small_button_width: f32,
    #[serde(default = "ThemeSpacing::default_toolbar_button_gap")]
    pub toolbar_button_gap: f32,
    #[serde(default = "ThemeSpacing::default_toolbar_group_gap")]
    pub toolbar_group_gap: f32,
    #[serde(default = "ThemeSpacing::default_separator_padding")]
    pub separator_padding: f32,

    // ── 탭 스택 (도킹 탭 바) ──
    #[serde(default = "ThemeSpacing::default_tab_bar_height")]
    pub tab_bar_height: f32,
    #[serde(default = "ThemeSpacing::default_tab_min_width")]
    pub tab_min_width: f32,
    #[serde(default = "ThemeSpacing::default_tab_max_width")]
    pub tab_max_width: f32,
    #[serde(default = "ThemeSpacing::default_tab_spacing")]
    pub tab_spacing: f32,
    #[serde(default = "ThemeSpacing::default_tab_h_padding")]
    pub tab_h_padding: f32,
    #[serde(default = "ThemeSpacing::default_tab_v_padding")]
    pub tab_v_padding: f32,
    #[serde(default = "ThemeSpacing::default_tab_inactive_extra_pad")]
    pub tab_inactive_extra_pad: f32,
    #[serde(default = "ThemeSpacing::default_tab_icon_size")]
    pub tab_icon_size: f32,
    #[serde(default = "ThemeSpacing::default_tab_icon_margin")]
    pub tab_icon_margin: f32,
    #[serde(default = "ThemeSpacing::default_tab_close_size")]
    pub tab_close_size: f32,
    #[serde(default = "ThemeSpacing::default_tab_close_margin")]
    pub tab_close_margin: f32,

    // ── 메이저 탭 바 ──
    #[serde(default = "ThemeSpacing::default_major_tab_height")]
    pub major_tab_height: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_min_width")]
    pub major_tab_min_width: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_max_width")]
    pub major_tab_max_width: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_left_pad")]
    pub major_tab_left_pad: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_right_pad")]
    pub major_tab_right_pad: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_spacing")]
    pub major_tab_spacing: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_v_padding")]
    pub major_tab_v_padding: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_icon_size")]
    pub major_tab_icon_size: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_icon_margin")]
    pub major_tab_icon_margin: f32,
    #[serde(default = "ThemeSpacing::default_major_tab_close_size")]
    pub major_tab_close_size: f32,

    // ── 탭 시각 피드백 ──
    /// 외부 드래그 고스트 탭 불투명도
    #[serde(default = "ThemeSpacing::default_tab_ghost_opacity")]
    pub tab_ghost_opacity: f32,
    /// 로컬 리오더 드래그 탭 불투명도
    #[serde(default = "ThemeSpacing::default_tab_drag_opacity")]
    pub tab_drag_opacity: f32,
    /// 알림 플래시 블렌드 강도
    #[serde(default = "ThemeSpacing::default_tab_flash_blend")]
    pub tab_flash_blend: f32,
    /// 비활성 탭 아이콘 불투명도
    #[serde(default = "ThemeSpacing::default_tab_inactive_icon_opacity")]
    pub tab_inactive_icon_opacity: f32,
    /// 탭 구분선 높이 비율
    #[serde(default = "ThemeSpacing::default_tab_separator_height_ratio")]
    pub tab_separator_height_ratio: f32,

    // ── 탭 인터랙션 ──
    /// 탭웰 표시/숨김 애니메이션 속도
    #[serde(default = "ThemeSpacing::default_tab_well_anim_speed")]
    pub tab_well_anim_speed: f32,
    /// 드래그 호버 탭 활성화 지연(초)
    #[serde(default = "ThemeSpacing::default_drag_hover_activation_delay")]
    pub drag_hover_activation_delay: f32,
    /// 로컬 리오더 시작 임계값(px)
    #[serde(default = "ThemeSpacing::default_local_drag_threshold")]
    pub local_drag_threshold: f32,
    /// 크로스윈도우 드래그 탈출 임계값(px)
    #[serde(default = "ThemeSpacing::default_drag_escape_threshold")]
    pub drag_escape_threshold: f32,
    /// 탭웰 content-right 최소 폭
    #[serde(default = "ThemeSpacing::default_tab_well_min_slot_width")]
    pub tab_well_min_slot_width: f32,

    // ── 스플리터 ──
    /// 분할 자식 최소 크기(px)
    #[serde(default = "ThemeSpacing::default_splitter_min_child_size")]
    pub splitter_min_child_size: f32,
    /// 분할 비율 상한
    #[serde(default = "ThemeSpacing::default_splitter_max_ratio")]
    pub splitter_max_ratio: f32,

    // ── 윈도우/플로팅 ──
    /// 리사이즈 핸들 두께(px)
    #[serde(default = "ThemeSpacing::default_window_resize_border")]
    pub window_resize_border: f32,
    /// 데코레이터 윈도우 불투명도
    #[serde(default = "ThemeSpacing::default_float_window_opacity")]
    pub float_window_opacity: f32,
    /// 플로팅 윈도우 스폰 오프셋 X
    #[serde(default = "ThemeSpacing::default_float_spawn_offset_x")]
    pub float_spawn_offset_x: f32,
    /// 플로팅 윈도우 스폰 오프셋 Y
    #[serde(default = "ThemeSpacing::default_float_spawn_offset_y")]
    pub float_spawn_offset_y: f32,
    /// 기본 플로팅 윈도우 크기
    #[serde(default = "ThemeSpacing::default_default_float_window_size")]
    pub default_float_window_size: f32,
}

// ── Default impls ──

impl Default for EditorTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl EditorTheme {
    /// 빌트인 다크 테마 (현재 하드코딩 값)
    pub fn dark() -> Self {
        Self {
            name: "Dark".into(),
            colors: ThemeColors::dark(),
            fonts: ThemeFonts::default(),
            spacing: ThemeSpacing::default(),
        }
    }

    // resolve_color() — 삭제됨 (Phase 4-1: SlateColor가 FromTheme(fn) 방식으로 전환)
    // 이제 SlateColor::FromTheme(|tc| tc.text_primary) 처럼 컴파일 타임에 안전하게 참조.

    /// JSON 직렬화
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// JSON 역직렬화
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl ThemeColors {
    // ── serde default helpers (sRGB→linear 변환 포함) ──
    fn default_header_bg() -> Color { Color::rgba(0.184, 0.184, 0.184, 1.0).to_linear() }
    fn default_viewport_bg() -> Color { Color::rgba(0.059, 0.059, 0.059, 1.0).to_linear() }
    fn default_section_header_bg() -> Color { Color::rgba(0.184, 0.184, 0.184, 1.0).to_linear() }
    fn default_vec3_x() -> Color { Color::rgba(1.0, 0.271, 0.227, 1.0).to_linear() }
    fn default_vec3_y() -> Color { Color::rgba(0.196, 0.843, 0.294, 1.0).to_linear() }
    fn default_vec3_z() -> Color { Color::rgba(0.039, 0.518, 1.0, 1.0).to_linear() }
    fn default_vec3_w() -> Color { Color::rgba(0.7, 0.5, 0.2, 1.0).to_linear() }
    fn default_row_stripe_bg() -> Color { Color::rgba(1.0, 1.0, 1.0, 0.02) }  // 알파만 사용, RGB=1.0→변환 불필요
    fn default_search_bg() -> Color { Color::rgba(0.102, 0.102, 0.102, 1.0).to_linear() }
    fn default_toolbar_group_bg() -> Color { Color::rgba(0.141, 0.141, 0.145, 1.0).to_linear() }
    fn default_hover_overlay() -> Color { Color::rgba(1.0, 1.0, 1.0, 0.06) }  // RGB=1.0→변환 불필요

    // ── 에셋 타입 serde default helpers ──
    fn default_asset_folder() -> Color { Color::rgba(0.714, 0.561, 0.333, 1.0).to_linear() }
    fn default_asset_scene() -> Color { Color::rgba(0.3, 0.8, 0.4, 1.0).to_linear() }
    fn default_asset_mesh() -> Color { Color::rgba(0.4, 0.6, 0.9, 1.0).to_linear() }
    fn default_asset_texture() -> Color { Color::rgba(0.9, 0.5, 0.3, 1.0).to_linear() }
    fn default_asset_material() -> Color { Color::rgba(0.8, 0.3, 0.8, 1.0).to_linear() }
    fn default_asset_script() -> Color { Color::rgba(0.5, 0.9, 0.5, 1.0).to_linear() }
    fn default_asset_audio() -> Color { Color::rgba(0.3, 0.9, 0.9, 1.0).to_linear() }
    fn default_asset_prefab() -> Color { Color::rgba(0.6, 0.4, 0.9, 1.0).to_linear() }
    fn default_asset_ui_layout() -> Color { Color::rgba(0.9, 0.6, 0.8, 1.0).to_linear() }
    fn default_asset_unknown() -> Color { Color::rgba(0.5, 0.5, 0.5, 1.0).to_linear() }

    /// 모든 색상 필드를 sRGB → linear 변환
    ///
    /// Bgra8UnormSrgb 서피스에서 GPU가 자동으로 linear→sRGB 인코딩하므로,
    /// 테마 색상을 linear space로 저장해야 화면에 의도한 sRGB 색상이 표시된다.
    pub fn srgb_to_linear(mut self) -> Self {
        macro_rules! lin {
            ($($field:ident),* $(,)?) => {
                $(self.$field = self.$field.to_linear();)*
            };
        }
        lin!(
            // 기본 배경
            window_bg, panel_bg, content_bg, titlebar_bg, toolbar_bg,
            // 탭 바
            tab_bar_bg, tab_active_bg, tab_inactive_bg, tab_hover_bg,
            // 텍스트
            text_primary, text_secondary, text_muted, text_bright,
            // 아이콘
            icon_tint,
            // 액센트
            accent, accent_hover, accent_preview,
            // 위험/닫기
            danger, danger_hover, danger_bg,
            // 보더/구분선
            border, separator, shadow,
            // 스플리터
            splitter_bg, splitter_hover, splitter_drag,
            // 사이드바
            sidebar_bg, sidebar_button_active, sidebar_button_hover,
            sidebar_button_normal, sidebar_drawer_bg, sidebar_drawer_header_bg,
            sidebar_drawer_header_text,
            // 메뉴
            menu_bg, menu_border, menu_hover, menu_text, menu_divider,
            // 나침반
            compass_line, compass_hover, compass_preview,
            // 윈도우 컨트롤
            window_button_bg, window_button_hover, window_close_hover, window_button_icon,
            // 드래그 프리뷰
            drag_preview_bg, drag_preview_border, drag_tab_bar_bg, drag_title_text,
            // 도킹 타겟
            dock_target_fill, dock_target_border,
            // 메이저 탭 바
            major_tab_bar_bg, major_tab_active_bg, major_tab_hover_bg,
            major_tab_inactive_bg, major_tab_inactive_text, major_tab_accent,
            // 로고
            logo_tint,
            // 컨트롤 공통
            control_bg, control_bg_hover, control_bg_pressed, control_bg_disabled,
            control_border, focus_border, selection_bg,
            // 스크롤바
            scrollbar_track, scrollbar_thumb, scrollbar_thumb_hover,
            // 상태 색상
            success, warning,
            // 팝업/툴팁
            popup_bg, popup_border, popup_dim,
            // 에디터 확장
            header_bg, viewport_bg, section_header_bg,
            vec3_x_color, vec3_y_color, vec3_z_color, vec3_w_color,
            // UE5.7 확장
            row_stripe_bg, search_bg, toolbar_group_bg, hover_overlay,
            // 에셋 타입
            asset_folder, asset_scene, asset_mesh, asset_texture,
            asset_material, asset_script, asset_audio, asset_prefab,
            asset_ui_layout, asset_unknown,
        );
        self
    }

    /// HTML 레퍼런스 + UE5.7 Starship 기반 다크 테마
    ///
    /// 모든 배경색은 중성 회색 (R=G=B), 블루 틴트 없음.
    /// sRGB 값을 직접 사용 (Rgba8UnormSrgb 텍스처 포맷 자동 변환).
    ///
    /// UE5.7 EStyleColor 참조:
    ///   Background=#151515, Panel=#242424, Header=#2F2F2F
    ///   Recessed=#1A1A1A, Dropdown=#383838
    pub fn dark() -> Self {
        let colors = Self {
            // ── 기본 배경 ── UE5.7: Background=#151515, Panel=#242424
            window_bg:   Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515 (UE5.7 Background)
            panel_bg:    Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424 (UE5.7 Panel)
            content_bg:  Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            titlebar_bg: Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515 (UE5.7 Title)
            toolbar_bg:  Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424

            // ── 탭 바 ── UE5.7: Dropdown=#383838
            tab_bar_bg:     Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            tab_active_bg:  Color::rgba(0.220, 0.220, 0.220, 1.0),  // #383838
            tab_inactive_bg: Color::rgba(0.0, 0.0, 0.0, 0.0),       // transparent
            tab_hover_bg:   Color::rgba(0.220, 0.220, 0.220, 0.6),  // #383838 @ 60%

            // ── 텍스트 ── UE5.7: Foreground=#C0C0C0, ForegroundHover=#FFFFFF
            text_primary:   Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0 (UE5.7 Foreground)
            text_secondary: Color::rgba(0.565, 0.565, 0.565, 1.0),  // #909090
            text_muted:     Color::rgba(0.400, 0.400, 0.400, 1.0),  // #666666
            text_bright:    Color::rgba(1.0, 1.0, 1.0, 1.0),        // #FFFFFF

            // ── 아이콘 ──
            icon_tint: Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0

            // ── 액센트 ── UE5.7: Primary=#0070E0, PrimaryHover=#0E86FF
            accent:         Color::rgba(0.0, 0.439, 0.878, 1.0),    // #0070E0 (UE5.7 Primary)
            accent_hover:   Color::rgba(0.055, 0.525, 1.0, 1.0),    // #0E86FF (UE5.7 PrimaryHover)
            accent_preview: Color::rgba(0.0, 0.439, 0.878, 0.25),   // #0070E0 @ 25%

            // ── 위험/닫기 ── UE5.7: Error=#EF3535
            danger:       Color::rgba(0.937, 0.208, 0.208, 1.0),   // #EF3535 (UE5.7 Error)
            danger_hover: Color::rgba(1.0, 0.35, 0.35, 1.0),
            danger_bg:    Color::rgba(0.937, 0.208, 0.208, 0.6),

            // ── 보더/구분선 ── HTML ref: border=#333333
            border:    Color::rgba(0.200, 0.200, 0.200, 1.0),  // #333333
            separator: Color::rgba(0.300, 0.300, 0.300, 1.0),  // #4D4D4D
            shadow:    Color::rgba(0.0, 0.0, 0.0, 0.4),

            // ── 스플리터 ──
            splitter_bg:    Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            splitter_hover: Color::rgba(0.0, 0.439, 0.878, 0.5),    // accent @ 50%
            splitter_drag:  Color::rgba(0.0, 0.439, 0.878, 0.8),    // accent @ 80%

            // ── 사이드바 ──
            sidebar_bg:                 Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            sidebar_button_active:      Color::rgba(0.0, 0.439, 0.878, 0.8),    // accent
            sidebar_button_hover:       Color::rgba(0.200, 0.200, 0.200, 1.0),  // #333333
            sidebar_button_normal:      Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            sidebar_drawer_bg:          Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            sidebar_drawer_header_bg:   Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            sidebar_drawer_header_text: Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0

            // ── 메뉴 ── UE5.7: Panel=#242424, DropdownOutline=#4C4C4C
            menu_bg:      Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            menu_border:  Color::rgba(0.298, 0.298, 0.298, 1.0),  // #4C4C4C (UE5.7 DropdownOutline)
            menu_hover:   Color::rgba(0.0, 0.439, 0.878, 0.6),    // accent @ 60%
            menu_text:    Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0
            menu_divider: Color::rgba(1.0, 1.0, 1.0, 0.15),       // White15

            // ── 나침반 ──
            compass_line:    Color::rgba(0.753, 0.753, 0.753, 0.8),
            compass_hover:   Color::rgba(1.0, 0.35, 0.0, 1.0),
            compass_preview: Color::rgba(1.0, 0.75, 0.5, 0.35),

            // ── 윈도우 컨트롤 ──
            window_button_bg:    Color::rgba(0.0, 0.0, 0.0, 0.0),
            window_button_hover: Color::rgba(0.300, 0.300, 0.300, 1.0),  // #4D4D4D
            window_close_hover:  Color::rgba(0.937, 0.208, 0.208, 1.0),   // #EF3535
            window_button_icon:  Color::rgba(0.753, 0.753, 0.753, 1.0), // #C0C0C0

            // ── 드래그 프리뷰 ──
            drag_preview_bg:     Color::rgba(0.0, 0.439, 0.878, 0.9),
            drag_preview_border: Color::rgba(0.055, 0.525, 1.0, 0.9),
            drag_tab_bar_bg:     Color::rgba(0.082, 0.082, 0.082, 0.8),  // #151515
            drag_title_text:     Color::rgba(1.0, 1.0, 1.0, 0.9),

            // ── 도킹 타겟 ──
            dock_target_fill:   Color::rgba(0.0, 0.439, 0.878, 0.25),
            dock_target_border: Color::rgba(0.055, 0.525, 1.0, 0.7),

            // ── 메이저 탭 바 ── UE5.7: Background=#151515, Dropdown=#383838
            major_tab_bar_bg:      Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            major_tab_active_bg:   Color::rgba(0.220, 0.220, 0.220, 1.0),  // #383838
            major_tab_hover_bg:    Color::rgba(0.220, 0.220, 0.220, 0.6),  // #383838 @ 60%
            major_tab_inactive_bg: Color::rgba(0.0, 0.0, 0.0, 0.0),       // transparent
            major_tab_inactive_text: Color::rgba(0.753, 0.753, 0.753, 1.0), // #C0C0C0
            major_tab_accent:      Color::rgba(0.0, 0.439, 0.878, 1.0),    // #0070E0

            // ── 로고 ──
            logo_tint: Color::rgba(1.0, 1.0, 1.0, 0.08),

            // ── 컨트롤 공통 ── UE5.7: InputOutline=#383838
            control_bg:          Color::rgba(0.141, 0.141, 0.145, 1.0),  // #242425
            control_bg_hover:    Color::rgba(0.180, 0.180, 0.180, 1.0),  // #2E2E2E
            control_bg_pressed:  Color::rgba(0.102, 0.102, 0.102, 1.0),  // #1A1A1A (UE5.7 Recessed)
            control_bg_disabled: Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            control_border:      Color::rgba(0.220, 0.220, 0.220, 1.0),  // #383838 (UE5.7 InputOutline)
            focus_border:        Color::rgba(0.0, 0.439, 0.878, 1.0),    // #0070E0
            selection_bg:        Color::rgba(0.0, 0.439, 0.878, 1.0),    // #0070E0

            // ── 스크롤바 ──
            scrollbar_track:      Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            scrollbar_thumb:      Color::rgba(0.300, 0.300, 0.300, 1.0),  // #4D4D4D
            scrollbar_thumb_hover: Color::rgba(0.400, 0.400, 0.400, 1.0), // #666666

            // ── 상태 색상 ── UE5.7: Warning=#FFB800, Success=#1FE44B
            success: Color::rgba(0.122, 0.894, 0.294, 1.0),  // #1FE44B (UE5.7 Success)
            warning: Color::rgba(1.0, 0.722, 0.0, 1.0),      // #FFB800 (UE5.7 Warning)

            // ── 팝업/툴팁 ──
            popup_bg:     Color::rgba(0.141, 0.141, 0.141, 0.98),  // #242424 @98%
            popup_border: Color::rgba(0.298, 0.298, 0.298, 1.0),   // #4C4C4C (UE5.7 DropdownOutline)
            popup_dim:    Color::rgba(0.0, 0.0, 0.0, 0.5),         // modal dim overlay

            // ── 에디터 확장 ──
            header_bg:         Color::rgba(0.184, 0.184, 0.184, 1.0),  // #2F2F2F (UE5.7 Header)
            viewport_bg:       Color::rgba(0.059, 0.059, 0.059, 1.0),  // #0F0F0F
            section_header_bg: Color::rgba(0.184, 0.184, 0.184, 1.0),  // #2F2F2F (UE5.7 Header)
            vec3_x_color:      Color::rgba(1.0, 0.271, 0.227, 1.0),    // #FF453A
            vec3_y_color:      Color::rgba(0.196, 0.843, 0.294, 1.0),  // #32D74B
            vec3_z_color:      Color::rgba(0.039, 0.518, 1.0, 1.0),    // #0A84FF
            vec3_w_color:      Color::rgba(0.7, 0.5, 0.2, 1.0),      // orange W axis

            // ── UE5.7 기반 확장 ──
            row_stripe_bg:     Color::rgba(1.0, 1.0, 1.0, 0.02),
            search_bg:         Color::rgba(0.102, 0.102, 0.102, 1.0),  // #1A1A1A (UE5.7 Recessed)
            toolbar_group_bg:  Color::rgba(0.141, 0.141, 0.145, 1.0),  // #242425
            hover_overlay:     Color::rgba(1.0, 1.0, 1.0, 0.06),

            // ── 에셋 타입 ──
            asset_folder:    Color::rgba(0.714, 0.561, 0.333, 1.0),  // #B68F55
            asset_scene:     Color::rgba(0.3, 0.8, 0.4, 1.0),
            asset_mesh:      Color::rgba(0.4, 0.6, 0.9, 1.0),
            asset_texture:   Color::rgba(0.9, 0.5, 0.3, 1.0),
            asset_material:  Color::rgba(0.8, 0.3, 0.8, 1.0),
            asset_script:    Color::rgba(0.5, 0.9, 0.5, 1.0),
            asset_audio:     Color::rgba(0.3, 0.9, 0.9, 1.0),
            asset_prefab:    Color::rgba(0.6, 0.4, 0.9, 1.0),
            asset_ui_layout: Color::rgba(0.9, 0.6, 0.8, 1.0),
            asset_unknown:   Color::rgba(0.5, 0.5, 0.5, 1.0),
        };
        // sRGB → linear 변환: Bgra8UnormSrgb 서피스에서 GPU가 linear→sRGB 자동 인코딩
        let colors = colors.srgb_to_linear();
        log::info!(
            "[ThemeColors::dark] sRGB→linear 적용됨 — window_bg=({:.4},{:.4},{:.4}) text_primary=({:.4},{:.4},{:.4}) accent=({:.4},{:.4},{:.4})",
            colors.window_bg.r, colors.window_bg.g, colors.window_bg.b,
            colors.text_primary.r, colors.text_primary.g, colors.text_primary.b,
            colors.accent.r, colors.accent.g, colors.accent.b,
        );
        colors
    }
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            small: 11.0,
            medium: 14.0,
            large: 16.0,
        }
    }
}

impl ThemeSpacing {
    fn default_doc_tab_height() -> f32 { 40.0 }
    fn default_panel_header_height() -> f32 { 32.0 }
    fn default_gap() -> f32 { 4.0 }
    fn default_pill_radius() -> f32 { 20.0 }  // tab height / 2
    fn default_toolbar_button_radius() -> f32 { 6.0 }

    // UE5.7 CoreStyleConstants 기반
    fn default_corner_radius_small() -> f32 { 4.0 }      // 검색바 (HTML ref radius:4)
    fn default_corner_radius_medium() -> f32 { 4.0 }     // 버튼, 입력 (UE5 InputFocusRadius)
    fn default_corner_radius_large() -> f32 { 8.0 }      // 패널, 카드
    fn default_hierarchy_row_height() -> f32 { 22.0 }   // HTML ref rowH=22
    fn default_tree_indent() -> f32 { 14.0 }              // HTML ref depth*14
    fn default_icon_column_width() -> f32 { 18.0 }
    fn default_grid_item_size() -> f32 { 80.0 }
    fn default_grid_icon_size() -> f32 { 32.0 }
    fn default_inspector_label_width() -> f32 { 100.0 }
    fn default_vec3_indicator_width() -> f32 { 8.0 }
    fn default_sidebar_button_size() -> f32 { 28.0 }
    fn default_sidebar_drawer_header_height() -> f32 { 28.0 }
    fn default_toolbar_button_width() -> f32 { 50.0 }
    fn default_toolbar_small_button_width() -> f32 { 30.0 }
    fn default_toolbar_button_gap() -> f32 { 4.0 }
    fn default_toolbar_group_gap() -> f32 { 12.0 }
    fn default_separator_padding() -> f32 { 6.0 }

    // ── 탭 스택 (도킹 탭 바) 기본값 ──
    fn default_tab_bar_height() -> f32 { 25.0 }
    fn default_tab_min_width() -> f32 { 60.0 }
    fn default_tab_max_width() -> f32 { 160.0 }
    fn default_tab_spacing() -> f32 { 4.0 }
    fn default_tab_h_padding() -> f32 { 8.0 }
    fn default_tab_v_padding() -> f32 { 3.0 }
    fn default_tab_inactive_extra_pad() -> f32 { 2.0 }
    fn default_tab_icon_size() -> f32 { 16.0 }
    fn default_tab_icon_margin() -> f32 { 5.0 }
    fn default_tab_close_size() -> f32 { 16.0 }
    fn default_tab_close_margin() -> f32 { 10.0 }

    // ── 메이저 탭 바 기본값 ──
    fn default_major_tab_height() -> f32 { 40.0 }
    fn default_major_tab_min_width() -> f32 { 100.0 }
    fn default_major_tab_max_width() -> f32 { 210.0 }
    fn default_major_tab_left_pad() -> f32 { 4.0 }
    fn default_major_tab_right_pad() -> f32 { 10.0 }
    fn default_major_tab_spacing() -> f32 { 2.0 }
    fn default_major_tab_v_padding() -> f32 { 4.0 }
    fn default_major_tab_icon_size() -> f32 { 16.0 }
    fn default_major_tab_icon_margin() -> f32 { 5.0 }
    fn default_major_tab_close_size() -> f32 { 16.0 }

    // ── 탭 시각 피드백 기본값 ──
    fn default_tab_ghost_opacity() -> f32 { 0.4 }
    fn default_tab_drag_opacity() -> f32 { 0.85 }
    fn default_tab_flash_blend() -> f32 { 0.4 }
    fn default_tab_inactive_icon_opacity() -> f32 { 0.7 }
    fn default_tab_separator_height_ratio() -> f32 { 0.65 }

    // ── 탭 인터랙션 기본값 ──
    fn default_tab_well_anim_speed() -> f32 { 8.0 }
    fn default_drag_hover_activation_delay() -> f32 { 0.75 }
    fn default_local_drag_threshold() -> f32 { 5.0 }
    fn default_drag_escape_threshold() -> f32 { 20.0 }
    fn default_tab_well_min_slot_width() -> f32 { 20.0 }

    // ── 스플리터 기본값 ──
    fn default_splitter_min_child_size() -> f32 { 100.0 }
    fn default_splitter_max_ratio() -> f32 { 0.5 }

    // ── 윈도우/플로팅 기본값 ──
    fn default_window_resize_border() -> f32 { 5.0 }
    fn default_float_window_opacity() -> f32 { 0.45 }
    fn default_float_spawn_offset_x() -> f32 { 50.0 }
    fn default_float_spawn_offset_y() -> f32 { 15.0 }
    fn default_default_float_window_size() -> f32 { 400.0 }

    /// ThemeSpacing → TitleBarStyle 파생 (이중 정의 방지)
    ///
    /// `TitleBarStyle::from_theme(&spacing)`과 동일한 결과.
    pub fn to_title_bar_style(&self) -> crate::docking::TitleBarStyle {
        crate::docking::TitleBarStyle::from_theme(self)
    }
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            titlebar_height: 38.0,
            toolbar_height: 48.0,
            menu_bar_height: 38.0,
            sidebar_width: 32.0,
            sidebar_drawer_width: 280.0,
            menu_item_height: 24.0,
            menu_width: 160.0,
            close_button_size: 14.0,
            control_height: 24.0,
            small_control_height: 20.0,
            button_padding_h: 12.0,
            button_padding_v: 4.0,
            input_padding: 6.0,
            content_padding: 8.0,
            border_width: 1.0,
            border_radius: 4.0,  // UE5.7 InputFocusRadius = 4.0
            doc_tab_height: 40.0,
            panel_header_height: 32.0,
            gap: 4.0,
            pill_radius: 20.0,
            toolbar_button_radius: 6.0,
            // HTML ref 기반 확장
            corner_radius_small: 4.0,   // HTML ref search bar radius:4
            corner_radius_medium: 4.0,
            corner_radius_large: 8.0,
            hierarchy_row_height: 22.0, // HTML ref rowH=22
            tree_indent: 14.0,          // HTML ref depth*14
            icon_column_width: 18.0,
            grid_item_size: 80.0,
            grid_icon_size: 32.0,
            inspector_label_width: 100.0,
            vec3_indicator_width: 8.0,
            sidebar_button_size: 28.0,
            sidebar_drawer_header_height: 28.0,
            toolbar_button_width: 50.0,
            toolbar_small_button_width: 30.0,
            toolbar_button_gap: 4.0,
            toolbar_group_gap: 12.0,
            separator_padding: 6.0,
            // 탭 스택 (도킹 탭 바)
            tab_bar_height: 25.0,
            tab_min_width: 60.0,
            tab_max_width: 160.0,
            tab_spacing: 4.0,
            tab_h_padding: 8.0,
            tab_v_padding: 3.0,
            tab_inactive_extra_pad: 2.0,
            tab_icon_size: 16.0,
            tab_icon_margin: 5.0,
            tab_close_size: 16.0,
            tab_close_margin: 10.0,
            // 메이저 탭 바
            major_tab_height: 40.0,
            major_tab_min_width: 100.0,
            major_tab_max_width: 210.0,
            major_tab_left_pad: 4.0,
            major_tab_right_pad: 10.0,
            major_tab_spacing: 2.0,
            major_tab_v_padding: 4.0,
            major_tab_icon_size: 16.0,
            major_tab_icon_margin: 5.0,
            major_tab_close_size: 16.0,
            // 탭 시각 피드백
            tab_ghost_opacity: 0.4,
            tab_drag_opacity: 0.85,
            tab_flash_blend: 0.4,
            tab_inactive_icon_opacity: 0.7,
            tab_separator_height_ratio: 0.65,
            // 탭 인터랙션
            tab_well_anim_speed: 8.0,
            drag_hover_activation_delay: 0.75,
            local_drag_threshold: 5.0,
            drag_escape_threshold: 20.0,
            tab_well_min_slot_width: 20.0,
            // 스플리터
            splitter_min_child_size: 100.0,
            splitter_max_ratio: 0.5,
            // 윈도우/플로팅
            window_resize_border: 5.0,
            float_window_opacity: 0.45,
            float_spawn_offset_x: 50.0,
            float_spawn_offset_y: 15.0,
            default_float_window_size: 400.0,
        }
    }
}

// StyleSet / StyleManager — 삭제됨 (Phase 4-3: 미사용 코드 정리)
// 모던 접근법: EditorTheme 직접 사용 (ThemeColors, ThemeFonts, ThemeSpacing)
