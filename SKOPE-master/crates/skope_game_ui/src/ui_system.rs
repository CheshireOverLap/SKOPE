//! SKOPE UI - UiSystem
//!
//! 서브시스템들을 조합하여 UI를 관리하는 메인 시스템

use std::collections::HashMap;
use std::path::Path;

use crate::types::*;
use crate::parser::parse_widget;
use crate::binding::BindingValue;
use crate::animation::ActiveAnimation;
use crate::systems::{
    InputSystem, LayoutSystem, AnimationSystem, BindingSystem, StateManager, UiEvent,
};
use crate::systems::input_system::{
    DragState, TooltipInfo, DragRenderInfo, SpecialKey,
};

/// UI 시스템 메인 구조체
pub struct UiSystem {
    /// 로드된 UI 위젯 트리
    pub root: Option<Widget>,
    /// 위젯 ID로 빠른 접근
    pub widgets_by_id: HashMap<String, WidgetHandle>,

    // ============ 서브시스템들 ============
    /// 입력 처리 시스템
    pub input: InputSystem,
    /// 레이아웃 시스템
    pub layout: LayoutSystem,
    /// 애니메이션 시스템
    pub animation: AnimationSystem,
    /// 바인딩 시스템
    pub binding: BindingSystem,
}

/// 위젯 핸들 (ID 참조용)
#[derive(Debug, Clone)]
pub struct WidgetHandle {
    pub id: String,
}

// UiEvent는 systems 모듈에서 re-export됨

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

impl UiSystem {
    pub fn new() -> Self {
        Self {
            root: None,
            widgets_by_id: HashMap::new(),
            input: InputSystem::new(),
            layout: LayoutSystem::default(),
            animation: AnimationSystem::new(),
            binding: BindingSystem::new(),
        }
    }

    // ==================== 위젯 관리 ====================

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

    /// 위젯 인덱싱
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

    // ==================== 화면 설정 ====================

