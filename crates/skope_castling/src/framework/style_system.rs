//! 스타일링 시스템 — 위젯 타입별 스타일 구조체 및 스타일셋 관리
//!
//! UE Slate의 FSlateStyleSet / FWidgetStyle에 해당.

use std::collections::HashMap;
use std::sync::Arc;
use crate::core::{Color, FontSelector, Margin, SlateBrush};

/// 위젯 스타일 기본 trait
pub trait WidgetStyle: Send + Sync + std::fmt::Debug {
    /// 스타일 타입 이름
    fn type_name(&self) -> &'static str;
    /// 기본 스타일 반환
    fn default_style() -> Self where Self: Sized;
}

/// 버튼 스타일 (SlateBrush 기반)
///
/// 위젯 레벨 `s_button::ButtonStyle`과 동일한 패턴.
#[derive(Debug, Clone)]
pub struct ButtonStyle {
    pub normal: SlateBrush,
    pub hovered: SlateBrush,
    pub pressed: SlateBrush,
    pub disabled: SlateBrush,
    pub normal_foreground: Color,
    pub hovered_foreground: Color,
    pub pressed_foreground: Color,
    pub disabled_foreground: Color,
    pub font: FontSelector,
    pub padding: Margin,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        let normal = Color::rgba(0.2, 0.2, 0.2, 1.0);
        let hovered = Color::rgba(0.3, 0.3, 0.3, 1.0);
        let pressed = Color::rgba(0.15, 0.15, 0.15, 1.0);
        let disabled = Color::rgba(0.1, 0.1, 0.1, 0.5);
        let border = Color::rgba(0.4, 0.4, 0.4, 1.0);
        Self {
            normal: SlateBrush::rounded_with_outline(normal, border, 1.0, 2.0),
            hovered: SlateBrush::rounded_with_outline(hovered, border, 1.0, 2.0),
            pressed: SlateBrush::rounded_with_outline(pressed, border, 1.0, 2.0),
            disabled: SlateBrush::rounded_with_outline(disabled, border, 1.0, 2.0),
            normal_foreground: Color::WHITE,
            hovered_foreground: Color::WHITE,
            pressed_foreground: Color::WHITE,
            disabled_foreground: Color::rgba(0.6, 0.6, 0.6, 0.5),
            font: FontSelector::default(),
            padding: Margin::uniform(4.0),
        }
    }
}

impl WidgetStyle for ButtonStyle {
    fn type_name(&self) -> &'static str { "ButtonStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 체크박스 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct CheckBoxStyle {
    pub box_size: f32,
    pub unchecked_image: SlateBrush,
    pub unchecked_hovered_image: SlateBrush,
    pub checked_image: SlateBrush,
    pub checked_hovered_image: SlateBrush,
    pub undetermined_image: SlateBrush,
    pub disabled_image: SlateBrush,
    pub foreground_color: Color,
    pub padding: Margin,
}

impl Default for CheckBoxStyle {
    fn default() -> Self {
        let unchecked = Color::rgba(0.15, 0.15, 0.17, 1.0);
        let checked = Color::rgba(0.2, 0.5, 0.8, 1.0);
        let hovered = Color::rgba(0.25, 0.25, 0.28, 1.0);
        let disabled = Color::rgba(0.3, 0.3, 0.32, 0.5);
        let border = Color::rgba(0.4, 0.4, 0.45, 1.0);
        Self {
            box_size: 16.0,
            unchecked_image: SlateBrush::rounded_with_outline(unchecked, border, 1.0, 2.0),
            unchecked_hovered_image: SlateBrush::rounded_with_outline(hovered, border, 1.0, 2.0),
            checked_image: SlateBrush::rounded_with_outline(checked, border, 1.0, 2.0),
            checked_hovered_image: SlateBrush::rounded_with_outline(
                Color::rgba(0.25, 0.55, 0.85, 1.0), border, 1.0, 2.0,
            ),
            undetermined_image: SlateBrush::rounded_with_outline(
                Color::rgba(0.18, 0.35, 0.55, 1.0), border, 1.0, 2.0,
            ),
            disabled_image: SlateBrush::rounded_with_outline(disabled, border, 1.0, 2.0),
            foreground_color: Color::WHITE,
            padding: Margin::uniform(0.0),
        }
    }
}

