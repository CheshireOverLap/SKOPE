// SKOPE Scripting API
// Lua에서 사용 가능한 엔진 API
#![allow(dead_code)]

use mlua::{Lua, Result as LuaResult, Table};

/// 모든 API 등록
pub fn register_all(lua: &Lua) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;

    // Vec3 API
    register_vec3(lua, &skope)?;

    // Math API
    register_math(lua, &skope)?;

    // Input API (placeholder)
    register_input(lua, &skope)?;

    // Debug API
    register_debug(lua, &skope)?;

    // Time API
    register_time(lua, &skope)?;

    Ok(())
}

/// Vec3 API
fn register_vec3(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let vec3_mt = lua.create_table()?;

    // Vec3.new(x, y, z)
    vec3_mt.set("new", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let t = lua.create_table()?;
        t.set("x", x)?;
        t.set("y", y)?;
        t.set("z", z)?;
        Ok(t)
    })?)?;

    // Vec3.zero()
    vec3_mt.set("zero", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 0.0)?;
        Ok(t)
    })?)?;

    // Vec3.one()
    vec3_mt.set("one", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 1.0)?;
        t.set("y", 1.0)?;
        t.set("z", 1.0)?;
        Ok(t)
    })?)?;

    // Vec3.up()
    vec3_mt.set("up", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 1.0)?;
        t.set("z", 0.0)?;
        Ok(t)
    })?)?;

    // Vec3.forward()
    vec3_mt.set("forward", lua.create_function(|lua, ()| {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", -1.0)?;
        Ok(t)
    })?)?;

    // Vec3.add(a, b)
    vec3_mt.set("add", lua.create_function(|lua, (a, b): (Table, Table)| {
        let t = lua.create_table()?;
        t.set("x", a.get::<f32>("x")? + b.get::<f32>("x")?)?;
        t.set("y", a.get::<f32>("y")? + b.get::<f32>("y")?)?;
        t.set("z", a.get::<f32>("z")? + b.get::<f32>("z")?)?;
        Ok(t)
    })?)?;

    // Vec3.sub(a, b)
    vec3_mt.set("sub", lua.create_function(|lua, (a, b): (Table, Table)| {
        let t = lua.create_table()?;
        t.set("x", a.get::<f32>("x")? - b.get::<f32>("x")?)?;
        t.set("y", a.get::<f32>("y")? - b.get::<f32>("y")?)?;
        t.set("z", a.get::<f32>("z")? - b.get::<f32>("z")?)?;
        Ok(t)
    })?)?;

    // Vec3.mul(v, scalar)
    vec3_mt.set("mul", lua.create_function(|lua, (v, s): (Table, f32)| {
        let t = lua.create_table()?;
        t.set("x", v.get::<f32>("x")? * s)?;
        t.set("y", v.get::<f32>("y")? * s)?;
        t.set("z", v.get::<f32>("z")? * s)?;
        Ok(t)
    })?)?;

    // Vec3.length(v)
    vec3_mt.set("length", lua.create_function(|_, v: Table| {
        let x: f32 = v.get("x")?;
        let y: f32 = v.get("y")?;
        let z: f32 = v.get("z")?;
        Ok((x * x + y * y + z * z).sqrt())
    })?)?;

    // Vec3.normalize(v)
    vec3_mt.set("normalize", lua.create_function(|lua, v: Table| {
        let x: f32 = v.get("x")?;
        let y: f32 = v.get("y")?;
        let z: f32 = v.get("z")?;
        let len = (x * x + y * y + z * z).sqrt();
        let t = lua.create_table()?;
        if len > 0.0001 {
            t.set("x", x / len)?;
            t.set("y", y / len)?;
            t.set("z", z / len)?;
        } else {
            t.set("x", 0.0)?;
            t.set("y", 0.0)?;
            t.set("z", 0.0)?;
        }
        Ok(t)
    })?)?;

    // Vec3.dot(a, b)
    vec3_mt.set("dot", lua.create_function(|_, (a, b): (Table, Table)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        Ok(ax * bx + ay * by + az * bz)
    })?)?;

    // Vec3.cross(a, b)
    vec3_mt.set("cross", lua.create_function(|lua, (a, b): (Table, Table)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        let t = lua.create_table()?;
        t.set("x", ay * bz - az * by)?;
        t.set("y", az * bx - ax * bz)?;
        t.set("z", ax * by - ay * bx)?;
        Ok(t)
    })?)?;

    // Vec3.lerp(a, b, t)
    vec3_mt.set("lerp", lua.create_function(|lua, (a, b, t): (Table, Table, f32)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        let result = lua.create_table()?;
        result.set("x", ax + (bx - ax) * t)?;
        result.set("y", ay + (by - ay) * t)?;
        result.set("z", az + (bz - az) * t)?;
        Ok(result)
    })?)?;

    // Vec3.distance(a, b)
    vec3_mt.set("distance", lua.create_function(|_, (a, b): (Table, Table)| {
        let dx = a.get::<f32>("x")? - b.get::<f32>("x")?;
        let dy = a.get::<f32>("y")? - b.get::<f32>("y")?;
        let dz = a.get::<f32>("z")? - b.get::<f32>("z")?;
        Ok((dx * dx + dy * dy + dz * dz).sqrt())
    })?)?;

    skope.set("Vec3", vec3_mt)?;
    Ok(())
}

/// Math API
fn register_math(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let math_t = lua.create_table()?;

    // 상수
    math_t.set("PI", std::f32::consts::PI)?;
    math_t.set("TAU", std::f32::consts::TAU)?;
    math_t.set("E", std::f32::consts::E)?;

    // clamp
    math_t.set("clamp", lua.create_function(|_, (v, min, max): (f32, f32, f32)| {
        Ok(v.clamp(min, max))
    })?)?;

    // lerp
    math_t.set("lerp", lua.create_function(|_, (a, b, t): (f32, f32, f32)| {
        Ok(a + (b - a) * t)
    })?)?;

    // smoothstep
    math_t.set("smoothstep", lua.create_function(|_, (edge0, edge1, x): (f32, f32, f32)| {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        Ok(t * t * (3.0 - 2.0 * t))
    })?)?;

    // deg_to_rad
    math_t.set("deg_to_rad", lua.create_function(|_, deg: f32| {
        Ok(deg.to_radians())
    })?)?;

    // rad_to_deg
    math_t.set("rad_to_deg", lua.create_function(|_, rad: f32| {
        Ok(rad.to_degrees())
    })?)?;

    // sign
    math_t.set("sign", lua.create_function(|_, v: f32| {
        Ok(if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 })
    })?)?;

    skope.set("Math", math_t)?;
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

    // log
    debug_t.set("log", lua.create_function(|_, msg: String| {
        println!("[Lua:Debug] {}", msg);
        Ok(())
    })?)?;

    // warn
    debug_t.set("warn", lua.create_function(|_, msg: String| {
        println!("[Lua:Warn] {}", msg);
        Ok(())
    })?)?;

    // error
    debug_t.set("error", lua.create_function(|_, msg: String| {
        eprintln!("[Lua:Error] {}", msg);
        Ok(())
    })?)?;

    // draw_line (placeholder)
    debug_t.set("draw_line", lua.create_function(|_, (_from, _to, _color): (Table, Table, Table)| {
        // TODO: 실제 디버그 라인 렌더링
        Ok(())
    })?)?;

    // draw_sphere (placeholder)
    debug_t.set("draw_sphere", lua.create_function(|_, (_center, _radius, _color): (Table, f32, Table)| {
        // TODO: 실제 디버그 구 렌더링
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
