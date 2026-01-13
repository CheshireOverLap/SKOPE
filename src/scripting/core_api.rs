//! Core API for Lua
//!
//! Input, Debug, Time, Transform API

use mlua::{Lua, Result as LuaResult, Table};

/// Core API 등록 (Input, Debug, Time, Transform)
pub fn register_core_apis(lua: &Lua, skope: &Table) -> LuaResult<()> {
    register_input(lua, skope)?;
    register_debug(lua, skope)?;
    register_time(lua, skope)?;
    register_transform(lua, skope)?;
    Ok(())
}

/// Input API (placeholder - 실제 구현은 main.rs에서 업데이트)
fn register_input(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let input_t = lua.create_table()?;

    // Input state table (will be updated from Rust)
    let state = lua.create_table()?;
    state.set("mouse_x", 0.0)?;
    state.set("mouse_y", 0.0)?;
    state.set("mouse_delta_x", 0.0)?;
    state.set("mouse_delta_y", 0.0)?;
    input_t.set("state", state)?;

    // Pressed keys table
    let keys = lua.create_table()?;
    input_t.set("keys", keys)?;

    // is_key_pressed
    input_t.set("is_key_pressed", lua.create_function(|lua, key: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let input: Table = skope.get("Input")?;
        let keys: Table = input.get("keys")?;
        let pressed: bool = keys.get(key).unwrap_or(false);
        Ok(pressed)
    })?)?;

    // get_mouse_position
    input_t.set("get_mouse_position", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let input: Table = skope.get("Input")?;
        let state: Table = input.get("state")?;
        let x: f32 = state.get("mouse_x")?;
        let y: f32 = state.get("mouse_y")?;
        let pos = lua.create_table()?;
        pos.set("x", x)?;
        pos.set("y", y)?;
        Ok(pos)
    })?)?;

    // get_mouse_delta
    input_t.set("get_mouse_delta", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let input: Table = skope.get("Input")?;
        let state: Table = input.get("state")?;
        let dx: f32 = state.get("mouse_delta_x")?;
        let dy: f32 = state.get("mouse_delta_y")?;
        let delta = lua.create_table()?;
        delta.set("x", dx)?;
        delta.set("y", dy)?;
        Ok(delta)
    })?)?;

    // get_axis (WASD -> -1 to 1)
    input_t.set("get_axis", lua.create_function(|lua, axis: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let input: Table = skope.get("Input")?;
        let keys: Table = input.get("keys")?;

        let value = match axis.as_str() {
            "horizontal" => {
                let left: bool = keys.get("A").unwrap_or(false) || keys.get("Left").unwrap_or(false);
                let right: bool = keys.get("D").unwrap_or(false) || keys.get("Right").unwrap_or(false);
                (if right { 1.0 } else { 0.0 }) - (if left { 1.0 } else { 0.0 })
            }
            "vertical" => {
                let down: bool = keys.get("S").unwrap_or(false) || keys.get("Down").unwrap_or(false);
                let up: bool = keys.get("W").unwrap_or(false) || keys.get("Up").unwrap_or(false);
                (if up { 1.0 } else { 0.0 }) - (if down { 1.0 } else { 0.0 })
            }
            _ => 0.0
        };

        Ok(value)
    })?)?;

    skope.set("Input", input_t)?;
    Ok(())
}

