//! Debugging Infrastructure — 성능 카운터, 위젯 리스트, 이벤트 트레이서
//!
//! UE 참조: `SlateCore/Public/Debugging/SlateDebugging.h`
//!
//! 성능 프로파일링, 위젯 추적, 입력 이벤트 트레이싱을 제공합니다.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

// ============================================================================
// SlatePerformanceCounters — 프레임별 성능 카운터
// ============================================================================

/// Slate 렌더링 성능 카운터
///
/// 프레임 단위로 드로우 콜, 엘리먼트 수, 페인트 시간 등을 추적합니다.
#[derive(Debug, Clone)]
pub struct SlatePerformanceCounters {
    /// 총 프레임 수
    pub frame_count: u64,
    /// 현재 프레임 드로우 콜 수
    pub draw_calls: u32,
    /// 현재 프레임 드로우 엘리먼트 수
    pub element_count: u32,
    /// 현재 프레임 페인트 시간 (밀리초)
    pub paint_time_ms: f64,
    /// 현재 프레임 위젯 수
    pub widget_count: u32,
    /// 현재 프레임 가시 위젯 수
    pub visible_widget_count: u32,
    /// 프레임 시작 시각 (내부)
    frame_start: Option<Instant>,
    /// 최근 N 프레임 페인트 시간 (이동 평균)
    recent_paint_times: Vec<f64>,
}

impl Default for SlatePerformanceCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl SlatePerformanceCounters {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            draw_calls: 0,
            element_count: 0,
            paint_time_ms: 0.0,
            widget_count: 0,
            visible_widget_count: 0,
            frame_start: None,
            recent_paint_times: Vec::with_capacity(60),
        }
    }

    /// 프레임 시작
    pub fn begin_frame(&mut self) {
        self.draw_calls = 0;
        self.element_count = 0;
        self.widget_count = 0;
        self.visible_widget_count = 0;
        self.frame_start = Some(Instant::now());
    }

    /// 프레임 종료
    pub fn end_frame(&mut self) {
        self.frame_count += 1;
        if let Some(start) = self.frame_start.take() {
            self.paint_time_ms = start.elapsed().as_secs_f64() * 1000.0;
            // 이동 평균 (최근 60 프레임)
            if self.recent_paint_times.len() >= 60 {
                self.recent_paint_times.remove(0);
            }
            self.recent_paint_times.push(self.paint_time_ms);
        }
    }

    /// 드로우 콜 기록
    pub fn record_draw_call(&mut self) {
        self.draw_calls += 1;
    }

    /// 드로우 엘리먼트 수 기록
    pub fn record_elements(&mut self, count: u32) {
        self.element_count += count;
    }

    /// 위젯 수 기록
    pub fn record_widget(&mut self, visible: bool) {
        self.widget_count += 1;
        if visible {
            self.visible_widget_count += 1;
        }
    }

    /// 페인트 시간 직접 기록 (Duration)
    pub fn record_paint_time(&mut self, duration: Duration) {
        self.paint_time_ms = duration.as_secs_f64() * 1000.0;
    }

    /// 평균 페인트 시간 (최근 60 프레임)
    pub fn avg_paint_time_ms(&self) -> f64 {
        if self.recent_paint_times.is_empty() {
            return 0.0;
        }
        self.recent_paint_times.iter().sum::<f64>() / self.recent_paint_times.len() as f64
    }

    /// FPS 추정 (평균 페인트 시간 기반)
    pub fn estimated_fps(&self) -> f64 {
        let avg = self.avg_paint_time_ms();
        if avg > 0.0 { 1000.0 / avg } else { 0.0 }
    }

    /// 한 줄 요약 텍스트
    pub fn summary_text(&self) -> String {
        format!(
            "Frame {} | {:.1}ms ({:.0} FPS) | DC:{} Elem:{} | W:{}/{}",
            self.frame_count,
            self.paint_time_ms,
            self.estimated_fps(),
            self.draw_calls,
            self.element_count,
            self.visible_widget_count,
            self.widget_count,
        )
    }

    /// 리셋
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ============================================================================
// GlobalWidgetList — 전역 위젯 추적
// ============================================================================

