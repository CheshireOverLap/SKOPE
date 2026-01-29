//! Output Log Panel - 출력 로그 패널
//!
//! 로그 레벨별 필터링, 스크롤, 자동 스크롤 지원.

use std::any::Any;
use glam::Vec2;

use crate::core::{
    Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;
use crate::widget::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 로그 레벨
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Verbose,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn label(&self) -> &'static str {
        match self {
            LogLevel::Verbose => "Verbose",
            LogLevel::Info => "Info",
            LogLevel::Warning => "Warning",
            LogLevel::Error => "Error",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            LogLevel::Verbose => Color::rgba(0.5, 0.5, 0.5, 1.0),
            LogLevel::Info => Color::rgba(0.85, 0.85, 0.85, 1.0),
            LogLevel::Warning => Color::rgba(1.0, 0.85, 0.2, 1.0),
            LogLevel::Error => Color::rgba(1.0, 0.3, 0.3, 1.0),
        }
    }
}

/// 로그 항목
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: f64,
    pub level: LogLevel,
    pub category: String,
    pub message: String,
}

/// 출력 로그 스타일
#[derive(Debug, Clone)]
pub struct OutputLogStyle {
    pub background_color: Color,
    pub alt_row_color: Color,
    pub font_size: f32,
    pub line_height: f32,
    pub padding: f32,
    pub timestamp_color: Color,
    pub category_color: Color,
}

impl Default for OutputLogStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.1, 0.1, 0.12, 1.0),
            alt_row_color: Color::rgba(0.12, 0.12, 0.14, 1.0),
            font_size: 11.0,
            line_height: 18.0,
            padding: 4.0,
            timestamp_color: Color::rgba(0.4, 0.4, 0.4, 1.0),
            category_color: Color::rgba(0.5, 0.7, 0.9, 1.0),
        }
    }
}

/// 출력 로그 패널
pub struct SOutputLog {
    /// 위젯 고유 ID
    id: u64,
    entries: Vec<LogEntry>,
    max_entries: usize,
    scroll_offset: f32,
    auto_scroll: bool,
    filter_level: LogLevel,
    filter_text: String,
    /// 필터 적용된 인덱스 캐시
    filtered_indices: Vec<usize>,
    hovered_index: Option<usize>,
    style: OutputLogStyle,
    visibility: Visibility,
    enabled: bool,
    dirty: InvalidateWidgetReason,
}

