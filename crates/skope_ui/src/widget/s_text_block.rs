//! STextBlock - 텍스트 표시 위젯 (Slate의 STextBlock)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Visibility, Color, SlateRect, HAlign, VAlign, InvalidateWidgetReason, FontFamily, FontSelector, Attribute, SlateAttribute};
use crate::event::{Reply, PointerEvent};
use crate::render::text_renderer::TextMeasurer;
use crate::render::text_run::TextRunStyle;
use crate::render::text_layout::{TextLayout, TextLayoutParams, TextLayoutResult, LineBreakMode};
use super::{Widget, LeafWidget, PaintArgs, DrawElementList};

/// 텍스트 자동 줄바꿈 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrapping {
    /// 줄바꿈 없음
    #[default]
    NoWrap,
    /// 단어 단위 줄바꿈
    WordWrap,
    /// 문자 단위 줄바꿈
    CharWrap,
}

/// 텍스트 오버플로우 처리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextOverflow {
    /// 잘림
    #[default]
    Clip,
    /// 말줄임표
    Ellipsis,
}

/// 텍스트 표시 위젯
pub struct STextBlock {
    /// 표시할 텍스트 (바인딩 가능)
    text: SlateAttribute<String>,
    /// 폰트 크기 (바인딩 가능)
    font_size: SlateAttribute<f32>,
    /// 텍스트 색상 (바인딩 가능)
    color: SlateAttribute<Color>,
    /// 수평 정렬
    h_align: HAlign,
    /// 수직 정렬
    v_align: VAlign,
    /// 줄바꿈 모드
    wrapping: TextWrapping,
    /// 오버플로우 처리
    overflow: TextOverflow,
    /// 줄 높이 배율 (1.0 = 기본)
    line_height_ratio: f32,
    /// 최대 줄 수 (0 = 무제한)
    max_lines: usize,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 그림자 색상 (None = 그림자 없음)
    shadow_color: Option<Color>,
    /// 그림자 오프셋
    shadow_offset: Vec2,
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
}

impl Default for STextBlock {
    fn default() -> Self {
        Self {
            text: SlateAttribute::from_value(
                String::new(),
                InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
            ),
            font_size: SlateAttribute::from_value(
                12.0,
                InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
            ),
            color: SlateAttribute::from_value(
                Color::WHITE,
                InvalidateWidgetReason::PAINT,
            ),
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            wrapping: TextWrapping::NoWrap,
            overflow: TextOverflow::Clip,
            line_height_ratio: 1.2,
            max_lines: 0,
            visibility: Visibility::Visible,
            enabled: true,
            shadow_color: None,
            shadow_offset: Vec2::new(1.0, 1.0),
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
        }
    }
}

impl STextBlock {
    /// 빌더 시작
    pub fn new() -> STextBlockBuilder {
        STextBlockBuilder::default()
    }

    /// 텍스트 직접 생성
    pub fn simple(text: impl Into<String>) -> Self {
        let mut s = Self::default();
        s.text.set(text.into());
        s
    }

    /// 텍스트 내용
    pub fn text(&self) -> &str {
        self.text.get()
    }

    /// 텍스트 설정
    pub fn set_text(&mut self, text: impl Into<String>) {
        let new_text = text.into();
        if *self.text.get() != new_text {
            self.text.set(new_text);
            self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
        }
    }

