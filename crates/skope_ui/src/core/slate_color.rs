//! SlateColor — 테마 색상 간접 참조
//!
//! UE 참조: `SlateCore/Public/Styling/SlateColor.h`
//!
//! 직접 색상 값 또는 테마에서 동적으로 해석되는 색상을 표현합니다.
//! 테마 전환 시 위젯을 수정하지 않고도 색상이 자동 변경됩니다.

use super::Color;
use crate::theme::ThemeColors;

/// 테마 색상 리졸버 함수 타입 (컴파일 타임 안전)
pub type ThemeColorFn = fn(&ThemeColors) -> Color;

/// 테마 의존적 색상 (직접 값 또는 간접 참조)
///
/// # Examples
/// ```ignore
/// // 직접 색상
/// let red = SlateColor::Specified(Color::RED);
///
/// // 테마에서 해석 (컴파일 타임 안전)
/// let text = SlateColor::FromTheme(|tc| tc.text_primary);
///
/// // 전경색 (부모 컨텍스트에서 자동 해석)
/// let fg = SlateColor::Foreground;
///
/// // 알파 오버라이드
/// let faded = SlateColor::with_alpha(SlateColor::Foreground, 0.5);
/// ```
#[derive(Clone)]
pub enum SlateColor {
    /// 직접 지정된 색상 (테마 독립적)
    Specified(Color),
    /// 전경색 (부모 위젯의 전경색 자동 해석)
    Foreground,
    /// 전경색의 약한 버전 (× 0.6)
    ForegroundSubdued,
    /// 테마에서 함수로 해석 (컴파일 타임 타입 안전)
    FromTheme(ThemeColorFn),
    /// 다른 SlateColor에 알파 값 오버라이드 적용
    WithAlpha(Box<SlateColor>, f32),
}

impl std::fmt::Debug for SlateColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Specified(c) => write!(f, "Specified({:?})", c),
            Self::Foreground => write!(f, "Foreground"),
            Self::ForegroundSubdued => write!(f, "ForegroundSubdued"),
            Self::FromTheme(_) => write!(f, "FromTheme(<fn>)"),
            Self::WithAlpha(base, alpha) => write!(f, "WithAlpha({:?}, {})", base, alpha),
        }
    }
}

impl SlateColor {
    /// 직접 색상으로 생성
    pub fn specified(color: Color) -> Self {
        SlateColor::Specified(color)
    }

    /// 테마 함수로 생성
    pub fn from_theme(f: ThemeColorFn) -> Self {
        SlateColor::FromTheme(f)
    }

    /// 알파 오버라이드 적용
    pub fn with_alpha(base: SlateColor, alpha: f32) -> Self {
        SlateColor::WithAlpha(Box::new(base), alpha)
    }

    /// 테마 의존 여부
    pub fn is_theme_dependent(&self) -> bool {
        match self {
            SlateColor::Specified(_) => false,
            SlateColor::Foreground | SlateColor::ForegroundSubdued => true,
            SlateColor::FromTheme(_) => true,
            SlateColor::WithAlpha(base, _) => base.is_theme_dependent(),
        }
    }

    /// 테마 색상으로 해석
    ///
    /// `foreground_color`는 부모 위젯이 제공하는 전경색 (None이면 기본값 사용)
    pub fn resolve_with_theme(
        &self,
        theme_colors: &ThemeColors,
        foreground_color: Option<Color>,
    ) -> Color {
        match self {
            SlateColor::Specified(color) => *color,
            SlateColor::Foreground => {
                foreground_color.unwrap_or(theme_colors.text_primary)
            }
            SlateColor::ForegroundSubdued => {
                let fg = foreground_color.unwrap_or(theme_colors.text_primary);
                Color::rgba(fg.r * 0.6, fg.g * 0.6, fg.b * 0.6, fg.a)
            }
            SlateColor::FromTheme(f) => f(theme_colors),
            SlateColor::WithAlpha(base, alpha) => {
                let mut color = base.resolve_with_theme(theme_colors, foreground_color);
                color.a = *alpha;
                color
            }
        }
    }

    /// 간단한 해석 (전경색 컨텍스트 없음)
    pub fn resolve_simple(&self, theme_colors: &ThemeColors) -> Color {
        self.resolve_with_theme(theme_colors, None)
    }
}

/// Color로부터 직접 변환 (하위 호환)
impl From<Color> for SlateColor {
    fn from(color: Color) -> Self {
        SlateColor::Specified(color)
    }
}

/// 기본값: 투명
impl Default for SlateColor {
    fn default() -> Self {
        SlateColor::Specified(Color::TRANSPARENT)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme_colors() -> ThemeColors {
        ThemeColors::dark()
    }

    #[test]
    fn test_specified_resolve() {
        let sc = SlateColor::Specified(Color::RED);
        let tc = test_theme_colors();
        let color = sc.resolve_simple(&tc);
        assert_eq!(color, Color::RED);
        assert!(!sc.is_theme_dependent());
    }

    #[test]
    fn test_from_theme_resolve() {
        let sc = SlateColor::from_theme(|tc| tc.accent);
        let tc = test_theme_colors();
        let color = sc.resolve_simple(&tc);
        assert_eq!(color, tc.accent);
        assert!(sc.is_theme_dependent());
    }

    #[test]
    fn test_foreground() {
        let sc = SlateColor::Foreground;
        let tc = test_theme_colors();

        // 전경색 컨텍스트 없음 → text_primary
        let color = sc.resolve_simple(&tc);
        assert_eq!(color, tc.text_primary);

        // 전경색 컨텍스트 있음
        let custom_fg = Color::rgba(1.0, 0.0, 0.0, 1.0);
        let color2 = sc.resolve_with_theme(&tc, Some(custom_fg));
        assert_eq!(color2, custom_fg);

        assert!(sc.is_theme_dependent());
    }

    #[test]
    fn test_foreground_subdued() {
        let sc = SlateColor::ForegroundSubdued;
        let tc = test_theme_colors();
        let fg = Color::rgba(1.0, 1.0, 1.0, 1.0);
        let color = sc.resolve_with_theme(&tc, Some(fg));
        assert!((color.r - 0.6).abs() < 0.01);
        assert!((color.g - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_with_alpha() {
        let base = SlateColor::from_theme(|tc| tc.accent);
        let sc = SlateColor::with_alpha(base, 0.5);
        let tc = test_theme_colors();
        let color = sc.resolve_simple(&tc);
        assert_eq!(color.a, 0.5);
        assert!(sc.is_theme_dependent());
    }

    #[test]
    fn test_from_color() {
        let sc: SlateColor = Color::RED.into();
        match sc {
            SlateColor::Specified(c) => assert_eq!(c, Color::RED),
            _ => panic!("Expected Specified"),
        }
    }
}
