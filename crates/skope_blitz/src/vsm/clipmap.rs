// SKOPE Engine - Virtual Shadow Maps: Clipmap Levels
// Multiple resolution levels for directional light shadows

use glam::{Mat4, Vec3};
use bytemuck::{Pod, Zeroable};

/// Per-level clipmap data uploaded to GPU.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ClipmapLevelData {
    /// Light-space view-projection for this level.
    pub view_proj: [[f32; 4]; 4],
    /// Inverse of view_proj for world-space reconstruction.
    pub inv_view_proj: [[f32; 4]; 4],
    /// World-space half-extent of this level.
    pub half_extent: f32,
    /// Texel size in world units.
    pub texel_size: f32,
    /// Offset into the virtual page table (row of pages for this level).
    pub page_table_y_offset: u32,
    /// Number of rows this level occupies in the page table.
    pub page_table_rows: u32,
}

/// Configuration for the clipmap hierarchy.
#[derive(Debug, Clone, Copy)]
pub struct ClipmapConfig {
    /// Number of clipmap levels (e.g. 6).
    pub levels: u32,
    /// World-space half-extent of the finest (level 0) clipmap.
    pub base_half_extent: f32,
    /// Scale factor between successive levels (typically 2.0).
    pub level_scale: f32,
}

impl Default for ClipmapConfig {
    fn default() -> Self {
        Self {
            levels: 6,
            base_half_extent: 5.0,
            level_scale: 2.0,
        }
    }
}

/// CPU-side clipmap manager.
///
/// Splits the virtual page table vertically: each level gets a horizontal
/// band of rows.  With a 128-row page table and 6 levels, the first 5
/// levels get 20 rows each (20*128 = 2560 pages) and the last gets 28.
pub struct Clipmap {
    pub config: ClipmapConfig,
    pub levels: Vec<ClipmapLevelData>,
}

impl Clipmap {
    pub fn new(config: ClipmapConfig, page_table_size: u32) -> Self {
        let rows_per_level = page_table_size / config.levels;
        let remainder = page_table_size % config.levels;

        let mut levels = Vec::with_capacity(config.levels as usize);
        let mut y_offset = 0u32;

        for i in 0..config.levels {
            let rows = if i == config.levels - 1 {
                rows_per_level + remainder
            } else {
                rows_per_level
            };

            let half_extent = config.base_half_extent * config.level_scale.powi(i as i32);
            let texel_size = (half_extent * 2.0) / (rows * page_table_size) as f32;

            levels.push(ClipmapLevelData {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                half_extent,
                texel_size,
                page_table_y_offset: y_offset,
                page_table_rows: rows,
            });

            y_offset += rows;
        }

        Self { config, levels }
    }

    /// Recalculate light-space matrices for each level.
    ///
    /// `camera_pos` is used to center the clipmap around the viewer.
    /// `light_dir` is the normalised direction *from* the light (e.g. sun
    /// direction pointing towards the ground).
    pub fn update_matrices(&mut self, camera_pos: Vec3, light_dir: Vec3) {
        for (i, level) in self.levels.iter_mut().enumerate() {
            let half = level.half_extent;

            // Snap to texel grid to avoid shadow swimming
            let texel = level.texel_size;
            let snapped = if texel > 0.0 {
                Vec3::new(
                    (camera_pos.x / texel).floor() * texel,
                    camera_pos.y,
                    (camera_pos.z / texel).floor() * texel,
                )
            } else {
                camera_pos
            };

            let light_view = Mat4::look_at_rh(
                snapped - light_dir * (half * 2.0),
                snapped,
                Vec3::Y,
            );

            let light_proj = Mat4::orthographic_rh(
                -half, half,
                -half, half,
                0.0, half * 4.0,
            );

            let vp = light_proj * light_view;
            level.view_proj = vp.to_cols_array_2d();
            level.inv_view_proj = vp.inverse().to_cols_array_2d();

            // Recalculate texel size based on ortho extents and page-table rows
            let page_table_texels = level.page_table_rows as f32 * 128.0; // page_table_size cols
            level.texel_size = (half * 2.0) / page_table_texels;

            log::trace!(
                "VSM clipmap level {} half_extent={:.2} texel={:.4}",
                i, half, level.texel_size
            );
        }
    }

    /// Select the finest clipmap level that covers the given world-space
    /// distance from the camera.
    pub fn select_level(&self, world_distance: f32) -> u32 {
        for (i, level) in self.levels.iter().enumerate() {
            if world_distance <= level.half_extent {
                return i as u32;
            }
        }
        (self.levels.len() - 1) as u32
    }

    pub fn level_data(&self) -> &[ClipmapLevelData] {
        &self.levels
    }
}
