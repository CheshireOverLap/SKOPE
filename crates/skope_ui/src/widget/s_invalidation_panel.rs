//! SInvalidationPanel — 무효화 영역 래퍼
//!
//! UE 참조: `SInvalidationPanel`. 자식 위젯의 렌더링을 캐시하여
//! 무효화되지 않은 경우 재계산을 생략합니다.
//! 복잡한 UI 서브트리의 성능 최적화에 사용됩니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, InvalidateWidgetReason, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{ArrangedChildren, CompoundWidget, DrawElementList, PaintArgs, Widget};

/// 캐시 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationCacheMode {
    /// 레이아웃만 캐시
    LayoutOnly,
    /// 레이아웃 + 페인트 캐시
    LayoutAndPaint,
    /// 모든 것 캐시 (이벤트 결과 포함)
    Full,
}

impl Default for InvalidationCacheMode {
    fn default() -> Self {
        Self::LayoutAndPaint
    }
}

/// 무효화 영역 래퍼
///
/// 자식 위젯 서브트리의 desired size와 paint 결과를 캐시합니다.
/// 무효화 플래그가 설정될 때만 재계산합니다.
pub struct SInvalidationPanel {
    id: u64,
    dirty: InvalidateWidgetReason,
    content: Option<Box<dyn Widget>>,
    /// 캐시 모드
    cache_mode: InvalidationCacheMode,
    /// 캐시된 desired size
    cached_desired_size: Option<Vec2>,
    /// 캐시가 유효한지 여부
    layout_cache_valid: bool,
    /// 페인트 캐시 유효 여부
    paint_cache_valid: bool,
    /// 캐시된 페인트 레이어
    cached_max_layer: u32,
    visibility: Visibility,
    enabled: bool,
}

impl SInvalidationPanel {
    pub fn new() -> SInvalidationPanelBuilder {
        SInvalidationPanelBuilder {
            content: None,
            cache_mode: InvalidationCacheMode::LayoutAndPaint,
        }
    }

    /// 레이아웃 캐시 무효화
    pub fn invalidate_layout(&mut self) {
        self.layout_cache_valid = false;
        self.cached_desired_size = None;
    }

    /// 페인트 캐시 무효화
    pub fn invalidate_paint(&mut self) {
        self.paint_cache_valid = false;
    }

    /// 전체 캐시 무효화
    pub fn invalidate_all(&mut self) {
        self.invalidate_layout();
        self.invalidate_paint();
    }

    /// 레이아웃 캐시 유효 여부
    pub fn is_layout_cached(&self) -> bool {
        self.layout_cache_valid
    }

    /// 페인트 캐시 유효 여부
    pub fn is_paint_cached(&self) -> bool {
        self.paint_cache_valid
    }

    /// 캐시 모드 조회
    pub fn cache_mode(&self) -> InvalidationCacheMode {
        self.cache_mode
    }

    /// 캐시 모드 설정
    pub fn set_cache_mode(&mut self, mode: InvalidationCacheMode) {
        if self.cache_mode != mode {
            self.cache_mode = mode;
            self.invalidate_all();
        }
    }
}

/// SInvalidationPanel 빌더
pub struct SInvalidationPanelBuilder {
    content: Option<Box<dyn Widget>>,
    cache_mode: InvalidationCacheMode,
}

impl SInvalidationPanelBuilder {
    pub fn content(mut self, widget: impl Widget + 'static) -> Self {
        self.content = Some(Box::new(widget));
        self
    }

    pub fn cache_mode(mut self, mode: InvalidationCacheMode) -> Self {
        self.cache_mode = mode;
        self
    }

    pub fn build(self) -> SInvalidationPanel {
        SInvalidationPanel {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            content: self.content,
            cache_mode: self.cache_mode,
            cached_desired_size: None,
            layout_cache_valid: false,
            paint_cache_valid: false,
            cached_max_layer: 0,
            visibility: Visibility::SelfHitTestInvisible,
            enabled: true,
        }
    }
}

impl Widget for SInvalidationPanel {
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2 {
        // 캐시가 유효하면 캐시된 값 반환
        if self.layout_cache_valid {
            if let Some(cached) = self.cached_desired_size {
                return cached;
            }
        }

        let size = self.content
            .as_ref()
            .map(|c| c.compute_desired_size(layout_scale))
            .unwrap_or(Vec2::ZERO);

        // 캐시 업데이트 (interior mutability 없이 반환만)
        size
    }

    fn arrange_children(&self, geometry: &Geometry, arranged: &mut ArrangedChildren) {
        if self.content.is_some() {
            let child_geo = geometry.make_child(Vec2::ZERO, geometry.local_size);
            arranged.add(0, child_geo);
        }
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        // 페인트 캐시가 유효하고 LayoutAndPaint/Full 모드면 캐시된 레이어 반환
        if self.paint_cache_valid
            && self.cache_mode != InvalidationCacheMode::LayoutOnly
        {
            return self.cached_max_layer.max(layer);
        }

        if let Some(ref content) = self.content {
            let mut arranged = ArrangedChildren::new();
            self.arrange_children(geometry, &mut arranged);

            if let Some(child_arranged) = arranged.children.first() {
                let result = content.on_paint(
                    args,
                    &child_arranged.geometry,
                    culling_rect,
                    draw_elements,
                    layer,
                    is_enabled && self.enabled,
                );
                return result;
            }
        }
        layer
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_down(geometry, event);
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if let Some(ref mut content) = self.content {
            return content.on_mouse_button_up(geometry, event);
        }
        Reply::unhandled()
    }

