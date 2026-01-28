//! SProgressBar - 프로그레스 바 위젯 (언리얼 Slate의 SProgressBar)
//!
//! 로딩, 진행 상황 등을 표시하는 위젯입니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{Color, Geometry, PaintGeometry, SlateRect, Visibility};
use crate::event::{PointerEvent, Reply};

use super::{DrawElementList, PaintArgs, Widget};

// ============================================================================
// ProgressBarFillType
// ============================================================================

/// 프로그레스 바 채우기 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressBarFillType {
    /// 왼쪽에서 오른쪽으로
    #[default]
    LeftToRight,
    /// 오른쪽에서 왼쪽으로
    RightToLeft,
    /// 중앙에서 양쪽으로 (수평)
    FillFromCenterHorizontal,
    /// 위에서 아래로
    TopToBottom,
    /// 아래에서 위로
    BottomToTop,
    /// 중앙에서 양쪽으로 (수직)
    FillFromCenterVertical,
}

// ============================================================================
// ProgressBarStyle
// ============================================================================

/// 프로그레스 바 스타일
#[derive(Debug, Clone)]
pub struct ProgressBarStyle {
    /// 배경 색상
    pub background_color: Color,
    /// 채우기 색상
    pub fill_color: Color,
    /// 테두리 색상
    pub border_color: Color,
    /// 테두리 두께
    pub border_width: f32,
    /// 코너 반경
    pub corner_radius: f32,
    /// 마키 (불확정 상태) 색상
    pub marquee_color: Color,
    /// 마키 너비
    pub marquee_width: f32,
}

impl Default for ProgressBarStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.15, 0.15, 0.17, 1.0),
            fill_color: Color::rgba(0.3, 0.6, 0.9, 1.0),
            border_color: Color::rgba(0.3, 0.3, 0.35, 1.0),
            border_width: 1.0,
            corner_radius: 2.0,
            marquee_color: Color::rgba(0.4, 0.7, 1.0, 0.8),
            marquee_width: 60.0,
        }
    }
}

// ============================================================================
// SProgressBar
// ============================================================================

/// 프로그레스 바 위젯
///
/// 언리얼 Slate의 `SProgressBar`에 해당합니다.
/// 0.0 ~ 1.0 사이의 값을 시각적으로 표시합니다.
pub struct SProgressBar {
    /// 진행률 (0.0 ~ 1.0, None이면 불확정 상태)
    percent: Option<f32>,
    /// 채우기 방향
    fill_type: ProgressBarFillType,
    /// 스타일
    style: ProgressBarStyle,
    /// 가시성
    visibility: Visibility,
    /// 원하는 높이
    desired_height: f32,
    /// 마키 오프셋 (애니메이션용)
    marquee_offset: f32,
    /// 마키 속도
    marquee_speed: f32,
}

impl Default for SProgressBar {
    fn default() -> Self {
        Self {
            percent: Some(0.0),
            fill_type: ProgressBarFillType::LeftToRight,
            style: ProgressBarStyle::default(),
            visibility: Visibility::Visible,
            desired_height: 16.0,
            marquee_offset: 0.0,
            marquee_speed: 100.0,
        }
    }
}

impl SProgressBar {
    /// 빌더 시작
    pub fn new() -> SProgressBarBuilder {
        SProgressBarBuilder::default()
    }

    /// 진행률 설정
    pub fn set_percent(&mut self, percent: Option<f32>) {
        self.percent = percent.map(|p| p.clamp(0.0, 1.0));
    }

    /// 진행률 가져오기
    pub fn percent(&self) -> Option<f32> {
        self.percent
    }

    /// 채우기 방향 설정
    pub fn set_fill_type(&mut self, fill_type: ProgressBarFillType) {
        self.fill_type = fill_type;
    }

    /// 채우기 색상 설정
    pub fn set_fill_color(&mut self, color: Color) {
        self.style.fill_color = color;
    }

    /// 틱 (마키 애니메이션용)
    pub fn tick(&mut self, delta_time: f32, width: f32) {
        if self.percent.is_none() {
            // 마키 모드
            self.marquee_offset += self.marquee_speed * delta_time;
            let total_width = width + self.style.marquee_width;
            if self.marquee_offset > total_width {
                self.marquee_offset = -self.style.marquee_width;
            }
        }
    }
}

// ============================================================================
// SProgressBarBuilder
// ============================================================================

/// SProgressBar 빌더
#[derive(Default)]
pub struct SProgressBarBuilder {
    inner: SProgressBar,
}

impl SProgressBarBuilder {
    /// 진행률 설정 (None이면 불확정/마키 모드)
    pub fn percent(mut self, percent: Option<f32>) -> Self {
        self.inner.percent = percent.map(|p| p.clamp(0.0, 1.0));
        self
    }

