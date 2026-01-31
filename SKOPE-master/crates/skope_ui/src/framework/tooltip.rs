//! Tooltip System - 툴팁 관리
//!
//! 마우스 호버 시 지연 표시되는 툴팁을 관리합니다.

use glam::Vec2;
use crate::core::{Color, PaintGeometry, SlateRect};
use crate::widget::{DrawElementList, WidgetId};

// ============================================================================
// TooltipContent
// ============================================================================

/// 툴팁 콘텐츠
#[derive(Clone)]
pub enum TooltipContent {
    /// 단순 텍스트
    Text(String),
    /// 리치 텍스트 (볼드, 색상 등)
    RichText(String),
    /// 커스텀 (위젯 ID 참조)
    Custom,
}

impl TooltipContent {
    /// 텍스트 툴팁
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// 빈 내용인지
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(s) => s.is_empty(),
            Self::RichText(s) => s.is_empty(),
            Self::Custom => false,
        }
    }
}

impl From<&str> for TooltipContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<String> for TooltipContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

// ============================================================================
// TooltipStyle
// ============================================================================

/// 툴팁 스타일
#[derive(Debug, Clone)]
pub struct TooltipStyle {
    /// 배경색
    pub background_color: Color,
    /// 테두리색
    pub border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 텍스트 색상
    pub text_color: Color,
    /// 패딩
    pub padding: f32,
    /// 최대 너비
    pub max_width: f32,
    /// 폰트 크기
    pub font_size: f32,
    /// 코너 반경
    pub corner_radius: f32,
    /// 그림자 오프셋
    pub shadow_offset: Vec2,
    /// 그림자 색상
    pub shadow_color: Color,
}

impl Default for TooltipStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.12, 0.12, 0.14, 0.95),
            border_color: Color::rgba(0.3, 0.3, 0.35, 1.0),
            border_width: 1.0,
            text_color: Color::rgba(0.9, 0.9, 0.9, 1.0),
            padding: 8.0,
            max_width: 300.0,
            font_size: 13.0,
            corner_radius: 4.0,
            shadow_offset: Vec2::new(2.0, 2.0),
            shadow_color: Color::rgba(0.0, 0.0, 0.0, 0.3),
        }
    }
}

// ============================================================================
// TooltipState
// ============================================================================

/// 현재 표시 중인 툴팁 상태
struct ActiveTooltip {
    /// 콘텐츠
    content: TooltipContent,
    /// 위치
    position: Vec2,
    /// 크기
    size: Vec2,
    /// 소스 위젯
    source_widget: WidgetId,
    /// 표시 시작 시간
    show_time: f64,
    /// 페이드 인 진행도 (0~1)
    fade_progress: f32,
}

/// 대기 중인 툴팁
struct PendingTooltip {
    /// 콘텐츠
    content: TooltipContent,
    /// 소스 위젯
    source_widget: WidgetId,
    /// 호버 시작 시간
    hover_start_time: f64,
    /// 마우스 위치
    cursor_position: Vec2,
}

// ============================================================================
// TooltipManager
// ============================================================================

/// 툴팁 관리자
pub struct TooltipManager {
    /// 현재 표시 중인 툴팁
    active_tooltip: Option<ActiveTooltip>,
    /// 대기 중인 툴팁
    pending_tooltip: Option<PendingTooltip>,
    /// 표시 지연 시간 (초)
    show_delay: f32,
    /// 사라지기 지연 시간 (초)
    hide_delay: f32,
    /// 페이드 인 시간 (초)
    fade_in_duration: f32,
    /// 스타일
    style: TooltipStyle,
    /// 윈도우 크기
    window_size: Vec2,
    /// 현재 시간
    current_time: f64,
    /// 커서 오프셋
    cursor_offset: Vec2,
}

impl Default for TooltipManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TooltipManager {
    /// 새 툴팁 관리자
    pub fn new() -> Self {
        Self {
            active_tooltip: None,
            pending_tooltip: None,
            show_delay: 0.5,
            hide_delay: 0.1,
            fade_in_duration: 0.15,
            style: TooltipStyle::default(),
            window_size: Vec2::new(1920.0, 1080.0),
            current_time: 0.0,
            cursor_offset: Vec2::new(16.0, 16.0),
        }
    }

    /// 표시 지연 시간 설정
    pub fn set_show_delay(&mut self, delay: f32) {
        self.show_delay = delay;
    }

