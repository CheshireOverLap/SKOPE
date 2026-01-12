// SKOPE Scripting API
// Lua에서 사용 가능한 엔진 API
#![allow(dead_code)]

use mlua::{Lua, Result as LuaResult, Table};

use super::ui_api::register_ui;
use super::math_api::register_math_apis;

/// 모든 API 등록
pub fn register_all(lua: &Lua) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;

    // Math APIs (Vec3, Quat, Math)
    register_math_apis(lua, &skope)?;

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

    // Spell API
    register_spell(lua, &skope)?;

    // Trigger API
    register_trigger(lua, &skope)?;

    // Effect API
    register_effect(lua, &skope)?;

    // Camera API
    register_camera(lua, &skope)?;

    // Physics API
    register_physics(lua, &skope)?;

    // Particles API
    register_particles(lua, &skope)?;

    // Lighting API
    register_lighting(lua, &skope)?;

    // UI API
    register_ui(lua, &skope)?;

    // Animation API
    register_animation(lua, &skope)?;

    // Animator API
    register_animator(lua, &skope)?;

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
    Play3D { sound: String, position: (f32, f32, f32), volume: f32, looping: bool },
    PlayMusic { sound: String },
    Stop { id: u64 },
    StopAll,
    StopMusic,
    Pause { id: u64 },
    Resume { id: u64 },
    SetMasterVolume { volume: f32 },
    SetMusicVolume { volume: f32 },
    SetSfxVolume { volume: f32 },
    SetSourcePosition { id: u64, position: (f32, f32, f32) },
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

    // Audio.play_3d(sound_name, position, [volume], [loop])
    audio.set("play_3d", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let mut iter = args.into_iter();
        let sound: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let position = iter.next()
            .and_then(|v| v.as_table().cloned())
            .unwrap_or_else(|| lua.create_table().unwrap());
        let volume: f32 = iter.next()
            .and_then(|v| v.as_number().map(|n| n as f32))
            .unwrap_or(1.0);
        let looping: bool = iter.next()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let cmd = lua.create_table()?;
        cmd.set("type", "play_3d")?;
        cmd.set("sound", sound)?;
        cmd.set("x", position.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", position.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", position.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("volume", volume)?;
        cmd.set("loop", looping)?;

        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.pause(id)
    audio.set("pause", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "pause")?;
        cmd.set("id", id)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.resume(id)
    audio.set("resume", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "resume")?;
        cmd.set("id", id)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_source_position(id, position)
    audio.set("set_source_position", lua.create_function(|lua, (id, position): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_source_position")?;
        cmd.set("id", id)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;
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
                "play_3d" => Some(AudioCommand::Play3D {
                    sound: cmd.get("sound").unwrap_or_default(),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    volume: cmd.get("volume").unwrap_or(1.0),
                    looping: cmd.get("loop").unwrap_or(false),
                }),
                "pause" => Some(AudioCommand::Pause {
                    id: cmd.get("id").unwrap_or(0),
                }),
                "resume" => Some(AudioCommand::Resume {
                    id: cmd.get("id").unwrap_or(0),
                }),
                "set_source_position" => Some(AudioCommand::SetSourcePosition {
                    id: cmd.get("id").unwrap_or(0),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
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

// ============ Spell API ============

/// Spell command from Lua
#[derive(Debug, Clone)]
pub enum SpellCommand {
    Cast {
        spell_name: String,
        caster_id: u64,
        target_pos: (f32, f32, f32),
        timestamp: f64,
    },
    ApplyEffect {
        target_id: u64,
        effect_name: String,
        duration: f32,
        params: Vec<(String, f32)>,
    },
}

/// Register Spell API
fn register_spell(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let spell = lua.create_table()?;

    // Spell definitions storage: { [name] = { damage_type, base_damage, cooldown, ... } }
    let definitions = lua.create_table()?;
    spell.set("_definitions", definitions)?;

    // Spell cast queue (Rust processes these)
    let cast_queue = lua.create_table()?;
    spell.set("_cast_queue", cast_queue)?;

    // Cooldown tracking: { [caster_id] = { [spell_name] = ready_at_time } }
    let cooldowns = lua.create_table()?;
    spell.set("_cooldowns", cooldowns)?;

    // Effect queue (for apply_effect calls)
    let effect_queue = lua.create_table()?;
    spell.set("_effect_queue", effect_queue)?;

    // Spell.define(name, definition) - define a new spell
    spell.set("define", lua.create_function(|lua, (name, definition): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        // Clone the definition table
        let def = lua.create_table()?;

        // Copy standard fields
        if let Ok(v) = definition.get::<String>("damage_type") { def.set("damage_type", v)?; }
        if let Ok(v) = definition.get::<f32>("base_damage") { def.set("base_damage", v)?; }
        if let Ok(v) = definition.get::<f32>("cooldown") { def.set("cooldown", v)?; }
        if let Ok(v) = definition.get::<f32>("range") { def.set("range", v)?; }
        if let Ok(v) = definition.get::<f32>("aoe_radius") { def.set("aoe_radius", v)?; }
        if let Ok(v) = definition.get::<f32>("mana_cost") { def.set("mana_cost", v)?; }
        if let Ok(v) = definition.get::<f32>("cast_time") { def.set("cast_time", v)?; }

        // Copy callbacks
        if let Ok(f) = definition.get::<mlua::Function>("on_cast") { def.set("on_cast", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_hit") { def.set("on_hit", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_end") { def.set("on_end", f)?; }

        definitions.set(name.clone(), def)?;
        log::debug!("[Lua:Spell] Defined spell: {}", name);
        Ok(())
    })?)?;

    // Spell.cast(spell_name, caster_id, target_pos) - cast a spell
    spell.set("cast", lua.create_function(|lua, (spell_name, caster_id, target_pos): (String, u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        // Check if spell exists
        if definitions.get::<Table>(spell_name.clone()).is_err() {
            log::warn!("[Lua:Spell] Unknown spell: {}", spell_name);
            return Ok(false);
        }

        // Check cooldown
        let time: Table = skope.get("Time")?;
        let elapsed: f64 = time.get("elapsed").unwrap_or(0.0);

        let cooldowns: Table = spell_t.get("_cooldowns")?;
        let caster_cooldowns: Table = cooldowns.get(caster_id)
            .unwrap_or_else(|_| lua.create_table().unwrap());

        let ready_at: f64 = caster_cooldowns.get(spell_name.clone()).unwrap_or(0.0);
        if elapsed < ready_at {
            log::debug!("[Lua:Spell] {} on cooldown for caster {}", spell_name, caster_id);
            return Ok(false);
        }

        // Add to cast queue
        let cast_queue: Table = spell_t.get("_cast_queue")?;
        let len = cast_queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("spell_name", spell_name.clone())?;
        cmd.set("caster_id", caster_id)?;
        cmd.set("target_x", target_pos.get::<f32>(1).or_else(|_| target_pos.get::<f32>("x")).unwrap_or(0.0))?;
        cmd.set("target_y", target_pos.get::<f32>(2).or_else(|_| target_pos.get::<f32>("y")).unwrap_or(0.0))?;
        cmd.set("target_z", target_pos.get::<f32>(3).or_else(|_| target_pos.get::<f32>("z")).unwrap_or(0.0))?;
        cmd.set("timestamp", elapsed)?;

        cast_queue.set(len + 1, cmd)?;

        // Set cooldown
        if let Ok(def) = definitions.get::<Table>(spell_name.clone()) {
            let cooldown: f32 = def.get("cooldown").unwrap_or(0.0);
            caster_cooldowns.set(spell_name.clone(), elapsed + cooldown as f64)?;
            cooldowns.set(caster_id, caster_cooldowns)?;
        }

        log::debug!("[Lua:Spell] Cast {} by caster {}", spell_name, caster_id);
        Ok(true)
    })?)?;

    // Spell.is_ready(spell_name, caster_id) - check if spell is off cooldown
    spell.set("is_ready", lua.create_function(|lua, (spell_name, caster_id): (String, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let cooldowns: Table = spell_t.get("_cooldowns")?;

        let time: Table = skope.get("Time")?;
        let elapsed: f64 = time.get("elapsed").unwrap_or(0.0);

        if let Ok(caster_cooldowns) = cooldowns.get::<Table>(caster_id) {
            let ready_at: f64 = caster_cooldowns.get(spell_name).unwrap_or(0.0);
            Ok(elapsed >= ready_at)
        } else {
            Ok(true) // No cooldowns recorded = ready
        }
    })?)?;

    // Spell.get_cooldown(spell_name, caster_id) - get remaining cooldown time
    spell.set("get_cooldown", lua.create_function(|lua, (spell_name, caster_id): (String, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let cooldowns: Table = spell_t.get("_cooldowns")?;

        let time: Table = skope.get("Time")?;
        let elapsed: f64 = time.get("elapsed").unwrap_or(0.0);

        if let Ok(caster_cooldowns) = cooldowns.get::<Table>(caster_id) {
            let ready_at: f64 = caster_cooldowns.get(spell_name).unwrap_or(0.0);
            let remaining = (ready_at - elapsed).max(0.0);
            Ok(remaining as f32)
        } else {
            Ok(0.0f32)
        }
    })?)?;

    // Spell.get_definition(spell_name) - get spell definition
    spell.set("get_definition", lua.create_function(|lua, spell_name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        let def: Option<Table> = definitions.get(spell_name).ok();
        Ok(def)
    })?)?;

    // Spell.apply_effect(target_id, effect_name, duration, params) - apply buff/debuff
    spell.set("apply_effect", lua.create_function(|lua, (target_id, effect_name, duration, params): (u64, String, f32, Option<Table>)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let effect_queue: Table = spell_t.get("_effect_queue")?;
        let len = effect_queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("target_id", target_id)?;
        cmd.set("effect_name", effect_name.clone())?;
        cmd.set("duration", duration)?;

        // Copy params if provided
        if let Some(p) = params {
            let params_copy = lua.create_table()?;
            for pair in p.pairs::<String, f32>() {
                if let Ok((k, v)) = pair {
                    params_copy.set(k, v)?;
                }
            }
            cmd.set("params", params_copy)?;
        }

        effect_queue.set(len + 1, cmd)?;
        log::debug!("[Lua:Spell] Applied effect {} to {} for {}s", effect_name, target_id, duration);
        Ok(())
    })?)?;

    // Spell.list() - list all defined spells
    spell.set("list", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let spell_t: Table = skope.get("Spell")?;
        let definitions: Table = spell_t.get("_definitions")?;

        let result = lua.create_table()?;
        let mut idx = 1;
        for pair in definitions.pairs::<String, Table>() {
            if let Ok((name, _)) = pair {
                result.set(idx, name)?;
                idx += 1;
            }
        }
        Ok(result)
    })?)?;

    skope.set("Spell", spell)?;
    Ok(())
}

/// Process spell cast commands from Lua (called from Rust each frame)
pub fn process_spell_commands(lua: &Lua) -> LuaResult<Vec<SpellCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let spell: Table = skope.get("Spell")?;
    let cast_queue: Table = spell.get("_cast_queue")?;
    let effect_queue: Table = spell.get("_effect_queue")?;

    let mut commands = Vec::new();

    // Process cast commands
    for pair in cast_queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            commands.push(SpellCommand::Cast {
                spell_name: cmd.get("spell_name").unwrap_or_default(),
                caster_id: cmd.get("caster_id").unwrap_or(0),
                target_pos: (
                    cmd.get("target_x").unwrap_or(0.0),
                    cmd.get("target_y").unwrap_or(0.0),
                    cmd.get("target_z").unwrap_or(0.0),
                ),
                timestamp: cmd.get("timestamp").unwrap_or(0.0),
            });
        }
    }

    // Process effect commands
    for pair in effect_queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let mut params = Vec::new();
            if let Ok(p) = cmd.get::<Table>("params") {
                for pair in p.pairs::<String, f32>() {
                    if let Ok((k, v)) = pair {
                        params.push((k, v));
                    }
                }
            }
            commands.push(SpellCommand::ApplyEffect {
                target_id: cmd.get("target_id").unwrap_or(0),
                effect_name: cmd.get("effect_name").unwrap_or_default(),
                duration: cmd.get("duration").unwrap_or(0.0),
                params,
            });
        }
    }

    // Clear queues
    let new_cast_queue = lua.create_table()?;
    let new_effect_queue = lua.create_table()?;
    spell.set("_cast_queue", new_cast_queue)?;
    spell.set("_effect_queue", new_effect_queue)?;

    Ok(commands)
}

