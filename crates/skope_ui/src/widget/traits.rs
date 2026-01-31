//! Widget traits - 위젯 계층 구조 정의 (Slate의 SWidget, SLeafWidget, SCompoundWidget, SPanel)

use glam::Vec2;
use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, WindowZone, Margin, InvalidateWidgetReason, SlateBrush, CornerRadius, FontSelector};
use crate::event::{Reply, PointerEvent, KeyEvent, CharEvent, CursorIcon, WidgetDragDropEvent};

// ============================================================================
// Widget ID Generator
// ============================================================================

/// 전역 위젯 ID 카운터 (Atomic — Send+Sync safe)
static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);

/// 고유 위젯 ID 생성 (언리얼 SWidget의 고유 ID에 해당)
///
/// 각 위젯 생성 시 호출하여 유일한 ID를 부여합니다.
/// InvalidationRoot에서 캐시 키로 사용됩니다.
pub fn next_widget_id() -> u64 {
    NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed)
}

/// 배치된 자식 위젯 정보
#[derive(Debug)]
pub struct ArrangedWidget {
    pub geometry: Geometry,
    pub widget_index: usize,
}

/// 배치된 자식들 컬렉션
#[derive(Debug, Default)]
pub struct ArrangedChildren {
    pub children: Vec<ArrangedWidget>,
}

impl ArrangedChildren {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { children: Vec::with_capacity(capacity) }
    }

    pub fn add(&mut self, widget_index: usize, geometry: Geometry) {
        self.children.push(ArrangedWidget { geometry, widget_index });
    }

    pub fn clear(&mut self) {
        self.children.clear();
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

/// 페인팅 인자
#[derive(Debug, Clone)]
pub struct PaintArgs {
    /// 부모 위젯 활성화 상태
    pub parent_enabled: bool,
    /// 현재 시간 (애니메이션용)
    pub current_time: f64,
    /// 델타 시간
    pub delta_time: f32,
}

impl Default for PaintArgs {
    fn default() -> Self {
        Self {
            parent_enabled: true,
            current_time: 0.0,
            delta_time: 0.0,
        }
    }
}

/// 이미지 스케일링 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageScaling {
    /// 원본 크기 유지
    #[default]
    None,
    /// 비율 유지하며 맞춤 (Contain)
    Fit,
    /// 비율 유지하며 채움 (Cover)
    Fill,
    /// 늘려서 채움 (Stretch)
    Stretch,
}

/// 그리기 요소 타입
#[derive(Debug, Clone)]
pub enum DrawElement {
    /// 단색 박스
    Box {
        geometry: PaintGeometry,
        color: Color,
    },
    /// 테두리 박스
    Border {
        geometry: PaintGeometry,
        color: Color,
        border_color: Color,
        border_width: f32,
    },
    /// 텍스트
    Text {
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
        font_family: crate::core::FontFamily,
    },
    /// 이미지
    Image {
        geometry: PaintGeometry,
        path: String,
        tint: Color,
        scaling: ImageScaling,
    },
    /// 삼각형 (화살표용)
    Triangle {
        /// 세 꼭짓점 좌표 (절대 좌표)
        points: [Vec2; 3],
        color: Color,
    },
    /// 라운드 박스 (코너별 라디우스, 아웃라인)
    /// 언리얼 Slate의 FSlateBrush::RoundedBox DrawType에 해당
    RoundedBox {
        geometry: PaintGeometry,
        fill_color: Color,
        outline_color: Color,
        outline_width: f32,
        corner_radius: CornerRadius,
    },
    /// 선형 그래디언트 박스
    Gradient {
        geometry: PaintGeometry,
        start_color: Color,
        end_color: Color,
        /// 그래디언트 각도 (도, 0=좌→우, 90=상→하)
        angle: f32,
    },
    /// 9-Slice 박스 (언리얼 Slate의 FSlateBrush::Box DrawType)
    /// margin이 9분할 영역을 정의
    NineSlice {
        geometry: PaintGeometry,
        texture_path: String,
        tint: Color,
        margin: Margin,
    },
    /// SlateBrush 고수준 렌더링 (렌더러에서 실제 DrawElement로 분해)
    Brush {
        geometry: PaintGeometry,
        brush: SlateBrush,
    },
    /// 스타일 텍스트 (FontSelector 기반 — 가중치/스타일 지원)
    StyledText {
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
        font_selector: FontSelector,
    },
}

