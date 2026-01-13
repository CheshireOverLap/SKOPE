//! Magic Circle Pipeline
//!
//! SDF 렌더러 및 바인드 그룹

pub mod sdf_renderer;

pub use sdf_renderer::{
    // GPU Data
    CircleUniform,
    LayerGpuData,
    NodeGpuData,
    ConnectionGpuData,
    CountsUniform,
    // Render Data
    CircleRenderInstance,
    MagicCircleRenderData,
    // System
    magic_circle_extract_system,
};

#[cfg(feature = "gpu")]
pub use sdf_renderer::MagicCircleRenderer;
