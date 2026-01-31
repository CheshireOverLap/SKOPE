//! SKOPE Script Error Handling
//!
//! Provides structured error handling for Lua script errors,
//! with support for reporting to Blender via Live Link.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Error severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Warning - script continues with fallback behavior
    Warning,

    /// Error - script is disabled, other scripts continue
    Error,

    /// Critical - system-wide alert, may affect engine stability
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Warning => write!(f, "WARN"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Stack frame for error context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    pub function: String,
    pub file: String,
    pub line: u32,
    pub is_native: bool,
}

/// Detailed error information for Lua errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaErrorInfo {
    /// File where error occurred
    pub file: String,

    /// Line number (if available)
    pub line: Option<u32>,

    /// Column number (if available)
    pub column: Option<u32>,

    /// Error message
    pub message: String,

    /// Error severity
    pub severity: ErrorSeverity,

    /// Stack trace
    pub stack_trace: Vec<StackFrame>,

    /// Script entity (if error occurred during entity script)
    pub entity: Option<String>,

    /// Timestamp
    pub timestamp: f64,

    /// Error category for filtering
    pub category: ErrorCategory,
}

/// Error categories for filtering and grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Syntax error in Lua code
    Syntax,

    /// Runtime error during execution
    Runtime,

    /// Type error (wrong argument types)
    Type,

    /// API usage error (wrong function usage)
    Api,

    /// Resource error (missing file, etc.)
    Resource,

    /// Timeout error
    Timeout,

    /// Memory error
    Memory,

    /// Validation error (sandbox violation)
    Validation,

    /// Unknown/other error
    Unknown,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Syntax => write!(f, "Syntax"),
            ErrorCategory::Runtime => write!(f, "Runtime"),
            ErrorCategory::Type => write!(f, "Type"),
            ErrorCategory::Api => write!(f, "API"),
            ErrorCategory::Resource => write!(f, "Resource"),
            ErrorCategory::Timeout => write!(f, "Timeout"),
            ErrorCategory::Memory => write!(f, "Memory"),
            ErrorCategory::Validation => write!(f, "Validation"),
            ErrorCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

impl LuaErrorInfo {
    /// Create from mlua error
    pub fn from_mlua_error(err: &mlua::Error, file: &str, elapsed_time: f64) -> Self {
        let (message, category, severity) = categorize_error(err);
        let stack_trace = extract_stack_trace(err);
        let line = extract_line_number(&message);

        Self {
            file: file.to_string(),
            line,
            column: None,
            message,
            severity,
            stack_trace,
            entity: None,
            timestamp: elapsed_time,
            category,
        }
    }

    /// Create a simple error
    pub fn simple(file: &str, message: &str, severity: ErrorSeverity, elapsed_time: f64) -> Self {
        Self {
            file: file.to_string(),
            line: None,
            column: None,
            message: message.to_string(),
            severity,
            stack_trace: Vec::new(),
            entity: None,
            timestamp: elapsed_time,
            category: ErrorCategory::Unknown,
        }
    }

    /// Create a validation error
    pub fn validation(file: &str, message: &str, line: Option<u32>, elapsed_time: f64) -> Self {
        Self {
            file: file.to_string(),
            line,
            column: None,
            message: message.to_string(),
            severity: ErrorSeverity::Error,
            stack_trace: Vec::new(),
            entity: None,
            timestamp: elapsed_time,
            category: ErrorCategory::Validation,
        }
    }

    /// Set the associated entity
    pub fn with_entity(mut self, entity: &str) -> Self {
        self.entity = Some(entity.to_string());
        self
    }

    /// Format for console output
    pub fn format_console(&self) -> String {
        let mut output = format!(
            "[{}] [{}] {}",
            self.severity,
            self.category,
            self.file
        );

        if let Some(line) = self.line {
            output.push_str(&format!(":{}", line));
        }

        output.push_str(&format!(": {}", self.message));

        if !self.stack_trace.is_empty() {
            output.push_str("\n  Stack trace:");
            for frame in &self.stack_trace {
                if frame.is_native {
                    output.push_str(&format!("\n    [native] {}", frame.function));
                } else {
                    output.push_str(&format!(
                        "\n    {}:{}  {}",
                        frame.file, frame.line, frame.function
                    ));
                }
            }
        }

        output
    }

    /// Format for Blender display
    pub fn format_blender(&self) -> String {
        let mut output = format!("{}: ", self.message);

        if let Some(line) = self.line {
            output.push_str(&format!("(line {})", line));
        }

        output
    }
}

