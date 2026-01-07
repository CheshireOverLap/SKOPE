//! SKOPE UI - Input System
//!
//! 마우스, 키보드 입력 처리 및 이벤트 생성

use crate::types::*;
use crate::layout::hit_test;

/// UI 이벤트
#[derive(Debug, Clone)]
pub enum UiEvent {
    Click { widget_id: String },
    MouseDown { widget_id: String },
    Hover { widget_id: String },
    HoverEnd { widget_id: String },
    Focus { widget_id: String },
    Blur { widget_id: String },
    ValueChanged { widget_id: String, value: String },
    Custom { widget_id: String, event_name: String },
    DragStart { widget_id: String },
    DragEnd { widget_id: String },
    Drop {
        source_widget_id: String,
        target_widget_id: String,
        data: Option<String>,
    },
}

/// 입력 시스템 - 마우스/키보드 입력 처리
pub struct InputSystem {
    /// 마우스 위치
    pub mouse_pos: (f32, f32),
    /// 호버된 위젯 ID
    pub hovered_widget: Option<String>,
    /// 포커스된 위젯 ID
    pub focused_widget: Option<String>,
    /// 마우스 버튼 누름 상태
    pub mouse_pressed: bool,
    /// 현재 눌린 위젯 ID
    pub pressed_widget: Option<String>,
    /// 드래그 상태
    pub drag_state: Option<DragState>,
    /// 드래그 시작 위치
    pub drag_start_pos: Option<(f32, f32)>,
    /// 툴팁 호버 시간
    pub tooltip_hover_time: f32,
    /// 활성 툴팁 정보
    pub active_tooltip: Option<TooltipInfo>,
    /// 이벤트 큐
    event_queue: Vec<UiEvent>,
}

/// 드래그 상태
#[derive(Debug, Clone)]
pub struct DragState {
    /// 드래그 중인 위젯 ID
    pub source_widget_id: String,
    /// 드래그 데이터
    pub data: Option<String>,
    /// 드래그 그룹
    pub group: Option<String>,
    /// 드래그 시작 시 위젯 내 오프셋
    pub offset: (f32, f32),
    /// 드래그 중인 위젯의 원래 위치
    pub original_rect: Rect,
    /// 현재 마우스 위치
    pub current_pos: (f32, f32),
}

/// 툴팁 정보
#[derive(Debug, Clone)]
pub struct TooltipInfo {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub widget_id: String,
}

/// 드래그 렌더링 정보
#[derive(Debug, Clone)]
pub struct DragRenderInfo {
    pub widget_id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub background_color: Option<Color>,
    pub background_image: Option<String>,
}

