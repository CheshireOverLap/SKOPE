//! Shadow Atlas Tile Allocator
//!
//! Dynamic tile allocation for shadow maps

use super::types::TileAllocation;

/// Shadow Atlas Tile Allocator
pub struct TileAllocator {
    atlas_size: u32,
    min_tile_size: u32,
    /// Occupancy grid (for each min_tile_size block)
    occupancy: Vec<bool>,
    grid_size: u32,
}

impl TileAllocator {
    pub fn new(atlas_size: u32, min_tile_size: u32) -> Self {
        let grid_size = atlas_size / min_tile_size;
        let total_cells = (grid_size * grid_size) as usize;
        Self {
            atlas_size,
            min_tile_size,
            occupancy: vec![false; total_cells],
            grid_size,
        }
    }

    pub fn allocate(&mut self, size: u32) -> Option<TileAllocation> {
        let tiles_needed = size / self.min_tile_size;

        // Simple first-fit allocation
        for gy in 0..(self.grid_size - tiles_needed + 1) {
            for gx in 0..(self.grid_size - tiles_needed + 1) {
                if self.can_allocate(gx, gy, tiles_needed) {
                    self.mark_occupied(gx, gy, tiles_needed);
                    return Some(TileAllocation {
                        x: gx * self.min_tile_size,
                        y: gy * self.min_tile_size,
                        size,
                        tile_index: gy * self.grid_size + gx,
                    });
                }
            }
        }
        None
    }

    fn can_allocate(&self, gx: u32, gy: u32, tiles_needed: u32) -> bool {
        for dy in 0..tiles_needed {
            for dx in 0..tiles_needed {
                let idx = ((gy + dy) * self.grid_size + (gx + dx)) as usize;
                if self.occupancy[idx] {
                    return false;
                }
            }
        }
        true
    }

    fn mark_occupied(&mut self, gx: u32, gy: u32, tiles_needed: u32) {
        for dy in 0..tiles_needed {
            for dx in 0..tiles_needed {
                let idx = ((gy + dy) * self.grid_size + (gx + dx)) as usize;
                self.occupancy[idx] = true;
            }
        }
    }

    pub fn free(&mut self, tile: &TileAllocation) {
        let gx = tile.x / self.min_tile_size;
        let gy = tile.y / self.min_tile_size;
        let tiles_needed = tile.size / self.min_tile_size;

        for dy in 0..tiles_needed {
            for dx in 0..tiles_needed {
                let idx = ((gy + dy) * self.grid_size + (gx + dx)) as usize;
                self.occupancy[idx] = false;
            }
        }
    }

    pub fn reset(&mut self) {
        self.occupancy.fill(false);
    }
}

/// Per-light allocation tracking
pub struct LightAllocation {
    pub tiles: Vec<TileAllocation>,
    pub frame_last_used: u64,
    pub importance: f32,
}
