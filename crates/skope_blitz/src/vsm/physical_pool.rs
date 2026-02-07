// SKOPE Engine - Virtual Shadow Maps: Physical Page Pool
// Atlas-based Depth32Float texture with free-list + LRU eviction

/// Coordinates of a physical page inside the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPageCoord {
    pub x: u32,
    pub y: u32,
}

/// LRU entry tracking when a physical page was last used.
#[derive(Debug, Clone, Copy)]
struct LruEntry {
    /// Frame index when this page was last touched.
    last_used_frame: u64,
    /// Virtual page that currently owns this physical page (if any).
    virtual_x: u32,
    virtual_y: u32,
    occupied: bool,
}

/// Free-list + LRU physical page allocator.
///
/// The physical atlas is `pool_size x pool_size` pixels, divided into
/// `page_size x page_size` pages.  Total pages = (pool_size / page_size)^2.
pub struct PhysicalPool {
    pub pool_size: u32,
    pub page_size: u32,
    pages_per_side: u32,
    max_pages: u32,

    free_list: Vec<PhysicalPageCoord>,
    lru: Vec<LruEntry>,
}

impl PhysicalPool {
    pub fn new(pool_size: u32, page_size: u32) -> Self {
        let pages_per_side = pool_size / page_size;
        let max_pages = pages_per_side * pages_per_side;

        // Build free list (all pages start free)
        let mut free_list = Vec::with_capacity(max_pages as usize);
        for y in 0..pages_per_side {
            for x in 0..pages_per_side {
                free_list.push(PhysicalPageCoord { x, y });
            }
        }

        let lru = vec![
            LruEntry {
                last_used_frame: 0,
                virtual_x: 0,
                virtual_y: 0,
                occupied: false,
            };
            max_pages as usize
        ];

        Self {
            pool_size,
            page_size,
            pages_per_side,
            max_pages,
            free_list,
            lru,
        }
    }

    /// Try to allocate a physical page. Returns `None` when the pool is full
    /// (caller should evict first).
    pub fn allocate(&mut self, virtual_x: u32, virtual_y: u32, frame: u64) -> Option<PhysicalPageCoord> {
        if let Some(coord) = self.free_list.pop() {
            let idx = self.page_index(coord);
            self.lru[idx] = LruEntry {
                last_used_frame: frame,
                virtual_x,
                virtual_y,
                occupied: true,
            };
            Some(coord)
        } else {
            None
        }
    }

    /// Mark a physical page as recently used.
    pub fn touch(&mut self, coord: PhysicalPageCoord, frame: u64) {
        let idx = self.page_index(coord);
        self.lru[idx].last_used_frame = frame;
    }

    /// Free a specific physical page back to the pool.
    pub fn free(&mut self, coord: PhysicalPageCoord) {
        let idx = self.page_index(coord);
        if self.lru[idx].occupied {
            self.lru[idx].occupied = false;
            self.free_list.push(coord);
        }
    }

    /// Evict the least-recently-used page, returning its coord and the
    /// virtual page it was mapped to.
    pub fn evict_lru(&mut self) -> Option<(PhysicalPageCoord, u32, u32)> {
        let mut oldest_frame = u64::MAX;
        let mut oldest_idx: Option<usize> = None;

        for (i, entry) in self.lru.iter().enumerate() {
            if entry.occupied && entry.last_used_frame < oldest_frame {
                oldest_frame = entry.last_used_frame;
                oldest_idx = Some(i);
            }
        }

        if let Some(idx) = oldest_idx {
            let entry = self.lru[idx];
            let coord = PhysicalPageCoord {
                x: (idx as u32) % self.pages_per_side,
                y: (idx as u32) / self.pages_per_side,
            };
            self.lru[idx].occupied = false;
            self.free_list.push(coord);
            Some((coord, entry.virtual_x, entry.virtual_y))
        } else {
            None
        }
    }

    pub fn free_count(&self) -> u32 {
        self.free_list.len() as u32
    }

    /// Access the current free list for GPU upload.
    pub fn free_list_entries(&self) -> &[PhysicalPageCoord] {
        &self.free_list
    }

    pub fn max_pages(&self) -> u32 {
        self.max_pages
    }

    pub fn pages_per_side(&self) -> u32 {
        self.pages_per_side
    }

    fn page_index(&self, coord: PhysicalPageCoord) -> usize {
        (coord.y * self.pages_per_side + coord.x) as usize
    }
}