impl SOutputLog {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            entries: Vec::new(),
            max_entries: 10000,
            scroll_offset: 0.0,
            auto_scroll: true,
            filter_level: LogLevel::Verbose,
            filter_text: String::new(),
            filtered_indices: Vec::new(),
            hovered_index: None,
            style: OutputLogStyle::default(),
            visibility: Visibility::Visible,
            enabled: true,
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
        }
    }

    /// 로그 추가
    pub fn add_log(&mut self, level: LogLevel, category: impl Into<String>, message: impl Into<String>) {
        let entry = LogEntry {
            timestamp: 0.0, // 외부에서 설정
            level,
            category: category.into(),
            message: message.into(),
        };
        self.entries.push(entry);

        // 최대 개수 초과 시 앞에서 제거
        if self.entries.len() > self.max_entries {
            self.entries.drain(0..self.entries.len() - self.max_entries);
        }

        self.rebuild_filter();

        if self.auto_scroll {
            self.scroll_to_bottom();
        }

        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 타임스탬프 포함 로그 추가
    pub fn add_log_with_time(
        &mut self,
        timestamp: f64,
        level: LogLevel,
        category: impl Into<String>,
        message: impl Into<String>,
    ) {
        let entry = LogEntry {
            timestamp,
            level,
            category: category.into(),
            message: message.into(),
        };
        self.entries.push(entry);

        if self.entries.len() > self.max_entries {
            self.entries.drain(0..self.entries.len() - self.max_entries);
        }

        self.rebuild_filter();
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 전체 삭제
    pub fn clear(&mut self) {
        self.entries.clear();
        self.filtered_indices.clear();
        self.scroll_offset = 0.0;
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 필터 레벨 설정
    pub fn set_filter_level(&mut self, level: LogLevel) {
        self.filter_level = level;
        self.rebuild_filter();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 텍스트 필터 설정
    pub fn set_filter_text(&mut self, text: impl Into<String>) {
        self.filter_text = text.into();
        self.rebuild_filter();
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    /// 자동 스크롤 토글
    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.auto_scroll = enabled;
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    fn rebuild_filter(&mut self) {
        self.filtered_indices.clear();
        let filter_lower = self.filter_text.to_lowercase();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.level < self.filter_level {
                continue;
            }
            if !filter_lower.is_empty() {
                let msg_lower = entry.message.to_lowercase();
                let cat_lower = entry.category.to_lowercase();
                if !msg_lower.contains(&filter_lower) && !cat_lower.contains(&filter_lower) {
                    continue;
                }
            }
            self.filtered_indices.push(i);
        }
    }

    fn scroll_to_bottom(&mut self) {
        let total_height = self.filtered_indices.len() as f32 * self.style.line_height;
        self.scroll_offset = total_height; // on_paint에서 clamp됨
    }

    fn format_timestamp(t: f64) -> String {
        let secs = t as u64;
        let mins = (secs / 60) % 60;
        let hours = (secs / 3600) % 24;
        let s = secs % 60;
        let ms = ((t - t.floor()) * 1000.0) as u32;
        format!("[{:02}:{:02}:{:02}.{:03}]", hours, mins, s, ms)
    }

    fn measure_text(text: &str, font_size: f32, font_scale: f32) -> f32 {
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(text, font_size, FontFamily::UI, font_scale)
        } else {
            let scaled = font_size * font_scale;
            text.chars().count() as f32 * scaled * 0.5
        }
    }
}

impl Widget for SOutputLog {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(400.0, 200.0) // 최소 사이즈
    }

    fn type_name(&self) -> &'static str {
        "SOutputLog"
    }

    fn widget_id(&self) -> u64 { self.id }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let pos = geometry.absolute_position;
        let size = geometry.local_size;
        let mut current_layer = layer;

        // 배경
        let bg_geo = PaintGeometry::new(pos, size, geometry.scale);
        draw_elements.add_box(current_layer, bg_geo, self.style.background_color);
        current_layer += 1;

        if self.filtered_indices.is_empty() {
            return current_layer;
        }

        let visible_lines = (size.y / self.style.line_height).ceil() as usize + 1;
        let total_lines = self.filtered_indices.len();
        let max_scroll = ((total_lines as f32 * self.style.line_height) - size.y).max(0.0);
        let scroll = self.scroll_offset.min(max_scroll).max(0.0);

        let start_line = (scroll / self.style.line_height).floor() as usize;
        let y_offset = -(scroll % self.style.line_height);

        let timestamp_x = pos.x + self.style.padding;
        let category_x = timestamp_x + Self::measure_text("[00:00:00.000]", self.style.font_size, 1.0) + 4.0;
        let message_x = category_x + 80.0;

        for vis_i in 0..visible_lines {
            let line_i = start_line + vis_i;
            if line_i >= total_lines {
                break;
            }

            let entry_idx = self.filtered_indices[line_i];
            let entry = &self.entries[entry_idx];
            let line_y = pos.y + y_offset + vis_i as f32 * self.style.line_height;

            // 줄 범위 밖 체크
            if line_y + self.style.line_height < pos.y || line_y > pos.y + size.y {
                continue;
            }

            // 교대 배경
            if line_i % 2 == 1 {
                let row_geo = PaintGeometry::new(
                    Vec2::new(pos.x, line_y),
                    Vec2::new(size.x, self.style.line_height),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer, row_geo, self.style.alt_row_color);
            }

            let text_y = line_y + (self.style.line_height - self.style.font_size) * 0.5;

            // 타임스탬프
            let ts_text = Self::format_timestamp(entry.timestamp);
            let ts_geo = PaintGeometry::new(
                Vec2::new(timestamp_x, text_y),
                Vec2::new(100.0, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer + 1,
                ts_geo,
                ts_text,
                self.style.timestamp_color,
                self.style.font_size,
            );

            // 카테고리
            let cat_geo = PaintGeometry::new(
                Vec2::new(category_x, text_y),
                Vec2::new(76.0, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer + 1,
                cat_geo,
                entry.category.clone(),
                self.style.category_color,
                self.style.font_size,
            );

            // 메시지
            let msg_geo = PaintGeometry::new(
                Vec2::new(message_x, text_y),
                Vec2::new(size.x - (message_x - pos.x) - self.style.padding, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(
                current_layer + 1,
                msg_geo,
                entry.message.clone(),
                entry.level.color(),
                self.style.font_size,
            );
        }
        current_layer += 2;

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

impl LeafWidget for SOutputLog {}
