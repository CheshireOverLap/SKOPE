//! SlateColor — 테마 색상 간접 참조
//!
//! UE 참조: `SlateCore/Public/Styling/SlateColor.h`
//!
//! 직접 색상 값 또는 테마/스타일셋에서 동적으로 해석되는 색상을 표현합니다.
//! 테마 전환 시 위젯을 수정하지 않고도 색상이 자동 변경됩니다.

use super::Color;

/// 테마 의존적 색상 (직접 값 또는 간접 참조)
///
/// # Examples
/// ```ignore
/// // 직접 색상
/// let red = SlateColor::Specified(Color::RED);
///
/// // 테마에서 해석
/// let text = SlateColor::UseTheme("text.primary".into());
///
/// // 알파 오버라이드
/// let faded = SlateColor::with_alpha(SlateColor::UseTheme("accent".into()), 0.5);
/// ```
#[derive(Debug, Clone)]
pub enum SlateColor {
    /// 직접 지정된 색상 (테마 독립적)
    Specified(Color),
    /// 테마 키 이름으로 해석 (EditorTheme.colors 필드명 기반)
    UseTheme(String),
    /// 스타일셋 키 이름으로 해석 (StyleSet.get_color 기반)
    UseStyle(String),
    /// 다른 SlateColor에 알파 값 오버라이드 적용
    WithAlpha(Box<SlateColor>, f32),
}

impl SlateColor {
    /// 직접 색상으로 생성
    pub fn specified(color: Color) -> Self {
        SlateColor::Specified(color)
    }

    /// 테마 키로 생성
    pub fn use_theme(key: impl Into<String>) -> Self {
        SlateColor::UseTheme(key.into())
    }

    /// 스타일셋 키로 생성
    pub fn use_style(key: impl Into<String>) -> Self {
        SlateColor::UseStyle(key.into())
    }

    /// 알파 오버라이드 적용
    pub fn with_alpha(base: SlateColor, alpha: f32) -> Self {
        SlateColor::WithAlpha(Box::new(base), alpha)
    }

    /// 테마/스타일셋 의존 여부
    pub fn is_theme_dependent(&self) -> bool {
        match self {
            SlateColor::Specified(_) => false,
            SlateColor::UseTheme(_) => true,
            SlateColor::UseStyle(_) => true,
            SlateColor::WithAlpha(base, _) => base.is_theme_dependent(),
        }
    }

    /// 색상 해석 (테마와 선택적 스타일셋으로부터)
    ///
    /// 해석 실패 시 `Color::MAGENTA`를 반환합니다 (디버깅 가시성).
    pub fn resolve(
        &self,
        theme_resolver: &dyn Fn(&str) -> Option<Color>,
        style_resolver: Option<&dyn Fn(&str) -> Option<Color>>,
    ) -> Color {
        match self {
            SlateColor::Specified(color) => *color,
            SlateColor::UseTheme(key) => {
                theme_resolver(key).unwrap_or(Color::MAGENTA)
            }
            SlateColor::UseStyle(key) => {
                style_resolver
                    .and_then(|resolver| resolver(key))
                    .or_else(|| theme_resolver(key))
                    .unwrap_or(Color::MAGENTA)
            }
            SlateColor::WithAlpha(base, alpha) => {
                let mut color = base.resolve(theme_resolver, style_resolver);
                color.a = *alpha;
                color
            }
        }
    }

    /// 테마 색상 맵으로부터 간단히 해석 (스타일셋 없음)
    ///
    /// `resolve_color_map`은 키 이름 → Color 매핑 함수입니다.
    pub fn resolve_simple(&self, resolve_color: &dyn Fn(&str) -> Option<Color>) -> Color {
        self.resolve(resolve_color, None)
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

// ── 편의 매크로 ──

/// 테마 키 이름으로 `SlateColor` 생성
///
/// ```ignore
/// let color = style_color!("text.primary");
/// ```
#[macro_export]
macro_rules! style_color {
    ($key:expr) => {
        $crate::core::SlateColor::UseTheme($key.into())
    };
}

/// 스타일셋 키 이름으로 `SlateColor` 생성
///
/// ```ignore
/// let color = style_set_color!("Button.bg");
/// ```
#[macro_export]
macro_rules! style_set_color {
    ($key:expr) => {
        $crate::core::SlateColor::UseStyle($key.into())
    };
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_theme_resolver() -> impl Fn(&str) -> Option<Color> {
        |key: &str| match key {
            "text.primary" => Some(Color::WHITE),
            "accent" => Some(Color::rgba(0.25, 0.56, 0.87, 1.0)),
            "danger" => Some(Color::rgba(0.8, 0.2, 0.2, 1.0)),
            _ => None,
        }
    }

    fn make_style_resolver() -> impl Fn(&str) -> Option<Color> {
        |key: &str| match key {
            "Button.bg" => Some(Color::rgba(0.2, 0.2, 0.2, 1.0)),
            "text.primary" => Some(Color::rgba(0.9, 0.9, 0.9, 1.0)), // style overrides theme
            _ => None,
        }
    }

    #[test]
    fn test_specified_resolve() {
        let sc = SlateColor::Specified(Color::RED);
        let resolver = make_theme_resolver();
        let color = sc.resolve(&resolver, None);
        assert_eq!(color, Color::RED);
        assert!(!sc.is_theme_dependent());
    }

    #[test]
    fn test_theme_resolve() {
        let sc = SlateColor::use_theme("text.primary");
        let resolver = make_theme_resolver();
        let color = sc.resolve(&resolver, None);
        assert_eq!(color, Color::WHITE);
        assert!(sc.is_theme_dependent());
    }

    #[test]
    fn test_theme_resolve_missing_key() {
        let sc = SlateColor::use_theme("nonexistent");
        let resolver = make_theme_resolver();
        let color = sc.resolve(&resolver, None);
        assert_eq!(color, Color::MAGENTA); // fallback
    }

    #[test]
    fn test_style_resolve_overrides_theme() {
        let sc = SlateColor::use_style("text.primary");
        let theme = make_theme_resolver();
        let style = make_style_resolver();
        let color = sc.resolve(&theme, Some(&style));
        // StyleSet 값이 우선
        assert_eq!(color, Color::rgba(0.9, 0.9, 0.9, 1.0));
    }

    #[test]
    fn test_style_resolve_fallback_to_theme() {
        let sc = SlateColor::use_style("accent");
        let theme = make_theme_resolver();
        let style = make_style_resolver();
        let color = sc.resolve(&theme, Some(&style));
        // StyleSet에 없으므로 테마에서 가져옴
        assert_eq!(color, Color::rgba(0.25, 0.56, 0.87, 1.0));
    }

    #[test]
    fn test_with_alpha() {
        let base = SlateColor::use_theme("accent");
        let sc = SlateColor::with_alpha(base, 0.5);
        let resolver = make_theme_resolver();
        let color = sc.resolve(&resolver, None);
        assert_eq!(color.r, 0.25);
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

    #[test]
    fn test_macro_style_color() {
        let sc = style_color!("text.primary");
        match sc {
            SlateColor::UseTheme(key) => assert_eq!(key, "text.primary"),
            _ => panic!("Expected UseTheme"),
        }
    }

    #[test]
    fn test_macro_style_set_color() {
        let sc = style_set_color!("Button.bg");
        match sc {
            SlateColor::UseStyle(key) => assert_eq!(key, "Button.bg"),
            _ => panic!("Expected UseStyle"),
        }
    }
}
