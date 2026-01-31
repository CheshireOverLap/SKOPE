//! Font Metrics Cache — 폰트 메트릭스 캐시 시스템
//!
//! 폰트+사이즈 조합별 메트릭스(ascent, descent, line_gap 등)를 캐시하여
//! TextLayout 엔진에서 빠르게 참조할 수 있게 합니다.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

use crate::core::{FontFamily, FontSelector};

// ============================================================================
// FontMetrics — 캐시된 폰트 메트릭스
// ============================================================================

/// 캐시된 폰트 메트릭스 (font+size 조합별)
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// 베이스라인 위 높이 (양수)
    pub ascent: f32,
    /// 베이스라인 아래 깊이 (양수로 표현)
    pub descent: f32,
    /// 라인 간 간격
    pub line_gap: f32,
    /// 전체 라인 높이 (ascent + descent + line_gap)
    pub line_height: f32,
    /// 소문자 x 높이
    pub x_height: f32,
    /// 대문자 높이
    pub cap_height: f32,
    /// 밑줄 오프셋 (베이스라인 기준, 양수 = 아래)
    pub underline_offset: f32,
    /// 밑줄 두께
    pub underline_thickness: f32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            ascent: 12.0,
            descent: 4.0,
            line_gap: 0.0,
            line_height: 16.0,
            x_height: 8.0,
            cap_height: 10.0,
            underline_offset: 2.0,
            underline_thickness: 1.0,
        }
    }
}

// ============================================================================
// FontMetricsCache — 메트릭스 캐시 (싱글톤)
// ============================================================================

/// 캐시 키: (FontSelector, font_size * 100 → u32)
type MetricsCacheKey = (FontSelector, u32);

/// 폰트 사이즈를 캐시 키로 변환 (소수점 2자리까지)
fn font_size_key(size: f32) -> u32 {
    (size * 100.0) as u32
}

/// 폰트 메트릭스 캐시
///
/// `FontSelector + font_size` 조합별로 메트릭스를 캐시합니다.
/// TextLayout 엔진에서 라인 높이, 베이스라인 계산 시 사용.
pub struct FontMetricsCache {
    cache: HashMap<MetricsCacheKey, FontMetrics>,
}

impl FontMetricsCache {
    /// 싱글톤 인스턴스
    pub fn instance() -> &'static RwLock<FontMetricsCache> {
        static INSTANCE: OnceLock<RwLock<FontMetricsCache>> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            RwLock::new(FontMetricsCache {
                cache: HashMap::new(),
            })
        })
    }

    /// 메트릭스 조회 (캐시 히트) 또는 계산
    pub fn get_or_compute(
        &mut self,
        selector: FontSelector,
        font_size: f32,
        font_chains: &HashMap<FontFamily, Vec<Vec<u8>>>,
    ) -> FontMetrics {
        let key = (selector, font_size_key(font_size));

        if let Some(metrics) = self.cache.get(&key) {
            return *metrics;
        }

        let metrics = Self::compute_metrics(selector, font_size, font_chains);
        self.cache.insert(key, metrics);
        metrics
    }

    /// 캐시 무효화 (폰트 변경 시)
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    /// 캐시된 메트릭스 수
    pub fn cached_count(&self) -> usize {
        self.cache.len()
    }

    /// 폰트 메트릭스 계산
    fn compute_metrics(
        selector: FontSelector,
        font_size: f32,
        font_chains: &HashMap<FontFamily, Vec<Vec<u8>>>,
    ) -> FontMetrics {
        // 폰트 체인에서 첫 번째 폰트 데이터 가져오기
        let font_data = font_chains
            .get(&selector.family)
            .and_then(|chain| chain.first());

        let Some(data) = font_data else {
            // 폰트 데이터 없으면 기본값 (스케일 적용)
            return FontMetrics {
                ascent: font_size * 0.8,
                descent: font_size * 0.2,
                line_gap: 0.0,
                line_height: font_size,
                x_height: font_size * 0.5,
                cap_height: font_size * 0.7,
                underline_offset: font_size * 0.15,
                underline_thickness: (font_size * 0.07).max(1.0),
            };
        };

        let Ok(font) = FontRef::try_from_slice(data) else {
            return FontMetrics::default();
        };

        let scale = PxScale::from(font_size);
        let scaled = font.as_scaled(scale);

        let ascent = scaled.ascent();
        let descent = scaled.descent().abs();
        let line_gap = scaled.line_gap();
        let line_height = ascent + descent + line_gap;

        // x_height: 'x' 글리프의 높이, 없으면 추정
        let x_height = {
            let glyph_id = font.glyph_id('x');
            let glyph = glyph_id.with_scale(scale);
            font.outline_glyph(glyph)
                .map(|outlined| outlined.px_bounds().height())
                .unwrap_or(ascent * 0.5)
        };

        // cap_height: 'H' 글리프의 높이, 없으면 추정
        let cap_height = {
            let glyph_id = font.glyph_id('H');
            let glyph = glyph_id.with_scale(scale);
            font.outline_glyph(glyph)
                .map(|outlined| outlined.px_bounds().height())
                .unwrap_or(ascent * 0.7)
        };

        // underline: 일반적으로 descent의 중간
        let underline_offset = descent * 0.5;
        let underline_thickness = (font_size * 0.07).max(1.0);

        FontMetrics {
            ascent,
            descent,
            line_gap,
            line_height,
            x_height,
            cap_height,
            underline_offset,
            underline_thickness,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_size_key() {
        assert_eq!(font_size_key(14.0), 1400);
        assert_eq!(font_size_key(12.5), 1250);
        assert_eq!(font_size_key(0.0), 0);
    }

    #[test]
    fn test_default_metrics() {
        let m = FontMetrics::default();
        assert!(m.line_height > 0.0);
        assert!(m.ascent > 0.0);
        assert!(m.descent > 0.0);
    }

    #[test]
    fn test_compute_fallback_no_font() {
        let chains: HashMap<FontFamily, Vec<Vec<u8>>> = HashMap::new();
        let metrics = FontMetricsCache::compute_metrics(
            FontSelector::default(),
            14.0,
            &chains,
        );
        assert!((metrics.ascent - 11.2).abs() < 0.1); // 14 * 0.8
        assert!((metrics.descent - 2.8).abs() < 0.1); // 14 * 0.2
    }
}
