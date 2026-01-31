//! Widget Invalidation System — FastUpdate 최적화
//!
//! UE의 SInvalidationPanel/SlateInvalidationRoot에 해당하는 시스템입니다.
//! 위젯 트리에서 변경된 부분만 재계산하고, 캐싱된 결과를 재사용합니다.

use std::collections::{HashMap, HashSet};
use glam::Vec2;

use crate::core::InvalidateWidgetReason;

/// 위젯 프록시 — 위젯의 무효화 상태를 추적
#[derive(Debug, Clone)]
pub struct WidgetProxy {
    pub widget_id: u64,
    pub parent_id: Option<u64>,
    pub dirty_flags: InvalidateWidgetReason,
    pub cached_desired_size: Option<Vec2>,
    pub layout_generation: u64,
    pub paint_generation: u64,
    pub is_volatile: bool,
    pub visibility_changed: bool,
}

impl WidgetProxy {
    pub fn new(widget_id: u64) -> Self {
        Self {
            widget_id,
            parent_id: None,
            dirty_flags: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            cached_desired_size: None,
            layout_generation: 0,
            paint_generation: 0,
            is_volatile: false,
            visibility_changed: false,
        }
    }

    pub fn needs_layout(&self) -> bool {
        self.dirty_flags.contains(InvalidateWidgetReason::LAYOUT)
    }

    pub fn needs_paint(&self) -> bool {
        self.dirty_flags.contains(InvalidateWidgetReason::PAINT) || self.is_volatile
    }

    pub fn mark_clean(&mut self, generation: u64) {
        self.dirty_flags = InvalidateWidgetReason::NONE;
        self.visibility_changed = false;
        self.layout_generation = generation;
        self.paint_generation = generation;
    }
}

/// 무효화된 위젯 리스트 — 우선순위 기반
#[derive(Debug)]
pub struct SlateInvalidationWidgetList {
    widgets: HashMap<u64, WidgetProxy>,
    dirty_layout: HashSet<u64>,
    dirty_paint: HashSet<u64>,
}

impl SlateInvalidationWidgetList {
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
            dirty_layout: HashSet::new(),
            dirty_paint: HashSet::new(),
        }
    }

    pub fn register(&mut self, widget_id: u64, parent_id: Option<u64>) {
        let mut proxy = WidgetProxy::new(widget_id);
        proxy.parent_id = parent_id;
        self.dirty_layout.insert(widget_id);
        self.dirty_paint.insert(widget_id);
        self.widgets.insert(widget_id, proxy);
    }

    pub fn unregister(&mut self, widget_id: u64) {
        self.widgets.remove(&widget_id);
        self.dirty_layout.remove(&widget_id);
        self.dirty_paint.remove(&widget_id);
    }

    pub fn invalidate(&mut self, widget_id: u64, reason: InvalidateWidgetReason) {
        if let Some(proxy) = self.widgets.get_mut(&widget_id) {
            proxy.dirty_flags = proxy.dirty_flags | reason;
            if reason.contains(InvalidateWidgetReason::LAYOUT) {
                self.dirty_layout.insert(widget_id);
                // 레이아웃 변경 → 부모도 레이아웃 필요
                if let Some(pid) = proxy.parent_id {
                    self.dirty_layout.insert(pid);
                    if let Some(pp) = self.widgets.get_mut(&pid) {
                        pp.dirty_flags = pp.dirty_flags | InvalidateWidgetReason::LAYOUT;
                    }
                }
            }
            if reason.contains(InvalidateWidgetReason::PAINT) {
                self.dirty_paint.insert(widget_id);
            }
        }
    }

    pub fn dirty_layout_count(&self) -> usize { self.dirty_layout.len() }
    pub fn dirty_paint_count(&self) -> usize { self.dirty_paint.len() }
    pub fn widget_count(&self) -> usize { self.widgets.len() }

    pub fn get_proxy(&self, widget_id: u64) -> Option<&WidgetProxy> {
        self.widgets.get(&widget_id)
    }

    pub fn get_proxy_mut(&mut self, widget_id: u64) -> Option<&mut WidgetProxy> {
        self.widgets.get_mut(&widget_id)
    }

    pub fn drain_dirty_layout(&mut self) -> Vec<u64> {
        let ids: Vec<u64> = self.dirty_layout.drain().collect();
        ids
    }

    pub fn drain_dirty_paint(&mut self) -> Vec<u64> {
        let ids: Vec<u64> = self.dirty_paint.drain().collect();
        ids
    }

    pub fn has_dirty(&self) -> bool {
        !self.dirty_layout.is_empty() || !self.dirty_paint.is_empty()
    }

    pub fn clear_all_dirty(&mut self) {
        self.dirty_layout.clear();
        self.dirty_paint.clear();
        for proxy in self.widgets.values_mut() {
            proxy.dirty_flags = InvalidateWidgetReason::NONE;
        }
    }
}

/// 캐시된 엘리먼트 데이터 — 위젯별 페인트 결과 캐시
#[derive(Debug, Clone)]
pub struct CachedElementData {
    pub widget_id: u64,
    pub layer: u32,
    pub element_count: usize,
    pub paint_generation: u64,
    pub is_valid: bool,
}

