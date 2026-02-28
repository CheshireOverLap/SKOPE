//! SRichTextBlock - 스타일이 다른 텍스트 런을 표시 (언리얼 Slate의 SRichTextBlock)
//!
//! 각 런마다 색상, 폰트 크기, 볼드/이탤릭/밑줄을 지정할 수 있습니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, FontFamily, FontSelector, FontWeight, FontStyle,
    Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;
use crate::render::text_run::{TextRunStyle, FSlateTextRun, TextRange};
use crate::render::text_layout::{TextLayout, TextLayoutParams, TextLayoutResult, LineBreakMode};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 텍스트 런 (하나의 스타일 구간)
#[derive(Debug, Clone)]
pub struct TextRun {
    pub text: String,
    pub color: Color,
    pub font_size: f32,
    pub font_family: FontFamily,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl TextRun {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: Color::WHITE,
            font_size: 12.0,
            font_family: FontFamily::UI,
            bold: false,
            italic: false,
            underline: false,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn font_family(mut self, family: FontFamily) -> Self {
        self.font_family = family;
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// TextRunStyle로 변환 (TextLayout 연동용)
    pub fn to_run_style(&self) -> TextRunStyle {
        let weight = if self.bold { FontWeight::Bold } else { FontWeight::Regular };
        let style = if self.italic { FontStyle::Italic } else { FontStyle::Normal };
        let selector = FontSelector::new(self.font_family)
            .with_weight(weight)
            .with_style(style);

        TextRunStyle {
            font_selector: selector,
            font_size: self.font_size,
            color: self.color,
            underline: self.underline,
            strikethrough: false,
            letter_spacing: 0.0,
        }
    }
}

/// 리치 텍스트 블록
pub struct SRichTextBlock {
    runs: Vec<TextRun>,
    line_height_ratio: f32,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
    /// 줄바꿈 모드
    wrap_mode: LineBreakMode,
    /// 캐시된 레이아웃 결과
    cached_layout: Option<TextLayoutResult>,
    /// 마지막 레이아웃 너비 (캐시 무효화 판단용)
    last_layout_width: f32,
}

impl Default for SRichTextBlock {
    fn default() -> Self {
        Self {
            runs: Vec::new(),
            line_height_ratio: 1.2,
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            wrap_mode: LineBreakMode::NoWrap,
            cached_layout: None,
            last_layout_width: 0.0,
        }
    }
}

impl SRichTextBlock {
    pub fn new() -> SRichTextBlockBuilder {
        SRichTextBlockBuilder {
            inner: SRichTextBlock::default(),
        }
    }

    /// 런 추가
    pub fn add_run(&mut self, run: TextRun) {
        self.runs.push(run);
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
    }

    /// 런 전체 교체
    pub fn set_runs(&mut self, runs: Vec<TextRun>) {
        self.runs = runs;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
    }

    /// 런 목록
    pub fn runs(&self) -> &[TextRun] {
        &self.runs
    }

    /// 줄바꿈 모드 설정
    pub fn set_wrap_mode(&mut self, mode: LineBreakMode) {
        if self.wrap_mode != mode {
            self.wrap_mode = mode;
            self.cached_layout = None;
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    /// 리치 텍스트 마크업 설정 (파서로 TextRun 자동 생성)
    ///
    /// 지원 태그: `<b>`, `<i>`, `<u>`, `<s>`, `<color=#RRGGBB>`
    pub fn set_markup(&mut self, markup: &str) {
        use crate::render::rich_text::{DefaultRichTextParser, IRichTextMarkupParser};

        // 기본 스타일 (첫 번째 런 또는 디폴트 사용)
        let base_style = if let Some(first_run) = self.runs.first() {
            first_run.to_run_style()
        } else {
            TextRunStyle::default()
        };

        let parser = DefaultRichTextParser::new();
        let parsed_runs = parser.parse(markup, &base_style);

        // ITextRun → 위젯 내부 TextRun 변환
        self.runs = parsed_runs.iter().map(|run| {
            let style = run.style();
            TextRun {
                text: run.text().to_string(),
                color: style.color,
                font_size: style.font_size,
                font_family: FontFamily::UI,
                bold: style.font_selector.weight == FontWeight::Bold,
                italic: style.font_selector.style == FontStyle::Italic,
                underline: style.underline,
            }
        }).collect();

        self.cached_layout = None;
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
    }

    /// 런별 폭 측정 (font_scale 기본값 1.0)
    fn measure_run_width(run: &TextRun, font_scale: f32) -> f32 {
        if run.text.is_empty() {
            return 0.0;
        }
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(&run.text, run.font_size, run.font_family, font_scale)
        } else {
            let scaled = run.font_size * font_scale;
            run.text.chars().count() as f32 * scaled * 0.5
        }
    }

    /// TextLayout 용 ITextRun 목록 생성
    fn build_layout_runs(&self) -> Vec<Box<dyn crate::render::text_run::ITextRun>> {
        let mut offset = 0usize;
        self.runs.iter().map(|run| {
            let style = run.to_run_style();
            let len = run.text.len();
            let range = TextRange::new(offset, offset + len);
            offset += len;
            Box::new(FSlateTextRun::new(run.text.clone(), style, range))
                as Box<dyn crate::render::text_run::ITextRun>
        }).collect()
    }

    /// 캐시된 레이아웃 가져오기 (필요 시 재계산)
    fn get_or_compute_layout(&self, max_width: f32) -> TextLayoutResult {
        // 캐시 히트 체크
        if let Some(ref cached) = self.cached_layout {
            if (self.last_layout_width - max_width).abs() < 0.5 {
                return cached.clone();
            }
        }
        // 재계산
        let layout_runs = self.build_layout_runs();
        let params = TextLayoutParams {
            max_width,
            line_break_mode: self.wrap_mode,
            line_height_ratio: self.line_height_ratio,
            max_lines: 0,
            font_scale: 1.0,
        };
        TextLayout::layout(&layout_runs, &params)
    }
}

/// SRichTextBlock 빌더
pub struct SRichTextBlockBuilder {
    inner: SRichTextBlock,
}

impl SRichTextBlockBuilder {
    /// 런 추가
    pub fn add_run(mut self, run: TextRun) -> Self {
        self.inner.runs.push(run);
        self
    }

    /// 일반 텍스트 추가 (기본 스타일)
    pub fn text(self, text: impl Into<String>) -> Self {
        self.add_run(TextRun::new(text))
    }

    /// 색상 텍스트 추가
    pub fn colored(self, text: impl Into<String>, color: Color) -> Self {
        self.add_run(TextRun::new(text).color(color))
    }

    /// 줄 높이 배율
    pub fn line_height(mut self, ratio: f32) -> Self {
        self.inner.line_height_ratio = ratio;
        self
    }

    /// 줄바꿈 모드 설정
    pub fn wrap_mode(mut self, mode: LineBreakMode) -> Self {
        self.inner.wrap_mode = mode;
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SRichTextBlock {
        self.inner
    }
}

impl Widget for SRichTextBlock {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        if self.runs.is_empty() {
            return Vec2::ZERO;
        }

        // TextLayout 사용: NoWrap이면 무제한 폭으로 측정
        let result = self.get_or_compute_layout(f32::INFINITY);
        if result.total_size != Vec2::ZERO {
            return result.total_size;
        }

        // 폴백: 기존 런 폭 합산
        let total_width: f32 = self.runs.iter()
            .map(|r| SRichTextBlock::measure_run_width(r, 1.0))
            .sum();

        let max_font_size = self.runs.iter()
            .map(|r| r.font_size)
            .fold(14.0f32, |a, b| a.max(b));

        let height = max_font_size * self.line_height_ratio;
        Vec2::new(total_width, height)
    }

    fn type_name(&self) -> &'static str {
        "SRichTextBlock"
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        if self.runs.is_empty() {
            return layer;
        }

        let paint_geo = geometry.to_paint_geometry();
        let mut current_layer = layer;

        // TextLayout 결과 기반 렌더링
        let max_width = if self.wrap_mode != LineBreakMode::NoWrap {
            paint_geo.size.x
        } else {
            f32::INFINITY
        };
        let layout_result = self.get_or_compute_layout(max_width);

        if !layout_result.lines.is_empty() {
            // TextLayout 기반 렌더링: 라인별 글리프 그룹 그리기
            for line in &layout_result.lines {
                // 같은 run_index 연속 글리프를 묶어서 텍스트로 출력
                let mut group_start = 0;
                while group_start < line.glyphs.len() {
                    let run_idx = line.glyphs[group_start].run_index;
                    let mut group_end = group_start + 1;
                    while group_end < line.glyphs.len()
                        && line.glyphs[group_end].run_index == run_idx
                    {
                        group_end += 1;
                    }

                    let group = &line.glyphs[group_start..group_end];
                    let text: String = group.iter().map(|g| g.codepoint).collect();
                    let x_pos = group[0].position.x;
                    let width: f32 = group.iter().map(|g| g.advance).sum();

                    let run = &self.runs[run_idx.min(self.runs.len() - 1)];
                    let run_color = if is_enabled {
                        run.color
                    } else {
                        Color::rgba(
                            run.color.r * 0.5, run.color.g * 0.5,
                            run.color.b * 0.5, run.color.a * 0.5,
                        )
                    };

                    let text_pos = paint_geo.position
                        + line.line_origin
                        + Vec2::new(x_pos, 0.0);
                    let text_size = Vec2::new(width, line.line_height);
                    let text_geo = PaintGeometry::new(text_pos, text_size, paint_geo.scale);

                    draw_elements.add_styled_text(
                        current_layer,
                        text_geo,
                        text,
                        run_color,
                        run.font_size,
                        run.to_run_style().font_selector,
                    );

                    // 밑줄
                    if run.underline {
                        let ul_y = text_pos.y + line.ascent + 1.0;
                        let ul_geo = PaintGeometry::new(
                            Vec2::new(text_pos.x, ul_y),
                            Vec2::new(width, 1.0),
                            paint_geo.scale,
                        );
                        draw_elements.add_box(current_layer, ul_geo, run_color);
                    }

                    group_start = group_end;
                    current_layer += 1;
                }
            }
            return current_layer;
        }

        // 폴백: 기존 단일 줄 렌더링
        let mut x_offset = 0.0f32;
        let max_font_size = self.runs.iter()
            .map(|r| r.font_size)
            .fold(14.0f32, |a, b| a.max(b));

        for run in &self.runs {
            if run.text.is_empty() {
                continue;
            }

            let run_width = SRichTextBlock::measure_run_width(run, 1.0);
            let run_color = if is_enabled {
                run.color
            } else {
                Color::rgba(run.color.r * 0.5, run.color.g * 0.5, run.color.b * 0.5, run.color.a * 0.5)
            };

            let y_offset = (max_font_size - run.font_size) * 0.5;

            let run_pos = paint_geo.position + Vec2::new(x_offset, y_offset);
            let run_size = Vec2::new(run_width, run.font_size);
            let run_geo = PaintGeometry::new(run_pos, run_size, paint_geo.scale);

            draw_elements.add_text_with_font(
                current_layer,
                run_geo,
                run.text.clone(),
                run_color,
                run.font_size,
                run.font_family,
            );

            if run.underline {
                let ul_y = run_pos.y + run.font_size + 1.0;
                let ul_geo = PaintGeometry::new(
                    Vec2::new(run_pos.x, ul_y),
                    Vec2::new(run_width, 1.0),
                    paint_geo.scale,
                );
                draw_elements.add_box(current_layer, ul_geo, run_color);
            }

            x_offset += run_width;
            current_layer += 1;
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SRichTextBlock {}