/// Categorize an mlua error
fn categorize_error(err: &mlua::Error) -> (String, ErrorCategory, ErrorSeverity) {
    match err {
        mlua::Error::SyntaxError { message, .. } => {
            (message.clone(), ErrorCategory::Syntax, ErrorSeverity::Error)
        }

        mlua::Error::RuntimeError(msg) => {
            let category = if msg.contains("attempt to call") || msg.contains("attempt to index") {
                ErrorCategory::Type
            } else if msg.contains("stack overflow") {
                ErrorCategory::Memory
            } else {
                ErrorCategory::Runtime
            };
            (msg.clone(), category, ErrorSeverity::Error)
        }

        mlua::Error::CallbackError { cause, .. } => {
            let msg = format!("Callback error: {}", cause);
            (msg, ErrorCategory::Api, ErrorSeverity::Error)
        }

        mlua::Error::ExternalError(err) => {
            let msg = err.to_string();
            let category = if msg.contains("timed out") {
                ErrorCategory::Timeout
            } else if msg.contains("memory") {
                ErrorCategory::Memory
            } else {
                ErrorCategory::Unknown
            };
            (msg, category, ErrorSeverity::Error)
        }

        mlua::Error::MemoryError(msg) => {
            (msg.clone(), ErrorCategory::Memory, ErrorSeverity::Critical)
        }

        _ => {
            (format!("{}", err), ErrorCategory::Unknown, ErrorSeverity::Error)
        }
    }
}

/// Extract stack trace from error
fn extract_stack_trace(err: &mlua::Error) -> Vec<StackFrame> {
    let mut frames = Vec::new();

    let err_str = format!("{:?}", err);

    let traceback_pattern = regex::Regex::new(
        r#"\[string "([^"]+)"\]:(\d+): in (function|main chunk|local) '?(\w*)'?"#
    ).ok();

    if let Some(pattern) = traceback_pattern {
        for cap in pattern.captures_iter(&err_str) {
            let file = cap.get(1).map(|m| m.as_str()).unwrap_or("?");
            let line: u32 = cap.get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let func = cap.get(4).map(|m| m.as_str()).unwrap_or("?");

            frames.push(StackFrame {
                function: func.to_string(),
                file: file.to_string(),
                line,
                is_native: false,
            });
        }
    }

    frames
}

/// Extract line number from error message
fn extract_line_number(message: &str) -> Option<u32> {
    let patterns = [
        regex::Regex::new(r":(\d+):").ok(),
        regex::Regex::new(r"line (\d+)").ok(),
    ];

    for pattern in patterns.into_iter().flatten() {
        if let Some(cap) = pattern.captures(message) {
            if let Some(num) = cap.get(1) {
                if let Ok(line) = num.as_str().parse() {
                    return Some(line);
                }
            }
        }
    }

    None
}

/// Error reporter for collecting and broadcasting errors
pub struct ErrorReporter {
    errors: Vec<LuaErrorInfo>,
    max_errors: usize,
}

impl Default for ErrorReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorReporter {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            max_errors: 100,
        }
    }

    /// Report an error
    pub fn report(&mut self, error: LuaErrorInfo) {
        log::error!("{}", error.format_console());

        self.errors.push(error);

        if self.errors.len() > self.max_errors {
            self.errors.remove(0);
        }
    }

    /// Get all errors
    pub fn errors(&self) -> &[LuaErrorInfo] {
        &self.errors
    }

    /// Get errors for a specific file
    pub fn errors_for_file(&self, file: &str) -> Vec<&LuaErrorInfo> {
        self.errors.iter()
            .filter(|e| e.file == file)
            .collect()
    }

    /// Get error count by severity
    pub fn count_by_severity(&self, severity: ErrorSeverity) -> usize {
        self.errors.iter()
            .filter(|e| e.severity == severity)
            .count()
    }

    /// Clear all errors
    pub fn clear(&mut self) {
        self.errors.clear();
    }

    /// Clear errors for a specific file
    pub fn clear_file(&mut self, file: &str) {
        self.errors.retain(|e| e.file != file);
    }

    /// Take errors for Live Link broadcast
    pub fn take_for_broadcast(&mut self) -> Vec<LuaErrorInfo> {
        std::mem::take(&mut self.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_formatting() {
        let error = LuaErrorInfo {
            file: "test.lua".to_string(),
            line: Some(42),
            column: None,
            message: "attempt to call nil value".to_string(),
            severity: ErrorSeverity::Error,
            stack_trace: vec![
                StackFrame {
                    function: "foo".to_string(),
                    file: "test.lua".to_string(),
                    line: 42,
                    is_native: false,
                }
            ],
            entity: Some("Player".to_string()),
            timestamp: 1.5,
            category: ErrorCategory::Runtime,
        };

        let console = error.format_console();
        assert!(console.contains("ERROR"));
        assert!(console.contains("test.lua:42"));
    }

    #[test]
    fn test_error_reporter() {
        let mut reporter = ErrorReporter::new();

        reporter.report(LuaErrorInfo::simple(
            "test.lua",
            "Test error",
            ErrorSeverity::Error,
            0.0
        ));

        assert_eq!(reporter.errors().len(), 1);
        assert_eq!(reporter.count_by_severity(ErrorSeverity::Error), 1);
    }
}
