//! Text Layout Engine — 텍스트 레이아웃 (UE5 FTextLayout)
//!
//! ITextRun 목록을 받아 줄바꿈, 글리프 배치, 멀티라인 레이아웃을 수행합니다.
//! WordWrap, CharWrap, NoWrap 모드를 지원합니다.

use glam::Vec2;

use crate::core::FontSelector;
use super::text_run::{ITextRun, TextRunStyle};
use super::font_metrics::{FontMetrics, FontMetricsCache};
use super::text_renderer::TextMeasurer;

// ============================================================================
// ShapedGlyphEntry — 배치된 글리프
// ============================================================================

/// 배치된 글리프 (레이아웃 결과의 최소 단위)
#[derive(Debug, Clone)]
pub struct ShapedGlyphEntry {
    /// 문자
    pub codepoint: char,
    /// 배치 위치 (라인 원점 기준)
    pub position: Vec2,
    /// 가로 전진 거리
    pub advance: f32,
    /// 사용 폰트 셀렉터
    pub font_selector: FontSelector,
    /// 폰트 크기
    pub font_size: f32,
    /// 색상 [r, g, b, a]
    pub color: [f32; 4],
    /// 소속 런 인덱스
    pub run_index: usize,
}

// ============================================================================
// ShapedTextLine — 배치된 텍스트 라인
// ============================================================================

/// 배치된 텍스트 라인
#[derive(Debug, Clone)]
pub struct ShapedTextLine {
    /// 이 라인의 글리프들
    pub glyphs: Vec<ShapedGlyphEntry>,
    /// 라인 원점 (전체 레이아웃 좌상단 기준)
    pub line_origin: Vec2,
    /// 어센트 (베이스라인 위)
    pub ascent: f32,
    /// 디센트 (베이스라인 아래)
    pub descent: f32,
    /// 라인 높이
    pub line_height: f32,
    /// 라인 너비
    pub width: f32,
    /// 소속 런 범위 (start, end — exclusive)
    pub run_range: (usize, usize),
}

// ============================================================================
// LineBreakMode
// ============================================================================

/// 줄바꿈 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineBreakMode {
    /// 줄바꿈 없음
    #[default]
    NoWrap,
    /// 단어 단위 줄바꿈
    WordWrap,
    /// 문자 단위 줄바꿈
    CharWrap,
}

// ============================================================================
// TextLayoutParams
// ============================================================================

/// 레이아웃 파라미터
#[derive(Debug, Clone)]
pub struct TextLayoutParams {
    /// 최대 너비 (f32::INFINITY = 무제한)
    pub max_width: f32,
    /// 줄바꿈 모드
    pub line_break_mode: LineBreakMode,
    /// 줄 높이 배율 (기본 1.2)
    pub line_height_ratio: f32,
    /// 최대 줄 수 (0 = 무제한)
    pub max_lines: usize,
    /// 폰트 스케일 (DPI 보정)
    pub font_scale: f32,
}

impl Default for TextLayoutParams {
    fn default() -> Self {
        Self {
            max_width: f32::INFINITY,
            line_break_mode: LineBreakMode::NoWrap,
            line_height_ratio: 1.2,
            max_lines: 0,
            font_scale: 1.0,
        }
    }
}

// ============================================================================
// TextLayoutResult
// ============================================================================

/// 레이아웃 결과
#[derive(Debug, Clone)]
pub struct TextLayoutResult {
    /// 배치된 라인들
    pub lines: Vec<ShapedTextLine>,
    /// 전체 크기
    pub total_size: Vec2,
}

impl TextLayoutResult {
    /// 빈 결과
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            total_size: Vec2::ZERO,
        }
    }

    /// 라인 수
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 전체 글리프 수
    pub fn glyph_count(&self) -> usize {
        self.lines.iter().map(|l| l.glyphs.len()).sum()
    }
}

// ============================================================================
// TextLayout — 레이아웃 엔진
// ============================================================================

