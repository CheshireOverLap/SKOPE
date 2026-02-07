//! Inspector Panel - 컴포넌트 속성 편집
//!
//! 선택된 엔티티의 컴포넌트들을 표시하고 편집

use std::any::Any;
use glam::{Vec2, Vec3, Quat};

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use crate::widget::{Widget, PaintArgs, DrawElementList};

use super::hierarchy::EntityId;

/// 속성 값 타입
#[derive(Debug, Clone)]
pub enum PropertyValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
    Vec2(Vec2),
    Vec3(Vec3),
    Quat(Quat),
    Color([f32; 4]),
}

/// 속성 정보
#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
    pub editable: bool,
}

/// 컴포넌트 정보
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub properties: Vec<Property>,
    pub is_expanded: bool,
    pub removable: bool,
}

/// Inspector 액션
#[derive(Debug, Clone)]
pub enum InspectorAction {
    None,
    PropertyChanged {
        entity: EntityId,
        component: String,
        property: String,
        value: PropertyValue,
    },
    RemoveComponent {
        entity: EntityId,
        component: String,
    },
    AddComponent {
        entity: EntityId,
        component: String,
    },
    ToggleComponentExpand(String),
}

/// Inspector 패널 위젯
pub struct SInspector {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 선택된 엔티티
    selected_entity: Option<EntityId>,
    /// 엔티티 이름
    entity_name: String,
    /// 컴포넌트 목록
    components: Vec<ComponentInfo>,
    /// 대기 중인 액션
    pending_action: Option<InspectorAction>,
    /// 표시 상태
    visibility: Visibility,
    /// 호버된 영역
    hovered_area: Option<HoverArea>,
    /// 스크롤 오프셋
    scroll_offset: f32,
}

#[derive(Debug, Clone, PartialEq)]
enum HoverArea {
    ComponentHeader(usize),
    Property(usize, usize),
}

impl SInspector {
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            selected_entity: None,
            entity_name: String::new(),
            components: Vec::new(),
            pending_action: None,
            visibility: Visibility::Visible,
            hovered_area: None,
            scroll_offset: 0.0,
        }
    }

    /// 선택된 엔티티 설정
    pub fn set_entity(&mut self, entity: Option<EntityId>, name: String) {
        self.selected_entity = entity;
        self.entity_name = name;
        self.scroll_offset = 0.0;
    }

    /// 컴포넌트 목록 설정
    pub fn set_components(&mut self, components: Vec<ComponentInfo>) {
        self.components = components;
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> InspectorAction {
        self.pending_action.take().unwrap_or(InspectorAction::None)
    }

    /// 컴포넌트 헤더 높이
    const HEADER_HEIGHT: f32 = 24.0;
    /// 속성 행 높이
    const PROPERTY_HEIGHT: f32 = 24.0;
    /// 라벨 너비
    const LABEL_WIDTH: f32 = 100.0;

    /// 컴포넌트의 총 높이 계산
    fn component_height(&self, comp: &ComponentInfo) -> f32 {
        Self::HEADER_HEIGHT + if comp.is_expanded {
            comp.properties.len() as f32 * Self::PROPERTY_HEIGHT
        } else {
            0.0
        }
    }
}

