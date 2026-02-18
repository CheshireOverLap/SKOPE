// SKOPE Engine - Visible Light Hash
//
// Spatial hash for O(1) per-tile visible light lookup.
// Replaces brute-force tile-light intersection with a hash map
// that maps spatial cells to light lists.
//
// Inspired by UE5's MegaLights visible light data structure.
//
// Algorithm:
// 1. Build a spatial hash of all lights based on their bounding volume
// 2. For each screen tile, compute which spatial cells overlap
// 3. Look up lights from those cells via the hash
// 4. Compact into a per-tile light list

use bytemuck::{Pod, Zeroable};

/// Maximum lights per hash cell.
pub const MAX_LIGHTS_PER_CELL: u32 = 32;

/// Hash grid resolution (cells per axis).
pub const HASH_GRID_SIZE: u32 = 64;

/// Total hash table entries.
pub const HASH_TABLE_SIZE: u32 = HASH_GRID_SIZE * HASH_GRID_SIZE * HASH_GRID_SIZE;

/// A single entry in the visible light hash.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LightHashEntry {
    /// Number of lights in this cell.
    pub count: u32,
    /// Offset into the compacted light index buffer.
    pub offset: u32,
}

/// Parameters for the hash grid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LightHashParams {
    /// World-space origin of the hash grid.
    pub grid_origin: [f32; 3],
    /// Cell size in world units.
    pub cell_size: f32,
    /// Grid dimensions (same on all axes).
    pub grid_size: u32,
    /// Total number of active lights.
    pub light_count: u32,
    pub _pad: [u32; 2],
}

/// CPU-side visible light hash builder.
///
/// Builds a spatial hash of light bounding volumes on the CPU,
/// then uploads the hash table and light index buffer to the GPU.
pub struct VisibleLightHash {
    /// Hash table: one entry per cell.
    hash_table: Vec<LightHashEntry>,
    /// Compacted light indices (variable length per cell).
    light_indices: Vec<u32>,
    /// GPU buffers
    pub hash_table_buffer: wgpu::Buffer,
    pub light_index_buffer: wgpu::Buffer,
    pub params_buffer: wgpu::Buffer,
    /// Current parameters.
    params: LightHashParams,
}

impl VisibleLightHash {
    pub fn new(device: &wgpu::Device) -> Self {
        let table_size = HASH_TABLE_SIZE as usize;
        let hash_table = vec![LightHashEntry { count: 0, offset: 0 }; table_size];
        let light_indices = Vec::new();

        let hash_table_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VLH Hash Table"),
            size: (table_size * std::mem::size_of::<LightHashEntry>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Pre-allocate for max scenario
        let max_light_refs = (table_size * MAX_LIGHTS_PER_CELL as usize).min(1_000_000);
        let light_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VLH Light Indices"),
            size: (max_light_refs * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VLH Params"),
            size: std::mem::size_of::<LightHashParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            hash_table,
            light_indices,
            hash_table_buffer,
            light_index_buffer,
            params_buffer,
            params: LightHashParams {
                grid_origin: [0.0; 3],
                cell_size: 2.0,
                grid_size: HASH_GRID_SIZE,
                light_count: 0,
                _pad: [0; 2],
            },
        }
    }

    /// Build the spatial hash from light bounding spheres.
    ///
    /// `lights`: array of (position[3], radius) tuples.
    pub fn build(
        &mut self,
        queue: &wgpu::Queue,
        lights: &[([f32; 3], f32)],
        camera_pos: [f32; 3],
    ) {
        let grid_size = HASH_GRID_SIZE;
        let half_extent = grid_size as f32 * self.params.cell_size * 0.5;

        // Center grid on camera
        self.params.grid_origin = [
            camera_pos[0] - half_extent,
            camera_pos[1] - half_extent,
            camera_pos[2] - half_extent,
        ];
        self.params.light_count = lights.len() as u32;

        // Reset hash table
        for entry in &mut self.hash_table {
            entry.count = 0;
            entry.offset = 0;
        }
        self.light_indices.clear();

        // First pass: count lights per cell
        let mut cell_lights: Vec<Vec<u32>> = vec![Vec::new(); (grid_size * grid_size * grid_size) as usize];

        for (light_idx, (pos, radius)) in lights.iter().enumerate() {
            let min_cell = self.world_to_cell([
                pos[0] - radius,
                pos[1] - radius,
                pos[2] - radius,
            ]);
            let max_cell = self.world_to_cell([
                pos[0] + radius,
                pos[1] + radius,
                pos[2] + radius,
            ]);

            for z in min_cell[2]..=max_cell[2] {
                for y in min_cell[1]..=max_cell[1] {
                    for x in min_cell[0]..=max_cell[0] {
                        let idx = (z * grid_size * grid_size + y * grid_size + x) as usize;
                        if idx < cell_lights.len() && cell_lights[idx].len() < MAX_LIGHTS_PER_CELL as usize {
                            cell_lights[idx].push(light_idx as u32);
                        }
                    }
                }
            }
        }

        // Second pass: compact into buffer
        let mut offset = 0u32;
        for (cell_idx, lights_in_cell) in cell_lights.iter().enumerate() {
            self.hash_table[cell_idx].count = lights_in_cell.len() as u32;
            self.hash_table[cell_idx].offset = offset;
            for &light_idx in lights_in_cell {
                self.light_indices.push(light_idx);
            }
            offset += lights_in_cell.len() as u32;
        }

        // Upload to GPU
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.params));
        queue.write_buffer(
            &self.hash_table_buffer,
            0,
            bytemuck::cast_slice(&self.hash_table),
        );
        if !self.light_indices.is_empty() {
            let upload_size = (self.light_indices.len() * 4).min(self.light_index_buffer.size() as usize);
            let upload_count = upload_size / 4;
            queue.write_buffer(
                &self.light_index_buffer,
                0,
                bytemuck::cast_slice(&self.light_indices[..upload_count]),
            );
        }
    }

    fn world_to_cell(&self, pos: [f32; 3]) -> [u32; 3] {
        let inv_cell = 1.0 / self.params.cell_size;
        let grid = self.params.grid_size;
        [
            ((pos[0] - self.params.grid_origin[0]) * inv_cell).clamp(0.0, (grid - 1) as f32) as u32,
            ((pos[1] - self.params.grid_origin[1]) * inv_cell).clamp(0.0, (grid - 1) as f32) as u32,
            ((pos[2] - self.params.grid_origin[2]) * inv_cell).clamp(0.0, (grid - 1) as f32) as u32,
        ]
    }

    pub fn hash_table_buffer(&self) -> &wgpu::Buffer { &self.hash_table_buffer }
    pub fn light_index_buffer(&self) -> &wgpu::Buffer { &self.light_index_buffer }
    pub fn params_buffer(&self) -> &wgpu::Buffer { &self.params_buffer }
    pub fn light_count(&self) -> u32 { self.params.light_count }
}
