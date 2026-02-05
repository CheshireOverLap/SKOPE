//! # skope_resource — GPU Resource Management
//!
//! Provides caching, pooling, and staging systems to reduce GPU resource
//! creation overhead and improve frame-to-frame performance.
//!
//! ## Components
//!
//! - **PipelineCache**: Hash-based render/compute pipeline deduplication
//! - **BufferPool**: Size-class buffer reuse
//! - **TexturePool**: Format/size-matched texture reuse
//! - **StagingBelt**: Ring-buffer staging for CPU→GPU uploads
//! - **BindGroupLayoutCache**: Layout deduplication

pub mod pipeline_cache;
pub mod buffer_pool;
pub mod texture_pool;
pub mod staging;
pub mod bind_group_cache;

pub use pipeline_cache::{PipelineCache, PipelineCacheStats, fnv1a_hash, hash_combine};

#[cfg(feature = "gpu")]
pub use buffer_pool::{BufferPool, BufferPoolStats};
#[cfg(feature = "gpu")]
pub use texture_pool::{TexturePool, TexturePoolStats};
#[cfg(feature = "gpu")]
pub use staging::{StagingBelt, StagingBeltStats};
#[cfg(feature = "gpu")]
pub use bind_group_cache::BindGroupLayoutCache;