impl WidgetStyle for CheckBoxStyle {
    fn type_name(&self) -> &'static str { "CheckBoxStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 슬라이더 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct SliderStyle {
    pub normal_bar_image: SlateBrush,
    pub hovered_bar_image: SlateBrush,
    pub disabled_bar_image: SlateBrush,
    pub fill_image: SlateBrush,
    pub normal_thumb_image: SlateBrush,
    pub hovered_thumb_image: SlateBrush,
    pub dragged_thumb_image: SlateBrush,
    pub disabled_thumb_image: SlateBrush,
    pub bar_thickness: f32,
    pub thumb_size: f32,
}

impl Default for SliderStyle {
    fn default() -> Self {
        let track = Color::rgba(0.2, 0.2, 0.22, 1.0);
        let fill = Color::rgba(0.3, 0.6, 0.9, 1.0);
        let handle = Color::rgba(0.9, 0.9, 0.95, 1.0);
        let handle_hover = Color::rgba(1.0, 1.0, 1.0, 1.0);
        let handle_drag = Color::rgba(0.3, 0.6, 0.9, 1.0);
        let disabled = Color::rgba(0.3, 0.3, 0.32, 0.5);
        Self {
            normal_bar_image: SlateBrush::rounded(track, 2.0),
            hovered_bar_image: SlateBrush::rounded(track, 2.0),
            disabled_bar_image: SlateBrush::rounded(disabled, 2.0),
            fill_image: SlateBrush::rounded(fill, 2.0),
            normal_thumb_image: SlateBrush::rounded(handle, 7.0),
            hovered_thumb_image: SlateBrush::rounded(handle_hover, 7.0),
            dragged_thumb_image: SlateBrush::rounded(handle_drag, 7.0),
            disabled_thumb_image: SlateBrush::rounded(disabled, 7.0),
            bar_thickness: 4.0,
            thumb_size: 14.0,
        }
    }
}

impl WidgetStyle for SliderStyle {
    fn type_name(&self) -> &'static str { "SliderStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 콤보박스 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct ComboBoxStyle {
    pub normal_brush: SlateBrush,
    pub hovered_brush: SlateBrush,
    pub pressed_brush: SlateBrush,
    pub disabled_brush: SlateBrush,
    pub text_color: Color,
    pub font_size: f32,
    pub padding: f32,
    pub min_width: f32,
    pub height: f32,
    pub item_height: f32,
    pub item_hover_brush: SlateBrush,
    pub item_selected_brush: SlateBrush,
    pub arrow_image: SlateBrush,
    pub max_visible_items: usize,
    pub dropdown_border_brush: SlateBrush,
}

impl Default for ComboBoxStyle {
    fn default() -> Self {
        let bg = Color::rgba(0.18, 0.18, 0.2, 1.0);
        let hover = Color::rgba(0.22, 0.22, 0.24, 1.0);
        let open = Color::rgba(0.2, 0.2, 0.22, 1.0);
        let border = Color::rgba(0.35, 0.35, 0.38, 1.0);
        let arrow = Color::rgba(0.6, 0.6, 0.65, 1.0);
        Self {
            normal_brush: SlateBrush::rounded_with_outline(bg, border, 1.0, 2.0),
            hovered_brush: SlateBrush::rounded_with_outline(hover, border, 1.0, 2.0),
            pressed_brush: SlateBrush::rounded_with_outline(open, border, 1.0, 2.0),
            disabled_brush: SlateBrush::rounded_with_outline(
                Color::rgba(0.14, 0.14, 0.15, 1.0), border, 1.0, 2.0,
            ),
            text_color: Color::rgba(0.9, 0.9, 0.92, 1.0),
            font_size: 11.0,
            padding: 6.0,
            min_width: 120.0,
            height: 24.0,
            item_height: 24.0,
            item_hover_brush: SlateBrush::Color(Color::rgba(0.25, 0.25, 0.28, 1.0)),
            item_selected_brush: SlateBrush::Color(Color::rgba(0.2, 0.4, 0.7, 0.8)),
            arrow_image: SlateBrush::Color(arrow),
            max_visible_items: 8,
            dropdown_border_brush: SlateBrush::rounded_with_outline(bg, border, 1.0, 0.0),
        }
    }
}

