//! AI Diff View Module
//!
//! AI가 제안한 변경 사항을 Git Diff 스타일로 시각화하고,
//! 사용자가 선택적으로 승인/거부할 수 있습니다.

mod command;
mod renderer;
mod types;

pub use command::*;
pub use renderer::*;
pub use types::*;
