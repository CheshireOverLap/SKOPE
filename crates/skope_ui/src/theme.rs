//! EditorTheme - 중앙집중식 테마 시스템
//!
//! UE의 FEditorStyle / FSlateStyleSet 패턴 참고.
//! 코드에서는 타입 안전한 구조체 접근, JSON으로 사용자 커스텀 테마 지원.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::core::{Color, Margin, SlateBrush};
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

    // ── 컨트롤 공통 ──
    pub control_bg: Color,
    pub control_bg_hover: Color,
    pub control_bg_pressed: Color,
    pub control_bg_disabled: Color,
    pub control_border: Color,
    pub focus_border: Color,
    pub selection_bg: Color,
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
    // 컨트롤 공통
    pub control_height: f32,
    pub small_control_height: f32,
    pub button_padding_h: f32,
    pub button_padding_v: f32,
    pub input_padding: f32,
    pub content_padding: f32,
    pub border_width: f32,
    pub border_radius: f32,
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

    /// 키 이름으로 테마 색상 조회
    ///
    /// 키는 ThemeColors 필드 이름 (예: "text_primary", "accent", "panel_bg")
    pub fn resolve_color(&self, key: &str) -> Option<Color> {
        match key {
            "window_bg" => Some(self.colors.window_bg),
            "panel_bg" => Some(self.colors.panel_bg),
            "content_bg" => Some(self.colors.content_bg),
            "titlebar_bg" => Some(self.colors.titlebar_bg),
            "toolbar_bg" => Some(self.colors.toolbar_bg),
            "tab_bar_bg" => Some(self.colors.tab_bar_bg),
            "tab_active_bg" => Some(self.colors.tab_active_bg),
            "tab_inactive_bg" => Some(self.colors.tab_inactive_bg),
            "tab_hover_bg" => Some(self.colors.tab_hover_bg),
            "text_primary" | "text.primary" => Some(self.colors.text_primary),
            "text_secondary" | "text.secondary" => Some(self.colors.text_secondary),
            "text_muted" | "text.muted" => Some(self.colors.text_muted),
            "text_bright" | "text.bright" => Some(self.colors.text_bright),
            "icon_tint" => Some(self.colors.icon_tint),
            "accent" => Some(self.colors.accent),
            "accent_hover" => Some(self.colors.accent_hover),
            "accent_preview" => Some(self.colors.accent_preview),
            "danger" => Some(self.colors.danger),
            "danger_hover" => Some(self.colors.danger_hover),
            "danger_bg" => Some(self.colors.danger_bg),
            "border" => Some(self.colors.border),
            "separator" => Some(self.colors.separator),
            "shadow" => Some(self.colors.shadow),
            "splitter_bg" => Some(self.colors.splitter_bg),
            "splitter_hover" => Some(self.colors.splitter_hover),
            "splitter_drag" => Some(self.colors.splitter_drag),
            "sidebar_bg" => Some(self.colors.sidebar_bg),
            "menu_bg" => Some(self.colors.menu_bg),
            "menu_border" => Some(self.colors.menu_border),
            "menu_hover" => Some(self.colors.menu_hover),
            "menu_text" => Some(self.colors.menu_text),
            "control_bg" => Some(self.colors.control_bg),
            "control_bg_hover" => Some(self.colors.control_bg_hover),
            "control_bg_pressed" => Some(self.colors.control_bg_pressed),
            "control_bg_disabled" => Some(self.colors.control_bg_disabled),
            "control_border" => Some(self.colors.control_border),
            "focus_border" => Some(self.colors.focus_border),
            "selection_bg" => Some(self.colors.selection_bg),
            _ => None,
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
    /// UE5 StyleColors.cpp 기준 다크 테마
    ///
    /// 모든 배경색은 중성 회색 (R=G=B), 블루 틴트 없음.
    /// sRGB 값을 직접 사용 (Rgba8UnormSrgb 텍스처 포맷 자동 변환).
    pub fn dark() -> Self {
        Self {
            // ── 기본 배경 ── (UE5 Background=#151515, Panel=#242424, Header=#2F2F2F)
            window_bg:   Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            panel_bg:    Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            content_bg:  Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            titlebar_bg: Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            toolbar_bg:  Color::rgba(0.184, 0.184, 0.184, 1.0),  // #2F2F2F

            // ── 탭 바 ── (Header=#2F2F2F, Dropdown=#383838, Recessed=#1A1A1A)
            tab_bar_bg:     Color::rgba(0.184, 0.184, 0.184, 1.0),  // #2F2F2F
            tab_active_bg:  Color::rgba(0.220, 0.220, 0.220, 1.0),  // #383838
            tab_inactive_bg: Color::rgba(0.102, 0.102, 0.102, 1.0), // #1A1A1A
            tab_hover_bg:   Color::rgba(0.165, 0.165, 0.165, 1.0),  // #2A2A2A

            // ── 텍스트 ── (Foreground=#C0C0C0, ForegroundHeader=#C8C8C8, Faded=#606060)
            text_primary:   Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0
            text_secondary: Color::rgba(0.376, 0.376, 0.376, 1.0),  // #606060
            text_muted:     Color::rgba(0.314, 0.314, 0.314, 1.0),  // #505050
            text_bright:    Color::rgba(0.784, 0.784, 0.784, 1.0),  // #C8C8C8

            // ── 아이콘 ──
            icon_tint: Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0

            // ── 액센트 ── (Primary=#0070E0, PrimaryHover=#0060C0)
            accent:         Color::rgba(0.0, 0.439, 0.878, 1.0),   // #0070E0
            accent_hover:   Color::rgba(0.0, 0.376, 0.753, 0.6),   // #0060C0
            accent_preview: Color::rgba(0.0, 0.439, 0.878, 0.25),  // #0070E0 @ 25%

            // ── 위험/닫기 ── (Error=#EF3535)
            danger:       Color::rgba(0.937, 0.208, 0.208, 1.0),  // #EF3535
            danger_hover: Color::rgba(1.0, 0.2, 0.2, 1.0),
            danger_bg:    Color::rgba(0.937, 0.208, 0.208, 0.6),

            // ── 보더/구분선 ── (#303030)
            border:    Color::rgba(0.188, 0.188, 0.188, 1.0),  // #303030
            separator: Color::rgba(0.188, 0.188, 0.188, 0.5),  // #303030 @ 50%
            shadow:    Color::rgba(0.0, 0.0, 0.0, 0.3),

            // ── 스플리터 ──
            splitter_bg:    Color::rgba(0.188, 0.188, 0.188, 1.0),  // #303030
            splitter_hover: Color::rgba(0.0, 0.439, 0.878, 0.5),    // Primary @ 50%
            splitter_drag:  Color::rgba(0.0, 0.439, 0.878, 0.8),    // Primary @ 80%

            // ── 사이드바 ──
            sidebar_bg:                 Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            sidebar_button_active:      Color::rgba(0.0, 0.439, 0.878, 0.8),    // Primary
            sidebar_button_hover:       Color::rgba(0.220, 0.220, 0.220, 1.0),  // #383838
            sidebar_button_normal:      Color::rgba(0.102, 0.102, 0.102, 1.0),  // #1A1A1A
            sidebar_drawer_bg:          Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            sidebar_drawer_header_bg:   Color::rgba(0.184, 0.184, 0.184, 1.0),  // #2F2F2F
            sidebar_drawer_header_text: Color::rgba(0.784, 0.784, 0.784, 1.0),  // #C8C8C8

            // ── 메뉴 ── (Recessed=#1A1A1A)
            menu_bg:      Color::rgba(0.102, 0.102, 0.102, 1.0),  // #1A1A1A
            menu_border:  Color::rgba(0.188, 0.188, 0.188, 1.0),  // #303030
            menu_hover:   Color::rgba(0.0, 0.439, 0.878, 0.6),    // Primary @ 60%
            menu_text:    Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0
            menu_divider: Color::rgba(0.188, 0.188, 0.188, 0.5),  // #303030 @ 50%

            // ── 나침반 ──
            compass_line:    Color::rgba(0.753, 0.753, 0.753, 0.8),
            compass_hover:   Color::rgba(0.9, 0.5, 0.1, 0.6),
            compass_preview: Color::rgba(0.9, 0.5, 0.1, 0.25),

            // ── 윈도우 컨트롤 ──
            window_button_bg:    Color::rgba(0.184, 0.184, 0.184, 1.0),  // #2F2F2F
            window_button_hover: Color::rgba(0.341, 0.341, 0.341, 1.0),  // #575757
            window_close_hover:  Color::rgba(0.937, 0.208, 0.208, 1.0),  // #EF3535
            window_button_icon:  Color::rgba(0.753, 0.753, 0.753, 1.0),  // #C0C0C0

            // ── 드래그 프리뷰 ──
            drag_preview_bg:     Color::rgba(0.0, 0.439, 0.878, 0.9),    // Primary
            drag_preview_border: Color::rgba(0.0, 0.502, 1.0, 0.9),
            drag_tab_bar_bg:     Color::rgba(0.082, 0.082, 0.082, 0.8),  // #151515
            drag_title_text:     Color::rgba(1.0, 1.0, 1.0, 0.9),

            // ── 도킹 타겟 ──
            dock_target_fill:   Color::rgba(0.0, 0.439, 0.878, 0.25),  // Primary @ 25%
            dock_target_border: Color::rgba(0.0, 0.502, 1.0, 0.7),

            // ── 메이저 탭 바 ──
            major_tab_bar_bg:      Color::rgba(0.082, 0.082, 0.082, 1.0),  // #151515
            major_tab_active_bg:   Color::rgba(0.141, 0.141, 0.141, 1.0),  // #242424
            major_tab_hover_bg:    Color::rgba(0.102, 0.102, 0.102, 1.0),  // #1A1A1A
            major_tab_inactive_bg: Color::rgba(0.082, 0.082, 0.082, 0.0),  // transparent
            major_tab_inactive_text: Color::rgba(0.376, 0.376, 0.376, 1.0), // #606060
            major_tab_accent:      Color::rgba(0.0, 0.439, 0.878, 1.0),    // #0070E0

            // ── 컨트롤 공통 ── (Input=#0F0F0F)
            control_bg:          Color::rgba(0.059, 0.059, 0.059, 1.0),  // #0F0F0F
            control_bg_hover:    Color::rgba(0.102, 0.102, 0.102, 1.0),  // #1A1A1A
            control_bg_pressed:  Color::rgba(0.039, 0.039, 0.039, 1.0),  // #0A0A0A
            control_bg_disabled: Color::rgba(0.071, 0.071, 0.071, 1.0),  // #121212
            control_border:      Color::rgba(0.188, 0.188, 0.188, 1.0),  // #303030
            focus_border:        Color::rgba(0.0, 0.439, 0.878, 1.0),    // #0070E0
            selection_bg:        Color::rgba(0.0, 0.239, 0.502, 0.50),   // #003D80 @ 50%
        }
    }
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            small: 9.0,
            normal: 10.0,
            medium: 12.0,
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
            control_height: 24.0,
            small_control_height: 20.0,
            button_padding_h: 12.0,
            button_padding_v: 4.0,
            input_padding: 6.0,
            content_padding: 8.0,
            border_width: 1.0,
            border_radius: 2.0,
        }
    }
}

