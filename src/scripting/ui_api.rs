//! SKOPE.UI Lua API
//!
//! 게임 UI 시스템을 Lua 스크립트에서 제어하기 위한 API

#![allow(dead_code)]

use mlua::{Lua, Result as LuaResult, Table, Value, Function};
use super::ui_commands::{UiCommand, LuaBindingValue, UiEventType};

/// SKOPE.UI API 등록
pub fn register_ui(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let ui = lua.create_table()?;

    // ============================================================
    // 내부 상태 테이블
    // ============================================================

    // 위젯 레지스트리 (Rust에서 동기화)
    let widget_registry = lua.create_table()?;
    ui.set("_widgets", widget_registry)?;

    // 커맨드 큐
    let command_queue = lua.create_table()?;
    ui.set("_command_queue", command_queue)?;

    // 이벤트 핸들러 테이블
    let handlers = lua.create_table()?;
    handlers.set("click", lua.create_table()?)?;
    handlers.set("hover", lua.create_table()?)?;
    handlers.set("hover_end", lua.create_table()?)?;
    handlers.set("focus", lua.create_table()?)?;
    handlers.set("blur", lua.create_table()?)?;
    handlers.set("value_changed", lua.create_table()?)?;
    handlers.set("drag_start", lua.create_table()?)?;
    handlers.set("drag_end", lua.create_table()?)?;
    handlers.set("drop", lua.create_table()?)?;
    ui.set("_handlers", handlers)?;

    // UI 상태
    let state = lua.create_table()?;
    state.set("mouse_over_ui", false)?;
    state.set("hovered_widget", Value::Nil)?;
    state.set("focused_widget", Value::Nil)?;
    state.set("is_dragging", false)?;
    ui.set("_state", state)?;

    // ============================================================
    // 위젯 조회 함수 (읽기 전용)
    // ============================================================

    // UI.exists(widget_id) -> boolean
    ui.set("exists", lua.create_function(|lua, widget_id: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let widgets: Table = ui.get("_widgets")?;
        Ok(widgets.contains_key(widget_id)?)
    })?)?;

    // UI.get_visible(widget_id) -> boolean
    ui.set("get_visible", lua.create_function(|lua, widget_id: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let widgets: Table = ui.get("_widgets")?;
        if let Ok(widget) = widgets.get::<Table>(widget_id) {
            Ok(widget.get::<bool>("visible").unwrap_or(true))
        } else {
            Ok(true)
        }
    })?)?;

    // UI.get_text(widget_id) -> string
    ui.set("get_text", lua.create_function(|lua, widget_id: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let widgets: Table = ui.get("_widgets")?;
        if let Ok(widget) = widgets.get::<Table>(widget_id) {
            Ok(widget.get::<String>("text").unwrap_or_default())
        } else {
            Ok(String::new())
        }
    })?)?;

    // UI.get_rect(widget_id) -> { x, y, width, height }
    ui.set("get_rect", lua.create_function(|lua, widget_id: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let widgets: Table = ui.get("_widgets")?;
        let result = lua.create_table()?;
        if let Ok(widget) = widgets.get::<Table>(widget_id) {
            result.set("x", widget.get::<f32>("x").unwrap_or(0.0))?;
            result.set("y", widget.get::<f32>("y").unwrap_or(0.0))?;
            result.set("width", widget.get::<f32>("width").unwrap_or(0.0))?;
            result.set("height", widget.get::<f32>("height").unwrap_or(0.0))?;
        }
        Ok(result)
    })?)?;

    // UI.get_progress(widget_id) -> value, max
    ui.set("get_progress", lua.create_function(|lua, widget_id: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let widgets: Table = ui.get("_widgets")?;
        if let Ok(widget) = widgets.get::<Table>(widget_id) {
            let value = widget.get::<f32>("progress_value").unwrap_or(0.0);
            let max = widget.get::<f32>("progress_max").unwrap_or(100.0);
            Ok((value, max))
        } else {
            Ok((0.0, 100.0))
        }
    })?)?;

    // UI.get_input_value(widget_id) -> string
    ui.set("get_input_value", lua.create_function(|lua, widget_id: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let widgets: Table = ui.get("_widgets")?;
        if let Ok(widget) = widgets.get::<Table>(widget_id) {
            Ok(widget.get::<String>("input_value").unwrap_or_default())
        } else {
            Ok(String::new())
        }
    })?)?;

    // ============================================================
    // UI 상태 조회
    // ============================================================

    // UI.is_mouse_over_ui() -> boolean
    ui.set("is_mouse_over_ui", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let state: Table = ui.get("_state")?;
        Ok(state.get::<bool>("mouse_over_ui").unwrap_or(false))
    })?)?;

    // UI.get_hovered_widget() -> widget_id or nil
    ui.set("get_hovered_widget", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let state: Table = ui.get("_state")?;
        Ok(state.get::<Value>("hovered_widget")?)
    })?)?;

    // UI.get_focused_widget() -> widget_id or nil
    ui.set("get_focused_widget", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let state: Table = ui.get("_state")?;
        Ok(state.get::<Value>("focused_widget")?)
    })?)?;

    // UI.is_dragging() -> boolean
    ui.set("is_dragging", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let state: Table = ui.get("_state")?;
        Ok(state.get::<bool>("is_dragging").unwrap_or(false))
    })?)?;

    // ============================================================
    // 속성 변경 함수 (커맨드 큐)
    // ============================================================

    // UI.set_visible(widget_id, visible)
    ui.set("set_visible", lua.create_function(|lua, (widget_id, visible): (String, bool)| {
        push_command(lua, "set_visible", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("visible", visible)?;
            Ok(())
        })
    })?)?;

    // UI.set_text(widget_id, text)
    ui.set("set_text", lua.create_function(|lua, (widget_id, text): (String, String)| {
        push_command(lua, "set_text", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("text", text)?;
            Ok(())
        })
    })?)?;

    // UI.set_progress(widget_id, value, max)
    ui.set("set_progress", lua.create_function(|lua, (widget_id, value, max): (String, f32, f32)| {
        push_command(lua, "set_progress", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("value", value)?;
            cmd.set("max_value", max)?;
            Ok(())
        })
    })?)?;

    // UI.set_input_value(widget_id, value)
    ui.set("set_input_value", lua.create_function(|lua, (widget_id, value): (String, String)| {
        push_command(lua, "set_input_value", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("value", value)?;
            Ok(())
        })
    })?)?;

    // UI.set_opacity(widget_id, opacity)
    ui.set("set_opacity", lua.create_function(|lua, (widget_id, opacity): (String, f32)| {
        push_command(lua, "set_opacity", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("opacity", opacity)?;
            Ok(())
        })
    })?)?;

    // UI.set_tooltip(widget_id, text)
    ui.set("set_tooltip", lua.create_function(|lua, (widget_id, text): (String, Option<String>)| {
        push_command(lua, "set_tooltip", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("text", text)?;
            Ok(())
        })
    })?)?;

    // UI.set_interactive(widget_id, interactive)
    ui.set("set_interactive", lua.create_function(|lua, (widget_id, interactive): (String, bool)| {
        push_command(lua, "set_interactive", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("interactive", interactive)?;
            Ok(())
        })
    })?)?;

    // UI.set_state(widget_id, state)
    ui.set("set_state", lua.create_function(|lua, (widget_id, state): (String, String)| {
        push_command(lua, "set_state", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("state", state)?;
            Ok(())
        })
    })?)?;

    // ============================================================
    // 데이터 바인딩
    // ============================================================

    // UI.set_binding(key, value)
    ui.set("set_binding", lua.create_function(|lua, (key, value): (String, Value)| {
        push_command(lua, "set_binding", |cmd| {
            cmd.set("key", key)?;
            cmd.set("value", value)?;
            Ok(())
        })
    })?)?;

    // UI.get_binding(key) -> value
    ui.set("get_binding", lua.create_function(|lua, key: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let bindings: Table = ui.get("_bindings").unwrap_or_else(|_| lua.create_table().unwrap());
        Ok(bindings.get::<Value>(key)?)
    })?)?;

    // 바인딩 저장소 초기화
    let bindings = lua.create_table()?;
    ui.set("_bindings", bindings)?;

    // ============================================================
    // 이벤트 핸들러 등록
    // ============================================================

    // UI.on_click(widget_id, callback)
    ui.set("on_click", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "click", &widget_id, callback)
    })?)?;

    // UI.on_hover(widget_id, callback)
    ui.set("on_hover", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "hover", &widget_id, callback)
    })?)?;

    // UI.on_hover_end(widget_id, callback)
    ui.set("on_hover_end", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "hover_end", &widget_id, callback)
    })?)?;

    // UI.on_focus(widget_id, callback)
    ui.set("on_focus", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "focus", &widget_id, callback)
    })?)?;

    // UI.on_blur(widget_id, callback)
    ui.set("on_blur", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "blur", &widget_id, callback)
    })?)?;

    // UI.on_value_changed(widget_id, callback)
    ui.set("on_value_changed", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "value_changed", &widget_id, callback)
    })?)?;

    // UI.on_drop(widget_id, callback)
    ui.set("on_drop", lua.create_function(|lua, (widget_id, callback): (String, Function)| {
        register_handler(lua, "drop", &widget_id, callback)
    })?)?;

    // UI.off(event_type, widget_id) - 핸들러 제거
    ui.set("off", lua.create_function(|lua, (event_type, widget_id): (String, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let ui: Table = skope.get("UI")?;
        let handlers: Table = ui.get("_handlers")?;
        if let Ok(event_handlers) = handlers.get::<Table>(event_type) {
            event_handlers.set(widget_id, Value::Nil)?;
        }
        Ok(())
    })?)?;

    // ============================================================
    // 애니메이션 (Phase 5)
    // ============================================================

    // UI.animate(widget_id, animation_name, params)
    ui.set("animate", lua.create_function(|lua, (widget_id, animation_name, params): (String, String, Option<Table>)| {
        push_command(lua, "play_animation", |cmd| {
            cmd.set("widget_id", widget_id)?;
            cmd.set("animation_name", animation_name)?;
            if let Some(p) = params {
                cmd.set("duration", p.get::<Option<f32>>("duration").unwrap_or(None))?;
            }
            Ok(())
        })
    })?)?;

    // UI.stop_animation(widget_id)
    ui.set("stop_animation", lua.create_function(|lua, widget_id: String| {
        push_command(lua, "stop_animation", |cmd| {
            cmd.set("widget_id", widget_id)?;
            Ok(())
        })
    })?)?;

    // ============================================================
    // SKOPE에 등록
    // ============================================================

    skope.set("UI", ui)?;

    Ok(())
}

