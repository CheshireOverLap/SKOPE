// SKOPE Engine - VSM Cache Manager
//
// Tracks per-page dirty state for Virtual Shadow Maps.
// Static geometry pages are cached across frames; only pages
// affected by dynamic objects are re-rendered.
//
// Inspired by UE5's VSM cache invalidation system.

#![allow(dead_code)]

/// Per-page cache state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PageCacheState {
    /// Page has no allocated physical tile.
    Unallocated,
    /// Page was just allocated this frame — needs full render.
    NewlyAllocated,
    /// Page is cached from a previous frame — valid depth data.
    Cached,
    /// Page was invalidated (dynamic object moved) — needs re-render.
    Invalidated,
}

/// VSM cache manager.
///
/// Maintains a grid of cache states matching the virtual page table.
/// The renderer queries this to determine which pages need shadow rendering.
pub struct VsmCacheManager {
    /// Page cache states: `page_table_size x page_table_size`.
    states: Vec<PageCacheState>,
    page_table_size: u32,
    /// Indices of pages that need rendering this frame.
    dirty_pages: Vec<u32>,
    /// Frame counter for LRU eviction.
    frame_index: u64,
    /// Per-page last-used frame (for LRU eviction of physical tiles).
    last_used: Vec<u64>,
}

impl VsmCacheManager {
    pub fn new(page_table_size: u32) -> Self {
        let total = (page_table_size * page_table_size) as usize;
        Self {
            states: vec![PageCacheState::Unallocated; total],
            page_table_size,
            dirty_pages: Vec::with_capacity(total / 4),
            frame_index: 0,
            last_used: vec![0; total],
        }
    }

    /// Begin a new frame. Resets dirty list.
    pub fn begin_frame(&mut self) {
        self.frame_index += 1;
        self.dirty_pages.clear();
    }

    /// Mark a page as newly allocated (needs full render).
    pub fn mark_allocated(&mut self, page_x: u32, page_y: u32) {
        let idx = self.page_index(page_x, page_y);
        self.states[idx] = PageCacheState::NewlyAllocated;
        self.dirty_pages.push(idx as u32);
        self.last_used[idx] = self.frame_index;
    }

    /// Mark a page as invalidated (dynamic object moved through it).
    pub fn invalidate(&mut self, page_x: u32, page_y: u32) {
        let idx = self.page_index(page_x, page_y);
        if self.states[idx] == PageCacheState::Cached {
            self.states[idx] = PageCacheState::Invalidated;
            self.dirty_pages.push(idx as u32);
        }
        self.last_used[idx] = self.frame_index;
    }

    /// Invalidate all pages overlapping a world-space bounding sphere.
    /// `page_min`/`page_max` are the page-space bounds of the sphere.
    pub fn invalidate_range(&mut self, page_min_x: u32, page_min_y: u32, page_max_x: u32, page_max_y: u32) {
        let max_x = page_max_x.min(self.page_table_size - 1);
        let max_y = page_max_y.min(self.page_table_size - 1);
        for py in page_min_y..=max_y {
            for px in page_min_x..=max_x {
                self.invalidate(px, py);
            }
        }
    }

    /// After rendering, mark all dirty pages as cached.
    pub fn mark_rendered(&mut self) {
        for &page_idx in &self.dirty_pages {
            let idx = page_idx as usize;
            if idx < self.states.len() {
                self.states[idx] = PageCacheState::Cached;
            }
        }
    }

    /// Get the list of pages that need rendering this frame.
    pub fn dirty_pages(&self) -> &[u32] {
        &self.dirty_pages
    }

    /// Number of dirty pages.
    pub fn dirty_count(&self) -> usize {
        self.dirty_pages.len()
    }

    /// Check if a specific page needs rendering.
    pub fn needs_render(&self, page_x: u32, page_y: u32) -> bool {
        let idx = self.page_index(page_x, page_y);
        matches!(self.states[idx], PageCacheState::NewlyAllocated | PageCacheState::Invalidated)
    }

    /// Get pages that haven't been used for `threshold` frames (candidates for eviction).
    pub fn find_stale_pages(&self, threshold: u64) -> Vec<u32> {
        let cutoff = self.frame_index.saturating_sub(threshold);
        self.states.iter().enumerate()
            .filter(|(i, state)| {
                **state == PageCacheState::Cached && self.last_used[*i] < cutoff
            })
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// Reset all pages to unallocated.
    pub fn reset(&mut self) {
        self.states.fill(PageCacheState::Unallocated);
        self.dirty_pages.clear();
        self.last_used.fill(0);
    }

    fn page_index(&self, x: u32, y: u32) -> usize {
        (y * self.page_table_size + x) as usize
    }

    pub fn page_table_size(&self) -> u32 {
        self.page_table_size
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }
}