/// 그리기 요소 리스트
#[derive(Debug)]
pub struct DrawElementList {
    pub elements: Vec<(u32, DrawElement)>, // (layer, element)
    /// 각 element의 클립 상태 인덱스 (elements와 1:1 대응)
    clip_state_indices: Vec<Option<usize>>,
    /// 계층적 클리핑 매니저
    clipping_manager: crate::core::SlateClippingManager,
    /// 캐시된 정렬 인덱스 (레이어 순)
    sorted_indices: Vec<usize>,
    /// 정렬 캐시 유효 여부
    sort_valid: bool,
}

impl Default for DrawElementList {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            clip_state_indices: Vec::new(),
            clipping_manager: crate::core::SlateClippingManager::new(),
            sorted_indices: Vec::new(),
            sort_valid: false,
        }
    }
}

impl DrawElementList {
    pub fn new() -> Self {
        Self::default()
    }

    /// 요소가 비어있는지
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// 클리핑 존 push (SlateClippingManager 위임)
    ///
    /// 계층적 클리핑 합성 (축 정렬: Scissor 교차, 비축 정렬: Stencil 누적).
    pub fn push_clip(&mut self, zone: crate::core::SlateClippingZone) {
        self.clipping_manager.push_clip(zone);
    }

    /// 축 정렬 rect [x, y, w, h]로 클리핑 push (하위 호환)
    ///
    /// 기존 `push_clip([f32;4])` 대체. SScrollBox 등에서 사용.
    pub fn push_clip_rect(&mut self, rect: [f32; 4]) {
        let zone = crate::core::SlateClippingZone::from_rect(rect);
        self.clipping_manager.push_clip(zone);
    }

    /// 클리핑 pop
    pub fn pop_clip(&mut self) {
        self.clipping_manager.pop_clip();
    }

    /// 현재 클립 상태 인덱스 (None = 클리핑 없음)
    fn current_clip_index(&self) -> Option<usize> {
        self.clipping_manager.current_clip_index()
    }

    /// 클리핑 매니저 참조 (렌더러에서 상태 조회용)
    pub fn clipping_manager(&self) -> &crate::core::SlateClippingManager {
        &self.clipping_manager
    }