/// 텍스트 레이아웃 엔진 (UE5 FTextLayout)
///
/// ITextRun 목록으로부터 멀티라인 글리프 배치를 생성합니다.
pub struct TextLayout;

impl TextLayout {
    /// 런 목록 → 라인 배치
    ///
    /// 각 런의 텍스트를 문자별로 advance 측정하고,
    /// `max_width` 초과 시 줄바꿈 모드에 따라 라인을 분리합니다.
    pub fn layout(runs: &[Box<dyn ITextRun>], params: &TextLayoutParams) -> TextLayoutResult {
        if runs.is_empty() {
            return TextLayoutResult::empty();
        }

        let measurer = TextMeasurer::instance();
        let measurer_guard = match measurer.read() {
            Ok(g) => g,
            Err(_) => return TextLayoutResult::empty(),
        };

        let metrics_cache = FontMetricsCache::instance();
        let mut metrics_guard = match metrics_cache.write() {
            Ok(g) => g,
            Err(_) => return TextLayoutResult::empty(),
        };

        let font_chains = measurer_guard.font_chains();

        // 전체 문자+메타데이터 수집
        let mut fragments: Vec<GlyphFragment> = Vec::new();

        for (run_idx, run) in runs.iter().enumerate() {
            let style = run.style();
            let text = run.text();
            let scaled_font_size = style.font_size * params.font_scale;

            for ch in text.chars() {
                let advance = if ch == '\n' {
                    0.0
                } else {
                    measure_char_advance(
                        &measurer_guard,
                        ch,
                        style.font_size,
                        style.font_selector,
                        params.font_scale,
                    ) + style.letter_spacing
                };

                fragments.push(GlyphFragment {
                    ch,
                    advance,
                    font_selector: style.font_selector,
                    font_size: scaled_font_size,
                    color: style.color.to_array(),
                    run_index: run_idx,
                    is_newline: ch == '\n',
                    is_whitespace: ch.is_whitespace(),
                });
            }
        }

        // 라인 분할
        let line_fragments = Self::break_into_lines(&fragments, params);

        // 라인별 글리프 배치
        let mut lines: Vec<ShapedTextLine> = Vec::new();
        let mut y_cursor: f32 = 0.0;
        let max_lines = if params.max_lines == 0 { usize::MAX } else { params.max_lines };
        let mut max_width: f32 = 0.0;

        for frags in line_fragments.iter() {
            if lines.len() >= max_lines {
                break;
            }

            // 이 라인에서 가장 큰 메트릭스 결정
            let mut line_ascent: f32 = 0.0;
            let mut line_descent: f32 = 0.0;
            let mut line_gap: f32 = 0.0;

            // 런별로 메트릭스 수집
            let mut seen_selectors: Vec<(FontSelector, f32)> = Vec::new();
            for frag in frags.iter() {
                let key = (frag.font_selector, frag.font_size);
                if !seen_selectors.contains(&key) {
                    seen_selectors.push(key);
                }
            }

            if seen_selectors.is_empty() {
                // 빈 라인 (줄바꿈만) — 기본 메트릭스 사용
                let default_metrics = FontMetrics::default();
                line_ascent = default_metrics.ascent;
                line_descent = default_metrics.descent;
            } else {
                for (sel, _font_size) in &seen_selectors {
                    let metrics = metrics_guard.get_or_compute(
                        *sel,
                        *_font_size / params.font_scale, // 원래 폰트 크기
                        font_chains,
                    );
                    line_ascent = line_ascent.max(metrics.ascent);
                    line_descent = line_descent.max(metrics.descent);
                    line_gap = line_gap.max(metrics.line_gap);
                }
            }

            let base_line_height = line_ascent + line_descent + line_gap;
            let line_height = base_line_height * params.line_height_ratio;

            // 글리프 배치
            let mut glyphs: Vec<ShapedGlyphEntry> = Vec::new();
            let mut x_cursor: f32 = 0.0;
            let mut min_run = usize::MAX;
            let mut max_run = 0usize;

            for frag in frags.iter() {
                if frag.is_newline {
                    continue;
                }

                min_run = min_run.min(frag.run_index);
                max_run = max_run.max(frag.run_index);

                glyphs.push(ShapedGlyphEntry {
                    codepoint: frag.ch,
                    position: Vec2::new(x_cursor, line_ascent),
                    advance: frag.advance,
                    font_selector: frag.font_selector,
                    font_size: frag.font_size,
                    color: frag.color,
                    run_index: frag.run_index,
                });

                x_cursor += frag.advance;
            }

            let line_width = x_cursor;
            max_width = max_width.max(line_width);

            let run_range = if min_run <= max_run {
                (min_run, max_run + 1)
            } else {
                (0, 0)
            };

            lines.push(ShapedTextLine {
                glyphs,
                line_origin: Vec2::new(0.0, y_cursor),
                ascent: line_ascent,
                descent: line_descent,
                line_height,
                width: line_width,
                run_range,
            });

            y_cursor += line_height;
        }

        let total_height = y_cursor;
        TextLayoutResult {
            lines,
            total_size: Vec2::new(max_width, total_height),
        }
    }

