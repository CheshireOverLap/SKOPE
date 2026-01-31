//! Unicode Ranges — 유니코드 블록 범위 정의
//!
//! CompositeFont에서 유니코드 범위별 서브 폰트 매핑에 사용됩니다.

/// 유니코드 블록 범위 상수
///
/// (시작 코드포인트, 끝 코드포인트 inclusive) 형식
pub struct UnicodeBlock;

impl UnicodeBlock {
    /// Basic Latin (U+0000..U+007F)
    pub const BASIC_LATIN: (u32, u32) = (0x0000, 0x007F);

    /// Latin-1 Supplement (U+0080..U+00FF)
    pub const LATIN_1_SUPPLEMENT: (u32, u32) = (0x0080, 0x00FF);

    /// Latin Extended-A (U+0100..U+017F)
    pub const LATIN_EXTENDED_A: (u32, u32) = (0x0100, 0x017F);

    /// Latin Extended-B (U+0180..U+024F)
    pub const LATIN_EXTENDED_B: (u32, u32) = (0x0180, 0x024F);

    /// Cyrillic (U+0400..U+04FF)
    pub const CYRILLIC: (u32, u32) = (0x0400, 0x04FF);

    /// Arabic (U+0600..U+06FF)
    pub const ARABIC: (u32, u32) = (0x0600, 0x06FF);

    /// Devanagari (U+0900..U+097F)
    pub const DEVANAGARI: (u32, u32) = (0x0900, 0x097F);

    /// Thai (U+0E00..U+0E7F)
    pub const THAI: (u32, u32) = (0x0E00, 0x0E7F);

    /// CJK Unified Ideographs (U+4E00..U+9FFF)
    pub const CJK_UNIFIED: (u32, u32) = (0x4E00, 0x9FFF);

    /// CJK Extension A (U+3400..U+4DBF)
    pub const CJK_EXTENSION_A: (u32, u32) = (0x3400, 0x4DBF);

    /// Hiragana (U+3040..U+309F)
    pub const HIRAGANA: (u32, u32) = (0x3040, 0x309F);

    /// Katakana (U+30A0..U+30FF)
    pub const KATAKANA: (u32, u32) = (0x30A0, 0x30FF);

    /// Hangul Jamo (U+1100..U+11FF)
    pub const HANGUL_JAMO: (u32, u32) = (0x1100, 0x11FF);

    /// Hangul Syllables (U+AC00..U+D7AF)
    pub const HANGUL_SYLLABLES: (u32, u32) = (0xAC00, 0xD7AF);

    /// Hangul Compatibility Jamo (U+3130..U+318F)
    pub const HANGUL_COMPATIBILITY_JAMO: (u32, u32) = (0x3130, 0x318F);

    /// Fullwidth Forms (U+FF00..U+FFEF)
    pub const FULLWIDTH_FORMS: (u32, u32) = (0xFF00, 0xFFEF);

    /// General Punctuation (U+2000..U+206F)
    pub const GENERAL_PUNCTUATION: (u32, u32) = (0x2000, 0x206F);

    /// Mathematical Operators (U+2200..U+22FF)
    pub const MATHEMATICAL_OPERATORS: (u32, u32) = (0x2200, 0x22FF);

    /// Box Drawing (U+2500..U+257F)
    pub const BOX_DRAWING: (u32, u32) = (0x2500, 0x257F);

    /// Emoji & Symbols (U+1F300..U+1F9FF)
    pub const EMOJI: (u32, u32) = (0x1F300, 0x1F9FF);

    /// 전체 한중일 범위 (CJK + 한글 + 히라가나 + 가타카나)
    pub const ALL_CJK: [(u32, u32); 6] = [
        Self::CJK_UNIFIED,
        Self::CJK_EXTENSION_A,
        Self::HIRAGANA,
        Self::KATAKANA,
        Self::HANGUL_SYLLABLES,
        Self::HANGUL_JAMO,
    ];

    /// 전체 라틴 범위 (Basic + Extended)
    pub const ALL_LATIN: [(u32, u32); 4] = [
        Self::BASIC_LATIN,
        Self::LATIN_1_SUPPLEMENT,
        Self::LATIN_EXTENDED_A,
        Self::LATIN_EXTENDED_B,
    ];
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_latin_range() {
        let (start, end) = UnicodeBlock::BASIC_LATIN;
        assert!(start <= 'A' as u32 && 'A' as u32 <= end);
        assert!(start <= 'z' as u32 && 'z' as u32 <= end);
    }

    #[test]
    fn test_hangul_range() {
        let (start, end) = UnicodeBlock::HANGUL_SYLLABLES;
        assert!(start <= '가' as u32 && '가' as u32 <= end);
        assert!(start <= '힣' as u32 && '힣' as u32 <= end);
    }

    #[test]
    fn test_cjk_range() {
        let (start, end) = UnicodeBlock::CJK_UNIFIED;
        assert!(start <= '漢' as u32 && '漢' as u32 <= end);
    }

    #[test]
    fn test_all_cjk_includes_hangul() {
        let cp = '가' as u32;
        let found = UnicodeBlock::ALL_CJK.iter().any(|&(s, e)| cp >= s && cp <= e);
        assert!(found);
    }
}