impl WidgetStyle for ComboBoxStyle {
    fn type_name(&self) -> &'static str { "ComboBoxStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 스핀박스 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct SpinBoxStyle {
    pub background_brush: SlateBrush,
    pub hovered_brush: SlateBrush,
    pub active_fill_brush: SlateBrush,
    pub focused_border_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub text_color: Color,
    pub font_size: f32,
    pub padding: f32,
    pub min_width: f32,
    pub height: f32,
}

impl Default for SpinBoxStyle {
    fn default() -> Self {
        let bg = Color::rgba(0.12, 0.12, 0.14, 1.0);
        let hover = Color::rgba(0.15, 0.15, 0.17, 1.0);
        let drag_highlight = Color::rgba(0.2, 0.4, 0.6, 0.3);
        Self {
            background_brush: SlateBrush::Color(bg),
            hovered_brush: SlateBrush::Color(hover),
            active_fill_brush: SlateBrush::Color(drag_highlight),
            focused_border_color: Color::rgba(0.3, 0.6, 0.9, 1.0),
            border_color: Color::rgba(0.3, 0.3, 0.32, 1.0),
            border_width: 1.0,
            text_color: Color::rgba(0.9, 0.9, 0.92, 1.0),
            font_size: 11.0,
            padding: 4.0,
            min_width: 60.0,
            height: 24.0,
        }
    }
}

impl WidgetStyle for SpinBoxStyle {
    fn type_name(&self) -> &'static str { "SpinBoxStyle" }
    fn default_style() -> Self { Self::default() }
}

/// DockTab 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct DockTabStyle {
    /// 비활성 탭 배경
    pub normal_brush: SlateBrush,
    /// 호버 탭 배경
    pub hovered_brush: SlateBrush,
    /// 활성 탭 배경
    pub active_brush: SlateBrush,
    /// 컨텐츠 영역 배경 (탭 아래)
    pub foreground_brush: SlateBrush,
    /// 탭 바 배경
    pub tab_well_brush: SlateBrush,
    /// 닫기 버튼 (일반)
    pub close_button_normal: SlateBrush,
    /// 닫기 버튼 (호버)
    pub close_button_hovered: SlateBrush,
    /// 활성 탭 전경색
    pub active_foreground_color: Color,
    /// 비활성 탭 전경색
    pub normal_foreground_color: Color,
    /// 호버 탭 전경색
    pub hovered_foreground_color: Color,
    /// 플래시 색상
    pub flash_color: Color,
    /// 탭 패딩
    pub tab_padding: Margin,
    /// 아이콘 크기
    pub icon_size: f32,
    /// 오버랩 너비
    pub overlap_width: f32,
}

impl Default for DockTabStyle {
    fn default() -> Self {
        let active_bg = Color::rgba(0.141, 0.141, 0.141, 1.0);   // #242424
        let inactive_bg = Color::TRANSPARENT;
        let tab_bar_bg = Color::rgba(0.082, 0.082, 0.082, 1.0);  // #151515
        let accent = Color::rgba(0.0, 0.439, 0.878, 1.0);        // #0070E0
        Self {
            normal_brush: SlateBrush::Color(inactive_bg),
            hovered_brush: SlateBrush::Color(Color::rgba(0.141, 0.141, 0.141, 0.5)),
            active_brush: SlateBrush::Color(active_bg),
            foreground_brush: SlateBrush::Color(active_bg),
            tab_well_brush: SlateBrush::Color(tab_bar_bg),
            close_button_normal: SlateBrush::None,
            close_button_hovered: SlateBrush::Color(Color::rgba(0.8, 0.2, 0.2, 0.6)),
            active_foreground_color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            normal_foreground_color: Color::rgba(0.753, 0.753, 0.753, 1.0),
            hovered_foreground_color: Color::rgba(0.9, 0.9, 0.9, 1.0),
            flash_color: accent,
            tab_padding: Margin::symmetric(4.0, 2.0),
            icon_size: 12.0,
            overlap_width: 0.0,
        }
    }
}