/// Call spell's on_cast callback (called from Rust when processing spell)
pub fn call_spell_on_cast(lua: &Lua, spell_name: &str, caster_id: u64, target_pos: (f32, f32, f32)) -> LuaResult<Option<Table>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let spell: Table = skope.get("Spell")?;
    let definitions: Table = spell.get("_definitions")?;

    if let Ok(def) = definitions.get::<Table>(spell_name) {
        if let Ok(on_cast) = def.get::<mlua::Function>("on_cast") {
            let pos = lua.create_table()?;
            pos.set("x", target_pos.0)?;
            pos.set("y", target_pos.1)?;
            pos.set("z", target_pos.2)?;

            let result: Option<Table> = on_cast.call((caster_id, pos)).ok();
            return Ok(result);
        }
    }
    Ok(None)
}

/// Call spell's on_hit callback
pub fn call_spell_on_hit(lua: &Lua, spell_name: &str, caster_id: u64, target_id: u64) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let spell: Table = skope.get("Spell")?;
    let definitions: Table = spell.get("_definitions")?;

    if let Ok(def) = definitions.get::<Table>(spell_name) {
        if let Ok(on_hit) = def.get::<mlua::Function>("on_hit") {
            let _ = on_hit.call::<()>((caster_id, target_id));
        }
    }
    Ok(())
}

// ============ Trigger API ============

/// Trigger event from Lua
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub trigger_name: String,
    pub entity_id: u64,
    pub event_type: TriggerEventType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriggerEventType {
    Enter,
    Stay,
    Exit,
}

/// Register Trigger API
fn register_trigger(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let trigger = lua.create_table()?;

    // Trigger definitions: { [name] = { shape, radius, position, ... } }
    let definitions = lua.create_table()?;
    trigger.set("_definitions", definitions)?;

    // Entities currently inside each trigger: { [trigger_name] = { [entity_id] = enter_time } }
    let entities_inside = lua.create_table()?;
    trigger.set("_entities_inside", entities_inside)?;

    // Event queue for Rust to process
    let event_queue = lua.create_table()?;
    trigger.set("_event_queue", event_queue)?;

    // Trigger.define(name, definition) - define a new trigger
    trigger.set("define", lua.create_function(|lua, (name, definition): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        // Clone definition
        let def = lua.create_table()?;

        // Shape type: sphere, box, cylinder
        if let Ok(v) = definition.get::<String>("shape") { def.set("shape", v)?; }
        else { def.set("shape", "sphere")?; }

        // Dimensions
        if let Ok(v) = definition.get::<f32>("radius") { def.set("radius", v)?; }
        if let Ok(v) = definition.get::<Table>("size") { def.set("size", v)?; }
        if let Ok(v) = definition.get::<f32>("height") { def.set("height", v)?; }

        // Position
        if let Ok(v) = definition.get::<Table>("position") {
            let pos = lua.create_table()?;
            pos.set("x", v.get::<f32>(1).or_else(|_| v.get::<f32>("x")).unwrap_or(0.0))?;
            pos.set("y", v.get::<f32>(2).or_else(|_| v.get::<f32>("y")).unwrap_or(0.0))?;
            pos.set("z", v.get::<f32>(3).or_else(|_| v.get::<f32>("z")).unwrap_or(0.0))?;
            def.set("position", pos)?;
        } else {
            let pos = lua.create_table()?;
            pos.set("x", 0.0)?;
            pos.set("y", 0.0)?;
            pos.set("z", 0.0)?;
            def.set("position", pos)?;
        }

        // Enabled by default
        def.set("enabled", definition.get::<bool>("enabled").unwrap_or(true))?;

        // Callbacks
        if let Ok(f) = definition.get::<mlua::Function>("filter") { def.set("filter", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_enter") { def.set("on_enter", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_stay") { def.set("on_stay", f)?; }
        if let Ok(f) = definition.get::<mlua::Function>("on_exit") { def.set("on_exit", f)?; }

        definitions.set(name.clone(), def)?;

        // Initialize entities_inside for this trigger
        let inside = lua.create_table()?;
        entities_inside.set(name.clone(), inside)?;

        log::debug!("[Lua:Trigger] Defined trigger: {}", name);
        Ok(())
    })?)?;

    // Trigger.enable(name, enabled) - enable/disable a trigger
    trigger.set("enable", lua.create_function(|lua, (name, enabled): (String, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        if let Ok(def) = definitions.get::<Table>(name.clone()) {
            def.set("enabled", enabled)?;
            log::debug!("[Lua:Trigger] {} {}", name, if enabled { "enabled" } else { "disabled" });
            Ok(true)
        } else {
            Ok(false)
        }
    })?)?;

    // Trigger.set_position(name, position) - move trigger
    trigger.set("set_position", lua.create_function(|lua, (name, position): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        if let Ok(def) = definitions.get::<Table>(name.clone()) {
            let pos = lua.create_table()?;
            pos.set("x", position.get::<f32>(1).or_else(|_| position.get::<f32>("x")).unwrap_or(0.0))?;
            pos.set("y", position.get::<f32>(2).or_else(|_| position.get::<f32>("y")).unwrap_or(0.0))?;
            pos.set("z", position.get::<f32>(3).or_else(|_| position.get::<f32>("z")).unwrap_or(0.0))?;
            def.set("position", pos)?;
            Ok(true)
        } else {
            Ok(false)
        }
    })?)?;

    // Trigger.get_position(name) - get trigger position
    trigger.set("get_position", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        if let Ok(def) = definitions.get::<Table>(name) {
            let pos: Option<Table> = def.get("position").ok();
            Ok(pos)
        } else {
            Ok(None)
        }
    })?)?;

    // Trigger.get_entities_in(name) - get all entities currently inside trigger
    trigger.set("get_entities_in", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        let result = lua.create_table()?;
        if let Ok(inside) = entities_inside.get::<Table>(name) {
            let mut idx = 1;
            for pair in inside.pairs::<u64, f64>() {
                if let Ok((entity_id, _)) = pair {
                    result.set(idx, entity_id)?;
                    idx += 1;
                }
            }
        }
        Ok(result)
    })?)?;

    // Trigger.is_inside(name, entity_id) - check if entity is inside trigger
    trigger.set("is_inside", lua.create_function(|lua, (name, entity_id): (String, u64)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        if let Ok(inside) = entities_inside.get::<Table>(name) {
            let is_in: Option<f64> = inside.get(entity_id).ok();
            Ok(is_in.is_some())
        } else {
            Ok(false)
        }
    })?)?;

    // Trigger.get_definition(name) - get trigger definition
    trigger.set("get_definition", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        let def: Option<Table> = definitions.get(name).ok();
        Ok(def)
    })?)?;

    // Trigger.list() - list all defined triggers
    trigger.set("list", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;

        let result = lua.create_table()?;
        let mut idx = 1;
        for pair in definitions.pairs::<String, Table>() {
            if let Ok((name, _)) = pair {
                result.set(idx, name)?;
                idx += 1;
            }
        }
        Ok(result)
    })?)?;

    // Trigger.remove(name) - remove a trigger
    trigger.set("remove", lua.create_function(|lua, name: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let trigger_t: Table = skope.get("Trigger")?;
        let definitions: Table = trigger_t.get("_definitions")?;
        let entities_inside: Table = trigger_t.get("_entities_inside")?;

        definitions.set(name.clone(), mlua::Value::Nil)?;
        entities_inside.set(name.clone(), mlua::Value::Nil)?;
        log::debug!("[Lua:Trigger] Removed trigger: {}", name);
        Ok(())
    })?)?;

    skope.set("Trigger", trigger)?;
    Ok(())
}

