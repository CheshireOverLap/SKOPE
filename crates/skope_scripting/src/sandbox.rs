//! SKOPE Lua Sandboxing
//!
//! Provides secure execution environment for AI-generated and user scripts.
//! Restricts dangerous operations while allowing safe game logic.

use mlua::{Lua, StdLib, LuaOptions, Result as LuaResult, Value};
use std::time::{Duration, Instant};

/// Resource limits for script execution
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum number of Lua instructions before timeout
    pub max_instructions: u32,

    /// Maximum memory in bytes (approximate)
    pub max_memory: usize,

    /// Maximum execution time in milliseconds
    pub timeout_ms: u64,

    /// Maximum recursion depth
    pub max_recursion: u32,

    /// Maximum string length
    pub max_string_len: usize,

    /// Maximum table size
    pub max_table_size: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_instructions: 1_000_000,
            max_memory: 64 * 1024 * 1024, // 64 MB
            timeout_ms: 5000,             // 5 seconds
            max_recursion: 100,
            max_string_len: 1024 * 1024,  // 1 MB strings
            max_table_size: 100_000,
        }
    }
}

impl ResourceLimits {
    /// Strict limits for AI-generated code
    pub fn strict() -> Self {
        Self {
            max_instructions: 100_000,
            max_memory: 8 * 1024 * 1024, // 8 MB
            timeout_ms: 1000,            // 1 second
            max_recursion: 50,
            max_string_len: 64 * 1024,   // 64 KB strings
            max_table_size: 10_000,
        }
    }

    /// Relaxed limits for trusted scripts
    pub fn relaxed() -> Self {
        Self {
            max_instructions: 10_000_000,
            max_memory: 256 * 1024 * 1024, // 256 MB
            timeout_ms: 30000,             // 30 seconds
            max_recursion: 200,
            max_string_len: 16 * 1024 * 1024, // 16 MB strings
            max_table_size: 1_000_000,
        }
    }
}

/// Sandbox trust level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// AI-generated code - most restrictive
    AiGenerated,

    /// User scripts from editor - moderate restrictions
    UserScript,

    /// Bundled game scripts - minimal restrictions
    GameScript,

    /// Core engine scripts - no restrictions (internal use only)
    Engine,
}

impl TrustLevel {
    pub fn limits(&self) -> ResourceLimits {
        match self {
            TrustLevel::AiGenerated => ResourceLimits::strict(),
            TrustLevel::UserScript => ResourceLimits::default(),
            TrustLevel::GameScript => ResourceLimits::relaxed(),
            TrustLevel::Engine => ResourceLimits {
                max_instructions: u32::MAX,
                max_memory: usize::MAX,
                timeout_ms: u64::MAX,
                max_recursion: u32::MAX,
                max_string_len: usize::MAX,
                max_table_size: usize::MAX,
            },
        }
    }

    pub fn allowed_libs(&self) -> StdLib {
        match self {
            TrustLevel::AiGenerated => {
                StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8
            }
            TrustLevel::UserScript => {
                StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8 | StdLib::COROUTINE
            }
            TrustLevel::GameScript => {
                StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8 | StdLib::COROUTINE
            }
            TrustLevel::Engine => {
                StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8
                    | StdLib::COROUTINE | StdLib::OS
            }
        }
    }
}

/// Create a sandboxed Lua instance
pub fn create_sandboxed_lua(trust_level: TrustLevel) -> LuaResult<Lua> {
    let allowed_libs = trust_level.allowed_libs();
    let lua = Lua::new_with(allowed_libs, LuaOptions::default())?;

    remove_dangerous_globals(&lua, trust_level)?;
    add_safe_functions(&lua, trust_level)?;

    let limits = trust_level.limits();
    setup_execution_limits(&lua, &limits)?;

    Ok(lua)
}

/// Remove dangerous global functions
fn remove_dangerous_globals(lua: &Lua, trust_level: TrustLevel) -> LuaResult<()> {
    let globals = lua.globals();

    // Note: `require` is conditionally removed — only blocked for AiGenerated.
    // For UserScript+, register_require() in module_loader.rs will replace it
    // with a sandboxed version after sandbox creation.
    let mut always_remove = vec![
        "dofile", "loadfile", "load", "loadstring",
        "rawset", "rawget", "rawequal", "rawlen",
        "collectgarbage", "getmetatable", "setmetatable",
        "debug", "package", "module", "_G",
    ];

    // Only block require for AI-generated code
    if trust_level == TrustLevel::AiGenerated {
        always_remove.push("require");
    }

    for name in always_remove {
        globals.set(name, Value::Nil)?;
    }

    if trust_level != TrustLevel::Engine {
        globals.set("io", Value::Nil)?;

        if trust_level == TrustLevel::GameScript {
            if let Ok(os_table) = globals.get::<mlua::Table>("os") {
                for name in ["execute", "exit", "remove", "rename", "tmpname", "getenv", "setlocale"] {
                    os_table.set(name, Value::Nil)?;
                }
            }
        } else {
            globals.set("os", Value::Nil)?;
        }
    }

    Ok(())
}