/// 특수 키 종류
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpecialKey {
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    SelectAll,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            mouse_pos: (0.0, 0.0),
            hovered_widget: None,
            focused_widget: None,
            mouse_pressed: false,
            pressed_widget: None,
            drag_state: None,
            drag_start_pos: None,
            tooltip_hover_time: 0.0,
            active_tooltip: None,
            event_queue: Vec::new(),
        }
    }

    /// 마우스 이동 처리
    pub fn on_mouse_move(&mut self, root: &mut Widget, x: f32, y: f32) {
        self.mouse_pos = (x, y);

        // 드래그 중이면 드래그 위치 업데이트
        if let Some(ref mut drag) = self.drag_state {
            drag.current_pos = (x, y);

            // 드롭 대상 위젯 찾기
            let drop_target = find_drop_target_at(root, x, y, drag.group.as_deref());
            if let Some(target_id) = drop_target {
                if self.hovered_widget.as_ref() != Some(&target_id) {
                    if let Some(old_id) = self.hovered_widget.take() {
                        set_widget_state(root, &old_id, "default");
                    }
                    set_widget_state(root, &target_id, "hover");
                    self.hovered_widget = Some(target_id);
                }
            }
            return;
        }

        // 드래그 시작 감지
        if self.mouse_pressed {
            if let Some((start_x, start_y)) = self.drag_start_pos {
                let dx = x - start_x;
                let dy = y - start_y;
                let distance = (dx * dx + dy * dy).sqrt();

                const DRAG_THRESHOLD: f32 = 5.0;
                if distance > DRAG_THRESHOLD {
                    if let Some(ref pressed_id) = self.pressed_widget.clone() {
                        if let Some(widget) = find_widget_by_id(root, pressed_id) {
                            if widget.draggable {
                                let offset = (start_x - widget.computed_rect.x, start_y - widget.computed_rect.y);
                                self.drag_state = Some(DragState {
                                    source_widget_id: pressed_id.clone(),
                                    data: widget.drag_data.clone(),
                                    group: widget.drag_group.clone(),
                                    offset,
                                    original_rect: widget.computed_rect,
                                    current_pos: (x, y),
                                });

                                self.event_queue.push(UiEvent::DragStart {
                                    widget_id: pressed_id.clone(),
                                });

                                self.drag_start_pos = None;
                                return;
                            }
                        }
                    }
                }
            }
        }

        // 히트 테스트로 호버 위젯 찾기
        let new_hovered = hit_test(root, x, y).and_then(|w| w.id.clone());

        // 호버 상태 변경 감지
        if new_hovered != self.hovered_widget {
            if let Some(old_id) = self.hovered_widget.take() {
                self.event_queue.push(UiEvent::HoverEnd { widget_id: old_id.clone() });
                set_widget_state(root, &old_id, "default");
            }

            if let Some(ref new_id) = new_hovered {
                self.event_queue.push(UiEvent::Hover { widget_id: new_id.clone() });
                let pressed_id = self.pressed_widget.clone();
                if !self.mouse_pressed || pressed_id.as_ref() != Some(new_id) {
                    set_widget_state(root, new_id, "hover");
                }
            }

            self.tooltip_hover_time = 0.0;
            self.active_tooltip = None;
            self.hovered_widget = new_hovered;
        }
    }

    /// 마우스 버튼 누름 처리
    pub fn on_mouse_down(&mut self, root: &mut Widget, x: f32, y: f32) -> Option<UiEvent> {
        self.mouse_pressed = true;
        self.mouse_pos = (x, y);

        let clicked = hit_test(root, x, y).and_then(|w| w.id.clone());

        if let Some(ref widget_id) = clicked {
            self.pressed_widget = Some(widget_id.clone());
            set_widget_state(root, widget_id, "pressed");

            // 드래그 가능한 위젯이면 드래그 시작 위치 기록
            if let Some(widget) = find_widget_by_id(root, widget_id) {
                if widget.draggable {
                    self.drag_start_pos = Some((x, y));
                }
            }

            // 포커스 변경
            if self.focused_widget.as_ref() != Some(widget_id) {
                if let Some(ref old_focus) = self.focused_widget.clone() {
                    self.event_queue.push(UiEvent::Blur { widget_id: old_focus.clone() });
                    if let Some(old_widget) = find_widget_by_id_mut(root, old_focus) {
                        old_widget.input_focused = false;
                    }
                }
                self.focused_widget = Some(widget_id.clone());
                self.event_queue.push(UiEvent::Focus { widget_id: widget_id.clone() });

                // InputField면 포커스 설정
                if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
                    if matches!(widget.widget_type, WidgetType::InputField { .. }) {
                        widget.input_focused = true;
                        widget.input_cursor_blink = 0.0;
                        if let WidgetType::InputField { ref value, .. } = widget.widget_type {
                            widget.input_cursor_pos = value.chars().count();
                        }
                    }
                }
            }
        } else {
            // 빈 곳 클릭시 포커스 해제
            if let Some(ref old_focus) = self.focused_widget.clone() {
                self.event_queue.push(UiEvent::Blur { widget_id: old_focus.clone() });
                if let Some(old_widget) = find_widget_by_id_mut(root, old_focus) {
                    old_widget.input_focused = false;
                }
            }
            self.focused_widget = None;
        }

        clicked.map(|id| UiEvent::MouseDown { widget_id: id })
    }

    /// 마우스 버튼 해제 처리
    pub fn on_mouse_up(&mut self, root: &mut Widget, x: f32, y: f32) -> Option<UiEvent> {
        self.mouse_pressed = false;
        self.mouse_pos = (x, y);
        self.drag_start_pos = None;

        // 드래그 중이었으면 드롭 처리
        if let Some(drag) = self.drag_state.take() {
            let drop_target = find_drop_target_at(root, x, y, drag.group.as_deref());

            if let Some(ref hovered) = self.hovered_widget.take() {
                set_widget_state(root, hovered, "default");
            }

            set_widget_state(root, &drag.source_widget_id, "default");
            self.pressed_widget = None;

            if let Some(target_id) = drop_target {
                trigger_widget_event(root, &mut self.event_queue, &target_id, "on_drop");

                return Some(UiEvent::Drop {
                    source_widget_id: drag.source_widget_id,
                    target_widget_id: target_id,
                    data: drag.data,
                });
            } else {
                return Some(UiEvent::DragEnd {
                    widget_id: drag.source_widget_id,
                });
            }
        }

        let pressed = self.pressed_widget.take();
        let current = hit_test(root, x, y).and_then(|w| w.id.clone());

        let click_event = if pressed.is_some() && pressed == current {
            let widget_id = pressed.as_ref().unwrap().clone();
            set_widget_state(root, &widget_id, "hover");
            trigger_widget_event(root, &mut self.event_queue, &widget_id, "on_click");
            Some(UiEvent::Click { widget_id })
        } else {
            if let Some(ref pressed_id) = pressed {
                set_widget_state(root, pressed_id, "default");
            }
            if let Some(ref current_id) = current {
                set_widget_state(root, current_id, "hover");
            }
            None
        };

        self.hovered_widget = current;
        click_event
    }

    /// 마우스 휠 스크롤 처리
    pub fn on_mouse_wheel(&mut self, root: &mut Widget, delta_x: f32, delta_y: f32) -> bool {
        let (mouse_x, mouse_y) = self.mouse_pos;

        if let Some(scroll_widget) = find_scrollview_at(root, mouse_x, mouse_y) {
            if let WidgetType::ScrollView { scroll_x, scroll_y } = &scroll_widget.widget_type {
                let scroll_speed = 30.0;

                if *scroll_x {
                    scroll_widget.scroll_offset.0 -= delta_x * scroll_speed;
                }
                if *scroll_y {
                    scroll_widget.scroll_offset.1 -= delta_y * scroll_speed;
                }

                clamp_scroll_offset(scroll_widget);
                return true;
            }
        }

        false
    }

    /// 텍스트 입력 처리
    pub fn on_text_input(&mut self, root: &mut Widget, text: &str) -> bool {
        let focused_id = match &self.focused_widget {
            Some(id) => id.clone(),
            None => return false,
        };

        if let Some(widget) = find_widget_by_id_mut(root, &focused_id) {
            if let WidgetType::InputField { ref mut value, max_length, .. } = &mut widget.widget_type {
                // 선택 영역 삭제
                if let Some((start, end)) = widget.input_selection.take() {
                    let (start, end) = (start.min(end), start.max(end));
                    let mut chars: Vec<char> = value.chars().collect();
                    if end <= chars.len() {
                        chars.drain(start..end);
                        *value = chars.into_iter().collect();
                        widget.input_cursor_pos = start;
                    }
                }

                // 최대 길이 확인
                if let Some(max) = max_length {
                    if value.chars().count() >= *max {
                        return true;
                    }
                }

                // 커서 위치에 텍스트 삽입
                let cursor = widget.input_cursor_pos;
                let mut chars: Vec<char> = value.chars().collect();
                let input_chars: Vec<char> = text.chars().collect();

                let max = max_length.unwrap_or(usize::MAX);
                let available = max.saturating_sub(chars.len());
                let to_insert: Vec<char> = input_chars.into_iter().take(available).collect();
                let insert_count = to_insert.len();

                for (i, c) in to_insert.into_iter().enumerate() {
                    chars.insert(cursor + i, c);
                }
                *value = chars.into_iter().collect();
                widget.input_cursor_pos = cursor + insert_count;
                widget.input_cursor_blink = 0.0;

                self.event_queue.push(UiEvent::ValueChanged {
                    widget_id: focused_id.clone(),
                    value: value.clone(),
                });

                return true;
            }
        }
        false
    }

    /// 특수 키 입력 처리
    pub fn on_special_key(&mut self, root: &mut Widget, key: SpecialKey, shift_held: bool) -> bool {
        let focused_id = match &self.focused_widget {
            Some(id) => id.clone(),
            None => return false,
        };

        if let Some(widget) = find_widget_by_id_mut(root, &focused_id) {
            if let WidgetType::InputField { ref mut value, .. } = &mut widget.widget_type {
                let mut chars: Vec<char> = value.chars().collect();
                let len = chars.len();
                let cursor = widget.input_cursor_pos.min(len);

                match key {
                    SpecialKey::Backspace => {
                        if let Some((start, end)) = widget.input_selection.take() {
                            let (start, end) = (start.min(end), start.max(end));
                            if end <= len {
                                chars.drain(start..end);
                                *value = chars.into_iter().collect();
                                widget.input_cursor_pos = start;
                            }
                        } else if cursor > 0 {
                            chars.remove(cursor - 1);
                            *value = chars.into_iter().collect();
                            widget.input_cursor_pos = cursor - 1;
                        }
                        widget.input_cursor_blink = 0.0;
                    }
                    SpecialKey::Delete => {
                        if let Some((start, end)) = widget.input_selection.take() {
                            let (start, end) = (start.min(end), start.max(end));
                            if end <= len {
                                chars.drain(start..end);
                                *value = chars.into_iter().collect();
                                widget.input_cursor_pos = start;
                            }
                        } else if cursor < len {
                            chars.remove(cursor);
                            *value = chars.into_iter().collect();
                        }
                        widget.input_cursor_blink = 0.0;
                    }
                    SpecialKey::Left => {
                        if shift_held {
                            let sel = widget.input_selection.unwrap_or((cursor, cursor));
                            if cursor > 0 {
                                widget.input_cursor_pos = cursor - 1;
                                widget.input_selection = Some((sel.0, cursor - 1));
                            }
                        } else {
                            if widget.input_selection.is_some() {
                                let (start, end) = widget.input_selection.take().unwrap();
                                widget.input_cursor_pos = start.min(end);
                            } else if cursor > 0 {
                                widget.input_cursor_pos = cursor - 1;
                            }
                        }
                        widget.input_cursor_blink = 0.0;
                    }
                    SpecialKey::Right => {
                        if shift_held {
                            let sel = widget.input_selection.unwrap_or((cursor, cursor));
                            if cursor < len {
                                widget.input_cursor_pos = cursor + 1;
                                widget.input_selection = Some((sel.0, cursor + 1));
                            }
                        } else {
                            if widget.input_selection.is_some() {
                                let (start, end) = widget.input_selection.take().unwrap();
                                widget.input_cursor_pos = start.max(end);
                            } else if cursor < len {
                                widget.input_cursor_pos = cursor + 1;
                            }
                        }
                        widget.input_cursor_blink = 0.0;
                    }
                    SpecialKey::Home => {
                        if shift_held {
                            let sel = widget.input_selection.unwrap_or((cursor, cursor));
                            widget.input_cursor_pos = 0;
                            widget.input_selection = Some((sel.0, 0));
                        } else {
                            widget.input_selection = None;
                            widget.input_cursor_pos = 0;
                        }
                        widget.input_cursor_blink = 0.0;
                    }
                    SpecialKey::End => {
                        if shift_held {
                            let sel = widget.input_selection.unwrap_or((cursor, cursor));
                            widget.input_cursor_pos = len;
                            widget.input_selection = Some((sel.0, len));
                        } else {
                            widget.input_selection = None;
                            widget.input_cursor_pos = len;
                        }
                        widget.input_cursor_blink = 0.0;
                    }
                    SpecialKey::SelectAll => {
                        widget.input_selection = Some((0, len));
                        widget.input_cursor_pos = len;
                    }
                }

                return true;
            }
        }
        false
    }

    /// 툴팁 업데이트
    pub fn update_tooltip(&mut self, root: &Widget, delta_time: f32) {
        if self.drag_state.is_some() || self.mouse_pressed {
            self.active_tooltip = None;
            self.tooltip_hover_time = 0.0;
            return;
        }

        let hovered_id = match &self.hovered_widget {
            Some(id) => id.clone(),
            None => {
                self.active_tooltip = None;
                self.tooltip_hover_time = 0.0;
                return;
            }
        };

        let tooltip_info = if let Some(widget) = find_widget_by_id(root, &hovered_id) {
            if let Some(ref tooltip_text) = widget.tooltip {
                let delay = widget.tooltip_delay.unwrap_or(0.5);
                Some((tooltip_text.clone(), delay, widget.computed_rect))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((text, delay, rect)) = tooltip_info {
            self.tooltip_hover_time += delta_time;

            if self.tooltip_hover_time >= delay && self.active_tooltip.is_none() {
                let tooltip_x = self.mouse_pos.0;
                let tooltip_y = rect.y + rect.height + 8.0;

                self.active_tooltip = Some(TooltipInfo {
                    text,
                    x: tooltip_x,
                    y: tooltip_y,
                    widget_id: hovered_id,
                });
            }
        } else {
            self.active_tooltip = None;
            self.tooltip_hover_time = 0.0;
        }
    }

    /// 입력 필드 커서 깜빡임 업데이트
    pub fn update_input_cursor_blink(&mut self, root: &mut Widget, delta_time: f32) {
        if let Some(ref focused_id) = self.focused_widget {
            if let Some(widget) = find_widget_by_id_mut(root, focused_id) {
                if matches!(widget.widget_type, WidgetType::InputField { .. }) {
                    widget.input_cursor_blink += delta_time;
                    if widget.input_cursor_blink > 1.0 {
                        widget.input_cursor_blink -= 1.0;
                    }
                }
            }
        }
    }

    /// 이벤트 큐에서 이벤트 가져오기
    pub fn poll_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.event_queue)
    }

    /// 마우스가 UI 위에 있는지 확인
    pub fn is_mouse_over_ui(&self) -> bool {
        self.hovered_widget.is_some()
    }

    /// 포커스된 입력 필드가 있는지 확인
    pub fn has_focused_input(&self, root: &Widget) -> bool {
        if let Some(ref focused_id) = self.focused_widget {
            if let Some(widget) = find_widget_by_id(root, focused_id) {
                return matches!(widget.widget_type, WidgetType::InputField { .. });
            }
        }
        false
    }

    /// 드래그 중인지 확인
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_some()
    }

    /// 드래그 정보 가져오기 (렌더링용)
    pub fn get_drag_info(&self, root: &Widget) -> Option<DragRenderInfo> {
        let drag = self.drag_state.as_ref()?;

        if let Some(widget) = find_widget_by_id(root, &drag.source_widget_id) {
            return Some(DragRenderInfo {
                widget_id: drag.source_widget_id.clone(),
                x: drag.current_pos.0 - drag.offset.0,
                y: drag.current_pos.1 - drag.offset.1,
                width: drag.original_rect.width,
                height: drag.original_rect.height,
                background_color: widget.style.background_color,
                background_image: widget.style.background_image.clone(),
            });
        }
        None
    }

    /// 툴팁 정보 가져오기 (렌더링용)
    pub fn get_tooltip_info(&self) -> Option<&TooltipInfo> {
        self.active_tooltip.as_ref()
    }
}