    /// 화면 크기 업데이트
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.layout.set_screen_size(width, height);
    }

    // ==================== 입력 처리 (InputSystem 위임) ====================

    /// 마우스 이동 처리
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if let Some(ref mut root) = self.root {
            self.input.on_mouse_move(root, x, y);
        }
    }

    /// 마우스 버튼 누름 처리
    pub fn on_mouse_down(&mut self, x: f32, y: f32) -> Option<UiEvent> {
        if let Some(ref mut root) = self.root {
            self.input.on_mouse_down(root, x, y)
        } else {
            None
        }
    }

    /// 마우스 버튼 해제 처리
    pub fn on_mouse_up(&mut self, x: f32, y: f32) -> Option<UiEvent> {
        if let Some(ref mut root) = self.root {
            self.input.on_mouse_up(root, x, y)
        } else {
            None
        }
    }

    /// 마우스 휠 스크롤 처리
    pub fn on_mouse_wheel(&mut self, delta_x: f32, delta_y: f32) -> bool {
        if let Some(ref mut root) = self.root {
            self.input.on_mouse_wheel(root, delta_x, delta_y)
        } else {
            false
        }
    }

    /// 텍스트 입력 처리
    pub fn on_text_input(&mut self, text: &str) -> bool {
        if let Some(ref mut root) = self.root {
            self.input.on_text_input(root, text)
        } else {
            false
        }
    }

    /// 특수 키 입력 처리
    pub fn on_special_key(&mut self, key: SpecialKey, shift_held: bool) -> bool {
        if let Some(ref mut root) = self.root {
            self.input.on_special_key(root, key, shift_held)
        } else {
            false
        }
    }

    // ==================== 상태 조회 ====================

    /// 이벤트 큐에서 이벤트 가져오기
    pub fn poll_events(&mut self) -> Vec<UiEvent> {
        self.input.poll_events()
    }

    /// 마우스가 UI 위에 있는지 확인
    pub fn is_mouse_over_ui(&self) -> bool {
        self.input.is_mouse_over_ui()
    }

    /// 포커스된 입력 필드가 있는지 확인
    pub fn has_focused_input(&self) -> bool {
        if let Some(ref root) = self.root {
            self.input.has_focused_input(root)
        } else {
            false
        }
    }

    /// 드래그 중인지 확인
    pub fn is_dragging(&self) -> bool {
        self.input.is_dragging()
    }

    /// 드래그 정보 가져오기 (렌더링용)
    pub fn get_drag_info(&self) -> Option<DragRenderInfo> {
        if let Some(ref root) = self.root {
            self.input.get_drag_info(root)
        } else {
            None
        }
    }

    /// 툴팁 정보 가져오기 (렌더링용)
    pub fn get_tooltip_info(&self) -> Option<&TooltipInfo> {
        self.input.get_tooltip_info()
    }

    // ==================== 호버/포커스 상태 (호환성) ====================

    /// 호버된 위젯 ID
    pub fn hovered_widget(&self) -> Option<&String> {
        self.input.hovered_widget.as_ref()
    }

    /// 포커스된 위젯 ID
    pub fn focused_widget(&self) -> Option<&String> {
        self.input.focused_widget.as_ref()
    }

    /// 드래그 상태
    pub fn drag_state(&self) -> Option<&DragState> {
        self.input.drag_state.as_ref()
    }

    // ==================== 업데이트 ====================

    /// 프레임 업데이트
    pub fn update(&mut self, delta_time: f32) {
        if let Some(ref mut root) = self.root {
            // 애니메이션 업데이트
            let _completed_events = self.animation.update(root, delta_time);

            // 툴팁 업데이트
            self.input.update_tooltip(root, delta_time);

            // 바인딩 업데이트
            self.binding.update_widget(root);
        }
    }

    /// 입력 필드 커서 깜빡임 업데이트
    pub fn update_input_cursor_blink(&mut self, delta_time: f32) {
        if let Some(ref mut root) = self.root {
            self.input.update_input_cursor_blink(root, delta_time);
        }
    }

    /// 레이아웃 계산
    pub fn calculate_layout(&mut self) {
        if let Some(ref mut root) = self.root {
            self.layout.calculate(root);
        }
    }

    // ==================== 바인딩 (BindingSystem 위임) ====================

    /// 데이터 바인딩 값 설정
    pub fn set_binding_value(&mut self, key: &str, value: BindingValue) {
        self.binding.set(key, value);
    }

    /// 데이터 바인딩 값 가져오기
    pub fn get_binding_value(&self, key: &str) -> Option<&BindingValue> {
        self.binding.get(key)
    }

    // ==================== 애니메이션 (AnimationSystem 위임) ====================

    /// 애니메이션 재생
    pub fn play_animation(&mut self, animation: ActiveAnimation) {
        self.animation.play(animation);
    }

    /// 애니메이션 중지
    pub fn stop_animation(&mut self, widget_id: &str, animation_name: Option<&str>) {
        self.animation.stop(widget_id, animation_name);
    }

    /// 모든 애니메이션 중지
    pub fn stop_all_animations(&mut self) {
        self.animation.stop_all();
    }

    // ==================== 위젯 상태 (StateManager 위임) ====================

    /// 위젯 상태 변경
    pub fn set_widget_state(&mut self, widget_id: &str, state: &str) {
        if let Some(ref mut root) = self.root {
            StateManager::set_state(root, widget_id, state);
        }
    }

    /// 위젯 가시성 설정
    pub fn set_visible(&mut self, widget_id: &str, visible: bool) {
        if let Some(ref mut root) = self.root {
            StateManager::set_visible(root, widget_id, visible);
        }
    }

    /// 위젯 가시성 가져오기
    pub fn get_visible(&self, widget_id: &str) -> Option<bool> {
        if let Some(ref root) = self.root {
            StateManager::get_visible(root, widget_id)
        } else {
            None
        }
    }

    // ==================== 스크롤 (LayoutSystem 위임) ====================

    /// 스크롤 위치 설정
    pub fn set_scroll_offset(&mut self, widget_id: &str, x: f32, y: f32) {
        if let Some(ref mut root) = self.root {
            self.layout.set_scroll_offset(root, widget_id, x, y);
        }
    }

    /// 스크롤 위치 가져오기
    pub fn get_scroll_offset(&self, widget_id: &str) -> Option<(f32, f32)> {
        if let Some(ref root) = self.root {
            self.layout.get_scroll_offset(root, widget_id)
        } else {
            None
        }
    }

    // ==================== 호환성을 위한 별칭 ====================

    /// 바인딩 컨텍스트 접근 (호환성)
    pub fn binding_context(&self) -> &crate::binding::BindingContext {
        self.binding.context()
    }

    /// 바인딩 컨텍스트 가변 접근 (호환성)
    pub fn binding_context_mut(&mut self) -> &mut crate::binding::BindingContext {
        self.binding.context_mut()
    }

    /// config 접근 (호환성)
    pub fn config(&self) -> &UiConfig {
        &self.layout.config
    }

    /// screen_size 접근 (호환성)
    pub fn screen_size(&self) -> (f32, f32) {
        self.layout.screen_size
    }

}

// ==================== 호환성을 위한 필드 접근 ====================

impl UiSystem {
    /// mouse_pos 필드 접근 (호환성)
    #[inline]
    pub fn get_mouse_pos(&self) -> (f32, f32) {
        self.input.mouse_pos
    }
}

impl Default for UiSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Helper Functions ====================

fn find_widget_by_id<'a>(widget: &'a Widget, id: &str) -> Option<&'a Widget> {
    if widget.id.as_deref() == Some(id) {
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
    if widget.id.as_deref() == Some(id) {
        return Some(widget);
    }
    for child in &mut widget.children {
        if let Some(found) = find_widget_by_id_mut(child, id) {
            return Some(found);
        }
    }
    None
}
