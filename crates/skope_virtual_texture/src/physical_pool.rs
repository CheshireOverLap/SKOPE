//! Physical Page Atlas Pool
//!
//! Manages a large atlas texture (e.g. 4096x4096) that stores
//! physical pages. Each page occupies VT_PAGE_WITH_BORDER texels.
//!
//! Uses a simple slot-based allocator with LRU eviction.

use crate::types::{VT_POOL_SIZE, VT_PAGE_WITH_BORDER};

/// A slot in the physical atlas.
#[derive(Clone, Debug)]
struct PhysicalSlot {
    /// Grid position in the atlas (in page units).
    pub grid_x: u32,
    pub grid_y: u32,
    /// Currently mapped virtual page, if any.
    pub mapping: Option<PageMapping>,
    /// Frame number when this slot was last accessed.
    pub last_access_frame: u64,
}

/// Mapping from a virtual page to this physical slot.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PageMapping {
    pub texture_id: u8,
    pub virtual_page_x: u16,
    pub virtual_page_y: u16,
    pub mip_level: u8,
}

/// Physical atlas page pool.
pub struct PhysicalPool {
    /// Atlas dimensions in pages.
    pub pages_x: u32,
    pub pages_y: u32,
    /// All slots in the atlas.
    slots: Vec<PhysicalSlot>,
    /// Free slot indices.
    free_slots: Vec<usize>,
    /// Total allocated pages.
    allocated_count: u32,
}

impl PhysicalPool {
    pub fn new() -> Self {
        let pages_x = VT_POOL_SIZE / VT_PAGE_WITH_BORDER;
        let pages_y = VT_POOL_SIZE / VT_PAGE_WITH_BORDER;
        let total = (pages_x * pages_y) as usize;

        let mut slots = Vec::with_capacity(total);
        let mut free_slots = Vec::with_capacity(total);

        for i in 0..total {
            let gx = (i as u32) % pages_x;
            let gy = (i as u32) / pages_x;
            slots.push(PhysicalSlot {
                grid_x: gx,
                grid_y: gy,
                mapping: None,
                last_access_frame: 0,
            });
            free_slots.push(i);
        }

        Self {
            pages_x,
            pages_y,
            slots,
            free_slots,
            allocated_count: 0,
        }
    }

    /// Try to allocate a slot for a virtual page.
    /// Returns (grid_x, grid_y) of the allocated physical page.
    pub fn allocate(&mut self, mapping: PageMapping, frame: u64) -> Option<(u32, u32)> {
        if let Some(slot_idx) = self.free_slots.pop() {
            let slot = &mut self.slots[slot_idx];
            slot.mapping = Some(mapping);
            slot.last_access_frame = frame;
            self.allocated_count += 1;
            return Some((slot.grid_x, slot.grid_y));
        }
        None
    }

    /// Evict the least recently used page and return its slot.
    /// Returns the evicted mapping and the (grid_x, grid_y) of the freed slot.
    pub fn evict_lru(&mut self, _current_frame: u64) -> Option<(PageMapping, u32, u32)> {
        let mut oldest_frame = u64::MAX;
        let mut oldest_idx = None;

        for (i, slot) in self.slots.iter().enumerate() {
            if slot.mapping.is_some() && slot.last_access_frame < oldest_frame {
                oldest_frame = slot.last_access_frame;
                oldest_idx = Some(i);
            }
        }

        if let Some(idx) = oldest_idx {
            let slot = &mut self.slots[idx];
            let mapping = slot.mapping.take().unwrap();
            let gx = slot.grid_x;
            let gy = slot.grid_y;
            slot.last_access_frame = 0;
            self.allocated_count -= 1;
            self.free_slots.push(idx);
            Some((mapping, gx, gy))
        } else {
            None
        }
    }

    /// Touch a slot to mark it as recently accessed.
    pub fn touch(&mut self, grid_x: u32, grid_y: u32, frame: u64) {
        let idx = (grid_y * self.pages_x + grid_x) as usize;
        if idx < self.slots.len() {
            self.slots[idx].last_access_frame = frame;
        }
    }

    /// Free a specific slot by grid position.
    pub fn free(&mut self, grid_x: u32, grid_y: u32) -> Option<PageMapping> {
        let idx = (grid_y * self.pages_x + grid_x) as usize;
        if idx >= self.slots.len() {
            return None;
        }
        let slot = &mut self.slots[idx];
        if let Some(mapping) = slot.mapping.take() {
            slot.last_access_frame = 0;
            self.allocated_count -= 1;
            self.free_slots.push(idx);
            Some(mapping)
        } else {
            None
        }
    }

    /// Number of currently free slots.
    pub fn free_count(&self) -> u32 {
        self.free_slots.len() as u32
    }

    /// Number of currently allocated slots.
    pub fn allocated_count(&self) -> u32 {
        self.allocated_count
    }

    /// Total number of slots.
    pub fn total_slots(&self) -> u32 {
        self.slots.len() as u32
    }

    /// Check if a grid position has a mapping.
    pub fn get_mapping(&self, grid_x: u32, grid_y: u32) -> Option<&PageMapping> {
        let idx = (grid_y * self.pages_x + grid_x) as usize;
        if idx >= self.slots.len() {
            return None;
        }
        self.slots[idx].mapping.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_alloc_free() {
        let mut pool = PhysicalPool::new();
        let total = pool.total_slots();
        assert_eq!(pool.free_count(), total);
        assert_eq!(pool.allocated_count(), 0);

        let mapping = PageMapping {
            texture_id: 0,
            virtual_page_x: 3,
            virtual_page_y: 5,
            mip_level: 0,
        };

        let (gx, gy) = pool.allocate(mapping.clone(), 1).unwrap();
        assert_eq!(pool.allocated_count(), 1);
        assert_eq!(pool.free_count(), total - 1);

        assert_eq!(pool.get_mapping(gx, gy), Some(&mapping));

        pool.free(gx, gy);
        assert_eq!(pool.allocated_count(), 0);
    }

    #[test]
    fn test_pool_evict_lru() {
        let mut pool = PhysicalPool::new();

        let m1 = PageMapping {
            texture_id: 0,
            virtual_page_x: 0,
            virtual_page_y: 0,
            mip_level: 0,
        };
        let m2 = PageMapping {
            texture_id: 0,
            virtual_page_x: 1,
            virtual_page_y: 0,
            mip_level: 0,
        };

        pool.allocate(m1.clone(), 10);
        pool.allocate(m2.clone(), 20);

        // Evict LRU should return m1 (frame 10 < frame 20)
        let (evicted, _, _) = pool.evict_lru(30).unwrap();
        assert_eq!(evicted, m1);
    }
}
