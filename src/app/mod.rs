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
pub use input::ModifierKeys;

// App 및 관련 타입 내보내기
pub use runner::{App, AppMode, init_ecs, init_egui, init_game_ui, init_scripting, load_window_icon};
