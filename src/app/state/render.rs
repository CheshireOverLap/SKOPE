//! State render — PerViewBufferPool
//!
//! State::render() / prepare_lighting() 데드 코드 제거 완료 (Step 8).
//! 로직은 RenderState::render_scene() + EngineHandler::pre_render()로 이동됨.

use crate::renderer;

// ---------------------------------------------------------------------------
// Persistent per-view GPU buffer pool (avoids per-frame buffer allocation)
// ---------------------------------------------------------------------------

/// Reusable GPU buffer pool for one camera view.
///
/// Instead of calling `create_buffer_init` N times per frame, this pool
/// keeps persistent buffers and updates them via `queue.write_buffer()`.
pub struct PerViewBufferPool {
    /// Single camera uniform buffer (shared by all instances in a view)
    camera_buffer: wgpu::Buffer,
    /// Per-instance model uniform buffers
    model_buffers: Vec<wgpu::Buffer>,
    /// Per-instance bind groups (binding 0 = camera, binding 1 = model)
    bind_groups: Vec<wgpu::BindGroup>,
    /// Current pool capacity (number of instance slots)
    capacity: usize,
}

impl PerViewBufferPool {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PerView Camera Buffer"),
            size: std::mem::size_of::<renderer::CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_capacity = 64;
        let (model_buffers, bind_groups) = Self::create_slots(
            device, layout, &camera_buffer, initial_capacity,
        );

        Self {
            camera_buffer,
            model_buffers,
            bind_groups,
            capacity: initial_capacity,
        }
    }

    /// Ensure pool has at least `count` instance slots. Grows if needed.
    pub fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        count: usize,
    ) {
        if count <= self.capacity {
            return;
        }
        // Grow to next power of 2
        let new_cap = count.next_power_of_two();
        let (model_buffers, bind_groups) = Self::create_slots(
            device, layout, &self.camera_buffer, new_cap,
        );
        self.model_buffers = model_buffers;
        self.bind_groups = bind_groups;
        self.capacity = new_cap;
    }

    /// Write camera uniform for this view (once per frame).
    pub fn write_camera(&self, queue: &wgpu::Queue, camera: &renderer::CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera));
    }

    /// Write model uniform for instance `i`.
    pub fn write_model(&self, queue: &wgpu::Queue, index: usize, model: &renderer::ModelUniform) {
        queue.write_buffer(&self.model_buffers[index], 0, bytemuck::bytes_of(model));
    }

    /// Get the bind group for instance `i`.
    pub fn bind_group(&self, index: usize) -> &wgpu::BindGroup {
        &self.bind_groups[index]
    }

    fn create_slots(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
        count: usize,
    ) -> (Vec<wgpu::Buffer>, Vec<wgpu::BindGroup>) {
        let mut model_buffers = Vec::with_capacity(count);
        let mut bind_groups = Vec::with_capacity(count);

        for i in 0..count {
            let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("PerView Model Buffer {}", i)),
                size: std::mem::size_of::<renderer::ModelUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("PerView Bind Group {}", i)),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: model_buffer.as_entire_binding(),
                    },
                ],
            });

            model_buffers.push(model_buffer);
            bind_groups.push(bind_group);
        }

        (model_buffers, bind_groups)
    }
}
