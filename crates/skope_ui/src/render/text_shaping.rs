//! Text Shaping — 텍스트 셰이핑 캐시 및 셰이퍼 (HarfBuzz 스텁)
//!
//! 텍스트 셰이핑 결과를 캐시하여 동일 텍스트/스타일 조합의
//! 반복 셰이핑을 방지합니다. 향후 HarfBuzz 연동 확장점.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::core::FontSelector;

// ============================================================================
// ShapedTextCacheKey — 캐시 키
// ============================================================================

/// 셰이핑 결과 캐시 키
#[derive(Clone)]
struct ShapedTextCacheKey {
    /// 텍스트 내용
    text: String,
    /// 폰트 셀렉터
    selector: FontSelector,
    /// 폰트 크기 (×100 정수화)
    font_size_key: u32,
    /// 자간 (×100 정수화)
    letter_spacing_key: i32,
}

impl PartialEq for ShapedTextCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.selector == other.selector
            && self.font_size_key == other.font_size_key
            && self.letter_spacing_key == other.letter_spacing_key
    }
}

impl Eq for ShapedTextCacheKey {}

impl Hash for ShapedTextCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.selector.hash(state);
        self.font_size_key.hash(state);
        self.letter_spacing_key.hash(state);
    }
}

impl ShapedTextCacheKey {
    fn new(text: &str, selector: FontSelector, font_size: f32, letter_spacing: f32) -> Self {
        Self {
            text: text.to_string(),
            selector,
            font_size_key: (font_size * 100.0) as u32,
            letter_spacing_key: (letter_spacing * 100.0) as i32,
        }
    }
}

// ============================================================================
// ShapedGlyph — 셰이핑된 개별 글리프
// ============================================================================

/// 셰이핑된 개별 글리프
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// 글리프 ID (폰트 내 인덱스, 0 = 미해결)
    pub glyph_id: u32,
    /// 문자
    pub codepoint: char,
    /// x 전진 거리 (px)
    pub x_advance: f32,
    /// y 전진 거리 (px, 일반적으로 0)
    pub y_advance: f32,
    /// x 오프셋 (px)
    pub x_offset: f32,
    /// y 오프셋 (px)
    pub y_offset: f32,
    /// 소스 텍스트 내 클러스터 인덱스
    pub cluster: u32,
}

// ============================================================================
// ShapedTextResult — 셰이핑 결과
// ============================================================================

/// 텍스트 셰이핑 결과
#[derive(Debug, Clone)]
pub struct ShapedTextResult {
    /// 셰이핑된 글리프 목록
    pub glyphs: Vec<ShapedGlyph>,
    /// 전체 텍스트 너비 (px)
    pub total_width: f32,
}

impl ShapedTextResult {
    /// 빈 결과
    pub fn empty() -> Self {
        Self {
            glyphs: Vec::new(),
            total_width: 0.0,
        }
    }
}

// ============================================================================
// ShapedTextCache — 셰이핑 결과 캐시
// ============================================================================

/// 셰이핑 결과 캐시
///
/// 동일 (text, selector, size, spacing) 조합의 셰이핑 결과를 재사용합니다.
pub struct ShapedTextCache {
    cache: HashMap<ShapedTextCacheKey, ShapedTextResult>,
    /// 최대 캐시 항목 수
    max_entries: usize,
    /// 캐시 히트 카운터
    hit_count: u64,
    /// 캐시 미스 카운터
    miss_count: u64,
}

impl ShapedTextCache {
    /// 새 캐시 생성
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_entries,
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// 캐시 조회
    pub fn get(
        &mut self,
        text: &str,
        selector: FontSelector,
        font_size: f32,
        letter_spacing: f32,
    ) -> Option<&ShapedTextResult> {
        let key = ShapedTextCacheKey::new(text, selector, font_size, letter_spacing);
        if self.cache.contains_key(&key) {
            self.hit_count += 1;
            self.cache.get(&key)
        } else {
            self.miss_count += 1;
            None
        }
    }

    /// 캐시에 결과 저장
    pub fn insert(
        &mut self,
        text: &str,
        selector: FontSelector,
        font_size: f32,
        letter_spacing: f32,
        result: ShapedTextResult,
    ) {
        // 캐시 크기 제한 — 초과 시 전체 클리어 (간단한 전략)
        if self.cache.len() >= self.max_entries {
            self.cache.clear();
        }
        let key = ShapedTextCacheKey::new(text, selector, font_size, letter_spacing);
        self.cache.insert(key, result);
    }

    /// 캐시 무효화
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    /// 캐시된 항목 수
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 캐시가 비어있는지
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 캐시 히트율 (0.0~1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f64 / total as f64
        }
    }
}

impl Default for ShapedTextCache {
    fn default() -> Self {
        Self::new(1024)
    }
}

// ============================================================================
// TextShaper — 텍스트 셰이퍼 (HarfBuzz 스텁)
// ============================================================================

