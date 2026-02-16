//! ComponentRegistry-based Scene Serialization System
//!
//! Provides automatic save/load for any registered ECS component.
//! New components only need one `registry.register::<T>("TypeName")` call.

pub mod registry;
pub mod types;
pub mod io;
pub mod registrations;

pub use registry::ComponentRegistry;
pub use io::{save_to_file, load_from_file};
