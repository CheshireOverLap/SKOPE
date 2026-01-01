//! SKOPE AI Code Validator
//!
//! Validates AI-generated Lua code for safety and correctness.
//! Performs both static analysis and structure validation.

use regex::Regex;
use std::collections::HashSet;

/// Validation result
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub metrics: CodeMetrics,
}

/// Validation error (blocks execution)
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub code: ErrorCode,
    pub message: String,
}

/// Validation warning (allows execution with caution)
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub line: Option<usize>,
    pub message: String,
}

/// Error codes for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ForbiddenPattern,
    InfiniteLoop,
    DangerousFunction,
    MemoryBomb,
    CodeInjection,
    GlobalManipulation,
    MetatableAccess,
    FileAccess,
    NetworkAccess,
    SystemCommand,
    SyntaxError,
    StructureError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::ForbiddenPattern => write!(f, "FORBIDDEN_PATTERN"),
            ErrorCode::InfiniteLoop => write!(f, "INFINITE_LOOP"),
            ErrorCode::DangerousFunction => write!(f, "DANGEROUS_FUNCTION"),
            ErrorCode::MemoryBomb => write!(f, "MEMORY_BOMB"),
            ErrorCode::CodeInjection => write!(f, "CODE_INJECTION"),
            ErrorCode::GlobalManipulation => write!(f, "GLOBAL_MANIPULATION"),
            ErrorCode::MetatableAccess => write!(f, "METATABLE_ACCESS"),
            ErrorCode::FileAccess => write!(f, "FILE_ACCESS"),
            ErrorCode::NetworkAccess => write!(f, "NETWORK_ACCESS"),
            ErrorCode::SystemCommand => write!(f, "SYSTEM_COMMAND"),
            ErrorCode::SyntaxError => write!(f, "SYNTAX_ERROR"),
            ErrorCode::StructureError => write!(f, "STRUCTURE_ERROR"),
        }
    }
}

/// Code complexity and structure metrics
#[derive(Debug, Default)]
pub struct CodeMetrics {
    pub line_count: usize,
    pub function_count: usize,
    pub loop_count: usize,
    pub nesting_depth: usize,
    pub variable_count: usize,
    pub string_literals: usize,
    pub total_string_length: usize,
    pub comment_lines: usize,
}

