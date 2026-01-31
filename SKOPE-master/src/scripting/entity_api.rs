//! Entity API for Lua
//!
//! Entity 조회 및 조작, 레지스트리 관리

use mlua::{Lua, Result as LuaResult, Table};

/// Entity API 등록
pub fn register_entity_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
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

        for (name, id) in name_lookup.pairs::<String, u64>().flatten() {
            if name.contains(&pattern) {
                results.set(idx, id)?;
                idx += 1;
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

        for (id, _) in registry.pairs::<u64, Table>().flatten() {
            results.set(idx, id)?;
            idx += 1;
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

/// Entity Transform 데이터 (Rust에서 Lua로 전달용)
#[derive(Debug, Clone)]
pub struct EntityTransform {
    pub position: (f32, f32, f32),
    pub rotation: (f32, f32, f32, f32),  // Quaternion (x, y, z, w)
    pub scale: (f32, f32, f32),
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
