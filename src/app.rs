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

// Phase 0: 공유 상태 및 명령 큐
mod editor_context;
mod commands;

// Phase 0: 드래그/드롭 시스템
mod drag_state;
mod drop_target;

// Phase 1: 모달 시스템
mod modal;

// ImGui 백엔드 (새 UI 시스템)
#[cfg(feature = "imgui-ui")]
pub mod imgui_backend;

// App Runner - 메인 애플리케이션 구조체
pub mod runner;
mod event_handler;
mod input_handlers;
mod mouse_handlers;
mod redraw_handler;
mod helpers;
mod viewports;

pub use gpu_context::MinimalGpuContext;
pub use viewports::{ViewportRegistry, ViewportData, FloatingWindowRequest};
pub use state_builder::StateBuilder;
pub use state::State;

// Phase 0: 공유 상태 및 명령 큐 내보내기
pub use editor_context::{EditorContext, SharedEditorContext, create_shared_context};
pub use commands::{EditorCommand, CommandQueue, ViewportAction};

// Phase 0: 드래그/드롭 시스템 내보내기
pub use drag_state::{GlobalDragState, DragType, DragPhase, AssetType, DropPreview};
pub use drop_target::{DropTarget, DropResult, SceneViewDropTarget, HierarchyDropTarget, HierarchyDropPosition};

// Phase 1: 모달 시스템 내보내기
pub use modal::{ModalState, ModalScope, ModalPriority, ModalContent, ModalResult, ModalButton, ModalManager, FileDialogState, FileDialogType};

// App 및 초기화 함수 내보내기
pub use runner::{App, init_ecs, init_egui, init_game_ui, init_scripting};
