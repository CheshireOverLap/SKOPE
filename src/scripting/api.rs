// SKOPE Scripting API
// Lua에서 사용 가능한 엔진 API
#![allow(dead_code)]

use mlua::{Lua, Result as LuaResult, Table};

/// 모든 API 등록
pub fn register_all(lua: &Lua) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;

    // Vec3 API
    register_vec3(lua, &skope)?;

    // Quat API
    register_quat(lua, &skope)?;

    // Math API
    register_math(lua, &skope)?;

    // Input API (placeholder)
    register_input(lua, &skope)?;

    // Debug API
    register_debug(lua, &skope)?;

    // Time API
    register_time(lua, &skope)?;

    // Transform API
    register_transform(lua, &skope)?;

    // Entity API
    register_entity(lua, &skope)?;

    // Audio API
    register_audio(lua, &skope)?;

    // Collision API
    register_collision(lua, &skope)?;

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

/// Quat (Quaternion) API
fn register_quat(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let quat_t = lua.create_table()?;

    // Quat.identity() - 단위 쿼터니언
    quat_t.set("identity", lua.create_function(|lua, ()| {
        let q = lua.create_table()?;
        q.set("x", 0.0)?;
        q.set("y", 0.0)?;
        q.set("z", 0.0)?;
        q.set("w", 1.0)?;
        Ok(q)
    })?)?;

    // Quat.from_axis_angle(axis, angle) - 축-각도로 쿼터니언 생성
    quat_t.set("from_axis_angle", lua.create_function(|lua, (axis, angle): (Table, f32)| {
        let ax: f32 = axis.get("x").unwrap_or(0.0);
        let ay: f32 = axis.get("y").unwrap_or(1.0);
        let az: f32 = axis.get("z").unwrap_or(0.0);

        // 축 정규화
        let len = (ax * ax + ay * ay + az * az).sqrt();
        let (ax, ay, az) = if len > 0.0001 {
            (ax / len, ay / len, az / len)
        } else {
            (0.0, 1.0, 0.0)
        };

        let half_angle = angle * 0.5;
        let s = half_angle.sin();
        let c = half_angle.cos();

        let q = lua.create_table()?;
        q.set("x", ax * s)?;
        q.set("y", ay * s)?;
        q.set("z", az * s)?;
        q.set("w", c)?;
        Ok(q)
    })?)?;

    // Quat.from_euler(x, y, z) - 오일러 각도로 쿼터니언 생성 (라디안)
    quat_t.set("from_euler", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let (sx, cx) = (x * 0.5).sin_cos();
        let (sy, cy) = (y * 0.5).sin_cos();
        let (sz, cz) = (z * 0.5).sin_cos();

        let q = lua.create_table()?;
        q.set("x", sx * cy * cz - cx * sy * sz)?;
        q.set("y", cx * sy * cz + sx * cy * sz)?;
        q.set("z", cx * cy * sz - sx * sy * cz)?;
        q.set("w", cx * cy * cz + sx * sy * sz)?;
        Ok(q)
    })?)?;

    // Quat.from_euler_deg(x, y, z) - 오일러 각도로 쿼터니언 생성 (도)
    quat_t.set("from_euler_deg", lua.create_function(|lua, (x, y, z): (f32, f32, f32)| {
        let x = x.to_radians();
        let y = y.to_radians();
        let z = z.to_radians();

        let (sx, cx) = (x * 0.5).sin_cos();
        let (sy, cy) = (y * 0.5).sin_cos();
        let (sz, cz) = (z * 0.5).sin_cos();

        let q = lua.create_table()?;
        q.set("x", sx * cy * cz - cx * sy * sz)?;
        q.set("y", cx * sy * cz + sx * cy * sz)?;
        q.set("z", cx * cy * sz - sx * sy * cz)?;
        q.set("w", cx * cy * cz + sx * sy * sz)?;
        Ok(q)
    })?)?;

    // Quat.mul(a, b) - 쿼터니언 곱셈
    quat_t.set("mul", lua.create_function(|lua, (a, b): (Table, Table)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let aw: f32 = a.get("w")?;
        let bx: f32 = b.get("x")?;
        let by: f32 = b.get("y")?;
        let bz: f32 = b.get("z")?;
        let bw: f32 = b.get("w")?;

        let q = lua.create_table()?;
        q.set("x", aw * bx + ax * bw + ay * bz - az * by)?;
        q.set("y", aw * by - ax * bz + ay * bw + az * bx)?;
        q.set("z", aw * bz + ax * by - ay * bx + az * bw)?;
        q.set("w", aw * bw - ax * bx - ay * by - az * bz)?;
        Ok(q)
    })?)?;

    // Quat.normalize(q) - 쿼터니언 정규화
    quat_t.set("normalize", lua.create_function(|lua, q: Table| {
        let x: f32 = q.get("x")?;
        let y: f32 = q.get("y")?;
        let z: f32 = q.get("z")?;
        let w: f32 = q.get("w")?;

        let len = (x * x + y * y + z * z + w * w).sqrt();
        let result = lua.create_table()?;
        if len > 0.0001 {
            result.set("x", x / len)?;
            result.set("y", y / len)?;
            result.set("z", z / len)?;
            result.set("w", w / len)?;
        } else {
            result.set("x", 0.0)?;
            result.set("y", 0.0)?;
            result.set("z", 0.0)?;
            result.set("w", 1.0)?;
        }
        Ok(result)
    })?)?;

    // Quat.inverse(q) - 쿼터니언 역원
    quat_t.set("inverse", lua.create_function(|lua, q: Table| {
        let x: f32 = q.get("x")?;
        let y: f32 = q.get("y")?;
        let z: f32 = q.get("z")?;
        let w: f32 = q.get("w")?;

        let len_sq = x * x + y * y + z * z + w * w;
        let result = lua.create_table()?;
        if len_sq > 0.0001 {
            result.set("x", -x / len_sq)?;
            result.set("y", -y / len_sq)?;
            result.set("z", -z / len_sq)?;
            result.set("w", w / len_sq)?;
        } else {
            result.set("x", 0.0)?;
            result.set("y", 0.0)?;
            result.set("z", 0.0)?;
            result.set("w", 1.0)?;
        }
        Ok(result)
    })?)?;

    // Quat.slerp(a, b, t) - 구면 선형 보간
    quat_t.set("slerp", lua.create_function(|lua, (a, b, t): (Table, Table, f32)| {
        let ax: f32 = a.get("x")?;
        let ay: f32 = a.get("y")?;
        let az: f32 = a.get("z")?;
        let aw: f32 = a.get("w")?;
        let mut bx: f32 = b.get("x")?;
        let mut by: f32 = b.get("y")?;
        let mut bz: f32 = b.get("z")?;
        let mut bw: f32 = b.get("w")?;

        // 내적 계산
        let mut dot = ax * bx + ay * by + az * bz + aw * bw;

        // 최단 경로를 위해 필요시 부호 반전
        if dot < 0.0 {
            bx = -bx;
            by = -by;
            bz = -bz;
            bw = -bw;
            dot = -dot;
        }

        let result = lua.create_table()?;
        if dot > 0.9995 {
            // 거의 같은 경우 선형 보간
            result.set("x", ax + t * (bx - ax))?;
            result.set("y", ay + t * (by - ay))?;
            result.set("z", az + t * (bz - az))?;
            result.set("w", aw + t * (bw - aw))?;
        } else {
            let theta = dot.acos();
            let sin_theta = theta.sin();
            let wa = ((1.0 - t) * theta).sin() / sin_theta;
            let wb = (t * theta).sin() / sin_theta;

            result.set("x", wa * ax + wb * bx)?;
            result.set("y", wa * ay + wb * by)?;
            result.set("z", wa * az + wb * bz)?;
            result.set("w", wa * aw + wb * bw)?;
        }
        Ok(result)
    })?)?;

    // Quat.rotate_vec3(q, v) - 쿼터니언으로 벡터 회전
    quat_t.set("rotate_vec3", lua.create_function(|lua, (q, v): (Table, Table)| {
        let qx: f32 = q.get("x")?;
        let qy: f32 = q.get("y")?;
        let qz: f32 = q.get("z")?;
        let qw: f32 = q.get("w")?;
        let vx: f32 = v.get("x")?;
        let vy: f32 = v.get("y")?;
        let vz: f32 = v.get("z")?;

        // q * v * q^-1 계산 (최적화된 버전)
        let tx = 2.0 * (qy * vz - qz * vy);
        let ty = 2.0 * (qz * vx - qx * vz);
        let tz = 2.0 * (qx * vy - qy * vx);

        let result = lua.create_table()?;
        result.set("x", vx + qw * tx + qy * tz - qz * ty)?;
        result.set("y", vy + qw * ty + qz * tx - qx * tz)?;
        result.set("z", vz + qw * tz + qx * ty - qy * tx)?;
        Ok(result)
    })?)?;

    // Quat.look_at(forward, up) - 방향을 바라보는 쿼터니언 생성
    quat_t.set("look_at", lua.create_function(|lua, (forward, up): (Table, Table)| {
        let fx: f32 = forward.get("x")?;
        let fy: f32 = forward.get("y")?;
        let fz: f32 = forward.get("z")?;
        let ux: f32 = up.get("x").unwrap_or(0.0);
        let uy: f32 = up.get("y").unwrap_or(1.0);
        let uz: f32 = up.get("z").unwrap_or(0.0);

        // forward 정규화
        let f_len = (fx * fx + fy * fy + fz * fz).sqrt();
        let (fx, fy, fz) = if f_len > 0.0001 {
            (fx / f_len, fy / f_len, fz / f_len)
        } else {
            (0.0, 0.0, -1.0)
        };

        // right = up × forward
        let rx = uy * fz - uz * fy;
        let ry = uz * fx - ux * fz;
        let rz = ux * fy - uy * fx;
        let r_len = (rx * rx + ry * ry + rz * rz).sqrt();
        let (rx, ry, rz) = if r_len > 0.0001 {
            (rx / r_len, ry / r_len, rz / r_len)
        } else {
            (1.0, 0.0, 0.0)
        };

        // 실제 up = forward × right
        let ux = fy * rz - fz * ry;
        let uy = fz * rx - fx * rz;
        let uz = fx * ry - fy * rx;

        // 회전 행렬에서 쿼터니언 추출
        let trace = rx + uy + fz;
        let q = lua.create_table()?;

        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            q.set("w", 0.25 * s)?;
            q.set("x", (uz - fy) / s)?;
            q.set("y", (fx - rz) / s)?;
            q.set("z", (ry - ux) / s)?;
        } else if rx > uy && rx > fz {
            let s = (1.0 + rx - uy - fz).sqrt() * 2.0;
            q.set("w", (uz - fy) / s)?;
            q.set("x", 0.25 * s)?;
            q.set("y", (ux + ry) / s)?;
            q.set("z", (fx + rz) / s)?;
        } else if uy > fz {
            let s = (1.0 + uy - rx - fz).sqrt() * 2.0;
            q.set("w", (fx - rz) / s)?;
            q.set("x", (ux + ry) / s)?;
            q.set("y", 0.25 * s)?;
            q.set("z", (fy + uz) / s)?;
        } else {
            let s = (1.0 + fz - rx - uy).sqrt() * 2.0;
            q.set("w", (ry - ux) / s)?;
            q.set("x", (fx + rz) / s)?;
            q.set("y", (fy + uz) / s)?;
            q.set("z", 0.25 * s)?;
        }

        Ok(q)
    })?)?;

    skope.set("Quat", quat_t)?;
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

