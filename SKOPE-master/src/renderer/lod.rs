// SKOPE Engine - Level of Detail (LOD) System
//
// Provides mesh LOD selection based on screen-space coverage
// with dithered cross-fade transitions (TAA-friendly).
//
// Reference: SKOPE Engine Rendering Pipeline v1.1 Design Doc

use glam::{Vec3, Mat4};
use bytemuck::{Pod, Zeroable};

/// LOD Configuration
#[derive(Debug, Clone)]
pub struct LodConfig {
    /// Screen coverage thresholds for LOD transitions (0.0 - 1.0)
    /// LOD 0 when coverage > thresholds[0]
    /// LOD 1 when coverage > thresholds[1]
    /// etc.
    pub thresholds: [f32; 4],

    /// Transition zone size (fraction of threshold)
    pub transition_zone: f32,

    /// Quality bias multiplier (higher = use higher LOD longer)
    pub bias: f32,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self {
            thresholds: [0.3, 0.1, 0.03, 0.01],  // 30%, 10%, 3%, 1% screen coverage
            transition_zone: 0.1,
            bias: 1.0,
        }
    }
}

/// LOD Instance Data (GPU-side)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LodInstanceData {
    /// Current LOD level (0 = highest detail)
    pub lod_level: u32,

    /// Transition factor (0.0 = current LOD, 1.0 = next LOD)
    pub transition_factor: f32,

    /// Dither seed based on screen position
    pub dither_seed: f32,

    /// Padding
    pub _pad: f32,
}

/// Bounding volume for LOD calculations
#[derive(Debug, Clone, Copy)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    /// Create from axis-aligned bounding box
    pub fn from_aabb(min: Vec3, max: Vec3) -> Self {
        let center = (min + max) * 0.5;
        let radius = (max - min).length() * 0.5;
        Self { center, radius }
    }
}

/// LOD Mesh Definition
#[derive(Debug, Clone)]
pub struct LodMesh {
    /// LOD levels (0 = highest detail)
    pub levels: Vec<LodLevel>,

    /// Bounding sphere for culling/LOD selection
    pub bounds: BoundingSphere,
}

/// Single LOD Level
#[derive(Debug, Clone)]
pub struct LodLevel {
    /// Mesh index in geometry buffer
    pub mesh_idx: usize,

    /// Vertex count for this LOD
    pub vertex_count: u32,

    /// Index count for this LOD
    pub index_count: u32,

    /// Triangle reduction ratio vs LOD 0 (for stats)
    pub reduction_ratio: f32,
}

/// LOD Selector
pub struct LodSelector {
    pub config: LodConfig,
}

impl LodSelector {
    pub fn new(config: LodConfig) -> Self {
        Self { config }
    }

    /// Calculate screen-space coverage of an object
    ///
    /// Returns a value from 0.0 to 1.0 representing what fraction
    /// of the screen the object covers.
    pub fn calculate_coverage(
        &self,
        bounds: &BoundingSphere,
        camera_pos: Vec3,
        proj: Mat4,
        screen_height: f32,
    ) -> f32 {
        // Distance from camera to object center
        let distance = (bounds.center - camera_pos).length();

        // Avoid division by zero
        if distance < 0.001 {
            return 1.0;
        }

        // Project bounding sphere radius to screen space
        // Using simple approximation: screen_size = (radius / distance) * proj_scale
        let proj_scale = proj.y_axis.y;  // Y scale from projection matrix
        let screen_radius = (bounds.radius / distance) * proj_scale * screen_height * 0.5;

        // Coverage as fraction of screen height
        let coverage = (screen_radius * 2.0) / screen_height;

        coverage.clamp(0.0, 1.0)
    }

    /// Select LOD level based on screen coverage
    pub fn select_lod(
        &self,
        coverage: f32,
        max_lod: u32,
    ) -> LodSelection {
        let adjusted_coverage = coverage * self.config.bias;

        for (i, &threshold) in self.config.thresholds.iter().enumerate() {
            if adjusted_coverage > threshold {
                // Check if in transition zone
                let transition_start = threshold * (1.0 + self.config.transition_zone);
                let transition_factor = if adjusted_coverage < transition_start {
                    1.0 - (adjusted_coverage - threshold) / (transition_start - threshold)
                } else {
                    0.0
                };

                return LodSelection {
                    level: (i as u32).min(max_lod),
                    transition_factor,
                    next_level: ((i + 1) as u32).min(max_lod),
                };
            }
        }

        // Lowest LOD
        LodSelection {
            level: max_lod,
            transition_factor: 0.0,
            next_level: max_lod,
        }
    }