impl Default for InputSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Helper Functions ====================

fn find_widget_by_id<'a>(widget: &'a Widget, id: &str) -> Option<&'a Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &widget.children {
        if let Some(found) = find_widget_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

fn find_widget_by_id_mut<'a>(widget: &'a mut Widget, id: &str) -> Option<&'a mut Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &mut widget.children {
        if let Some(found) = find_widget_by_id_mut(child, id) {
            return Some(found);
        }
    }
    None
}

fn set_widget_state(root: &mut Widget, widget_id: &str, state: &str) {
    if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
        widget.current_state = state.to_string();
    }
}

fn find_drop_target_at(widget: &Widget, x: f32, y: f32, drag_group: Option<&str>) -> Option<String> {
    if !widget.visible {
        return None;
    }

    for child in widget.children.iter().rev() {
        if let Some(found) = find_drop_target_at(child, x, y, drag_group) {
            return Some(found);
        }
    }

    if widget.drop_target && widget.computed_rect.contains(x, y) {
        let group_match = match (drag_group, widget.drag_group.as_deref()) {
            (None, _) => true,
            (_, None) => true,
            (Some(a), Some(b)) => a == b,
        };

        if group_match {
            if let Some(ref id) = widget.id {
                return Some(id.clone());
            }
        }
    }

    None
}

