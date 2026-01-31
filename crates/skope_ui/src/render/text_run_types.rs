//! Text Run Types — 확장 텍스트 런 타입
//!
//! 기본 FSlateTextRun 외에 하이퍼링크, 이미지, 패스워드 등
//! 특수 목적의 텍스트 런을 정의합니다.

use glam::Vec2;

use crate::core::Color;
use super::text_run::{ITextRun, TextRange, TextRunStyle};
use super::text_renderer::TextMeasurer;

// ============================================================================
// SlateHyperlinkRun — 하이퍼링크 텍스트 런
// ============================================================================

/// 하이퍼링크 텍스트 런 (UE5 FSlateHyperlinkRun)
///
/// 클릭 가능한 링크 텍스트를 표현합니다.
#[derive(Debug, Clone)]
pub struct SlateHyperlinkRun {
    /// 표시 텍스트
    pub text: String,
    /// 링크 URL/식별자
    pub url: String,
    /// 스타일
    pub style: TextRunStyle,
    /// 범위
    pub range: TextRange,
    /// 호버 시 색상
    pub hover_color: Color,
    /// 브라우저 스타일 밑줄 (호버 시만 표시)
    pub underline_on_hover_only: bool,
}

impl SlateHyperlinkRun {
    /// 새 하이퍼링크 런 생성
    pub fn new(text: impl Into<String>, url: impl Into<String>, style: TextRunStyle) -> Self {
        let t: String = text.into();
        let len = t.len();
        Self {
            text: t,
            url: url.into(),
            style: TextRunStyle {
                underline: true,
                color: Color::rgb(0.3, 0.5, 1.0), // 기본 링크 색상
                ..style
            },
            range: TextRange::new(0, len),
            hover_color: Color::rgb(0.5, 0.7, 1.0),
            underline_on_hover_only: false,
        }
    }
}

impl ITextRun for SlateHyperlinkRun {
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
// SlateImageRun — 인라인 이미지 런
// ============================================================================

/// 인라인 이미지 런
///
/// 텍스트 내에 이미지를 삽입합니다. 텍스처 ID로 식별됩니다.
#[derive(Debug, Clone)]
pub struct SlateImageRun {
    /// 이미지 크기
    pub image_size: Vec2,
    /// 텍스처 식별자 (렌더러에서 해석)
    pub texture_id: u64,
    /// 스타일 (baseline 정렬용)
    pub style: TextRunStyle,
    /// 범위
    pub range: TextRange,
    /// 베이스라인 오프셋
    pub baseline_offset: f32,
    /// 틴트 색상
    pub tint_color: Color,
}

impl SlateImageRun {
    /// 새 이미지 런 생성
    pub fn new(texture_id: u64, size: Vec2) -> Self {
        Self {
            image_size: size,
            texture_id,
            style: TextRunStyle::default(),
            range: TextRange::new(0, 1), // ORC 1문자 차지
            baseline_offset: 0.0,
            tint_color: Color::WHITE,
        }
    }

    /// 베이스라인 오프셋 설정
    pub fn with_baseline_offset(mut self, offset: f32) -> Self {
        self.baseline_offset = offset;
        self
    }

    /// 틴트 색상 설정
    pub fn with_tint(mut self, color: Color) -> Self {
        self.tint_color = color;
        self
    }
}

impl ITextRun for SlateImageRun {
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
        self.image_size.x
    }
}

// ============================================================================
// SlatePasswordRun — 패스워드 마스킹 런
// ============================================================================

/// 패스워드 마스킹 런
///
/// 실제 텍스트 대신 마스크 문자(●)를 표시합니다.
#[derive(Debug, Clone)]
pub struct SlatePasswordRun {
    /// 실제 텍스트 (마스킹됨)
    pub text: String,
    /// 마스크 문자 (기본: ●)
    pub mask_char: char,
    /// 스타일
    pub style: TextRunStyle,
    /// 범위
    pub range: TextRange,
}

impl SlatePasswordRun {
    /// 새 패스워드 런 생성
    pub fn new(text: impl Into<String>, style: TextRunStyle) -> Self {
        let t: String = text.into();
        let len = t.len();
        Self {
            text: t,
            mask_char: '●',
            style,
            range: TextRange::new(0, len),
        }
    }

    /// 마스크 문자 설정
    pub fn with_mask_char(mut self, ch: char) -> Self {
        self.mask_char = ch;
        self
    }

    /// 마스킹된 표시 문자열 생성
    pub fn masked_text(&self) -> String {
        self.mask_char.to_string().repeat(self.text.chars().count())
    }
}

impl ITextRun for SlatePasswordRun {
    fn text(&self) -> &str {
        // ITextRun::text()는 표시용 텍스트를 반환해야 하나
        // 마스킹 문자열은 동적이므로 여기서는 원본 반환
        // 렌더러에서 masked_text()를 호출하여 마스킹 처리
        &self.text
    }

    fn style(&self) -> &TextRunStyle {
        &self.style
    }

    fn range(&self) -> TextRange {
        self.range.clone()
    }

    fn measure_width(&self, font_scale: f32) -> f32 {
        // 마스크 문자 하나의 너비 × 문자 수
        let measurer = TextMeasurer::instance();
        if let Ok(m) = measurer.read() {
            let mask_str = self.mask_char.to_string();
            let single_width = m.measure_width_with_selector(
                &mask_str,
                self.style.font_size,
                self.style.font_selector,
                font_scale,
            );
            single_width * self.text.chars().count() as f32
        } else {
            0.0
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::FontSelector;

    #[test]
    fn test_hyperlink_run_default_style() {
        let run = SlateHyperlinkRun::new("Click here", "https://example.com", TextRunStyle::default());
        assert!(run.style.underline);
        assert_eq!(run.url, "https://example.com");
        assert_eq!(run.text(), "Click here");
    }

    #[test]
    fn test_image_run_orc() {
        let run = SlateImageRun::new(42, Vec2::new(16.0, 16.0));
        assert_eq!(run.text(), "\u{FFFC}");
        assert_eq!(run.measure_width(1.0), 16.0);
    }

    #[test]
    fn test_password_run_masking() {
        let run = SlatePasswordRun::new("secret123", TextRunStyle::default());
        assert_eq!(run.masked_text(), "●●●●●●●●●");
        assert_eq!(run.masked_text().chars().count(), 9);
    }

    #[test]
    fn test_password_run_custom_mask() {
        let run = SlatePasswordRun::new("pass", TextRunStyle::default())
            .with_mask_char('*');
        assert_eq!(run.masked_text(), "****");
    }

    #[test]
    fn test_image_run_builder() {
        let run = SlateImageRun::new(1, Vec2::new(24.0, 24.0))
            .with_baseline_offset(4.0)
            .with_tint(Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(run.baseline_offset, 4.0);
        assert_eq!(run.tint_color.r, 1.0);
    }
}
