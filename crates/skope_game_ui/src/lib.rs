//! SKOPE Game UI System
//!
//! RON-based declarative UI with data binding and animations.
//!
//! # Features
//! - `gpu`: Enable wgpu-dependent code (renderer, text_renderer)

#![allow(dead_code)]
#![allow(unused_imports)]

// Always available (data structures)
pub mod types;
pub mod parser;
pub mod layout;
pub mod binding;
pub mod animation;
pub mod style;
pub mod hot_reload;

// GPU-dependent modules
#[cfg(feature = "gpu")]
pub mod renderer;
#[cfg(feature = "gpu")]
pub mod text_renderer;

pub use types::*;
pub use parser::*;
pub use layout::*;
pub use binding::*;
pub use animation::*;
pub use style::*;
pub use hot_reload::*;

#[cfg(feature = "gpu")]
pub use renderer::*;
#[cfg(feature = "gpu")]
pub use text_renderer::TextRenderer;

use std::collections::HashMap;
use std::path::Path;

/// UI 시스템 메인 구조체
pub struct UiSystem {
    /// 로드된 UI 위젯 트리
    pub root: Option<Widget>,
    /// 위젯 ID로 빠른 접근
    pub widgets_by_id: HashMap<String, WidgetHandle>,
    /// 데이터 바인딩 컨텍스트
    pub binding_context: BindingContext,
    /// 활성 애니메이션들
    pub active_animations: Vec<ActiveAnimation>,
    /// UI 설정
    pub config: UiConfig,
    /// 현재 화면 크기
    pub screen_size: (f32, f32),
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
    /// 이벤트 큐
    pub event_queue: Vec<UiEvent>,
    /// 드래그 상태
    pub drag_state: Option<DragState>,
    /// 드래그 시작 위치 (드래그 시작 감지용)
    pub drag_start_pos: Option<(f32, f32)>,
    /// 툴팁 호버 시간 (현재 위젯에 얼마나 오래 호버했는지)
    pub tooltip_hover_time: f32,
    /// 활성 툴팁 정보
    pub active_tooltip: Option<TooltipInfo>,
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
    /// 툴팁 텍스트
    pub text: String,
    /// 툴팁 표시 위치 X
    pub x: f32,
    /// 툴팁 표시 위치 Y
    pub y: f32,
    /// 소스 위젯 ID
    pub widget_id: String,
}

impl UiSystem {
    pub fn new() -> Self {
        Self {
            root: None,
            widgets_by_id: HashMap::new(),
            binding_context: BindingContext::new(),
            active_animations: Vec::new(),
            config: UiConfig::default(),
            screen_size: (1920.0, 1080.0),
            mouse_pos: (0.0, 0.0),
            hovered_widget: None,
            focused_widget: None,
            mouse_pressed: false,
            pressed_widget: None,
            event_queue: Vec::new(),
            drag_state: None,
            drag_start_pos: None,
            tooltip_hover_time: 0.0,
            active_tooltip: None,
        }
    }

