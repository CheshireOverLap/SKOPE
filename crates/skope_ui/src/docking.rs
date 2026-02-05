//! Slate-style Docking System
//!
//! 언리얼 엔진의 Slate 도킹 시스템을 러스트로 구현
//!
//! ## 구조
//! - `DockArea`: OS 윈도우 하나를 담당하는 최상위 컨테이너
//! - `DockSplitter`: 화면을 가로/세로로 분할하는 가지 노드
//! - `DockTabStack`: 탭들이 모인 잎 노드
//!
//! ## 사용법
//! ```ignore
//! let mut tree = DockTree::new();
//! tree.add_tab("viewport", ViewportTab::new());
//! tree.add_tab("details", DetailsTab::new());
//! tree.dock_left("details", "viewport"); // viewport 왼쪽에 details 도킹
//! ```

mod types;
mod tree;
mod node;
mod tab;
mod compass;
mod drag_state;
mod widget;
mod layout;
mod major_tab;
mod major_tab_bar;
mod spawner;
mod sidebar;
mod workspace;
mod tab_drawer;
mod tab_commands;

pub use types::*;
pub use tree::*;
pub use node::*;
pub use tab::*;
pub use compass::*;
pub use drag_state::*;
pub use widget::*;
pub use layout::*;
pub use major_tab::*;
pub use major_tab_bar::*;
pub use spawner::*;
pub use sidebar::*;
pub use workspace::*;
pub use tab_drawer::*;
pub use tab_commands::*;
