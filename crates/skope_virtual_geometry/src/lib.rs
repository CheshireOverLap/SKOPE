//! # skope_virtual_geometry — Virtual Geometry System
//!
//! GPU-driven cluster-based mesh rendering inspired by UE5's Nanite.
//!
//! ## Architecture
//!
//! 1. **Meshlet Builder** — Converts triangle meshes into clusters (meshlets)
//! 2. **GPU Culling** — Instance culling + per-cluster frustum/occlusion culling
//! 3. **Mesh Shader Rasterization** — Task + Mesh shader for large clusters
//! 4. **SW Rasterization** — Compute shader for sub-pixel clusters
//! 5. **Visibility Resolve** — Merge HW+SW into unified V-Buffer
//!
//! ## WGPU Features
//!
//! - Mesh shaders (wgpu 28 EXPERIMENTAL_MESH_SHADER) → Task + Mesh shader rasterization
//! - No 64-bit atomics → use `atomicMin(u32)` on storage buffers
//! - 4 bind group limit → carefully partition resources

pub mod types;
pub mod meshlet;
pub mod cull;
pub mod rasterize;
pub mod visibility;
pub mod streaming;

pub use types::*;
pub use meshlet::{NaniteMesh, build_meshlets};
pub use cull::{VisibleCluster, CullCounters};

#[cfg(feature = "gpu")]
pub use cull::NaniteCullPipeline;
#[cfg(feature = "gpu")]
pub use rasterize::{NaniteMeshRasterPipeline, NaniteSwRasterPipeline};
#[cfg(feature = "gpu")]
pub use visibility::NaniteVBuffer;
#[cfg(feature = "gpu")]
pub use streaming::NaniteStreamingPipeline;
