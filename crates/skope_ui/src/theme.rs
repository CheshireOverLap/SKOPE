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
}

/// 테마 폰트 크기
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFonts {
    pub small: f32,
    pub normal: f32,
    pub medium: f32,
    pub large: f32,
}

/// 테마 간격/크기
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
    pub fn dark() -> Self {
        Self {
            // 기본 배경
            window_bg: Color::rgba(0.10, 0.10, 0.12, 1.0),
            panel_bg: Color::rgba(0.12, 0.12, 0.14, 1.0),
            content_bg: Color::rgba(0.14, 0.14, 0.16, 1.0),
            titlebar_bg: Color::rgba(0.15, 0.15, 0.18, 1.0),
            toolbar_bg: Color::rgba(0.15, 0.15, 0.17, 1.0),

            // 탭 바
            tab_bar_bg: Color::rgba(0.18, 0.18, 0.20, 1.0),
            tab_active_bg: Color::rgba(0.25, 0.25, 0.28, 1.0),
            tab_inactive_bg: Color::rgba(0.15, 0.15, 0.17, 1.0),
            tab_hover_bg: Color::rgba(0.20, 0.20, 0.24, 1.0),

            // 텍스트
            text_primary: Color::WHITE,
            text_secondary: Color::rgba(0.7, 0.7, 0.7, 1.0),
            text_muted: Color::rgba(0.6, 0.6, 0.6, 1.0),
            text_bright: Color::rgba(0.9, 0.9, 0.9, 1.0),

            // 아이콘
            icon_tint: Color::WHITE,

            // 액센트
            accent: Color::rgba(0.25, 0.56, 0.87, 1.0),
            accent_hover: Color::rgba(0.12, 0.44, 0.93, 0.6),
            accent_preview: Color::rgba(0.2, 0.4, 0.8, 0.25),

            // 위험/닫기
            danger: Color::rgba(0.8, 0.2, 0.2, 1.0),
            danger_hover: Color::rgba(0.9, 0.2, 0.2, 1.0),
            danger_bg: Color::rgba(0.6, 0.2, 0.2, 0.6),

            // 보더/구분선
            border: Color::rgba(0.3, 0.3, 0.35, 1.0),
            separator: Color::rgba(0.3, 0.3, 0.3, 0.5),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.3),

            // 스플리터
            splitter_bg: Color::rgba(0.3, 0.3, 0.35, 1.0),
            splitter_hover: Color::rgba(0.4, 0.6, 0.9, 0.5),
            splitter_drag: Color::rgba(0.3, 0.5, 0.8, 0.8),

            // 사이드바
            sidebar_bg: Color::rgba(0.10, 0.10, 0.12, 1.0),
            sidebar_button_active: Color::rgba(0.20, 0.40, 0.70, 0.8),
            sidebar_button_hover: Color::rgba(0.22, 0.22, 0.26, 1.0),
            sidebar_button_normal: Color::rgba(0.14, 0.14, 0.16, 1.0),
            sidebar_drawer_bg: Color::rgba(0.14, 0.14, 0.16, 1.0),
            sidebar_drawer_header_bg: Color::rgba(0.18, 0.18, 0.20, 1.0),
            sidebar_drawer_header_text: Color::rgba(0.9, 0.9, 0.9, 1.0),

            // 메뉴
            menu_bg: Color::rgba(0.18, 0.18, 0.18, 1.0),
            menu_border: Color::rgba(0.3, 0.3, 0.3, 1.0),
            menu_hover: Color::rgba(0.12, 0.44, 0.93, 0.6),
            menu_text: Color::rgba(0.9, 0.9, 0.9, 1.0),
            menu_divider: Color::rgba(0.3, 0.3, 0.3, 0.5),

            // 나침반
            compass_line: Color::rgba(0.8, 0.8, 0.8, 0.8),
            compass_hover: Color::rgba(0.9, 0.5, 0.1, 0.6),
            compass_preview: Color::rgba(0.9, 0.5, 0.1, 0.25),

            // 윈도우 컨트롤
            window_button_bg: Color::rgba(0.18, 0.18, 0.2, 1.0),
            window_button_hover: Color::rgba(0.3, 0.3, 0.32, 1.0),
            window_close_hover: Color::rgba(0.9, 0.2, 0.2, 1.0),
            window_button_icon: Color::rgba(0.8, 0.8, 0.8, 1.0),

            // 드래그 프리뷰
            drag_preview_bg: Color::rgba(0.2, 0.4, 0.7, 0.9),
            drag_preview_border: Color::rgba(0.3, 0.5, 0.8, 0.9),
            drag_tab_bar_bg: Color::rgba(0.12, 0.12, 0.15, 0.8),
            drag_title_text: Color::rgba(1.0, 1.0, 1.0, 0.9),

            // 도킹 타겟
            dock_target_fill: Color::rgba(0.2, 0.4, 0.8, 0.25),
            dock_target_border: Color::rgba(0.3, 0.5, 1.0, 0.7),

            // 메이저 탭 바
            major_tab_bar_bg: Color::rgba(0.10, 0.10, 0.12, 1.0),
            major_tab_active_bg: Color::rgba(0.20, 0.20, 0.24, 1.0),
            major_tab_hover_bg: Color::rgba(0.16, 0.16, 0.20, 1.0),
            major_tab_inactive_bg: Color::rgba(0.10, 0.10, 0.12, 0.0),
            major_tab_inactive_text: Color::rgba(0.6, 0.6, 0.6, 1.0),
            major_tab_accent: Color::rgba(0.25, 0.56, 0.87, 1.0),
        }
    }
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            small: 11.0,
            normal: 12.0,
            medium: 13.0,
            large: 14.0,
        }
    }
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            titlebar_height: 28.0,
            toolbar_height: 32.0,
            menu_bar_height: 30.0,
            sidebar_width: 32.0,
            sidebar_drawer_width: 280.0,
            menu_item_height: 24.0,
            menu_width: 160.0,
            close_button_size: 14.0,
        }
    }
}
