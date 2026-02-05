// SKOPE Engine - Texture Module
//
// Texture loading and management utilities

#[allow(dead_code)]
pub mod ktx2_loader;
#[allow(dead_code)]
pub mod bindless;

#[allow(unused_imports)]
pub use ktx2_loader::{load_ktx2, load_ktx2_from_memory, create_texture_from_ktx2, Ktx2Texture, Ktx2Error};
#[allow(unused_imports)]
pub use bindless::{BindlessTextureHeap, TextureHandle, BindlessStats, MAX_BINDLESS_TEXTURES};
