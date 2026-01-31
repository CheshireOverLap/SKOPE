//! Font Settings — 폰트 외곽선, 그림자, 합성 폰트, 힌팅 설정
//!
//! UE5의 FFontOutlineSettings, FSlateFontInfo 확장에 해당합니다.

use glam::Vec2;

use super::color::Color;
use super::font_family::{FontFamily, FontSelector, FontWeight};

// ============================================================================
// FontHinting — 폰트 힌팅 모드
// ============================================================================

/// 폰트 힌팅 모드
///
/// 글리프 래스터라이즈 시 그리드 맞춤 수준을 결정합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontHinting {
    /// 힌팅 없음 (순수 아웃라인)
    None,
    /// 가벼운 힌팅 (수직 축만)
    Light,
    /// 일반 힌팅 (기본값)
    #[default]
    Normal,
    /// 완전 힌팅 (모노크롬)
    Full,
}

// ============================================================================
// FontOutlineSettings — 폰트 외곽선 설정
// ============================================================================

/// 폰트 외곽선 설정 (UE5 FFontOutlineSettings)
///
/// 텍스트 글리프 주위에 외곽선(아웃라인)을 그리는 설정입니다.
#[derive(Debug, Clone)]
pub struct FontOutlineSettings {
    /// 외곽선 두께 (px, 0 = 비활성)
    pub outline_size: f32,
    /// 외곽선 색상
    pub outline_color: Color,
    /// 별도 채우기 색상 사용 여부
    pub separate_fill_alpha: bool,
    /// 외곽선이 글리프 내부로도 적용되는지
    pub apply_outline_to_drop_shadow: bool,
}

impl Default for FontOutlineSettings {
    fn default() -> Self {
        Self {
            outline_size: 0.0,
            outline_color: Color::BLACK,
            separate_fill_alpha: false,
            apply_outline_to_drop_shadow: false,
        }
    }
}

impl FontOutlineSettings {
    /// 비활성 상태인지
    pub fn is_visible(&self) -> bool {
        self.outline_size > 0.0
    }

    /// 외곽선 두께 설정
    pub fn with_size(mut self, size: f32) -> Self {
        self.outline_size = size;
        self
    }

    /// 외곽선 색상 설정
    pub fn with_color(mut self, color: Color) -> Self {
        self.outline_color = color;
        self
    }
}

// ============================================================================
// FontDropShadow — 텍스트 드롭 섀도
// ============================================================================

/// 텍스트 드롭 섀도 설정
///
/// 글리프 뒤에 그림자를 드리우는 설정입니다.
#[derive(Debug, Clone)]
pub struct FontDropShadow {
    /// 그림자 오프셋 (px)
    pub offset: Vec2,
    /// 그림자 색상
    pub color: Color,
    /// 블러 반경 (px, 0 = 하드 섀도)
    pub blur_radius: f32,
}

impl Default for FontDropShadow {
    fn default() -> Self {
        Self {
            offset: Vec2::new(1.0, 1.0),
            color: Color::rgba(0.0, 0.0, 0.0, 0.5),
            blur_radius: 0.0,
        }
    }
}

impl FontDropShadow {
    /// 비활성 상태인지
    pub fn is_visible(&self) -> bool {
        self.color.a > 0.0 && (self.offset.length() > 0.0 || self.blur_radius > 0.0)
    }

    /// 오프셋 설정
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// 색상 설정
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// 블러 반경 설정
    pub fn with_blur(mut self, radius: f32) -> Self {
        self.blur_radius = radius;
        self
    }
}

// ============================================================================
// CompositeFontEntry — 합성 폰트 엔트리
// ============================================================================

/// 합성 폰트 엔트리: 유니코드 범위별 서브 폰트
#[derive(Debug, Clone)]
pub struct CompositeFontEntry {
    /// 이 엔트리가 커버하는 유니코드 범위 (start, end inclusive)
    pub ranges: Vec<(u32, u32)>,
    /// 사용할 폰트 패밀리
    pub family: FontFamily,
    /// 스케일 보정 (기본 1.0)
    pub scale_factor: f32,
}

impl CompositeFontEntry {
    /// 새 엔트리 생성
    pub fn new(family: FontFamily, ranges: Vec<(u32, u32)>) -> Self {
        Self {
            ranges,
            family,
            scale_factor: 1.0,
        }
    }

    /// 해당 코드포인트가 이 엔트리 범위에 포함되는지
    pub fn contains_codepoint(&self, cp: u32) -> bool {
        self.ranges.iter().any(|&(start, end)| cp >= start && cp <= end)
    }
}

// ============================================================================
// CompositeFont — 합성 폰트
// ============================================================================