/// Entity API - 엔티티 조회 및 조작
fn register_entity(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let entity_t = lua.create_table()?;

    // Entity registry (Rust에서 채워짐)
    // 구조: { [entity_id] = { name = "...", transform = {...}, ... } }
    let registry = lua.create_table()?;
    entity_t.set("_registry", registry)?;

    // Name-to-ID lookup table
    let name_lookup = lua.create_table()?;
    entity_t.set("_name_lookup", name_lookup)?;

    // Entity.find(name) - 이름으로 엔티티 찾기
    entity_t.set("find", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let name_lookup: Table = entity.get("_name_lookup")?;

        let id: Option<u64> = name_lookup.get(name.clone()).ok();
        Ok(id)
    })?)?;

    // Entity.find_all(pattern) - 패턴으로 여러 엔티티 찾기
    entity_t.set("find_all", lua.create_function(|lua, pattern: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let name_lookup: Table = entity.get("_name_lookup")?;

        let results = lua.create_table()?;
        let mut idx = 1;

        for pair in name_lookup.pairs::<String, u64>() {
            if let Ok((name, id)) = pair {
                if name.contains(&pattern) {
                    results.set(idx, id)?;
                    idx += 1;
                }
            }
        }
        Ok(results)
    })?)?;

    // Entity.get_name(id) - 엔티티 이름 가져오기
    entity_t.set("get_name", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(id) {
            let name: Option<String> = entity_data.get("name").ok();
            Ok(name)
        } else {
            Ok(None)
        }
    })?)?;

    // Entity.get_transform(id) - Transform 컴포넌트 가져오기
    entity_t.set("get_transform", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(id) {
            let transform: Option<Table> = entity_data.get("transform").ok();
            Ok(transform)
        } else {
            Ok(None)
        }
    })?)?;

    // Entity.get_position(id) - 위치만 가져오기
    entity_t.set("get_position", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(id) {
            if let Ok(transform) = entity_data.get::<Table>("transform") {
                let pos: Option<Table> = transform.get("position").ok();
                return Ok(pos);
            }
        }
        Ok(None)
    })?)?;

    // Entity.get_rotation(id) - 회전만 가져오기
    entity_t.set("get_rotation", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(id) {
            if let Ok(transform) = entity_data.get::<Table>("transform") {
                let rot: Option<Table> = transform.get("rotation").ok();
                return Ok(rot);
            }
        }
        Ok(None)
    })?)?;

    // Entity.get_scale(id) - 스케일만 가져오기
    entity_t.set("get_scale", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(id) {
            if let Ok(transform) = entity_data.get::<Table>("transform") {
                let scale: Option<Table> = transform.get("scale").ok();
                return Ok(scale);
            }
        }
        Ok(None)
    })?)?;

    // Entity.distance(id1, id2) - 두 엔티티 사이 거리
    entity_t.set("distance", lua.create_function(|lua, (id1, id2): (u64, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        let get_pos = |id: u64| -> Option<(f32, f32, f32)> {
            if let Ok(entity_data) = registry.get::<Table>(id) {
                if let Ok(transform) = entity_data.get::<Table>("transform") {
                    if let Ok(pos) = transform.get::<Table>("position") {
                        let x: f32 = pos.get("x").unwrap_or(0.0);
                        let y: f32 = pos.get("y").unwrap_or(0.0);
                        let z: f32 = pos.get("z").unwrap_or(0.0);
                        return Some((x, y, z));
                    }
                }
            }
            None
        };

        if let (Some((x1, y1, z1)), Some((x2, y2, z2))) = (get_pos(id1), get_pos(id2)) {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let dz = z2 - z1;
            Ok(Some((dx * dx + dy * dy + dz * dz).sqrt()))
        } else {
            Ok(None)
        }
    })?)?;

    // Entity.get_all() - 모든 엔티티 ID 가져오기
    entity_t.set("get_all", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        let results = lua.create_table()?;
        let mut idx = 1;

        for pair in registry.pairs::<u64, Table>() {
            if let Ok((id, _)) = pair {
                results.set(idx, id)?;
                idx += 1;
            }
        }
        Ok(results)
    })?)?;

    // Entity.count() - 엔티티 개수
    entity_t.set("count", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        let mut count = 0;
        for _ in registry.pairs::<u64, Table>() {
            count += 1;
        }
        Ok(count)
    })?)?;

    // Entity.has_component(id, component_name) - 컴포넌트 존재 확인
    entity_t.set("has_component", lua.create_function(|lua, (id, component): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(id) {
            let components: Option<Table> = entity_data.get("components").ok();
            if let Some(comps) = components {
                let has: bool = comps.get(component).unwrap_or(false);
                return Ok(has);
            }
        }
        Ok(false)
    })?)?;

    skope.set("Entity", entity_t)?;
    Ok(())
}

