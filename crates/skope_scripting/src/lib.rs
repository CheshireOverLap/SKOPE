//! SKOPE Scripting System
//!
//! Lua scripting with sandboxing and validation for SKOPE Engine.
//!
//! # Features
//! - Sandbox execution environment with trust levels
//! - AI code validation with security checks
//! - Structured error reporting

#![allow(dead_code)]

pub mod sandbox;
pub mod validator;
pub mod error;

pub use sandbox::{ResourceLimits, TrustLevel, create_sandboxed_lua, execute_sandboxed, validate_code, SandboxContext};
pub use validator::{AiCodeValidator, ValidationResult, ValidationError, ValidationWarning, ErrorCode, CodeMetrics};
pub use error::{ErrorSeverity, ErrorCategory, LuaErrorInfo, StackFrame, ErrorReporter};

// Re-export mlua for convenience
pub use mlua;
