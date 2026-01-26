//! 도킹 나침반 (Docking Compass) - Unreal Engine 스타일
//!
//! SDockingCross 완벽 재현
//! - 정규화 좌표 기반 영역 판정
//! - 기울기(slope) 기반 방향 결정
//! - 동적 영역 크기 (30%, min 5, max 150)

use super::{DockPosition, NodeRect};
use crate::core::Color;
use glam::Vec2;

/// Unreal 도킹 상수
pub mod constants {
    /// 도킹 영역 비율 (30%)
    pub const ZONE_FRACTION: f32 = 0.3;
    /// 최대 영역 크기
    pub const MAX_ZONE_SIZE: f32 = 150.0;
    /// 최소 영역 크기
    pub const MIN_ZONE_SIZE: f32 = 5.0;
}

/// 나침반 스타일
#[derive(Debug, Clone)]
pub struct CompassStyle {
    /// 선 색상
    pub line_color: Color,
    /// 호버 영역 색상
    pub hover_color: Color,
    /// 미리보기 색상
    pub preview_color: Color,
    /// 선 두께
    pub line_width: f32,
}

impl Default for CompassStyle {
    fn default() -> Self {
        Self {
            line_color: Color::rgba(0.8, 0.8, 0.8, 0.8),
            hover_color: Color::rgba(0.9, 0.5, 0.1, 0.6),
            preview_color: Color::rgba(0.9, 0.5, 0.1, 0.25),
            line_width: 2.0,
        }
    }
}

/// 나침반 버튼 (도킹 방향)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompassButton {
    Center,
    Left,   // LeftOf
    Right,  // RightOf
    Top,    // Above
    Bottom, // Below
}

impl CompassButton {
    /// 도킹 위치로 변환
    pub fn to_dock_position(&self) -> DockPosition {
        match self {
            Self::Center => DockPosition::Center,
            Self::Left => DockPosition::Left,
            Self::Right => DockPosition::Right,
            Self::Top => DockPosition::Top,
            Self::Bottom => DockPosition::Bottom,
        }
    }

    /// 모든 버튼
    pub fn all() -> [CompassButton; 5] {
        [
            CompassButton::Center,
            CompassButton::Left,
            CompassButton::Right,
            CompassButton::Top,
            CompassButton::Bottom,
        ]
    }
}

/// 도킹 나침반 (Unreal SDockingCross 스타일)
pub struct DockingCompass {
    /// 스타일
    pub style: CompassStyle,
    /// 타겟 영역 (전체 도킹 가능 영역)
    target_rect: NodeRect,
    /// 현재 호버된 버튼
    hovered_button: Option<CompassButton>,
    /// 표시 여부
    visible: bool,
}

impl Default for DockingCompass {
    fn default() -> Self {
        Self::new()
    }
}

impl DockingCompass {
    pub fn new() -> Self {
        Self {
            style: CompassStyle::default(),
            target_rect: NodeRect::default(),
            hovered_button: None,
            visible: false,
        }
    }

    /// 나침반 표시
    pub fn show(&mut self, target_rect: NodeRect) {
        self.target_rect = target_rect;
        self.visible = true;
    }