/// Entity 레지스트리 업데이트 (Rust에서 호출)
/// ECS World의 엔티티 정보를 Lua로 동기화
pub fn update_entity_registry(
    lua: &Lua,
    entities: &[(u64, String, Option<EntityTransform>)],
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let entity: Table = skope.get("Entity")?;

    // 새 레지스트리 생성
    let registry = lua.create_table()?;
    let name_lookup = lua.create_table()?;

    for (id, name, transform) in entities {
        let entity_data = lua.create_table()?;
        entity_data.set("name", name.clone())?;

        // Transform 데이터
        if let Some(t) = transform {
            let transform_t = lua.create_table()?;

            let pos = lua.create_table()?;
            pos.set("x", t.position.0)?;
            pos.set("y", t.position.1)?;
            pos.set("z", t.position.2)?;
            transform_t.set("position", pos)?;

            let rot = lua.create_table()?;
            rot.set("x", t.rotation.0)?;
            rot.set("y", t.rotation.1)?;
            rot.set("z", t.rotation.2)?;
            rot.set("w", t.rotation.3)?;
            transform_t.set("rotation", rot)?;

            let scale = lua.create_table()?;
            scale.set("x", t.scale.0)?;
            scale.set("y", t.scale.1)?;
            scale.set("z", t.scale.2)?;
            transform_t.set("scale", scale)?;

            entity_data.set("transform", transform_t)?;
        }

        // 컴포넌트 플래그 (확장 가능)
        let components = lua.create_table()?;
        components.set("Transform", transform.is_some())?;
        entity_data.set("components", components)?;

        registry.set(*id, entity_data)?;
        name_lookup.set(name.clone(), *id)?;
    }

    entity.set("_registry", registry)?;
    entity.set("_name_lookup", name_lookup)?;

    Ok(())
}

