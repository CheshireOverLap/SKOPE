//! Module Loader — `require()` implementation
//!
//! Provides `require("skills/fireball")` → loads and caches `scripts/modules/skills/fireball.lua`.
//! Respects trust level for path access restrictions and sandbox validation.

use mlua::{Lua, Result as LuaResult, Value, Table};
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};

use super::{TrustLevel, validate_code};

/// Module cache and dependency tracking.
pub struct ModuleCache {
    /// module_name → is loaded flag (actual cache is in Lua registry)
    loaded: HashSet<String>,
    /// module_name → set of scripts that depend on it (for hot reload)
    pub dependents: HashMap<String, HashSet<PathBuf>>,
}

impl Default for ModuleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleCache {
    pub fn new() -> Self {
        Self {
            loaded: HashSet::new(),
            dependents: HashMap::new(),
        }
    }

    /// Invalidate a module (for hot reload).
    pub fn invalidate(&mut self, lua: &Lua, module_name: &str) -> LuaResult<()> {
        self.loaded.remove(module_name);

        // Clear from Lua cache (source of truth)
        if let Ok(Value::Table(cache)) = lua.named_registry_value::<Value>("__module_cache") {
            cache.set(module_name, Value::Nil)?;
        }

        Ok(())
    }

    /// Invalidate a module by file path (for hot reload).
    /// Checks Lua-side cache directly, not just the Rust-side HashSet.
    pub fn invalidate_by_path(&mut self, lua: &Lua, file_path: &Path, base_path: &Path) -> LuaResult<Vec<String>> {
        let mut invalidated = Vec::new();

        // Convert file path to module name
        if let Ok(relative) = file_path.strip_prefix(base_path) {
            let module_name = relative
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");

            // Check Lua-side cache (source of truth) instead of Rust-side HashSet
            let is_cached = if let Ok(Value::Table(cache)) = lua.named_registry_value::<Value>("__module_cache") {
                !cache.get::<Value>(module_name.as_str())
                    .map(|v| v.is_nil())
                    .unwrap_or(true)
            } else {
                false
            };

            if is_cached {
                self.invalidate(lua, &module_name)?;
                invalidated.push(module_name);
            }
        }

        Ok(invalidated)
    }

    /// Check if a module is loaded (checks Lua-side cache).
    pub fn is_loaded(&self, module_name: &str) -> bool {
        self.loaded.contains(module_name)
    }

    /// Mark a module as loaded.
    pub fn mark_loaded(&mut self, module_name: &str) {
        self.loaded.insert(module_name.to_string());
    }
}

/// Register the `require` global function.
pub fn register_require(lua: &Lua, base_path: &Path, trust_level: TrustLevel) -> LuaResult<()> {
    // Initialize module cache in Lua registry
    let cache = lua.create_table()?;
    lua.set_named_registry_value("__module_cache", cache)?;

    // Store base path and trust level in registry for the closure
    lua.set_named_registry_value("__module_base_path",
        lua.create_string(base_path.to_string_lossy().as_ref())?)?;
    lua.set_named_registry_value("__module_trust_level",
        trust_level as i32)?;

    // Register require as a global function
    let require_fn = lua.create_function(move |lua, module_name: String| {
        require_module(lua, &module_name)
    })?;

    lua.globals().set("require", require_fn)?;

    Ok(())
}

/// Load a module by name, with caching.
fn require_module(lua: &Lua, module_name: &str) -> LuaResult<Value> {
    // 1. Check cache
    let cache: Table = lua.named_registry_value("__module_cache")?;
    if let Ok(cached) = cache.get::<Value>(module_name) {
        if !cached.is_nil() {
            return Ok(cached);
        }
    }

    // 2. Validate module name (no directory traversal)
    if module_name.contains("..") {
        return Err(mlua::Error::external(format!(
            "require: directory traversal not allowed: '{}'", module_name
        )));
    }

    // 3. Get base path and trust level
    let base_path_str: String = lua.named_registry_value("__module_base_path")?;
    let trust_level_int: i32 = lua.named_registry_value("__module_trust_level")?;
    let trust_level = match trust_level_int {
        0 => TrustLevel::AiGenerated,
        1 => TrustLevel::UserScript,
        2 => TrustLevel::GameScript,
        _ => TrustLevel::Engine,
    };

    // 4. Check access restrictions
    match trust_level {
        TrustLevel::AiGenerated => {
            return Err(mlua::Error::external(
                "require: not available for AI-generated scripts"
            ));
        }
        TrustLevel::UserScript => {
            // Only modules/ subdirectory allowed
            if !module_name.starts_with("modules/") && !module_name.starts_with("skope/") {
                return Err(mlua::Error::external(format!(
                    "require: UserScript can only require from 'modules/' or 'skope/', got '{}'",
                    module_name
                )));
            }
        }
        TrustLevel::GameScript | TrustLevel::Engine => {
            // Full scripts/ access
        }
    }

    // 5. Resolve file path
    let base_path = PathBuf::from(&base_path_str);
    let file_path = resolve_module_path(&base_path, module_name);

    // 6. Read file
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| mlua::Error::external(format!(
            "require: cannot load module '{}': {}", module_name, e
        )))?;

    // 7. Validate code
    if trust_level != TrustLevel::Engine {
        if let Err(msg) = validate_code(&content, trust_level) {
            return Err(mlua::Error::external(format!(
                "require: validation failed for '{}': {}", module_name, msg
            )));
        }
    }

    // 8. Load and execute
    let chunk = lua.load(&content)
        .set_name(format!("module:{}", module_name));
    let result: Value = chunk.eval()?;

    // 9. Cache the result
    cache.set(module_name, result.clone())?;

    log::info!("[require] Loaded module: {} from {}", module_name, file_path.display());

    Ok(result)
}

/// Resolve module name to file path.
///
/// `"skills/fireball"` → `{base_path}/skills/fireball.lua`
/// `"skope/combat"` → `{base_path}/skope/combat.lua`
fn resolve_module_path(base_path: &Path, module_name: &str) -> PathBuf {
    let normalized = module_name.replace('.', "/");
    base_path.join(format!("{}.lua", normalized))
}
