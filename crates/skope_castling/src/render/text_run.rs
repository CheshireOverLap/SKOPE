//! Text Run System — 스타일된 텍스트 런 (UE5 ISlateRun)
//!
//! 텍스트를 연속된 스타일 구간(Run)으로 분할하여
//! TextLayout 엔진에서 혼합 스타일 레이아웃을 지원합니다.

use glam::Vec2;

use crate::core::{Color, FontFamily, FontSelector, FontWeight, FontStyle};
use super::text_renderer::TextMeasurer;

// ============================================================================
// TextRange
// ============================================================================

/// 소스 텍스트 내 바이트 범위
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRange {
    /// 시작 오프셋 (바이트)
    pub start: usize,
    /// 끝 오프셋 (바이트, exclusive)
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

// ============================================================================
// TextRunStyle
// ============================================================================

/// 텍스트 런의 시각 스타일
#[derive(Debug, Clone)]
pub struct TextRunStyle {
    /// 폰트 셀렉터 (family + weight + style)
    pub font_selector: FontSelector,
    /// 폰트 크기 (pt)
    pub font_size: f32,
    /// 텍스트 색상
    pub color: Color,
    /// 밑줄
    pub underline: bool,
    /// 취소선
    pub strikethrough: bool,
    /// 자간 (추가 간격, px)
    pub letter_spacing: f32,
}

impl Default for TextRunStyle {
    fn default() -> Self {
        Self {
            font_selector: FontSelector::default(),
            font_size: 14.0,
            color: Color::WHITE,
            underline: false,
            strikethrough: false,
            letter_spacing: 0.0,
        }
    }
}

impl TextRunStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_font_selector(mut self, selector: FontSelector) -> Self {
        self.font_selector = selector;
        self
    }

    pub fn with_family(mut self, family: FontFamily) -> Self {
        self.font_selector.family = family;
        self
    }

    pub fn with_bold(mut self) -> Self {
        self.font_selector.weight = FontWeight::Bold;
        self
    }

    pub fn with_italic(mut self) -> Self {
        self.font_selector.style = FontStyle::Italic;
        self
    }

    pub fn with_underline(mut self) -> Self {
        self.underline = true;
        self
    }
}

// ============================================================================
// ITextRun trait
// ============================================================================

/// 텍스트 런 트레이트 (UE5 ISlateRun)
///
/// 연속된 동일 스타일 텍스트 구간을 표현합니다.
/// TextLayout 엔진이 런 목록을 받아 라인별로 배치합니다.
pub trait ITextRun: Send + Sync {
    /// 텍스트 내용
    fn text(&self) -> &str;
    /// 시각 스타일
    fn style(&self) -> &TextRunStyle;
    /// 소스 텍스트 내 범위
    fn range(&self) -> TextRange;
    /// 이 런의 너비 측정
    fn measure_width(&self, font_scale: f32) -> f32;
}

// ============================================================================
// FSlateTextRun — 기본 텍스트 런
// ============================================================================

/// 기본 텍스트 런 (가장 일반적 사용)
#[derive(Debug, Clone)]
pub struct FSlateTextRun {
    pub text: String,
    pub style: TextRunStyle,
    pub range: TextRange,
}

impl FSlateTextRun {
    pub fn new(text: impl Into<String>, style: TextRunStyle, range: TextRange) -> Self {
        Self {
            text: text.into(),
            style,
            range,
        }
    }

    /// 편의 생성자: 범위를 자동으로 계산
    pub fn simple(text: impl Into<String>, style: TextRunStyle) -> Self {
        let t: String = text.into();
        let len = t.len();
        Self {
            text: t,
            style,
            range: TextRange::new(0, len),
        }
    }
}

impl ITextRun for FSlateTextRun {
    fn text(&self) -> &str {
        &self.text
    }

    fn style(&self) -> &TextRunStyle {
        &self.style
    }

    fn range(&self) -> TextRange {
        self.range.clone()
    }

    fn measure_width(&self, font_scale: f32) -> f32 {
        let measurer = TextMeasurer::instance();
        if let Ok(m) = measurer.read() {
            m.measure_width_with_selector(
                &self.text,
                self.style.font_size,
                self.style.font_selector,
                font_scale,
            )
        } else {
            0.0
        }
    }
}

// ============================================================================
// FSlateWidgetRun — 인라인 위젯 런
// ============================================================================

/// 인라인 위젯 런 (텍스트 내 위젯 삽입 — UE5 SRichTextBlock 패턴)
#[derive(Debug, Clone)]
pub struct FSlateWidgetRun {
    /// 위젯 크기 (고정)
    pub widget_size: Vec2,
    /// 스타일 (baseline 정렬 등에 사용)
    pub style: TextRunStyle,
    /// 범위
    pub range: TextRange,
    /// 베이스라인 오프셋 (위젯 하단과 텍스트 베이스라인의 차이)
    pub baseline_offset: f32,
}

impl ITextRun for FSlateWidgetRun {
    fn text(&self) -> &str {
        "\u{FFFC}" // Object Replacement Character
    }

    fn style(&self) -> &TextRunStyle {
        &self.style
    }

    fn range(&self) -> TextRange {
        self.range.clone()
    }

    fn measure_width(&self, _font_scale: f32) -> f32 {
        self.widget_size.x
    }
}
