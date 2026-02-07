// SKOPE Engine - GPU Scene
//
// Unified GPU-side scene data management, inspired by UE5's FGPUScene.
//
// All renderable instances are stored in a GPU buffer (SoA layout) and
// uploaded incrementally each frame.  Every rendering pass (Nanite, VSM,
// MegaLights, Instance Culling, Material Eval) reads from this single
// source of truth instead of constructing its own per-frame uploads.
//
// ## Data flow
//
// ```text
// CPU: add_instance / update_instance / remove_instance
//   |
//   v
// dirty_list  (indices that changed this frame)
//   |
//   v
// upload()  →  gpu_instance_buffer  (storage buffer, read by all passes)
// ```

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of instances supported by the GPU Scene.
/// Can be increased; the GPU buffer is resized dynamically.
pub const MAX_GPU_INSTANCES: u32 = 65_536;

/// Instance flag bits (matches WGSL constants)
pub mod instance_flags {
    pub const VISIBLE: u32 = 1 << 0;
    pub const SHADOW_CASTER: u32 = 1 << 1;
    pub const MOVABLE: u32 = 1 << 2;
    pub const NANITE: u32 = 1 << 3;
    pub const SKINNED: u32 = 1 << 4;
    pub const TRANSPARENT: u32 = 1 << 5;
    pub const TWO_SIDED: u32 = 1 << 6;
    pub const RECEIVES_DECALS: u32 = 1 << 7;
}

// ---------------------------------------------------------------------------
// GPU structs (must match WGSL)
// ---------------------------------------------------------------------------

/// Per-instance data stored on the GPU.
///
/// 192 bytes, 16-byte aligned.  Matches the WGSL `GpuInstance` struct in
/// `shaders/gpu_scene.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuInstance {
    /// Current frame world transform (model → world)
    pub world_matrix: [[f32; 4]; 4],        // 64 bytes (offset 0)
    /// Previous frame world transform (for motion vectors / temporal)
    pub prev_world_matrix: [[f32; 4]; 4],    // 64 bytes (offset 64)
    /// Bounding sphere center (world space)
    pub bounds_center: [f32; 3],             // 12 bytes (offset 128)
    /// Bounding sphere radius (world space)
    pub bounds_radius: f32,                   // 4 bytes  (offset 140)
    /// Index into the unified mesh data (vertex/index offsets)
    pub mesh_id: u32,                         // 4 bytes  (offset 144)
    /// Index into the GPU material array
    pub material_id: u32,                     // 4 bytes  (offset 148)
    /// Bitfield: VISIBLE | SHADOW_CASTER | MOVABLE | NANITE | ...
    pub flags: u32,                           // 4 bytes  (offset 152)
    /// Application-specific payload (e.g. LOD bias, custom stencil)
    pub custom_data: u32,                     // 4 bytes  (offset 156)
    /// Vertex offset in unified geometry buffer
    pub vertex_offset: u32,                   // 4 bytes  (offset 160)
    /// Index offset in unified geometry buffer
    pub index_offset: u32,                    // 4 bytes  (offset 164)
    /// Number of indices
    pub index_count: u32,                     // 4 bytes  (offset 168)
    /// LOD level (0 = highest detail)
    pub lod_level: u32,                       // 4 bytes  (offset 172)
    /// Padding to 192 bytes (multiple of 16)
    pub _pad: [f32; 4],                       // 16 bytes (offset 176)
    // Total: 192 bytes
}

impl Default for GpuInstance {
    fn default() -> Self {
        let identity = Mat4::IDENTITY.to_cols_array_2d();
        Self {
            world_matrix: identity,
            prev_world_matrix: identity,
            bounds_center: [0.0; 3],
            bounds_radius: 1.0,
            mesh_id: 0,
            material_id: 0,
            flags: instance_flags::VISIBLE | instance_flags::SHADOW_CASTER,
            custom_data: 0,
            vertex_offset: 0,
            index_offset: 0,
            index_count: 0,
            lod_level: 0,
            _pad: [0.0; 4],
        }
    }
}

/// GPU Scene parameters uniform (uploaded each frame).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuSceneParams {
    pub instance_count: u32,
    pub frame_index: u32,
    pub _pad: [u32; 2],
}