    /// 나침반 숨기기
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_button = None;
    }

    /// 표시 중인지
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 도킹 영역 크기 계산 (Unreal 스타일)
    /// DockZoneSize = clamp(geometry_size * ZONE_FRACTION, MIN_ZONE_SIZE, MAX_ZONE_SIZE)
    fn dock_zone_size(&self) -> Vec2 {
        use constants::*;
        Vec2::new(
            (self.target_rect.size.x * ZONE_FRACTION).clamp(MIN_ZONE_SIZE, MAX_ZONE_SIZE),
            (self.target_rect.size.y * ZONE_FRACTION).clamp(MIN_ZONE_SIZE, MAX_ZONE_SIZE),
        )
    }

    /// 내부 박스 꼭지점 계산 (P0, P1, P2, P3)
    /// ```text
    ///   P0---------P1
    ///   |           |
    ///   |  (center) |
    ///   |           |
    ///   P3---------P2
    /// ```
    fn inner_box_points(&self) -> [Vec2; 4] {
        let zone = self.dock_zone_size();
        let size = self.target_rect.size;

        [
            Vec2::new(zone.x, zone.y),                      // P0: 좌상
            Vec2::new(size.x - zone.x, zone.y),             // P1: 우상
            Vec2::new(size.x - zone.x, size.y - zone.y),    // P2: 우하
            Vec2::new(zone.x, size.y - zone.y),             // P3: 좌하
        ]
    }

    /// 외부 박스 꼭지점 (전체 영역)
    fn outer_box_points(&self) -> [Vec2; 4] {
        let size = self.target_rect.size;
        [
            Vec2::new(0.0, 0.0),           // 좌상
            Vec2::new(size.x, 0.0),        // 우상
            Vec2::new(size.x, size.y),     // 우하
            Vec2::new(0.0, size.y),        // 좌하
        ]
    }

    /// 마우스 위치로 드롭 타겟 계산 (Unreal GetDropTarget 로직)
    pub fn update_hover(&mut self, mouse_pos: Vec2) -> Option<CompassButton> {
        if !self.visible {
            self.hovered_button = None;
            return None;
        }

        // 로컬 좌표로 변환
        let local_pos = mouse_pos - self.target_rect.position;
        let size = self.target_rect.size;

        // 영역 밖이면 None
        if local_pos.x < 0.0 || local_pos.x > size.x || local_pos.y < 0.0 || local_pos.y > size.y {
            self.hovered_button = None;
            return None;
        }

        // 정규화 (0~1)
        let normalized = Vec2::new(
            local_pos.x / size.x,
            local_pos.y / size.y,
        );

        // 기울기 계산 (0으로 나누기 방지)
        let mouse_slope = if normalized.x.abs() < 0.0001 {
            if normalized.y >= 0.0 { f32::INFINITY } else { f32::NEG_INFINITY }
        } else {
            normalized.y / normalized.x
        };

        // slope=1 선을 따른 거리
        // Unreal: FVector2D::DotProduct(NormalizedMousePos, FVector2D::UnitVector)
        // UnitVector = (1, 1) (정규화되지 않음!)
        // 즉, distance = x + y
        let distance_along_slope_one = normalized.x + normalized.y;

        // 도킹 영역 안에 있는지 확인
        let zone = self.dock_zone_size();
        let is_in_dock_zone =
            local_pos.x < zone.x ||
            local_pos.x > (size.x - zone.x) ||
            local_pos.y < zone.y ||
            local_pos.y > (size.y - zone.y);

        if !is_in_dock_zone {
            // 중앙 영역 - Center로 처리
            self.hovered_button = Some(CompassButton::Center);
            return self.hovered_button;
        }

        // Unreal 로직: slope와 distance로 방향 결정
        //
        //   (0,0)         (1,0)
        //       +--------+
        //       |\  T   /|     slope < 1, dist < 1 = Top
        //       | \    / |     slope < 1, dist > 1 = Right
        //       |L \  / R|     slope > 1, dist < 1 = Left
        //       |   \/   |     slope > 1, dist > 1 = Bottom
        //       |   /\   |
        //       |  /  \  |
        //       | /  B \ |
        //       +--------+
        //   (0,1)         (1,1)
        //
        // 대각선 slope=1: y = x
        // 대각선 slope=-1 (반대): y = 1 - x (즉, x + y = 1)

        self.hovered_button = Some(if mouse_slope > 1.0 {
            // 왼쪽 아래 삼각형 영역
            if distance_along_slope_one > 1.0 {
                CompassButton::Bottom  // Below
            } else {
                CompassButton::Left    // LeftOf
            }
        } else {
            // 오른쪽 위 삼각형 영역
            if distance_along_slope_one > 1.0 {
                CompassButton::Right   // RightOf
            } else {
                CompassButton::Top     // Above
            }
        });

        self.hovered_button
    }

    /// 현재 호버된 버튼
    pub fn hovered_button(&self) -> Option<CompassButton> {
        self.hovered_button
    }

    /// 호버된 위치의 도킹 미리보기 영역
    pub fn preview_rect(&self) -> Option<NodeRect> {
        self.hovered_button.map(|button| {
            match button {
                CompassButton::Center => self.target_rect,
                CompassButton::Left => self.target_rect.left_half(),
                CompassButton::Right => self.target_rect.right_half(),
                CompassButton::Top => self.target_rect.top_half(),
                CompassButton::Bottom => self.target_rect.bottom_half(),
            }
        })
    }

    /// 타겟 영역
    pub fn target_rect(&self) -> NodeRect {
        self.target_rect
    }

    /// 버튼 색상 가져오기
    pub fn button_color(&self, button: CompassButton) -> Color {
        if self.hovered_button == Some(button) {
            self.style.hover_color
        } else {
            Color::TRANSPARENT
        }
    }
}

