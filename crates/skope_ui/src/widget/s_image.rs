//! SImage - 이미지 표시 위젯 (Slate의 SImage)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Visibility, Color, SlateRect, InvalidateWidgetReason, Attribute, SlateAttribute};
use crate::event::{Reply, PointerEvent};
use super::{Widget, LeafWidget, PaintArgs, DrawElementList, ImageScaling};

/// 이미지 표시 위젯
pub struct SImage {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 이미지 경로/이름
    image_path: String,
    /// 이미지 크기 (로드 후 설정)
    image_size: Option<Vec2>,
    /// 원하는 크기 (None = 이미지 원본 크기)
    desired_size: Option<Vec2>,
    /// 스케일링 모드
    scaling: ImageScaling,
    /// 틴트 색상
    tint: SlateAttribute<Color>,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
}

impl Default for SImage {
    fn default() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            image_path: String::new(),
            image_size: None,
            desired_size: None,
            scaling: ImageScaling::None,
            tint: SlateAttribute::from_value(Color::WHITE, InvalidateWidgetReason::PAINT),
            visibility: Visibility::Visible,
            enabled: true,
        }
    }
}

impl SImage {
    /// 빌더 시작
    pub fn new() -> SImageBuilder {
        SImageBuilder::default()
    }

    /// 이미지 경로
    pub fn path(&self) -> &str {
        &self.image_path
    }

    /// 이미지 크기 설정 (렌더러에서 호출)
    pub fn set_image_size(&mut self, size: Vec2) {
        self.image_size = Some(size);
    }

    /// 틴트 색상 변경
    pub fn set_tint(&mut self, tint: Color) {
        if *self.tint.get() != tint {
            self.tint.set(tint);
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }
    }
}

/// SImage 빌더
#[derive(Default)]
pub struct SImageBuilder {
    inner: SImage,
}

impl SImageBuilder {
    /// 이미지 경로 설정
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.inner.image_path = path.into();
        self
    }

    /// 원하는 크기 설정
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.inner.desired_size = Some(Vec2::new(width, height));
        self
    }

    /// 너비만 설정
    pub fn width(mut self, width: f32) -> Self {
        let height = self.inner.desired_size.map(|s| s.y).unwrap_or(0.0);
        self.inner.desired_size = Some(Vec2::new(width, height));
        self
    }

    /// 높이만 설정
    pub fn height(mut self, height: f32) -> Self {
        let width = self.inner.desired_size.map(|s| s.x).unwrap_or(0.0);
        self.inner.desired_size = Some(Vec2::new(width, height));
        self
    }

    /// 스케일링 모드
    pub fn scaling(mut self, scaling: ImageScaling) -> Self {
        self.inner.scaling = scaling;
        self
    }

    /// 틴트 색상 (정적 값)
    pub fn tint(mut self, color: Color) -> Self {
        self.inner.tint.set(color);
        self
    }

    /// 틴트 색상 바인딩 (동적 값)
    pub fn tint_attr(mut self, attr: Attribute<Color>) -> Self {
        self.inner.tint.assign(attr);
        self
    }

    /// 틴트 색상 (hex)
    pub fn tint_hex(mut self, hex: &str) -> Self {
        if let Some(color) = Color::from_hex(hex) {
            self.inner.tint.set(color);
        }
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SImage {
        self.inner
    }
}

impl Widget for SImage {
    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        crate::update_attributes!(self, tint)
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

    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        // 명시적 크기가 있으면 사용
        if let Some(size) = self.desired_size {
            return size;
        }

        // 이미지 크기가 있으면 사용
        if let Some(size) = self.image_size {
            return size;
        }

        // 기본값
        Vec2::new(100.0, 100.0)
    }

    fn type_name(&self) -> &'static str {
        "SImage"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::Image
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
        if self.image_path.is_empty() {
            return layer;
        }

        let paint_geo = geometry.to_paint_geometry();

        // 비활성화 시 색상 변경
        let tint_color = *self.tint.get();
        let tint = if is_enabled {
            tint_color
        } else {
            Color::rgba(
                tint_color.r * 0.5,
                tint_color.g * 0.5,
                tint_color.b * 0.5,
                tint_color.a * 0.5,
            )
        };

        // 이미지 DrawElement 추가
        draw_elements.add_image(
            layer,
            paint_geo,
            self.image_path.clone(),
            tint,
            self.scaling,
        );

        layer + 1
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SImage {}
