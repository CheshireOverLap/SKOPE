//! Inspector Panel - 컴포넌트 속성 편집
//!
//! 선택된 엔티티의 컴포넌트들을 표시하고 편집

use std::any::Any;
use glam::{Vec2, Vec3, Quat};

use crate::core::{Color, CornerRadius, Geometry, Visibility, SlateRect, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use crate::theme::EditorTheme;
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
    /// 에디터 테마
    theme: EditorTheme,
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
            theme: EditorTheme::default(),
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

    /// 컴포넌트 헤더 높이 (테마 기반)
    fn header_height(&self) -> f32 { self.theme.spacing.control_height }
    /// 속성 행 높이 (테마 기반)
    fn property_height(&self) -> f32 { self.theme.spacing.control_height }
    /// 라벨 너비 (테마 기반)
    fn label_width(&self) -> f32 { self.theme.spacing.inspector_label_width }
    /// 엔티티 바 높이 (이름 표시 영역)
    fn entity_bar_height(&self) -> f32 { self.theme.spacing.panel_header_height - self.theme.spacing.gap }

    /// 콘텐츠 영역 시작 Y (패널헤더 + 엔티티바 + gap + add_btn + gap)
    fn content_start_y(&self) -> f32 {
        let ts = &self.theme.spacing;
        ts.panel_header_height + self.entity_bar_height() + ts.gap + ts.control_height + ts.gap
    }

    /// 컴포넌트의 총 높이 계산
    fn component_height(&self, comp: &ComponentInfo) -> f32 {
        self.header_height() + if comp.is_expanded {
            comp.properties.len() as f32 * self.property_height()
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

        let tc = &self.theme.colors;
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let pad = ts.content_padding;
        let gap = ts.gap;
        let panel_h = ts.panel_header_height;
        let row_h = self.header_height();
        let prop_h = self.property_height();
        let label_w = self.label_width();
        let entity_bar_h = self.entity_bar_height();
        let radius_m = CornerRadius::uniform(ts.corner_radius_medium);

        // 배경
        draw_elements.add_box(
            current_layer,
            geometry.to_paint_geometry(),
            tc.panel_bg,
        );
        current_layer += 1;

        // 패널 헤더
        draw_elements.add_box(
            current_layer,
            geometry.paint_at(
                geometry.absolute_position,
                Vec2::new(geometry.local_size.x, panel_h),
            ),
            tc.header_bg,
        );
        draw_elements.add_text(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(pad, (panel_h - tf.large) * 0.5),
                Vec2::new(100.0, tf.large),
            ),
            "Inspector".to_string(),
            tc.text_bright,
            tf.large,
        );
        current_layer += 2;

        // 엔티티 정보 없으면 여기서 종료
        if self.selected_entity.is_none() {
            draw_elements.add_text(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pad, panel_h + tf.large),
                    Vec2::new(geometry.local_size.x - pad * 2.0, tf.large),
                ),
                "No entity selected".to_string(),
                tc.text_muted,
                tf.large,
            );
            return current_layer + 1;
        }

        // 엔티티 이름 바
        let entity_bar_y = panel_h;
        draw_elements.add_box(
            current_layer,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(0.0, entity_bar_y),
                Vec2::new(geometry.local_size.x, entity_bar_h),
            ),
            tc.section_header_bg,
        );
        draw_elements.add_text(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(pad, entity_bar_y + (entity_bar_h - tf.large) * 0.5),
                Vec2::new(geometry.local_size.x - pad * 2.0, tf.large),
            ),
            self.entity_name.clone(),
            tc.text_bright,
            tf.large,
        );
        current_layer += 2;

        // "Add Component" 버튼
        let add_btn_y = entity_bar_y + entity_bar_h + gap;
        let add_btn_w = geometry.local_size.x - pad * 2.0;
        let add_btn_h = ts.control_height;
        draw_elements.add_rounded_box(
            current_layer,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(pad, add_btn_y),
                Vec2::new(add_btn_w, add_btn_h),
            ),
            Color::TRANSPARENT,
            tc.accent,
            ts.border_width,
            radius_m,
        );
        draw_elements.add_text(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(pad, add_btn_y + (add_btn_h - tf.large) * 0.5),
                Vec2::new(add_btn_w, tf.large),
            ),
            "+ Add Component".to_string(),
            tc.accent,
            tf.large,
        );
        current_layer += 2;

        // 컴포넌트들
        let content_start = self.content_start_y();
        let mut y = content_start - self.scroll_offset;
        let text_v_center = |h: f32| (h - tf.large) * 0.5;

        for (comp_idx, comp) in self.components.iter().enumerate() {
            let header_y = y;
            let is_header_hovered = self.hovered_area == Some(HoverArea::ComponentHeader(comp_idx));

            // 헤더 배경
            draw_elements.add_box(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(0.0, header_y),
                    Vec2::new(geometry.local_size.x, row_h),
                ),
                if is_header_hovered { tc.sidebar_button_hover } else { tc.section_header_bg },
            );

            // 확장/축소 chevron
            let expand_icon = if comp.is_expanded { "v" } else { ">" };
            draw_elements.add_text(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pad, header_y + text_v_center(row_h)),
                    Vec2::new(tf.large, tf.large),
                ),
                expand_icon.to_string(),
                tc.text_primary,
                tf.large,
            );

            // 컴포넌트 이름
            let name_x = pad + tf.large + gap;
            draw_elements.add_text(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(name_x, header_y + text_v_center(row_h)),
                    Vec2::new(geometry.local_size.x - name_x - pad, tf.large),
                ),
                comp.name.clone(),
                tc.text_bright,
                tf.large,
            );

            y += row_h;

            // 속성들 (확장된 경우만)
            if comp.is_expanded {
                for (prop_idx, prop) in comp.properties.iter().enumerate() {
                    let prop_y = y;
                    let is_prop_hovered = self.hovered_area == Some(HoverArea::Property(comp_idx, prop_idx));

                    // 속성 배경
                    if is_prop_hovered {
                        draw_elements.add_box(
                            current_layer,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(0.0, prop_y),
                                Vec2::new(geometry.local_size.x, prop_h),
                            ),
                            tc.control_bg_hover,
                        );
                    }

                    // Vec3 컬러 스트립 (축별 색상 표시)
                    if matches!(&prop.value, PropertyValue::Vec3(_)) {
                        let strip_w = ts.vec3_indicator_width;
                        let strip_h = (prop_h - gap) / 3.0;
                        let strip_x = label_w - 2.0;
                        let axis_colors = [tc.vec3_x_color, tc.vec3_y_color, tc.vec3_z_color];
                        for (i, &color) in axis_colors.iter().enumerate() {
                            draw_elements.add_box(
                                current_layer + 1,
                                geometry.paint_at(
                                    geometry.absolute_position + Vec2::new(strip_x, prop_y + gap * 0.5 + strip_h * i as f32),
                                    Vec2::new(strip_w, strip_h),
                                ),
                                color,
                            );
                        }
                    }

                    // 속성 이름 (label)
                    let prop_text_x = pad * 2.0;
                    draw_elements.add_text(
                        current_layer + 1,
                        geometry.paint_at(
                            geometry.absolute_position + Vec2::new(prop_text_x, prop_y + text_v_center(prop_h)),
                            Vec2::new(label_w - prop_text_x - gap, tf.large),
                        ),
                        prop.name.clone(),
                        tc.text_secondary,
                        tf.large,
                    );

                    // 속성 값
                    let value_x_offset = if matches!(&prop.value, PropertyValue::Vec3(_)) {
                        label_w + ts.vec3_indicator_width + gap
                    } else {
                        label_w
                    };

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
                        geometry.paint_at(
                            geometry.absolute_position + Vec2::new(value_x_offset, prop_y + text_v_center(prop_h)),
                            Vec2::new(geometry.local_size.x - value_x_offset - pad, tf.large),
                        ),
                        value_str,
                        if prop.editable { tc.text_primary } else { tc.text_muted },
                        tf.large,
                    );

                    y += prop_h;
                }
            }
        }
        current_layer += 2;

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let content_top = self.content_start_y();
        let content_y = local_pos.y - content_top + self.scroll_offset;
        let row_h = self.header_height();
        let prop_h = self.property_height();

        if content_y < 0.0 {
            self.hovered_area = None;
            return Reply::unhandled();
        }

        // 영역 찾기
        let mut y = 0.0;
        for (comp_idx, comp) in self.components.iter().enumerate() {
            if content_y >= y && content_y < y + row_h {
                self.hovered_area = Some(HoverArea::ComponentHeader(comp_idx));
                return Reply::unhandled();
            }
            y += row_h;

            if comp.is_expanded {
                for prop_idx in 0..comp.properties.len() {
                    if content_y >= y && content_y < y + prop_h {
                        self.hovered_area = Some(HoverArea::Property(comp_idx, prop_idx));
                        return Reply::unhandled();
                    }
                    y += prop_h;
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

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.theme = theme.clone();
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