// ============================================================================
// StyleSet — 계층적 스타일 상속 (UE의 FSlateStyleSet)
// ============================================================================

/// 키-값 기반 스타일 세트 (부모 폴백 체인 지원)
///
/// ```ignore
/// let global = Arc::new(StyleSet::new("Global"));
/// global.set_color("text.primary", Color::WHITE);
///
/// let custom = StyleSet::with_parent("Custom", global.clone());
/// // custom.get_color("text.primary") → Color::WHITE (부모에서 상속)
/// ```
pub struct StyleSet {
    name: String,
    parent: Option<Arc<StyleSet>>,
    colors: HashMap<String, Color>,
    floats: HashMap<String, f32>,
    brushes: HashMap<String, SlateBrush>,
    margins: HashMap<String, Margin>,
}

impl StyleSet {
    /// 루트 스타일 세트 생성
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent: None,
            colors: HashMap::new(),
            floats: HashMap::new(),
            brushes: HashMap::new(),
            margins: HashMap::new(),
        }
    }

    /// 부모를 가진 스타일 세트 생성
    pub fn with_parent(name: impl Into<String>, parent: Arc<StyleSet>) -> Self {
        Self {
            name: name.into(),
            parent: Some(parent),
            colors: HashMap::new(),
            floats: HashMap::new(),
            brushes: HashMap::new(),
            margins: HashMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    // ── Color ──

    pub fn set_color(&mut self, key: impl Into<String>, color: Color) {
        self.colors.insert(key.into(), color);
    }

    pub fn get_color(&self, key: &str) -> Option<Color> {
        self.colors.get(key).copied()
            .or_else(|| self.parent.as_ref()?.get_color(key))
    }

    // ── Float ──

    pub fn set_float(&mut self, key: impl Into<String>, value: f32) {
        self.floats.insert(key.into(), value);
    }

    pub fn get_float(&self, key: &str) -> Option<f32> {
        self.floats.get(key).copied()
            .or_else(|| self.parent.as_ref()?.get_float(key))
    }

    // ── Brush ──

    pub fn set_brush(&mut self, key: impl Into<String>, brush: SlateBrush) {
        self.brushes.insert(key.into(), brush);
    }

    pub fn get_brush(&self, key: &str) -> Option<&SlateBrush> {
        self.brushes.get(key)
            .or_else(|| self.parent.as_ref()?.get_brush(key))
    }

    // ── Margin ──

    pub fn set_margin(&mut self, key: impl Into<String>, margin: Margin) {
        self.margins.insert(key.into(), margin);
    }

    pub fn get_margin(&self, key: &str) -> Option<Margin> {
        self.margins.get(key).copied()
            .or_else(|| self.parent.as_ref()?.get_margin(key))
    }

    /// SlateColor 해석 (이 스타일셋과 테마를 사용)
    pub fn resolve_slate_color(
        &self,
        sc: &crate::core::SlateColor,
        theme: &EditorTheme,
    ) -> Color {
        let theme_resolver = |key: &str| theme.resolve_color(key);
        let style_resolver = |key: &str| self.get_color(key);
        sc.resolve(&theme_resolver, Some(&style_resolver))
    }
}

// ============================================================================
// StyleManager — 글로벌 스타일 세트 레지스트리
// ============================================================================

/// 글로벌 스타일 매니저 (이름으로 StyleSet 관리)
pub struct StyleManager {
    sets: HashMap<String, Arc<StyleSet>>,
}

impl StyleManager {
    /// 싱글톤 인스턴스
    pub fn instance() -> &'static Mutex<StyleManager> {
        static INSTANCE: OnceLock<Mutex<StyleManager>> = OnceLock::new();
        INSTANCE.get_or_init(|| Mutex::new(StyleManager {
            sets: HashMap::new(),
        }))
    }

    /// 스타일 세트 등록
    pub fn register(&mut self, style_set: Arc<StyleSet>) {
        self.sets.insert(style_set.name().to_string(), style_set);
    }

    /// 스타일 세트 조회
    pub fn get(&self, name: &str) -> Option<Arc<StyleSet>> {
        self.sets.get(name).cloned()
    }

    /// 등록된 스타일 세트 이름 목록
    pub fn names(&self) -> Vec<&str> {
        self.sets.keys().map(|s| s.as_str()).collect()
    }
}
