//! # skope_promotion — Virtual Textures
//!
//! Page-based texture streaming system for the SKOPE engine.
//!
//! ## Architecture
//!
//! 1. **Page Table** — Indirection texture mapping virtual UV to physical page
//! 2. **Physical Pool** — Large atlas texture storing resident pages
//! 3. **Feedback Buffer** — GPU writes which pages are needed → CPU reads back
//! 4. **Streaming** — Async page loading from disk based on feedback
//! 5. **Cache** — LRU eviction of least-recently-used pages
//!
//! ## Integration with Material Eval
//!
//! The VT system replaces direct bindless texture sampling:
//! ```wgsl
//! // Before: let color = textureSample(bindless[handle], sampler, uv);
//! // After:  let resolved = vt_resolve(uv, mip, page_table_id);
//! //         let color = textureSample(physical_atlas, sampler, resolved.uv);
//! ```

pub mod types;
pub mod page_table;
pub mod physical_pool;
pub mod feedback;
pub mod cache;

pub use types::*;
pub use page_table::{PageTable, PageUpdate, UpdateParams};
pub use physical_pool::{PhysicalPool, PageMapping};
pub use feedback::parse_feedback;
pub use cache::VTCache;

#[cfg(feature = "gpu")]
pub use page_table::PageTablePipeline;
#[cfg(feature = "gpu")]
pub use feedback::FeedbackPipeline;
