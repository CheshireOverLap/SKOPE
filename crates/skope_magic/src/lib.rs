//! SKOPE Magic Circle System
//!
//! SDF 기반 노드 마법진 시스템
//!
//! # Features
//!
//! - `gpu`: GPU 렌더러 활성화 (wgpu, bytemuck 의존성)
//!
//! # Example
//!
//! ```ignore
//! use skope_magic::prelude::*;
//!
//! // 마법진 정의 로드
//! let def = MagicCircleDefinition::from_ron(ron_str)?;
//!
//! // 레지스트리에 등록
//! registry.register(def);
//!
//! // 스폰 이벤트 전송
//! events.send(SpawnMagicCircleEvent::new("fireball_01", [0.0, 1.0, 0.0]));
//! ```

pub mod data;
pub mod components;
pub mod systems;
pub mod pipeline;

/// Prelude - 자주 사용되는 타입들
pub mod prelude {
    // Data
    pub use crate::data::{
        ElementType,
        PolarPosition,
        NodeDef,
        Connection,
        LayerType,
        LayerDef,
        LayerState,
        MagicCircleDefinition,
        MagicCircleRegistry,
        PatternAnalysis,
    };

    // Components
    pub use crate::components::{
        MagicCircle,
        CircleState,
        CircleTransform,
    };

    // Systems
    pub use crate::systems::{
        MagicTime,
        magic_circle_update_system,
        magic_circle_despawn_system,
        SpawnMagicCircleEvent,
        SpawnCircleOptions,
        magic_circle_spawn_system,
    };

    // Pipeline
    pub use crate::pipeline::{
        MagicCircleRenderData,
        magic_circle_extract_system,
    };

    #[cfg(feature = "gpu")]
    pub use crate::pipeline::MagicCircleRenderer;
}

// Re-exports at crate root
pub use data::*;
pub use components::*;
pub use systems::*;
pub use pipeline::*;
