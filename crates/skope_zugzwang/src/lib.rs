//! # skope_zugzwang — Render Dependency Graph
//!
//! Declarative render graph for the SKOPE engine. Replaces sequential command
//! encoding with a graph of passes that declare their resource dependencies.
//!
//! ## Features
//!
//! - **Automatic pass ordering** via topological sort of read/write dependencies
//! - **Dead pass elimination** — passes whose outputs are never consumed are culled
//! - **Transient resource pooling** — textures/buffers reused across frames
//! - **Side-effect passes** — present/blit passes that are never culled
//!
//! ## Usage
//!
//! ```ignore
//! use skope_zugzwang::{RenderGraph, RDGBuilder, RDGTextureDesc};
//!
//! let mut graph = RenderGraph::new();
//! let mut builder = RDGBuilder::new(&mut graph);
//!
//! let depth = builder.create_texture(RDGTextureDesc::new_2d("Depth", 1920, 1080, Depth32Float));
//! let hdr = builder.create_texture(RDGTextureDesc::new_2d("HDR", 1920, 1080, Rgba16Float));
//!
//! builder.add_render_pass("Visibility", |setup| {
//!     setup.write_texture(depth);
//!     Box::new(move |ctx| { /* encode commands */ })
//! });
//!
//! builder.add_compute_pass("MaterialEval", |setup| {
//!     setup.read_texture(depth);
//!     setup.write_texture(hdr);
//!     Box::new(move |ctx| { /* encode compute */ })
//! });
//!
//! graph.compile(&device);
//! graph.execute(&device, &queue, &mut encoder);
//! graph.reclaim(); // return transient resources to pool
//! ```

pub mod handle;
pub mod resource;
pub mod pass;
pub mod graph;
pub mod pool;

#[cfg(feature = "gpu")]
pub mod builder;

// Re-exports for convenience.
pub use handle::{RDGTextureHandle, RDGBufferHandle};
pub use pass::{PassType, RDGPassNode, RDGPassSetup};

#[cfg(feature = "gpu")]
pub use resource::{RDGTextureDesc, RDGBufferDesc, RDGTexture, RDGBuffer};
#[cfg(feature = "gpu")]
pub use graph::{RenderGraph, RDGResourceRegistry};
#[cfg(feature = "gpu")]
pub use builder::RDGBuilder;
#[cfg(feature = "gpu")]
pub use pass::RDGPassContext;
#[cfg(feature = "gpu")]
pub use pool::TransientResourcePool;