/// Get all trigger definitions for Rust-side processing
pub fn get_trigger_definitions(lua: &Lua) -> LuaResult<Vec<(String, TriggerDefinition)>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let trigger: Table = skope.get("Trigger")?;
    let definitions: Table = trigger.get("_definitions")?;

    let mut result = Vec::new();
    for pair in definitions.pairs::<String, Table>() {
        if let Ok((name, def)) = pair {
            let enabled: bool = def.get("enabled").unwrap_or(true);
            if !enabled { continue; }

            let shape: String = def.get("shape").unwrap_or_else(|_| "sphere".to_string());
            let radius: f32 = def.get("radius").unwrap_or(1.0);

            let position = if let Ok(pos) = def.get::<Table>("position") {
                (
                    pos.get("x").unwrap_or(0.0),
                    pos.get("y").unwrap_or(0.0),
                    pos.get("z").unwrap_or(0.0),
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            result.push((name, TriggerDefinition {
                shape,
                radius,
                position,
            }));
        }
    }
    Ok(result)
}

/// Trigger definition for Rust
#[derive(Debug, Clone)]
pub struct TriggerDefinition {
    pub shape: String,
    pub radius: f32,
    pub position: (f32, f32, f32),
}

/// Update trigger state and fire callbacks (called from Rust)
pub fn update_trigger_state(
    lua: &Lua,
    trigger_name: &str,
    entity_id: u64,
    is_inside: bool,
    elapsed_time: f64,
) -> LuaResult<Option<TriggerEvent>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let trigger: Table = skope.get("Trigger")?;
    let definitions: Table = trigger.get("_definitions")?;
    let entities_inside: Table = trigger.get("_entities_inside")?;

    // Get definition
    let def = match definitions.get::<Table>(trigger_name) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    // Check filter callback if exists
    if let Ok(filter) = def.get::<mlua::Function>("filter") {
        let passes: bool = filter.call::<bool>(entity_id).unwrap_or(true);
        if !passes {
            return Ok(None);
        }
    }

    // Get or create entities_inside table for this trigger
    let inside: Table = entities_inside.get(trigger_name)
        .unwrap_or_else(|_| lua.create_table().unwrap());

    let was_inside: bool = inside.get::<f64>(entity_id).is_ok();

    let event_type = match (was_inside, is_inside) {
        (false, true) => {
            // Enter
            inside.set(entity_id, elapsed_time)?;
            entities_inside.set(trigger_name, inside)?;

            if let Ok(on_enter) = def.get::<mlua::Function>("on_enter") {
                let _ = on_enter.call::<()>((entity_id, trigger_name));
            }
            Some(TriggerEventType::Enter)
        }
        (true, true) => {
            // Stay
            let enter_time: f64 = inside.get(entity_id).unwrap_or(elapsed_time);
            let duration = elapsed_time - enter_time;

            if let Ok(on_stay) = def.get::<mlua::Function>("on_stay") {
                let _ = on_stay.call::<()>((entity_id, trigger_name, duration));
            }
            Some(TriggerEventType::Stay)
        }
        (true, false) => {
            // Exit
            inside.set(entity_id, mlua::Value::Nil)?;
            entities_inside.set(trigger_name, inside)?;

            if let Ok(on_exit) = def.get::<mlua::Function>("on_exit") {
                let _ = on_exit.call::<()>((entity_id, trigger_name));
            }
            Some(TriggerEventType::Exit)
        }
        (false, false) => None,
    };

    Ok(event_type.map(|et| TriggerEvent {
        trigger_name: trigger_name.to_string(),
        entity_id,
        event_type: et,
    }))
}