/// Debug API
fn register_debug(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let debug_t = lua.create_table()?;

    // 디버그 드로우 큐 (Rust에서 읽어감)
    let draw_queue = lua.create_table()?;
    debug_t.set("_draw_queue", draw_queue)?;

    // log
    debug_t.set("log", lua.create_function(|_, msg: String| {
        log::debug!("[Lua:Debug] {}", msg);
        Ok(())
    })?)?;

    // warn
    debug_t.set("warn", lua.create_function(|_, msg: String| {
        log::debug!("[Lua:Warn] {}", msg);
        Ok(())
    })?)?;

    // error
    debug_t.set("error", lua.create_function(|_, msg: String| {
        log::debug!("[Lua:Error] {}", msg);
        Ok(())
    })?)?;

    // draw_line(from, to, color) - 라인 그리기
    debug_t.set("draw_line", lua.create_function(|lua, (from, to, color): (Table, Table, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let debug: Table = skope.get("Debug")?;
        let queue: Table = debug.get("_draw_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "line")?;
        cmd.set("from_x", from.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("from_y", from.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("from_z", from.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("to_x", to.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("to_y", to.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("to_z", to.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("r", color.get::<f32>("r").unwrap_or(1.0))?;
        cmd.set("g", color.get::<f32>("g").unwrap_or(1.0))?;
        cmd.set("b", color.get::<f32>("b").unwrap_or(1.0))?;
        cmd.set("a", color.get::<f32>("a").unwrap_or(1.0))?;

        let len = queue.len()? + 1;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    // draw_sphere(center, radius, color) - 구 그리기
    debug_t.set("draw_sphere", lua.create_function(|lua, (center, radius, color): (Table, f32, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let debug: Table = skope.get("Debug")?;
        let queue: Table = debug.get("_draw_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "sphere")?;
        cmd.set("x", center.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", center.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", center.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("radius", radius)?;
        cmd.set("r", color.get::<f32>("r").unwrap_or(1.0))?;
        cmd.set("g", color.get::<f32>("g").unwrap_or(1.0))?;
        cmd.set("b", color.get::<f32>("b").unwrap_or(1.0))?;
        cmd.set("a", color.get::<f32>("a").unwrap_or(1.0))?;

        let len = queue.len()? + 1;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    // draw_box(min, max, color) - 박스 그리기
    debug_t.set("draw_box", lua.create_function(|lua, (min, max, color): (Table, Table, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let debug: Table = skope.get("Debug")?;
        let queue: Table = debug.get("_draw_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "box")?;
        cmd.set("min_x", min.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("min_y", min.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("min_z", min.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("max_x", max.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("max_y", max.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("max_z", max.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("r", color.get::<f32>("r").unwrap_or(1.0))?;
        cmd.set("g", color.get::<f32>("g").unwrap_or(1.0))?;
        cmd.set("b", color.get::<f32>("b").unwrap_or(1.0))?;
        cmd.set("a", color.get::<f32>("a").unwrap_or(1.0))?;

        let len = queue.len()? + 1;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    // draw_point(position, color, size) - 점 그리기
    debug_t.set("draw_point", lua.create_function(|lua, (pos, color, size): (Table, Table, Option<f32>)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let debug: Table = skope.get("Debug")?;
        let queue: Table = debug.get("_draw_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "point")?;
        cmd.set("x", pos.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", pos.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", pos.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("size", size.unwrap_or(0.1))?;
        cmd.set("r", color.get::<f32>("r").unwrap_or(1.0))?;
        cmd.set("g", color.get::<f32>("g").unwrap_or(1.0))?;
        cmd.set("b", color.get::<f32>("b").unwrap_or(1.0))?;
        cmd.set("a", color.get::<f32>("a").unwrap_or(1.0))?;

        let len = queue.len()? + 1;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    // draw_axis(position, size) - 축 기즈모 그리기
    debug_t.set("draw_axis", lua.create_function(|lua, (pos, size): (Table, Option<f32>)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let debug: Table = skope.get("Debug")?;
        let queue: Table = debug.get("_draw_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "axis")?;
        cmd.set("x", pos.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", pos.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", pos.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("size", size.unwrap_or(1.0))?;

        let len = queue.len()? + 1;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    skope.set("Debug", debug_t)?;
    Ok(())
}

/// Time API
fn register_time(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let time_t = lua.create_table()?;

    // Time values (updated from Rust each frame)
    time_t.set("delta", 0.016)?;
    time_t.set("elapsed", 0.0)?;
    time_t.set("frame_count", 0)?;
    time_t.set("fps", 60.0)?;

    skope.set("Time", time_t)?;
    Ok(())
}

/// Input 상태 업데이트 (매 프레임 Rust에서 호출)
pub fn update_input_state(lua: &Lua, mouse_x: f32, mouse_y: f32, delta_x: f32, delta_y: f32) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let input: Table = skope.get("Input")?;
    let state: Table = input.get("state")?;

    state.set("mouse_x", mouse_x)?;
    state.set("mouse_y", mouse_y)?;
    state.set("mouse_delta_x", delta_x)?;
    state.set("mouse_delta_y", delta_y)?;

    Ok(())
}

/// 키 상태 업데이트
pub fn update_key_state(lua: &Lua, key: &str, pressed: bool) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let input: Table = skope.get("Input")?;
    let keys: Table = input.get("keys")?;

    keys.set(key, pressed)?;

    Ok(())
}

/// Time 업데이트
pub fn update_time(lua: &Lua, delta: f32, elapsed: f32, frame_count: u64, fps: f32) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let time: Table = skope.get("Time")?;

    time.set("delta", delta)?;
    time.set("elapsed", elapsed)?;
    time.set("frame_count", frame_count)?;
    time.set("fps", fps)?;

    Ok(())
}

/// Transform API - 엔티티 Transform 조작
fn register_transform(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let transform_t = lua.create_table()?;

    // Transform.new(pos, rot, scale) - Transform 생성
    transform_t.set("new", lua.create_function(|lua, (pos, rot, scale): (Option<Table>, Option<Table>, Option<Table>)| {
        let t = lua.create_table()?;

        // Position
        if let Some(p) = pos {
            let pos_t = lua.create_table()?;
            pos_t.set("x", p.get::<f32>("x").unwrap_or(0.0))?;
            pos_t.set("y", p.get::<f32>("y").unwrap_or(0.0))?;
            pos_t.set("z", p.get::<f32>("z").unwrap_or(0.0))?;
            t.set("position", pos_t)?;
        } else {
            let pos_t = lua.create_table()?;
            pos_t.set("x", 0.0)?;
            pos_t.set("y", 0.0)?;
            pos_t.set("z", 0.0)?;
            t.set("position", pos_t)?;
        }

        // Rotation
        if let Some(r) = rot {
            let rot_t = lua.create_table()?;
            rot_t.set("x", r.get::<f32>("x").unwrap_or(0.0))?;
            rot_t.set("y", r.get::<f32>("y").unwrap_or(0.0))?;
            rot_t.set("z", r.get::<f32>("z").unwrap_or(0.0))?;
            rot_t.set("w", r.get::<f32>("w").unwrap_or(1.0))?;
            t.set("rotation", rot_t)?;
        } else {
            let rot_t = lua.create_table()?;
            rot_t.set("x", 0.0)?;
            rot_t.set("y", 0.0)?;
            rot_t.set("z", 0.0)?;
            rot_t.set("w", 1.0)?;
            t.set("rotation", rot_t)?;
        }

        // Scale
        if let Some(s) = scale {
            let scale_t = lua.create_table()?;
            scale_t.set("x", s.get::<f32>("x").unwrap_or(1.0))?;
            scale_t.set("y", s.get::<f32>("y").unwrap_or(1.0))?;
            scale_t.set("z", s.get::<f32>("z").unwrap_or(1.0))?;
            t.set("scale", scale_t)?;
        } else {
            let scale_t = lua.create_table()?;
            scale_t.set("x", 1.0)?;
            scale_t.set("y", 1.0)?;
            scale_t.set("z", 1.0)?;
            t.set("scale", scale_t)?;
        }

        Ok(t)
    })?)?;

    // Transform.identity() - 단위 Transform
    transform_t.set("identity", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let pos = lua.create_table()?;
        pos.set("x", 0.0)?;
        pos.set("y", 0.0)?;
        pos.set("z", 0.0)?;
        t.set("position", pos)?;

        let rot = lua.create_table()?;
        rot.set("x", 0.0)?;
        rot.set("y", 0.0)?;
        rot.set("z", 0.0)?;
        rot.set("w", 1.0)?;
        t.set("rotation", rot)?;

        let scale = lua.create_table()?;
        scale.set("x", 1.0)?;
        scale.set("y", 1.0)?;
        scale.set("z", 1.0)?;
        t.set("scale", scale)?;

        Ok(t)
    })?)?;

    skope.set("Transform", transform_t)?;
    Ok(())
}
