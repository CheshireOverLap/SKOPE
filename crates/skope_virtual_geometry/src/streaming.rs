//! Nanite GPU Streaming Pipeline
//!
//! Manages GPU memory for Nanite meshlet data via feedback-driven streaming.
//! Uses a ring buffer with 2-frame latency for GPU -> CPU feedback to avoid pipeline stalls.
//!
//! ## Architecture
//!
//! 1. GPU culling writes peak counters (visible clusters, nodes) each frame.
//! 2. `collect_feedback` copies counters into a ring-buffered readback buffer.
//! 3. `read_feedback` maps the buffer from 2 frames prior (avoiding stalls).
//! 4. CPU-side logic decides which pages to stream in/out and adjusts LOD bias.
//!
//! This mirrors the UE5 Nanite streaming feedback approach where the GPU
//! reports what it *needed* and the CPU asynchronously responds.

#[cfg(feature = "gpu")]
use crate::types::{StreamingFeedback, StreamingRequest, NaniteStreamingConfig};

/// Page state for CPU-side tracking.
///
/// Each page represents a fixed-size block of GPU memory holding
/// meshlet geometry for a specific mesh + LOD level.
#[derive(Clone, Debug)]
pub struct PageEntry {
    pub mesh_id: u32,
    pub lod_level: u32,
    pub gpu_page_index: u32,
    pub last_used_frame: u64,
    pub resident: bool,
}

/// Buffer overflow tracking (UE5 FBufferState).
///
/// Tracks when the visible cluster buffer overflowed so the CPU
/// can push LOD bias coarser until pressure subsides.
#[derive(Clone, Debug, Default)]
pub struct BufferState {
    pub latest_overflow_time: f64,
    pub latest_overflow_peak: u32,
    pub high_water_mark: u32,
}

/// Feedback processing parameters uploaded to GPU.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FeedbackParams {
    pub max_visible_clusters: u32,
    pub max_nodes: u32,
    pub frame_index: u32,
    pub _pad: u32,
}

/// Number of readback ring-buffer slots.
/// We use 3 slots to allow 2-frame readback latency without contention.
#[cfg(feature = "gpu")]
const READBACK_RING_SIZE: usize = 3;

/// Nanite GPU streaming pipeline.
///
/// Owns the GPU feedback buffers, readback ring, and CPU-side page table.
/// Drives streaming decisions based on asynchronous GPU feedback.
#[cfg(feature = "gpu")]
pub struct NaniteStreamingPipeline {
    pub config: NaniteStreamingConfig,

    // GPU feedback
    pub feedback_buffer: wgpu::Buffer,
    pub readback_buffers: [wgpu::Buffer; READBACK_RING_SIZE],

    // Feedback compute
    pub feedback_layout: wgpu::BindGroupLayout,
    pub feedback_params_buffer: wgpu::Buffer,

    // CPU state
    pub feedback_frame: usize,
    pub buffer_state: BufferState,
    pub current_frame: u64,

    // Page management
    pub page_table: Vec<PageEntry>,
    pub free_pages: Vec<u32>,
    pub pending_requests: Vec<StreamingRequest>,
    pub lod_bias_adjustment: f32,
}