    /// RON 파일에서 UI 로드
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), UiError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| UiError::IoError(e.to_string()))?;
        self.load_from_str(&content)
    }

    /// RON 문자열에서 UI 로드
    pub fn load_from_str(&mut self, ron_str: &str) -> Result<(), UiError> {
        let widget = parse_widget(ron_str)?;
        self.set_root(widget);
        Ok(())
    }

    /// 루트 위젯 설정
    pub fn set_root(&mut self, widget: Widget) {
        self.widgets_by_id.clear();
        self.index_widget(&widget);
        self.root = Some(widget);
    }

    /// 위젯 인덱싱 (ID로 빠른 접근용)
    fn index_widget(&mut self, widget: &Widget) {
        if let Some(ref id) = widget.id {
            self.widgets_by_id.insert(id.clone(), WidgetHandle {
                id: id.clone(),
            });
        }
        for child in &widget.children {
            self.index_widget(child);
        }
    }

    /// 화면 크기 업데이트
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = (width, height);
    }

    /// 마우스 이동 처리
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_pos = (x, y);

        // 드래그 중이면 드래그 위치 업데이트
        if let Some(ref mut drag) = self.drag_state {
            drag.current_pos = (x, y);

            // 드래그 중 드롭 대상 위젯 찾기
            if let Some(ref root) = self.root {
                let drop_target = find_drop_target_at(root, x, y, drag.group.as_deref());
                if let Some(target_id) = drop_target {
                    // 드롭 대상 호버 상태
                    if self.hovered_widget.as_ref() != Some(&target_id) {
                        if let Some(old_id) = self.hovered_widget.take() {
                            self.set_widget_state_internal(&old_id, "default");
                        }
                        self.set_widget_state_internal(&target_id, "hover");
                        self.hovered_widget = Some(target_id);
                    }
                }
            }
            return;
        }

        // 드래그 시작 감지 (마우스 누름 상태에서 일정 거리 이동)
        if self.mouse_pressed {
            if let Some((start_x, start_y)) = self.drag_start_pos {
                let dx = x - start_x;
                let dy = y - start_y;
                let distance = (dx * dx + dy * dy).sqrt();

                const DRAG_THRESHOLD: f32 = 5.0;
                if distance > DRAG_THRESHOLD {
                    // 드래그 시작!
                    if let Some(ref pressed_id) = self.pressed_widget.clone() {
                        if let Some(ref root) = self.root {
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
        }

        // 히트 테스트로 호버 위젯 찾기
        let new_hovered = if let Some(ref root) = self.root {
            hit_test(root, x, y)
                .and_then(|w| w.id.clone())
        } else {
            None
        };

        // 호버 상태 변경 감지
        if new_hovered != self.hovered_widget {
            // 이전 호버 해제
            if let Some(old_id) = self.hovered_widget.take() {
                self.event_queue.push(UiEvent::HoverEnd { widget_id: old_id.clone() });
                self.set_widget_state_internal(&old_id, "default");
            }

            // 새 호버 설정
            if let Some(ref new_id) = new_hovered {
                self.event_queue.push(UiEvent::Hover { widget_id: new_id.clone() });
                // 누름 상태가 아니면 hover 상태로
                let pressed_id = self.pressed_widget.clone();
                if !self.mouse_pressed || pressed_id.as_ref() != Some(new_id) {
                    self.set_widget_state_internal(new_id, "hover");
                }
            }

            // 호버 위젯이 바뀌면 툴팁 상태 리셋
            self.tooltip_hover_time = 0.0;
            self.active_tooltip = None;

            self.hovered_widget = new_hovered;
        }
    }

    /// 마우스 버튼 누름 처리
    pub fn on_mouse_down(&mut self, x: f32, y: f32) -> Option<UiEvent> {
        self.mouse_pressed = true;
        self.mouse_pos = (x, y);

        // 히트 테스트로 클릭된 위젯 찾기
        let clicked = if let Some(ref root) = self.root {
            hit_test(root, x, y)
                .and_then(|w| w.id.clone())
        } else {
            None
        };

        if let Some(ref widget_id) = clicked {
            self.pressed_widget = Some(widget_id.clone());
            self.set_widget_state_internal(widget_id, "pressed");

            // 드래그 가능한 위젯이면 드래그 시작 위치 기록
            if let Some(ref root) = self.root {
                if let Some(widget) = find_widget_by_id(root, widget_id) {
                    if widget.draggable {
                        self.drag_start_pos = Some((x, y));
                    }
                }
            }

            // 포커스 변경
            if self.focused_widget.as_ref() != Some(widget_id) {
                // 이전 포커스 해제
                if let Some(ref old_focus) = self.focused_widget.clone() {
                    self.event_queue.push(UiEvent::Blur { widget_id: old_focus.clone() });
                    // 이전 InputField의 포커스 해제
                    if let Some(ref mut root) = self.root {
                        if let Some(old_widget) = find_widget_by_id_mut(root, old_focus) {
                            old_widget.input_focused = false;
                        }
                    }
                }
                self.focused_widget = Some(widget_id.clone());
                self.event_queue.push(UiEvent::Focus { widget_id: widget_id.clone() });

                // InputField면 포커스 설정
                if let Some(ref mut root) = self.root {
                    if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
                        if matches!(widget.widget_type, WidgetType::InputField { .. }) {
                            widget.input_focused = true;
                            widget.input_cursor_blink = 0.0;
                            // 커서를 끝으로 이동
                            if let WidgetType::InputField { ref value, .. } = widget.widget_type {
                                widget.input_cursor_pos = value.chars().count();
                            }
                        }
                    }
                }
            }
        } else {
            // 빈 곳 클릭시 포커스 해제
            if let Some(ref old_focus) = self.focused_widget.clone() {
                self.event_queue.push(UiEvent::Blur { widget_id: old_focus.clone() });
                // InputField의 포커스 해제
                if let Some(ref mut root) = self.root {
                    if let Some(old_widget) = find_widget_by_id_mut(root, &old_focus) {
                        old_widget.input_focused = false;
                    }
                }
            }
            self.focused_widget = None;
        }

        clicked.map(|id| UiEvent::MouseDown { widget_id: id })
    }

    /// 마우스 버튼 해제 처리
    pub fn on_mouse_up(&mut self, x: f32, y: f32) -> Option<UiEvent> {
        self.mouse_pressed = false;
        self.mouse_pos = (x, y);
        self.drag_start_pos = None;

        // 드래그 중이었으면 드롭 처리
        if let Some(drag) = self.drag_state.take() {
            // 드롭 대상 찾기
            let drop_target = if let Some(ref root) = self.root {
                find_drop_target_at(root, x, y, drag.group.as_deref())
            } else {
                None
            };

            // 드롭 대상 상태 초기화
            if let Some(ref hovered) = self.hovered_widget.take() {
                self.set_widget_state_internal(hovered, "default");
            }

            // 드래그 소스 위젯 상태 초기화
            self.set_widget_state_internal(&drag.source_widget_id, "default");
            self.pressed_widget = None;

            if let Some(target_id) = drop_target {
                // 드롭 성공
                self.trigger_widget_event(&target_id, "on_drop");

                return Some(UiEvent::Drop {
                    source_widget_id: drag.source_widget_id,
                    target_widget_id: target_id,
                    data: drag.data,
                });
            } else {
                // 드롭 실패 (빈 곳에 드롭)
                return Some(UiEvent::DragEnd {
                    widget_id: drag.source_widget_id,
                });
            }
        }

        let pressed = self.pressed_widget.take();

        // 히트 테스트로 현재 위치의 위젯 찾기
        let current = if let Some(ref root) = self.root {
            hit_test(root, x, y)
                .and_then(|w| w.id.clone())
        } else {
            None
        };

        // 같은 위젯에서 누르고 뗐으면 클릭 이벤트
        let click_event = if pressed.is_some() && pressed == current {
            let widget_id = pressed.as_ref().unwrap().clone();
            self.set_widget_state_internal(&widget_id, "hover");

            // 이벤트 핸들러 확인
            self.trigger_widget_event(&widget_id, "on_click");

            Some(UiEvent::Click { widget_id })
        } else {
            // 누른 위젯에서 떨어진 곳에서 뗌
            if let Some(ref pressed_id) = pressed {
                self.set_widget_state_internal(pressed_id, "default");
            }
            if let Some(ref current_id) = current {
                self.set_widget_state_internal(current_id, "hover");
            }
            None
        };

        self.hovered_widget = current;
        click_event
    }

    /// 위젯 이벤트 트리거
    fn trigger_widget_event(&mut self, widget_id: &str, event_name: &str) {
        if let Some(ref mut root) = self.root {
            if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
                if let Some(handler) = widget.events.get(event_name) {
                    self.event_queue.push(UiEvent::Custom {
                        widget_id: widget_id.to_string(),
                        event_name: handler.clone(),
                    });
                }
            }
        }
    }

    /// 위젯 상태 변경 (내부용)
    fn set_widget_state_internal(&mut self, widget_id: &str, state: &str) {
        if let Some(ref mut root) = self.root {
            if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
                widget.current_state = state.to_string();
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

    /// 마우스 휠 스크롤 처리
    pub fn on_mouse_wheel(&mut self, delta_x: f32, delta_y: f32) -> bool {
        let (mouse_x, mouse_y) = self.mouse_pos;

        // 마우스 위치에서 ScrollView 찾기
        if let Some(ref mut root) = self.root {
            if let Some(scroll_widget) = find_scrollview_at(root, mouse_x, mouse_y) {
                // ScrollView의 스크롤 가능 방향 확인
                if let WidgetType::ScrollView { scroll_x, scroll_y } = &scroll_widget.widget_type {
                    let scroll_speed = 30.0; // 스크롤 속도

                    if *scroll_x {
                        scroll_widget.scroll_offset.0 -= delta_x * scroll_speed;
                    }
                    if *scroll_y {
                        scroll_widget.scroll_offset.1 -= delta_y * scroll_speed;
                    }

                    // 스크롤 범위 제한
                    clamp_scroll_offset(scroll_widget);

                    return true; // 스크롤 처리됨
                }
            }
        }

        false // 스크롤 처리 안됨
    }

    /// 특정 위젯의 스크롤 위치 설정
    pub fn set_scroll_offset(&mut self, widget_id: &str, offset_x: f32, offset_y: f32) {
        if let Some(ref mut root) = self.root {
            if let Some(widget) = find_widget_by_id_mut(root, widget_id) {
                widget.scroll_offset = (offset_x, offset_y);
                clamp_scroll_offset(widget);
            }
        }
    }

    /// 특정 위젯의 스크롤 위치 가져오기
    pub fn get_scroll_offset(&self, widget_id: &str) -> Option<(f32, f32)> {
        if let Some(ref root) = self.root {
            if let Some(widget) = find_widget_by_id(root, widget_id) {
                return Some(widget.scroll_offset);
            }
        }
        None
    }

    /// 프레임 업데이트
    pub fn update(&mut self, delta_time: f32) {
        // 애니메이션 업데이트 및 적용
        self.update_animations(delta_time);

        // 툴팁 업데이트
        self.update_tooltip(delta_time);

        // 데이터 바인딩 업데이트
        if let Some(ref mut root) = self.root {
            self.binding_context.update_widget(root);
        }
    }

    /// 툴팁 업데이트
    fn update_tooltip(&mut self, delta_time: f32) {
        // 드래그 중이거나 마우스 누름 상태면 툴팁 숨김
        if self.drag_state.is_some() || self.mouse_pressed {
            self.active_tooltip = None;
            self.tooltip_hover_time = 0.0;
            return;
        }

        // 호버된 위젯이 없으면 툴팁 숨김
        let hovered_id = match &self.hovered_widget {
            Some(id) => id.clone(),
            None => {
                self.active_tooltip = None;
                self.tooltip_hover_time = 0.0;
                return;
            }
        };

        // 호버된 위젯의 툴팁 정보 확인
        let tooltip_info = if let Some(ref root) = self.root {
            if let Some(widget) = find_widget_by_id(root, &hovered_id) {
                if let Some(ref tooltip_text) = widget.tooltip {
                    let delay = widget.tooltip_delay.unwrap_or(0.5);
                    Some((tooltip_text.clone(), delay, widget.computed_rect))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // 툴팁이 있는 위젯이면 타이머 업데이트
        if let Some((text, delay, rect)) = tooltip_info {
            self.tooltip_hover_time += delta_time;

            // 지연 시간이 지났으면 툴팁 활성화
            if self.tooltip_hover_time >= delay && self.active_tooltip.is_none() {
                // 툴팁 위치 계산 (위젯 아래에 표시, 마우스 근처)
                let tooltip_x = self.mouse_pos.0;
                let tooltip_y = rect.y + rect.height + 8.0; // 위젯 아래 8px

                self.active_tooltip = Some(TooltipInfo {
                    text,
                    x: tooltip_x,
                    y: tooltip_y,
                    widget_id: hovered_id,
                });
            }
        } else {
            // 툴팁이 없는 위젯
            self.active_tooltip = None;
            self.tooltip_hover_time = 0.0;
        }
    }

    /// 애니메이션 업데이트
    fn update_animations(&mut self, delta_time: f32) {
        // 완료된 애니메이션의 on_complete 이벤트 수집
        let mut completed_events: Vec<String> = Vec::new();

        // 애니메이션 업데이트 및 위젯 속성 적용
        let animations: Vec<ActiveAnimation> = self.active_animations.drain(..).collect();

        for mut anim in animations {
            anim.elapsed += delta_time;
            let progress = anim.progress();

            // 위젯에 애니메이션 속성 적용
            if let Some(ref mut root) = self.root {
                if let Some(widget) = find_widget_by_id_mut(root, &anim.widget_id) {
                    for track in &anim.tracks {
                        let value = track.value_at(progress);
                        apply_animated_value(widget, &track.property, &value);
                    }
                }
            }

            // 완료 확인
            if anim.is_complete() {
                if let Some(ref event) = anim.on_complete {
                    completed_events.push(event.clone());
                }

                // Loop 또는 PingPong인 경우 다시 추가
                match anim.repeat {
                    AnimationRepeat::Loop => {
                        anim.elapsed = anim.elapsed % anim.duration;
                        self.active_animations.push(anim);
                    }
                    AnimationRepeat::PingPong => {
                        // 방향 반전 (tracks의 from/to 교환)
                        anim.elapsed = anim.elapsed % anim.duration;
                        for track in &mut anim.tracks {
                            std::mem::swap(&mut track.from, &mut track.to);
                        }
                        self.active_animations.push(anim);
                    }
                    AnimationRepeat::Count(n) => {
                        let cycles = ((anim.elapsed - anim.delay) / anim.duration) as u32;
                        if cycles < n {
                            self.active_animations.push(anim);
                        }
                    }
                    AnimationRepeat::Once => {
                        // 완료됨 - 추가하지 않음
                    }
                }
            } else {
                self.active_animations.push(anim);
            }
        }

        // 완료 이벤트 발행
        for event_name in completed_events {
            self.event_queue.push(UiEvent::Custom {
                widget_id: String::new(),
                event_name,
            });
        }
    }

    /// 애니메이션 재생
    pub fn play_animation(&mut self, animation: ActiveAnimation) {
        // 같은 위젯의 같은 이름 애니메이션 제거
        self.active_animations.retain(|a| {
            !(a.widget_id == animation.widget_id && a.name == animation.name)
        });
        self.active_animations.push(animation);
    }

    /// 애니메이션 중지
    pub fn stop_animation(&mut self, widget_id: &str, animation_name: Option<&str>) {
        self.active_animations.retain(|a| {
            if a.widget_id != widget_id {
                return true;
            }
            if let Some(name) = animation_name {
                a.name != name
            } else {
                false // 모든 애니메이션 제거
            }
        });
    }

    /// 모든 애니메이션 중지
    pub fn stop_all_animations(&mut self) {
        self.active_animations.clear();
    }

    /// 레이아웃 계산
    pub fn calculate_layout(&mut self) {
        if let Some(ref mut root) = self.root {
            let screen_rect = Rect {
                x: 0.0,
                y: 0.0,
                width: self.screen_size.0,
                height: self.screen_size.1,
            };
            calculate_widget_layout(root, &screen_rect, &self.config);
        }
    }

    /// 위젯 상태 변경 (외부용)
    pub fn set_widget_state(&mut self, widget_id: &str, state: &str) {
        self.set_widget_state_internal(widget_id, state);
    }

    /// 데이터 바인딩 값 설정
    pub fn set_binding_value(&mut self, key: &str, value: BindingValue) {
        self.binding_context.set(key, value);
    }

    /// ID로 위젯 찾기
    pub fn get_widget(&self, id: &str) -> Option<&Widget> {
        if let Some(ref root) = self.root {
            find_widget_by_id(root, id)
        } else {
            None
        }
    }

    /// ID로 위젯 찾기 (mutable)
    pub fn get_widget_mut(&mut self, id: &str) -> Option<&mut Widget> {
        if let Some(ref mut root) = self.root {
            find_widget_by_id_mut(root, id)
        } else {
            None
        }
    }

    // ==================== 입력 필드 처리 ====================

    /// 텍스트 입력 처리 (문자 입력)
    pub fn on_text_input(&mut self, text: &str) -> bool {
        // 포커스된 InputField 위젯 찾기
        let focused_id = match &self.focused_widget {
            Some(id) => id.clone(),
            None => return false,
        };

        if let Some(ref mut root) = self.root {
            if let Some(widget) = find_widget_by_id_mut(root, &focused_id) {
                if let WidgetType::InputField { ref mut value, max_length, .. } = &mut widget.widget_type {
                    // 선택 영역이 있으면 삭제
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

                    // 최대 길이 제한
                    let max = max_length.unwrap_or(usize::MAX);
                    let available = max.saturating_sub(chars.len());
                    let to_insert: Vec<char> = input_chars.into_iter().take(available).collect();
                    let insert_count = to_insert.len();

                    // 삽입
                    for (i, c) in to_insert.into_iter().enumerate() {
                        chars.insert(cursor + i, c);
                    }
                    *value = chars.into_iter().collect();
                    widget.input_cursor_pos = cursor + insert_count;
                    widget.input_cursor_blink = 0.0; // 커서 깜빡임 리셋

                    // 값 변경 이벤트
                    self.event_queue.push(UiEvent::ValueChanged {
                        widget_id: focused_id.clone(),
                        value: value.clone(),
                    });

                    return true;
                }
            }
        }
        false
    }

    /// 특수 키 입력 처리 (Backspace, Delete, 화살표 등)
    pub fn on_special_key(&mut self, key: SpecialKey, shift_held: bool) -> bool {
        let focused_id = match &self.focused_widget {
            Some(id) => id.clone(),
            None => return false,
        };

        if let Some(ref mut root) = self.root {
            if let Some(widget) = find_widget_by_id_mut(root, &focused_id) {
                if let WidgetType::InputField { ref mut value, .. } = &mut widget.widget_type {
                    let mut chars: Vec<char> = value.chars().collect();
                    let len = chars.len();
                    let cursor = widget.input_cursor_pos.min(len);

                    match key {
                        SpecialKey::Backspace => {
                            // 선택 영역이 있으면 삭제
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
                                // 선택 확장
                                let sel = widget.input_selection.unwrap_or((cursor, cursor));
                                if cursor > 0 {
                                    widget.input_cursor_pos = cursor - 1;
                                    widget.input_selection = Some((sel.0, cursor - 1));
                                }
                            } else {
                                // 선택 해제 후 이동
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
        }
        false
    }

    /// 입력 필드 커서 깜빡임 업데이트
    pub fn update_input_cursor_blink(&mut self, delta_time: f32) {
        if let Some(ref focused_id) = self.focused_widget {
            if let Some(ref mut root) = self.root {
                if let Some(widget) = find_widget_by_id_mut(root, focused_id) {
                    if matches!(widget.widget_type, WidgetType::InputField { .. }) {
                        widget.input_cursor_blink += delta_time;
                        // 1초 주기로 리셋
                        if widget.input_cursor_blink > 1.0 {
                            widget.input_cursor_blink -= 1.0;
                        }
                    }
                }
            }
        }
    }

    /// 포커스된 입력 필드가 있는지 확인
    pub fn has_focused_input(&self) -> bool {
        if let Some(ref focused_id) = self.focused_widget {
            if let Some(ref root) = self.root {
                if let Some(widget) = find_widget_by_id(root, focused_id) {
                    return matches!(widget.widget_type, WidgetType::InputField { .. });
                }
            }
        }
        false
    }

    /// 드래그 중인지 확인
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_some()
    }

    /// 드래그 정보 가져오기 (렌더링용)
    pub fn get_drag_info(&self) -> Option<DragRenderInfo> {
        let drag = self.drag_state.as_ref()?;

        // 드래그 중인 위젯의 원본 찾기
        if let Some(ref root) = self.root {
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
        }
        None
    }

    /// 툴팁 정보 가져오기 (렌더링용)
    pub fn get_tooltip_info(&self) -> Option<&TooltipInfo> {
        self.active_tooltip.as_ref()
    }
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
    SelectAll, // Ctrl+A
}

impl Default for UiSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// 위젯 핸들 (ID 참조용)
#[derive(Debug, Clone)]
pub struct WidgetHandle {
    pub id: String,
}

/// UI 이벤트
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// 마우스 클릭 (down + up on same widget)
    Click { widget_id: String },
    /// 마우스 버튼 누름
    MouseDown { widget_id: String },
    /// 마우스 호버 시작
    Hover { widget_id: String },
    /// 마우스 호버 종료
    HoverEnd { widget_id: String },
    /// 포커스 획득
    Focus { widget_id: String },
    /// 포커스 상실
    Blur { widget_id: String },
    /// 값 변경 (입력 필드, 슬라이더 등)
    ValueChanged { widget_id: String, value: String },
    /// 커스텀 이벤트
    Custom { widget_id: String, event_name: String },
    /// 드래그 시작
    DragStart { widget_id: String },
    /// 드래그 종료 (드롭 없이)
    DragEnd { widget_id: String },
    /// 드롭 (드래그 완료)
    Drop {
        source_widget_id: String,
        target_widget_id: String,
        data: Option<String>,
    },
}

/// UI 에러
#[derive(Debug)]
pub enum UiError {
    ParseError(String),
    IoError(String),
    LayoutError(String),
    BindingError(String),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::ParseError(s) => write!(f, "Parse error: {}", s),
            UiError::IoError(s) => write!(f, "IO error: {}", s),
            UiError::LayoutError(s) => write!(f, "Layout error: {}", s),
            UiError::BindingError(s) => write!(f, "Binding error: {}", s),
        }
    }
}

impl std::error::Error for UiError {}

/// 재귀적으로 위젯 찾기
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

/// 재귀적으로 위젯 찾기 (mutable)
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

/// 드롭 대상 위젯 찾기
fn find_drop_target_at(widget: &Widget, x: f32, y: f32, drag_group: Option<&str>) -> Option<String> {
    if !widget.visible {
        return None;
    }

    // 자식들 먼저 확인 (위에 있는 것 우선)
    for child in widget.children.iter().rev() {
        if let Some(found) = find_drop_target_at(child, x, y, drag_group) {
            return Some(found);
        }
    }

    // 현재 위젯이 드롭 대상이고 마우스가 그 안에 있으면
    if widget.drop_target && widget.computed_rect.contains(x, y) {
        // 드래그 그룹 확인
        let group_match = match (drag_group, widget.drag_group.as_deref()) {
            (None, _) => true,              // 드래그 그룹 없으면 모두 허용
            (_, None) => true,              // 대상 그룹 없으면 모두 허용
            (Some(a), Some(b)) => a == b,   // 둘 다 있으면 같아야 함
        };

        if group_match {
            if let Some(ref id) = widget.id {
                return Some(id.clone());
            }
        }
    }

    None
}

/// 애니메이션 값을 위젯에 적용
fn apply_animated_value(widget: &mut Widget, property: &AnimatedProperty, value: &AnimatedValue) {
    match property {
        AnimatedProperty::Opacity => {
            widget.style.opacity = value.as_float();
        }
        AnimatedProperty::OffsetX => {
            widget.layout.offset.0 = value.as_float();
        }
        AnimatedProperty::OffsetY => {
            widget.layout.offset.1 = value.as_float();
        }
        AnimatedProperty::ScaleX => {
            widget.style.scale.0 = value.as_float();
        }
        AnimatedProperty::ScaleY => {
            widget.style.scale.1 = value.as_float();
        }
        AnimatedProperty::Rotation => {
            widget.style.rotation = value.as_float();
        }
        AnimatedProperty::Width => {
            if let Size::Fixed(_, h) = widget.layout.size {
                widget.layout.size = Size::Fixed(value.as_float(), h);
            }
        }
        AnimatedProperty::Height => {
            if let Size::Fixed(w, _) = widget.layout.size {
                widget.layout.size = Size::Fixed(w, value.as_float());
            }
        }
        AnimatedProperty::BackgroundColor => {
            let [r, g, b, a] = value.as_color();
            widget.style.background_color = Some(Color::Rgba(r, g, b, a));
        }
        AnimatedProperty::TextColor => {
            let [r, g, b, a] = value.as_color();
            widget.style.text_color = Some(Color::Rgba(r, g, b, a));
        }
        AnimatedProperty::BorderColor => {
            let [r, g, b, a] = value.as_color();
            widget.style.border_color = Some(Color::Rgba(r, g, b, a));
        }
        AnimatedProperty::BorderRadius => {
            widget.style.border_radius = value.as_float();
        }
        AnimatedProperty::BorderWidth => {
            widget.style.border_width = value.as_float();
        }
    }
}

/// 마우스 위치에서 ScrollView 찾기
fn find_scrollview_at<'a>(widget: &'a mut Widget, x: f32, y: f32) -> Option<&'a mut Widget> {
    if !widget.visible {
        return None;
    }

    // 자식들 먼저 확인 (z-order) - 인덱스 기반으로 수정
    let child_count = widget.children.len();
    for i in (0..child_count).rev() {
        // 각 반복에서 새로 빌림
        let found_in_child = {
            let child = &mut widget.children[i];
            if let Some(_) = find_scrollview_at_check(child, x, y) {
                true
            } else {
                false
            }
        };

        if found_in_child {
            return find_scrollview_at(&mut widget.children[i], x, y);
        }
    }

    // 현재 위젯이 ScrollView이고 마우스가 그 안에 있으면 반환
    if widget.computed_rect.contains(x, y) {
        if matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
            return Some(widget);
        }
    }

    None
}

/// ScrollView가 있는지 확인만 (빌림 문제 해결용)
fn find_scrollview_at_check(widget: &Widget, x: f32, y: f32) -> Option<()> {
    if !widget.visible {
        return None;
    }

    // 자식들 먼저 확인
    for child in widget.children.iter().rev() {
        if find_scrollview_at_check(child, x, y).is_some() {
            return Some(());
        }
    }

    // 현재 위젯이 ScrollView이고 마우스가 그 안에 있으면
    if widget.computed_rect.contains(x, y) {
        if matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
            return Some(());
        }
    }

    None
}

/// 스크롤 오프셋 범위 제한
fn clamp_scroll_offset(widget: &mut Widget) {
    let viewport_w = widget.computed_rect.width;
    let viewport_h = widget.computed_rect.height;
    let content_w = widget.content_size.0;
    let content_h = widget.content_size.1;

    // 최대 스크롤 범위 계산
    let max_scroll_x = (content_w - viewport_w).max(0.0);
    let max_scroll_y = (content_h - viewport_h).max(0.0);

    // 스크롤 범위 제한
    widget.scroll_offset.0 = widget.scroll_offset.0.clamp(0.0, max_scroll_x);
    widget.scroll_offset.1 = widget.scroll_offset.1.clamp(0.0, max_scroll_y);
}

/// ScrollView의 컨텐츠 크기 계산
pub fn calculate_scroll_content_size(widget: &mut Widget) {
    if !matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
        return;
    }

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;

    // 자식들의 최대 범위 계산
    for child in &widget.children {
        let child_right = child.computed_rect.x + child.computed_rect.width - widget.computed_rect.x;
        let child_bottom = child.computed_rect.y + child.computed_rect.height - widget.computed_rect.y;

        max_x = max_x.max(child_right);
        max_y = max_y.max(child_bottom);
    }

    widget.content_size = (max_x, max_y);
}
