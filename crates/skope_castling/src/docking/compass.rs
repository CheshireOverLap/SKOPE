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
    /// 프리뷰 모핑 속도 (UE SetHoveredTarget MorphToShape, ~0.1초 수렴)
    pub const MORPH_SPEED: f32 = 12.0;
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
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl CompassStyle {
    /// 테마에서 색상 초기화
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        Self {
            line_color: theme.colors.compass_line,
            hover_color: theme.colors.compass_hover,
            preview_color: theme.colors.compass_preview,
            line_width: 2.0,
        }
    }
}

/// 나침반 버튼 (도킹 방향) - UE5 SDockingCross 스타일 (4방향만)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompassButton {
    Left,   // LeftOf
    Right,  // RightOf
    Top,    // Above
    Bottom, // Below
}

impl CompassButton {
    /// 도킹 위치로 변환
    pub fn to_dock_position(&self) -> DockPosition {
        match self {
            Self::Left => DockPosition::Left,
            Self::Right => DockPosition::Right,
            Self::Top => DockPosition::Top,
            Self::Bottom => DockPosition::Bottom,
        }
    }

    /// 모든 버튼 (4방향만, UE5 SDockingCross 스타일)
    pub fn all() -> [CompassButton; 4] {
        [
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
    /// 타겟 영역 (전체 스택 영역, 탭바 포함 - 프리뷰용)
    target_rect: NodeRect,
    /// 콘텐츠 영역 (탭바 제외 - 나침반 영역 계산용)
    content_rect: NodeRect,
    /// 현재 호버된 버튼
    hovered_button: Option<CompassButton>,
    /// 표시 여부
    visible: bool,
    // === MorphToShape 애니메이션 (UE SetHoveredTarget 스타일) ===
    /// 현재 보간 중인 프리뷰 rect
    current_preview: NodeRect,
    /// 목표 프리뷰 rect
    target_preview: Option<NodeRect>,
    /// 모핑 진행도 (0→1)
    morph_progress: f32,
    /// 이전 호버 버튼 (변경 감지용)
    prev_button: Option<CompassButton>,
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
            content_rect: NodeRect::default(),
            hovered_button: None,
            visible: false,
            current_preview: NodeRect::default(),
            target_preview: None,
            morph_progress: 1.0,
            prev_button: None,
        }
    }

    /// 나침반 표시
    /// - target_rect: 전체 스택 영역 (탭바 포함, 프리뷰용)
    /// - content_rect: 콘텐츠 영역 (탭바 제외, 나침반 영역 계산용)
    pub fn show(&mut self, target_rect: NodeRect) {
        self.target_rect = target_rect;
        self.content_rect = target_rect; // 기본값: 동일
        self.visible = true;
    }

    /// 나침반 표시 (콘텐츠 영역 분리 - UE5 스타일)
    /// - target_rect: 전체 스택 영역 (탭바 포함, 프리뷰용)
    /// - content_rect: 콘텐츠 영역 (탭바 제외, 나침반 영역 계산용)
    pub fn show_with_content(&mut self, target_rect: NodeRect, content_rect: NodeRect) {
        self.target_rect = target_rect;
        self.content_rect = content_rect;
        self.visible = true;
    }

