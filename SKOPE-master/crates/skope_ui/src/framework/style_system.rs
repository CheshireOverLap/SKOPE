//! 스타일링 시스템 — 위젯 타입별 스타일 구조체 및 스타일셋 관리
//!
//! UE Slate의 FSlateStyleSet / FWidgetStyle에 해당.

use std::collections::HashMap;
use std::sync::Arc;
use crate::core::{Color, FontSelector, Margin};

/// 위젯 스타일 기본 trait
pub trait WidgetStyle: Send + Sync + std::fmt::Debug {
    /// 스타일 타입 이름
    fn type_name(&self) -> &'static str;
    /// 기본 스타일 반환
    fn default_style() -> Self where Self: Sized;
}

/// 버튼 스타일
#[derive(Debug, Clone)]
pub struct ButtonStyle {
    pub normal_color: Color,
    pub hovered_color: Color,
    pub pressed_color: Color,
    pub disabled_color: Color,
    pub text_color: Color,
    pub font: FontSelector,
    pub padding: Margin,
    pub border_color: Color,
    pub border_width: f32,
    pub corner_radius: f32,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            normal_color: Color::rgba(0.2, 0.2, 0.2, 1.0),
            hovered_color: Color::rgba(0.3, 0.3, 0.3, 1.0),
            pressed_color: Color::rgba(0.15, 0.15, 0.15, 1.0),
            disabled_color: Color::rgba(0.1, 0.1, 0.1, 0.5),
            text_color: Color::WHITE,
            font: FontSelector::default(),
            padding: Margin::uniform(4.0),
            border_color: Color::rgba(0.4, 0.4, 0.4, 1.0),
            border_width: 1.0,
            corner_radius: 2.0,
        }
    }
}

impl WidgetStyle for ButtonStyle {
    fn type_name(&self) -> &'static str { "ButtonStyle" }
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
        assert!(style.corner_radius > 0.0);
        assert_eq!(style.border_width, 1.0);
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
        custom.corner_radius = 10.0;
        child.set(custom);

        let style: ButtonStyle = child.get_or_default("ButtonStyle");
        assert_eq!(style.corner_radius, 10.0);
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
    }
}