    /// 단일 스타일 편의 메서드
    pub fn layout_simple(
        text: &str,
        style: &TextRunStyle,
        params: &TextLayoutParams,
    ) -> TextLayoutResult {
        use super::text_run::{FSlateTextRun, TextRange};

        if text.is_empty() {
            return TextLayoutResult::empty();
        }

        let run = FSlateTextRun::new(
            text.to_string(),
            style.clone(),
            TextRange::new(0, text.len()),
        );
        let runs: Vec<Box<dyn ITextRun>> = vec![Box::new(run)];
        Self::layout(&runs, params)
    }

    /// 프래그먼트를 라인으로 분할
    fn break_into_lines(
        fragments: &[GlyphFragment],
        params: &TextLayoutParams,
    ) -> Vec<Vec<GlyphFragment>> {
        if fragments.is_empty() {
            return vec![Vec::new()];
        }

        let mut lines: Vec<Vec<GlyphFragment>> = Vec::new();
        let mut current_line: Vec<GlyphFragment> = Vec::new();
        let mut line_width: f32 = 0.0;

        // 마지막 단어 경계 추적 (WordWrap용)
        let mut last_break_pos: Option<usize> = None; // current_line 내 인덱스

        for frag in fragments {
            // 명시적 줄바꿈
            if frag.is_newline {
                current_line.push(frag.clone());
                lines.push(std::mem::take(&mut current_line));
                line_width = 0.0;
                last_break_pos = None;
                continue;
            }

            // NoWrap — 줄바꿈 없이 계속 추가
            if params.line_break_mode == LineBreakMode::NoWrap {
                current_line.push(frag.clone());
                line_width += frag.advance;
                continue;
            }

            let new_width = line_width + frag.advance;

            // 단어 경계 추적
            if frag.is_whitespace || is_cjk_char(frag.ch) {
                last_break_pos = Some(current_line.len());

            }

            // 최대 너비 초과 여부
            if new_width > params.max_width && !current_line.is_empty() {
                match params.line_break_mode {
                    LineBreakMode::WordWrap => {
                        if let Some(break_pos) = last_break_pos {
                            // 단어 경계에서 분할
                            let remainder: Vec<GlyphFragment> =
                                current_line.split_off(break_pos);
                            lines.push(std::mem::take(&mut current_line));

                            // remainder에서 선행 공백 제거
                            let trimmed: Vec<GlyphFragment> = remainder
                                .into_iter()
                                .skip_while(|f| f.is_whitespace)
                                .collect();

                            line_width = trimmed.iter().map(|f| f.advance).sum();
                            current_line = trimmed;
                            last_break_pos = None;
        
                        } else {
                            // 단어가 너무 김 → CharWrap 폴백
                            lines.push(std::mem::take(&mut current_line));
                            line_width = 0.0;
                            last_break_pos = None;
        
                        }
                    }
                    LineBreakMode::CharWrap => {
                        lines.push(std::mem::take(&mut current_line));
                        line_width = 0.0;
                        last_break_pos = None;
    
                    }
                    LineBreakMode::NoWrap => unreachable!(),
                }
            }

            current_line.push(frag.clone());
            line_width += frag.advance;
        }

        // 마지막 라인
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // 빈 입력이면 빈 라인 하나
        if lines.is_empty() {
            lines.push(Vec::new());
        }

        lines
    }
}

