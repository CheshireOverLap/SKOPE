//! SKOPE Application Module
//!
//! GPU 상태 및 렌더링 로직을 관리하는 State 구조체 포함

// State 관련 모듈들
mod gpu_context;
mod state_builder;
mod data_types;
mod state;
mod game_ui_commands;

mod ui_sync;
mod input;
mod keyboard_handler;
mod scene_manager;

// 공유 상태 및 명령 큐
mod editor_context;
mod commands;

// ImGui 백엔드 (UI 시스템)
pub mod imgui_backend;

// App Runner - 메인 애플리케이션 구조체
pub mod runner;
mod event_handler;
mod input_handlers;
mod mouse_handlers;
mod redraw_handler;
mod helpers;

pub use gpu_context::MinimalGpuContext;
pub use state_builder::StateBuilder;
pub use state::State;

// 공유 상태 및 명령 큐 내보내기
pub use editor_context::{EditorContext, SharedEditorContext, create_shared_context};
pub use commands::{EditorCommand, CommandQueue};

// App 및 초기화 함수 내보내기
pub use runner::{App, init_ecs, init_game_ui, init_scripting};
