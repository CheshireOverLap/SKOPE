//! 디버그 뷰어 — UI 디버그 정보 수집 및 표시
//!
//! 위젯 트리 구조, 아틀라스 상태, 배칭 통계 등을 시각화.

use std::collections::VecDeque;

/// 디버그 오버레이 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlayMode {
    /// 오버레이 없음
    None,
    /// 위젯 바운딩 박스
    WidgetBounds,
    /// 클리핑 영역
    ClipRegions,
    /// 배치 경계
    BatchBoundaries,
    /// 레이어 시각화
    Layers,
    /// 무효화 영역
    InvalidationRegions,
}

/// 디버그 프레임 정보
#[derive(Debug, Clone, Default)]
pub struct DebugFrameInfo {
    pub frame_number: u64,
    pub widget_count: u32,
    pub visible_widget_count: u32,
    pub draw_call_count: u32,
    pub batch_count: u32,
    pub vertex_count: u32,
    pub total_clip_zones: u32,
    pub invalidated_widgets: u32,
    pub cache_hit_ratio: f32,
    pub frame_time_ms: f32,
    pub layout_time_ms: f32,
    pub paint_time_ms: f32,
}

/// 디버그 위젯 정보
#[derive(Debug, Clone)]
pub struct DebugWidgetInfo {
    pub widget_id: u64,
    pub type_name: String,
    pub position: glam::Vec2,
    pub size: glam::Vec2,
    pub is_visible: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
    pub child_count: u32,
    pub depth: u32,
}

/// UI 디버그 뷰어
pub struct UiDebugViewer {
    enabled: bool,
    overlay_mode: DebugOverlayMode,
    frame_history: VecDeque<DebugFrameInfo>,
    max_history: usize,
    selected_widget: Option<u64>,
    widget_infos: Vec<DebugWidgetInfo>,
    show_fps_counter: bool,
    show_stats_panel: bool,
    show_widget_tree: bool,
}

impl UiDebugViewer {
    pub fn new() -> Self {
        Self {
            enabled: false,
            overlay_mode: DebugOverlayMode::None,
            frame_history: VecDeque::new(),
            max_history: 120, // ~2초 @ 60fps
            selected_widget: None,
            widget_infos: Vec::new(),
            show_fps_counter: false,
            show_stats_panel: false,
            show_widget_tree: false,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_overlay_mode(&mut self, mode: DebugOverlayMode) {
        self.overlay_mode = mode;
    }

    pub fn toggle_fps_counter(&mut self) {
        self.show_fps_counter = !self.show_fps_counter;
    }

    pub fn toggle_stats_panel(&mut self) {
        self.show_stats_panel = !self.show_stats_panel;
    }

    pub fn toggle_widget_tree(&mut self) {
        self.show_widget_tree = !self.show_widget_tree;
    }

    /// 프레임 정보 기록
    pub fn record_frame(&mut self, info: DebugFrameInfo) {
        if !self.enabled { return; }
        self.frame_history.push_back(info);
        while self.frame_history.len() > self.max_history {
            self.frame_history.pop_front();
        }
    }

    /// 위젯 정보 갱신
    pub fn update_widget_infos(&mut self, infos: Vec<DebugWidgetInfo>) {
        self.widget_infos = infos;
    }

    /// 위젯 선택
    pub fn select_widget(&mut self, widget_id: Option<u64>) {
        self.selected_widget = widget_id;
    }

    /// 선택된 위젯 정보
    pub fn selected_widget_info(&self) -> Option<&DebugWidgetInfo> {
        let id = self.selected_widget?;
        self.widget_infos.iter().find(|w| w.widget_id == id)
    }

    /// 평균 FPS (최근 히스토리 기반)
    pub fn average_fps(&self) -> f32 {
        if self.frame_history.is_empty() { return 0.0; }
        let total_ms: f32 = self.frame_history.iter()
            .map(|f| f.frame_time_ms)
            .sum();
        let avg_ms = total_ms / self.frame_history.len() as f32;
        if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 }
    }

    /// 평균 드로우콜 수
    pub fn average_draw_calls(&self) -> f32 {
        if self.frame_history.is_empty() { return 0.0; }
        let total: u32 = self.frame_history.iter()
            .map(|f| f.draw_call_count)
            .sum();
        total as f32 / self.frame_history.len() as f32
    }

    /// 최근 프레임의 레이아웃 시간 비율
    pub fn layout_time_ratio(&self) -> f32 {
        if let Some(last) = self.frame_history.back() {
            if last.frame_time_ms > 0.0 {
                return last.layout_time_ms / last.frame_time_ms;
            }
        }
        0.0
    }

    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn overlay_mode(&self) -> DebugOverlayMode { self.overlay_mode }
    pub fn show_fps_counter(&self) -> bool { self.show_fps_counter }
    pub fn show_stats_panel(&self) -> bool { self.show_stats_panel }
    pub fn show_widget_tree(&self) -> bool { self.show_widget_tree }
    pub fn frame_count(&self) -> usize { self.frame_history.len() }
    pub fn widget_count(&self) -> usize { self.widget_infos.len() }
}

/// 아틀라스 디버그 정보
#[derive(Debug, Clone)]
pub struct AtlasDebugInfo {
    pub atlas_size: (u32, u32),
    pub used_slots: u32,
    pub total_slots: u32,
    pub fill_ratio: f32,
    pub largest_free_region: (u32, u32),
    pub format: String,
}

/// 아틀라스 디버그 뷰어
pub struct AtlasDebugViewer {
    atlases: Vec<AtlasDebugInfo>,
    selected_atlas: usize,
    show_grid: bool,
    show_used_regions: bool,
    zoom: f32,
}

impl AtlasDebugViewer {
    pub fn new() -> Self {
        Self {
            atlases: Vec::new(),
            selected_atlas: 0,
            show_grid: true,
            show_used_regions: true,
            zoom: 1.0,
        }
    }