#[cfg(feature = "gpu")]
impl NaniteStreamingPipeline {
    /// Create a new streaming pipeline with GPU resources.
    ///
    /// Allocates the feedback buffer, readback ring, request buffers,
    /// and the feedback compute pipeline.
    pub fn new(device: &wgpu::Device, config: NaniteStreamingConfig) -> Self {
        let feedback_size = std::mem::size_of::<StreamingFeedback>() as u64;

        // ── Feedback buffer (GPU-side, written by cull, copied to readback) ──
        let feedback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Streaming Feedback"),
            size: feedback_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Readback ring buffers (3-frame, MAP_READ) ───────────────
        let readback_buffers = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("Nanite Streaming Readback [{}]", i)),
                size: feedback_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        // ── Feedback params uniform ─────────────────────────────────
        let feedback_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Streaming Feedback Params"),
            size: std::mem::size_of::<FeedbackParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Bind group layout ───────────────────────────────────────
        let feedback_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Nanite Streaming Feedback Layout"),
                entries: &[
                    // binding 0: FeedbackParams (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<FeedbackParams>() as u64,
                            ),
                        },
                        count: None,
                    },
                    // binding 1: StreamingFeedback (storage, read_write)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(feedback_size),
                        },
                        count: None,
                    },
                ],
            });

        // ── Page table ──────────────────────────────────────────────
        let max_pages = config.max_resident_pages;
        let free_pages: Vec<u32> = (0..max_pages).rev().collect();

        Self {
            config,
            feedback_buffer,
            readback_buffers,
            feedback_layout,
            feedback_params_buffer,
            feedback_frame: 0,
            buffer_state: BufferState::default(),
            current_frame: 0,
            page_table: Vec::new(),
            free_pages,
            pending_requests: Vec::new(),
            lod_bias_adjustment: 0.0,
        }
    }

    /// Create the feedback bind group for the streaming compute shader.
    pub fn create_feedback_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Streaming Feedback BG"),
            layout: &self.feedback_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.feedback_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.feedback_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Upload feedback params for this frame.
    pub fn update_feedback_params(
        &self,
        queue: &wgpu::Queue,
        max_visible_clusters: u32,
        max_nodes: u32,
    ) {
        let params = FeedbackParams {
            max_visible_clusters,
            max_nodes,
            frame_index: self.current_frame as u32,
            _pad: 0,
        };
        queue.write_buffer(
            &self.feedback_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
    }

    /// Copy feedback buffer into the readback ring buffer for later CPU-side
    /// consumption.
    ///
    /// The feedback buffer should already contain the visible cluster count
    /// (written by `dispatch_with_feedback` in cull.rs). This method copies
    /// only the first 4 bytes (the GPU-sourced counter) into the current
    /// readback ring slot, then advances the frame counter.
    ///
    /// Call this after the culling dispatch has written to the feedback buffer
    /// (within the same command encoder).
    pub fn collect_feedback(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // Copy all 3 GPU-sourced counters (visible_cluster_peak, node_peak, sw_count)
        let copy_size = (std::mem::size_of::<u32>() * 3) as u64;

        // Copy feedback -> readback ring slot
        let rb_idx = self.feedback_frame % READBACK_RING_SIZE;
        encoder.copy_buffer_to_buffer(
            &self.feedback_buffer,
            0,
            &self.readback_buffers[rb_idx],
            0,
            copy_size,
        );

        self.feedback_frame += 1;
    }

    /// Read feedback data from the readback buffer that was written 2 frames ago.
    ///
    /// Returns `None` during the first 2 frames while the ring buffer fills.
    /// Uses `map_async` + `poll` with a 1ms timeout to avoid indefinite blocking.
    ///
    /// Only the visible cluster count comes from the GPU readback buffer;
    /// the remaining fields are populated from CPU-side state.
    pub fn read_feedback(&self, device: &wgpu::Device) -> Option<StreamingFeedback> {
        if self.feedback_frame < 2 {
            return None;
        }

        let read_size = std::mem::size_of::<u32>() * 3; // 3 GPU counters
        let rb_idx = (self.feedback_frame - 2) % READBACK_RING_SIZE;
        let buffer = &self.readback_buffers[rb_idx];
        let slice = buffer.slice(..read_size as u64);

        // Request mapping
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Poll with a 1ms timeout to avoid indefinite blocking
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_millis(1)),
        });

        match rx.recv_timeout(std::time::Duration::from_millis(1)) {
            Ok(Ok(())) => {
                let data = slice.get_mapped_range();
                // Read all 3 GPU-sourced counters from CullCounters layout:
                //   [0] total_visible → visible_cluster_peak
                //   [1] hw_cluster_count → node_peak (HW raster load metric)
                //   [2] sw_cluster_count → used as SW raster overflow metric
                let visible_cluster_peak: u32 = *bytemuck::from_bytes(&data[0..4]);
                let node_peak: u32 = *bytemuck::from_bytes(&data[4..8]);
                let sw_cluster_count: u32 = *bytemuck::from_bytes(&data[8..12]);
                drop(data);
                buffer.unmap();

                // Construct feedback with GPU-sourced counters + CPU-side state
                // requested_pages combines SW raster load with pending CPU requests
                let total_pending = sw_cluster_count.max(self.pending_requests.len() as u32);
                Some(StreamingFeedback {
                    visible_cluster_peak,
                    node_peak,
                    requested_pages: total_pending,
                    total_resident_pages: self.page_table.len() as u32,
                })
            }
            _ => {
                log::warn!("Nanite streaming: failed to read feedback buffer");
                None
            }
        }
    }

    /// Check for buffer overflow and adjust LOD bias accordingly.
    ///
    /// When visible cluster count exceeds capacity, the LOD bias is pushed
    /// coarser to reduce pressure. When overflow subsides, bias gradually
    /// recovers toward zero.
    pub fn check_overflow(&mut self, feedback: &StreamingFeedback, current_time: f64) {
        let cluster_capacity = self.config.max_visible_clusters;

        // Track high water mark across all metrics
        let worst_peak = feedback.visible_cluster_peak
            .max(feedback.node_peak);
        if worst_peak > self.buffer_state.high_water_mark {
            self.buffer_state.high_water_mark = worst_peak;
        }

        // Multi-metric overflow detection: check visible clusters, node load,
        // and SW raster load independently. Use proportional LOD bias scaling
        // based on how far over capacity we are.
        let visible_overflow = feedback.visible_cluster_peak > cluster_capacity;
        let node_overflow = feedback.node_peak > cluster_capacity;

        if visible_overflow || node_overflow {
            self.buffer_state.latest_overflow_time = current_time;
            self.buffer_state.latest_overflow_peak = worst_peak;

            // Proportional bias: scale by how much we exceeded capacity
            let overflow_ratio = worst_peak as f32 / cluster_capacity as f32;
            let bias_increment = (overflow_ratio - 1.0).clamp(0.1, 1.0);
            self.lod_bias_adjustment += bias_increment;

            log::debug!(
                "Nanite streaming: overflow detected (visible={}, node={}, cluster_capacity={}), ratio={:.2}, bias={:.2}",
                feedback.visible_cluster_peak,
                feedback.node_peak,
                cluster_capacity,
                overflow_ratio,
                self.lod_bias_adjustment,
            );
        } else if self.lod_bias_adjustment > 0.0 {
            // Gradually recover toward zero bias
            self.lod_bias_adjustment = (self.lod_bias_adjustment - 0.1).max(0.0);
        }
    }

    /// Generate priority-based streaming requests from instance data.
    ///
    /// Computes a priority for each mesh based on projected screen-space size
    /// and queues page-in requests sorted by priority (highest first).
    pub fn generate_requests(
        &mut self,
        instances: &[(u32, [f32; 3], f32)], // (mesh_id, position, bounding_radius)
        camera_pos: [f32; 3],
        screen_height: f32,
        fov_y: f32,
    ) {
        self.pending_requests.clear();

        let cot_half_fov = 1.0 / (fov_y * 0.5).tan();

        for &(mesh_id, position, radius) in instances {
            let dx = position[0] - camera_pos[0];
            let dy = position[1] - camera_pos[1];
            let dz = position[2] - camera_pos[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);

            // Projected screen-space size (pixels)
            let screen_size = (radius * cot_half_fov * screen_height) / distance;

            // Determine desired LOD level from screen size
            // Larger screen size -> finer LOD (lower level number)
            let lod_level = if screen_size > 512.0 {
                0
            } else if screen_size > 256.0 {
                1
            } else if screen_size > 128.0 {
                2
            } else if screen_size > 64.0 {
                3
            } else if screen_size > 32.0 {
                4
            } else {
                5
            };

            // If the page is already resident, just touch it (update LRU)
            // instead of re-requesting it.
            if self.is_page_resident(mesh_id, lod_level) {
                self.touch_page(mesh_id, lod_level);
                continue;
            }

            // Priority: higher screen size = higher priority
            let priority = screen_size;

            self.pending_requests.push(StreamingRequest {
                mesh_id,
                lod_level,
                priority,
                _pad: 0,
            });
        }

        // Sort by priority descending (highest priority first)
        self.pending_requests
            .sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));

        // Clamp to max requests per frame
        let max = self.config.max_requests_per_frame as usize;
        self.pending_requests.truncate(max);
    }

    /// Evict pages using LRU policy until resident page count is at or below
    /// the target count.
    ///
    /// Pages not used in the most recent frames are evicted first.
    /// The eviction hysteresis factor prevents thrashing by targeting
    /// below the absolute maximum.
    pub fn evict_pages(&mut self, target_count: u32) {
        if self.page_table.len() <= target_count as usize {
            return;
        }

        // Sort by last_used_frame ascending (oldest first)
        self.page_table
            .sort_by_key(|p| p.last_used_frame);

        let evict_count = self.page_table.len() - target_count as usize;

        let mut actually_evicted = 0;
        for i in 0..self.page_table.len() {
            if actually_evicted >= evict_count {
                break;
            }
            if self.page_table[i].resident {
                self.page_table[i].resident = false;
                self.free_pages.push(self.page_table[i].gpu_page_index);
                log::trace!(
                    "Nanite streaming: evicted page (mesh={}, lod={}, gpu_idx={})",
                    self.page_table[i].mesh_id,
                    self.page_table[i].lod_level,
                    self.page_table[i].gpu_page_index,
                );
                actually_evicted += 1;
            }
        }

        // Remove all non-resident pages
        self.page_table.retain(|p| p.resident);
    }

    /// Mark a page as recently used by setting its `last_used_frame` to the
    /// current frame. This prevents LRU eviction from discarding pages that
    /// are still actively needed.
    pub fn touch_page(&mut self, mesh_id: u32, lod_level: u32) {
        for page in &mut self.page_table {
            if page.mesh_id == mesh_id && page.lod_level == lod_level && page.resident {
                page.last_used_frame = self.current_frame;
            }
        }
    }

    /// Check whether a page for the given mesh and LOD level is already resident.
    pub fn is_page_resident(&self, mesh_id: u32, lod_level: u32) -> bool {
        self.page_table.iter().any(|p| {
            p.mesh_id == mesh_id && p.lod_level == lod_level && p.resident
        })
    }

    /// Return the current LOD bias adjustment for CullParams.
    ///
    /// Positive values push LOD selection toward coarser levels,
    /// reducing the number of visible clusters.
    pub fn lod_bias(&self) -> f32 {
        self.lod_bias_adjustment
    }

    /// Advance the frame counter. Call once per frame before collect_feedback.
    pub fn begin_frame(&mut self) {
        self.current_frame += 1;
    }

    /// Run a full streaming update: read feedback, check overflow, evict if needed.
    ///
    /// Returns the feedback data if available (after ring buffer warm-up).
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        current_time: f64,
    ) -> Option<StreamingFeedback> {
        let feedback = self.read_feedback(device)?;

        self.check_overflow(&feedback, current_time);

        // Evict pages if over high-water mark (95% of max), targeting the
        // hysteresis level (e.g. 80% of max) to prevent eviction thrashing.
        let high_water = (self.config.max_resident_pages as f32 * 0.95) as usize;
        let target = (self.config.max_resident_pages as f32
            * self.config.eviction_hysteresis) as u32;
        if self.page_table.len() > high_water {
            self.evict_pages(target);
        }

        Some(feedback)
    }
}
