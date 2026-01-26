//! 도킹 위젯
//!
//! DockTree를 렌더링하고 이벤트를 처리하는 위젯

use std::any::Any;
use glam::Vec2;

use crate::core::{Geometry, Visibility, SlateRect, Color, PaintGeometry};
use crate::event::{Reply, PointerEvent};
use crate::widget::{Widget, PaintArgs, DrawElementList, ArrangedChildren};

use super::{
    NodeId, TabId, NodeRect, DockTree, TabRegistry,
    DragState, DragResult, DockPosition, CompassButton,
};

/// 플로팅 탭 요청
pub struct FloatTabRequest {
    pub tab_id: TabId,
    pub title: String,
    pub position: Vec2,
    pub size: Vec2,
    pub content: Option<Box<dyn Widget>>,
    /// 드래그 중인지 (true면 반투명 윈도우, false면 일반 윈도우)
    pub is_dragging: bool,
}

/// 드래그 종료 알림
pub struct DragEndNotification {
    pub tab_id: TabId,
    /// 도킹 위치 (Some이면 도킹, None이면 플로팅 유지)
    pub dock_target: Option<(NodeId, DockPosition)>,
}

/// 도킹 패널 위젯
pub struct SDockingPanel {
    /// 도킹 트리
    pub tree: DockTree,
    /// 탭 레지스트리
    pub tabs: TabRegistry,
    /// 드래그 상태
    drag_state: DragState,
    /// 표시 상태
    visibility: Visibility,
    /// 활성화 상태
    enabled: bool,
    /// 현재 크기 (레이아웃용)
    #[allow(dead_code)]
    size: Vec2,
    /// 대기 중인 플로팅 탭 요청
    pending_float_requests: Vec<FloatTabRequest>,
    /// 대기 중인 드래그 종료 알림
    pending_drag_end: Vec<DragEndNotification>,
    /// 드래그 중인 탭 콘텐츠 (드롭 시까지 보관)
    pending_drag_content: Option<(TabId, String, Box<dyn Widget>)>,
}

