//! SRichTextBlock - 스타일이 다른 텍스트 런을 표시 (언리얼 Slate의 SRichTextBlock)
//!
//! 각 런마다 색상, 폰트 크기, 볼드/이탤릭/밑줄을 지정할 수 있습니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;

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

    /// 빌드 완료
    pub fn build(self) -> SRichTextBlock {
        self.inner
    }
}

impl Widget for SRichTextBlock {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        // 모든 런의 폭 합산 (단일 줄 가정)
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
        let mut x_offset = 0.0f32;
        let mut current_layer = layer;

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

            // 텍스트 y 위치 조정 (baseline 정렬 — 큰 폰트 기준 중앙 맞춤)
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

            // 밑줄
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
