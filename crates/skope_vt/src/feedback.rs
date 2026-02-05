//! GPU Feedback Buffer Pipeline
//!
//! Manages the GPU feedback buffer and its CPU readback.
//! The feedback buffer records which virtual texture pages
//! are needed by visible pixels.
//!
//! Pipeline:
//! 1. Material eval appends page requests to the feedback buffer
//! 2. CPU maps the feedback buffer for reading (async)
//! 3. CPU deduplicates and prioritizes page requests
//! 4. Streaming system loads the requested pages

use crate::types::PageRequest;
#[cfg(feature = "gpu")]
use crate::types::VTFeedbackParams;

/// Maximum feedback entries per frame.
pub const MAX_FEEDBACK_ENTRIES: u32 = 65536;

/// GPU feedback pipeline.
#[cfg(feature = "gpu")]
pub struct FeedbackPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub params_layout: wgpu::BindGroupLayout,
    pub feedback_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub feedback_buffer: wgpu::Buffer,
    pub counter_buffer: wgpu::Buffer,
    /// Readback buffer for CPU access (MAP_READ).
    pub readback_buffer: wgpu::Buffer,
    pub readback_counter_buffer: wgpu::Buffer,
    pub max_entries: u32,
}

#[cfg(feature = "gpu")]
impl FeedbackPipeline {
    pub fn new(device: &wgpu::Device, max_entries: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VT Feedback Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/vt_feedback.wgsl").into(),
            ),
        });

        // ── Group 0: Params ──────────────────────────────────────
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VT Feedback G0: Params"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<VTFeedbackParams>() as u64,
                    ),
                },
                count: None,
            }],
        });

        // ── Group 1: Feedback buffer + counter ───────────────────
        let feedback_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VT Feedback G1: Buffer"),
            entries: &[
                // Feedback buffer (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Atomic counter (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(4),
                    },
                    count: None,
                },
            ],
        });

        // ── Pipeline ─────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VT Feedback Pipeline Layout"),
            bind_group_layouts: &[&params_layout, &feedback_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("VT Feedback Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Buffers ──────────────────────────────────────────────
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT Feedback Params"),
            size: std::mem::size_of::<VTFeedbackParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 2 u32 per entry (vec2<u32>)
        let feedback_size = (max_entries as u64) * 8;
        let feedback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT Feedback Buffer"),
            size: feedback_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT Feedback Counter"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Readback buffers (MAP_READ)
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT Feedback Readback"),
            size: feedback_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let readback_counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VT Feedback Counter Readback"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            params_layout,
            feedback_layout,
            params_buffer,
            feedback_buffer,
            counter_buffer,
            readback_buffer,
            readback_counter_buffer,
            max_entries,
        }
    }

    /// Reset the feedback counter to zero before a new frame.
    pub fn reset_counter(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.counter_buffer, 0, &[0u8; 4]);
    }

    /// Copy feedback data from GPU to readback buffers.
    pub fn copy_to_readback(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_buffer_to_buffer(
            &self.counter_buffer,
            0,
            &self.readback_counter_buffer,
            0,
            4,
        );
        encoder.copy_buffer_to_buffer(
            &self.feedback_buffer,
            0,
            &self.readback_buffer,
            0,
            (self.max_entries as u64) * 8,
        );
    }
}

/// Parse raw feedback data from the readback buffer into page requests.
///
/// Deduplicates requests by (texture_id, page_x, page_y, mip).
pub fn parse_feedback(
    data: &[u8],
    entry_count: u32,
    max_entries: u32,
) -> Vec<PageRequest> {
    use std::collections::HashSet;

    let count = entry_count.min(max_entries) as usize;
    let mut seen = HashSet::new();
    let mut requests = Vec::with_capacity(count);

    for i in 0..count {
        let offset = i * 8; // 2 * u32
        if offset + 8 > data.len() {
            break;
        }

        let xy = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let meta = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);

        let page_x = (xy & 0xFFFF) as u16;
        let page_y = ((xy >> 16) & 0xFFFF) as u16;
        let mip = (meta & 0xFF) as u8;
        let tex_id = ((meta >> 8) & 0xFF) as u8;

        let key = (tex_id, page_x, page_y, mip);
        if seen.insert(key) {
            requests.push(PageRequest {
                texture_id: tex_id,
                virtual_page_x: page_x,
                virtual_page_y: page_y,
                mip_level: mip,
                priority: 1.0, // Base priority; could be weighted by frequency
            });
        }
    }

    requests
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_feedback_dedup() {
        // Two identical entries should be deduplicated
        let mut data = Vec::new();
        // Entry: page(3, 5), mip=0, tex_id=0
        let xy: u32 = 3 | (5 << 16);
        let meta: u32 = 0; // mip=0, tex_id=0
        data.extend_from_slice(&xy.to_le_bytes());
        data.extend_from_slice(&meta.to_le_bytes());
        data.extend_from_slice(&xy.to_le_bytes());
        data.extend_from_slice(&meta.to_le_bytes());

        let requests = parse_feedback(&data, 2, 2);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].virtual_page_x, 3);
        assert_eq!(requests[0].virtual_page_y, 5);
    }

    #[test]
    fn test_parse_feedback_multiple() {
        let mut data = Vec::new();
        // Entry 1: page(0, 0)
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        // Entry 2: page(1, 2), mip=1, tex_id=1
        let xy2: u32 = 1 | (2 << 16);
        let meta2: u32 = 1 | (1 << 8);
        data.extend_from_slice(&xy2.to_le_bytes());
        data.extend_from_slice(&meta2.to_le_bytes());

        let requests = parse_feedback(&data, 2, 100);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].mip_level, 1);
        assert_eq!(requests[1].texture_id, 1);
    }
}