/// 커맨드 큐에 커맨드 추가
fn push_command<F>(lua: &Lua, cmd_type: &str, setup: F) -> LuaResult<()>
where
    F: FnOnce(&Table) -> LuaResult<()>,
{
    let skope: Table = lua.globals().get("SKOPE")?;
    let ui: Table = skope.get("UI")?;
    let queue: Table = ui.get("_command_queue")?;

    let cmd = lua.create_table()?;
    cmd.set("type", cmd_type)?;
    setup(&cmd)?;

    let len = queue.len()? as i64;
    queue.set(len + 1, cmd)?;

    Ok(())
}

/// 이벤트 핸들러 등록
fn register_handler(lua: &Lua, event_type: &str, widget_id: &str, callback: Function) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let ui: Table = skope.get("UI")?;
    let handlers: Table = ui.get("_handlers")?;
    let event_handlers: Table = handlers.get(event_type)?;
    event_handlers.set(widget_id.to_string(), callback)?;
    Ok(())
}

// ============================================================
// Rust 측 함수 (커맨드 처리 및 상태 동기화)
// ============================================================

/// Lua 커맨드 큐에서 UI 커맨드 추출
pub fn process_ui_commands(lua: &Lua) -> LuaResult<Vec<UiCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let ui: Table = skope.get("UI")?;
    let queue: Table = ui.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "set_visible" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let visible: bool = cmd.get("visible")?;
                    Some(UiCommand::SetVisible { widget_id, visible })
                }
                "set_text" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let text: String = cmd.get("text")?;
                    Some(UiCommand::SetText { widget_id, text })
                }
                "set_progress" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let value: f32 = cmd.get("value")?;
                    let max_value: f32 = cmd.get("max_value")?;
                    Some(UiCommand::SetProgress { widget_id, value, max_value })
                }
                "set_input_value" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let value: String = cmd.get("value")?;
                    Some(UiCommand::SetInputValue { widget_id, value })
                }
                "set_opacity" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let opacity: f32 = cmd.get("opacity")?;
                    Some(UiCommand::SetOpacity { widget_id, opacity })
                }
                "set_tooltip" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let text: Option<String> = cmd.get("text").ok();
                    Some(UiCommand::SetTooltip { widget_id, text })
                }
                "set_interactive" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let interactive: bool = cmd.get("interactive")?;
                    Some(UiCommand::SetInteractive { widget_id, interactive })
                }
                "set_state" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let state: String = cmd.get("state")?;
                    Some(UiCommand::SetState { widget_id, state })
                }
                "set_binding" => {
                    let key: String = cmd.get("key")?;
                    let value: Value = cmd.get("value")?;
                    let binding_value = match value {
                        Value::String(s) => LuaBindingValue::String(s.to_str()?.to_string()),
                        Value::Integer(i) => LuaBindingValue::Number(i as f64),
                        Value::Number(n) => LuaBindingValue::Number(n),
                        Value::Boolean(b) => LuaBindingValue::Bool(b),
                        _ => LuaBindingValue::String(String::new()),
                    };
                    Some(UiCommand::SetBinding { key, value: binding_value })
                }
                "play_animation" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    let animation_name: String = cmd.get("animation_name")?;
                    let duration: Option<f32> = cmd.get("duration").ok();
                    Some(UiCommand::PlayAnimation { widget_id, animation_name, duration })
                }
                "stop_animation" => {
                    let widget_id: String = cmd.get("widget_id")?;
                    Some(UiCommand::StopAnimation { widget_id })
                }
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // 큐 초기화
    ui.set("_command_queue", lua.create_table()?)?;

    Ok(commands)
}

