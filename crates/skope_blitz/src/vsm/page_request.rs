// SKOPE Engine - Virtual Shadow Maps: Page Request / Allocation
// GPU compute passes for marking & allocating virtual pages

use bytemuck::{Pod, Zeroable};

/// Per-page flags stored in the `page_flags` storage buffer.
///
/// Bit layout matches `PageTableEntry` flags so the GPU can OR them directly:
///   [16] FLAG_MAPPED
///   [17] FLAG_DIRTY
///   [18] FLAG_REQUESTED
pub const PAGE_FLAG_MAPPED: u32 = 1 << 16;
pub const PAGE_FLAG_DIRTY: u32 = 1 << 17;
pub const PAGE_FLAG_REQUESTED: u32 = 1 << 18;

/// Atomic counter + metadata for the GPU allocation pass.
///
/// Layout (16 bytes, 16-byte aligned):
///   [0]  next_free_index   - atomically incremented to claim pages
///   [1]  total_free_pages  - snapshot of available pages this frame
///   [2]  allocated_count   - how many pages were allocated this frame
///   [3]  _pad
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PageAllocatorData {
    pub next_free_index: u32,
    pub total_free_pages: u32,
    pub allocated_count: u32,
    pub _pad: u32,
}

impl PageAllocatorData {
    pub fn new(total_free: u32) -> Self {
        Self {
            next_free_index: 0,
            total_free_pages: total_free,
            allocated_count: 0,
            _pad: 0,
        }
    }
}

/// Free-list entry uploaded to GPU for the allocate pass.
///
/// Each entry stores the physical page coord that can be assigned.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuFreeListEntry {
    pub physical_x: u32,
    pub physical_y: u32,
}