/// 나침반 렌더링 데이터 (Unreal 스타일)
#[derive(Debug, Clone)]
pub struct CompassRenderData {
    /// 타겟 영역 (로컬 좌표 기준)
    pub target_rect: NodeRect,
    /// 내부 박스 꼭지점 (P0, P1, P2, P3) - 로컬 좌표
    pub inner_box: [Vec2; 4],
    /// 외부 박스 꼭지점 - 로컬 좌표
    pub outer_box: [Vec2; 4],
    /// 선 색상
    pub line_color: Color,
    /// 선 두께
    pub line_width: f32,
    /// 호버된 영역 (있으면)
    pub hovered_zone: Option<HoveredZone>,
    /// 미리보기 영역 (있으면)
    pub preview: Option<NodeRect>,
    /// 미리보기 색상
    pub preview_color: Color,
}

/// 호버된 영역 정보
#[derive(Debug, Clone)]
pub struct HoveredZone {
    /// 방향
    pub direction: CompassButton,
    /// 영역 (사다리꼴/사각형)
    pub vertices: Vec<Vec2>,
    /// 색상
    pub color: Color,
}

impl DockingCompass {
    /// 렌더링 데이터 생성 (Unreal 스타일)
    pub fn render_data(&self) -> Option<CompassRenderData> {
        if !self.visible {
            return None;
        }

        let inner_box = self.inner_box_points();
        let outer_box = self.outer_box_points();

        // 호버된 영역 계산
        let hovered_zone = self.hovered_button.map(|button| {
            let vertices = match button {
                CompassButton::Center => {
                    // 중앙: 내부 박스
                    vec![inner_box[0], inner_box[1], inner_box[2], inner_box[3]]
                }
                CompassButton::Top => {
                    // 위: 사다리꼴 (외부 좌상 -> 외부 우상 -> 내부 우상 -> 내부 좌상)
                    vec![outer_box[0], outer_box[1], inner_box[1], inner_box[0]]
                }
                CompassButton::Bottom => {
                    // 아래: 사다리꼴
                    vec![inner_box[3], inner_box[2], outer_box[2], outer_box[3]]
                }
                CompassButton::Left => {
                    // 왼쪽: 사다리꼴
                    vec![outer_box[0], inner_box[0], inner_box[3], outer_box[3]]
                }
                CompassButton::Right => {
                    // 오른쪽: 사다리꼴
                    vec![inner_box[1], outer_box[1], outer_box[2], inner_box[2]]
                }
            };

            HoveredZone {
                direction: button,
                vertices,
                color: self.style.hover_color,
            }
        });

        Some(CompassRenderData {
            target_rect: self.target_rect,
            inner_box,
            outer_box,
            line_color: self.style.line_color,
            line_width: self.style.line_width,
            hovered_zone,
            preview: self.preview_rect(),
            preview_color: self.style.preview_color,
        })
    }
}

// === 이전 API 호환성 유지 ===

/// 버튼 렌더링 데이터 (이전 호환)
#[derive(Debug, Clone)]
pub struct CompassButtonRenderData {
    pub rect: NodeRect,
    pub color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub button_type: CompassButton,
    pub is_hovered: bool,
}
