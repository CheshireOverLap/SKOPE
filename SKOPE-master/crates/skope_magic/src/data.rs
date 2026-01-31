//! Magic Circle Data Structures
//!
//! 노드, 레이어, 마법진 정의 데이터 타입

pub mod node;
pub mod layer;
pub mod circle_def;

pub use node::{ElementType, PolarPosition, NodeDef, Connection};
pub use layer::{LayerType, LayerDef, LayerState};
pub use circle_def::{MagicCircleDefinition, MagicCircleRegistry, PatternAnalysis};
