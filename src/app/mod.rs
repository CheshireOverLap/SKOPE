//! SKOPE Application Module
//!
//! GPU 상태 및 렌더링 로직을 관리하는 State 구조체 포함

mod state;
mod ui_sync;
mod input;
mod keyboard_handler;
mod mouse_handler;

pub use state::State;
// ModifierKeys는 향후 main.rs 리팩토링 시 사용 예정
#[allow(unused_imports)]
pub use input::ModifierKeys;