impl SDockingPanel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            tree: DockTree::new(title),
            tabs: TabRegistry::new(),
            drag_state: DragState::new(),
            visibility: Visibility::Visible,
            enabled: true,
            size: Vec2::ZERO,
            pending_float_requests: Vec::new(),
            pending_drag_end: Vec::new(),
            pending_drag_content: None,
        }
    }

    /// 대기 중인 플로팅 요청 가져오기 (큐 비움)
    pub fn drain_float_requests(&mut self) -> Vec<FloatTabRequest> {
        std::mem::take(&mut self.pending_float_requests)
    }

    /// 대기 중인 드래그 종료 알림 가져오기 (큐 비움)
    pub fn drain_drag_end_notifications(&mut self) -> Vec<DragEndNotification> {
        std::mem::take(&mut self.pending_drag_end)
    }

    /// 탭 추가
    pub fn add_tab(&mut self, title: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let tab_id = self.tabs.register_new(title, content);
        self.tree.add_tab(tab_id);
        tab_id
    }

    /// 기존 탭 ID로 탭 재추가 (재도킹용)
    pub fn add_tab_with_id(&mut self, tab_id: TabId, title: impl Into<String>, content: Box<dyn Widget>) {
        self.tabs.register_with_id(tab_id, title, content);
        self.tree.add_tab(tab_id);
    }

    /// 탭을 특정 스택/위치에 재도킹
    pub fn add_tab_with_content(
        &mut self,
        tab_id: TabId,
        title: impl Into<String>,
        content: Box<dyn Widget>,
        target_stack_id: NodeId,
        position: DockPosition,
    ) {
        // 탭 등록
        self.tabs.register_with_id(tab_id, title, content);

        // 도킹
        self.tree.dock_tab(tab_id, target_stack_id, position);
    }

    /// 탭 도킹
    pub fn dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        self.tree.dock_tab(tab_id, target_stack_id, position)
    }

    /// 탭 제거
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        if self.tree.remove_tab(tab_id) {
            self.tabs.remove(tab_id);
            true
        } else {
            false
        }
    }

    /// 드래그 결과 적용
    fn apply_drag_result(&mut self, result: DragResult) {
        match result {
            DragResult::Cancelled => {
                // 드래그 취소 - 탭을 원래 위치로 복구
                if let Some((tab_id, title, content)) = self.pending_drag_content.take() {
                    // 탭 레지스트리에 다시 등록
                    self.tabs.register_with_id(tab_id, title.clone(), content);
                    log::info!("Drag cancelled - restored tab {} '{}'", tab_id.0, title);
                }
            }
            DragResult::DockTab { tab_id, source_stack_id, target_stack_id, position } => {
                // pending_drag_content에서 콘텐츠 가져와서 레지스트리에 복구
                if let Some((drag_tab_id, title, content)) = self.pending_drag_content.take() {
                    if drag_tab_id == tab_id {
                        self.tabs.register_with_id(tab_id, title, content);
                    } else {
                        log::warn!("Tab ID mismatch in DockTab: {} vs {}", drag_tab_id.0, tab_id.0);
                    }
                }

                // 소스와 타겟이 같고 Center 위치면 탭만 복구하고 끝
                if source_stack_id == target_stack_id && position == DockPosition::Center {
                    // 탭을 원래 스택에 다시 추가
                    if let Some(stack) = self.tree.find_tab_stack_mut(source_stack_id) {
                        stack.add_tab(tab_id);
                    }
                    log::debug!("Same stack center drop - restored tab");
                    // return 대신 cleanup 후 종료
                    self.tree.cleanup_empty_stacks();
                    return;
                }

                log::info!("Docking tab {:?} from {:?} to {:?} at {:?}",
                    tab_id, source_stack_id, target_stack_id, position);

                // 타겟에 도킹 (소스에서는 이미 드래그 시작 시 제거됨)
                self.tree.dock_tab(tab_id, target_stack_id, position);
            }
            DragResult::FloatTab { tab_id, source_stack_id: _, position } => {
                // 탭은 이미 드래그 시작 시 스택에서 제거됨
                // pending_drag_content에서 콘텐츠 가져오기
                if let Some((drag_tab_id, title, content)) = self.pending_drag_content.take() {
                    if drag_tab_id == tab_id {
                        // 플로팅 요청 추가
                        self.pending_float_requests.push(FloatTabRequest {
                            tab_id,
                            title,
                            position,
                            size: Vec2::new(400.0, 300.0), // 기본 크기
                            content: Some(content),
                            is_dragging: false, // 드롭으로 인한 플로팅 (드래그 아님)
                        });
                        log::info!("Float tab {} at {:?}", tab_id.0, position);
                    } else {
                        log::warn!("Tab ID mismatch: drag content {} vs result {}", drag_tab_id.0, tab_id.0);
                    }
                } else {
                    log::warn!("No drag content for FloatTab result");
                }
            }
            DragResult::ResizeSplitter { splitter_id, child_index, delta } => {
                // TODO: 스플리터 크기 조절
                log::info!("Resize splitter {} child {} delta {:?}", splitter_id.0, child_index, delta);
            }
        }

        // 드래그 완료 후 빈 스택 정리
        self.tree.cleanup_empty_stacks();
    }

    /// 탭 바 클릭 위치에서 탭 인덱스 찾기
    fn find_tab_at_position(&self, local_x: f32) -> Option<usize> {
        let tab_width = 120.0; // TODO: 동적 계산
        let tab_spacing = self.tree.tab_style.tab_spacing;
        let tab_padding = self.tree.tab_style.tab_padding;

        // 패딩 이전 영역은 무효
        if local_x < tab_padding {
            return None;
        }

        // 탭 인덱스 계산
        let adjusted_x = local_x - tab_padding;
        let tab_total_width = tab_width + tab_spacing;
        let index = (adjusted_x / tab_total_width) as usize;

        // 탭 내부인지 확인 (간격 영역은 제외)
        let pos_in_tab = adjusted_x - (index as f32 * tab_total_width);
        if pos_in_tab <= tab_width {
            Some(index)
        } else {
            None
        }
    }

    /// 레이아웃 업데이트
    #[allow(dead_code)]
    fn update_layout(&mut self, size: Vec2) {
        self.size = size;
        let rect = NodeRect::new(0.0, 0.0, size.x, size.y);
        self.tree.compute_layout(rect);
    }
}