/// 위젯 정보를 Lua에 동기화
pub fn sync_widget_registry(lua: &Lua, widgets: &[(String, WidgetInfo)]) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let ui: Table = skope.get("UI")?;
    let registry: Table = ui.get("_widgets")?;

    // 기존 데이터 클리어
    for pair in registry.clone().pairs::<String, Value>() {
        if let Ok((key, _)) = pair {
            registry.set(key, Value::Nil)?;
        }
    }

    // 새 위젯 정보 설정
    for (id, info) in widgets {
        let widget_table = lua.create_table()?;
        widget_table.set("visible", info.visible)?;
        widget_table.set("x", info.x)?;
        widget_table.set("y", info.y)?;
        widget_table.set("width", info.width)?;
        widget_table.set("height", info.height)?;
        if let Some(ref text) = info.text {
            widget_table.set("text", text.clone())?;
        }
        if let Some((value, max)) = info.progress {
            widget_table.set("progress_value", value)?;
            widget_table.set("progress_max", max)?;
        }
        if let Some(ref input_value) = info.input_value {
            widget_table.set("input_value", input_value.clone())?;
        }
        registry.set(id.clone(), widget_table)?;
    }

    Ok(())
}

/// UI 상태를 Lua에 동기화
pub fn sync_ui_state(lua: &Lua, state: &UiState) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let ui: Table = skope.get("UI")?;
    let lua_state: Table = ui.get("_state")?;

    lua_state.set("mouse_over_ui", state.mouse_over_ui)?;
    if let Some(ref hovered) = state.hovered_widget {
        lua_state.set("hovered_widget", hovered.clone())?;
    } else {
        lua_state.set("hovered_widget", Value::Nil)?;
    }
    if let Some(ref focused) = state.focused_widget {
        lua_state.set("focused_widget", focused.clone())?;
    } else {
        lua_state.set("focused_widget", Value::Nil)?;
    }
    lua_state.set("is_dragging", state.is_dragging)?;

    Ok(())
}

