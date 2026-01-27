//! Widget traits - 위젯 계층 구조 정의 (Slate의 SWidget, SLeafWidget, SCompoundWidget, SPanel)

use glam::Vec2;
use std::any::Any;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, WindowZone};
use crate::event::{Reply, PointerEvent, KeyEvent, CursorIcon};

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
}

/// 그리기 요소 리스트
#[derive(Debug, Default)]
pub struct DrawElementList {
    pub elements: Vec<(u32, DrawElement)>, // (layer, element)
}

impl DrawElementList {
    pub fn new() -> Self {
        Self { elements: Vec::new() }
    }

    pub fn add_box(&mut self, layer: u32, geometry: PaintGeometry, color: Color) {
        self.elements.push((layer, DrawElement::Box { geometry, color }));
    }

    pub fn add_border(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        color: Color,
        border_color: Color,
        border_width: f32,
    ) {
        self.elements.push((layer, DrawElement::Border {
            geometry,
            color,
            border_color,
            border_width,
        }));
    }

    pub fn add_text(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
    ) {
        self.elements.push((layer, DrawElement::Text {
            geometry,
            text,
            color,
            font_size,
        }));
    }

    pub fn add_image(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        path: String,
        tint: Color,
        scaling: ImageScaling,
    ) {
        self.elements.push((layer, DrawElement::Image {
            geometry,
            path,
            tint,
            scaling,
        }));
    }

    /// 삼각형 추가 (화살표 등)
    pub fn add_triangle(
        &mut self,
        layer: u32,
        points: [Vec2; 3],
        color: Color,
    ) {
        self.elements.push((layer, DrawElement::Triangle { points, color }));
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
        // Quad를 2개의 삼각형으로 분할: [0,1,2] + [0,2,3]
        self.elements.push((layer, DrawElement::Triangle {
            points: [points[0], points[1], points[2]],
            color,
        }));
        self.elements.push((layer, DrawElement::Triangle {
            points: [points[0], points[2], points[3]],
            color,
        }));
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

    pub fn clear(&mut self) {
        self.elements.clear();
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

    // ============ 이벤트 핸들러 ============

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
    fn on_focus_received(&mut self) {}
    fn on_focus_lost(&mut self) {}

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
