//! BiDi Support — 양방향 텍스트 지원 (UAX#9 스텁)
//!
//! 좌횡서(LTR)와 우횡서(RTL) 텍스트가 혼합된 양방향 텍스트를
//! 올바르게 레이아웃하기 위한 기본 인프라입니다.
//! 현재는 간소화된 스텁 구현이며, 향후 unicode-bidi crate 연동 가능.

use crate::core::FlowDirection;

// ============================================================================
// BidiLevel
// ============================================================================

/// BiDi 임베딩 레벨
///
/// 짝수 레벨 = LTR, 홀수 레벨 = RTL
pub type BidiLevel = u8;

/// 기본 LTR 레벨
pub const BIDI_LEVEL_LTR: BidiLevel = 0;
/// 기본 RTL 레벨
pub const BIDI_LEVEL_RTL: BidiLevel = 1;

// ============================================================================
// BidiCharType — 문자의 BiDi 카테고리
// ============================================================================

/// 문자의 BiDi 카테고리 (간소화)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiCharType {
    /// 강한 좌횡서 (Latin, CJK, etc.)
    StrongLTR,
    /// 강한 우횡서 (Arabic, Hebrew)
    StrongRTL,
    /// 약한 타입 (숫자)
    Weak,
    /// 중립 (공백, 구두점)
    Neutral,
}

/// 문자의 BiDi 카테고리 결정 (간소화)
pub fn classify_bidi_char(ch: char) -> BidiCharType {
    let cp = ch as u32;
    match cp {
        // Arabic
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF |
        0xFB50..=0xFDFF | 0xFE70..=0xFEFF => BidiCharType::StrongRTL,
        // Hebrew
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => BidiCharType::StrongRTL,
        // Thaana, Syriac, etc.
        0x0700..=0x074F | 0x0780..=0x07BF => BidiCharType::StrongRTL,
        // Numbers (European digits)
        0x0030..=0x0039 => BidiCharType::Weak,
        // Whitespace and common punctuation
        _ if ch.is_whitespace() => BidiCharType::Neutral,
        _ if ch.is_ascii_punctuation() => BidiCharType::Neutral,
        // Everything else is LTR
        _ => BidiCharType::StrongLTR,
    }
}

// ============================================================================
// BidiRun — BiDi 런
// ============================================================================

/// 동일 BiDi 레벨을 가진 연속 텍스트 구간
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// 소스 텍스트 내 시작 인덱스 (char 단위)
    pub start: usize,
    /// 끝 인덱스 (char 단위, exclusive)
    pub end: usize,
    /// BiDi 임베딩 레벨
    pub level: BidiLevel,
}

impl BidiRun {
    /// 이 런이 RTL인지
    pub fn is_rtl(&self) -> bool {
        self.level % 2 == 1
    }

    /// 이 런이 LTR인지
    pub fn is_ltr(&self) -> bool {
        self.level % 2 == 0
    }

    /// 런 길이 (문자 수)
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// 빈 런인지
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// FlowDirection 반환
    pub fn flow_direction(&self) -> FlowDirection {
        if self.is_rtl() {
            FlowDirection::RightToLeft
        } else {
            FlowDirection::LeftToRight
        }
    }
}

// ============================================================================
// analyze_bidi — BiDi 분석 (간소화 스텁)
// ============================================================================

/// 텍스트의 BiDi 런 분석 (간소화 UAX#9 스텁)
///
/// 현재는 간단한 강한 문자 기반 분할만 수행합니다.
/// 완전한 UAX#9 구현은 unicode-bidi crate를 사용하세요.
///
/// # Arguments
/// * `text` — 분석할 텍스트
/// * `base_direction` — 기본 텍스트 방향 (None = 자동 감지)
///
/// # Returns
/// 정렬 순서대로 정렬된 BiDi 런 목록
pub fn analyze_bidi(text: &str, base_direction: Option<FlowDirection>) -> Vec<BidiRun> {
    if text.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();

    // 기본 방향 결정
    let base_level = match base_direction {
        Some(FlowDirection::RightToLeft) => BIDI_LEVEL_RTL,
        Some(FlowDirection::LeftToRight) => BIDI_LEVEL_LTR,
        None => detect_base_direction(&chars),
    };

    // 문자별 레벨 할당 (간소화: 강한 문자 기반)
    let levels: Vec<BidiLevel> = chars.iter().map(|&ch| {
        match classify_bidi_char(ch) {
            BidiCharType::StrongRTL => BIDI_LEVEL_RTL,
            BidiCharType::StrongLTR => BIDI_LEVEL_LTR,
            BidiCharType::Weak | BidiCharType::Neutral => base_level,
        }
    }).collect();

    // 동일 레벨 연속 구간을 런으로 분할
    let mut runs: Vec<BidiRun> = Vec::new();
    let mut run_start = 0;

    for i in 1..levels.len() {
        if levels[i] != levels[i - 1] {
            runs.push(BidiRun {
                start: run_start,
                end: i,
                level: levels[run_start],
            });
            run_start = i;
        }
    }
    // 마지막 런
    runs.push(BidiRun {
        start: run_start,
        end: levels.len(),
        level: levels[run_start],
    });

    runs
}