/// 텍스트 셰이퍼 (HarfBuzz 스텁)
///
/// 현재는 단순한 문자별 셰이핑을 수행합니다.
/// 향후 HarfBuzz (harfbuzz-rs) 연동으로 올바른 셰이핑을 지원합니다.
pub struct TextShaper;

impl TextShaper {
    /// 텍스트 셰이핑 (스텁 구현)
    ///
    /// 각 문자를 개별 글리프로 변환합니다.
    /// 합자(ligature), 커닝, 결합 문자 등은 처리하지 않습니다.
    pub fn shape(
        text: &str,
        _selector: FontSelector,
        _font_size: f32,
        char_advance_fn: impl Fn(char) -> f32,
    ) -> ShapedTextResult {
        let mut glyphs = Vec::new();
        let mut total_width = 0.0;

        for (i, ch) in text.chars().enumerate() {
            let advance = char_advance_fn(ch);
            glyphs.push(ShapedGlyph {
                glyph_id: 0, // 스텁: 글리프 ID 미해결
                codepoint: ch,
                x_advance: advance,
                y_advance: 0.0,
                x_offset: 0.0,
                y_offset: 0.0,
                cluster: i as u32,
            });
            total_width += advance;
        }

        ShapedTextResult {
            glyphs,
            total_width,
        }
    }

    /// 셰이핑 결과에서 X 오프셋으로 문자 인덱스 찾기
    pub fn find_index_at_x(result: &ShapedTextResult, x: f32) -> usize {
        let mut accumulated = 0.0;
        for (i, glyph) in result.glyphs.iter().enumerate() {
            let mid = accumulated + glyph.x_advance * 0.5;
            if x < mid {
                return i;
            }
            accumulated += glyph.x_advance;
        }
        result.glyphs.len()
    }

    /// 셰이핑 결과에서 특정 인덱스의 X 위치
    pub fn x_position_at_index(result: &ShapedTextResult, index: usize) -> f32 {
        result.glyphs.iter().take(index).map(|g| g.x_advance).sum()
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
    fn test_shaped_text_cache_basic() {
        let mut cache = ShapedTextCache::new(100);
        assert!(cache.is_empty());

        let result = ShapedTextResult {
            glyphs: vec![],
            total_width: 42.0,
        };

        let sel = FontSelector::default();
        cache.insert("hello", sel, 14.0, 0.0, result);
        assert_eq!(cache.len(), 1);

        let cached = cache.get("hello", sel, 14.0, 0.0);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().total_width, 42.0);
    }

    #[test]
    fn test_shaped_text_cache_miss() {
        let mut cache = ShapedTextCache::new(100);
        let sel = FontSelector::default();
        let result = cache.get("missing", sel, 14.0, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_shaped_text_cache_eviction() {
        let mut cache = ShapedTextCache::new(2);
        let sel = FontSelector::default();
        cache.insert("a", sel, 14.0, 0.0, ShapedTextResult::empty());
        cache.insert("b", sel, 14.0, 0.0, ShapedTextResult::empty());
        assert_eq!(cache.len(), 2);

        // 3번째 삽입 시 전체 클리어 후 삽입
        cache.insert("c", sel, 14.0, 0.0, ShapedTextResult::empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_text_shaper_stub() {
        let sel = FontSelector::default();
        let result = TextShaper::shape("ABC", sel, 14.0, |_| 10.0);
        assert_eq!(result.glyphs.len(), 3);
        assert_eq!(result.total_width, 30.0);
    }

    #[test]
    fn test_find_index_at_x() {
        let sel = FontSelector::default();
        let result = TextShaper::shape("ABCD", sel, 14.0, |_| 10.0);

        assert_eq!(TextShaper::find_index_at_x(&result, 0.0), 0);
        assert_eq!(TextShaper::find_index_at_x(&result, 4.0), 0); // < 5.0 (mid of A)
        assert_eq!(TextShaper::find_index_at_x(&result, 6.0), 1); // > 5.0
        assert_eq!(TextShaper::find_index_at_x(&result, 100.0), 4); // past end
    }

    #[test]
    fn test_x_position_at_index() {
        let sel = FontSelector::default();
        let result = TextShaper::shape("ABC", sel, 14.0, |_| 10.0);

        assert_eq!(TextShaper::x_position_at_index(&result, 0), 0.0);
        assert_eq!(TextShaper::x_position_at_index(&result, 1), 10.0);
        assert_eq!(TextShaper::x_position_at_index(&result, 3), 30.0);
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = ShapedTextCache::new(100);
        assert_eq!(cache.hit_rate(), 0.0);

        let sel = FontSelector::default();
        cache.insert("test", sel, 14.0, 0.0, ShapedTextResult::empty());
        let _ = cache.get("test", sel, 14.0, 0.0); // hit
        let _ = cache.get("missing", sel, 14.0, 0.0); // miss
        assert_eq!(cache.hit_rate(), 0.5);
    }
}
