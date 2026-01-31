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
    #[allow(dead_code)]
    allowed_globals: HashSet<&'static str>,
}

impl Default for AiCodeValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl AiCodeValidator {
    pub fn new() -> Self {
        let forbidden_patterns = vec![
            (Regex::new(r"while\s+true\s+do").unwrap(), ErrorCode::InfiniteLoop, "Infinite while loop"),
            (Regex::new(r"repeat\s+[\s\S]*?\s+until\s+false").unwrap(), ErrorCode::InfiniteLoop, "Infinite repeat loop"),
            (Regex::new(r"for\s+\w+\s*=\s*1\s*,\s*math\.huge").unwrap(), ErrorCode::InfiniteLoop, "Loop to math.huge"),
            (Regex::new(r"for\s+\w+\s*=\s*\d+\s*,\s*1e308").unwrap(), ErrorCode::InfiniteLoop, "Loop to very large number"),
            (Regex::new(r"_G\s*\[").unwrap(), ErrorCode::GlobalManipulation, "Direct _G access"),
            (Regex::new(r"_G\s*\.").unwrap(), ErrorCode::GlobalManipulation, "Direct _G access"),
            (Regex::new(r"_ENV\s*=").unwrap(), ErrorCode::GlobalManipulation, "Environment manipulation"),
            (Regex::new(r"rawset\s*\(").unwrap(), ErrorCode::GlobalManipulation, "rawset usage"),
            (Regex::new(r"rawget\s*\(").unwrap(), ErrorCode::GlobalManipulation, "rawget usage"),
            (Regex::new(r"getmetatable\s*\(").unwrap(), ErrorCode::MetatableAccess, "getmetatable usage"),
            (Regex::new(r"setmetatable\s*\(").unwrap(), ErrorCode::MetatableAccess, "setmetatable usage"),
            (Regex::new(r"__\w+\s*=").unwrap(), ErrorCode::MetatableAccess, "Metamethod definition"),
            (Regex::new(r"loadstring\s*\(").unwrap(), ErrorCode::CodeInjection, "loadstring usage"),
            (Regex::new(r"load\s*\(").unwrap(), ErrorCode::CodeInjection, "load usage"),
            (Regex::new(r"dofile\s*\(").unwrap(), ErrorCode::CodeInjection, "dofile usage"),
            (Regex::new(r"loadfile\s*\(").unwrap(), ErrorCode::CodeInjection, "loadfile usage"),
            (Regex::new(r"io\.\w+\s*\(").unwrap(), ErrorCode::FileAccess, "io library usage"),
            (Regex::new(r"file:\w+\s*\(").unwrap(), ErrorCode::FileAccess, "file method usage"),
            (Regex::new(r"os\.execute\s*\(").unwrap(), ErrorCode::SystemCommand, "os.execute usage"),
            (Regex::new(r"os\.exit\s*\(").unwrap(), ErrorCode::SystemCommand, "os.exit usage"),
            (Regex::new(r"os\.remove\s*\(").unwrap(), ErrorCode::SystemCommand, "os.remove usage"),
            (Regex::new(r"os\.rename\s*\(").unwrap(), ErrorCode::SystemCommand, "os.rename usage"),
            (Regex::new(r"os\.getenv\s*\(").unwrap(), ErrorCode::SystemCommand, "os.getenv usage"),
            (Regex::new(r"socket\.\w+").unwrap(), ErrorCode::NetworkAccess, "socket library usage"),
            (Regex::new(r"http\.\w+").unwrap(), ErrorCode::NetworkAccess, "http library usage"),
            (Regex::new(r"debug\.\w+").unwrap(), ErrorCode::DangerousFunction, "debug library usage"),
            (Regex::new(r"require\s*\(").unwrap(), ErrorCode::DangerousFunction, "require usage"),
            (Regex::new(r"package\.\w+").unwrap(), ErrorCode::DangerousFunction, "package library usage"),
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
            "print", "type", "tostring", "tonumber", "pairs", "ipairs", "next",
            "select", "unpack", "table", "string", "math", "assert", "error",
            "pcall", "xpcall", "coroutine", "bit32", "utf8",
            "SKOPE",
        ].into_iter().collect();

        Self {
            forbidden_patterns,
            warning_patterns,
            allowed_globals,
        }
    }

    /// Validate AI-generated code
    pub fn validate(&self, code: &str) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        let metrics = self.calculate_metrics(code);

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

        for (pattern, message) in &self.warning_patterns {
            if let Some(mat) = pattern.find(code) {
                let line = code[..mat.start()].lines().count();
                warnings.push(ValidationWarning {
                    line: Some(line),
                    message: message.to_string(),
                });
            }
        }

        if metrics.total_string_length > 1024 * 1024 {
            errors.push(ValidationError {
                line: None,
                column: None,
                code: ErrorCode::MemoryBomb,
                message: format!("Total string literal size too large: {} bytes", metrics.total_string_length),
            });
        }

        if metrics.nesting_depth > 10 {
            warnings.push(ValidationWarning {
                line: None,
                message: format!("Deep nesting detected: {} levels", metrics.nesting_depth),
            });
        }

        for (i, line) in code.lines().enumerate() {
            if line.len() > 500 {
                warnings.push(ValidationWarning {
                    line: Some(i + 1),
                    message: format!("Very long line: {} characters", line.len()),
                });
            }
        }

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

    fn calculate_metrics(&self, code: &str) -> CodeMetrics {
        let mut metrics = CodeMetrics::default();

        let lines: Vec<&str> = code.lines().collect();
        metrics.line_count = lines.len();

        let func_pattern = Regex::new(r"function\s+\w+").unwrap();
        metrics.function_count = func_pattern.find_iter(code).count();

        let loop_pattern = Regex::new(r"(for|while|repeat)\s+").unwrap();
        metrics.loop_count = loop_pattern.find_iter(code).count();

        let local_pattern = Regex::new(r"local\s+\w+").unwrap();
        metrics.variable_count = local_pattern.find_iter(code).count();

        let string_pattern = Regex::new(r#""([^"\\]|\\.)*"|'([^'\\]|\\.)*'|\[\[[\s\S]*?\]\]"#).unwrap();
        for mat in string_pattern.find_iter(code) {
            metrics.string_literals += 1;
            metrics.total_string_length += mat.len();
        }

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with("--") {
                metrics.comment_lines += 1;
            }
        }

        let mut current_depth: usize = 0;
        let mut max_depth: usize = 0;
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.starts_with("end") || trimmed.starts_with("until") || trimmed == "else" || trimmed == "elseif" {
                current_depth = current_depth.saturating_sub(1);
            }
            max_depth = max_depth.max(current_depth);
            if trimmed.contains(" do") || trimmed.contains(" then")
                || trimmed.starts_with("function") || trimmed.starts_with("repeat") {
                current_depth += 1;
            }
        }
        metrics.nesting_depth = max_depth;

        metrics
    }

    fn validate_structure(&self, code: &str) -> Result<(), String> {
        let mut block_count: i32 = 0;

        for line in code.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("--") {
                continue;
            }

            if trimmed.contains(" do") || trimmed.contains(" then")
                || trimmed.starts_with("function") || trimmed.contains("function ")
                || trimmed.starts_with("repeat") {
                block_count += 1;
            }

            if trimmed.starts_with("end") || trimmed.starts_with("until") {
                block_count -= 1;
            }

            if block_count < 0 {
                return Err("Unbalanced block structure: unexpected 'end'".to_string());
            }
        }

        if block_count > 0 {
            return Err(format!("Unbalanced block structure: {} unclosed block(s)", block_count));
        }

        Ok(())
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
}
