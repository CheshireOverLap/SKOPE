// SKOPE Engine - Virtual Shadow Maps: Page Table
// Maps virtual pages (128x128 grid) to physical pages in the atlas

use bytemuck::{Pod, Zeroable};

/// Page table entry stored as R32Uint in the page table texture.
///
/// Bit layout:
///   [4:0]   physical_x  (0..31)
///   [9:5]   physical_y  (0..31)
///   [15:10] reserved
///   [16]    FLAG_MAPPED    - page has a physical allocation
///   [17]    FLAG_DIRTY     - page needs re-render
///   [18]    FLAG_REQUESTED - page was requested this frame
///   [31:19] reserved
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PageTableEntry {
    pub packed: u32,
}

impl PageTableEntry {
    pub const FLAG_MAPPED: u32 = 1 << 16;
    pub const FLAG_DIRTY: u32 = 1 << 17;
    pub const FLAG_REQUESTED: u32 = 1 << 18;

    pub const UNMAPPED: Self = Self { packed: 0 };

    pub fn new(physical_x: u32, physical_y: u32, flags: u32) -> Self {
        debug_assert!(physical_x < 32, "physical_x must be < 32");
        debug_assert!(physical_y < 32, "physical_y must be < 32");
        Self {
            packed: (physical_x & 0x1F)
                | ((physical_y & 0x1F) << 5)
                | flags,
        }
    }

    pub fn physical_x(self) -> u32 {
        self.packed & 0x1F
    }

    pub fn physical_y(self) -> u32 {
        (self.packed >> 5) & 0x1F
    }

    pub fn is_mapped(self) -> bool {
        self.packed & Self::FLAG_MAPPED != 0
    }

    pub fn is_dirty(self) -> bool {
        self.packed & Self::FLAG_DIRTY != 0
    }

    pub fn is_requested(self) -> bool {
        self.packed & Self::FLAG_REQUESTED != 0
    }
}

/// CPU-side page table tracking.
///
/// The actual page table lives on GPU as a R32Uint texture (128x128).
/// This mirrors the mapping for CPU-side bookkeeping.
pub struct PageTable {
    /// page_table_size x page_table_size entries
    entries: Vec<PageTableEntry>,
    pub page_table_size: u32,
}

impl PageTable {
    pub fn new(page_table_size: u32) -> Self {
        let count = (page_table_size * page_table_size) as usize;
        Self {
            entries: vec![PageTableEntry::UNMAPPED; count],
            page_table_size,
        }
    }

    pub fn get(&self, x: u32, y: u32) -> PageTableEntry {
        let idx = (y * self.page_table_size + x) as usize;
        self.entries[idx]
    }

    pub fn set(&mut self, x: u32, y: u32, entry: PageTableEntry) {
        let idx = (y * self.page_table_size + x) as usize;
        self.entries[idx] = entry;
    }

    /// Mark all entries as unmapped (full invalidation).
    pub fn clear(&mut self) {
        self.entries.fill(PageTableEntry::UNMAPPED);
    }

    pub fn entries(&self) -> &[PageTableEntry] {
        &self.entries
    }

    /// Raw u32 data suitable for uploading to the GPU texture.
    pub fn as_u32_slice(&self) -> &[u32] {
        bytemuck::cast_slice(&self.entries)
    }
}
