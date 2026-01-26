//! STextBlock - 텍스트 표시 위젯 (Slate의 STextBlock)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Visibility, Color, SlateRect, HAlign, VAlign};
use crate::event::{Reply, PointerEvent};
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
    /// 표시할 텍스트
    text: String,
    /// 폰트 크기
    font_size: f32,
    /// 텍스트 색상
    color: Color,
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
}

impl Default for STextBlock {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 14.0,
            color: Color::WHITE,
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
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    /// 텍스트 내용
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 텍스트 설정
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// 색상 설정
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    /// 텍스트 크기 측정 (근사값)
    /// 실제 구현에서는 폰트 메트릭을 사용해야 함
    fn measure_text(&self, _scale: f32) -> Vec2 {
        if self.text.is_empty() {
            return Vec2::ZERO;
        }

        // 근사 문자 폭 계산 (font_size의 약 0.5~0.6배)
        // CJK 문자는 full-width, ASCII는 half-width로 계산
        let mut width = 0.0;
        let char_width_half = self.font_size * 0.5;
        let char_width_full = self.font_size;

        for c in self.text.chars() {
            if c == '\n' {
                // 줄바꿈은 무시 (여러 줄 계산은 아래에서)
                continue;
            } else if c.is_ascii() {
                width += char_width_half;
            } else {
                // CJK 및 기타 full-width 문자
                width += char_width_full;
            }
        }

        // 줄 수 계산
        let line_count = self.text.lines().count().max(1);
        let line_height = self.font_size * self.line_height_ratio;
        let height = line_height * line_count as f32;

        // 여러 줄인 경우 가장 긴 줄 기준
        if line_count > 1 {
            let max_line_width = self.text.lines()
                .map(|line| {
                    let mut w = 0.0;
                    for c in line.chars() {
                        if c.is_ascii() {
                            w += char_width_half;
                        } else {
                            w += char_width_full;
                        }
                    }
                    w
                })
                .fold(0.0f32, |a, b| a.max(b));
            return Vec2::new(max_line_width, height);
        }

        Vec2::new(width, height)
    }
}

/// STextBlock 빌더
#[derive(Default)]
pub struct STextBlockBuilder {
    inner: STextBlock,
}

impl STextBlockBuilder {
    /// 텍스트 설정
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.inner.text = text.into();
        self
    }

    /// 폰트 크기
    pub fn font_size(mut self, size: f32) -> Self {
        self.inner.font_size = size;
        self
    }

    /// 텍스트 색상
    pub fn color(mut self, color: Color) -> Self {
        self.inner.color = color;
        self
    }

    /// 텍스트 색상 (hex)
    pub fn color_hex(mut self, hex: &str) -> Self {
        if let Some(color) = Color::from_hex(hex) {
            self.inner.color = color;
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
        self.measure_text(layout_scale)
    }

    fn type_name(&self) -> &'static str {
        "STextBlock"
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
        if self.text.is_empty() {
            return layer;
        }

        let paint_geo = geometry.to_paint_geometry();

        // 비활성화 시 색상 변경
        let text_color = if is_enabled {
            self.color
        } else {
            Color::rgba(
                self.color.r * 0.5,
                self.color.g * 0.5,
                self.color.b * 0.5,
                self.color.a * 0.5,
            )
        };

        let mut current_layer = layer;

        // 그림자 그리기
        if let Some(shadow_color) = self.shadow_color {
            let shadow_geo = crate::core::PaintGeometry {
                position: paint_geo.position + self.shadow_offset,
                size: paint_geo.size,
                scale: paint_geo.scale,
            };
            draw_elements.add_text(
                current_layer,
                shadow_geo,
                self.text.clone(),
                shadow_color,
                self.font_size,
            );
            current_layer += 1;
        }

        // 본문 텍스트 그리기
        draw_elements.add_text(
            current_layer,
            paint_geo,
            self.text.clone(),
            text_color,
            self.font_size,
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for STextBlock {}