// ============================================================================
// TextHitPoint — 텍스트 히트 테스트 결과
// ============================================================================

/// 텍스트 히트 테스트 결과
///
/// 로컬 좌표가 텍스트 레이아웃 내 어디에 해당하는지 나타냅니다.
#[derive(Debug, Clone)]
pub struct TextHitPoint {
    /// 히트된 문자 인덱스 (글리프 인덱스)
    pub char_index: usize,
    /// 히트된 라인 인덱스
    pub line_index: usize,
    /// 문자 내부에서의 비율 (0.0=왼쪽 가장자리, 1.0=오른쪽 가장자리)
    pub fraction: f32,
    /// 텍스트 영역 밖에 있는지
    pub is_outside: bool,
    /// 커서 삽입 위치 (char_index 앞이면 false, 뒤면 true)
    pub trailing: bool,
}

impl TextHitPoint {
    /// 텍스트 영역 밖 결과
    pub fn outside() -> Self {
        Self {
            char_index: 0,
            line_index: 0,
            fraction: 0.0,
            is_outside: true,
            trailing: false,
        }
    }

    /// 커서 삽입 위치 계산
    ///
    /// fraction > 0.5 이면 해당 문자 뒤, 아니면 앞
    pub fn insertion_index(&self) -> usize {
        if self.trailing {
            self.char_index + 1
        } else {
            self.char_index
        }
    }
}