    pub fn update_atlas_info(&mut self, infos: Vec<AtlasDebugInfo>) {
        self.atlases = infos;
    }

    pub fn select_atlas(&mut self, index: usize) {
        if index < self.atlases.len() {
            self.selected_atlas = index;
        }
    }

    pub fn selected_atlas_info(&self) -> Option<&AtlasDebugInfo> {
        self.atlases.get(self.selected_atlas)
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.1, 10.0);
    }

    pub fn toggle_grid(&mut self) { self.show_grid = !self.show_grid; }
    pub fn toggle_used_regions(&mut self) { self.show_used_regions = !self.show_used_regions; }

    pub fn atlas_count(&self) -> usize { self.atlases.len() }
    pub fn zoom(&self) -> f32 { self.zoom }
    pub fn show_grid(&self) -> bool { self.show_grid }
    pub fn show_used_regions(&self) -> bool { self.show_used_regions }

    /// 전체 아틀라스 메모리 사용량 (바이트)
    pub fn total_memory_usage(&self) -> u64 {
        self.atlases.iter()
            .map(|a| (a.atlas_size.0 as u64) * (a.atlas_size.1 as u64) * 4) // RGBA
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_viewer_creation() {
        let viewer = UiDebugViewer::new();
        assert!(!viewer.is_enabled());
        assert_eq!(viewer.overlay_mode(), DebugOverlayMode::None);
    }

    #[test]
    fn test_debug_viewer_frame_recording() {
        let mut viewer = UiDebugViewer::new();
        viewer.set_enabled(true);
        for i in 0..5 {
            viewer.record_frame(DebugFrameInfo {
                frame_number: i,
                frame_time_ms: 16.6,
                draw_call_count: 10,
                ..Default::default()
            });
        }
        assert_eq!(viewer.frame_count(), 5);
        assert!((viewer.average_fps() - 60.24).abs() < 1.0);
    }

    #[test]
    fn test_debug_viewer_disabled_no_record() {
        let mut viewer = UiDebugViewer::new();
        // disabled → 기록 안 함
        viewer.record_frame(DebugFrameInfo::default());
        assert_eq!(viewer.frame_count(), 0);
    }

    #[test]
    fn test_debug_viewer_widget_selection() {
        let mut viewer = UiDebugViewer::new();
        viewer.update_widget_infos(vec![
            DebugWidgetInfo {
                widget_id: 42,
                type_name: "SButton".into(),
                position: glam::Vec2::ZERO,
                size: glam::Vec2::new(100.0, 30.0),
                is_visible: true,
                is_hovered: false,
                is_focused: false,
                child_count: 0,
                depth: 2,
            },
        ]);
        viewer.select_widget(Some(42));
        let info = viewer.selected_widget_info().unwrap();
        assert_eq!(info.type_name, "SButton");
    }

    #[test]
    fn test_atlas_debug_viewer() {
        let mut viewer = AtlasDebugViewer::new();
        viewer.update_atlas_info(vec![
            AtlasDebugInfo {
                atlas_size: (1024, 1024),
                used_slots: 50,
                total_slots: 100,
                fill_ratio: 0.5,
                largest_free_region: (256, 256),
                format: "RGBA8".into(),
            },
        ]);
        assert_eq!(viewer.atlas_count(), 1);
        assert_eq!(viewer.total_memory_usage(), 1024 * 1024 * 4);
    }

    #[test]
    fn test_atlas_debug_zoom() {
        let mut viewer = AtlasDebugViewer::new();
        viewer.set_zoom(5.0);
        assert_eq!(viewer.zoom(), 5.0);
        viewer.set_zoom(20.0); // clamped
        assert_eq!(viewer.zoom(), 10.0);
    }

    #[test]
    fn test_overlay_modes() {
        let mut viewer = UiDebugViewer::new();
        viewer.set_overlay_mode(DebugOverlayMode::WidgetBounds);
        assert_eq!(viewer.overlay_mode(), DebugOverlayMode::WidgetBounds);
        viewer.set_overlay_mode(DebugOverlayMode::BatchBoundaries);
        assert_eq!(viewer.overlay_mode(), DebugOverlayMode::BatchBoundaries);
    }
}