/// Entity Transform 데이터 (Rust에서 Lua로 전달용)
#[derive(Debug, Clone)]
pub struct EntityTransform {
    pub position: (f32, f32, f32),
    pub rotation: (f32, f32, f32, f32),  // Quaternion (x, y, z, w)
    pub scale: (f32, f32, f32),
}

/// 디버그 드로우 명령
#[derive(Debug, Clone)]
pub enum DebugDrawCommand {
    Line {
        from: (f32, f32, f32),
        to: (f32, f32, f32),
        color: (f32, f32, f32, f32),
    },
    Sphere {
        center: (f32, f32, f32),
        radius: f32,
        color: (f32, f32, f32, f32),
    },
    Box {
        min: (f32, f32, f32),
        max: (f32, f32, f32),
        color: (f32, f32, f32, f32),
    },
    Point {
        position: (f32, f32, f32),
        size: f32,
        color: (f32, f32, f32, f32),
    },
    Axis {
        position: (f32, f32, f32),
        size: f32,
    },
}

/// 디버그 드로우 큐에서 명령들을 읽어옴 (Rust에서 호출)
/// 읽은 후 큐를 비움
pub fn read_debug_draw_queue(lua: &Lua) -> LuaResult<Vec<DebugDrawCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let debug: Table = skope.get("Debug")?;
    let queue: Table = debug.get("_draw_queue")?;

    let mut commands = Vec::new();

    for i in 1..=queue.len()? {
        if let Ok(cmd) = queue.get::<Table>(i) {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "line" => Some(DebugDrawCommand::Line {
                    from: (
                        cmd.get("from_x").unwrap_or(0.0),
                        cmd.get("from_y").unwrap_or(0.0),
                        cmd.get("from_z").unwrap_or(0.0),
                    ),
                    to: (
                        cmd.get("to_x").unwrap_or(0.0),
                        cmd.get("to_y").unwrap_or(0.0),
                        cmd.get("to_z").unwrap_or(0.0),
                    ),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "sphere" => Some(DebugDrawCommand::Sphere {
                    center: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    radius: cmd.get("radius").unwrap_or(1.0),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "box" => Some(DebugDrawCommand::Box {
                    min: (
                        cmd.get("min_x").unwrap_or(0.0),
                        cmd.get("min_y").unwrap_or(0.0),
                        cmd.get("min_z").unwrap_or(0.0),
                    ),
                    max: (
                        cmd.get("max_x").unwrap_or(0.0),
                        cmd.get("max_y").unwrap_or(0.0),
                        cmd.get("max_z").unwrap_or(0.0),
                    ),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "point" => Some(DebugDrawCommand::Point {
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    size: cmd.get("size").unwrap_or(0.1),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                        cmd.get("a").unwrap_or(1.0),
                    ),
                }),
                "axis" => Some(DebugDrawCommand::Axis {
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    size: cmd.get("size").unwrap_or(1.0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // 큐 비우기
    let new_queue = lua.create_table()?;
    debug.set("_draw_queue", new_queue)?;

    Ok(commands)
}

// ============ Audio API ============

/// Audio command from Lua
#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play { sound: String, volume: f32, looping: bool },
    PlayMusic { sound: String },
    Stop { id: u64 },
    StopAll,
    StopMusic,
    SetMasterVolume { volume: f32 },
    SetMusicVolume { volume: f32 },
    SetSfxVolume { volume: f32 },
}

/// Register Audio API
fn register_audio(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let audio = lua.create_table()?;

    // Command queue for Rust to process
    let cmd_queue = lua.create_table()?;
    audio.set("_command_queue", cmd_queue)?;

    // Audio.play(sound_name, [volume], [loop])
    audio.set("play", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let mut iter = args.into_iter();
        let sound: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let volume: f32 = iter.next()
            .and_then(|v| v.as_number().map(|n| n as f32))
            .unwrap_or(1.0);
        let looping: bool = iter.next()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let cmd = lua.create_table()?;
        cmd.set("type", "play")?;
        cmd.set("sound", sound)?;
        cmd.set("volume", volume)?;
        cmd.set("loop", looping)?;

        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.play_music(sound_name)
    audio.set("play_music", lua.create_function(|lua, sound: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "play_music")?;
        cmd.set("sound", sound)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.stop(id) - stop specific sound
    audio.set("stop", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("id", id)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.stop_all()
    audio.set("stop_all", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop_all")?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.stop_music()
    audio.set("stop_music", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop_music")?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_volume(volume) - master volume
    audio.set("set_volume", lua.create_function(|lua, volume: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_master_volume")?;
        cmd.set("volume", volume)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_music_volume(volume)
    audio.set("set_music_volume", lua.create_function(|lua, volume: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_music_volume")?;
        cmd.set("volume", volume)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_sfx_volume(volume)
    audio.set("set_sfx_volume", lua.create_function(|lua, volume: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sfx_volume")?;
        cmd.set("volume", volume)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Audio", audio)?;
    Ok(())
}

/// Process audio commands from Lua
pub fn process_audio_commands(lua: &Lua) -> LuaResult<Vec<AudioCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let audio: Table = skope.get("Audio")?;
    let queue: Table = audio.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "play" => Some(AudioCommand::Play {
                    sound: cmd.get("sound").unwrap_or_default(),
                    volume: cmd.get("volume").unwrap_or(1.0),
                    looping: cmd.get("loop").unwrap_or(false),
                }),
                "play_music" => Some(AudioCommand::PlayMusic {
                    sound: cmd.get("sound").unwrap_or_default(),
                }),
                "stop" => Some(AudioCommand::Stop {
                    id: cmd.get("id").unwrap_or(0),
                }),
                "stop_all" => Some(AudioCommand::StopAll),
                "stop_music" => Some(AudioCommand::StopMusic),
                "set_master_volume" => Some(AudioCommand::SetMasterVolume {
                    volume: cmd.get("volume").unwrap_or(1.0),
                }),
                "set_music_volume" => Some(AudioCommand::SetMusicVolume {
                    volume: cmd.get("volume").unwrap_or(1.0),
                }),
                "set_sfx_volume" => Some(AudioCommand::SetSfxVolume {
                    volume: cmd.get("volume").unwrap_or(1.0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    let new_queue = lua.create_table()?;
    audio.set("_command_queue", new_queue)?;

    Ok(commands)
}

// ============ Collision API ============

/// Collision event data for Lua
#[derive(Debug, Clone)]
pub struct LuaCollisionEvent {
    pub entity_a: u64,
    pub entity_b: u64,
    pub is_enter: bool,  // true = enter, false = exit
}

/// Register Collision API
fn register_collision(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let collision = lua.create_table()?;

    // Event queue (Rust pushes events here)
    let events_queue = lua.create_table()?;
    collision.set("_events", events_queue)?;

    // Registered handlers table
    let handlers = lua.create_table()?;
    collision.set("_handlers", handlers)?;

    // Collision.on_enter(entity_id, callback) - register enter handler
    collision.set("on_enter", lua.create_function(|lua, (entity_id, callback): (u64, mlua::Function)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let handlers: Table = collision.get("_handlers")?;

        // Get or create handler table for this entity
        let entity_handlers: Table = handlers.get(entity_id)
            .unwrap_or_else(|_| lua.create_table().unwrap());

        entity_handlers.set("on_enter", callback)?;
        handlers.set(entity_id, entity_handlers)?;
        Ok(())
    })?)?;

    // Collision.on_exit(entity_id, callback) - register exit handler
    collision.set("on_exit", lua.create_function(|lua, (entity_id, callback): (u64, mlua::Function)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let handlers: Table = collision.get("_handlers")?;

        let entity_handlers: Table = handlers.get(entity_id)
            .unwrap_or_else(|_| lua.create_table().unwrap());

        entity_handlers.set("on_exit", callback)?;
        handlers.set(entity_id, entity_handlers)?;
        Ok(())
    })?)?;

    // Collision.get_events() - get all events this frame (for polling)
    collision.set("get_events", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let events: Table = collision.get("_events")?;

        // Return a copy of events
        let result = lua.create_table()?;
        for pair in events.pairs::<i64, Table>() {
            if let Ok((i, event)) = pair {
                result.set(i, event)?;
            }
        }
        Ok(result)
    })?)?;

    // Collision.get_collisions_with(entity_id) - get entities colliding with this one
    collision.set("get_collisions_with", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let events: Table = collision.get("_events")?;

        let result = lua.create_table()?;
        let mut idx = 1i64;

        for pair in events.pairs::<i64, Table>() {
            if let Ok((_, event)) = pair {
                let is_enter: bool = event.get("is_enter").unwrap_or(false);
                if !is_enter { continue; }

                let a: u64 = event.get("entity_a").unwrap_or(0);
                let b: u64 = event.get("entity_b").unwrap_or(0);

                if a == entity_id {
                    result.set(idx, b)?;
                    idx += 1;
                } else if b == entity_id {
                    result.set(idx, a)?;
                    idx += 1;
                }
            }
        }
        Ok(result)
    })?)?;

    // Collision.is_colliding(entity_a, entity_b) - check if two entities are colliding
    collision.set("is_colliding", lua.create_function(|lua, (entity_a, entity_b): (u64, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let collision: Table = skope.get("Collision")?;
        let events: Table = collision.get("_events")?;

        for pair in events.pairs::<i64, Table>() {
            if let Ok((_, event)) = pair {
                let is_enter: bool = event.get("is_enter").unwrap_or(false);
                if !is_enter { continue; }

                let a: u64 = event.get("entity_a").unwrap_or(0);
                let b: u64 = event.get("entity_b").unwrap_or(0);

                if (a == entity_a && b == entity_b) || (a == entity_b && b == entity_a) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    })?)?;

    skope.set("Collision", collision)?;
    Ok(())
}

/// Push collision events to Lua (called from Rust each frame)
pub fn push_collision_events(lua: &Lua, events: &[LuaCollisionEvent]) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let collision: Table = skope.get("Collision")?;

    // Create fresh events table
    let events_table = lua.create_table()?;
    for (i, event) in events.iter().enumerate() {
        let e = lua.create_table()?;
        e.set("entity_a", event.entity_a)?;
        e.set("entity_b", event.entity_b)?;
        e.set("is_enter", event.is_enter)?;
        events_table.set((i + 1) as i64, e)?;
    }
    collision.set("_events", events_table)?;

    // Call registered handlers
    let handlers: Table = collision.get("_handlers")?;
    for event in events {
        // Call handler for entity_a
        if let Ok(entity_handlers) = handlers.get::<Table>(event.entity_a) {
            let callback_name = if event.is_enter { "on_enter" } else { "on_exit" };
            if let Ok(callback) = entity_handlers.get::<mlua::Function>(callback_name) {
                let _ = callback.call::<()>(event.entity_b);
            }
        }

        // Call handler for entity_b
        if let Ok(entity_handlers) = handlers.get::<Table>(event.entity_b) {
            let callback_name = if event.is_enter { "on_enter" } else { "on_exit" };
            if let Ok(callback) = entity_handlers.get::<mlua::Function>(callback_name) {
                let _ = callback.call::<()>(event.entity_a);
            }
        }
    }

    Ok(())
}