    /// 색상 설정
    pub fn set_color(&mut self, color: Color) {
        if *self.color.get() != color {
            self.color.set(color);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }

    /// 텍스트 크기 측정 (ab_glyph 폰트 메트릭 기반)
    ///
    /// TextMeasurer 싱글톤을 통해 실제 글리프 h_advance를 사용합니다.
    /// 폰트가 아직 등록되지 않은 경우 근사값으로 폴백합니다.
    ///
    /// UE Slate 패턴: font_size는 고정, font_scale(layout_scale)을 별도 파라미터로 전달.
    /// 이렇게 하면 폰트 크기 자체는 변경하지 않으면서 DPI 스케일링을 적용할 수 있습니다.
    fn measure_text(&self, font_scale: f32) -> Vec2 {
        let text = self.text.get();
        if text.is_empty() {
            return Vec2::ZERO;
        }

        let font_size = *self.font_size.get();
        if let Ok(measurer) = TextMeasurer::instance().read() {
            // UE 패턴: font_size는 그대로, font_scale을 별도 파라미터로 전달
            measurer.measure_size(text, font_size, FontFamily::UI, self.line_height_ratio, font_scale)
        } else {
            // 폴백: 근사값 (font_scale 적용)
            let scaled_font_size = font_size * font_scale;
            let line_height = scaled_font_size * self.line_height_ratio;
            let line_count = text.lines().count().max(1) as f32;
            let width = text.len() as f32 * scaled_font_size * 0.5;
            Vec2::new(width, line_height * line_count)
        }
    }

    /// TextWrapping → LineBreakMode 변환
    fn to_line_break_mode(&self) -> LineBreakMode {
        match self.wrapping {
            TextWrapping::NoWrap => LineBreakMode::NoWrap,
            TextWrapping::WordWrap => LineBreakMode::WordWrap,
            TextWrapping::CharWrap => LineBreakMode::CharWrap,
        }
    }

    /// TextRunStyle 생성 (현재 스타일 기반)
    fn make_run_style(&self) -> TextRunStyle {
        TextRunStyle {
            font_selector: FontSelector::new(FontFamily::UI),
            font_size: *self.font_size.get(),
            color: *self.color.get(),
            underline: false,
            strikethrough: false,
            letter_spacing: 0.0,
        }
    }

    /// TextLayout 기반 레이아웃 계산
    fn compute_text_layout(&self, max_width: f32, font_scale: f32) -> TextLayoutResult {
        let text = self.text.get();
        let style = self.make_run_style();
        let params = TextLayoutParams {
            max_width,
            line_break_mode: self.to_line_break_mode(),
            line_height_ratio: self.line_height_ratio,
            max_lines: self.max_lines,
            font_scale,
        };
        TextLayout::layout_simple(text, &style, &params)
    }
}

/// STextBlock 빌더
#[derive(Default)]
pub struct STextBlockBuilder {
    inner: STextBlock,
}

impl STextBlockBuilder {
    /// 텍스트 설정 (정적 값)
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.inner.text.set(text.into());
        self
    }

    /// 텍스트 바인딩 (동적 값)
    pub fn text_attr(mut self, attr: Attribute<String>) -> Self {
        self.inner.text.assign(attr);
        self
    }

    /// 폰트 크기 (정적 값)
    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size.set(size);
        self
    }

    /// 폰트 크기 바인딩 (동적 값)
    pub fn font_size_attr(mut self, attr: Attribute<f32>) -> Self {
        self.inner.font_size.assign(attr);
        self
    }

    /// 텍스트 색상 (정적 값)
    pub fn color(mut self, color: Color) -> Self {
        self.inner.color.set(color);
        self
    }

    /// 텍스트 색상 바인딩 (동적 값)
    pub fn color_attr(mut self, attr: Attribute<Color>) -> Self {
        self.inner.color.assign(attr);
        self
    }

    /// 텍스트 색상 (hex)
    pub fn color_hex(mut self, hex: &str) -> Self {
        if let Some(color) = Color::from_hex(hex) {
            self.inner.color.set(color);
        }
        self
    }

    /// 수평 정렬
    pub fn h_align(mut self, align: HAlign) -> Self {
        self.inner.h_align = align;
        self
    }

    /// 수직 정렬
    pub fn v_align(mut self, align: VAlign) -> Self {
        self.inner.v_align = align;
        self
    }

    /// 가운데 정렬 (수평 + 수직)
    pub fn center(mut self) -> Self {
        self.inner.h_align = HAlign::Center;
        self.inner.v_align = VAlign::Center;
        self
    }

    /// 줄바꿈 모드
    pub fn wrapping(mut self, wrapping: TextWrapping) -> Self {
        self.inner.wrapping = wrapping;
        self
    }

    /// 오버플로우 처리
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.inner.overflow = overflow;
        self
    }

    /// 줄 높이 배율
    pub fn line_height(mut self, ratio: f32) -> Self {
        self.inner.line_height_ratio = ratio;
        self
    }

    /// 최대 줄 수
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.inner.max_lines = lines;
        self
    }

    /// 그림자 효과
    pub fn shadow(mut self, color: Color, offset: Vec2) -> Self {
        self.inner.shadow_color = Some(color);
        self.inner.shadow_offset = offset;
        self
    }

    /// 그림자 (기본 오프셋)
    pub fn shadow_color(mut self, color: Color) -> Self {
        self.inner.shadow_color = Some(color);
        self
    }

    /// 빌드 완료
    pub fn build(self) -> STextBlock {
        self.inner
    }
}

