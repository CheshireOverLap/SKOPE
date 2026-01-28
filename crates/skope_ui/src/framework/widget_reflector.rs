//! Widget Reflector — 위젯 디버거
//!
//! UE의 Widget Reflector 참고. F9로 토글, 마우스 오버 위젯 정보 오버레이.

use glam::Vec2;
use crate::core::{Color, SlateRect, PaintGeometry, Visibility};
use crate::widget::DrawElementList;

/// 위젯 디버그 정보
#[derive(Debug, Clone, Default)]
pub struct WidgetDebugInfo {
    pub type_name: String,
    pub desired_size: Option<Vec2>,
    pub actual_bounds: Option<SlateRect>,
    pub visibility: Option<Visibility>,
    pub enabled: bool,
    pub child_count: usize,
}

/// 위젯 리플렉터 (디버거)
pub struct WidgetReflector {
    /// 활성 여부
    pub enabled: bool,
    /// 현재 호버 중인 위젯 정보
    pub hovered_info: Option<WidgetDebugInfo>,
    /// 호버 위젯 바운딩 박스
    pub hovered_bounds: Option<SlateRect>,
    /// 마우스 위치
    pub mouse_position: Vec2,
}

impl Default for WidgetReflector {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetReflector {
    pub fn new() -> Self {
        Self {
            enabled: false,
            hovered_info: None,
            hovered_bounds: None,
            mouse_position: Vec2::ZERO,
        }
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.hovered_info = None;
            self.hovered_bounds = None;
        }
        log::info!("[WidgetReflector] {}", if self.enabled { "Enabled (F9)" } else { "Disabled" });
    }

    /// 마우스 위치 업데이트
    pub fn update_mouse(&mut self, pos: Vec2) {
        self.mouse_position = pos;
    }

    /// 호버 위젯 정보 설정 (SlateApp에서 호출)
    pub fn set_hovered(&mut self, info: WidgetDebugInfo, bounds: SlateRect) {
        self.hovered_info = Some(info);
        self.hovered_bounds = Some(bounds);
    }

    /// 호버 초기화
    pub fn clear_hovered(&mut self) {
        self.hovered_info = None;
        self.hovered_bounds = None;
    }

    /// 오버레이 렌더링
    pub fn paint_overlay(
        &self,
        draw_elements: &mut DrawElementList,
        base_layer: u32,
    ) -> u32 {
        if !self.enabled {
            return base_layer;
        }

        let mut layer = base_layer;

        // 바운딩 박스 (빨간 테두리)
        if let Some(ref bounds) = self.hovered_bounds {
            let pos = bounds.top_left();
            let size = bounds.size();
            let geo = PaintGeometry::new(pos, size, 1.0);
            draw_elements.add_border(
                layer,
                geo,
                Color::rgba(1.0, 0.0, 0.0, 0.1),   // 반투명 빨간 배경
                Color::rgba(1.0, 0.2, 0.2, 0.9),    // 빨간 테두리
                2.0,
            );
            layer += 1;
        }

        // 정보 패널
        if let Some(ref info) = self.hovered_info {
            let panel_width = 280.0_f32;
            let panel_height = 80.0_f32;
            let margin = 15.0_f32;

            // 마우스 오른쪽 아래에 표시
            let panel_x = self.mouse_position.x + margin;
            let panel_y = self.mouse_position.y + margin;

            // 배경
            let bg_geo = PaintGeometry::new(
                Vec2::new(panel_x, panel_y),
                Vec2::new(panel_width, panel_height),
                1.0,
            );
            draw_elements.add_border(
                layer,
                bg_geo,
                Color::rgba(0.05, 0.05, 0.08, 0.95),
                Color::rgba(1.0, 0.3, 0.3, 0.8),
                1.0,
            );
            layer += 1;

            let padding = 6.0_f32;
            let line_height = 16.0_f32;
            let text_color = Color::rgba(1.0, 0.9, 0.7, 1.0);

            // 타입 이름
            let type_geo = PaintGeometry::new(
                Vec2::new(panel_x + padding, panel_y + padding),
                Vec2::new(panel_width - padding * 2.0, line_height),
                1.0,
            );
            draw_elements.add_text(layer, type_geo, info.type_name.clone(), text_color, 13.0);
            layer += 1;

            // 바운드
            if let Some(ref bounds) = info.actual_bounds {
                let bounds_text = format!(
                    "Bounds: ({:.0}, {:.0}) {:.0}x{:.0}",
                    bounds.left, bounds.top, bounds.width(), bounds.height()
                );
                let bounds_geo = PaintGeometry::new(
                    Vec2::new(panel_x + padding, panel_y + padding + line_height),
                    Vec2::new(panel_width - padding * 2.0, line_height),
                    1.0,
                );
                draw_elements.add_text(layer, bounds_geo, bounds_text, Color::rgba(0.7, 0.7, 0.7, 1.0), 11.0);
                layer += 1;
            }

            // 자식 수 + 활성 상태
            let state_text = format!(
                "Children: {}  Enabled: {}",
                info.child_count, info.enabled
            );
            let state_geo = PaintGeometry::new(
                Vec2::new(panel_x + padding, panel_y + padding + line_height * 2.0),
                Vec2::new(panel_width - padding * 2.0, line_height),
                1.0,
            );
            draw_elements.add_text(layer, state_geo, state_text, Color::rgba(0.7, 0.7, 0.7, 1.0), 11.0);
            layer += 1;

            // Desired size
            if let Some(ds) = info.desired_size {
                let ds_text = format!("Desired: {:.0}x{:.0}", ds.x, ds.y);
                let ds_geo = PaintGeometry::new(
                    Vec2::new(panel_x + padding, panel_y + padding + line_height * 3.0),
                    Vec2::new(panel_width - padding * 2.0, line_height),
                    1.0,
                );
                draw_elements.add_text(layer, ds_geo, ds_text, Color::rgba(0.7, 0.7, 0.7, 1.0), 11.0);
                layer += 1;
            }
        }

        layer
    }
}