/// 위젯 리스트 엔트리
#[derive(Debug, Clone)]
pub struct WidgetListEntry {
    /// 위젯 ID
    pub id: u64,
    /// 위젯 타입 이름
    pub type_name: &'static str,
    /// 생성 시각
    pub created_at: Instant,
}

/// 전역 라이브 위젯 리스트 (메모리 디버깅)
///
/// 모든 활성 위젯을 추적합니다. 메모리 누수 감지에 사용.
pub struct GlobalWidgetList {
    entries: Vec<WidgetListEntry>,
}

impl Default for GlobalWidgetList {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalWidgetList {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// 싱글톤 인스턴스
    pub fn instance() -> &'static RwLock<GlobalWidgetList> {
        static INSTANCE: OnceLock<RwLock<GlobalWidgetList>> = OnceLock::new();
        INSTANCE.get_or_init(|| RwLock::new(GlobalWidgetList::new()))
    }

    /// 위젯 등록
    pub fn register(&mut self, id: u64, type_name: &'static str) {
        self.entries.push(WidgetListEntry {
            id,
            type_name,
            created_at: Instant::now(),
        });
    }

    /// 위젯 등록 해제
    pub fn unregister(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    /// 총 위젯 수
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// 타입별 위젯 수
    pub fn count_by_type(&self) -> HashMap<&'static str, usize> {
        let mut counts: HashMap<&'static str, usize> = HashMap::new();
        for entry in &self.entries {
            *counts.entry(entry.type_name).or_insert(0) += 1;
        }
        counts
    }

    /// 모든 엔트리 참조
    pub fn entries(&self) -> &[WidgetListEntry] {
        &self.entries
    }

    /// 리셋 (테스트용)
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ============================================================================
// InputEventTracer — 입력 이벤트 트레이서
// ============================================================================

/// 이벤트 트레이스 엔트리
#[derive(Debug, Clone)]
pub struct EventTraceEntry {
    /// 이벤트 타입 (예: "MouseDown", "KeyDown", "Focus")
    pub event_type: &'static str,
    /// 타임스탬프
    pub timestamp: Instant,
    /// 처리한 위젯 ID (None = 미처리)
    pub widget_id: Option<u64>,
    /// 위젯 타입명
    pub widget_type: Option<&'static str>,
    /// 처리 여부
    pub handled: bool,
}

/// 입력 이벤트 트레이서 (링 버퍼)
///
/// 최근 N개 이벤트를 추적하여 입력 라우팅 디버깅에 사용.
pub struct InputEventTracer {
    /// 이벤트 버퍼
    events: Vec<EventTraceEntry>,
    /// 최대 크기
    capacity: usize,
    /// 활성 여부
    pub enabled: bool,
}

impl Default for InputEventTracer {
    fn default() -> Self {
        Self::new(256)
    }
}

impl InputEventTracer {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity),
            capacity,
            enabled: false,
        }
    }

    /// 이벤트 기록
    pub fn trace_event(
        &mut self,
        event_type: &'static str,
        widget_id: Option<u64>,
        widget_type: Option<&'static str>,
        handled: bool,
    ) {
        if !self.enabled {
            return;
        }

        // 링 버퍼 — 오래된 이벤트 제거
        if self.events.len() >= self.capacity {
            self.events.remove(0);
        }

        self.events.push(EventTraceEntry {
            event_type,
            timestamp: Instant::now(),
            widget_id,
            widget_type,
            handled,
        });
    }

    /// 최근 이벤트들
    pub fn recent_events(&self) -> &[EventTraceEntry] {
        &self.events
    }

    /// 최근 N개 이벤트
    pub fn last_n(&self, n: usize) -> &[EventTraceEntry] {
        let start = self.events.len().saturating_sub(n);
        &self.events[start..]
    }

    /// 특정 타입 이벤트만 필터
    pub fn filter_by_type(&self, event_type: &str) -> Vec<&EventTraceEntry> {
        self.events.iter()
            .filter(|e| e.event_type == event_type)
            .collect()
    }

    /// 이벤트 수
    pub fn count(&self) -> usize {
        self.events.len()
    }

    /// 클리어
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// ============================================================================
// WidgetTreeSnapshot — 위젯 트리 스냅샷
// ============================================================================