impl CachedElementData {
    pub fn new(widget_id: u64) -> Self {
        Self {
            widget_id,
            layer: 0,
            element_count: 0,
            paint_generation: 0,
            is_valid: false,
        }
    }

    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }
}

/// 무효화 힙 — 위젯을 깊이 기반으로 정렬하여 처리
#[derive(Debug)]
pub struct SlateInvalidationWidgetHeap {
    entries: Vec<(u64, u32)>, // (widget_id, depth)
}

impl SlateInvalidationWidgetHeap {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn push(&mut self, widget_id: u64, depth: u32) {
        self.entries.push((widget_id, depth));
    }

    /// 깊이가 얕은 순서로 정렬 (부모 먼저 처리)
    pub fn sort_by_depth(&mut self) {
        self.entries.sort_by_key(|&(_, depth)| depth);
    }

    pub fn drain(&mut self) -> Vec<(u64, u32)> {
        let result = self.entries.clone();
        self.entries.clear();
        result
    }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn len(&self) -> usize { self.entries.len() }
}

/// 무효화 컨텍스트 — 프레임당 무효화 처리 상태
pub struct SlateInvalidationContext {
    pub current_generation: u64,
    pub widgets_laid_out: usize,
    pub widgets_painted: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl SlateInvalidationContext {
    pub fn new() -> Self {
        Self {
            current_generation: 0,
            widgets_laid_out: 0,
            widgets_painted: 0,
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.current_generation += 1;
        self.widgets_laid_out = 0;
        self.widgets_painted = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    pub fn record_layout(&mut self) { self.widgets_laid_out += 1; }
    pub fn record_paint(&mut self) { self.widgets_painted += 1; }
    pub fn record_cache_hit(&mut self) { self.cache_hits += 1; }
    pub fn record_cache_miss(&mut self) { self.cache_misses += 1; }

    pub fn cache_hit_ratio(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 { 0.0 } else { self.cache_hits as f32 / total as f32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_proxy() {
        let proxy = WidgetProxy::new(42);
        assert!(proxy.needs_layout());
        assert!(proxy.needs_paint());
        assert_eq!(proxy.widget_id, 42);
    }

    #[test]
    fn test_widget_proxy_clean() {
        let mut proxy = WidgetProxy::new(1);
        proxy.mark_clean(5);
        assert!(!proxy.needs_layout());
        assert!(!proxy.needs_paint());
        assert_eq!(proxy.layout_generation, 5);
    }

    #[test]
    fn test_invalidation_list_register() {
        let mut list = SlateInvalidationWidgetList::new();
        list.register(1, None);
        list.register(2, Some(1));
        assert_eq!(list.widget_count(), 2);
        assert_eq!(list.dirty_layout_count(), 2);
    }

    #[test]
    fn test_invalidation_list_invalidate() {
        let mut list = SlateInvalidationWidgetList::new();
        list.register(1, None);
        list.register(2, Some(1));
        list.clear_all_dirty();
        assert_eq!(list.dirty_layout_count(), 0);

        list.invalidate(2, InvalidateWidgetReason::LAYOUT);
        assert_eq!(list.dirty_layout_count(), 2); // child + parent
    }

    #[test]
    fn test_invalidation_list_drain() {
        let mut list = SlateInvalidationWidgetList::new();
        list.register(1, None);
        list.register(2, None);
        let layout_ids = list.drain_dirty_layout();
        assert_eq!(layout_ids.len(), 2);
        assert_eq!(list.dirty_layout_count(), 0);
    }

    #[test]
    fn test_invalidation_list_unregister() {
        let mut list = SlateInvalidationWidgetList::new();
        list.register(1, None);
        list.unregister(1);
        assert_eq!(list.widget_count(), 0);
    }

    #[test]
    fn test_cached_element_data() {
        let mut cache = CachedElementData::new(1);
        assert!(!cache.is_valid);
        cache.is_valid = true;
        cache.paint_generation = 3;
        cache.invalidate();
        assert!(!cache.is_valid);
    }

    #[test]
    fn test_invalidation_heap() {
        let mut heap = SlateInvalidationWidgetHeap::new();
        heap.push(3, 2);
        heap.push(1, 0);
        heap.push(2, 1);
        heap.sort_by_depth();
        let entries = heap.drain();
        assert_eq!(entries[0], (1, 0)); // 가장 얕은 것 먼저
        assert_eq!(entries[1], (2, 1));
        assert_eq!(entries[2], (3, 2));
    }

    #[test]
    fn test_invalidation_context() {
        let mut ctx = SlateInvalidationContext::new();
        ctx.begin_frame();
        assert_eq!(ctx.current_generation, 1);
        ctx.record_cache_hit();
        ctx.record_cache_hit();
        ctx.record_cache_miss();
        assert!((ctx.cache_hit_ratio() - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_paint_only_invalidation() {
        let mut list = SlateInvalidationWidgetList::new();
        list.register(1, None);
        list.clear_all_dirty();

        list.invalidate(1, InvalidateWidgetReason::PAINT);
        assert_eq!(list.dirty_paint_count(), 1);
        assert_eq!(list.dirty_layout_count(), 0);
    }
}
