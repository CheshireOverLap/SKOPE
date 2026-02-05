//! LRU Page Cache
//!
//! Coordinates between the page table, physical pool, and streaming system.
//! Decides which pages to load and which to evict based on GPU feedback.

use crate::page_table::{PageTable, PageUpdate};
use crate::physical_pool::{PhysicalPool, PageMapping};
use crate::types::{PageRequest, VTConfig, VTStats, page_flags};

/// Virtual texture cache that manages page loading and eviction.
pub struct VTCache {
    pub page_table: PageTable,
    pub physical_pool: PhysicalPool,
    pub config: VTConfig,
    /// Pending page updates to upload to GPU this frame.
    pending_updates: Vec<PageUpdate>,
    /// Current frame number.
    frame: u64,
    /// Stats for the current frame.
    stats: VTStats,
}

impl VTCache {
    pub fn new(
        virtual_width: u32,
        virtual_height: u32,
        config: VTConfig,
    ) -> Self {
        Self {
            page_table: PageTable::new(virtual_width, virtual_height),
            physical_pool: PhysicalPool::new(),
            config,
            pending_updates: Vec::new(),
            frame: 0,
            stats: VTStats::default(),
        }
    }

    /// Process page requests from GPU feedback.
    ///
    /// Returns the list of page updates to send to the GPU page table.
    pub fn process_requests(&mut self, requests: &[PageRequest]) -> &[PageUpdate] {
        self.pending_updates.clear();
        self.stats = VTStats::default();
        self.stats.resident_pages = self.physical_pool.allocated_count();
        self.stats.feedback_requests = requests.len() as u32;
        self.frame += 1;

        let mut loaded = 0u32;

        for req in requests {
            if loaded >= self.config.max_pages_per_frame {
                break;
            }

            let px = req.virtual_page_x as u32;
            let py = req.virtual_page_y as u32;

            // Already resident? Just touch it.
            if self.page_table.is_resident(px, py) {
                if let Some(entry) = self.page_table.get(px, py) {
                    self.physical_pool
                        .touch(entry.physical_x as u32, entry.physical_y as u32, self.frame);
                }
                continue;
            }

            // Try to allocate a physical slot
            let mapping = PageMapping {
                texture_id: req.texture_id,
                virtual_page_x: req.virtual_page_x,
                virtual_page_y: req.virtual_page_y,
                mip_level: req.mip_level,
            };

            let slot = if self.physical_pool.free_count() > 0 {
                self.physical_pool.allocate(mapping, self.frame)
            } else {
                // Evict LRU page
                if let Some((evicted, _gx, _gy)) = self.physical_pool.evict_lru(self.frame) {
                    // Clear old mapping in page table
                    self.page_table.clear_mapping(
                        evicted.virtual_page_x as u32,
                        evicted.virtual_page_y as u32,
                    );
                    self.stats.pages_evicted_this_frame += 1;

                    // Reallocate the freed slot
                    self.physical_pool.allocate(mapping, self.frame)
                } else {
                    None
                }
            };

            if let Some((gx, gy)) = slot {
                // Update CPU page table
                self.page_table.set_mapping(
                    px,
                    py,
                    gx as u16,
                    gy as u16,
                    req.mip_level,
                );

                // Queue GPU page table update
                self.pending_updates.push(PageUpdate {
                    virtual_page_x: px,
                    virtual_page_y: py,
                    physical_page_x: gx,
                    physical_page_y: gy,
                    mip_level: req.mip_level as u32,
                    flags: page_flags::RESIDENT as u32,
                    _pad0: 0,
                    _pad1: 0,
                });

                loaded += 1;
                self.stats.pages_loaded_this_frame += 1;
            }
        }

        // Update cache hit rate
        if self.stats.feedback_requests > 0 {
            let misses = self.stats.pages_loaded_this_frame;
            let total = self.stats.feedback_requests;
            self.stats.cache_hit_rate = 1.0 - (misses as f32 / total as f32);
        } else {
            self.stats.cache_hit_rate = 1.0;
        }

        self.stats.resident_pages = self.physical_pool.allocated_count();
        &self.pending_updates
    }

    /// Get current frame stats.
    pub fn stats(&self) -> &VTStats {
        &self.stats
    }

    /// Get the pending page updates for GPU upload.
    pub fn pending_updates(&self) -> &[PageUpdate] {
        &self.pending_updates
    }

    /// Current frame number.
    pub fn frame(&self) -> u64 {
        self.frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_process_requests() {
        let config = VTConfig {
            max_pages_per_frame: 10,
            ..Default::default()
        };
        let mut cache = VTCache::new(1024, 1024, config);

        let requests = vec![
            PageRequest {
                texture_id: 0,
                virtual_page_x: 0,
                virtual_page_y: 0,
                mip_level: 0,
                priority: 1.0,
            },
            PageRequest {
                texture_id: 0,
                virtual_page_x: 1,
                virtual_page_y: 0,
                mip_level: 0,
                priority: 1.0,
            },
        ];

        let updates = cache.process_requests(&requests);
        assert_eq!(updates.len(), 2);
        assert_eq!(cache.stats().pages_loaded_this_frame, 2);
        assert!(cache.page_table.is_resident(0, 0));
        assert!(cache.page_table.is_resident(1, 0));

        // Second call with same requests → no new loads (already resident)
        let updates = cache.process_requests(&requests);
        assert_eq!(updates.len(), 0);
        assert_eq!(cache.stats().pages_loaded_this_frame, 0);
    }

    #[test]
    fn test_cache_eviction() {
        let config = VTConfig {
            max_pages_per_frame: 1000,
            ..Default::default()
        };
        // Use a large virtual texture (4096x4096 = 32x32 = 1024 pages)
        // to ensure all physical pool slots (~900) can be filled.
        let mut cache = VTCache::new(4096, 4096, config);
        let pt_w = cache.page_table.width; // 32

        // Fill all physical pool slots
        let total_slots = cache.physical_pool.total_slots();
        let mut requests: Vec<PageRequest> = Vec::new();
        for i in 0..total_slots {
            requests.push(PageRequest {
                texture_id: 0,
                virtual_page_x: (i % pt_w) as u16,
                virtual_page_y: (i / pt_w) as u16,
                mip_level: 0,
                priority: 1.0,
            });
        }
        cache.process_requests(&requests);
        assert_eq!(cache.physical_pool.free_count(), 0);

        // Request a page not yet resident → should trigger eviction
        let new_req = vec![PageRequest {
            texture_id: 0,
            virtual_page_x: 31,
            virtual_page_y: 31,
            mip_level: 0,
            priority: 1.0,
        }];
        // Only works if (31,31) was not already allocated
        // total_slots = 900, pages used = 0..899 → (899 % 32, 899 / 32) = (3, 28)
        // So (31, 31) = index 31*32+31 = 1023 > 900, not yet allocated
        let updates = cache.process_requests(&new_req);
        assert_eq!(updates.len(), 1);
        assert_eq!(cache.stats().pages_evicted_this_frame, 1);
    }
}
