//! EntityHandle UserData for Lua
//!
//! Roblox-style entity handle: `entity.position`, `entity.health`, `entity.OnDamaged:Connect(fn)`
//! Reads from SKOPE.Entity._registry, signals from __signals registry.

use mlua::{Lua, Result as LuaResult, Table, Value, Function};

/// EntityHandle — Lua UserData wrapping an entity ID.
///
/// Provides property-style access to entity data (position, health, etc.)
/// and lazy-created Signal objects for event subscription.
#[derive(Debug, Clone, Copy)]
pub struct EntityHandle {
    pub entity_bits: u64,
}

impl EntityHandle {
    pub fn new(entity_bits: u64) -> Self {
        Self { entity_bits }
    }

    /// Get entity data table from SKOPE.Entity._registry
    fn get_entity_data<'a>(lua: &'a Lua, entity_bits: u64) -> LuaResult<Option<Table>> {
        let skope: Table = lua.globals().get("SKOPE")?;
        let entity: Table = skope.get("Entity")?;
        let registry: Table = entity.get("_registry")?;
        registry.get::<Option<Table>>(entity_bits)
    }

    /// Get or create a signal for an entity
    fn get_or_create_signal(lua: &Lua, entity_bits: u64, signal_name: &str) -> LuaResult<Table> {
        // Get __signals registry
        let signals: Table = match lua.named_registry_value::<Value>("__signals")? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table()?;
                lua.set_named_registry_value("__signals", t.clone())?;
                t
            }
        };

        // Get or create entity-level signals table
        let entity_signals: Table = match signals.get::<Option<Table>>(entity_bits)? {
            Some(t) => t,
            None => {
                let t = lua.create_table()?;
                signals.set(entity_bits, t.clone())?;
                t
            }
        };

        // Get or create the specific signal
        match entity_signals.get::<Option<Table>>(signal_name)? {
            Some(sig) => Ok(sig),
            None => {
                // Create using Signal.new()
                let signal_class: Table = lua.named_registry_value("__Signal")?;
                let new_fn: Function = signal_class.get("new")?;
                let sig: Table = new_fn.call(())?;
                entity_signals.set(signal_name, sig.clone())?;
                Ok(sig)
            }
        }
    }
}

/// Extract entity bits from EntityHandle userdata, integer, or number.
/// Shared utility used by Combat, Tag, and Tween APIs.
pub fn extract_entity_bits(value: &Value) -> mlua::Result<u64> {
    match value {
        Value::Integer(i) => Ok(*i as u64),
        Value::Number(n) => Ok(*n as u64),
        Value::UserData(ud) => {
            let handle = ud.borrow::<EntityHandle>()?;
            Ok(handle.entity_bits)
        }
        _ => Err(mlua::Error::external("Expected entity handle or entity ID")),
    }
}

/// Extract optional entity bits (returns None for Nil or invalid values).
pub fn extract_optional_entity_bits(value: Option<Value>) -> Option<u64> {
    value.and_then(|v| extract_entity_bits(&v).ok())
}

impl mlua::UserData for EntityHandle {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        // ============ Identity ============

        fields.add_field_method_get("id", |_, this| Ok(this.entity_bits));