    /// 나침반 숨기기
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_button = None;
        self.prev_button = None;
        self.current_preview = NodeRect::default();
        self.target_preview = None;
        self.morph_progress = 1.0;
    }

    /// 표시 중인지
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 도킹 영역 크기 계산 (Unreal 스타일) - 콘텐츠 영역 기준
    /// DockZoneSize = clamp(geometry_size * ZONE_FRACTION, MIN_ZONE_SIZE, MAX_ZONE_SIZE)
    fn dock_zone_size(&self) -> Vec2 {
        use constants::*;
        Vec2::new(
            (self.content_rect.size.x * ZONE_FRACTION).clamp(MIN_ZONE_SIZE, MAX_ZONE_SIZE),
            (self.content_rect.size.y * ZONE_FRACTION).clamp(MIN_ZONE_SIZE, MAX_ZONE_SIZE),
        )
    }

    /// 내부 박스 꼭지점 계산 (P0, P1, P2, P3) - 콘텐츠 영역 기준
    /// ```text
    ///   P0---------P1
    ///   |           |
    ///   |  (center) |
    ///   |           |
    ///   P3---------P2
    /// ```
    fn inner_box_points(&self) -> [Vec2; 4] {
        let zone = self.dock_zone_size();
        let size = self.content_rect.size;
        // 콘텐츠 영역 오프셋 (탭바 높이)
        let offset = self.content_rect.position - self.target_rect.position;

        [
            Vec2::new(offset.x + zone.x, offset.y + zone.y),                      // P0: 좌상
            Vec2::new(offset.x + size.x - zone.x, offset.y + zone.y),             // P1: 우상
            Vec2::new(offset.x + size.x - zone.x, offset.y + size.y - zone.y),    // P2: 우하
            Vec2::new(offset.x + zone.x, offset.y + size.y - zone.y),             // P3: 좌하
        ]
    }

    /// 외부 박스 꼭지점 (콘텐츠 영역 - 나침반이 탭바를 가리지 않도록)
    fn outer_box_points(&self) -> [Vec2; 4] {
        let size = self.content_rect.size;
        // 콘텐츠 영역 오프셋 (탭바 높이)
        let offset = self.content_rect.position - self.target_rect.position;
        [
            Vec2::new(offset.x, offset.y),                     // 좌상
            Vec2::new(offset.x + size.x, offset.y),            // 우상
            Vec2::new(offset.x + size.x, offset.y + size.y),   // 우하
            Vec2::new(offset.x, offset.y + size.y),            // 좌하
        ]
    }

    /// 마우스 위치로 드롭 타겟 계산 (Unreal GetDropTarget 로직)
    /// 콘텐츠 영역 기준으로 판정 (탭바 제외)
    pub fn update_hover(&mut self, mouse_pos: Vec2) -> Option<CompassButton> {
        if !self.visible {
            self.hovered_button = None;
            self.check_morph_trigger();
            return None;
        }

        // 콘텐츠 영역 기준 로컬 좌표로 변환
        let local_pos = mouse_pos - self.content_rect.position;
        let size = self.content_rect.size;

        // 콘텐츠 영역 밖이면 None (탭바 영역에서는 나침반 호버 안 됨)
        if local_pos.x < 0.0 || local_pos.x > size.x || local_pos.y < 0.0 || local_pos.y > size.y {
            self.hovered_button = None;
            self.check_morph_trigger();
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
            // 중앙 영역 - None 반환 (UE5: 나침반은 4방향만, 탭웰로 병합)
            self.hovered_button = None;
            self.check_morph_trigger();
            return None;
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

        self.check_morph_trigger();
        self.hovered_button
    }

    // === MorphToShape 애니메이션 (UE SetHoveredTarget 스타일) ===

    /// 프리뷰 모핑 애니메이션 틱 (QuadOut 이징, ~0.1초)
    pub fn tick(&mut self, dt: f32) {
        if let Some(target) = self.target_preview {
            if self.morph_progress >= 1.0 {
                return;
            }
            self.morph_progress = (self.morph_progress + constants::MORPH_SPEED * dt).min(1.0);
            // QuadOut: t' = 1 - (1 - t)^2
            let t = self.morph_progress;
            let eased = 1.0 - (1.0 - t) * (1.0 - t);
            self.current_preview = self.current_preview.lerp(&target, eased);
            if self.morph_progress >= 1.0 {
                self.current_preview = target;
            }
        }
    }

    /// 호버 버튼 변경 감지 → 모핑 시작
    fn check_morph_trigger(&mut self) {
        if self.hovered_button != self.prev_button {
            self.prev_button = self.hovered_button;
            if let Some(new_preview) = self.preview_rect() {
                self.start_morph_to(new_preview);
            } else {
                self.target_preview = None;
            }
        }
    }

    /// 새 목표 rect로 모핑 시작
    fn start_morph_to(&mut self, new_target: NodeRect) {
        // 첫 등장 시 snap (보간 시작점이 없으므로)
        if self.current_preview.is_zero() {
            self.current_preview = new_target;
            self.target_preview = Some(new_target);
            self.morph_progress = 1.0;
            return;
        }
        self.target_preview = Some(new_target);
        self.morph_progress = 0.0;
    }

    /// 현재 애니메이션 적용된 프리뷰 rect (렌더링용)
    pub fn animated_preview_rect(&self) -> Option<NodeRect> {
        if self.target_preview.is_some() && !self.current_preview.is_zero() {
            Some(self.current_preview)
        } else {
            None
        }
    }

    /// 현재 호버된 버튼
    pub fn hovered_button(&self) -> Option<CompassButton> {
        self.hovered_button
    }

    /// 호버 상태 초기화
    pub fn clear_hover(&mut self) {
        self.hovered_button = None;
    }

    /// 호버된 위치의 도킹 미리보기 영역 (4방향만)
    pub fn preview_rect(&self) -> Option<NodeRect> {
        self.hovered_button.map(|button| {
            match button {
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

        // 호버된 영역 계산 (4방향만, UE5 SDockingCross 스타일)
        let hovered_zone = self.hovered_button.map(|button| {
            let vertices = match button {
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
            preview: self.animated_preview_rect(),
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