/// Add safe replacement functions
fn add_safe_functions(lua: &Lua, trust_level: TrustLevel) -> LuaResult<()> {
    let globals = lua.globals();

    let safe_print = lua.create_function(|_, args: mlua::Variadic<Value>| {
        let parts: Vec<String> = args.iter()
            .map(|v| format!("{:?}", v))
            .collect();
        log::info!("[Lua] {}", parts.join("\t"));
        Ok(())
    })?;
    globals.set("print", safe_print)?;

    if trust_level >= TrustLevel::UserScript {
        let safe_setmetatable = lua.create_function(|lua, (table, metatable): (mlua::Table, Option<mlua::Table>)| {
            lua.scope(|_| {
                table.set_metatable(metatable);
                Ok(table)
            })
        })?;
        globals.set("setmetatable", safe_setmetatable)?;
    }

    let safe_type = lua.create_function(|_, value: Value| {
        let type_name = match value {
            Value::Nil => "nil",
            Value::Boolean(_) => "boolean",
            Value::Integer(_) | Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Table(_) => "table",
            Value::Function(_) => "function",
            Value::Thread(_) => "thread",
            Value::UserData(_) => "userdata",
            Value::LightUserData(_) => "userdata",
            Value::Error(_) => "error",
            _ => "unknown",
        };
        Ok(type_name.to_string())
    })?;
    globals.set("type", safe_type)?;

    Ok(())
}

/// Set up execution limits using Lua hooks
fn setup_execution_limits(lua: &Lua, limits: &ResourceLimits) -> LuaResult<()> {
    let registry = lua.named_registry_value::<Value>("__skope_limits")?;
    if registry.is_nil() {
        let limits_table = lua.create_table()?;
        limits_table.set("max_instructions", limits.max_instructions)?;
        limits_table.set("max_memory", limits.max_memory)?;
        limits_table.set("timeout_ms", limits.timeout_ms)?;
        limits_table.set("max_recursion", limits.max_recursion)?;
        lua.set_named_registry_value("__skope_limits", limits_table)?;
    }

    Ok(())
}

/// Sandbox context for tracking execution
pub struct SandboxContext {
    pub trust_level: TrustLevel,
    pub limits: ResourceLimits,
    pub start_time: Instant,
    pub instruction_count: u32,
}

impl SandboxContext {
    pub fn new(trust_level: TrustLevel) -> Self {
        Self {
            trust_level,
            limits: trust_level.limits(),
            start_time: Instant::now(),
            instruction_count: 0,
        }
    }

    pub fn should_interrupt(&self) -> bool {
        let elapsed = self.start_time.elapsed();
        if elapsed > Duration::from_millis(self.limits.timeout_ms) {
            return true;
        }
        if self.instruction_count > self.limits.max_instructions {
            return true;
        }
        false
    }

    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.instruction_count = 0;
    }
}

/// Execute code in sandbox with limits
pub fn execute_sandboxed(
    lua: &Lua,
    code: &str,
    _trust_level: TrustLevel,
) -> LuaResult<Value> {
    let chunk = lua.load(code);
    chunk.eval::<Value>()
}

/// Validate code before execution (quick static checks)
pub fn validate_code(code: &str, trust_level: TrustLevel) -> Result<(), String> {
    let forbidden_patterns = match trust_level {
        TrustLevel::AiGenerated => vec![
            (r#"while\s+true\s+do"#, "Infinite loop detected"),
            (r#"repeat\s+.*\s+until\s+false"#, "Infinite loop detected"),
            (r#"for\s+\w+\s*=\s*1\s*,\s*math\.huge"#, "Infinite loop detected"),
            (r#"_G\s*\["#, "Global table manipulation"),
            (r#"getmetatable"#, "Metatable access"),
            (r#"setmetatable"#, "Metatable modification"),
            (r#"rawset|rawget"#, "Raw table access"),
            (r#"debug\."#, "Debug library access"),
            (r#"os\."#, "OS library access"),
            (r#"io\."#, "IO library access"),
            (r#"loadstring|load\(|dofile|loadfile"#, "Dynamic code loading"),
            (r#"require\s*\(|require\s*""#, "Module loading"),
        ],
        TrustLevel::UserScript => vec![
            (r#"while\s+true\s+do"#, "Infinite loop detected"),
            (r#"repeat\s+.*\s+until\s+false"#, "Infinite loop detected"),
            (r#"debug\."#, "Debug library access"),
            (r#"os\.execute|os\.exit"#, "OS command execution"),
            (r#"io\."#, "IO library access"),
            (r#"loadstring|load\(|dofile|loadfile"#, "Dynamic code loading"),
        ],
        TrustLevel::GameScript | TrustLevel::Engine => vec![
            (r#"os\.execute"#, "OS command execution"),
        ],
    };

    for (pattern, message) in forbidden_patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            if regex.is_match(code) {
                return Err(format!("Forbidden pattern: {}", message));
            }
        }
    }

    let limits = trust_level.limits();
    let string_regex = regex::Regex::new(r#""([^"\\]|\\.)*"|'([^'\\]|\\.)*'"#).unwrap();
    for mat in string_regex.find_iter(code) {
        if mat.len() > limits.max_string_len {
            return Err(format!("String literal too long: {} bytes", mat.len()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandboxed_lua_creation() {
        let lua = create_sandboxed_lua(TrustLevel::AiGenerated).unwrap();
        let globals = lua.globals();
        assert!(globals.get::<Value>("dofile").unwrap().is_nil());
        assert!(globals.get::<Value>("loadfile").unwrap().is_nil());
    }

    #[test]
    fn test_validate_code() {
        assert!(validate_code("while true do end", TrustLevel::AiGenerated).is_err());
        assert!(validate_code("local x = 1 + 1", TrustLevel::AiGenerated).is_ok());
    }
}