/// AI Code Validator
pub struct AiCodeValidator {
    forbidden_patterns: Vec<(Regex, ErrorCode, &'static str)>,
    warning_patterns: Vec<(Regex, &'static str)>,
    allowed_globals: HashSet<&'static str>,
    allowed_skope_apis: HashSet<&'static str>,
}

impl Default for AiCodeValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AiCodeValidator {
    pub fn new() -> Self {
        let forbidden_patterns = vec![
            // Infinite loops
            (Regex::new(r"while\s+true\s+do").unwrap(), ErrorCode::InfiniteLoop, "Infinite while loop"),
            (Regex::new(r"repeat\s+[\s\S]*?\s+until\s+false").unwrap(), ErrorCode::InfiniteLoop, "Infinite repeat loop"),
            (Regex::new(r"for\s+\w+\s*=\s*1\s*,\s*math\.huge").unwrap(), ErrorCode::InfiniteLoop, "Loop to math.huge"),
            (Regex::new(r"for\s+\w+\s*=\s*\d+\s*,\s*1e308").unwrap(), ErrorCode::InfiniteLoop, "Loop to very large number"),

            // Global manipulation
            (Regex::new(r"_G\s*\[").unwrap(), ErrorCode::GlobalManipulation, "Direct _G access"),
            (Regex::new(r"_G\s*\.").unwrap(), ErrorCode::GlobalManipulation, "Direct _G access"),
            (Regex::new(r"_ENV\s*=").unwrap(), ErrorCode::GlobalManipulation, "Environment manipulation"),
            (Regex::new(r"rawset\s*\(").unwrap(), ErrorCode::GlobalManipulation, "rawset usage"),
            (Regex::new(r"rawget\s*\(").unwrap(), ErrorCode::GlobalManipulation, "rawget usage"),

            // Metatable access
            (Regex::new(r"getmetatable\s*\(").unwrap(), ErrorCode::MetatableAccess, "getmetatable usage"),
            (Regex::new(r"setmetatable\s*\(").unwrap(), ErrorCode::MetatableAccess, "setmetatable usage"),
            (Regex::new(r"__\w+\s*=").unwrap(), ErrorCode::MetatableAccess, "Metamethod definition"),

            // Code injection
            (Regex::new(r"loadstring\s*\(").unwrap(), ErrorCode::CodeInjection, "loadstring usage"),
            (Regex::new(r"load\s*\(").unwrap(), ErrorCode::CodeInjection, "load usage"),
            (Regex::new(r"dofile\s*\(").unwrap(), ErrorCode::CodeInjection, "dofile usage"),
            (Regex::new(r"loadfile\s*\(").unwrap(), ErrorCode::CodeInjection, "loadfile usage"),

            // File/IO access
            (Regex::new(r"io\.\w+\s*\(").unwrap(), ErrorCode::FileAccess, "io library usage"),
            (Regex::new(r"file:\w+\s*\(").unwrap(), ErrorCode::FileAccess, "file method usage"),

            // System commands
            (Regex::new(r"os\.execute\s*\(").unwrap(), ErrorCode::SystemCommand, "os.execute usage"),
            (Regex::new(r"os\.exit\s*\(").unwrap(), ErrorCode::SystemCommand, "os.exit usage"),
            (Regex::new(r"os\.remove\s*\(").unwrap(), ErrorCode::SystemCommand, "os.remove usage"),
            (Regex::new(r"os\.rename\s*\(").unwrap(), ErrorCode::SystemCommand, "os.rename usage"),
            (Regex::new(r"os\.getenv\s*\(").unwrap(), ErrorCode::SystemCommand, "os.getenv usage"),

            // Network access (if socket library exists)
            (Regex::new(r"socket\.\w+").unwrap(), ErrorCode::NetworkAccess, "socket library usage"),
            (Regex::new(r"http\.\w+").unwrap(), ErrorCode::NetworkAccess, "http library usage"),

            // Debug library
            (Regex::new(r"debug\.\w+").unwrap(), ErrorCode::DangerousFunction, "debug library usage"),

            // Package/require
            (Regex::new(r"require\s*\(").unwrap(), ErrorCode::DangerousFunction, "require usage"),
            (Regex::new(r"package\.\w+").unwrap(), ErrorCode::DangerousFunction, "package library usage"),

            // Garbage collection manipulation
            (Regex::new(r"collectgarbage\s*\(").unwrap(), ErrorCode::DangerousFunction, "collectgarbage usage"),
        ];

        let warning_patterns = vec![
            (Regex::new(r"while\s+.+\s+do").unwrap(), "while loop detected - ensure it has exit condition"),
            (Regex::new(r"repeat\s+").unwrap(), "repeat loop detected - ensure it has proper termination"),
            (Regex::new(r"for\s+\w+\s*=\s*\d+\s*,\s*\d{6,}").unwrap(), "Loop with large iteration count"),
            (Regex::new(r"string\.rep\s*\(").unwrap(), "string.rep could cause memory issues"),
            (Regex::new(r"table\.concat\s*\(").unwrap(), "table.concat with large tables could be slow"),
            (Regex::new(r"pcall\s*\(").unwrap(), "pcall usage - errors may be silently caught"),
            (Regex::new(r"xpcall\s*\(").unwrap(), "xpcall usage - errors may be silently caught"),
            (Regex::new(r"\[\s*\]\s*=").unwrap(), "Table with computed keys - review carefully"),
            (Regex::new(r"\.\.").unwrap(), "String concatenation in loop could be slow"),
        ];

        let allowed_globals: HashSet<&'static str> = [
            // Built-in safe globals
            "print", "type", "tostring", "tonumber", "pairs", "ipairs", "next",
            "select", "unpack", "table", "string", "math", "assert", "error",
            "pcall", "xpcall", "coroutine", "bit32", "utf8",

            // SKOPE API
            "SKOPE",
        ].into_iter().collect();

        let allowed_skope_apis: HashSet<&'static str> = [
            // Math/Vector
            "SKOPE.Math", "SKOPE.Vec3", "SKOPE.Quat",

            // Entity
            "SKOPE.Entity", "SKOPE.Entity.get_position", "SKOPE.Entity.set_position",
            "SKOPE.Entity.get_rotation", "SKOPE.Entity.set_rotation",
            "SKOPE.Entity.get_scale", "SKOPE.Entity.set_scale",
            "SKOPE.Entity.spawn", "SKOPE.Entity.destroy", "SKOPE.Entity.find",
            "SKOPE.Entity.apply_damage", "SKOPE.Entity.is_damageable",

            // Physics
            "SKOPE.Physics", "SKOPE.Physics.raycast", "SKOPE.Physics.overlap_sphere",

            // Audio
            "SKOPE.Audio", "SKOPE.Audio.play", "SKOPE.Audio.play_at", "SKOPE.Audio.stop",

            // VFX
            "SKOPE.VFX", "SKOPE.VFX.spawn",

            // Input
            "SKOPE.Input", "SKOPE.Input.key_pressed", "SKOPE.Input.key_held",
            "SKOPE.Input.mouse_position", "SKOPE.Input.mouse_delta",

            // Debug
            "SKOPE.Debug", "SKOPE.Debug.log", "SKOPE.Debug.warn", "SKOPE.Debug.error",
            "SKOPE.Debug.draw_line", "SKOPE.Debug.draw_sphere",

            // Time
            "SKOPE.Time", "SKOPE.Time.delta", "SKOPE.Time.elapsed",

            // Spell
            "SKOPE.Spell", "SKOPE.Spell.register", "SKOPE.Spell.apply_debuff",
        ].into_iter().collect();

        Self {
            forbidden_patterns,
            warning_patterns,
            allowed_globals,
            allowed_skope_apis,
        }
    }