impl WidgetStyle for DockTabStyle {
    fn type_name(&self) -> &'static str { "DockTabStyle" }
    fn default_style() -> Self { Self::default() }
}

/// Window 스타일 (SlateBrush 기반)
#[derive(Debug, Clone)]
pub struct WindowStyle {
    /// 최소화 버튼 (일반)
    pub minimize_button_normal: SlateBrush,
    /// 최소화 버튼 (호버)
    pub minimize_button_hovered: SlateBrush,
    /// 최대화 버튼 (일반)
    pub maximize_button_normal: SlateBrush,
    /// 최대화 버튼 (호버)
    pub maximize_button_hovered: SlateBrush,
    /// 닫기 버튼 (일반)
    pub close_button_normal: SlateBrush,
    /// 닫기 버튼 (호버)
    pub close_button_hovered: SlateBrush,
    /// 버튼 아이콘 색상
    pub button_icon_color: Color,
    /// 타이틀 바 브러시
    pub title_bar_brush: SlateBrush,
    /// 테두리 브러시
    pub border_brush: SlateBrush,
}

impl Default for WindowStyle {
    fn default() -> Self {
        let btn_bg = Color::TRANSPARENT;
        let btn_hover = Color::rgba(0.25, 0.25, 0.27, 1.0);
        let close_hover = Color::rgba(0.77, 0.17, 0.15, 1.0);
        Self {
            minimize_button_normal: SlateBrush::Color(btn_bg),
            minimize_button_hovered: SlateBrush::Color(btn_hover),
            maximize_button_normal: SlateBrush::Color(btn_bg),
            maximize_button_hovered: SlateBrush::Color(btn_hover),
            close_button_normal: SlateBrush::Color(btn_bg),
            close_button_hovered: SlateBrush::Color(close_hover),
            button_icon_color: Color::rgba(0.7, 0.7, 0.7, 1.0),
            title_bar_brush: SlateBrush::Color(Color::rgba(0.082, 0.082, 0.082, 1.0)),
            border_brush: SlateBrush::Color(Color::rgba(0.2, 0.2, 0.22, 1.0)),
        }
    }
}

impl WidgetStyle for WindowStyle {
    fn type_name(&self) -> &'static str { "WindowStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 텍스트 블록 스타일
#[derive(Debug, Clone)]
pub struct TextBlockStyle {
    pub color: Color,
    pub font: FontSelector,
    pub shadow_offset: glam::Vec2,
    pub shadow_color: Color,
    pub highlight_color: Color,
    pub highlight_shape: HighlightShape,
}

/// 하이라이트 형태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightShape {
    Box,
    Underline,
    Strikethrough,
}

impl Default for TextBlockStyle {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            font: FontSelector::default(),
            shadow_offset: glam::Vec2::ZERO,
            shadow_color: Color::TRANSPARENT,
            highlight_color: Color::rgba(0.2, 0.4, 0.8, 0.5),
            highlight_shape: HighlightShape::Box,
        }
    }
}