impl TextLayoutResult {
    /// 로컬 좌표에서 텍스트 히트 테스트
    pub fn hit_test(&self, local_pos: Vec2) -> TextHitPoint {
        if self.lines.is_empty() {
            return TextHitPoint::outside();
        }

        // 라인 찾기
        let mut target_line = 0;
        let mut is_outside_y = false;

        if local_pos.y < 0.0 {
            target_line = 0;
            is_outside_y = true;
        } else if local_pos.y >= self.total_size.y {
            target_line = self.lines.len() - 1;
            is_outside_y = true;
        } else {
            for (i, line) in self.lines.iter().enumerate() {
                if local_pos.y >= line.line_origin.y
                    && local_pos.y < line.line_origin.y + line.line_height
                {
                    target_line = i;
                    break;
                }
            }
        }

        let line = &self.lines[target_line];
        let x = local_pos.x;

        if line.glyphs.is_empty() {
            return TextHitPoint {
                char_index: 0,
                line_index: target_line,
                fraction: 0.0,
                is_outside: true,
                trailing: false,
            };
        }

        // 글리프에서 X 위치 찾기
        let mut accumulated_x = 0.0;
        for (i, glyph) in line.glyphs.iter().enumerate() {
            let glyph_start = accumulated_x;
            let glyph_end = accumulated_x + glyph.advance;

            if x >= glyph_start && x < glyph_end {
                let fraction = (x - glyph_start) / glyph.advance.max(0.001);
                let trailing = fraction > 0.5;
                return TextHitPoint {
                    char_index: i,
                    line_index: target_line,
                    fraction,
                    is_outside: is_outside_y,
                    trailing,
                };
            }

            accumulated_x = glyph_end;
        }

        // X가 라인 끝을 넘음
        TextHitPoint {
            char_index: line.glyphs.len().saturating_sub(1),
            line_index: target_line,
            fraction: 1.0,
            is_outside: true,
            trailing: true,
        }
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// 내부 글리프 프래그먼트 (레이아웃 전 문자 데이터)
#[derive(Debug, Clone)]
struct GlyphFragment {
    ch: char,
    advance: f32,
    font_selector: FontSelector,
    font_size: f32,
    color: [f32; 4],
    run_index: usize,
    is_newline: bool,
    is_whitespace: bool,
}

/// CJK 문자 판별 (문자 단위 줄바꿈 허용)
fn is_cjk_char(c: char) -> bool {
    let cp = c as u32;
    // CJK Unified Ideographs
    (0x4E00..=0x9FFF).contains(&cp)
    // CJK Extension A
    || (0x3400..=0x4DBF).contains(&cp)
    // CJK Extension B
    || (0x20000..=0x2A6DF).contains(&cp)
    // CJK Compatibility Ideographs
    || (0xF900..=0xFAFF).contains(&cp)
    // Hangul Syllables
    || (0xAC00..=0xD7AF).contains(&cp)
    // Katakana
    || (0x30A0..=0x30FF).contains(&cp)
    // Hiragana
    || (0x3040..=0x309F).contains(&cp)
    // Fullwidth punctuation
    || (0xFF00..=0xFFEF).contains(&cp)
}

/// 단일 문자의 advance 측정 (TextMeasurer 위임)
fn measure_char_advance(
    measurer: &TextMeasurer,
    ch: char,
    font_size: f32,
    selector: FontSelector,
    font_scale: f32,
) -> f32 {
    // 단일 문자 문자열로 측정 (TextMeasurer API 활용)
    let s: String = ch.to_string();
    measurer.measure_width_with_selector(&s, font_size, selector, font_scale)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;
    use crate::render::text_run::TextRunStyle;

    fn make_style() -> TextRunStyle {
        TextRunStyle {
            font_selector: FontSelector::default(),
            font_size: 14.0,
            color: Color::WHITE,
            underline: false,
            strikethrough: false,
            letter_spacing: 0.0,
        }
    }

    #[test]
    fn test_empty_layout() {
        let runs: Vec<Box<dyn ITextRun>> = vec![];
        let result = TextLayout::layout(&runs, &TextLayoutParams::default());
        assert_eq!(result.line_count(), 0);
        assert_eq!(result.total_size, Vec2::ZERO);
    }

    #[test]
    fn test_single_line_no_wrap() {
        let style = make_style();
        let params = TextLayoutParams {
            max_width: f32::INFINITY,
            line_break_mode: LineBreakMode::NoWrap,
            ..Default::default()
        };
        let result = TextLayout::layout_simple("Hello", &style, &params);
        // 폰트가 없어도 최소 1라인 생성
        assert!(result.line_count() >= 1);
    }

    #[test]
    fn test_explicit_newline() {
        let style = make_style();
        let params = TextLayoutParams::default();
        let result = TextLayout::layout_simple("Line1\nLine2\nLine3", &style, &params);
        assert_eq!(result.line_count(), 3);
    }

    #[test]
    fn test_max_lines() {
        let style = make_style();
        let params = TextLayoutParams {
            max_lines: 2,
            ..Default::default()
        };
        let result = TextLayout::layout_simple("A\nB\nC\nD", &style, &params);
        assert!(result.line_count() <= 2);
    }

    #[test]
    fn test_line_break_mode_default() {
        assert_eq!(LineBreakMode::default(), LineBreakMode::NoWrap);
    }

    #[test]
    fn test_layout_result_glyph_count() {
        let result = TextLayoutResult::empty();
        assert_eq!(result.glyph_count(), 0);
        assert_eq!(result.line_count(), 0);
    }

    #[test]
    fn test_cjk_detection() {
        assert!(is_cjk_char('가')); // Hangul
        assert!(is_cjk_char('漢')); // CJK ideograph
        assert!(is_cjk_char('カ')); // Katakana
        assert!(is_cjk_char('あ')); // Hiragana
        assert!(!is_cjk_char('A'));  // Latin
        assert!(!is_cjk_char(' '));  // Space
    }
}