impl Widget for STextBlock {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // wrapping 모드에서는 TextLayout으로 desired size 계산
        if self.wrapping != TextWrapping::NoWrap {
            let result = self.compute_text_layout(f32::INFINITY, layout_scale);
            if result.total_size != Vec2::ZERO {
                return result.total_size;
            }
        }
        self.measure_text(layout_scale)
    }

    fn type_name(&self) -> &'static str {
        "STextBlock"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::Label
    }

    fn accessibility_state(&self) -> crate::framework::AccessibilityState {
        crate::framework::AccessibilityState {
            enabled: self.is_enabled(),
            value_text: Some(self.text.get().clone()),
            ..Default::default()
        }
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
        let text = self.text.get();
        if text.is_empty() {
            return layer;
        }

        let paint_geo = geometry.to_paint_geometry();
        let color = *self.color.get();
        let font_size = *self.font_size.get();

        // 비활성화 시 색상 변경
        let text_color = if is_enabled {
            color
        } else {
            Color::rgba(
                color.r * 0.5,
                color.g * 0.5,
                color.b * 0.5,
                color.a * 0.5,
            )
        };

        let mut current_layer = layer;

        // 그림자 그리기
        if let Some(shadow_color) = self.shadow_color {
            let shadow_geo = crate::core::PaintGeometry::new(
                paint_geo.position + self.shadow_offset,
                paint_geo.size,
                paint_geo.scale,
            );
            draw_elements.add_text(
                current_layer,
                shadow_geo,
                text.clone(),
                shadow_color,
                font_size,
            );
            current_layer += 1;
        }

        // 멀티라인 렌더링: wrapping 모드에서 TextLayout 사용
        if self.wrapping != TextWrapping::NoWrap {
            let layout_result = self.compute_text_layout(paint_geo.size.x, 1.0);
            if !layout_result.lines.is_empty() {
                for line in &layout_result.lines {
                    let line_text: String = line.glyphs.iter().map(|g| g.codepoint).collect();
                    if line_text.is_empty() {
                        continue;
                    }

                    let line_pos = paint_geo.position + line.line_origin;
                    let line_size = Vec2::new(line.width, line.line_height);
                    let line_geo = crate::core::PaintGeometry::new(line_pos, line_size, paint_geo.scale);

                    draw_elements.add_text(
                        current_layer,
                        line_geo,
                        line_text,
                        text_color,
                        font_size,
                    );
                    current_layer += 1;
                }
                return current_layer;
            }
        }

        // 단일 라인 또는 NoWrap: 기존 경로
        draw_elements.add_text(
            current_layer,
            paint_geo,
            text.clone(),
            text_color,
            font_size,
        );

        current_layer + 1
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        // 텍스트는 기본적으로 이벤트를 처리하지 않음
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

    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, text, color, font_size)
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

impl LeafWidget for STextBlock {}

// ============================================================================
// IAccessibleText — UE5.7 FSlateAccessibleTextBlock
// ============================================================================

impl crate::framework::IAccessibleText for STextBlock {
    fn get_text(&self) -> &str {
        self.text.get()
    }
}