/// 텍스트 시각 순서로 BiDi 런 재정렬 (간소화)
///
/// 기본 레벨이 LTR이면 RTL 런만 뒤집습니다.
pub fn reorder_bidi_runs(runs: &[BidiRun]) -> Vec<usize> {
    // 간소화: 원본 순서 인덱스 반환
    // 완전한 구현에서는 최대 레벨부터 순차적으로 뒤집기
    let max_level = runs.iter().map(|r| r.level).max().unwrap_or(0);
    let mut indices: Vec<usize> = (0..runs.len()).collect();

    // 레벨별 역순화 (Bidi Reordering Algorithm L2)
    let mut level = max_level;
    while level > 0 {
        let mut i = 0;
        while i < indices.len() {
            if runs[indices[i]].level >= level {
                // 연속된 같은 레벨 이상 런 찾기
                let start = i;
                while i < indices.len() && runs[indices[i]].level >= level {
                    i += 1;
                }
                // 역순화
                indices[start..i].reverse();
            } else {
                i += 1;
            }
        }
        level -= 1;
    }

    indices
}

/// 첫 강한 문자 기반 기본 방향 감지
fn detect_base_direction(chars: &[char]) -> BidiLevel {
    for &ch in chars {
        match classify_bidi_char(ch) {
            BidiCharType::StrongLTR => return BIDI_LEVEL_LTR,
            BidiCharType::StrongRTL => return BIDI_LEVEL_RTL,
            _ => continue,
        }
    }
    BIDI_LEVEL_LTR // 기본값: LTR
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_bidi_latin() {
        assert_eq!(classify_bidi_char('A'), BidiCharType::StrongLTR);
        assert_eq!(classify_bidi_char('z'), BidiCharType::StrongLTR);
    }

    #[test]
    fn test_classify_bidi_arabic() {
        assert_eq!(classify_bidi_char('\u{0627}'), BidiCharType::StrongRTL); // Arabic Alef
    }

    #[test]
    fn test_classify_bidi_hebrew() {
        assert_eq!(classify_bidi_char('\u{05D0}'), BidiCharType::StrongRTL); // Hebrew Alef
    }

    #[test]
    fn test_classify_bidi_neutral() {
        assert_eq!(classify_bidi_char(' '), BidiCharType::Neutral);
        assert_eq!(classify_bidi_char('.'), BidiCharType::Neutral);
    }

    #[test]
    fn test_classify_bidi_number() {
        assert_eq!(classify_bidi_char('0'), BidiCharType::Weak);
        assert_eq!(classify_bidi_char('9'), BidiCharType::Weak);
    }

    #[test]
    fn test_analyze_pure_ltr() {
        let runs = analyze_bidi("Hello World", None);
        assert_eq!(runs.len(), 1);
        assert!(runs[0].is_ltr());
    }

    #[test]
    fn test_analyze_pure_rtl() {
        let runs = analyze_bidi("\u{0627}\u{0628}\u{0629}", None); // Arabic
        assert_eq!(runs.len(), 1);
        assert!(runs[0].is_rtl());
    }

    #[test]
    fn test_analyze_mixed() {
        // "Hello" + Arabic + "World"
        let text = "Hello \u{0627}\u{0628} World";
        let runs = analyze_bidi(text, None);
        assert!(runs.len() >= 2); // At least LTR and RTL runs
    }

    #[test]
    fn test_analyze_empty() {
        let runs = analyze_bidi("", None);
        assert!(runs.is_empty());
    }

    #[test]
    fn test_bidi_run_direction() {
        let ltr = BidiRun { start: 0, end: 5, level: 0 };
        let rtl = BidiRun { start: 0, end: 5, level: 1 };
        assert!(ltr.is_ltr());
        assert!(!ltr.is_rtl());
        assert!(rtl.is_rtl());
        assert!(!rtl.is_ltr());
    }

    #[test]
    fn test_reorder_pure_ltr() {
        let runs = vec![
            BidiRun { start: 0, end: 5, level: 0 },
        ];
        let order = reorder_bidi_runs(&runs);
        assert_eq!(order, vec![0]);
    }

    #[test]
    fn test_reorder_ltr_rtl_ltr() {
        let runs = vec![
            BidiRun { start: 0, end: 5, level: 0 },
            BidiRun { start: 5, end: 8, level: 1 },
            BidiRun { start: 8, end: 13, level: 0 },
        ];
        let order = reorder_bidi_runs(&runs);
        // RTL run should be reversed in visual order
        // For level 1: runs at indices with level >= 1 are the middle one
        assert_eq!(order.len(), 3);
    }
}