    /// Validate AI-generated code
    pub fn validate(&self, code: &str) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Calculate metrics
        let metrics = self.calculate_metrics(code);

        // Check forbidden patterns
        for (pattern, error_code, message) in &self.forbidden_patterns {
            if let Some(mat) = pattern.find(code) {
                let line = code[..mat.start()].lines().count();
                errors.push(ValidationError {
                    line: Some(line),
                    column: None,
                    code: *error_code,
                    message: message.to_string(),
                });
            }
        }

        // Check warning patterns
        for (pattern, message) in &self.warning_patterns {
            if let Some(mat) = pattern.find(code) {
                let line = code[..mat.start()].lines().count();
                warnings.push(ValidationWarning {
                    line: Some(line),
                    message: message.to_string(),
                });
            }
        }

        // Check for memory bombs
        if metrics.total_string_length > 1024 * 1024 {
            errors.push(ValidationError {
                line: None,
                column: None,
                code: ErrorCode::MemoryBomb,
                message: format!("Total string literal size too large: {} bytes", metrics.total_string_length),
            });
        }

        // Check nesting depth
        if metrics.nesting_depth > 10 {
            warnings.push(ValidationWarning {
                line: None,
                message: format!("Deep nesting detected: {} levels", metrics.nesting_depth),
            });
        }

        // Check for very long lines (potential obfuscation)
        for (i, line) in code.lines().enumerate() {
            if line.len() > 500 {
                warnings.push(ValidationWarning {
                    line: Some(i + 1),
                    message: format!("Very long line: {} characters", line.len()),
                });
            }
        }

