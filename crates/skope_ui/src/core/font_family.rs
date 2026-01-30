//! Font family types for multi-font support

/// 폰트 패밀리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    /// UI 폰트 (Noto Sans KR 등)
    #[default]
    UI,
    /// 모노스페이스 폰트 (JetBrains Mono + D2Coding 폴백)
    Monospace,
}

// ============================================================================
// FontWeight — 폰트 가중치 (UE5 EFontWeight)
// ============================================================================

/// 폰트 가중치
///
/// CSS font-weight 표준값 기반 (100~900).
/// UE5의 `EFontWeight` 대응.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum FontWeight {
    /// 100 — Thin / Hairline
    Thin = 100,
    /// 200 — Extra Light / Ultra Light
    ExtraLight = 200,
    /// 300 — Light
    Light = 300,
    /// 400 — Regular / Normal (기본값)
    #[default]
    Regular = 400,
    /// 500 — Medium
    Medium = 500,
    /// 600 — Semi Bold / Demi Bold
    SemiBold = 600,
    /// 700 — Bold
    Bold = 700,
    /// 800 — Extra Bold / Ultra Bold
    ExtraBold = 800,
    /// 900 — Black / Heavy
    Black = 900,
}

impl FontWeight {
    /// CSS 수치값 반환
    pub fn value(&self) -> u16 {
        *self as u16
    }

    /// 수치값에서 가장 가까운 가중치 반환
    pub fn from_value(value: u16) -> Self {
        match value {
            0..=150 => Self::Thin,
            151..=250 => Self::ExtraLight,
            251..=350 => Self::Light,
            351..=450 => Self::Regular,
            451..=550 => Self::Medium,
            551..=650 => Self::SemiBold,
            651..=750 => Self::Bold,
            751..=850 => Self::ExtraBold,
            _ => Self::Black,
        }
    }
}

// ============================================================================
// FontStyle — 폰트 스타일
// ============================================================================

/// 폰트 스타일
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    /// 일반
    #[default]
    Normal,
    /// 이탤릭
    Italic,
    /// 오블리크 (기울임)
    Oblique,
}

// ============================================================================
// FontSelector — 폰트 셀렉터 (family + weight + style)
// ============================================================================

/// 폰트 셀렉터: family + weight + style 조합
///
/// 폰트 변형(variant) 체인을 조회하는 키로 사용됩니다.
/// `FontSelector::new(family)` → Regular/Normal 기본 셀렉터.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontSelector {
    pub family: FontFamily,
    pub weight: FontWeight,
    pub style: FontStyle,
}

impl FontSelector {
    /// 기본 셀렉터 (Regular + Normal)
    pub fn new(family: FontFamily) -> Self {
        Self {
            family,
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
        }
    }

    /// 가중치 설정
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// 스타일 설정
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    /// 볼드 여부
    pub fn is_bold(&self) -> bool {
        self.weight >= FontWeight::Bold
    }

    /// 이탤릭 여부
    pub fn is_italic(&self) -> bool {
        self.style == FontStyle::Italic || self.style == FontStyle::Oblique
    }
}

impl Default for FontSelector {
    fn default() -> Self {
        Self::new(FontFamily::UI)
    }
}

impl From<FontFamily> for FontSelector {
    fn from(family: FontFamily) -> Self {
        Self::new(family)
    }
}
