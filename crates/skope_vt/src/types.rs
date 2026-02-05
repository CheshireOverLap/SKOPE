//! Virtual Texture GPU data types.

use bytemuck::{Pod, Zeroable};

/// Physical page size in texels (excluding border).
pub const VT_PAGE_SIZE: u32 = 128;
/// Border texels for bilinear/anisotropic filtering across page boundaries.
pub const VT_BORDER_SIZE: u32 = 4;
/// Total page tile size including border.
pub const VT_PAGE_WITH_BORDER: u32 = VT_PAGE_SIZE + VT_BORDER_SIZE * 2; // 136
/// Physical atlas texture size (square).
pub const VT_POOL_SIZE: u32 = 4096;
/// Maximum number of physical pages in the atlas.
pub const VT_MAX_PAGES: u32 = (VT_POOL_SIZE / VT_PAGE_WITH_BORDER) * (VT_POOL_SIZE / VT_PAGE_WITH_BORDER);

/// A page table entry mapping virtual page coordinates to physical page position.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct PageTableEntry {
    /// Physical page X coordinate in the atlas.
    pub physical_x: u16,
    /// Physical page Y coordinate in the atlas.
    pub physical_y: u16,
    /// Currently loaded mip level (0 = finest).
    pub mip_level: u8,
    /// Flags: RESIDENT(1), LOADING(2), FALLBACK(4).
    pub flags: u8,
    pub _pad: [u8; 2],
}

/// GPU feedback entry recording which virtual pages were requested.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FeedbackEntry {
    /// Virtual page X coordinate.
    pub virtual_page_x: u16,
    /// Virtual page Y coordinate.
    pub virtual_page_y: u16,
    /// Requested mip level.
    pub mip_level: u8,
    /// Virtual texture ID.
    pub texture_id: u8,
    pub _pad: [u8; 2],
}

/// Parameters for the VT feedback generation shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct VTFeedbackParams {
    /// Page table dimensions.
    pub page_table_width: u32,
    pub page_table_height: u32,
    /// Physical atlas dimensions in pages.
    pub atlas_pages_x: u32,
    pub atlas_pages_y: u32,
    /// Maximum feedback entries to write.
    pub max_feedback_entries: u32,
    /// Current atomic counter offset.
    pub feedback_counter_offset: u32,
    pub _pad: [u32; 2],
}

/// Configuration for the virtual texture system.
#[derive(Clone, Debug)]
pub struct VTConfig {
    /// Maximum number of pages to load per frame.
    pub max_pages_per_frame: u32,
    /// Feedback buffer downsample factor (1 = full res, 4 = quarter).
    pub feedback_downsample: u32,
    /// Maximum anisotropic filtering level.
    pub aniso_level: u32,
    /// Maximum number of mip levels.
    pub max_mip_levels: u32,
    /// Enable asynchronous page loading.
    pub async_loading: bool,
}

impl Default for VTConfig {
    fn default() -> Self {
        Self {
            max_pages_per_frame: 32,
            feedback_downsample: 4,
            aniso_level: 8,
            max_mip_levels: 12,
            async_loading: true,
        }
    }
}

/// Virtual texture page request (CPU-side).
#[derive(Clone, Debug)]
pub struct PageRequest {
    pub texture_id: u8,
    pub virtual_page_x: u16,
    pub virtual_page_y: u16,
    pub mip_level: u8,
    pub priority: f32,
}

/// Runtime statistics for the VT system.
#[derive(Clone, Debug, Default)]
pub struct VTStats {
    pub resident_pages: u32,
    pub pages_loaded_this_frame: u32,
    pub pages_evicted_this_frame: u32,
    pub feedback_requests: u32,
    pub cache_hit_rate: f32,
}

/// Page table entry flags.
pub mod page_flags {
    pub const RESIDENT: u8 = 1;
    pub const LOADING: u8 = 2;
    pub const FALLBACK: u8 = 4;
}