impl WidgetStyle for TextBlockStyle {
    fn type_name(&self) -> &'static str { "TextBlockStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 에디터블 텍스트 스타일
#[derive(Debug, Clone)]
pub struct EditableTextStyle {
    pub text_style: TextBlockStyle,
    pub caret_color: Color,
    pub selection_background: Color,
    pub background_color: Color,
    pub focused_border_color: Color,
    pub padding: Margin,
}

impl Default for EditableTextStyle {
    fn default() -> Self {
        Self {
            text_style: TextBlockStyle::default(),
            caret_color: Color::WHITE,
            selection_background: Color::rgba(0.2, 0.4, 0.8, 0.5),
            background_color: Color::rgba(0.1, 0.1, 0.1, 1.0),
            focused_border_color: Color::rgba(0.3, 0.5, 0.9, 1.0),
            padding: Margin::uniform(4.0),
        }
    }
}

impl WidgetStyle for EditableTextStyle {
    fn type_name(&self) -> &'static str { "EditableTextStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 스크롤바 스타일
#[derive(Debug, Clone)]
pub struct ScrollBarStyle {
    pub thumb_color: Color,
    pub thumb_hovered_color: Color,
    pub track_color: Color,
    pub thickness: f32,
    pub min_thumb_length: f32,
}

impl Default for ScrollBarStyle {
    fn default() -> Self {
        Self {
            thumb_color: Color::rgba(0.4, 0.4, 0.4, 0.8),
            thumb_hovered_color: Color::rgba(0.5, 0.5, 0.5, 1.0),
            track_color: Color::rgba(0.15, 0.15, 0.15, 0.5),
            thickness: 8.0,
            min_thumb_length: 20.0,
        }
    }
}

impl WidgetStyle for ScrollBarStyle {
    fn type_name(&self) -> &'static str { "ScrollBarStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 진행 바 스타일
#[derive(Debug, Clone)]
pub struct ProgressBarStyle {
    pub background_color: Color,
    pub fill_color: Color,
    pub border_color: Color,
    pub height: f32,
    pub corner_radius: f32,
}

impl Default for ProgressBarStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.15, 0.15, 0.15, 1.0),
            fill_color: Color::rgba(0.2, 0.6, 1.0, 1.0),
            border_color: Color::rgba(0.3, 0.3, 0.3, 1.0),
            height: 16.0,
            corner_radius: 2.0,
        }
    }
}

impl WidgetStyle for ProgressBarStyle {
    fn type_name(&self) -> &'static str { "ProgressBarStyle" }
    fn default_style() -> Self { Self::default() }
}

/// 스타일셋 — 위젯 타입별 스타일 레지스트리
pub struct SlateStyleSet {
    name: String,
    styles: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
    parent: Option<Arc<SlateStyleSet>>,
}

impl SlateStyleSet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            styles: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(mut self, parent: Arc<SlateStyleSet>) -> Self {
        self.parent = Some(parent);
        self
    }

    /// 스타일 등록
    pub fn set<T: WidgetStyle + Clone + 'static>(&mut self, style: T) {
        self.styles.insert(
            style.type_name().to_string(),
            Box::new(style),
        );
    }

    /// 스타일 조회 (부모 체인 탐색)
    pub fn get<T: WidgetStyle + Clone + 'static>(&self, type_name: &str) -> Option<T> {
        if let Some(any) = self.styles.get(type_name) {
            return any.downcast_ref::<T>().cloned();
        }
        if let Some(ref parent) = self.parent {
            return parent.get(type_name);
        }
        None
    }

    /// 스타일 조회 또는 기본값
    pub fn get_or_default<T: WidgetStyle + Clone + 'static>(&self, type_name: &str) -> T {
        self.get(type_name).unwrap_or_else(T::default_style)
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn style_count(&self) -> usize { self.styles.len() }
    pub fn has_parent(&self) -> bool { self.parent.is_some() }
}

impl std::fmt::Debug for SlateStyleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlateStyleSet")
            .field("name", &self.name)
            .field("style_count", &self.styles.len())
            .field("has_parent", &self.parent.is_some())
            .finish()
    }
}

/// 콘텐츠 루트 — 스타일셋 적용 범위
#[derive(Debug)]
pub struct ContentRoot {
    pub style_set: Arc<SlateStyleSet>,
    pub widget_id: u64,
}

impl ContentRoot {
    pub fn new(widget_id: u64, style_set: Arc<SlateStyleSet>) -> Self {
        Self { style_set, widget_id }
    }
}