/// Effect API for spawning and controlling visual effects
fn register_effect(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let effect = lua.create_table()?;

    // Command queue for Rust to process
    let cmd_queue = lua.create_table()?;
    effect.set("_command_queue", cmd_queue)?;

    // Callback storage (handle_id -> lua_ref)
    let callbacks = lua.create_table()?;
    effect.set("_callbacks", callbacks)?;

    // Handle counter
    effect.set("_next_handle", 1u64)?;

    // Effect.spawn(name, position, [options])
    // Returns: handle_id (number)
    effect.set("spawn", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        // Get next handle ID
        let handle: u64 = effect.get("_next_handle")?;
        effect.set("_next_handle", handle + 1)?;

        let mut iter = args.into_iter();

        // Required: effect name
        let name: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        // Required: position
        let pos = iter.next();
        let (x, y, z) = if let Some(mlua::Value::Table(t)) = pos {
            (
                t.get::<f32>("x").unwrap_or(0.0),
                t.get::<f32>("y").unwrap_or(0.0),
                t.get::<f32>("z").unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        // Optional: options table
        let opts = iter.next();
        let (speed, scale, color_r, color_g, color_b, color_a, emission) = if let Some(mlua::Value::Table(t)) = opts {
            (
                t.get::<f32>("speed").unwrap_or(1.0),
                t.get::<f32>("scale").unwrap_or(1.0),
                t.get::<f32>("color_r").unwrap_or(1.0),
                t.get::<f32>("color_g").unwrap_or(1.0),
                t.get::<f32>("color_b").unwrap_or(1.0),
                t.get::<f32>("color_a").unwrap_or(1.0),
                t.get::<f32>("emission").unwrap_or(1.0),
            )
        } else {
            (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
        };

        let cmd = lua.create_table()?;
        cmd.set("type", "spawn")?;
        cmd.set("handle", handle)?;
        cmd.set("name", name)?;
        cmd.set("x", x)?;
        cmd.set("y", y)?;
        cmd.set("z", z)?;
        cmd.set("speed", speed)?;
        cmd.set("scale", scale)?;
        cmd.set("color_r", color_r)?;
        cmd.set("color_g", color_g)?;
        cmd.set("color_b", color_b)?;
        cmd.set("color_a", color_a)?;
        cmd.set("emission", emission)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;

        Ok(handle)
    })?)?;

    // Effect.stop(handle)
    effect.set("stop", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("handle", handle)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.on_complete(handle, callback)
    // Stores the callback function directly in the callbacks table
    effect.set("on_complete", lua.create_function(|lua, (handle, callback): (u64, mlua::Function)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let callbacks: Table = effect.get("_callbacks")?;

        // Store callback function directly
        callbacks.set(handle, callback)?;

        Ok(())
    })?)?;

    // Effect.set_speed(handle, speed)
    effect.set("set_speed", lua.create_function(|lua, (handle, speed): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_speed")?;
        cmd.set("handle", handle)?;
        cmd.set("speed", speed)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.pause(handle)
    effect.set("pause", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "pause")?;
        cmd.set("handle", handle)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.resume(handle)
    effect.set("resume", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "resume")?;
        cmd.set("handle", handle)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.attach(handle, entity_id, [offset])
    effect.set("attach", lua.create_function(|lua, (handle, entity_id, offset): (u64, u64, Option<Table>)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let (ox, oy, oz) = if let Some(t) = offset {
            (
                t.get::<f32>("x").unwrap_or(0.0),
                t.get::<f32>("y").unwrap_or(0.0),
                t.get::<f32>("z").unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        let cmd = lua.create_table()?;
        cmd.set("type", "attach")?;
        cmd.set("handle", handle)?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("offset_x", ox)?;
        cmd.set("offset_y", oy)?;
        cmd.set("offset_z", oz)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.detach(handle)
    effect.set("detach", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let queue: Table = effect.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "detach")?;
        cmd.set("handle", handle)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Effect.is_playing(handle) -> bool
    // Note: This is synchronous, returns last known state
    effect.set("_playing", lua.create_table()?)?;
    effect.set("is_playing", lua.create_function(|lua, handle: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let effect: Table = skope.get("Effect")?;
        let playing: Table = effect.get("_playing")?;

        let is_playing: bool = playing.get(handle).unwrap_or(false);
        Ok(is_playing)
    })?)?;

    skope.set("Effect", effect)?;
    Ok(())
}

/// Effect command enum for processing Lua commands
#[derive(Debug, Clone)]
pub enum EffectCommand {
    /// Spawn a new effect instance
    Spawn {
        handle: u64,
        name: String,
        position: (f32, f32, f32),
        speed: f32,
        scale: f32,
        color: [f32; 4],
    },
    /// Stop and despawn an effect
    Stop { handle: u64 },
    /// Set effect playback speed
    SetSpeed { handle: u64, speed: f32 },
    /// Pause effect playback
    Pause { handle: u64 },
    /// Resume effect playback
    Resume { handle: u64 },
    /// Attach effect to an entity
    Attach {
        handle: u64,
        entity_id: u64,
        offset: (f32, f32, f32),
    },
    /// Detach effect from entity
    Detach { handle: u64 },
}

/// Process effect commands from Lua
pub fn process_effect_commands(lua: &Lua) -> LuaResult<Vec<EffectCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let queue: Table = effect.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "spawn" => Some(EffectCommand::Spawn {
                    handle: cmd.get("handle").unwrap_or(0),
                    name: cmd.get("name").unwrap_or_default(),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    speed: cmd.get("speed").unwrap_or(1.0),
                    scale: cmd.get("scale").unwrap_or(1.0),
                    color: [
                        cmd.get("color_r").unwrap_or(1.0),
                        cmd.get("color_g").unwrap_or(1.0),
                        cmd.get("color_b").unwrap_or(1.0),
                        cmd.get("color_a").unwrap_or(1.0),
                    ],
                }),
                "stop" => Some(EffectCommand::Stop {
                    handle: cmd.get("handle").unwrap_or(0),
                }),
                "set_speed" => Some(EffectCommand::SetSpeed {
                    handle: cmd.get("handle").unwrap_or(0),
                    speed: cmd.get("speed").unwrap_or(1.0),
                }),
                "pause" => Some(EffectCommand::Pause {
                    handle: cmd.get("handle").unwrap_or(0),
                }),
                "resume" => Some(EffectCommand::Resume {
                    handle: cmd.get("handle").unwrap_or(0),
                }),
                "attach" => Some(EffectCommand::Attach {
                    handle: cmd.get("handle").unwrap_or(0),
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    offset: (
                        cmd.get("offset_x").unwrap_or(0.0),
                        cmd.get("offset_y").unwrap_or(0.0),
                        cmd.get("offset_z").unwrap_or(0.0),
                    ),
                }),
                "detach" => Some(EffectCommand::Detach {
                    handle: cmd.get("handle").unwrap_or(0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    effect.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Update effect playing state in Lua
pub fn update_effect_playing_state(lua: &Lua, handle: u64, playing: bool) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let playing_table: Table = effect.get("_playing")?;
    playing_table.set(handle, playing)?;
    Ok(())
}

/// Get effect completion callback for a handle
pub fn get_effect_callback(lua: &Lua, handle: u64) -> LuaResult<Option<mlua::Function>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let callbacks: Table = effect.get("_callbacks")?;
    callbacks.get(handle)
}

/// Remove effect callback after firing
pub fn remove_effect_callback(lua: &Lua, handle: u64) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let effect: Table = skope.get("Effect")?;
    let callbacks: Table = effect.get("_callbacks")?;
    callbacks.set(handle, mlua::Value::Nil)?;
    Ok(())
}

// ============ Camera API ============

/// Camera command enum
#[derive(Debug, Clone)]
pub enum CameraCommand {
    SetPosition { x: f32, y: f32, z: f32 },
    SetRotation { x: f32, y: f32, z: f32, w: f32 },
    SetYawPitch { yaw: f32, pitch: f32 },
    LookAt { x: f32, y: f32, z: f32 },
}

/// Register Camera API
fn register_camera(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let camera = lua.create_table()?;

    // State table (updated from Rust each frame)
    let state = lua.create_table()?;
    state.set("position", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 5.0)?;
        t.set("z", 10.0)?;
        t
    })?;
    state.set("rotation", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 0.0)?;
        t.set("w", 1.0)?;
        t
    })?;
    // Z-up 좌표계 (Blender 호환)
    state.set("forward", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", -1.0)?;  // Z-up: forward is -Y
        t.set("z", 0.0)?;
        t
    })?;
    state.set("right", {
        let t = lua.create_table()?;
        t.set("x", 1.0)?;
        t.set("y", 0.0)?;
        t.set("z", 0.0)?;
        t
    })?;
    state.set("up", {
        let t = lua.create_table()?;
        t.set("x", 0.0)?;
        t.set("y", 0.0)?;
        t.set("z", 1.0)?;  // Z-up
        t
    })?;
    state.set("yaw", 0.0)?;
    state.set("pitch", 0.0)?;
    camera.set("_state", state)?;

    // Command queue
    camera.set("_command_queue", lua.create_table()?)?;

    // Camera.get_position()
    camera.set("get_position", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("position")?)
    })?)?;

    // Camera.get_rotation()
    camera.set("get_rotation", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("rotation")?)
    })?)?;

    // Camera.get_forward()
    camera.set("get_forward", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("forward")?)
    })?)?;

    // Camera.get_right()
    camera.set("get_right", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("right")?)
    })?)?;

    // Camera.get_up()
    camera.set("get_up", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<Table>("up")?)
    })?)?;

    // Camera.get_yaw()
    camera.set("get_yaw", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<f32>("yaw")?)
    })?)?;

    // Camera.get_pitch()
    camera.set("get_pitch", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let state: Table = camera.get("_state")?;
        Ok(state.get::<f32>("pitch")?)
    })?)?;

    // Camera.set_position(pos)
    camera.set("set_position", lua.create_function(|lua, pos: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let queue: Table = camera.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_position")?;
        cmd.set("x", pos.get::<f32>("x")?)?;
        cmd.set("y", pos.get::<f32>("y")?)?;
        cmd.set("z", pos.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Camera.set_yaw_pitch(yaw, pitch)
    camera.set("set_yaw_pitch", lua.create_function(|lua, (yaw, pitch): (f32, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let queue: Table = camera.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_yaw_pitch")?;
        cmd.set("yaw", yaw)?;
        cmd.set("pitch", pitch)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Camera.look_at(target)
    camera.set("look_at", lua.create_function(|lua, target: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let camera: Table = skope.get("Camera")?;
        let queue: Table = camera.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "look_at")?;
        cmd.set("x", target.get::<f32>("x")?)?;
        cmd.set("y", target.get::<f32>("y")?)?;
        cmd.set("z", target.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Camera", camera)?;
    Ok(())
}

/// Update camera state from Rust
pub fn update_camera_state(
    lua: &Lua,
    position: (f32, f32, f32),
    rotation: (f32, f32, f32, f32),
    forward: (f32, f32, f32),
    right: (f32, f32, f32),
    up: (f32, f32, f32),
    yaw: f32,
    pitch: f32,
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let camera: Table = skope.get("Camera")?;
    let state: Table = camera.get("_state")?;

    // Update position
    let pos = lua.create_table()?;
    pos.set("x", position.0)?;
    pos.set("y", position.1)?;
    pos.set("z", position.2)?;
    state.set("position", pos)?;

    // Update rotation
    let rot = lua.create_table()?;
    rot.set("x", rotation.0)?;
    rot.set("y", rotation.1)?;
    rot.set("z", rotation.2)?;
    rot.set("w", rotation.3)?;
    state.set("rotation", rot)?;

    // Update forward
    let fwd = lua.create_table()?;
    fwd.set("x", forward.0)?;
    fwd.set("y", forward.1)?;
    fwd.set("z", forward.2)?;
    state.set("forward", fwd)?;

    // Update right
    let rgt = lua.create_table()?;
    rgt.set("x", right.0)?;
    rgt.set("y", right.1)?;
    rgt.set("z", right.2)?;
    state.set("right", rgt)?;

    // Update up
    let u = lua.create_table()?;
    u.set("x", up.0)?;
    u.set("y", up.1)?;
    u.set("z", up.2)?;
    state.set("up", u)?;

    // Update yaw/pitch
    state.set("yaw", yaw)?;
    state.set("pitch", pitch)?;

    Ok(())
}

/// Process camera commands from Lua
pub fn process_camera_commands(lua: &Lua) -> LuaResult<Vec<CameraCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let camera: Table = skope.get("Camera")?;
    let queue: Table = camera.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "set_position" => Some(CameraCommand::SetPosition {
                    x: cmd.get("x").unwrap_or(0.0),
                    y: cmd.get("y").unwrap_or(0.0),
                    z: cmd.get("z").unwrap_or(0.0),
                }),
                "set_rotation" => Some(CameraCommand::SetRotation {
                    x: cmd.get("x").unwrap_or(0.0),
                    y: cmd.get("y").unwrap_or(0.0),
                    z: cmd.get("z").unwrap_or(0.0),
                    w: cmd.get("w").unwrap_or(1.0),
                }),
                "set_yaw_pitch" => Some(CameraCommand::SetYawPitch {
                    yaw: cmd.get("yaw").unwrap_or(0.0),
                    pitch: cmd.get("pitch").unwrap_or(0.0),
                }),
                "look_at" => Some(CameraCommand::LookAt {
                    x: cmd.get("x").unwrap_or(0.0),
                    y: cmd.get("y").unwrap_or(0.0),
                    z: cmd.get("z").unwrap_or(0.0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    camera.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

// ============ Physics API ============

/// Raycast hit result
#[derive(Debug, Clone)]
pub struct RaycastHit {
    pub hit: bool,
    pub entity_id: Option<u64>,
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub distance: f32,
}

/// Physics command enum
#[derive(Debug, Clone)]
pub enum PhysicsCommand {
    ApplyForce { entity_id: u64, force: (f32, f32, f32) },
    ApplyImpulse { entity_id: u64, impulse: (f32, f32, f32) },
    SetVelocity { entity_id: u64, velocity: (f32, f32, f32) },
    SetAngularVelocity { entity_id: u64, velocity: (f32, f32, f32) },
}

/// Register Physics API
fn register_physics(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let physics = lua.create_table()?;

    // Command queue
    physics.set("_command_queue", lua.create_table()?)?;

    // Raycast results (populated by Rust before script runs)
    physics.set("_raycast_results", lua.create_table()?)?;
    physics.set("_raycast_pending", lua.create_table()?)?;

    // Physics.raycast(from, to) -> hit_info or nil
    // Note: This queues a raycast request and returns the result from previous frame
    physics.set("raycast", lua.create_function(|lua, (from, to): (Table, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let pending: Table = physics.get("_raycast_pending")?;

        // Queue raycast request
        let req = lua.create_table()?;
        req.set("from_x", from.get::<f32>("x")?)?;
        req.set("from_y", from.get::<f32>("y")?)?;
        req.set("from_z", from.get::<f32>("z")?)?;
        req.set("to_x", to.get::<f32>("x")?)?;
        req.set("to_y", to.get::<f32>("y")?)?;
        req.set("to_z", to.get::<f32>("z")?)?;

        let len = pending.len()? as i64;
        pending.set(len + 1, req)?;

        // Return last result if available
        let results: Table = physics.get("_raycast_results")?;
        if let Ok(result) = results.get::<Table>(1) {
            let hit: bool = result.get("hit").unwrap_or(false);
            if hit {
                return Ok(Some(result));
            }
        }
        Ok(None)
    })?)?;

    // Physics.apply_force(entity_id, force)
    physics.set("apply_force", lua.create_function(|lua, (entity_id, force): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let queue: Table = physics.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "apply_force")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("x", force.get::<f32>("x")?)?;
        cmd.set("y", force.get::<f32>("y")?)?;
        cmd.set("z", force.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Physics.apply_impulse(entity_id, impulse)
    physics.set("apply_impulse", lua.create_function(|lua, (entity_id, impulse): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let queue: Table = physics.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "apply_impulse")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("x", impulse.get::<f32>("x")?)?;
        cmd.set("y", impulse.get::<f32>("y")?)?;
        cmd.set("z", impulse.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Physics.set_velocity(entity_id, velocity)
    physics.set("set_velocity", lua.create_function(|lua, (entity_id, velocity): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let physics: Table = skope.get("Physics")?;
        let queue: Table = physics.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_velocity")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("x", velocity.get::<f32>("x")?)?;
        cmd.set("y", velocity.get::<f32>("y")?)?;
        cmd.set("z", velocity.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Physics", physics)?;
    Ok(())
}

/// Process physics commands from Lua
pub fn process_physics_commands(lua: &Lua) -> LuaResult<Vec<PhysicsCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let physics: Table = skope.get("Physics")?;
    let queue: Table = physics.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "apply_force" => Some(PhysicsCommand::ApplyForce {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    force: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "apply_impulse" => Some(PhysicsCommand::ApplyImpulse {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    impulse: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "set_velocity" => Some(PhysicsCommand::SetVelocity {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    velocity: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "set_angular_velocity" => Some(PhysicsCommand::SetAngularVelocity {
                    entity_id: cmd.get("entity_id").unwrap_or(0),
                    velocity: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    physics.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Set raycast results for Lua to read
pub fn set_raycast_results(lua: &Lua, results: &[RaycastHit]) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let physics: Table = skope.get("Physics")?;

    let results_table = lua.create_table()?;

    for (i, hit) in results.iter().enumerate() {
        let t = lua.create_table()?;
        t.set("hit", hit.hit)?;
        if let Some(eid) = hit.entity_id {
            t.set("entity_id", eid)?;
        }
        let pos = lua.create_table()?;
        pos.set("x", hit.position.0)?;
        pos.set("y", hit.position.1)?;
        pos.set("z", hit.position.2)?;
        t.set("position", pos)?;

        let normal = lua.create_table()?;
        normal.set("x", hit.normal.0)?;
        normal.set("y", hit.normal.1)?;
        normal.set("z", hit.normal.2)?;
        t.set("normal", normal)?;

        t.set("distance", hit.distance)?;

        results_table.set(i + 1, t)?;
    }

    physics.set("_raycast_results", results_table)?;

    // Clear pending
    physics.set("_raycast_pending", lua.create_table()?)?;

    Ok(())
}

// ============ Particles API ============

/// Particles command enum
#[derive(Debug, Clone)]
pub enum ParticlesCommand {
    Emit { effect_name: String, position: (f32, f32, f32), count: Option<u32> },
    EmitPreset { preset: String, position: (f32, f32, f32) },
    Stop { effect_id: u64 },
    StopAll,
    SetPosition { effect_id: u64, position: (f32, f32, f32) },
    SetEmissionRate { effect_id: u64, rate: f32 },
}

/// Register Particles API
fn register_particles(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let particles = lua.create_table()?;

    // Command queue
    particles.set("_command_queue", lua.create_table()?)?;

    // Next effect ID counter
    particles.set("_next_id", 1u64)?;

    // Particles.emit(effect_name, position, [count]) -> effect_id
    particles.set("emit", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let mut iter = args.into_iter();

        let effect_name: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let position = iter.next()
            .and_then(|v| v.as_table().cloned())
            .unwrap_or_else(|| lua.create_table().unwrap());

        let count: Option<u32> = iter.next()
            .and_then(|v| v.as_number().map(|n| n as u32));

        // Get and increment effect ID
        let effect_id: u64 = particles.get("_next_id")?;
        particles.set("_next_id", effect_id + 1)?;

        let cmd = lua.create_table()?;
        cmd.set("type", "emit")?;
        cmd.set("effect_name", effect_name)?;
        cmd.set("x", position.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", position.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", position.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("count", count)?;
        cmd.set("effect_id", effect_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;

        Ok(effect_id)
    })?)?;

    // Particles.emit_preset(preset, position) -> effect_id
    particles.set("emit_preset", lua.create_function(|lua, (preset, position): (String, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let effect_id: u64 = particles.get("_next_id")?;
        particles.set("_next_id", effect_id + 1)?;

        let cmd = lua.create_table()?;
        cmd.set("type", "emit_preset")?;
        cmd.set("preset", preset)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;
        cmd.set("effect_id", effect_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;

        Ok(effect_id)
    })?)?;

    // Particles.stop(effect_id)
    particles.set("stop", lua.create_function(|lua, effect_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("effect_id", effect_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Particles.stop_all()
    particles.set("stop_all", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop_all")?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Particles.set_position(effect_id, position)
    particles.set("set_position", lua.create_function(|lua, (effect_id, position): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let particles: Table = skope.get("Particles")?;
        let queue: Table = particles.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_position")?;
        cmd.set("effect_id", effect_id)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Particles", particles)?;
    Ok(())
}

/// Process particles commands from Lua
pub fn process_particles_commands(lua: &Lua) -> LuaResult<Vec<ParticlesCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let particles: Table = skope.get("Particles")?;
    let queue: Table = particles.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "emit" => Some(ParticlesCommand::Emit {
                    effect_name: cmd.get("effect_name").unwrap_or_default(),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    count: cmd.get("count").ok(),
                }),
                "emit_preset" => Some(ParticlesCommand::EmitPreset {
                    preset: cmd.get("preset").unwrap_or_default(),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "stop" => Some(ParticlesCommand::Stop {
                    effect_id: cmd.get("effect_id").unwrap_or(0),
                }),
                "stop_all" => Some(ParticlesCommand::StopAll),
                "set_position" => Some(ParticlesCommand::SetPosition {
                    effect_id: cmd.get("effect_id").unwrap_or(0),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                "set_emission_rate" => Some(ParticlesCommand::SetEmissionRate {
                    effect_id: cmd.get("effect_id").unwrap_or(0),
                    rate: cmd.get("rate").unwrap_or(1.0),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    particles.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

// ============ Lighting API ============

/// Lighting command enum
#[derive(Debug, Clone)]
pub enum LightingCommand {
    SetSunDirection { x: f32, y: f32, z: f32 },
    SetSunColor { r: f32, g: f32, b: f32 },
    SetSunIntensity { intensity: f32 },
    SetAmbientColor { r: f32, g: f32, b: f32 },
    SetAmbientIntensity { intensity: f32 },
    CreatePointLight { position: (f32, f32, f32), color: (f32, f32, f32), intensity: f32, radius: f32 },
    DestroyLight { light_id: u64 },
    SetLightEnabled { light_id: u64, enabled: bool },
    SetLightIntensity { light_id: u64, intensity: f32 },
    SetLightPosition { light_id: u64, position: (f32, f32, f32) },
}

/// Register Lighting API
fn register_lighting(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let lighting = lua.create_table()?;

    // State table
    let state = lua.create_table()?;
    state.set("sun_direction", {
        let t = lua.create_table()?;
        t.set("x", -0.5)?;
        t.set("y", -1.0)?;
        t.set("z", -0.5)?;
        t
    })?;
    state.set("sun_color", {
        let t = lua.create_table()?;
        t.set("r", 1.0)?;
        t.set("g", 0.95)?;
        t.set("b", 0.85)?;
        t
    })?;
    state.set("sun_intensity", 1.0)?;
    state.set("ambient_color", {
        let t = lua.create_table()?;
        t.set("r", 0.2)?;
        t.set("g", 0.25)?;
        t.set("b", 0.3)?;
        t
    })?;
    state.set("ambient_intensity", 0.3)?;
    lighting.set("_state", state)?;

    // Command queue
    lighting.set("_command_queue", lua.create_table()?)?;

    // Next light ID
    lighting.set("_next_light_id", 1u64)?;

    // Lighting.get_sun_direction()
    lighting.set("get_sun_direction", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let state: Table = lighting.get("_state")?;
        Ok(state.get::<Table>("sun_direction")?)
    })?)?;

    // Lighting.set_sun_direction(direction)
    lighting.set("set_sun_direction", lua.create_function(|lua, direction: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sun_direction")?;
        cmd.set("x", direction.get::<f32>("x")?)?;
        cmd.set("y", direction.get::<f32>("y")?)?;
        cmd.set("z", direction.get::<f32>("z")?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_sun_color(color)
    lighting.set("set_sun_color", lua.create_function(|lua, color: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sun_color")?;
        cmd.set("r", color.get::<f32>("r").or_else(|_| color.get::<f32>("x"))?)?;
        cmd.set("g", color.get::<f32>("g").or_else(|_| color.get::<f32>("y"))?)?;
        cmd.set("b", color.get::<f32>("b").or_else(|_| color.get::<f32>("z"))?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_sun_intensity(intensity)
    lighting.set("set_sun_intensity", lua.create_function(|lua, intensity: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sun_intensity")?;
        cmd.set("intensity", intensity)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_ambient_color(color)
    lighting.set("set_ambient_color", lua.create_function(|lua, color: Table| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_ambient_color")?;
        cmd.set("r", color.get::<f32>("r").or_else(|_| color.get::<f32>("x"))?)?;
        cmd.set("g", color.get::<f32>("g").or_else(|_| color.get::<f32>("y"))?)?;
        cmd.set("b", color.get::<f32>("b").or_else(|_| color.get::<f32>("z"))?)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_ambient_intensity(intensity)
    lighting.set("set_ambient_intensity", lua.create_function(|lua, intensity: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_ambient_intensity")?;
        cmd.set("intensity", intensity)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.create_point_light(position, color, intensity, radius) -> light_id
    lighting.set("create_point_light", lua.create_function(|lua, (position, color, intensity, radius): (Table, Table, f32, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let light_id: u64 = lighting.get("_next_light_id")?;
        lighting.set("_next_light_id", light_id + 1)?;

        let cmd = lua.create_table()?;
        cmd.set("type", "create_point_light")?;
        cmd.set("light_id", light_id)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;
        cmd.set("r", color.get::<f32>("r").or_else(|_| color.get::<f32>("x"))?)?;
        cmd.set("g", color.get::<f32>("g").or_else(|_| color.get::<f32>("y"))?)?;
        cmd.set("b", color.get::<f32>("b").or_else(|_| color.get::<f32>("z"))?)?;
        cmd.set("intensity", intensity)?;
        cmd.set("radius", radius)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;

        Ok(light_id)
    })?)?;

    // Lighting.destroy_light(light_id)
    lighting.set("destroy_light", lua.create_function(|lua, light_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "destroy_light")?;
        cmd.set("light_id", light_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Lighting.set_light_enabled(light_id, enabled)
    lighting.set("set_light_enabled", lua.create_function(|lua, (light_id, enabled): (u64, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let lighting: Table = skope.get("Lighting")?;
        let queue: Table = lighting.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_light_enabled")?;
        cmd.set("light_id", light_id)?;
        cmd.set("enabled", enabled)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Lighting", lighting)?;
    Ok(())
}

/// Update lighting state from Rust
pub fn update_lighting_state(
    lua: &Lua,
    sun_dir: (f32, f32, f32),
    sun_color: (f32, f32, f32),
    sun_intensity: f32,
    ambient_color: (f32, f32, f32),
    ambient_intensity: f32,
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let lighting: Table = skope.get("Lighting")?;
    let state: Table = lighting.get("_state")?;

    let sun_d = lua.create_table()?;
    sun_d.set("x", sun_dir.0)?;
    sun_d.set("y", sun_dir.1)?;
    sun_d.set("z", sun_dir.2)?;
    state.set("sun_direction", sun_d)?;

    let sun_c = lua.create_table()?;
    sun_c.set("r", sun_color.0)?;
    sun_c.set("g", sun_color.1)?;
    sun_c.set("b", sun_color.2)?;
    state.set("sun_color", sun_c)?;

    state.set("sun_intensity", sun_intensity)?;

    let amb_c = lua.create_table()?;
    amb_c.set("r", ambient_color.0)?;
    amb_c.set("g", ambient_color.1)?;
    amb_c.set("b", ambient_color.2)?;
    state.set("ambient_color", amb_c)?;

    state.set("ambient_intensity", ambient_intensity)?;

    Ok(())
}

/// Process lighting commands from Lua
pub fn process_lighting_commands(lua: &Lua) -> LuaResult<Vec<LightingCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let lighting: Table = skope.get("Lighting")?;
    let queue: Table = lighting.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "set_sun_direction" => Some(LightingCommand::SetSunDirection {
                    x: cmd.get("x").unwrap_or(0.0),
                    y: cmd.get("y").unwrap_or(-1.0),
                    z: cmd.get("z").unwrap_or(0.0),
                }),
                "set_sun_color" => Some(LightingCommand::SetSunColor {
                    r: cmd.get("r").unwrap_or(1.0),
                    g: cmd.get("g").unwrap_or(1.0),
                    b: cmd.get("b").unwrap_or(1.0),
                }),
                "set_sun_intensity" => Some(LightingCommand::SetSunIntensity {
                    intensity: cmd.get("intensity").unwrap_or(1.0),
                }),
                "set_ambient_color" => Some(LightingCommand::SetAmbientColor {
                    r: cmd.get("r").unwrap_or(0.2),
                    g: cmd.get("g").unwrap_or(0.2),
                    b: cmd.get("b").unwrap_or(0.2),
                }),
                "set_ambient_intensity" => Some(LightingCommand::SetAmbientIntensity {
                    intensity: cmd.get("intensity").unwrap_or(0.3),
                }),
                "create_point_light" => Some(LightingCommand::CreatePointLight {
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    color: (
                        cmd.get("r").unwrap_or(1.0),
                        cmd.get("g").unwrap_or(1.0),
                        cmd.get("b").unwrap_or(1.0),
                    ),
                    intensity: cmd.get("intensity").unwrap_or(1.0),
                    radius: cmd.get("radius").unwrap_or(10.0),
                }),
                "destroy_light" => Some(LightingCommand::DestroyLight {
                    light_id: cmd.get("light_id").unwrap_or(0),
                }),
                "set_light_enabled" => Some(LightingCommand::SetLightEnabled {
                    light_id: cmd.get("light_id").unwrap_or(0),
                    enabled: cmd.get("enabled").unwrap_or(true),
                }),
                "set_light_intensity" => Some(LightingCommand::SetLightIntensity {
                    light_id: cmd.get("light_id").unwrap_or(0),
                    intensity: cmd.get("intensity").unwrap_or(1.0),
                }),
                "set_light_position" => Some(LightingCommand::SetLightPosition {
                    light_id: cmd.get("light_id").unwrap_or(0),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    lighting.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

// ============ Animation API ============

/// SKOPE.Animation API
/// AI/스크립트에서 애니메이션 제어
fn register_animation(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let animation = lua.create_table()?;

    // 애니메이션 상태 저장소 (entity_id -> animation_data)
    let state = lua.create_table()?;
    animation.set("_state", state)?;

    // 명령 큐
    animation.set("_command_queue", lua.create_table()?)?;

    // Animation.play(entity_id, clip_index_or_name)
    // 애니메이션 재생 시작
    animation.set("play", lua.create_function(|lua, (entity_id, clip): (u64, mlua::Value)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "play")?;
        cmd.set("entity_id", entity_id)?;

        // clip can be index (number) or name (string)
        match clip {
            mlua::Value::Integer(i) => cmd.set("clip_index", i)?,
            mlua::Value::Number(n) => cmd.set("clip_index", n as i64)?,
            mlua::Value::String(s) => cmd.set("clip_name", s.to_str()?.to_string())?,
            _ => cmd.set("clip_index", 0i64)?,
        }

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.stop(entity_id)
    // 애니메이션 정지 및 시간 초기화
    animation.set("stop", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("entity_id", entity_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.pause(entity_id)
    // 애니메이션 일시 정지
    animation.set("pause", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "pause")?;
        cmd.set("entity_id", entity_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.resume(entity_id)
    // 일시 정지된 애니메이션 재개
    animation.set("resume", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "resume")?;
        cmd.set("entity_id", entity_id)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_speed(entity_id, speed)
    // 재생 속도 설정 (1.0 = 정상, 2.0 = 2배속, 0.5 = 절반 속도)
    animation.set("set_speed", lua.create_function(|lua, (entity_id, speed): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_speed")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("speed", speed)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_time(entity_id, time)
    // 재생 시간 설정 (초 단위)
    animation.set("set_time", lua.create_function(|lua, (entity_id, time): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_time")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("time", time)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_normalized_time(entity_id, t)
    // 정규화된 시간 설정 (0.0 = 시작, 1.0 = 끝)
    animation.set("set_normalized_time", lua.create_function(|lua, (entity_id, t): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_normalized_time")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("normalized_time", t.clamp(0.0, 1.0))?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.set_looping(entity_id, looping)
    // 루프 재생 설정
    animation.set("set_looping", lua.create_function(|lua, (entity_id, looping): (u64, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_looping")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("looping", looping)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.crossfade(entity_id, clip, duration)
    // 현재 애니메이션에서 다른 애니메이션으로 크로스페이드
    animation.set("crossfade", lua.create_function(|lua, (entity_id, clip, duration): (u64, mlua::Value, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let queue: Table = animation.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "crossfade")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("duration", duration)?;

        match clip {
            mlua::Value::Integer(i) => cmd.set("clip_index", i)?,
            mlua::Value::Number(n) => cmd.set("clip_index", n as i64)?,
            mlua::Value::String(s) => cmd.set("clip_name", s.to_str()?.to_string())?,
            _ => cmd.set("clip_index", 0i64)?,
        }

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animation.get_clip_count(entity_id) -> number or nil
    // 엔티티의 애니메이션 클립 개수
    animation.set("get_clip_count", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let count: Option<u32> = entity_data.get("clip_count").ok();
            Ok(count)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_clip_names(entity_id) -> table or nil
    // 엔티티의 애니메이션 클립 이름 목록
    animation.set("get_clip_names", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let names: Option<Table> = entity_data.get("clip_names").ok();
            Ok(names)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_current_clip(entity_id) -> number or nil
    // 현재 재생 중인 클립 인덱스
    animation.set("get_current_clip", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let clip: Option<u32> = entity_data.get("current_clip").ok();
            Ok(clip)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_time(entity_id) -> number or nil
    // 현재 재생 시간 (초)
    animation.set("get_time", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let time: Option<f32> = entity_data.get("current_time").ok();
            Ok(time)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.get_normalized_time(entity_id) -> number or nil
    // 정규화된 재생 시간 (0.0 ~ 1.0)
    animation.set("get_normalized_time", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let progress: Option<f32> = entity_data.get("progress").ok();
            Ok(progress)
        } else {
            Ok(None)
        }
    })?)?;

    // Animation.is_playing(entity_id) -> boolean
    // 재생 중인지 확인
    animation.set("is_playing", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let playing: bool = entity_data.get("playing").unwrap_or(false);
            Ok(playing)
        } else {
            Ok(false)
        }
    })?)?;

    // Animation.get_duration(entity_id) -> number or nil
    // 현재 클립 길이 (초)
    animation.set("get_duration", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animation: Table = skope.get("Animation")?;
        let state: Table = animation.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let duration: Option<f32> = entity_data.get("duration").ok();
            Ok(duration)
        } else {
            Ok(None)
        }
    })?)?;

    skope.set("Animation", animation)?;
    Ok(())
}

// ============ Animator API ============

/// SKOPE.Animator API
/// 상태 머신 기반 애니메이션 제어 (Unity Animator 스타일)
fn register_animator(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let animator = lua.create_table()?;

    // Animator 상태 저장소
    let state = lua.create_table()?;
    animator.set("_state", state)?;

    // 명령 큐
    animator.set("_command_queue", lua.create_table()?)?;

    // Animator.set_bool(entity_id, param_name, value)
    animator.set("set_bool", lua.create_function(|lua, (entity_id, name, value): (u64, String, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_bool")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;
        cmd.set("value", value)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_float(entity_id, param_name, value)
    animator.set("set_float", lua.create_function(|lua, (entity_id, name, value): (u64, String, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_float")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;
        cmd.set("value", value)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_int(entity_id, param_name, value)
    animator.set("set_int", lua.create_function(|lua, (entity_id, name, value): (u64, String, i32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_int")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;
        cmd.set("value", value)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_trigger(entity_id, param_name)
    animator.set("set_trigger", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_trigger")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.reset_trigger(entity_id, param_name)
    animator.set("reset_trigger", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "reset_trigger")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("param_name", name)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_speed(entity_id, speed)
    animator.set("set_speed", lua.create_function(|lua, (entity_id, speed): (u64, f32)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_speed")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("speed", speed)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.set_enabled(entity_id, enabled)
    animator.set("set_enabled", lua.create_function(|lua, (entity_id, enabled): (u64, bool)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let queue: Table = animator.get("_command_queue")?;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_enabled")?;
        cmd.set("entity_id", entity_id)?;
        cmd.set("enabled", enabled)?;

        let len = queue.len()? as i64;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Animator.get_bool(entity_id, param_name) -> boolean or nil
    animator.set("get_bool", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            if let Ok(params) = entity_data.get::<Table>("parameters") {
                if let Ok(param) = params.get::<Table>(name) {
                    if param.get::<String>("type").unwrap_or_default() == "Bool" {
                        return Ok(param.get::<bool>("value").ok());
                    }
                }
            }
        }
        Ok(None)
    })?)?;

    // Animator.get_float(entity_id, param_name) -> number or nil
    animator.set("get_float", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            if let Ok(params) = entity_data.get::<Table>("parameters") {
                if let Ok(param) = params.get::<Table>(name) {
                    if param.get::<String>("type").unwrap_or_default() == "Float" {
                        return Ok(param.get::<f32>("value").ok());
                    }
                }
            }
        }
        Ok(None)
    })?)?;

    // Animator.get_int(entity_id, param_name) -> integer or nil
    animator.set("get_int", lua.create_function(|lua, (entity_id, name): (u64, String)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            if let Ok(params) = entity_data.get::<Table>("parameters") {
                if let Ok(param) = params.get::<Table>(name) {
                    if param.get::<String>("type").unwrap_or_default() == "Int" {
                        return Ok(param.get::<i32>("value").ok());
                    }
                }
            }
        }
        Ok(None)
    })?)?;

    // Animator.get_current_state(entity_id) -> number or nil
    animator.set("get_current_state", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let current: Option<u32> = entity_data.get("current_state").ok();
            Ok(current)
        } else {
            Ok(None)
        }
    })?)?;

    // Animator.get_parameters(entity_id) -> table or nil
    // 모든 파라미터 목록 반환
    animator.set("get_parameters", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let params: Option<Table> = entity_data.get("parameters").ok();
            Ok(params)
        } else {
            Ok(None)
        }
    })?)?;

    // Animator.is_enabled(entity_id) -> boolean
    animator.set("is_enabled", lua.create_function(|lua, entity_id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let animator: Table = skope.get("Animator")?;
        let state: Table = animator.get("_state")?;

        if let Ok(entity_data) = state.get::<Table>(entity_id) {
            let enabled: bool = entity_data.get("enabled").unwrap_or(true);
            Ok(enabled)
        } else {
            Ok(false)
        }
    })?)?;

    skope.set("Animator", animator)?;
    Ok(())
}

// ============ Animation Command Processing ============

/// 애니메이션 명령 타입
#[derive(Debug, Clone)]
pub enum AnimationCommand {
    Play {
        entity_id: u64,
        clip_index: Option<usize>,
        clip_name: Option<String>,
    },
    Stop {
        entity_id: u64,
    },
    Pause {
        entity_id: u64,
    },
    Resume {
        entity_id: u64,
    },
    SetSpeed {
        entity_id: u64,
        speed: f32,
    },
    SetTime {
        entity_id: u64,
        time: f32,
    },
    SetNormalizedTime {
        entity_id: u64,
        normalized_time: f32,
    },
    SetLooping {
        entity_id: u64,
        looping: bool,
    },
    Crossfade {
        entity_id: u64,
        clip_index: Option<usize>,
        clip_name: Option<String>,
        duration: f32,
    },
}

/// Animator 명령 타입
#[derive(Debug, Clone)]
pub enum AnimatorCommand {
    SetBool {
        entity_id: u64,
        param_name: String,
        value: bool,
    },
    SetFloat {
        entity_id: u64,
        param_name: String,
        value: f32,
    },
    SetInt {
        entity_id: u64,
        param_name: String,
        value: i32,
    },
    SetTrigger {
        entity_id: u64,
        param_name: String,
    },
    ResetTrigger {
        entity_id: u64,
        param_name: String,
    },
    SetSpeed {
        entity_id: u64,
        speed: f32,
    },
    SetEnabled {
        entity_id: u64,
        enabled: bool,
    },
}

/// Animation 명령 처리 (Lua → Rust)
pub fn process_animation_commands(lua: &Lua) -> LuaResult<Vec<AnimationCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animation: Table = skope.get("Animation")?;
    let queue: Table = animation.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();
            let entity_id: u64 = cmd.get("entity_id").unwrap_or(0);

            let command = match cmd_type.as_str() {
                "play" => Some(AnimationCommand::Play {
                    entity_id,
                    clip_index: cmd.get::<i64>("clip_index").ok().map(|i| i as usize),
                    clip_name: cmd.get::<String>("clip_name").ok(),
                }),
                "stop" => Some(AnimationCommand::Stop { entity_id }),
                "pause" => Some(AnimationCommand::Pause { entity_id }),
                "resume" => Some(AnimationCommand::Resume { entity_id }),
                "set_speed" => Some(AnimationCommand::SetSpeed {
                    entity_id,
                    speed: cmd.get("speed").unwrap_or(1.0),
                }),
                "set_time" => Some(AnimationCommand::SetTime {
                    entity_id,
                    time: cmd.get("time").unwrap_or(0.0),
                }),
                "set_normalized_time" => Some(AnimationCommand::SetNormalizedTime {
                    entity_id,
                    normalized_time: cmd.get("normalized_time").unwrap_or(0.0),
                }),
                "set_looping" => Some(AnimationCommand::SetLooping {
                    entity_id,
                    looping: cmd.get("looping").unwrap_or(true),
                }),
                "crossfade" => Some(AnimationCommand::Crossfade {
                    entity_id,
                    clip_index: cmd.get::<i64>("clip_index").ok().map(|i| i as usize),
                    clip_name: cmd.get::<String>("clip_name").ok(),
                    duration: cmd.get("duration").unwrap_or(0.25),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    animation.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Animator 명령 처리 (Lua → Rust)
pub fn process_animator_commands(lua: &Lua) -> LuaResult<Vec<AnimatorCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animator: Table = skope.get("Animator")?;
    let queue: Table = animator.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();
            let entity_id: u64 = cmd.get("entity_id").unwrap_or(0);

            let command = match cmd_type.as_str() {
                "set_bool" => Some(AnimatorCommand::SetBool {
                    entity_id,
                    param_name: cmd.get("param_name").unwrap_or_default(),
                    value: cmd.get("value").unwrap_or(false),
                }),
                "set_float" => Some(AnimatorCommand::SetFloat {
                    entity_id,
                    param_name: cmd.get("param_name").unwrap_or_default(),
                    value: cmd.get("value").unwrap_or(0.0),
                }),
                "set_int" => Some(AnimatorCommand::SetInt {
                    entity_id,
                    param_name: cmd.get("param_name").unwrap_or_default(),
                    value: cmd.get("value").unwrap_or(0),
                }),
                "set_trigger" => Some(AnimatorCommand::SetTrigger {
                    entity_id,
                    param_name: cmd.get("param_name").unwrap_or_default(),
                }),
                "reset_trigger" => Some(AnimatorCommand::ResetTrigger {
                    entity_id,
                    param_name: cmd.get("param_name").unwrap_or_default(),
                }),
                "set_speed" => Some(AnimatorCommand::SetSpeed {
                    entity_id,
                    speed: cmd.get("speed").unwrap_or(1.0),
                }),
                "set_enabled" => Some(AnimatorCommand::SetEnabled {
                    entity_id,
                    enabled: cmd.get("enabled").unwrap_or(true),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    animator.set("_command_queue", lua.create_table()?)?;
    Ok(commands)
}

/// Animation 상태 업데이트 (Rust → Lua)
/// 각 엔티티의 애니메이션 상태를 Lua에 동기화
pub fn update_animation_state(
    lua: &Lua,
    entities: &[(u64, AnimationStateData)],
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animation: Table = skope.get("Animation")?;
    let state: Table = animation.get("_state")?;

    for (entity_id, data) in entities {
        let entity_data = lua.create_table()?;

        entity_data.set("clip_count", data.clip_count)?;
        entity_data.set("current_clip", data.current_clip)?;
        entity_data.set("current_time", data.current_time)?;
        entity_data.set("duration", data.duration)?;
        entity_data.set("progress", data.progress)?;
        entity_data.set("speed", data.speed)?;
        entity_data.set("playing", data.playing)?;
        entity_data.set("looping", data.looping)?;

        // Clip names
        if !data.clip_names.is_empty() {
            let names = lua.create_table()?;
            for (i, name) in data.clip_names.iter().enumerate() {
                names.set(i + 1, name.clone())?;
            }
            entity_data.set("clip_names", names)?;
        }

        state.set(*entity_id, entity_data)?;
    }

    Ok(())
}

/// Animation 상태 데이터 (Rust → Lua 전달용)
#[derive(Debug, Clone, Default)]
pub struct AnimationStateData {
    pub clip_count: u32,
    pub clip_names: Vec<String>,
    pub current_clip: u32,
    pub current_time: f32,
    pub duration: f32,
    pub progress: f32,
    pub speed: f32,
    pub playing: bool,
    pub looping: bool,
}

/// Animator 상태 업데이트 (Rust → Lua)
pub fn update_animator_state(
    lua: &Lua,
    entities: &[(u64, AnimatorStateData)],
) -> LuaResult<()> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let animator: Table = skope.get("Animator")?;
    let state: Table = animator.get("_state")?;

    for (entity_id, data) in entities {
        let entity_data = lua.create_table()?;

        entity_data.set("current_state", data.current_state)?;
        entity_data.set("speed", data.speed)?;
        entity_data.set("enabled", data.enabled)?;

        // Parameters
        let params = lua.create_table()?;
        for (name, param) in &data.parameters {
            let param_data = lua.create_table()?;
            match param {
                AnimatorParamValue::Bool(v) => {
                    param_data.set("type", "Bool")?;
                    param_data.set("value", *v)?;
                }
                AnimatorParamValue::Float(v) => {
                    param_data.set("type", "Float")?;
                    param_data.set("value", *v)?;
                }
                AnimatorParamValue::Int(v) => {
                    param_data.set("type", "Int")?;
                    param_data.set("value", *v)?;
                }
                AnimatorParamValue::Trigger(v) => {
                    param_data.set("type", "Trigger")?;
                    param_data.set("value", *v)?;
                }
            }
            params.set(name.clone(), param_data)?;
        }
        entity_data.set("parameters", params)?;

        state.set(*entity_id, entity_data)?;
    }

    Ok(())
}

/// Animator 상태 데이터 (Rust → Lua 전달용)
#[derive(Debug, Clone, Default)]
pub struct AnimatorStateData {
    pub current_state: u32,
    pub speed: f32,
    pub enabled: bool,
    pub parameters: Vec<(String, AnimatorParamValue)>,
}

/// Animator 파라미터 값 (Lua 전달용)
#[derive(Debug, Clone)]
pub enum AnimatorParamValue {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger(bool),
}

// ============================================================================
// AnimatorController Lua 명령 처리 시스템
// ============================================================================

use bevy_ecs::entity::Entity;
use crate::ecs_components::{AnimatorController, AnimatorParameter};

/// Lua AnimatorCommand를 AnimatorController 컴포넌트에 적용하는 시스템
///
/// 사용법:
/// 1. 매 프레임 process_animator_commands()로 명령 수집
/// 2. apply_animator_commands_to_world()로 실제 적용
pub fn apply_animator_commands_to_world(
    world: &mut bevy_ecs::world::World,
    commands: &[AnimatorCommand],
) {
    for cmd in commands {
        match cmd {
            AnimatorCommand::SetBool { entity_id, param_name, value } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Bool(*value),
                        );
                    }
                }
            }
            AnimatorCommand::SetFloat { entity_id, param_name, value } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Float(*value),
                        );
                    }
                }
            }
            AnimatorCommand::SetInt { entity_id, param_name, value } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Int(*value),
                        );
                    }
                }
            }
            AnimatorCommand::SetTrigger { entity_id, param_name } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Trigger(true),
                        );
                    }
                }
            }
            AnimatorCommand::ResetTrigger { entity_id, param_name } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.parameters.insert(
                            param_name.clone(),
                            AnimatorParameter::Trigger(false),
                        );
                    }
                }
            }
            AnimatorCommand::SetSpeed { entity_id, speed } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.speed = *speed;
                    }
                }
            }
            AnimatorCommand::SetEnabled { entity_id, enabled } => {
                if let Ok(entity) = Entity::try_from_bits(*entity_id) {
                    if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
                        animator.enabled = *enabled;
                    }
                }
            }
        }
    }
}

/// AnimatorController 상태를 Lua로 동기화
pub fn sync_animator_controllers_to_lua(
    world: &mut bevy_ecs::world::World,
    lua: &Lua,
) -> LuaResult<()> {
    let mut entity_states = Vec::new();

    // Query all entities with AnimatorController
    let mut query = world.query::<(Entity, &AnimatorController)>();
    for (entity, animator) in query.iter(world) {
        let entity_id = entity.to_bits();

        // Convert parameters
        let params: Vec<(String, AnimatorParamValue)> = animator.parameters
            .iter()
            .map(|(name, param)| {
                let value = match param {
                    AnimatorParameter::Bool(v) => AnimatorParamValue::Bool(*v),
                    AnimatorParameter::Float(v) => AnimatorParamValue::Float(*v),
                    AnimatorParameter::Int(v) => AnimatorParamValue::Int(*v),
                    AnimatorParameter::Trigger(v) => AnimatorParamValue::Trigger(*v),
                };
                (name.clone(), value)
            })
            .collect();

        entity_states.push((entity_id, AnimatorStateData {
            current_state: animator.current_state as u32,
            speed: animator.speed,
            enabled: animator.enabled,
            parameters: params,
        }));
    }

    update_animator_state(lua, &entity_states)
}
