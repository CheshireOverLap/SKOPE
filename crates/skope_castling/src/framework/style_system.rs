//! 스타일링 시스템 — 위젯 타입별 스타일 구조체
//!
//! UE Slate의 FSlateStyleSet / FWidgetStyle에 해당.
//!
//! 위젯 로컬 스타일(s_button::ButtonStyle 등)이 우선.
//! 여기에는 도킹/윈도우 등 프레임워크 공용 스타일만 유지.

use crate::core::{Color, FontSelector, Margin, SlateBrush};

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
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl DockTabStyle {
    /// 테마에서 스타일 생성
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            normal_brush: SlateBrush::Color(tc.tab_inactive_bg),
            hovered_brush: SlateBrush::Color(tc.tab_hover_bg),
            active_brush: SlateBrush::Color(tc.tab_active_bg),
            foreground_brush: SlateBrush::Color(tc.tab_active_bg),
            tab_well_brush: SlateBrush::Color(tc.tab_bar_bg),
            close_button_normal: SlateBrush::None,
            close_button_hovered: SlateBrush::Color(tc.danger),
            active_foreground_color: tc.text_bright,
            normal_foreground_color: tc.text_primary,
            hovered_foreground_color: tc.text_bright,
            flash_color: tc.accent,
            tab_padding: Margin::symmetric(4.0, 2.0),
            icon_size: 16.0,
            overlap_width: 0.0,
        }
    }
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
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl WindowStyle {
    /// 테마에서 스타일 생성
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            minimize_button_normal: SlateBrush::Color(tc.window_button_bg),
            minimize_button_hovered: SlateBrush::Color(tc.window_button_hover),
            maximize_button_normal: SlateBrush::Color(tc.window_button_bg),
            maximize_button_hovered: SlateBrush::Color(tc.window_button_hover),
            close_button_normal: SlateBrush::Color(tc.window_button_bg),
            close_button_hovered: SlateBrush::Color(tc.window_close_hover),
            button_icon_color: tc.window_button_icon,
            title_bar_brush: SlateBrush::Color(tc.titlebar_bg),
            border_brush: SlateBrush::Color(tc.border),
        }
    }
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
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl TextBlockStyle {
    /// 테마에서 스타일 생성
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            color: tc.text_primary,
            font: FontSelector::default(),
            shadow_offset: glam::Vec2::ZERO,
            shadow_color: Color::TRANSPARENT,
            highlight_color: tc.accent.with_alpha(0.5),
            highlight_shape: HighlightShape::Box,
        }
    }
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
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl EditableTextStyle {
    /// 테마에서 스타일 생성
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            text_style: TextBlockStyle::from_theme(theme),
            caret_color: tc.text_bright,
            selection_background: tc.accent.with_alpha(0.5),
            background_color: tc.control_bg,
            focused_border_color: tc.focus_border,
            padding: Margin::uniform(4.0),
        }
    }
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
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl ScrollBarStyle {
    /// 테마에서 스타일 생성
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            thumb_color: tc.scrollbar_thumb,
            thumb_hovered_color: tc.scrollbar_thumb_hover,
            track_color: tc.scrollbar_track,
            thickness: 8.0,
            min_thumb_length: 20.0,
        }
    }
}