/// 아이콘 탐색기
pub struct SlateIconFinder {
    icon_paths: HashMap<String, String>,
    fallback_icon: Option<String>,
}

impl SlateIconFinder {
    pub fn new() -> Self {
        Self {
            icon_paths: HashMap::new(),
            fallback_icon: None,
        }
    }

    pub fn register(&mut self, name: impl Into<String>, path: impl Into<String>) {
        self.icon_paths.insert(name.into(), path.into());
    }

    pub fn find(&self, name: &str) -> Option<&str> {
        self.icon_paths.get(name)
            .map(|s| s.as_str())
            .or(self.fallback_icon.as_deref())
    }

    pub fn set_fallback(&mut self, path: impl Into<String>) {
        self.fallback_icon = Some(path.into());
    }

    pub fn icon_count(&self) -> usize { self.icon_paths.len() }
}

impl std::fmt::Debug for SlateIconFinder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlateIconFinder")
            .field("icon_count", &self.icon_paths.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_style_default() {
        let style = ButtonStyle::default();
        assert!(style.normal_foreground == Color::WHITE);
        assert!(style.padding.horizontal() > 0.0);
    }

    #[test]
    fn test_style_set_register_get() {
        let mut set = SlateStyleSet::new("default");
        set.set(ButtonStyle::default());
        let style: Option<ButtonStyle> = set.get("ButtonStyle");
        assert!(style.is_some());
    }

    #[test]
    fn test_style_set_parent_chain() {
        let mut parent = SlateStyleSet::new("parent");
        parent.set(ButtonStyle::default());
        let parent = Arc::new(parent);

        let child = SlateStyleSet::new("child").with_parent(parent);
        let style: Option<ButtonStyle> = child.get("ButtonStyle");
        assert!(style.is_some());
    }

    #[test]
    fn test_style_set_override() {
        let mut parent = SlateStyleSet::new("parent");
        parent.set(ButtonStyle::default());
        let parent = Arc::new(parent);

        let mut child = SlateStyleSet::new("child").with_parent(parent);
        let mut custom = ButtonStyle::default();
        custom.normal_foreground = Color::rgba(1.0, 0.0, 0.0, 1.0);
        child.set(custom);

        let style: ButtonStyle = child.get_or_default("ButtonStyle");
        assert_eq!(style.normal_foreground.r, 1.0);
    }

    #[test]
    fn test_get_or_default() {
        let set = SlateStyleSet::new("empty");
        let style: ScrollBarStyle = set.get_or_default("ScrollBarStyle");
        assert_eq!(style.thickness, 8.0);
    }

    #[test]
    fn test_icon_finder() {
        let mut finder = SlateIconFinder::new();
        finder.register("play", "/icons/play.png");
        finder.register("stop", "/icons/stop.png");
        assert_eq!(finder.find("play"), Some("/icons/play.png"));
        assert_eq!(finder.find("unknown"), None);
        finder.set_fallback("/icons/default.png");
        assert_eq!(finder.find("unknown"), Some("/icons/default.png"));
    }

    #[test]
    fn test_content_root() {
        let set = Arc::new(SlateStyleSet::new("root"));
        let root = ContentRoot::new(42, set.clone());
        assert_eq!(root.widget_id, 42);
    }

    #[test]
    fn test_all_styles_implement_trait() {
        fn assert_style<T: WidgetStyle + Clone>() {
            let s = T::default_style();
            let _ = s.type_name();
        }
        assert_style::<ButtonStyle>();
        assert_style::<TextBlockStyle>();
        assert_style::<EditableTextStyle>();
        assert_style::<ScrollBarStyle>();
        assert_style::<ProgressBarStyle>();
        assert_style::<CheckBoxStyle>();
        assert_style::<SliderStyle>();
        assert_style::<ComboBoxStyle>();
        assert_style::<SpinBoxStyle>();
        assert_style::<DockTabStyle>();
        assert_style::<WindowStyle>();
    }
}