    pub fn add_box(&mut self, layer: u32, geometry: PaintGeometry, color: Color) {
        let clip_idx = self.current_clip_index();
        self.elements.push((layer, DrawElement::Box { geometry, color }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    pub fn add_border(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        color: Color,
        border_color: Color,
        border_width: f32,
    ) {
        let clip_idx = self.current_clip_index();
        self.elements.push((layer, DrawElement::Border {
            geometry,
            color,
            border_color,
            border_width,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    pub fn add_text(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
    ) {
        let clip_idx = self.current_clip_index();
        let scaled_font_size = font_size * geometry.scale;
        self.elements.push((layer, DrawElement::Text {
            geometry,
            text,
            color,
            font_size: scaled_font_size,
            font_family: crate::core::FontFamily::UI,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    pub fn add_text_with_font(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
        font_family: crate::core::FontFamily,
    ) {
        let clip_idx = self.current_clip_index();
        let scaled_font_size = font_size * geometry.scale;
        self.elements.push((layer, DrawElement::Text {
            geometry,
            text,
            color,
            font_size: scaled_font_size,
            font_family,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    /// 스타일 텍스트 추가 (FontSelector 기반 — 가중치/스타일 지원)
    pub fn add_styled_text(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
        font_selector: FontSelector,
    ) {
        let clip_idx = self.current_clip_index();
        let scaled_font_size = font_size * geometry.scale;
        self.elements.push((layer, DrawElement::StyledText {
            geometry,
            text,
            color,
            font_size: scaled_font_size,
            font_selector,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    pub fn add_image(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        path: String,
        tint: Color,
        scaling: ImageScaling,
    ) {
        let clip_idx = self.current_clip_index();
        self.elements.push((layer, DrawElement::Image {
            geometry,
            path,
            tint,
            scaling,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    /// 삼각형 추가 (화살표 등)
    pub fn add_triangle(
        &mut self,
        layer: u32,
        points: [Vec2; 3],
        color: Color,
    ) {
        let clip_idx = self.current_clip_index();
        self.elements.push((layer, DrawElement::Triangle { points, color }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    /// 사각형(Quad) 추가 - 4개 꼭짓점을 2개 삼각형으로 그림
    /// 꼭짓점 순서: 시계 방향 또는 반시계 방향으로 연속된 4점
    /// [0] -> [1] -> [2] -> [3] -> [0]
    pub fn add_quad(
        &mut self,
        layer: u32,
        points: [Vec2; 4],
        color: Color,
    ) {
        let clip_idx = self.current_clip_index();
        // Quad를 2개의 삼각형으로 분할: [0,1,2] + [0,2,3]
        self.elements.push((layer, DrawElement::Triangle {
            points: [points[0], points[1], points[2]],
            color,
        }));
        self.clip_state_indices.push(clip_idx);
        self.elements.push((layer, DrawElement::Triangle {
            points: [points[0], points[2], points[3]],
            color,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    /// 선 추가 (두께 있는 직선)
    /// 두 점 사이를 연결하는 직사각형으로 렌더링
    pub fn add_line(
        &mut self,
        layer: u32,
        p1: Vec2,
        p2: Vec2,
        thickness: f32,
        color: Color,
    ) {
        // 선 방향 계산
        let dir = p2 - p1;
        let len = dir.length();
        if len < 0.001 {
            return;
        }

        // 법선 벡터 (두께를 위해)
        let normal = Vec2::new(-dir.y, dir.x).normalize() * (thickness * 0.5);

        // 4개 꼭짓점 (직사각형)
        let v0 = p1 + normal;
        let v1 = p1 - normal;
        let v2 = p2 - normal;
        let v3 = p2 + normal;

        self.add_quad(layer, [v0, v1, v2, v3], color);
    }

    /// 레이어 순서로 정렬된 요소들 반환
    pub fn sorted(&self) -> Vec<&DrawElement> {
        let mut sorted: Vec<_> = self.elements.iter().collect();
        sorted.sort_by_key(|(layer, _)| *layer);
        sorted.into_iter().map(|(_, elem)| elem).collect()
    }

    /// 레이어 순서로 정렬된 (요소, 클립 상태 인덱스) 반환
    pub fn sorted_with_clips(&self) -> Vec<(&DrawElement, Option<usize>)> {
        let mut indices: Vec<usize> = (0..self.elements.len()).collect();
        indices.sort_by_key(|&i| self.elements[i].0);
        indices.into_iter().map(|i| {
            let clip_idx = self.clip_state_indices.get(i).copied().flatten();
            (&self.elements[i].1, clip_idx)
        }).collect()
    }

    pub fn clear(&mut self) {
        self.elements.clear();
        self.clip_state_indices.clear();
        self.clipping_manager.reset();
        self.sorted_indices.clear();
        self.sort_valid = false;
    }

    /// 정렬 보장 (변경 시에만 실행, 캐시 재사용)
    pub fn ensure_sorted(&mut self) {
        if !self.sort_valid {
            self.sorted_indices.clear();
            self.sorted_indices.extend(0..self.elements.len());
            self.sorted_indices.sort_by_key(|&i| self.elements[i].0);
            self.sort_valid = true;
        }
    }

    /// 정렬된 순서로 (element, clip state index) 이터레이터 (ensure_sorted 호출 후 사용)
    pub fn sorted_iter(&self) -> impl Iterator<Item = (&DrawElement, Option<usize>)> {
        self.sorted_indices.iter().map(move |&i| {
            let clip_idx = self.clip_state_indices.get(i).copied().flatten();
            (&self.elements[i].1, clip_idx)
        })
    }

    /// 모든 요소의 알파에 opacity를 곱함 (데코레이터 윈도우 반투명 렌더링용)
    pub fn apply_opacity(&mut self, opacity: f32) {
        for (_, element) in &mut self.elements {
            match element {
                DrawElement::Box { color, .. } => color.a *= opacity,
                DrawElement::Border { color, border_color, .. } => {
                    color.a *= opacity;
                    border_color.a *= opacity;
                }
                DrawElement::Text { color, .. } => color.a *= opacity,
                DrawElement::Image { tint, .. } => tint.a *= opacity,
                DrawElement::Triangle { color, .. } => color.a *= opacity,
                DrawElement::RoundedBox { fill_color, outline_color, .. } => {
                    fill_color.a *= opacity;
                    outline_color.a *= opacity;
                }
                DrawElement::Gradient { start_color, end_color, .. } => {
                    start_color.a *= opacity;
                    end_color.a *= opacity;
                }
                DrawElement::NineSlice { tint, .. } => tint.a *= opacity,
                DrawElement::Brush { brush, .. } => {
                    // Brush의 tint/color에 opacity 적용
                    brush.apply_opacity(opacity);
                }
                DrawElement::StyledText { color, .. } => color.a *= opacity,
            }
        }
    }

    /// SlateBrush를 그리기 요소로 추가
    pub fn add_brush(&mut self, layer: u32, geometry: PaintGeometry, brush: &SlateBrush) {
        match brush {
            SlateBrush::None => {}
            SlateBrush::Color(color) => {
                self.add_box(layer, geometry, *color);
            }
            SlateBrush::RoundedBox { fill_color, outline_color, outline_width, corner_radius } => {
                self.add_rounded_box(
                    layer, geometry,
                    *fill_color, *outline_color, *outline_width, *corner_radius,
                );
            }
            SlateBrush::Gradient { start_color, end_color, angle } => {
                self.add_gradient(layer, geometry, *start_color, *end_color, *angle);
            }
            SlateBrush::Outline { color, width, corner_radius } => {
                self.add_rounded_box(
                    layer, geometry,
                    Color::TRANSPARENT, *color, *width, CornerRadius::uniform(*corner_radius),
                );
            }
            _ => {
                // Image 등 복합 브러시: Brush DrawElement로 전달
                let clip_idx = self.current_clip_index();
                self.elements.push((layer, DrawElement::Brush {
                    geometry,
                    brush: brush.clone(),
                }));
                self.clip_state_indices.push(clip_idx);
                self.sort_valid = false;
            }
        }
    }

    /// 라운드 박스 추가
    pub fn add_rounded_box(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        fill_color: Color,
        outline_color: Color,
        outline_width: f32,
        corner_radius: CornerRadius,
    ) {
        let clip_idx = self.current_clip_index();
        self.elements.push((layer, DrawElement::RoundedBox {
            geometry,
            fill_color,
            outline_color,
            outline_width,
            corner_radius,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }

    /// 그래디언트 박스 추가
    pub fn add_gradient(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        start_color: Color,
        end_color: Color,
        angle: f32,
    ) {
        let clip_idx = self.current_clip_index();
        self.elements.push((layer, DrawElement::Gradient {
            geometry,
            start_color,
            end_color,
            angle,
        }));
        self.clip_state_indices.push(clip_idx);
        self.sort_valid = false;
    }
}

// ============================================================================
// Framework-level Render Effects Helper
// ============================================================================

/// 자식 위젯의 렌더 트랜스폼과 불투명도를 Geometry에 적용하는 헬퍼 함수.
///
/// 언리얼 `FGeometry::MakeArrangedWidget`에서 자식 위젯의
/// `RenderTransform`, `RenderTransformPivot`, `RenderOpacity`를
/// 자동 적용하는 로직에 해당합니다.
///
/// 부모 위젯의 `on_paint()`에서 자식을 페인팅하기 전에 호출:
/// ```rust,ignore
/// let child_geo = apply_widget_render_effects(child, &arranged_geometry);
/// child.on_paint(args, &child_geo, culling_rect, draw_elements, layer, is_enabled);
/// ```
pub fn apply_widget_render_effects(widget: &dyn Widget, geometry: &Geometry) -> Geometry {
    let mut geo = *geometry;

    // 렌더 트랜스폼 적용 (None이면 스킵)
    if let Some(rt) = widget.render_transform() {
        let pivot = widget.render_transform_pivot();
        geo = geo.with_render_transform(&rt, pivot);
    }

    // 불투명도 적용 (1.0이면 스킵)
    let opacity = widget.render_opacity();
    if opacity < 1.0 {
        geo = geo.with_render_opacity(opacity);
    }

    geo
}

/// 자식 위젯 페인트 시 자동 클리핑 적용 헬퍼.
///
/// 자식의 `widget_clipping()` 모드에 따라 자동으로 `push_clip` / `pop_clip`을 수행.
/// 부모 컨테이너 위젯의 `on_paint`에서 자식을 페인팅할 때 사용:
/// ```rust,ignore
/// let child_geo = apply_widget_render_effects(child, &arranged_geo);
/// paint_child_with_clipping(child, args, &child_geo, culling_rect, draw_elements, layer, is_enabled);
/// ```
pub fn paint_child_with_clipping(
    child: &dyn Widget,
    args: &PaintArgs,
    child_geometry: &Geometry,
    culling_rect: &SlateRect,
    draw_elements: &mut DrawElementList,
    layer: u32,
    is_enabled: bool,
) -> u32 {
    use crate::core::{EWidgetClipping, SlateClippingZone};

    let clipping_mode = child.widget_clipping();
    let needs_clip = match clipping_mode {
        EWidgetClipping::Inherit => false,
        EWidgetClipping::ClipToBounds
        | EWidgetClipping::ClipToBoundsAlways
        | EWidgetClipping::ClipToBoundsWithoutIntersecting => true,
        EWidgetClipping::OnDemand => {
            // DesiredSize > allocated size 체크
            let desired = child.compute_desired_size(child_geometry.scale);
            desired.x > child_geometry.local_size.x + 0.1
                || desired.y > child_geometry.local_size.y + 0.1
        }
    };

    if needs_clip {
        let zone = SlateClippingZone::from_geometry(child_geometry, clipping_mode);
        draw_elements.push_clip(zone);
        let result_layer = child.on_paint(args, child_geometry, culling_rect, draw_elements, layer, is_enabled);
        draw_elements.pop_clip();
        result_layer
    } else {
        child.on_paint(args, child_geometry, culling_rect, draw_elements, layer, is_enabled)
    }
}

/// 모든 위젯의 기본 트레이트 (Slate의 SWidget)
pub trait Widget: Any + Send + Sync {
    // ============ 필수 구현 ============

    /// 원하는 크기 계산
    fn compute_desired_size(&self, layout_scale: f32) -> Vec2;

    /// 타입 이름 (디버깅용)
    fn type_name(&self) -> &'static str;

    // ============ 자식 관리 (기본: 없음) ============

    /// 자식 수
    fn num_children(&self) -> usize { 0 }

    /// 자식 접근 (읽기)
    fn get_child(&self, _index: usize) -> Option<&dyn Widget> { None }

    /// 자식 접근 (쓰기)
    fn get_child_mut(&mut self, _index: usize) -> Option<&mut dyn Widget> { None }

    // ============ 레이아웃 ============

    /// 자식 배치
    fn arrange_children(&self, _geometry: &Geometry, _arranged: &mut ArrangedChildren) {}

    // ============ 렌더링 ============

    /// 위젯 그리기
    fn on_paint(
        &self,
        _args: &PaintArgs,
        _geometry: &Geometry,
        _culling_rect: &SlateRect,
        _draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        layer
    }

    // ============ 렌더 트랜스폼 / 불투명도 ============

    /// 위젯 렌더 불투명도 (0.0 = 투명, 1.0 = 불투명)
    ///
    /// 부모의 불투명도와 곱셈되어 누적됩니다.
    /// 언리얼 SWidget::RenderOpacity에 해당.
    fn render_opacity(&self) -> f32 { 1.0 }

    /// 위젯 로컬 렌더 트랜스폼 (None = 변환 없음)
    ///
    /// 레이아웃에 영향 없이 렌더링과 히트테스트에만 적용.
    /// 언리얼 SWidget::RenderTransform에 해당.
    fn render_transform(&self) -> Option<crate::core::SlateRenderTransform> { None }

    /// 렌더 트랜스폼 피봇 (정규화 좌표, 0.5 = 중심)
    ///
    /// 언리얼 SWidget::RenderTransformPivot에 해당.
    fn render_transform_pivot(&self) -> Vec2 { Vec2::new(0.5, 0.5) }

    // ============ 클리핑 ============

    /// 위젯 클리핑 모드 (UE5 SWidget::Clipping)
    ///
    /// `EWidgetClipping::Inherit` (기본): 부모 클립 상속.
    /// `ClipToBounds`: 이 위젯 바운드로 추가 클리핑.
    /// `ClipToBoundsAlways`: 하위 위젯이 무시 불가.
    /// `OnDemand`: DesiredSize > AllocatedSize일 때만 클리핑.
    fn widget_clipping(&self) -> crate::core::EWidgetClipping {
        crate::core::EWidgetClipping::Inherit
    }

    // ============ Tick ============

    /// 프레임당 업데이트 (UE의 SWidget::Tick)
    fn tick(&mut self, _delta_time: f32) {}
    /// tick 호출이 필요한 위젯이면 true 반환
    fn can_tick(&self) -> bool { false }

    // ============ Active Timer (UE의 RegisterActiveTimer) ============

    /// 활성 타이머가 있는지 (prepass에서 tick 호출 여부 결정)
    fn has_active_timers(&self) -> bool { false }

    /// 활성 타이머 실행 (prepass에서 호출)
    ///
    /// `current_time`: 앱 시작 이후 경과 시간 (초)
    /// `delta_time`: 이전 프레임과의 시간 차이 (초)
    fn tick_active_timers(&mut self, _current_time: f64, _delta_time: f32) {}

    // ============ Invalidation (언리얼 EInvalidateWidgetReason 패턴) ============

    /// 위젯 고유 ID (InvalidationRoot 캐시 키)
    ///
    /// 기본값 0 = 캐싱 미지원 (하위호환).
    /// 새 위젯은 `next_widget_id()`로 생성 시 고유 ID 부여.
    fn widget_id(&self) -> u64 { 0 }

    /// 현재 dirty 플래그 반환
    ///
    /// 기본값: NONE (깨끗한 상태).
    /// 위젯은 실제 dirty 필드를 추적하여 PAINT/LAYOUT 등 설정.
    fn dirty_flags(&self) -> InvalidateWidgetReason {
        InvalidateWidgetReason::NONE
    }

    /// 다시 그려야 하는지 (dirty_flags 기반)
    fn needs_repaint(&self) -> bool {
        self.dirty_flags().contains(InvalidateWidgetReason::PAINT)
    }

    /// 레이아웃 재계산이 필요한지
    fn needs_layout(&self) -> bool {
        self.dirty_flags().contains(InvalidateWidgetReason::LAYOUT)
    }

    /// 위젯 무효화 (이유별 dirty 마킹)
    ///
    /// 언리얼 SWidget::Invalidate(EInvalidateWidgetReason)에 해당
    fn invalidate(&mut self, _reason: InvalidateWidgetReason) {}

    /// 변경 발생 시 dirty 마킹 (하위호환 — invalidate(PAINT) 호출)
    fn mark_dirty(&mut self) {
        self.invalidate(InvalidateWidgetReason::PAINT);
    }

    /// paint 완료 후 dirty 해제
    fn clear_dirty(&mut self) {}

    /// 항상 매 프레임 repaint 필요한 위젯 (애니메이션 등)
    fn is_volatile(&self) -> bool { false }

    /// Prepass: SlateAttribute 바인딩 업데이트
    ///
    /// 언리얼의 Prepass 단계에서 TSlateAttribute::UpdateNow()에 해당.
    /// 반환값: 업데이트로 인해 발생한 무효화 이유.
    fn update_attributes(&mut self) -> InvalidateWidgetReason {
        InvalidateWidgetReason::NONE
    }

    // ============ 이벤트 핸들러 ============

    // --- Tunnel (Preview) 단계: 부모 → 자식 순서 ---

    /// 키 다운 Preview (부모가 자식보다 먼저 처리)
    fn on_preview_key_down(&mut self, _geometry: &Geometry, _event: &KeyEvent) -> Reply {
        Reply::unhandled()
    }
    /// 마우스 버튼 다운 Preview
    fn on_preview_mouse_button_down(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    // --- Bubble 단계: 자식 → 부모 순서 (기존) ---

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {}
    fn on_mouse_leave(&mut self, _event: &PointerEvent) {}
    fn on_mouse_move(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }
    fn on_mouse_button_down(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }
    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }
    /// 더블 클릭 이벤트 (언리얼 OnMouseButtonDoubleClick)
    fn on_mouse_button_double_click(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }
    fn on_mouse_wheel(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }
    fn on_key_down(&mut self, _geometry: &Geometry, _event: &KeyEvent) -> Reply {
        Reply::unhandled()
    }
    fn on_key_up(&mut self, _geometry: &Geometry, _event: &KeyEvent) -> Reply {
        Reply::unhandled()
    }
    /// 문자 입력 이벤트 (UE의 OnKeyChar에 해당)
    ///
    /// OS 입력 처리 후 실제 타이핑된 문자를 수신합니다.
    /// KeyDown/KeyUp과 별도로, 텍스트 삽입에 사용합니다.
    fn on_key_char(&mut self, _geometry: &Geometry, _event: &CharEvent) -> Reply {
        Reply::unhandled()
    }
    fn on_focus_received(&mut self) {}
    fn on_focus_lost(&mut self) {}

    // ============ 드래그 앤 드롭 (UE5 SWidget D&D 콜백) ============

    /// 드래그가 감지되었을 때 호출 (임계값 초과 시)
    ///
    /// UE5 SWidget::OnDragDetected에 해당.
    /// `Reply::handled().begin_drag_drop(op)`으로 드래그 오퍼레이션 시작.
    fn on_drag_detected(
        &mut self,
        _geometry: &Geometry,
        _event: &PointerEvent,
    ) -> Reply {
        Reply::unhandled()
    }

    /// 드래그 오퍼레이션이 이 위젯 위로 진입
    ///
    /// UE5 SWidget::OnDragEnter에 해당.
    fn on_drag_enter(
        &mut self,
        _geometry: &Geometry,
        _event: &WidgetDragDropEvent,
    ) {}

    /// 드래그 오퍼레이션이 이 위젯을 벗어남
    ///
    /// UE5 SWidget::OnDragLeave에 해당.
    fn on_drag_leave(&mut self, _event: &WidgetDragDropEvent) {}

    /// 드래그 오퍼레이션이 이 위젯 위에서 이동 중
    ///
    /// UE5 SWidget::OnDragOver에 해당.
    /// 드롭 수락 여부에 따라 커서 변경 등 가능.
    fn on_drag_over(
        &mut self,
        _geometry: &Geometry,
        _event: &WidgetDragDropEvent,
    ) -> Reply {
        Reply::unhandled()
    }

    /// 드래그 오퍼레이션이 이 위젯에 드롭됨
    ///
    /// UE5 SWidget::OnDrop에 해당.
    /// `Reply::handled()` 반환 시 드롭 수락.
    fn on_drop(
        &mut self,
        _geometry: &Geometry,
        _event: &WidgetDragDropEvent,
    ) -> Reply {
        Reply::unhandled()
    }

    /// 마우스 캡처가 해제됨 (드래그 취소 등)
    ///
    /// UE5 SWidget::OnMouseCaptureLost에 해당.
    fn on_mouse_capture_lost(&mut self) {}

    // ============ IME (Input Method) ============

    /// IME preedit (조합 중) — 한글 등 조합 문자열 표시
    fn on_ime_preedit(&mut self, _text: &str, _cursor: Option<(usize, usize)>) {}
    /// IME commit (확정) — 조합 완료 문자열 삽입
    fn on_ime_commit(&mut self, _text: &str) {}

    // ============ 접근성 (Accessibility) ============

    /// 접근성 역할
    fn accessibility_role(&self) -> crate::framework::AccessibilityRole {
        crate::framework::AccessibilityRole::None
    }
    /// 접근성 이름 (스크린 리더가 읽는 텍스트)
    fn accessible_name(&self) -> String { self.type_name().to_string() }
    /// 접근성 설명
    fn accessible_description(&self) -> Option<String> { None }
    /// 접근성 상태
    fn accessibility_state(&self) -> crate::framework::AccessibilityState {
        crate::framework::AccessibilityState {
            enabled: self.is_enabled(),
            ..Default::default()
        }
    }

    // ============ 속성 ============

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn set_visibility(&mut self, _visibility: Visibility) {}
    fn is_enabled(&self) -> bool { true }
    fn set_enabled(&mut self, _enabled: bool) {}
    fn get_cursor(&self) -> Option<CursorIcon> { None }
    fn get_tool_tip(&self) -> Option<&str> { None }

    /// 윈도우 존 오버라이드 (언리얼 GetWindowZoneOverride)
    ///
    /// 이 위젯 전체 영역의 기본 존. 위젯 내 위치별 다른 존이 필요하면
    /// `get_window_zone_at()` 오버라이드.
    /// - `WindowZone::TitleBar` 반환 시 이 위젯 영역이 드래그로 창 이동
    /// - `WindowZone::Unspecified` (기본값)면 부모가 결정
    fn get_window_zone_override(&self) -> WindowZone { WindowZone::Unspecified }

    /// 주어진 위치의 윈도우 존 (언리얼 GetCurrentWindowZone 스타일)
    ///
    /// 기본 구현: 자식 위젯 순회 → 자신의 zone_override 반환
    /// 복잡한 위젯(SDockingPanel 등)은 오버라이드하여 영역별 다른 존 반환
    ///
    /// - `local_pos`: 이 위젯 로컬 좌표
    /// - `geometry`: 이 위젯의 geometry
    fn get_window_zone_at(&self, local_pos: Vec2, geometry: &Geometry) -> WindowZone {
        // 1. 자식 위젯 순회 (역순 - 위에 그려진 것 우선)
        let num = self.num_children();
        if num > 0 {
            let mut arranged = ArrangedChildren::with_capacity(num);
            self.arrange_children(geometry, &mut arranged);

            for arranged_child in arranged.children.iter().rev() {
                if let Some(child) = self.get_child(arranged_child.widget_index) {
                    let child_geo = &arranged_child.geometry;
                    // 자식 영역 내인지 확인
                    let child_local = local_pos - child_geo.position;
                    if child_local.x >= 0.0 && child_local.x <= child_geo.local_size.x
                        && child_local.y >= 0.0 && child_local.y <= child_geo.local_size.y
                    {
                        let zone = child.get_window_zone_at(child_local, child_geo);
                        if zone != WindowZone::Unspecified {
                            return zone;
                        }
                    }
                }
            }
        }

        // 2. 자식에서 Zone 없으면 자신의 override 반환
        self.get_window_zone_override()
    }

    // ============ 부모 추적 (UE SWidget::ParentWidgetPtr) ============

    /// 부모 위젯 ID 반환 (None = 루트 또는 미설정)
    fn parent_id(&self) -> Option<u64> { None }

    /// 부모 위젯 ID 설정 (자식 추가 시 호출)
    fn set_parent_id(&mut self, _parent_id: Option<u64>) {}

    // ============ 다운캐스팅 ============

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// 자식이 없는 리프 위젯 (Slate의 SLeafWidget)
///
/// 텍스트, 이미지 등 직접 렌더링하는 위젯
pub trait LeafWidget: Widget {
    // 기본 Widget 구현 사용
    // num_children() = 0 고정
}

/// 단일 자식을 가진 위젯 (Slate의 SCompoundWidget)
///
/// Border, Button 등 자식을 감싸는 위젯
pub trait CompoundWidget: Widget {
    /// 자식 위젯 참조
    fn get_content(&self) -> Option<&dyn Widget>;

    /// 자식 위젯 가변 참조
    fn get_content_mut(&mut self) -> Option<&mut dyn Widget>;

    /// 자식 설정
    fn set_content(&mut self, content: Option<Box<dyn Widget>>);
}

/// 여러 자식을 가진 패널 위젯 (Slate의 SPanel)
///
/// HBox, VBox 등 레이아웃 컨테이너
pub trait PanelWidget: Widget {
    /// 모든 자식 참조
    fn children(&self) -> &[Box<dyn Widget>];

    /// 모든 자식 가변 참조
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>>;

    /// 자식 추가
    fn add_child(&mut self, child: Box<dyn Widget>);

    /// 자식 제거
    fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>>;

    /// 모든 자식 제거
    fn clear_children(&mut self);
}

/// WidgetId - 위젯 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

impl WidgetId {
    /// 새 고유 ID 생성
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for WidgetId {
    fn default() -> Self {
        Self::new()
    }
}
