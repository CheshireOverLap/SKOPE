//! UI Commands for Lua API
//!
//! Lua 스크립트에서 UI 시스템을 제어하기 위한 커맨드 정의

#![allow(dead_code)]

use skope_game_ui::BindingValue;

/// Lua에서 생성된 UI 커맨드 (Rust에서 처리)
#[derive(Debug, Clone)]
pub enum UiCommand {
    // ---- 가시성 ----
    SetVisible {
        widget_id: String,
        visible: bool,
    },

    // ---- 텍스트 ----
    SetText {
        widget_id: String,
        text: String,
    },

    // ---- 프로그레스 바 ----
    SetProgress {
        widget_id: String,
        value: f32,
        max_value: f32,
    },

    // ---- 입력 필드 ----
    SetInputValue {
        widget_id: String,
        value: String,
    },

    // ---- 스타일 ----
    SetOpacity {
        widget_id: String,
        opacity: f32,
    },

    SetBackgroundColor {
        widget_id: String,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },

    SetTextColor {
        widget_id: String,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },

    // ---- 툴팁 ----
    SetTooltip {
        widget_id: String,
        text: Option<String>,
    },

    // ---- 상호작용 ----
    SetInteractive {
        widget_id: String,
        interactive: bool,
    },

    SetDraggable {
        widget_id: String,
        draggable: bool,
    },

    SetDropTarget {
        widget_id: String,
        drop_target: bool,
    },

    // ---- 레이아웃 ----
    SetOffset {
        widget_id: String,
        x: f32,
        y: f32,
    },

    SetSize {
        widget_id: String,
        width: f32,
        height: f32,
    },

    // ---- 스크롤 ----
    SetScroll {
        widget_id: String,
        x: f32,
        y: f32,
    },

    // ---- 상태 ----
    SetState {
        widget_id: String,
        state: String,
    },

    // ---- 데이터 바인딩 ----
    SetBinding {
        key: String,
        value: LuaBindingValue,
    },

    ClearBinding {
        key: String,
    },

    // ---- 애니메이션 (Phase 5) ----
    PlayAnimation {
        widget_id: String,
        animation_name: String,
        duration: Option<f32>,
    },

    StopAnimation {
        widget_id: String,
    },

    // ---- 위젯 라이프사이클 (Phase 6) ----
    Create {
        definition: WidgetDefinition,
        parent_id: Option<String>,
    },

    Destroy {
        widget_id: String,
    },

    SetParent {
        widget_id: String,
        new_parent_id: String,
    },
}

/// Lua에서 전달되는 바인딩 값
#[derive(Debug, Clone)]
pub enum LuaBindingValue {
    String(String),
    Number(f64),
    Bool(bool),
}

impl From<LuaBindingValue> for BindingValue {
    fn from(val: LuaBindingValue) -> Self {
        match val {
            LuaBindingValue::String(s) => BindingValue::String(s),
            LuaBindingValue::Number(n) => BindingValue::Number(n),
            LuaBindingValue::Bool(b) => BindingValue::Bool(b),
        }
    }
}

/// 런타임 위젯 생성을 위한 정의
#[derive(Debug, Clone, Default)]
pub struct WidgetDefinition {
    pub id: Option<String>,
    pub widget_type: String,
    pub text: Option<String>,
    pub src: Option<String>,
    pub anchor: Option<String>,
    pub offset: Option<(f32, f32)>,
    pub size: Option<(f32, f32)>,
    pub background_color: Option<(f32, f32, f32, f32)>,
    pub text_color: Option<(f32, f32, f32, f32)>,
    pub visible: bool,
    pub interactive: bool,
}

impl WidgetDefinition {
    pub fn new() -> Self {
        Self {
            visible: true,
            interactive: true,
            ..Default::default()
        }
    }
}

/// UI 이벤트 타입 (Rust → Lua)
#[derive(Debug, Clone, PartialEq)]
pub enum UiEventType {
    Click,
    Hover,
    HoverEnd,
    Focus,
    Blur,
    ValueChanged,
    DragStart,
    DragEnd,
    Drop,
}

/// Lua로 전달할 UI 이벤트
#[derive(Debug, Clone)]
pub struct LuaUiEvent {
    pub event_type: UiEventType,
    pub widget_id: String,
    pub data: Option<String>,
    /// Drop 이벤트 시 소스 위젯 ID
    pub source_widget_id: Option<String>,
}

impl LuaUiEvent {
    pub fn click(widget_id: &str) -> Self {
        Self {
            event_type: UiEventType::Click,
            widget_id: widget_id.to_string(),
            data: None,
            source_widget_id: None,
        }
    }

    pub fn hover(widget_id: &str) -> Self {
        Self {
            event_type: UiEventType::Hover,
            widget_id: widget_id.to_string(),
            data: None,
            source_widget_id: None,
        }
    }

    pub fn hover_end(widget_id: &str) -> Self {
        Self {
            event_type: UiEventType::HoverEnd,
            widget_id: widget_id.to_string(),
            data: None,
            source_widget_id: None,
        }
    }

    pub fn value_changed(widget_id: &str, value: &str) -> Self {
        Self {
            event_type: UiEventType::ValueChanged,
            widget_id: widget_id.to_string(),
            data: Some(value.to_string()),
            source_widget_id: None,
        }
    }

    pub fn drop(target_id: &str, source_id: &str, data: Option<&str>) -> Self {
        Self {
            event_type: UiEventType::Drop,
            widget_id: target_id.to_string(),
            data: data.map(|s| s.to_string()),
            source_widget_id: Some(source_id.to_string()),
        }
    }
}