/// UI 이벤트를 Lua 핸들러에 전달
pub fn dispatch_ui_event(lua: &Lua, event_type: &str, widget_id: &str, data: Option<&str>, source_id: Option<&str>) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let ui: Table = skope.get("UI")?;
    let handlers: Table = ui.get("_handlers")?;

    if let Ok(event_handlers) = handlers.get::<Table>(event_type) {
        if let Ok(callback) = event_handlers.get::<Function>(widget_id.to_string()) {
            // 이벤트 타입에 따라 다른 인자 전달
            match event_type {
                "drop" => {
                    // drop: callback(target_id, source_id, data)
                    let _ = callback.call::<()>((
                        widget_id.to_string(),
                        source_id.unwrap_or("").to_string(),
                        data.map(|s| s.to_string()),
                    ));
                }
                "value_changed" => {
                    // value_changed: callback(widget_id, value)
                    let _ = callback.call::<()>((
                        widget_id.to_string(),
                        data.unwrap_or("").to_string(),
                    ));
                }
                _ => {
                    // 기본: callback(widget_id)
                    let _ = callback.call::<()>(widget_id.to_string());
                }
            }
        }
    }

    Ok(())
}

/// 위젯 정보 구조체 (Rust → Lua 동기화용)
#[derive(Debug, Clone, Default)]
pub struct WidgetInfo {
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: Option<String>,
    pub progress: Option<(f32, f32)>,
    pub input_value: Option<String>,
}

/// UI 상태 구조체 (Rust → Lua 동기화용)
#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub mouse_over_ui: bool,
    pub hovered_widget: Option<String>,
    pub focused_widget: Option<String>,
    pub is_dragging: bool,
}