/// 합성 폰트 (UE5 FCompositeFont)
///
/// 여러 서브 폰트를 유니코드 범위별로 합성하여
/// 하나의 논리적 폰트로 사용합니다.
#[derive(Debug, Clone)]
pub struct CompositeFont {
    /// 기본 폰트 패밀리
    pub default_family: FontFamily,
    /// 유니코드 범위별 서브 폰트 엔트리
    pub sub_fonts: Vec<CompositeFontEntry>,
    /// 폴백 폰트 패밀리 (모든 엔트리에 없을 때)
    pub fallback_family: Option<FontFamily>,
}

impl CompositeFont {
    /// 기본 합성 폰트 생성
    pub fn new(default_family: FontFamily) -> Self {
        Self {
            default_family,
            sub_fonts: Vec::new(),
            fallback_family: None,
        }
    }

    /// 서브 폰트 추가
    pub fn with_sub_font(mut self, entry: CompositeFontEntry) -> Self {
        self.sub_fonts.push(entry);
        self
    }

    /// 폴백 설정
    pub fn with_fallback(mut self, family: FontFamily) -> Self {
        self.fallback_family = Some(family);
        self
    }

    /// 코드포인트에 해당하는 폰트 패밀리 찾기
    pub fn resolve_family(&self, codepoint: char) -> FontFamily {
        let cp = codepoint as u32;
        for entry in &self.sub_fonts {
            if entry.contains_codepoint(cp) {
                return entry.family;
            }
        }
        self.fallback_family.unwrap_or(self.default_family)
    }

    /// 코드포인트에 해당하는 스케일 팩터 찾기
    pub fn resolve_scale(&self, codepoint: char) -> f32 {
        let cp = codepoint as u32;
        for entry in &self.sub_fonts {
            if entry.contains_codepoint(cp) {
                return entry.scale_factor;
            }
        }
        1.0
    }
}

// ============================================================================
// FontMeasureInterface — 폰트 측정 인터페이스
// ============================================================================

/// 폰트 측정 인터페이스 (UE5 FSlateFontMeasure)
///
/// 문자열/문자의 렌더링 크기를 측정하는 추상 인터페이스입니다.
pub trait FontMeasureInterface: Send + Sync {
    /// 문자열 크기 측정
    fn measure_string(
        &self,
        text: &str,
        font_size: f32,
        selector: FontSelector,
        scale: f32,
    ) -> Vec2;

    /// 단일 문자 너비 측정
    fn measure_char_width(
        &self,
        ch: char,
        font_size: f32,
        selector: FontSelector,
        scale: f32,
    ) -> f32;

    /// 문자열에서 특정 X 오프셋에 해당하는 문자 인덱스 찾기
    fn find_char_index_at_offset(
        &self,
        text: &str,
        font_size: f32,
        selector: FontSelector,
        scale: f32,
        x_offset: f32,
    ) -> usize;

    /// 기준선(baseline) 높이
    fn get_baseline(&self, font_size: f32, selector: FontSelector, scale: f32) -> f32;

    /// 최대 문자 높이
    fn get_max_char_height(&self, font_size: f32, selector: FontSelector, scale: f32) -> f32;
}

// ============================================================================
// SlateFontInfo — 확장 폰트 정보 (UE5 FSlateFontInfo)
// ============================================================================

/// 확장 폰트 정보 (UE5 FSlateFontInfo)
///
/// FontSelector를 확장하여 외곽선, 그림자, 합성 폰트 등의
/// 렌더링 관련 설정을 포함합니다.
#[derive(Debug, Clone)]
pub struct SlateFontInfo {
    /// 기본 폰트 셀렉터
    pub selector: FontSelector,
    /// 폰트 크기 (pt)
    pub size: f32,
    /// 자간 (추가 간격, px)
    pub letter_spacing: f32,
    /// 기울임 (skew) 양 (0.0 = 없음, 양수 = 오른쪽 기울임)
    pub skew_amount: f32,
    /// 강제 모노스페이스 여부
    pub force_monospaced: bool,
    /// 힌팅 모드
    pub hinting: FontHinting,
    /// 외곽선 설정
    pub outline: FontOutlineSettings,
    /// 드롭 섀도 설정 (None = 비활성)
    pub drop_shadow: Option<FontDropShadow>,
    /// 합성 폰트 (None = FontSelector만 사용)
    pub composite_font: Option<CompositeFont>,
}

impl Default for SlateFontInfo {
    fn default() -> Self {
        Self {
            selector: FontSelector::default(),
            size: 14.0,
            letter_spacing: 0.0,
            skew_amount: 0.0,
            force_monospaced: false,
            hinting: FontHinting::default(),
            outline: FontOutlineSettings::default(),
            drop_shadow: None,
            composite_font: None,
        }
    }
}