impl Widget for SDockingPanel {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        // 도킹 패널은 가능한 모든 공간을 사용
        Vec2::new(f32::INFINITY, f32::INFINITY)
    }

    fn type_name(&self) -> &'static str {
        "SDockingPanel"
    }

    fn num_children(&self) -> usize {
        // 탭 콘텐츠는 직접 자식으로 취급하지 않음
        0
    }

    fn get_child(&self, _index: usize) -> Option<&dyn Widget> {
        None
    }

    fn get_child_mut(&mut self, _index: usize) -> Option<&mut dyn Widget> {
        None
    }

    fn arrange_children(&self, _geometry: &Geometry, _arranged: &mut ArrangedChildren) {
        // 도킹 패널은 자체적으로 자식 배치
    }

    fn on_paint(
        &self,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            Color::rgba(0.12, 0.12, 0.14, 1.0),
        );
        current_layer += 1;

        // 탭 스택들 렌더링
        self.tree.for_each_tab_stack(|stack| {
            current_layer = self.paint_tab_stack(
                stack,
                args,
                geometry,
                culling_rect,
                draw_elements,
                current_layer,
                is_enabled,
            );
        });

        // ---------------------------------------------------------
        // 나침반 오버레이 렌더링 (Unreal SDockingCross 스타일)
        // ---------------------------------------------------------
        if let Some(compass_data) = self.drag_state.compass.render_data() {
            let target_pos = compass_data.target_rect.position;

            // 1. 도킹 미리보기 영역 (반투명 박스)
            if let Some(preview_rect) = compass_data.preview {
                let preview_geo = PaintGeometry {
                    position: preview_rect.position,
                    size: preview_rect.size,
                    scale: geometry.scale,
                };
                draw_elements.add_box(current_layer, preview_geo, compass_data.preview_color);
                current_layer += 1;
            }

            // 2. 호버된 영역 하이라이트 (사다리꼴)
            if let Some(ref hovered_zone) = compass_data.hovered_zone {
                if hovered_zone.vertices.len() >= 4 {
                    // 글로벌 좌표로 변환
                    let v: Vec<Vec2> = hovered_zone.vertices.iter()
                        .map(|p| *p + target_pos)
                        .collect();

                    if hovered_zone.direction == CompassButton::Center {
                        // 중앙: 사각형
                        let geo = PaintGeometry {
                            position: v[0],
                            size: v[2] - v[0],
                            scale: geometry.scale,
                        };
                        draw_elements.add_box(current_layer, geo, hovered_zone.color);
                    } else {
                        // 방향: 사다리꼴
                        draw_elements.add_quad(current_layer, [v[0], v[1], v[2], v[3]], hovered_zone.color);
                    }
                }
                current_layer += 1;
            }

            // 3. 나침반 선 그리기 (Unreal 스타일: 내부박스 + 외부박스 + 대각선)
            let line_color = compass_data.line_color;
            let inner = compass_data.inner_box;
            let outer = compass_data.outer_box;

            // 내부 박스 (P0 -> P1 -> P2 -> P3 -> P0)
            for i in 0..4 {
                let p1 = inner[i] + target_pos;
                let p2 = inner[(i + 1) % 4] + target_pos;
                draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
            }

            // 외부 박스
            for i in 0..4 {
                let p1 = outer[i] + target_pos;
                let p2 = outer[(i + 1) % 4] + target_pos;
                draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
            }

            // 대각선 (외부 코너 -> 내부 코너)
            for i in 0..4 {
                let p1 = outer[i] + target_pos;
                let p2 = inner[i] + target_pos;
                draw_elements.add_line(current_layer, p1, p2, compass_data.line_width, line_color);
            }

            current_layer += 1;
        }

        // 드래그 중인 탭 프리뷰
        if self.drag_state.is_dragging {
            if let Some(tab_id) = self.drag_state.dragging_tab() {
                if let Some(title) = self.tabs.get_title(tab_id) {
                    let drag_pos = self.drag_state.current_pos;
                    let preview_geo = PaintGeometry {
                        position: drag_pos - Vec2::new(60.0, 14.0),
                        size: Vec2::new(120.0, 28.0),
                        scale: geometry.scale,
                    };
                    draw_elements.add_box(
                        current_layer,
                        preview_geo,
                        Color::rgba(0.2, 0.4, 0.7, 0.9),
                    );
                    draw_elements.add_text(
                        current_layer + 1,
                        PaintGeometry {
                            position: drag_pos - Vec2::new(55.0, 8.0),
                            size: Vec2::new(100.0, 20.0),
                            scale: geometry.scale,
                        },
                        title.to_string(),
                        Color::WHITE,
                        12.0,
                    );
                    current_layer += 2;
                }
            }
        }

        current_layer
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.enabled || !event.is_left_button() {
            return Reply::unhandled();
        }

        let pos = event.screen_position;
        log::debug!("Mouse down at {:?}", pos);

        // 탭 스택에서 탭 바 클릭 확인
        if let Some(stack_id) = self.tree.find_tab_stack_at(pos) {
            log::debug!("Found tab stack: {:?}", stack_id);
            // 먼저 필요한 데이터만 추출 (borrow 해제를 위해)
            let click_info = if let Some(stack) = self.tree.find_tab_stack(stack_id) {
                log::debug!("Tab bar rect: {:?}, pos: {:?}", stack.tab_bar_rect, pos);
                if stack.tab_bar_rect.contains(pos) {
                    let local_x = pos.x - stack.tab_bar_rect.position.x;
                    log::debug!("Click in tab bar, local_x: {}", local_x);
                    Some((stack.tabs.clone(), local_x, stack.content_rect.size))
                } else {
                    log::debug!("Click outside tab bar");
                    None
                }
            } else {
                None
            };

            if let Some((tabs, local_x, content_size)) = click_info {
                if let Some(tab_index) = self.find_tab_at_position(local_x) {
                    log::debug!("Tab index: {}, tabs: {:?}", tab_index, tabs);
                    if let Some(&tab_id) = tabs.get(tab_index) {
                        log::info!("Starting drag for tab {:?}", tab_id);

                        // 드래그 시작 - 내부 상태만 설정 (나침반용)
                        self.drag_state.start_tab_drag(tab_id, stack_id, pos);

                        // 탭 콘텐츠 추출
                        let tab = self.tabs.remove(tab_id);
                        let title = tab.as_ref()
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| format!("Tab {}", tab_id.0));
                        let content = tab.map(|t| t.content);

                        // 원본 스택에서 탭 제거
                        if let Some(stack) = self.tree.find_tab_stack_mut(stack_id) {
                            stack.remove_tab(tab_id);
                        }

                        // 탭 콘텐츠를 드래그 종료까지 보관 (언리얼 스타일)
                        // 드래그 중에는 slate_app이 데코레이터 윈도우를 표시
                        // 드롭 시에만 FloatTabRequest 생성
                        if let Some(widget) = content {
                            self.pending_drag_content = Some((tab_id, title, widget));
                        }

                        return Reply::handled().capture_mouse();
                    }
                } else {
                    log::debug!("No tab at position {}", local_x);
                }
            }
        } else {
            log::debug!("No tab stack at position {:?}", pos);
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        if self.drag_state.is_active() {
            let result = self.drag_state.finish();
            log::info!("Drag finished: {:?}", result);

            // 드래그 결과 적용 (탭 이동, 플로팅 요청 등)
            self.apply_drag_result(result);

            return Reply::handled().release_mouse_capture();
        }

        Reply::unhandled()
    }

    fn on_mouse_move(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !self.drag_state.is_active() {
            return Reply::unhandled();
        }

        let pos = event.screen_position;

        // 위치 업데이트 (is_dragging 플래그 체크)
        self.drag_state.update(pos);

        // 타겟 스택 찾기 및 나침반 표시
        if self.drag_state.is_dragging {
            if let Some(stack_id) = self.tree.find_tab_stack_at(pos) {
                // 스택 위에 있으면 나침반 표시 (소스 스택도 가장자리 도킹 가능)
                if let Some(stack) = self.tree.find_tab_stack(stack_id) {
                    self.drag_state.set_target(Some(stack_id), Some(stack.rect));
                }
            } else {
                self.drag_state.set_target(None, None);
            }

            // 나침반 호버 업데이트 (set_target 이후)
            self.drag_state.update_compass_hover(pos);

            // 디버그: 호버 상태 확인
            if let Some(hover) = self.drag_state.dock_position {
                log::debug!("Compass hover: {:?} at {:?}", hover, pos);
            }
        }

        Reply::handled()
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl SDockingPanel {
    /// 탭 스택 렌더링
    fn paint_tab_stack(
        &self,
        stack: &super::DockTabStack,
        args: &PaintArgs,
        geometry: &Geometry,
        culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 탭 바 배경
        let tab_bar_geo = PaintGeometry {
            position: stack.tab_bar_rect.position,
            size: stack.tab_bar_rect.size,
            scale: geometry.scale,
        };
        draw_elements.add_box(
            current_layer,
            tab_bar_geo,
            Color::rgba(0.18, 0.18, 0.2, 1.0),
        );
        current_layer += 1;

        // 탭 버튼들
        let tab_width = 120.0;
        let tab_spacing = self.tree.tab_style.tab_spacing;
        let mut x = stack.tab_bar_rect.position.x + self.tree.tab_style.tab_padding;

        for (i, &tab_id) in stack.tabs.iter().enumerate() {
            let is_active = i == stack.active_tab;
            let tab_color = if is_active {
                Color::rgba(0.25, 0.25, 0.28, 1.0)
            } else {
                Color::rgba(0.15, 0.15, 0.17, 1.0)
            };

            let tab_geo = PaintGeometry {
                position: Vec2::new(x, stack.tab_bar_rect.position.y + 2.0),
                size: Vec2::new(tab_width, stack.tab_bar_rect.size.y - 2.0),
                scale: geometry.scale,
            };
            draw_elements.add_box(current_layer, tab_geo, tab_color);

            // 탭 제목
            if let Some(title) = self.tabs.get_title(tab_id) {
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry {
                        position: Vec2::new(x + 8.0, stack.tab_bar_rect.position.y + 6.0),
                        size: Vec2::new(tab_width - 16.0, 16.0),
                        scale: geometry.scale,
                    },
                    title.to_string(),
                    if is_active {
                        Color::WHITE
                    } else {
                        Color::rgba(0.7, 0.7, 0.7, 1.0)
                    },
                    12.0,
                );
            }

            x += tab_width + tab_spacing;
        }
        current_layer += 2;

        // 콘텐츠 영역 배경
        let content_geo = PaintGeometry {
            position: stack.content_rect.position,
            size: stack.content_rect.size,
            scale: geometry.scale,
        };
        draw_elements.add_box(
            current_layer,
            content_geo,
            Color::rgba(0.14, 0.14, 0.16, 1.0),
        );
        current_layer += 1;

        // 활성 탭 콘텐츠 렌더링
        if let Some(tab_id) = stack.active_tab_id() {
            if let Some(tab) = self.tabs.get(tab_id) {
                let content_geometry = Geometry {
                    local_size: stack.content_rect.size,
                    position: stack.content_rect.position,
                    absolute_position: stack.content_rect.position,
                    scale: geometry.scale,
                };
                current_layer = tab.content.on_paint(
                    args,
                    &content_geometry,
                    culling_rect,
                    draw_elements,
                    current_layer,
                    is_enabled,
                );
            }
        }

        current_layer
    }
}