// ---------------------------------------------------------------------------
// CPU-side instance descriptor (what the game/editor hands us)
// ---------------------------------------------------------------------------

/// Describes an instance to be added or updated in the GPU Scene.
#[derive(Clone, Debug)]
pub struct InstanceDesc {
    pub world_transform: Mat4,
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
    pub mesh_id: u32,
    pub material_id: u32,
    pub flags: u32,
    pub custom_data: u32,
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub lod_level: u32,
}

impl Default for InstanceDesc {
    fn default() -> Self {
        Self {
            world_transform: Mat4::IDENTITY,
            bounds_center: Vec3::ZERO,
            bounds_radius: 1.0,
            mesh_id: 0,
            material_id: 0,
            flags: instance_flags::VISIBLE | instance_flags::SHADOW_CASTER,
            custom_data: 0,
            vertex_offset: 0,
            index_offset: 0,
            index_count: 0,
            lod_level: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

/// Opaque handle returned by `add_instance`.
/// Internally is just an index, but typed for safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u32);

// ---------------------------------------------------------------------------
// GPU Scene
// ---------------------------------------------------------------------------

/// GPU Scene manages all renderable instances on the GPU.
///
/// Usage per frame:
/// 1. Add / update / remove instances via `add_instance`, `update_transform`, `remove_instance`
/// 2. Call `upload(queue)` to flush dirty data to the GPU buffer
/// 3. Bind `instance_buffer()` in passes that need instance data
pub struct GpuScene {
    // CPU mirror of instance data
    instances: Vec<Option<GpuInstance>>,

    // Free-list for recycled slots
    free_list: Vec<u32>,

    // Indices modified since last upload
    dirty_indices: Vec<u32>,

    // GPU resources
    instance_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,

    // Bind group for passes that read the GPU scene
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // Stats
    live_count: u32,
    capacity: u32,
    frame_index: u64,
}

impl GpuScene {
    pub fn new(device: &wgpu::Device) -> Self {
        let capacity = 1024u32; // Start small, grow as needed

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Scene Instance Buffer"),
            size: (capacity as u64) * std::mem::size_of::<GpuInstance>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Scene Params"),
            size: std::mem::size_of::<GpuSceneParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GPU Scene Bind Group Layout"),
            entries: &[
                // binding 0: instance data (storage, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: scene params (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(
            device,
            &bind_group_layout,
            &instance_buffer,
            &params_buffer,
        );

        let mut instances = Vec::with_capacity(capacity as usize);
        instances.resize(capacity as usize, None);

        Self {
            instances,
            free_list: Vec::new(),
            dirty_indices: Vec::new(),
            instance_buffer,
            params_buffer,
            bind_group_layout,
            bind_group,
            live_count: 0,
            capacity,
            frame_index: 0,
        }
    }

    // =======================================================================
    // Instance management
    // =======================================================================

    /// Add a new instance to the GPU Scene. Returns an opaque handle.
    pub fn add_instance(&mut self, desc: &InstanceDesc) -> InstanceId {
        let slot = if let Some(recycled) = self.free_list.pop() {
            recycled
        } else {
            let slot = self.instances.len() as u32;
            self.instances.push(None);
            slot
        };

        let gpu = GpuInstance {
            world_matrix: desc.world_transform.to_cols_array_2d(),
            prev_world_matrix: desc.world_transform.to_cols_array_2d(),
            bounds_center: desc.bounds_center.to_array(),
            bounds_radius: desc.bounds_radius,
            mesh_id: desc.mesh_id,
            material_id: desc.material_id,
            flags: desc.flags,
            custom_data: desc.custom_data,
            vertex_offset: desc.vertex_offset,
            index_offset: desc.index_offset,
            index_count: desc.index_count,
            lod_level: desc.lod_level,
            _pad: [0.0; 4],
        };

        self.instances[slot as usize] = Some(gpu);
        self.dirty_indices.push(slot);
        self.live_count += 1;

        InstanceId(slot)
    }

    /// Remove an instance. The slot is recycled for future adds.
    pub fn remove_instance(&mut self, id: InstanceId) {
        let slot = id.0 as usize;
        if slot < self.instances.len() && self.instances[slot].is_some() {
            // Zero-out the slot so GPU reads get zero flags (invisible)
            self.instances[slot] = Some(GpuInstance::default());
            // Mark flag as not visible
            if let Some(ref mut inst) = self.instances[slot] {
                inst.flags = 0;
            }
            self.dirty_indices.push(id.0);
            self.free_list.push(id.0);
            self.live_count -= 1;
        }
    }

    /// Update the world transform of an existing instance.
    /// The previous transform is preserved automatically for motion vectors.
    pub fn update_transform(&mut self, id: InstanceId, new_transform: Mat4) {
        let slot = id.0 as usize;
        if let Some(ref mut inst) = self.instances.get_mut(slot).and_then(|o| o.as_mut()) {
            // Save current as previous
            inst.prev_world_matrix = inst.world_matrix;
            inst.world_matrix = new_transform.to_cols_array_2d();

            // Update bounds center from transform translation
            let translation = new_transform.col(3);
            inst.bounds_center = [translation.x, translation.y, translation.z];

            self.dirty_indices.push(id.0);
        }
    }

    /// Update material for an existing instance.
    pub fn update_material(&mut self, id: InstanceId, material_id: u32) {
        let slot = id.0 as usize;
        if let Some(ref mut inst) = self.instances.get_mut(slot).and_then(|o| o.as_mut()) {
            inst.material_id = material_id;
            self.dirty_indices.push(id.0);
        }
    }

    /// Update flags for an existing instance.
    pub fn update_flags(&mut self, id: InstanceId, flags: u32) {
        let slot = id.0 as usize;
        if let Some(ref mut inst) = self.instances.get_mut(slot).and_then(|o| o.as_mut()) {
            inst.flags = flags;
            self.dirty_indices.push(id.0);
        }
    }

    /// Bulk update: set the entire instance data (useful for full scene reload).
    pub fn update_instance(&mut self, id: InstanceId, desc: &InstanceDesc) {
        let slot = id.0 as usize;
        if let Some(ref mut inst) = self.instances.get_mut(slot).and_then(|o| o.as_mut()) {
            inst.prev_world_matrix = inst.world_matrix;
            inst.world_matrix = desc.world_transform.to_cols_array_2d();
            inst.bounds_center = desc.bounds_center.to_array();
            inst.bounds_radius = desc.bounds_radius;
            inst.mesh_id = desc.mesh_id;
            inst.material_id = desc.material_id;
            inst.flags = desc.flags;
            inst.custom_data = desc.custom_data;
            inst.vertex_offset = desc.vertex_offset;
            inst.index_offset = desc.index_offset;
            inst.index_count = desc.index_count;
            inst.lod_level = desc.lod_level;
            self.dirty_indices.push(id.0);
        }
    }

    // =======================================================================
    // Per-frame GPU upload
    // =======================================================================

    /// Flush dirty instance data to the GPU buffer.
    ///
    /// Call this once per frame after all add/update/remove operations.
    /// Only modified instances are uploaded (delta update).
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.frame_index += 1;

        // Grow the GPU buffer if needed
        let required_capacity = self.instances.len() as u32;
        if required_capacity > self.capacity {
            self.grow(device, required_capacity);
        }

        // Upload only dirty instances
        if !self.dirty_indices.is_empty() {
            // Deduplicate dirty indices
            self.dirty_indices.sort_unstable();
            self.dirty_indices.dedup();

            let instance_size = std::mem::size_of::<GpuInstance>() as u64;

            // Zeroed instance for cleared slots (flags=0 → invisible to shaders)
            let zeroed = GpuInstance::zeroed();

            for &idx in &self.dirty_indices {
                let data = match self.instances[idx as usize] {
                    Some(ref inst) => inst,
                    None => &zeroed,
                };
                let offset = idx as u64 * instance_size;
                queue.write_buffer(
                    &self.instance_buffer,
                    offset,
                    bytemuck::bytes_of(data),
                );
            }

            // Log first upload
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| {
                log::info!(
                    "[GPU Scene] First upload: {} dirty instances, {} live, {} capacity",
                    self.dirty_indices.len(),
                    self.live_count,
                    self.capacity,
                );
            });

            self.dirty_indices.clear();
        }

        // Upload scene params
        let params = GpuSceneParams {
            instance_count: self.live_count,
            frame_index: self.frame_index as u32,
            _pad: [0; 2],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Begin a new frame: preserve previous transforms for motion vectors.
    ///
    /// Call this at the very start of the frame, before any update_transform calls.
    /// This copies current → prev for ALL movable instances.
    pub fn begin_frame(&mut self) {
        for inst_opt in self.instances.iter_mut() {
            if let Some(ref mut inst) = inst_opt {
                if inst.flags & instance_flags::MOVABLE != 0 {
                    inst.prev_world_matrix = inst.world_matrix;
                }
            }
        }
    }

    /// Clear all instances for per-frame rebuild.
    ///
    /// This is the simplest strategy: every frame the app layer clears and
    /// re-adds all instances.  All slots are recycled but the GPU buffer
    /// is kept (no reallocation).  All slots are marked dirty so they
    /// get zeroed on the next `upload()`.
    pub fn clear_all(&mut self) {
        for (i, inst_opt) in self.instances.iter_mut().enumerate() {
            if inst_opt.is_some() {
                *inst_opt = None;
                self.dirty_indices.push(i as u32);
            }
        }
        self.free_list.clear();
        // Push all slots back into free list in reverse order (so low indices are allocated first)
        for i in (0..self.instances.len()).rev() {
            self.free_list.push(i as u32);
        }
        self.live_count = 0;
    }

    // =======================================================================
    // GPU buffer growth
    // =======================================================================

    fn grow(&mut self, device: &wgpu::Device, min_capacity: u32) {
        let new_capacity = (min_capacity * 2).max(1024).min(MAX_GPU_INSTANCES);
        if new_capacity <= self.capacity {
            return;
        }

        log::info!(
            "[GPU Scene] Growing buffer: {} -> {} instances",
            self.capacity,
            new_capacity,
        );

        let instance_size = std::mem::size_of::<GpuInstance>() as u64;

        self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Scene Instance Buffer"),
            size: new_capacity as u64 * instance_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        self.capacity = new_capacity;
        self.instances.resize(new_capacity as usize, None);

        // Rebuild bind group with new buffer
        self.bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &self.instance_buffer,
            &self.params_buffer,
        );

        // Mark ALL existing instances as dirty so they get re-uploaded
        self.dirty_indices.clear();
        for (i, inst) in self.instances.iter().enumerate() {
            if inst.is_some() {
                self.dirty_indices.push(i as u32);
            }
        }
    }

    // =======================================================================
    // Bind group
    // =======================================================================

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        instance_buffer: &wgpu::Buffer,
        params_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GPU Scene Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    // =======================================================================
    // Accessors
    // =======================================================================

    /// The storage buffer containing all instance data.
    pub fn instance_buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }

    /// The uniform buffer with scene-level parameters.
    pub fn params_buffer(&self) -> &wgpu::Buffer {
        &self.params_buffer
    }

    /// The bind group for passes that read the GPU Scene.
    /// Layout: binding 0 = instances (storage), binding 1 = params (uniform)
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// The bind group layout (for pipeline creation).
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Number of live (non-removed) instances.
    pub fn live_count(&self) -> u32 {
        self.live_count
    }

    /// Current GPU buffer capacity (in instances).
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Approximate VRAM usage in bytes.
    pub fn vram_usage(&self) -> u64 {
        let instance_size = std::mem::size_of::<GpuInstance>() as u64;
        self.capacity as u64 * instance_size + std::mem::size_of::<GpuSceneParams>() as u64
    }

    /// Get a reference to an instance (CPU-side copy).
    pub fn get_instance(&self, id: InstanceId) -> Option<&GpuInstance> {
        self.instances.get(id.0 as usize).and_then(|o| o.as_ref())
    }

    /// Iterate over all live instances with their IDs.
    pub fn iter_live(&self) -> impl Iterator<Item = (InstanceId, &GpuInstance)> {
        self.instances
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| {
                opt.as_ref()
                    .filter(|inst| inst.flags != 0)
                    .map(|inst| (InstanceId(i as u32), inst))
            })
    }

    /// Get all live instance IDs.
    pub fn live_ids(&self) -> Vec<InstanceId> {
        self.instances
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| {
                opt.as_ref()
                    .filter(|inst| inst.flags != 0)
                    .map(|_| InstanceId(i as u32))
            })
            .collect()
    }
}