        // Validate structure
        if let Err(e) = self.validate_structure(code) {
            errors.push(ValidationError {
                line: None,
                column: None,
                code: ErrorCode::StructureError,
                message: e,
            });
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            metrics,
        }
    }

    /// Calculate code metrics
    fn calculate_metrics(&self, code: &str) -> CodeMetrics {
        let mut metrics = CodeMetrics::default();

        let lines: Vec<&str> = code.lines().collect();
        metrics.line_count = lines.len();

        // Count functions
        let func_pattern = Regex::new(r"function\s+\w+").unwrap();
        metrics.function_count = func_pattern.find_iter(code).count();

        // Count loops
        let loop_pattern = Regex::new(r"(for|while|repeat)\s+").unwrap();
        metrics.loop_count = loop_pattern.find_iter(code).count();

        // Count local variables
        let local_pattern = Regex::new(r"local\s+\w+").unwrap();
        metrics.variable_count = local_pattern.find_iter(code).count();

        // Count string literals and their total length
        let string_pattern = Regex::new(r#""([^"\\]|\\.)*"|'([^'\\]|\\.)*'|\[\[[\s\S]*?\]\]"#).unwrap();
        for mat in string_pattern.find_iter(code) {
            metrics.string_literals += 1;
            metrics.total_string_length += mat.len();
        }

        // Count comment lines
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with("--") {
                metrics.comment_lines += 1;
            }
        }

        // Calculate nesting depth
        let mut current_depth: usize = 0;
        let mut max_depth: usize = 0;
        for line in &lines {
            let trimmed = line.trim();
            // Decrease depth for end/until/else
            if trimmed.starts_with("end") || trimmed.starts_with("until") || trimmed == "else" || trimmed == "elseif" {
                current_depth = current_depth.saturating_sub(1);
            }
            max_depth = max_depth.max(current_depth);
            // Increase depth for block starts
            if trimmed.contains(" do") || trimmed.contains(" then")
                || trimmed.starts_with("function") || trimmed.starts_with("repeat") {
                current_depth += 1;
            }
        }
        metrics.nesting_depth = max_depth;

        metrics
    }

    /// Validate code structure (balanced blocks, etc.)
    fn validate_structure(&self, code: &str) -> Result<(), String> {
        // Check for balanced blocks
        let mut block_count: i32 = 0;

        for line in code.lines() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("--") {
                continue;
            }

            // Count block openers
            // Note: Handle both "function" and "local function"
            if trimmed.contains(" do") || trimmed.contains(" then")
                || trimmed.starts_with("function") || trimmed.contains("function ")
                || trimmed.starts_with("repeat") {
                block_count += 1;
            }

            // Count block closers
            if trimmed.starts_with("end") || trimmed.starts_with("until") {
                block_count -= 1;
            }

            // Check for negative depth (more ends than begins)
            if block_count < 0 {
                return Err("Unbalanced block structure: unexpected 'end'".to_string());
            }
        }

        if block_count > 0 {
            return Err(format!("Unbalanced block structure: {} unclosed block(s)", block_count));
        }

        Ok(())
    }

    /// Validate that code only uses allowed globals
    pub fn validate_globals(&self, code: &str) -> Vec<String> {
        let mut unknown_globals = Vec::new();

        // Simple pattern to find global references (not perfect but catches most)
        let global_pattern = Regex::new(r"\b([A-Z][A-Za-z_]*)\s*[.\(]").unwrap();

        for cap in global_pattern.captures_iter(code) {
            let name = &cap[1];
            if !self.allowed_globals.contains(name) && name != "SKOPE" {
                if !unknown_globals.contains(&name.to_string()) {
                    unknown_globals.push(name.to_string());
                }
            }
        }

        unknown_globals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_code() {
        let validator = AiCodeValidator::new();

        let safe_code = r#"
            local function fireball(target)
                local pos = SKOPE.Entity.get_position(target)
                SKOPE.VFX.spawn("fire", pos)
                SKOPE.Entity.apply_damage(target, 50, "fire")
                return true
            end
        "#;

        let result = validator.validate(safe_code);
        assert!(result.is_valid, "Safe code should be valid: {:?}", result.errors);
    }

    #[test]
    fn test_infinite_loop() {
        let validator = AiCodeValidator::new();

        let dangerous_code = r#"
            while true do
                -- infinite loop
            end
        "#;

        let result = validator.validate(dangerous_code);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::InfiniteLoop));
    }

    #[test]
    fn test_code_injection() {
        let validator = AiCodeValidator::new();

        let dangerous_code = r#"
            local code = "os.execute('rm -rf /')"
            loadstring(code)()
        "#;

        let result = validator.validate(dangerous_code);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.code == ErrorCode::CodeInjection));
    }
}
