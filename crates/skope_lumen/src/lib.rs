//! # skope_lumen — Global Illumination System
//!
//! Lumen-inspired GI for the SKOPE engine. Uses software SDF tracing,
//! screen-space probes, world-space radiance cache, and surface cache.
//!
//! ## Pipeline (inserted after material evaluation)
//!
//! 1. **Surface Cache Update** — Capture card albedo/normal/emissive (every N frames)
//! 2. **Global SDF Update** — Merge per-mesh SDFs into composite volume
//! 3. **Screen Probe Placement** — Place probes on screen at regular intervals
//! 4. **Screen Probe Gather** — SDF trace + HZB screen trace + radiance cache
//! 5. **Screen Probe Filter** — Spatial + temporal denoising
//! 6. **Radiance Cache Update** — Refresh world-space SH probes
//! 7. **Composite** — Apply irradiance to HDR buffer
//!
//! ## Coexistence with DDGI
//!
//! Lumen and DDGI share the same bind group slots (Group 2, bindings 11-13).
//! A `RenderSettings` flag controls which system is active.

pub mod types;
pub mod sdf;
pub mod screen_probe;
pub mod radiance_cache;

pub use types::*;
pub use sdf::GlobalSDF;
pub use screen_probe::{ScreenProbeGrid, MAX_SCREEN_PROBES, DIRECTIONS_PER_PROBE};
pub use radiance_cache::RadianceCache;

#[cfg(feature = "gpu")]
pub use sdf::SDFVolumeGpu;
#[cfg(feature = "gpu")]
pub use screen_probe::ScreenProbePipeline;
#[cfg(feature = "gpu")]
pub use radiance_cache::RadianceCacheGpu;