    fn type_name(&self) -> &'static str { "SInvalidationPanel" }

    fn num_children(&self) -> usize {
        if self.content.is_some() { 1 } else { 0 }
    }

    fn get_child(&self, index: usize) -> Option<&dyn Widget> {
        if index == 0 { self.content.as_ref().map(|c| c.as_ref()) } else { None }
    }

    fn get_child_mut(&mut self, index: usize) -> Option<&mut dyn Widget> {
        if index == 0 { self.content.as_mut().map(|c| c.as_mut()) } else { None }
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;

        // 무효화 이유에 따라 캐시도 무효화
        if reason.contains(InvalidateWidgetReason::LAYOUT) {
            self.invalidate_layout();
        }
        if reason.contains(InvalidateWidgetReason::PAINT) {
            self.invalidate_paint();
        }
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
        // dirty 클리어 시 캐시를 유효로 마킹
        self.layout_cache_valid = true;
        self.paint_cache_valid = true;
    }

    fn get_visibility(&self) -> Visibility { self.visibility }
    fn set_visibility(&mut self, visibility: Visibility) { self.visibility = visibility; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl CompoundWidget for SInvalidationPanel {
    fn get_content(&self) -> Option<&dyn Widget> {
        self.content.as_ref().map(|c| c.as_ref())
    }

    fn get_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_mut().map(|c| c.as_mut())
    }

    fn set_content(&mut self, content: Option<Box<dyn Widget>>) {
        self.content = content;
        self.invalidate_all();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    #[test]
    fn test_invalidation_panel_basic() {
        let w = SInvalidationPanel::new()
            .content(SSpacer::new().size(100.0, 50.0).build())
            .build();

        let size = w.compute_desired_size(1.0);
        assert_eq!(size.x, 100.0);
        assert_eq!(size.y, 50.0);
        assert_eq!(w.type_name(), "SInvalidationPanel");
    }

    #[test]
    fn test_invalidation_panel_cache_mode() {
        let mut w = SInvalidationPanel::new()
            .cache_mode(InvalidationCacheMode::Full)
            .content(SSpacer::new().size(50.0, 50.0).build())
            .build();

        assert_eq!(w.cache_mode(), InvalidationCacheMode::Full);

        w.set_cache_mode(InvalidationCacheMode::LayoutOnly);
        assert_eq!(w.cache_mode(), InvalidationCacheMode::LayoutOnly);
    }

    #[test]
    fn test_invalidation_panel_cache_validity() {
        let mut w = SInvalidationPanel::new()
            .content(SSpacer::new().size(50.0, 50.0).build())
            .build();

        // 초기: 캐시 무효
        assert!(!w.is_layout_cached());
        assert!(!w.is_paint_cached());

        // clear_dirty → 캐시 유효
        w.clear_dirty();
        assert!(w.is_layout_cached());
        assert!(w.is_paint_cached());

        // invalidate LAYOUT → 레이아웃 캐시만 무효
        w.invalidate(InvalidateWidgetReason::LAYOUT);
        assert!(!w.is_layout_cached());
        assert!(w.is_paint_cached());

        // invalidate PAINT → 페인트 캐시도 무효
        w.invalidate(InvalidateWidgetReason::PAINT);
        assert!(!w.is_paint_cached());
    }

    #[test]
    fn test_invalidation_panel_invalidate_all() {
        let mut w = SInvalidationPanel::new()
            .content(SSpacer::new().size(50.0, 50.0).build())
            .build();

        w.clear_dirty();
        assert!(w.is_layout_cached());
        assert!(w.is_paint_cached());

        w.invalidate_all();
        assert!(!w.is_layout_cached());
        assert!(!w.is_paint_cached());
    }

    #[test]
    fn test_invalidation_panel_compound_widget() {
        let mut w = SInvalidationPanel::new()
            .content(SSpacer::new().size(50.0, 50.0).build())
            .build();

        assert_eq!(w.num_children(), 1);
        assert!(w.get_content().is_some());

        // 콘텐츠 교체 시 캐시 무효화
        w.clear_dirty();
        assert!(w.is_layout_cached());

        w.set_content(Some(Box::new(SSpacer::new().size(100.0, 100.0).build())));
        assert!(!w.is_layout_cached());
    }

    #[test]
    fn test_invalidation_panel_empty() {
        let w = SInvalidationPanel::new().build();
        assert_eq!(w.compute_desired_size(1.0), Vec2::ZERO);
        assert_eq!(w.num_children(), 0);
    }
}