/// 위젯 트리 스냅샷 노드
#[derive(Debug, Clone)]
pub struct SnapshotNode {
    /// 위젯 타입명
    pub type_name: &'static str,
    /// 위젯 ID
    pub id: u64,
    /// 자식 노드
    pub children: Vec<SnapshotNode>,
}

/// 스냅샷 간 차이
#[derive(Debug, Clone)]
pub enum SnapshotDiff {
    /// 새로 추가된 위젯
    Added { type_name: &'static str, id: u64 },
    /// 제거된 위젯
    Removed { type_name: &'static str, id: u64 },
}

/// 위젯 트리 스냅샷 유틸리티
pub struct WidgetTreeSnapshot;

impl WidgetTreeSnapshot {
    /// 위젯 트리의 현재 상태를 캡처
    pub fn capture(root: &dyn crate::widget::Widget) -> SnapshotNode {
        let mut children = Vec::new();
        for i in 0..root.num_children() {
            if let Some(child) = root.get_child(i) {
                children.push(Self::capture(child));
            }
        }
        SnapshotNode {
            type_name: root.type_name(),
            id: root.widget_id(),
            children,
        }
    }

    /// 두 스냅샷 간 차이 계산
    pub fn diff(old: &SnapshotNode, new: &SnapshotNode) -> Vec<SnapshotDiff> {
        let mut diffs = Vec::new();

        let old_ids = Self::collect_ids(old);
        let new_ids = Self::collect_ids(new);

        for (id, type_name) in &new_ids {
            if !old_ids.contains_key(id) {
                diffs.push(SnapshotDiff::Added { type_name, id: *id });
            }
        }

        for (id, type_name) in &old_ids {
            if !new_ids.contains_key(id) {
                diffs.push(SnapshotDiff::Removed { type_name, id: *id });
            }
        }

        diffs
    }

    /// 스냅샷에서 모든 ID 수집
    fn collect_ids(node: &SnapshotNode) -> HashMap<u64, &'static str> {
        let mut map = HashMap::new();
        Self::collect_ids_recursive(node, &mut map);
        map
    }

    fn collect_ids_recursive(node: &SnapshotNode, map: &mut HashMap<u64, &'static str>) {
        map.insert(node.id, node.type_name);
        for child in &node.children {
            Self::collect_ids_recursive(child, map);
        }
    }

    /// 트리의 총 노드 수
    pub fn count_nodes(node: &SnapshotNode) -> usize {
        1 + node.children.iter().map(|c| Self::count_nodes(c)).sum::<usize>()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_counters_basic() {
        let mut counters = SlatePerformanceCounters::new();
        counters.begin_frame();
        counters.record_draw_call();
        counters.record_draw_call();
        counters.record_elements(100);
        counters.record_widget(true);
        counters.record_widget(true);
        counters.record_widget(false);
        counters.end_frame();

        assert_eq!(counters.frame_count, 1);
        assert_eq!(counters.draw_calls, 2);
        assert_eq!(counters.element_count, 100);
        assert_eq!(counters.widget_count, 3);
        assert_eq!(counters.visible_widget_count, 2);
        assert!(counters.paint_time_ms >= 0.0);
    }

    #[test]
    fn test_performance_counters_summary() {
        let mut counters = SlatePerformanceCounters::new();
        counters.begin_frame();
        counters.record_draw_call();
        counters.record_elements(50);
        counters.end_frame();

        let summary = counters.summary_text();
        assert!(summary.contains("Frame 1"));
        assert!(summary.contains("DC:1"));
        assert!(summary.contains("Elem:50"));
    }

    #[test]
    fn test_performance_counters_reset() {
        let mut counters = SlatePerformanceCounters::new();
        counters.begin_frame();
        counters.record_draw_call();
        counters.end_frame();
        assert_eq!(counters.frame_count, 1);

        counters.reset();
        assert_eq!(counters.frame_count, 0);
        assert_eq!(counters.draw_calls, 0);
    }

    #[test]
    fn test_global_widget_list() {
        let mut list = GlobalWidgetList::new();
        list.register(1, "SButton");
        list.register(2, "STextBlock");
        list.register(3, "SButton");

        assert_eq!(list.count(), 3);

        let by_type = list.count_by_type();
        assert_eq!(by_type["SButton"], 2);
        assert_eq!(by_type["STextBlock"], 1);

        list.unregister(1);
        assert_eq!(list.count(), 2);
        assert_eq!(list.count_by_type()["SButton"], 1);
    }

    #[test]
    fn test_input_event_tracer() {
        let mut tracer = InputEventTracer::new(4);
        tracer.enabled = true;

        tracer.trace_event("MouseDown", Some(1), Some("SButton"), true);
        tracer.trace_event("MouseUp", Some(1), Some("SButton"), true);
        tracer.trace_event("KeyDown", None, None, false);
        tracer.trace_event("Focus", Some(2), Some("STextBlock"), true);

        assert_eq!(tracer.count(), 4);
        assert_eq!(tracer.last_n(2).len(), 2);

        let mouse_events = tracer.filter_by_type("MouseDown");
        assert_eq!(mouse_events.len(), 1);

        // 링 버퍼 — 초과 시 오래된 것 제거
        tracer.trace_event("Extra", None, None, false);
        assert_eq!(tracer.count(), 4); // capacity=4이므로 최대 4개
    }

    #[test]
    fn test_tracer_disabled() {
        let mut tracer = InputEventTracer::new(256);
        // enabled = false (기본)
        tracer.trace_event("MouseDown", Some(1), None, true);
        assert_eq!(tracer.count(), 0); // 비활성이므로 기록 안됨
    }

    #[test]
    fn test_widget_tree_snapshot() {
        use crate::widget::SNullWidget;

        let root = SNullWidget::new();
        let snapshot = WidgetTreeSnapshot::capture(&root);

        assert_eq!(snapshot.type_name, "SNullWidget");
        assert_eq!(snapshot.children.len(), 0);
        assert_eq!(WidgetTreeSnapshot::count_nodes(&snapshot), 1);
    }

    #[test]
    fn test_snapshot_diff() {
        let old = SnapshotNode {
            type_name: "Root",
            id: 1,
            children: vec![
                SnapshotNode { type_name: "A", id: 2, children: vec![] },
                SnapshotNode { type_name: "B", id: 3, children: vec![] },
            ],
        };
        let new = SnapshotNode {
            type_name: "Root",
            id: 1,
            children: vec![
                SnapshotNode { type_name: "A", id: 2, children: vec![] },
                SnapshotNode { type_name: "C", id: 4, children: vec![] },
            ],
        };

        let diffs = WidgetTreeSnapshot::diff(&old, &new);

        let added: Vec<_> = diffs.iter().filter(|d| matches!(d, SnapshotDiff::Added { .. })).collect();
        let removed: Vec<_> = diffs.iter().filter(|d| matches!(d, SnapshotDiff::Removed { .. })).collect();

        assert_eq!(added.len(), 1);
        assert_eq!(removed.len(), 1);

        match added[0] {
            SnapshotDiff::Added { id, .. } => assert_eq!(*id, 4),
            _ => panic!(),
        }
        match removed[0] {
            SnapshotDiff::Removed { id, .. } => assert_eq!(*id, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn test_avg_paint_time() {
        let mut counters = SlatePerformanceCounters::new();
        // 시뮬레이트: 3 프레임
        for _ in 0..3 {
            counters.begin_frame();
            counters.end_frame();
        }
        assert_eq!(counters.frame_count, 3);
        // 평균은 양수 (실제 시간 의존)
        assert!(counters.avg_paint_time_ms() >= 0.0);
    }
}