    /// 채우기 방향 설정
    pub fn fill_type(mut self, fill_type: ProgressBarFillType) -> Self {
        self.inner.fill_type = fill_type;
        self
    }

    /// 스타일 설정
    pub fn style(mut self, style: ProgressBarStyle) -> Self {
        self.inner.style = style;
        self
    }

    /// 채우기 색상 설정
    pub fn fill_color(mut self, color: Color) -> Self {
        self.inner.style.fill_color = color;
        self
    }

    /// 배경 색상 설정
    pub fn background_color(mut self, color: Color) -> Self {
        self.inner.style.background_color = color;
        self
    }

    /// 높이 설정
    pub fn height(mut self, height: f32) -> Self {
        self.inner.desired_height = height;
        self
    }

    /// 빌드 완료
    pub fn build(self) -> SProgressBar {
        self.inner
    }
}

// ============================================================================
// Widget Implementation
// ============================================================================

impl Widget for SProgressBar {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(100.0, self.desired_height)
    }

    fn type_name(&self) -> &'static str {
        "SProgressBar"
    }

    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::ProgressBar
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;
        let size = geometry.local_size;
        let pos = geometry.absolute_position;

        // 배경
        let bg_geo = PaintGeometry::new(pos, size, geometry.scale);
        draw_elements.add_border(
            current_layer,
            bg_geo,
            self.style.background_color,
            self.style.border_color,
            self.style.border_width,
        );
        current_layer += 1;

        // 채우기 영역 계산
        let inner_pos = pos + Vec2::splat(self.style.border_width);
        let inner_size = size - Vec2::splat(self.style.border_width * 2.0);

        if let Some(percent) = self.percent {
            // 확정 상태: 진행률 표시
            let (fill_pos, fill_size) = match self.fill_type {
                ProgressBarFillType::LeftToRight => {
                    let width = inner_size.x * percent;
                    (inner_pos, Vec2::new(width, inner_size.y))
                }
                ProgressBarFillType::RightToLeft => {
                    let width = inner_size.x * percent;
                    let x = inner_pos.x + inner_size.x - width;
                    (Vec2::new(x, inner_pos.y), Vec2::new(width, inner_size.y))
                }
                ProgressBarFillType::FillFromCenterHorizontal => {
                    let width = inner_size.x * percent;
                    let x = inner_pos.x + (inner_size.x - width) * 0.5;
                    (Vec2::new(x, inner_pos.y), Vec2::new(width, inner_size.y))
                }
                ProgressBarFillType::TopToBottom => {
                    let height = inner_size.y * percent;
                    (inner_pos, Vec2::new(inner_size.x, height))
                }
                ProgressBarFillType::BottomToTop => {
                    let height = inner_size.y * percent;
                    let y = inner_pos.y + inner_size.y - height;
                    (Vec2::new(inner_pos.x, y), Vec2::new(inner_size.x, height))
                }
                ProgressBarFillType::FillFromCenterVertical => {
                    let height = inner_size.y * percent;
                    let y = inner_pos.y + (inner_size.y - height) * 0.5;
                    (Vec2::new(inner_pos.x, y), Vec2::new(inner_size.x, height))
                }
            };

            if fill_size.x > 0.0 && fill_size.y > 0.0 {
                let fill_geo = PaintGeometry::new(fill_pos, fill_size, geometry.scale);
                draw_elements.add_box(current_layer, fill_geo, self.style.fill_color);
            }
        } else {
            // 불확정 상태: 마키 애니메이션
            let marquee_x = inner_pos.x + self.marquee_offset;
            let marquee_width = self.style.marquee_width.min(inner_size.x);

            // 클리핑을 위해 보이는 부분만 계산
            let visible_start = marquee_x.max(inner_pos.x);
            let visible_end = (marquee_x + marquee_width).min(inner_pos.x + inner_size.x);

            if visible_end > visible_start {
                let marquee_geo = PaintGeometry::new(
                    Vec2::new(visible_start, inner_pos.y),
                    Vec2::new(visible_end - visible_start, inner_size.y),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer, marquee_geo, self.style.marquee_color);
            }
        }

        current_layer + 1
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_creation() {
        let bar = SProgressBar::new()
            .percent(Some(0.5))
            .fill_color(Color::rgba(0.0, 1.0, 0.0, 1.0))
            .build();

        assert_eq!(bar.percent(), Some(0.5));
    }

    #[test]
    fn test_progress_bar_clamping() {
        let mut bar = SProgressBar::default();
        bar.set_percent(Some(1.5));
        assert_eq!(bar.percent(), Some(1.0));

        bar.set_percent(Some(-0.5));
        assert_eq!(bar.percent(), Some(0.0));
    }

    #[test]
    fn test_indeterminate_mode() {
        let bar = SProgressBar::new()
            .percent(None)
            .build();

        assert!(bar.percent().is_none());
    }
}