fn find_scrollview_at<'a>(widget: &'a mut Widget, x: f32, y: f32) -> Option<&'a mut Widget> {
    if !widget.visible {
        return None;
    }

    let child_count = widget.children.len();
    for i in (0..child_count).rev() {
        let found_in_child = {
            let child = &widget.children[i];
            find_scrollview_at_check(child, x, y)
        };

        if found_in_child {
            return find_scrollview_at(&mut widget.children[i], x, y);
        }
    }

    if widget.computed_rect.contains(x, y) {
        if matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
            return Some(widget);
        }
    }

    None
}

fn find_scrollview_at_check(widget: &Widget, x: f32, y: f32) -> bool {
    if !widget.visible {
        return false;
    }

    for child in widget.children.iter().rev() {
        if find_scrollview_at_check(child, x, y) {
            return true;
        }
    }

    if widget.computed_rect.contains(x, y) {
        if matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
            return true;
        }
    }

    false
}

fn clamp_scroll_offset(widget: &mut Widget) {
    let viewport_w = widget.computed_rect.width;
    let viewport_h = widget.computed_rect.height;
    let content_w = widget.content_size.0;
    let content_h = widget.content_size.1;

    let max_scroll_x = (content_w - viewport_w).max(0.0);
    let max_scroll_y = (content_h - viewport_h).max(0.0);

    widget.scroll_offset.0 = widget.scroll_offset.0.clamp(0.0, max_scroll_x);
    widget.scroll_offset.1 = widget.scroll_offset.1.clamp(0.0, max_scroll_y);
}

fn trigger_widget_event(root: &mut Widget, event_queue: &mut Vec<UiEvent>, widget_id: &str, event_name: &str) {
    if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
        if let Some(handler) = widget.events.get(event_name) {
            event_queue.push(UiEvent::Custom {
                widget_id: widget_id.to_string(),
                event_name: handler.clone(),
            });
        }
    }
}
