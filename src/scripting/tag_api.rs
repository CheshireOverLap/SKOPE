//! Tag API for Lua
//!
//! Provides `Tag.add(entity, tag)`, `Tag.remove(entity, tag)`,
//! `Tag.has(entity, tag)`, `Tag.getTagged(tag)`.
//!
//! Uses command-queue pattern for add/remove.
//! getTagged reads from entity registry's tag data.

use mlua::{Lua, Result as LuaResult, Table, Value};
use super::entity_handle::{EntityHandle, extract_entity_bits};

/// Tag command types
#[derive(Debug, Clone)]
pub enum TagCommand {
    Add { entity_bits: u64, tag: String },
    Remove { entity_bits: u64, tag: String },
}

/// Register the Tag API onto SKOPE namespace.
pub fn register_tag_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let tag_t = lua.create_table()?;

    // Command queue
    let queue = lua.create_table()?;
    tag_t.set("_command_queue", queue)?;

    // Tag.add(entity, tag_name)
    tag_t.set("add", lua.create_function(|lua, (target, tag): (Value, String)| {
        let bits = extract_entity_bits(&target)?;
        let skope: Table = lua.globals().get("SKOPE")?;
        let tag_api: Table = skope.get("Tag")?;
        let queue: Table = tag_api.get("_command_queue")?;
        let len = queue.len()? + 1;
        let cmd = lua.create_table()?;
        cmd.set("cmd", "add")?;
        cmd.set("entity", bits as i64)?;
        cmd.set("tag", tag)?;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    // Tag.remove(entity, tag_name)
    tag_t.set("remove", lua.create_function(|lua, (target, tag): (Value, String)| {
        let bits = extract_entity_bits(&target)?;
        let skope: Table = lua.globals().get("SKOPE")?;
        let tag_api: Table = skope.get("Tag")?;
        let queue: Table = tag_api.get("_command_queue")?;
        let len = queue.len()? + 1;
        let cmd = lua.create_table()?;
        cmd.set("cmd", "remove")?;
        cmd.set("entity", bits as i64)?;
        cmd.set("tag", tag)?;
        queue.set(len, cmd)?;
        Ok(())
    })?)?;

    // Tag.has(entity, tag_name) -> bool
    tag_t.set("has", lua.create_function(|lua, (target, tag): (Value, String)| {
        let bits = extract_entity_bits(&target)?;
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        if let Ok(entity_data) = registry.get::<Table>(bits) {
            if let Ok(tags) = entity_data.get::<Table>("tags") {
                let has: bool = tags.get(tag).unwrap_or(false);
                return Ok(has);
            }
        }
        Ok(false)
    })?)?;

    // Tag.getTagged(tag_name) -> array of EntityHandle
    tag_t.set("getTagged", lua.create_function(|lua, tag: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;

        let results = lua.create_table()?;
        let mut idx = 1;

        for pair in registry.pairs::<u64, Table>() {
            if let Ok((id, data)) = pair {
                if let Ok(tags) = data.get::<Table>("tags") {
                    let has: bool = tags.get(tag.clone()).unwrap_or(false);
                    if has {
                        let handle = EntityHandle::new(id);
                        results.set(idx, lua.create_userdata(handle)?)?;
                        idx += 1;
                    }
                }
            }
        }

        Ok(results)
    })?)?;

    skope.set("Tag", tag_t)?;
    Ok(())
}

/// Process tag commands from Lua.
pub fn process_tag_commands(lua: &Lua) -> LuaResult<Vec<TagCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let tag_api: Table = skope.get("Tag")?;
    let queue: Table = tag_api.get("_command_queue")?;

    let mut commands = Vec::new();

    for i in 1..=queue.len()? {
        if let Ok(cmd) = queue.get::<Table>(i) {
            let cmd_type: String = cmd.get("cmd").unwrap_or_default();
            let entity_bits = cmd.get::<i64>("entity").unwrap_or(0) as u64;
            let tag: String = cmd.get("tag").unwrap_or_default();

            match cmd_type.as_str() {
                "add" => commands.push(TagCommand::Add { entity_bits, tag }),
                "remove" => commands.push(TagCommand::Remove { entity_bits, tag }),
                _ => {}
            }
        }
    }

    // Clear queue
    tag_api.set("_command_queue", lua.create_table()?)?;

    Ok(commands)
}