        fields.add_field_method_get("name", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                let name: Option<String> = data.get("name").ok();
                Ok(Value::String(lua.create_string(name.unwrap_or_default())?))
            } else {
                Ok(Value::Nil)
            }
        });

        // ============ Transform ============

        fields.add_field_method_get("position", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                if let Ok(transform) = data.get::<Table>("transform") {
                    if let Ok(pos) = transform.get::<Table>("position") {
                        return Ok(Value::Table(pos));
                    }
                }
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("rotation", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                if let Ok(transform) = data.get::<Table>("transform") {
                    if let Ok(rot) = transform.get::<Table>("rotation") {
                        return Ok(Value::Table(rot));
                    }
                }
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("scale", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                if let Ok(transform) = data.get::<Table>("transform") {
                    if let Ok(s) = transform.get::<Table>("scale") {
                        return Ok(Value::Table(s));
                    }
                }
            }
            Ok(Value::Nil)
        });

        // ============ Health ============

        fields.add_field_method_get("health", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                let v: Option<f64> = data.get("health_current").ok();
                return Ok(v.map(Value::Number).unwrap_or(Value::Nil));
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("max_health", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                let v: Option<f64> = data.get("health_max").ok();
                return Ok(v.map(Value::Number).unwrap_or(Value::Nil));
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("health_percent", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                let current: f32 = data.get("health_current").unwrap_or(0.0);
                let max: f32 = data.get("health_max").unwrap_or(1.0);
                let pct = if max > 0.0 { current / max } else { 0.0 };
                return Ok(Value::Number(pct as f64));
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("is_alive", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                let current: f32 = data.get("health_current").unwrap_or(0.0);
                let has_health: bool = data.get("has_health").unwrap_or(false);
                return Ok(Value::Boolean(!has_health || current > 0.0));
            }
            Ok(Value::Boolean(false))
        });

        // ============ Team ============

        fields.add_field_method_get("team", |lua, this| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                let t: Option<String> = data.get("team").ok();
                return match t {
                    Some(s) => Ok(Value::String(lua.create_string(&s)?)),
                    None => Ok(Value::Nil),
                };
            }
            Ok(Value::Nil)
        });

        // ============ Signals (lazy-created) ============

        fields.add_field_method_get("OnDamaged", |lua, this| {
            let sig = Self::get_or_create_signal(lua, this.entity_bits, "OnDamaged")?;
            Ok(Value::Table(sig))
        });

        fields.add_field_method_get("Died", |lua, this| {
            let sig = Self::get_or_create_signal(lua, this.entity_bits, "Died")?;
            Ok(Value::Table(sig))
        });

        fields.add_field_method_get("OnCollisionEnter", |lua, this| {
            let sig = Self::get_or_create_signal(lua, this.entity_bits, "OnCollisionEnter")?;
            Ok(Value::Table(sig))
        });

        fields.add_field_method_get("OnCollisionExit", |lua, this| {
            let sig = Self::get_or_create_signal(lua, this.entity_bits, "OnCollisionExit")?;
            Ok(Value::Table(sig))
        });

        fields.add_field_method_get("OnStatusApplied", |lua, this| {
            let sig = Self::get_or_create_signal(lua, this.entity_bits, "OnStatusApplied")?;
            Ok(Value::Table(sig))
        });

        fields.add_field_method_get("OnStatusRemoved", |lua, this| {
            let sig = Self::get_or_create_signal(lua, this.entity_bits, "OnStatusRemoved")?;
            Ok(Value::Table(sig))
        });
    }

    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // distance_to(other_entity_handle) -> number
        methods.add_method("distance_to", |lua, this, other: mlua::AnyUserData| {
            let other_handle = other.borrow::<EntityHandle>()?;

            let get_pos = |bits: u64| -> Option<(f32, f32, f32)> {
                let data = Self::get_entity_data(lua, bits).ok()??;
                let transform: Table = data.get("transform").ok()?;
                let pos: Table = transform.get("position").ok()?;
                let x: f32 = pos.get("x").ok()?;
                let y: f32 = pos.get("y").ok()?;
                let z: f32 = pos.get("z").ok()?;
                Some((x, y, z))
            };

            if let (Some((x1, y1, z1)), Some((x2, y2, z2))) =
                (get_pos(this.entity_bits), get_pos(other_handle.entity_bits))
            {
                let dx = x2 - x1;
                let dy = y2 - y1;
                let dz = z2 - z1;
                Ok(Value::Number(((dx * dx + dy * dy + dz * dz).sqrt()) as f64))
            } else {
                Ok(Value::Nil)
            }
        });

        // has_tag(tag_name) -> bool
        methods.add_method("has_tag", |lua, this, tag: String| {
            if let Some(data) = Self::get_entity_data(lua, this.entity_bits)? {
                if let Ok(tags) = data.get::<Table>("tags") {
                    let has: bool = tags.get(tag).unwrap_or(false);
                    return Ok(has);
                }
            }
            Ok(false)
        });

        // __tostring metamethod
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, ()| {
            Ok(format!("EntityHandle({})", this.entity_bits))
        });

        // __eq metamethod
        methods.add_meta_method(mlua::MetaMethod::Eq, |_, this, other: mlua::AnyUserData| {
            if let Ok(other_handle) = other.borrow::<EntityHandle>() {
                Ok(this.entity_bits == other_handle.entity_bits)
            } else {
                Ok(false)
            }
        });
    }
}