impl Default for SInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SInspector {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(280.0, f32::INFINITY)
    }

    fn type_name(&self) -> &'static str {
        "SInspector"
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            Color::rgba(0.141, 0.141, 0.141, 1.0),  // Panel #242424
        );
        current_layer += 1;

        // 패널 헤더
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position,
                Vec2::new(geometry.local_size.x, 24.0),
                geometry.scale,
            ),
            Color::rgba(0.184, 0.184, 0.184, 1.0),  // Header #2F2F2F
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(8.0, 5.0),
                Vec2::new(100.0, 14.0),
                geometry.scale,
            ),
            "Inspector".to_string(),
            Color::rgba(0.784, 0.784, 0.784, 1.0),  // ForegroundHeader #C8C8C8
            10.0,
        );
        current_layer += 2;

        // 엔티티 정보 없으면 여기서 종료
        if self.selected_entity.is_none() {
            draw_elements.add_text(
                current_layer,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(8.0, 40.0),
                    Vec2::new(geometry.local_size.x - 16.0, 14.0),
                    geometry.scale,
                ),
                "No entity selected".to_string(),
                Color::rgba(0.314, 0.314, 0.314, 1.0),  // text_muted
                10.0,
            );
            return current_layer + 1;
        }

        // 엔티티 이름
        let entity_bar_y = 24.0;
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(0.0, entity_bar_y),
                Vec2::new(geometry.local_size.x, 28.0),
                geometry.scale,
            ),
            Color::rgba(0.184, 0.184, 0.184, 1.0),  // Header #2F2F2F
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(8.0, entity_bar_y + 7.0),
                Vec2::new(geometry.local_size.x - 16.0, 14.0),
                geometry.scale,
            ),
            self.entity_name.clone(),
            Color::rgba(0.784, 0.784, 0.784, 1.0),  // ForegroundHeader #C8C8C8
            12.0,
        );
        current_layer += 2;

        // 컴포넌트들
        let content_start_y = 52.0;
        let mut y = content_start_y - self.scroll_offset;

        for (comp_idx, comp) in self.components.iter().enumerate() {
            // 컴포넌트 헤더
            let header_y = y;
            let is_header_hovered = self.hovered_area == Some(HoverArea::ComponentHeader(comp_idx));

            // 헤더 배경
            draw_elements.add_box(
                current_layer,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(0.0, header_y),
                    Vec2::new(geometry.local_size.x, Self::HEADER_HEIGHT),
                    geometry.scale,
                ),
                if is_header_hovered {
                    Color::rgba(0.220, 0.220, 0.220, 1.0)  // Dropdown #383838
                } else {
                    Color::rgba(0.184, 0.184, 0.184, 1.0)  // Header #2F2F2F
                },
            );

            // 확장 아이콘
            let expand_icon = if comp.is_expanded { "v" } else { ">" };
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(8.0, header_y + 6.0),
                    Vec2::new(12.0, 14.0),
                    geometry.scale,
                ),
                expand_icon.to_string(),
                Color::rgba(0.376, 0.376, 0.376, 1.0),  // Faded #606060
                10.0,
            );

            // 컴포넌트 이름
            draw_elements.add_text(
                current_layer + 1,
                PaintGeometry::new(
                    geometry.absolute_position + Vec2::new(24.0, header_y + 6.0),
                    Vec2::new(geometry.local_size.x - 32.0, 14.0),
                    geometry.scale,
                ),
                comp.name.clone(),
                Color::rgba(0.784, 0.784, 0.784, 1.0),  // ForegroundHeader
                10.0,
            );

            y += Self::HEADER_HEIGHT;

            // 속성들 (확장된 경우만)
            if comp.is_expanded {
                for (prop_idx, prop) in comp.properties.iter().enumerate() {
                    let prop_y = y;
                    let is_prop_hovered = self.hovered_area == Some(HoverArea::Property(comp_idx, prop_idx));

                    // 속성 배경
                    if is_prop_hovered {
                        draw_elements.add_box(
                            current_layer,
                            PaintGeometry::new(
                                geometry.absolute_position + Vec2::new(0.0, prop_y),
                                Vec2::new(geometry.local_size.x, Self::PROPERTY_HEIGHT),
                                geometry.scale,
                            ),
                            Color::rgba(0.102, 0.102, 0.102, 1.0),  // Recessed #1A1A1A
                        );
                    }

                    // 속성 이름
                    draw_elements.add_text(
                        current_layer + 1,
                        PaintGeometry::new(
                            geometry.absolute_position + Vec2::new(16.0, prop_y + 4.0),
                            Vec2::new(Self::LABEL_WIDTH - 20.0, 14.0),
                            geometry.scale,
                        ),
                        prop.name.clone(),
                        Color::rgba(0.376, 0.376, 0.376, 1.0),  // Faded #606060
                        10.0,
                    );

                    // 속성 값
                    let value_str = match &prop.value {
                        PropertyValue::Bool(v) => if *v { "true" } else { "false" }.to_string(),
                        PropertyValue::Int(v) => v.to_string(),
                        PropertyValue::Float(v) => format!("{:.3}", v),
                        PropertyValue::String(v) => v.clone(),
                        PropertyValue::Vec2(v) => format!("({:.2}, {:.2})", v.x, v.y),
                        PropertyValue::Vec3(v) => format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z),
                        PropertyValue::Quat(v) => format!("({:.2}, {:.2}, {:.2}, {:.2})", v.x, v.y, v.z, v.w),
                        PropertyValue::Color(v) => format!("({:.2}, {:.2}, {:.2}, {:.2})", v[0], v[1], v[2], v[3]),
                    };

                    draw_elements.add_text(
                        current_layer + 1,
                        PaintGeometry::new(
                            geometry.absolute_position + Vec2::new(Self::LABEL_WIDTH, prop_y + 4.0),
                            Vec2::new(geometry.local_size.x - Self::LABEL_WIDTH - 8.0, 14.0),
                            geometry.scale,
                        ),
                        value_str,
                        if prop.editable {
                            Color::rgba(0.753, 0.753, 0.753, 1.0)  // Foreground #C0C0C0
                        } else {
                            Color::rgba(0.314, 0.314, 0.314, 1.0)  // text_muted
                        },
                        10.0,
                    );

                    y += Self::PROPERTY_HEIGHT;
                }
            }
        }
        current_layer += 2;

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let content_y = local_pos.y - 52.0 + self.scroll_offset;

        if content_y < 0.0 {
            self.hovered_area = None;
            return Reply::unhandled();
        }

        // 영역 찾기
        let mut y = 0.0;
        for (comp_idx, comp) in self.components.iter().enumerate() {
            // 헤더 영역
            if content_y >= y && content_y < y + Self::HEADER_HEIGHT {
                self.hovered_area = Some(HoverArea::ComponentHeader(comp_idx));
                return Reply::unhandled();
            }
            y += Self::HEADER_HEIGHT;

            // 속성 영역
            if comp.is_expanded {
                for prop_idx in 0..comp.properties.len() {
                    if content_y >= y && content_y < y + Self::PROPERTY_HEIGHT {
                        self.hovered_area = Some(HoverArea::Property(comp_idx, prop_idx));
                        return Reply::unhandled();
                    }
                    y += Self::PROPERTY_HEIGHT;
                }
            }
        }

        self.hovered_area = None;
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_area = None;
    }

    fn on_mouse_button_down(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        match &self.hovered_area {
            Some(HoverArea::ComponentHeader(comp_idx)) => {
                if *comp_idx < self.components.len() {
                    let comp_name = self.components[*comp_idx].name.clone();
                    self.pending_action = Some(InspectorAction::ToggleComponentExpand(comp_name));
                }
                Reply::handled()
            }
            Some(HoverArea::Property(_comp_idx, _prop_idx)) => {
                // TODO: 속성 편집 UI 열기
                Reply::handled()
            }
            None => Reply::unhandled(),
        }
    }

    fn on_mouse_wheel(&mut self, _geometry: &Geometry, event: &PointerEvent) -> Reply {
        let total_height: f32 = self.components.iter()
            .map(|c| self.component_height(c))
            .sum();

        self.scroll_offset = (self.scroll_offset - event.wheel_delta * 30.0)
            .max(0.0)
            .min(total_height.max(0.0));

        Reply::handled()
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