    /// Select LOD with dither seed for cross-fade
    pub fn select_lod_with_dither(
        &self,
        coverage: f32,
        max_lod: u32,
        screen_pos: (f32, f32),
        frame_index: u32,
    ) -> LodInstanceData {
        let selection = self.select_lod(coverage, max_lod);

        // Generate dither seed from screen position and frame
        let dither_seed = Self::compute_dither_seed(screen_pos, frame_index);

        LodInstanceData {
            lod_level: selection.level,
            transition_factor: selection.transition_factor,
            dither_seed,
            _pad: 0.0,
        }
    }

    /// Compute dither seed using interleaved gradient noise
    fn compute_dither_seed(screen_pos: (f32, f32), frame: u32) -> f32 {
        let magic = (0.06711056_f32, 0.00583715_f32, 52.982_918_f32);
        let frame_offset = (frame % 64) as f32 * 5.835_791;
        let x = screen_pos.0 + frame_offset;
        let y = screen_pos.1;
        (magic.2 * (x * magic.0 + y * magic.1).fract()).fract()
    }
}

/// LOD Selection Result
#[derive(Debug, Clone, Copy)]
pub struct LodSelection {
    /// Current LOD level
    pub level: u32,

    /// Transition factor to next LOD (0.0 = fully current, 1.0 = fully next)
    pub transition_factor: f32,

    /// Next LOD level (for cross-fade)
    pub next_level: u32,
}

impl LodSelection {
    /// Check if transitioning between LOD levels
    pub fn is_transitioning(&self) -> bool {
        self.transition_factor > 0.001 && self.level != self.next_level
    }

    /// Get the final LOD after dithered selection
    pub fn dithered_lod(&self, dither: f32) -> u32 {
        if dither < self.transition_factor {
            self.next_level
        } else {
            self.level
        }
    }
}

/// LOD Statistics (for debugging/profiling)
#[derive(Debug, Default)]
pub struct LodStats {
    pub objects_per_lod: [u32; 5],
    pub transitioning_objects: u32,
    pub total_triangles_rendered: u64,
    pub triangles_saved: u64,
}

impl LodStats {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn record(&mut self, selection: &LodSelection, triangles: u32, lod0_triangles: u32) {
        let lod = selection.dithered_lod(0.5) as usize;
        if lod < 5 {
            self.objects_per_lod[lod] += 1;
        }

        if selection.is_transitioning() {
            self.transitioning_objects += 1;
        }

        self.total_triangles_rendered += triangles as u64;
        self.triangles_saved += (lod0_triangles - triangles) as u64;
    }

    pub fn triangle_reduction_percent(&self) -> f32 {
        if self.total_triangles_rendered + self.triangles_saved == 0 {
            return 0.0;
        }
        let total = self.total_triangles_rendered + self.triangles_saved;
        (self.triangles_saved as f32 / total as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lod_selection() {
        let selector = LodSelector::new(LodConfig::default());

        // High coverage = LOD 0
        let sel = selector.select_lod(0.5, 3);
        assert_eq!(sel.level, 0);

        // Medium coverage = LOD 1
        let sel = selector.select_lod(0.15, 3);
        assert_eq!(sel.level, 1);

        // Low coverage = LOD 3
        let sel = selector.select_lod(0.005, 3);
        assert_eq!(sel.level, 3);
    }

    #[test]
    fn test_coverage_calculation() {
        let selector = LodSelector::new(LodConfig::default());
        let bounds = BoundingSphere::new(Vec3::new(0.0, 0.0, -10.0), 1.0);
        let camera_pos = Vec3::ZERO;
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 16.0/9.0, 0.1, 100.0);

        let coverage = selector.calculate_coverage(&bounds, camera_pos, proj, 1080.0);
        assert!(coverage > 0.0 && coverage < 1.0);
    }
}
