//! SKOPE Application Module
//!
//! GPU 상태 및 렌더링 로직을 관리하는 State 구조체 포함

// State 관련 모듈들
mod gpu_context;
mod state_builder;
mod data_types;
mod state;

mod input;
mod scene_manager;

// 공유 상태 및 명령 큐
mod editor_context;
mod commands;

// skope_ui 기반 에디터 UI
pub mod slate_ui;

// App Runner - 초기화 함수 (init_ecs, init_scripting)
pub mod runner;

// EngineHandler - SlateApp용 엔진 핸들러
pub mod engine_handler;

pub use gpu_context::MinimalGpuContext;
pub use state_builder::StateBuilder;
pub use state::State;

// 공유 상태 및 명령 큐 내보내기
pub use editor_context::{SharedEditorContext, create_shared_context};
pub use commands::CommandQueue;

// 초기화 함수 내보내기
pub use runner::{init_ecs, init_scripting};

// EngineHandler 내보내기
pub use engine_handler::EngineHandler;
