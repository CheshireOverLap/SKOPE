//! Inspector Panel - 컴포넌트 속성 편집
//!
//! 선택된 엔티티의 컴포넌트들을 표시하고 편집

use std::any::Any;
use glam::{Vec2, Vec3, Quat};

use crate::core::{Color, CornerRadius, Geometry, Visibility, SlateRect, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent, KeyEvent, KeyCode, CharEvent};
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
    /// 인라인 편집 상태
    editing: Option<EditingState>,
}

/// 인라인 편집 상태
struct EditingState {
    /// 편집 중인 컴포넌트 인덱스
    comp_idx: usize,
    /// 편집 중인 속성 인덱스
    prop_idx: usize,
    /// 텍스트 편집 버퍼
    buffer: String,
    /// 커서 위치 (바이트 단위)
    cursor: usize,
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
            editing: None,
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

    /// 인라인 편집 시작
    fn start_editing(&mut self, comp_idx: usize, prop_idx: usize) {
        if comp_idx >= self.components.len() { return; }
        let comp = &self.components[comp_idx];
        if prop_idx >= comp.properties.len() { return; }
        let prop = &comp.properties[prop_idx];
        if !prop.editable { return; }

        let buffer = match &prop.value {
            PropertyValue::Bool(v) => if *v { "true" } else { "false" }.to_string(),
            PropertyValue::Int(v) => v.to_string(),
            PropertyValue::Float(v) => format!("{:.3}", v),
            PropertyValue::String(v) => v.clone(),
            PropertyValue::Vec2(v) => format!("{:.3}, {:.3}", v.x, v.y),
            PropertyValue::Vec3(v) => format!("{:.3}, {:.3}, {:.3}", v.x, v.y, v.z),
            PropertyValue::Quat(v) => format!("{:.3}, {:.3}, {:.3}, {:.3}", v.x, v.y, v.z, v.w),
            PropertyValue::Color(v) => format!("{:.3}, {:.3}, {:.3}, {:.3}", v[0], v[1], v[2], v[3]),
        };
        let cursor = buffer.len();

        self.editing = Some(EditingState { comp_idx, prop_idx, buffer, cursor });
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// 편집 커밋 (Enter)
    fn commit_editing(&mut self) {
        let editing = match self.editing.take() {
            Some(e) => e,
            None => return,
        };

        if editing.comp_idx >= self.components.len() { return; }
        let comp = &self.components[editing.comp_idx];
        if editing.prop_idx >= comp.properties.len() { return; }
        let prop = &comp.properties[editing.prop_idx];

        // 버퍼 → PropertyValue 파싱
        let new_value = match &prop.value {
            PropertyValue::Bool(_) => {
                match editing.buffer.trim() {
                    "true" | "1" | "yes" => Some(PropertyValue::Bool(true)),
                    "false" | "0" | "no" => Some(PropertyValue::Bool(false)),
                    _ => None,
                }
            }
            PropertyValue::Int(_) => {
                editing.buffer.trim().parse::<i32>().ok().map(PropertyValue::Int)
            }
            PropertyValue::Float(_) => {
                editing.buffer.trim().parse::<f32>().ok().map(PropertyValue::Float)
            }
            PropertyValue::String(_) => {
                Some(PropertyValue::String(editing.buffer.clone()))
            }
            PropertyValue::Vec2(_) => {
                let parts: Vec<f32> = editing.buffer.split(',')
                    .filter_map(|s| s.trim().parse::<f32>().ok())
                    .collect();
                if parts.len() == 2 {
                    Some(PropertyValue::Vec2(Vec2::new(parts[0], parts[1])))
                } else {
                    None
                }
            }
            PropertyValue::Vec3(_) => {
                let parts: Vec<f32> = editing.buffer.split(',')
                    .filter_map(|s| s.trim().parse::<f32>().ok())
                    .collect();
                if parts.len() == 3 {
                    Some(PropertyValue::Vec3(Vec3::new(parts[0], parts[1], parts[2])))
                } else {
                    None
                }
            }
            PropertyValue::Quat(_) => {
                let parts: Vec<f32> = editing.buffer.split(',')
                    .filter_map(|s| s.trim().parse::<f32>().ok())
                    .collect();
                if parts.len() == 4 {
                    Some(PropertyValue::Quat(Quat::from_xyzw(parts[0], parts[1], parts[2], parts[3])))
                } else {
                    None
                }
            }
            PropertyValue::Color(_) => {
                let parts: Vec<f32> = editing.buffer.split(',')
                    .filter_map(|s| s.trim().parse::<f32>().ok())
                    .collect();
                if parts.len() == 4 {
                    Some(PropertyValue::Color([parts[0], parts[1], parts[2], parts[3]]))
                } else {
                    None
                }
            }
        };

        if let Some(value) = new_value {
            if let Some(entity) = self.selected_entity {
                self.pending_action = Some(InspectorAction::PropertyChanged {
                    entity,
                    component: comp.name.clone(),
                    property: prop.name.clone(),
                    value,
                });
            }
        }

        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// 편집 취소 (Escape)
    fn cancel_editing(&mut self) {
        self.editing = None;
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// 현재 편집 중인 속성인지
    fn is_editing(&self, comp_idx: usize, prop_idx: usize) -> bool {
        self.editing.as_ref()
            .map(|e| e.comp_idx == comp_idx && e.prop_idx == prop_idx)
            .unwrap_or(false)
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

                    let is_editing_this = self.is_editing(comp_idx, prop_idx);
                    let value_width = geometry.local_size.x - value_x_offset - pad;

                    if is_editing_this {
                        // 편집 모드: 입력 필드 배경 + 버퍼 텍스트 + 커서
                        let edit = self.editing.as_ref().unwrap();

                        // 입력 필드 배경 + 보더
                        draw_elements.add_rounded_box(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(value_x_offset - 2.0, prop_y + 1.0),
                                Vec2::new(value_width + 4.0, prop_h - 2.0),
                            ),
                            tc.control_bg,
                            tc.accent,
                            1.0,
                            CornerRadius::uniform(2.0),
                        );

                        // 편집 텍스트
                        draw_elements.add_text(
                            current_layer + 3,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(value_x_offset, prop_y + text_v_center(prop_h)),
                                Vec2::new(value_width, tf.large),
                            ),
                            edit.buffer.clone(),
                            tc.text_bright,
                            tf.large,
                        );

                        // 커서 (단순 수직선)
                        let cursor_x_approx = value_x_offset + edit.cursor as f32 * tf.large * 0.52;
                        draw_elements.add_box(
                            current_layer + 3,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(cursor_x_approx, prop_y + 3.0),
                                Vec2::new(1.0, prop_h - 6.0),
                            ),
                            tc.text_bright,
                        );
                    } else {
                        // 표시 모드: 기존 값 텍스트
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
                                Vec2::new(value_width, tf.large),
                            ),
                            value_str,
                            if prop.editable { tc.text_primary } else { tc.text_muted },
                            tf.large,
                        );
                    }

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
            Some(HoverArea::Property(comp_idx, prop_idx)) => {
                let comp_idx = *comp_idx;
                let prop_idx = *prop_idx;
                self.start_editing(comp_idx, prop_idx);
                Reply::handled().set_focus()
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

    fn supports_keyboard_focus(&self) -> bool {
        self.editing.is_some()
    }

    fn on_key_down(&mut self, _geometry: &Geometry, event: &KeyEvent) -> Reply {
        if self.editing.is_none() {
            return Reply::unhandled();
        }

        match event.key {
            KeyCode::Enter => {
                self.commit_editing();
                Reply::handled()
            }
            KeyCode::Escape => {
                self.cancel_editing();
                Reply::handled()
            }
            KeyCode::Backspace => {
                if let Some(ref mut editing) = self.editing {
                    if editing.cursor > 0 {
                        // 바이트 경계 찾기
                        let prev = editing.buffer[..editing.cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        editing.buffer.drain(prev..editing.cursor);
                        editing.cursor = prev;
                        self.dirty |= InvalidateWidgetReason::PAINT;
                    }
                }
                Reply::handled()
            }
            KeyCode::Delete => {
                if let Some(ref mut editing) = self.editing {
                    if editing.cursor < editing.buffer.len() {
                        let next = editing.buffer[editing.cursor..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| editing.cursor + i)
                            .unwrap_or(editing.buffer.len());
                        editing.buffer.drain(editing.cursor..next);
                        self.dirty |= InvalidateWidgetReason::PAINT;
                    }
                }
                Reply::handled()
            }
            KeyCode::Left => {
                if let Some(ref mut editing) = self.editing {
                    if editing.cursor > 0 {
                        editing.cursor = editing.buffer[..editing.cursor]
                            .char_indices()
                            .last()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                    self.dirty |= InvalidateWidgetReason::PAINT;
                }
                Reply::handled()
            }
            KeyCode::Right => {
                if let Some(ref mut editing) = self.editing {
                    if editing.cursor < editing.buffer.len() {
                        editing.cursor = editing.buffer[editing.cursor..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| editing.cursor + i)
                            .unwrap_or(editing.buffer.len());
                    }
                    self.dirty |= InvalidateWidgetReason::PAINT;
                }
                Reply::handled()
            }
            KeyCode::Home => {
                if let Some(ref mut editing) = self.editing {
                    editing.cursor = 0;
                    self.dirty |= InvalidateWidgetReason::PAINT;
                }
                Reply::handled()
            }
            KeyCode::End => {
                if let Some(ref mut editing) = self.editing {
                    editing.cursor = editing.buffer.len();
                    self.dirty |= InvalidateWidgetReason::PAINT;
                }
                Reply::handled()
            }
            _ => Reply::unhandled(),
        }
    }

    fn on_key_char(&mut self, _geometry: &Geometry, event: &CharEvent) -> Reply {
        if let Some(ref mut editing) = self.editing {
            let ch = event.character;
            // 제어 문자 무시
            if ch.is_control() {
                return Reply::handled();
            }
            editing.buffer.insert(editing.cursor, ch);
            editing.cursor += ch.len_utf8();
            self.dirty |= InvalidateWidgetReason::PAINT;
            Reply::handled()
        } else {
            Reply::unhandled()
        }
    }

    fn on_focus_lost(&mut self) {
        // 포커스 잃으면 편집 커밋
        if self.editing.is_some() {
            self.commit_editing();
        }
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