    /// 스타일 설정
    pub fn set_style(&mut self, style: TooltipStyle) {
        self.style = style;
    }

    /// 윈도우 크기 설정
    pub fn set_window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }

    /// 위젯 호버 시 호출
    pub fn on_widget_hover(
        &mut self,
        widget_id: WidgetId,
        content: TooltipContent,
        cursor_pos: Vec2,
        current_time: f64,
    ) {
        // 이미 같은 위젯의 툴팁이 대기/표시 중이면 무시
        if let Some(ref pending) = self.pending_tooltip {
            if pending.source_widget == widget_id {
                return;
            }
        }
        if let Some(ref active) = self.active_tooltip {
            if active.source_widget == widget_id {
                return;
            }
        }

        // 빈 콘텐츠면 무시
        if content.is_empty() {
            return;
        }

        // 대기 등록
        self.pending_tooltip = Some(PendingTooltip {
            content,
            source_widget: widget_id,
            hover_start_time: current_time,
            cursor_position: cursor_pos,
        });
    }

    /// 위젯 호버 종료 시 호출
    pub fn on_widget_leave(&mut self, widget_id: WidgetId) {
        // 대기 중인 툴팁 취소
        if let Some(ref pending) = self.pending_tooltip {
            if pending.source_widget == widget_id {
                self.pending_tooltip = None;
            }
        }

        // 표시 중인 툴팁 숨기기
        if let Some(ref active) = self.active_tooltip {
            if active.source_widget == widget_id {
                self.active_tooltip = None;
            }
        }
    }

    /// 모든 툴팁 숨기기
    pub fn hide_all(&mut self) {
        self.active_tooltip = None;
        self.pending_tooltip = None;
    }

    /// 매 프레임 업데이트
    pub fn tick(&mut self, current_time: f64, cursor_pos: Vec2, delta_time: f32) {
        self.current_time = current_time;

        // 대기 중인 툴팁 확인
        if let Some(ref pending) = self.pending_tooltip.take() {
            let elapsed = current_time - pending.hover_start_time;

            if elapsed >= self.show_delay as f64 {
                // 표시 시간 도달, 활성화
                let size = self.calculate_tooltip_size(&pending.content);
                let position = self.calculate_tooltip_position(cursor_pos, size);

                self.active_tooltip = Some(ActiveTooltip {
                    content: pending.content.clone(),
                    position,
                    size,
                    source_widget: pending.source_widget,
                    show_time: current_time,
                    fade_progress: 0.0,
                });
            } else {
                // 아직 대기 중, 위치 업데이트
                self.pending_tooltip = Some(PendingTooltip {
                    content: pending.content.clone(),
                    source_widget: pending.source_widget,
                    hover_start_time: pending.hover_start_time,
                    cursor_position: cursor_pos,
                });
            }
        }

        // 활성 툴팁 페이드 인
        if let Some(ref mut active) = self.active_tooltip {
            if active.fade_progress < 1.0 {
                active.fade_progress =
                    (active.fade_progress + delta_time / self.fade_in_duration).min(1.0);
            }
        }
    }

    /// 툴팁 크기 계산
    fn calculate_tooltip_size(&self, content: &TooltipContent) -> Vec2 {
        let text_len = match content {
            TooltipContent::Text(s) => s.len(),
            TooltipContent::RichText(s) => s.len(),
            TooltipContent::Custom => 100,
        };

        // 대략적인 크기 계산 (실제로는 텍스트 측정 필요)
        let char_width = self.style.font_size * 0.6;
        let line_height = self.style.font_size * 1.4;

        let max_chars_per_line = (self.style.max_width / char_width) as usize;
        let num_lines = (text_len / max_chars_per_line).max(1);

        let width = (text_len.min(max_chars_per_line) as f32 * char_width + self.style.padding * 2.0)
            .min(self.style.max_width);
        let height = num_lines as f32 * line_height + self.style.padding * 2.0;

        Vec2::new(width, height)
    }

    /// 툴팁 위치 계산
    fn calculate_tooltip_position(&self, cursor_pos: Vec2, size: Vec2) -> Vec2 {
        let mut pos = cursor_pos + self.cursor_offset;

        // 오른쪽 경계
        if pos.x + size.x > self.window_size.x {
            pos.x = cursor_pos.x - size.x - 4.0;
        }

        // 아래 경계
        if pos.y + size.y > self.window_size.y {
            pos.y = cursor_pos.y - size.y - 4.0;
        }

        // 최소 0
        pos.x = pos.x.max(0.0);
        pos.y = pos.y.max(0.0);

        pos
    }

    /// 툴팁이 표시 중인지
    pub fn is_showing(&self) -> bool {
        self.active_tooltip.is_some()
    }

    /// 툴팁 렌더링
    pub fn paint(&self, draw_elements: &mut DrawElementList, base_layer: u32) -> u32 {
        let Some(ref tooltip) = self.active_tooltip else {
            return base_layer;
        };

        let mut layer = base_layer;
        let alpha = tooltip.fade_progress;

        // 그림자
        let shadow_pos = tooltip.position + self.style.shadow_offset;
        let shadow_geo = PaintGeometry::new(shadow_pos, tooltip.size, 1.0);
        draw_elements.add_box(
            layer,
            shadow_geo,
            self.style.shadow_color.with_alpha(self.style.shadow_color.a * alpha),
        );
        layer += 1;

        // 배경
        let bg_geo = PaintGeometry::new(tooltip.position, tooltip.size, 1.0);
        draw_elements.add_border(
            layer,
            bg_geo,
            self.style.background_color.with_alpha(self.style.background_color.a * alpha),
            self.style.border_color.with_alpha(self.style.border_color.a * alpha),
            self.style.border_width,
        );
        layer += 1;

        // 텍스트
        let text_pos = tooltip.position + Vec2::splat(self.style.padding);
        match &tooltip.content {
            TooltipContent::Text(s) | TooltipContent::RichText(s) => {
                let text_size = tooltip.size - Vec2::splat(self.style.padding * 2.0);
                let text_geo = PaintGeometry::new(text_pos, text_size, 1.0);
                draw_elements.add_text(
                    layer,
                    text_geo,
                    s.clone(),
                    self.style.text_color.with_alpha(self.style.text_color.a * alpha),
                    self.style.font_size,
                );
                layer += 1;
            }
            TooltipContent::Custom => {
                // 커스텀 위젯은 별도 처리 필요
            }
        }

        layer
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_delay() {
        let mut manager = TooltipManager::new();
        manager.set_show_delay(0.5);
        manager.set_window_size(Vec2::new(800.0, 600.0));

        let widget_id = WidgetId(1);
        let content = TooltipContent::text("Hello");
        let cursor = Vec2::new(100.0, 100.0);

        // 호버 시작
        manager.on_widget_hover(widget_id, content, cursor, 0.0);
        assert!(manager.pending_tooltip.is_some());
        assert!(!manager.is_showing());

        // 지연 시간 전
        manager.tick(0.3, cursor, 0.3);
        assert!(!manager.is_showing());

        // 지연 시간 후
        manager.tick(0.6, cursor, 0.3);
        assert!(manager.is_showing());
    }

    #[test]
    fn test_tooltip_position_clamping() {
        let mut manager = TooltipManager::new();
        manager.set_window_size(Vec2::new(800.0, 600.0));
        manager.set_show_delay(0.0);

        let widget_id = WidgetId(1);
        let content = TooltipContent::text("Test tooltip");

        // 오른쪽 하단 코너 근처에서 호버
        let cursor = Vec2::new(750.0, 550.0);
        manager.on_widget_hover(widget_id, content, cursor, 0.0);
        manager.tick(0.1, cursor, 0.1);

        // 툴팁이 화면 내에 있어야 함
        if let Some(ref tooltip) = manager.active_tooltip {
            assert!(tooltip.position.x + tooltip.size.x <= 800.0);
            assert!(tooltip.position.y + tooltip.size.y <= 600.0);
        }
    }

    #[test]
    fn test_tooltip_hide_on_leave() {
        let mut manager = TooltipManager::new();
        manager.set_show_delay(0.0);
        manager.set_window_size(Vec2::new(800.0, 600.0));

        let widget_id = WidgetId(1);
        let cursor = Vec2::new(100.0, 100.0);

        manager.on_widget_hover(widget_id, TooltipContent::text("Hi"), cursor, 0.0);
        manager.tick(0.1, cursor, 0.1);
        assert!(manager.is_showing());

        manager.on_widget_leave(widget_id);
        assert!(!manager.is_showing());
    }
}
