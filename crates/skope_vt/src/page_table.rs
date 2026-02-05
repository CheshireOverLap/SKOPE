//! Virtual Texture Page Table
//!
//! The page table is an indirection texture (RGBA8Uint) that maps
//! virtual page coordinates to physical page locations in the atlas.
//!
//! Each texel encodes: R=physical_x, G=physical_y, B=mip, A=flags.

use crate::types::{PageTableEntry, VT_PAGE_SIZE, page_flags};
#[cfg(feature = "gpu")]
use crate::types::{VT_POOL_SIZE, VT_PAGE_WITH_BORDER};

/// CPU-side page table tracking which pages are resident.
pub struct PageTable {
    /// Page table dimensions (in pages, not texels).
    pub width: u32,
    pub height: u32,
    /// Per-page state, indexed [y * width + x].
    entries: Vec<PageTableEntry>,
}

impl PageTable {
    pub fn new(virtual_width_texels: u32, virtual_height_texels: u32) -> Self {
        let width = (virtual_width_texels + VT_PAGE_SIZE - 1) / VT_PAGE_SIZE;
        let height = (virtual_height_texels + VT_PAGE_SIZE - 1) / VT_PAGE_SIZE;
        let count = (width * height) as usize;
        let entries = vec![
            PageTableEntry {
                physical_x: 0,
                physical_y: 0,
                mip_level: 255, // Not resident
                flags: 0,
                _pad: [0; 2],
            };
            count
        ];
        Self {
            width,
            height,
            entries,
        }
    }

    /// Get the page table entry for a virtual page coordinate.
    pub fn get(&self, page_x: u32, page_y: u32) -> Option<&PageTableEntry> {
        if page_x >= self.width || page_y >= self.height {
            return None;
        }
        Some(&self.entries[(page_y * self.width + page_x) as usize])
    }

    /// Set the mapping for a virtual page.
    pub fn set_mapping(
        &mut self,
        page_x: u32,
        page_y: u32,
        physical_x: u16,
        physical_y: u16,
        mip_level: u8,
    ) {
        if page_x >= self.width || page_y >= self.height {
            return;
        }
        let idx = (page_y * self.width + page_x) as usize;
        self.entries[idx] = PageTableEntry {
            physical_x,
            physical_y,
            mip_level,
            flags: page_flags::RESIDENT,
            _pad: [0; 2],
        };
    }

    /// Clear a page mapping (mark as not resident).
    pub fn clear_mapping(&mut self, page_x: u32, page_y: u32) {
        if page_x >= self.width || page_y >= self.height {
            return;
        }
        let idx = (page_y * self.width + page_x) as usize;
        self.entries[idx].flags = 0;
        self.entries[idx].mip_level = 255;
    }

    /// Check if a page is resident.
    pub fn is_resident(&self, page_x: u32, page_y: u32) -> bool {
        self.get(page_x, page_y)
            .map(|e| e.flags & page_flags::RESIDENT != 0)
            .unwrap_or(false)
    }

    /// Total number of entries.
    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }

    /// Get raw entries slice for GPU upload.
    pub fn entries(&self) -> &[PageTableEntry] {
        &self.entries
    }
}

/// GPU page table update entry (matches WGSL PageUpdate struct).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PageUpdate {
    pub virtual_page_x: u32,
    pub virtual_page_y: u32,
    pub physical_page_x: u32,
    pub physical_page_y: u32,
    pub mip_level: u32,
    pub flags: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

/// GPU page table update pipeline.
#[cfg(feature = "gpu")]
pub struct PageTablePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub params_layout: wgpu::BindGroupLayout,
    pub page_table_layout: wgpu::BindGroupLayout,
    pub page_table_texture: wgpu::Texture,
    pub page_table_view: wgpu::TextureView,
    pub params_buffer: wgpu::Buffer,
    pub updates_buffer: wgpu::Buffer,
    pub max_updates_per_frame: u32,
}

/// Update params (matches WGSL UpdateParams).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UpdateParams {
    pub update_count: u32,
    pub page_table_width: u32,
    pub page_table_height: u32,
    pub atlas_pages_x: u32,
}

#[cfg(feature = "gpu")]
impl PageTablePipeline {
    pub fn new(
        device: &wgpu::Device,
        page_table_width: u32,
        page_table_height: u32,
        max_updates: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VT Page Table Update Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/vt_page_table.wgsl").into(),
            ),
        });

        // ── Group 0: Params + Updates ────────────────────────────
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VT PageTable G0: Params"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<UpdateParams>() as u64,
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ── Group 1: Page table storage texture ──────────────────
        let page_table_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("VT PageTable G1: Texture"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        // ── Pipeline ─────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VT PageTable Pipeline Layout"),
            bind_group_layouts: &[&params_layout, &page_table_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VT PageTable Update Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Resources ────────────────────────────────────────────
        let page_table_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("VT Page Table Texture"),
            size: wgpu::Extent3d {
                width: page_table_width,
                height: page_table_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let page_table_view =
            page_table_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT PageTable Params"),
            size: std::mem::size_of::<UpdateParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let updates_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT PageTable Updates"),
            size: (std::mem::size_of::<PageUpdate>() as u64) * max_updates as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            params_layout,
            page_table_layout,
            page_table_texture,
            page_table_view,
            params_buffer,
            updates_buffer,
            max_updates_per_frame: max_updates,
        }
    }

    /// Upload page updates and dispatch the update compute shader.
    pub fn apply_updates(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        updates: &[PageUpdate],
        page_table_width: u32,
        page_table_height: u32,
    ) {
        if updates.is_empty() {
            return;
        }

        let count = updates.len().min(self.max_updates_per_frame as usize);

        // Upload params
        let params = UpdateParams {
            update_count: count as u32,
            page_table_width,
            page_table_height,
            atlas_pages_x: VT_POOL_SIZE / VT_PAGE_WITH_BORDER,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(
            &self.updates_buffer,
            0,
            bytemuck::cast_slice(&updates[..count]),
        );

        // Create bind groups
        let params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VT PageTable G0"),
            layout: &self.params_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.updates_buffer.as_entire_binding(),
                },
            ],
        });

        let page_table_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("VT PageTable G1"),
            layout: &self.page_table_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.page_table_view),
            }],
        });

        // Dispatch
        let workgroups = (count as u32 + 63) / 64;
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("VT PageTable Update"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &params_bg, &[]);
        pass.set_bind_group(1, &page_table_bg, &[]);
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_table_basic() {
        let mut table = PageTable::new(1024, 1024);
        assert_eq!(table.width, 8); // 1024 / 128
        assert_eq!(table.height, 8);
        assert!(!table.is_resident(0, 0));

        table.set_mapping(3, 5, 1, 2, 0);
        assert!(table.is_resident(3, 5));

        let entry = table.get(3, 5).unwrap();
        assert_eq!(entry.physical_x, 1);
        assert_eq!(entry.physical_y, 2);
        assert_eq!(entry.mip_level, 0);

        table.clear_mapping(3, 5);
        assert!(!table.is_resident(3, 5));
    }

    #[test]
    fn test_page_table_bounds() {
        let table = PageTable::new(512, 512);
        assert!(table.get(100, 100).is_none());
        assert!(!table.is_resident(100, 100));
    }
}