impl SlateFontInfo {
    /// 기본 생성
    pub fn new(selector: FontSelector, size: f32) -> Self {
        Self {
            selector,
            size,
            ..Default::default()
        }
    }

    /// 외곽선 설정
    pub fn with_outline(mut self, outline: FontOutlineSettings) -> Self {
        self.outline = outline;
        self
    }

    /// 드롭 섀도 설정
    pub fn with_drop_shadow(mut self, shadow: FontDropShadow) -> Self {
        self.drop_shadow = Some(shadow);
        self
    }

    /// 자간 설정
    pub fn with_letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    /// 기울임 설정
    pub fn with_skew(mut self, skew: f32) -> Self {
        self.skew_amount = skew;
        self
    }

    /// 강제 모노스페이스 설정
    pub fn with_monospaced(mut self, mono: bool) -> Self {
        self.force_monospaced = mono;
        self
    }

    /// 힌팅 모드 설정
    pub fn with_hinting(mut self, hinting: FontHinting) -> Self {
        self.hinting = hinting;
        self
    }

    /// 합성 폰트 설정
    pub fn with_composite_font(mut self, composite: CompositeFont) -> Self {
        self.composite_font = Some(composite);
        self
    }

    /// 외곽선 활성 여부
    pub fn has_outline(&self) -> bool {
        self.outline.is_visible()
    }

    /// 드롭 섀도 활성 여부
    pub fn has_drop_shadow(&self) -> bool {
        self.drop_shadow.as_ref().map_or(false, |s| s.is_visible())
    }

    /// 볼드 여부
    pub fn is_bold(&self) -> bool {
        self.selector.weight >= FontWeight::Bold
    }

    /// 이탤릭 여부
    pub fn is_italic(&self) -> bool {
        self.selector.is_italic()
    }
}

impl From<FontSelector> for SlateFontInfo {
    fn from(selector: FontSelector) -> Self {
        Self::new(selector, 14.0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn test_font_hinting_default() {
        assert_eq!(FontHinting::default(), FontHinting::Normal);
    }

    #[test]
    fn test_outline_settings_default_invisible() {
        let outline = FontOutlineSettings::default();
        assert!(!outline.is_visible());
    }

    #[test]
    fn test_outline_settings_with_size() {
        let outline = FontOutlineSettings::default().with_size(2.0);
        assert!(outline.is_visible());
        assert_eq!(outline.outline_size, 2.0);
    }

    #[test]
    fn test_drop_shadow_visibility() {
        let shadow = FontDropShadow::default();
        assert!(shadow.is_visible()); // default has offset (1,1) and alpha 0.5

        let no_shadow = FontDropShadow {
            offset: Vec2::ZERO,
            color: Color::rgba(0.0, 0.0, 0.0, 0.0),
            blur_radius: 0.0,
        };
        assert!(!no_shadow.is_visible());
    }

    #[test]
    fn test_composite_font_resolve() {
        let composite = CompositeFont::new(FontFamily::UI)
            .with_sub_font(CompositeFontEntry::new(
                FontFamily::Monospace,
                vec![(0x0000, 0x007F)], // Basic Latin → Monospace
            ));

        assert_eq!(composite.resolve_family('A'), FontFamily::Monospace);
        assert_eq!(composite.resolve_family('가'), FontFamily::UI); // fallback to default
    }

    #[test]
    fn test_composite_font_entry_contains() {
        let entry = CompositeFontEntry::new(FontFamily::UI, vec![(0xAC00, 0xD7AF)]);
        assert!(entry.contains_codepoint(0xAC00)); // 가
        assert!(entry.contains_codepoint(0xD7AF));
        assert!(!entry.contains_codepoint(0x0041)); // A
    }

    #[test]
    fn test_slate_font_info_builder() {
        let info = SlateFontInfo::new(FontSelector::default(), 16.0)
            .with_letter_spacing(1.5)
            .with_skew(0.2)
            .with_monospaced(true)
            .with_hinting(FontHinting::Light)
            .with_outline(FontOutlineSettings::default().with_size(1.0))
            .with_drop_shadow(FontDropShadow::default());

        assert_eq!(info.size, 16.0);
        assert_eq!(info.letter_spacing, 1.5);
        assert_eq!(info.skew_amount, 0.2);
        assert!(info.force_monospaced);
        assert_eq!(info.hinting, FontHinting::Light);
        assert!(info.has_outline());
        assert!(info.has_drop_shadow());
    }

    #[test]
    fn test_slate_font_info_from_selector() {
        let info: SlateFontInfo = FontSelector::default().into();
        assert_eq!(info.size, 14.0);
        assert!(!info.has_outline());
        assert!(!info.has_drop_shadow());
    }
}
