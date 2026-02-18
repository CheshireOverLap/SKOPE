// SKOPE Engine - DDGI Probe Grid
//
// Manages a 3D grid of irradiance probes for one cascade level.
// Uses scrolling grid approach for infinite world support.

use wgpu;
use glam::{Vec3, IVec3};
use bytemuck::{Pod, Zeroable};

/// Cascade level for DDGI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeLevel {
    /// Near cascade: 2m spacing, 64m range (indoor detail)
    Near,
    /// Medium cascade: 8m spacing, 256m range (outdoor)
    Medium,
    /// Far cascade: 32m spacing, 1km range (distant)
    Far,
}

/// Probe Grid Configuration
#[derive(Debug, Clone)]
pub struct ProbeGridConfig {
    /// Grid dimensions (probes per axis)
    pub grid_size: IVec3,

    /// Spacing between probes (meters)
    pub spacing: f32,

    /// Cascade level
    pub level: CascadeLevel,

    /// Base index in global atlas
    pub atlas_offset: u32,
}

impl ProbeGridConfig {
    pub fn new(level: CascadeLevel) -> Self {
        match level {
            CascadeLevel::Near => Self {
                grid_size: IVec3::new(32, 32, 16),  // 32x32x16 = 16384 probes
                spacing: 2.0,
                level,
                atlas_offset: 0,
            },
            CascadeLevel::Medium => Self {
                grid_size: IVec3::new(32, 32, 8),   // 32x32x8 = 8192 probes
                spacing: 8.0,
                level,
                atlas_offset: 16384,
            },
            CascadeLevel::Far => Self {
                grid_size: IVec3::new(32, 32, 4),   // 32x32x4 = 4096 probes
                spacing: 32.0,
                level,
                atlas_offset: 16384 + 8192,
            },
        }
    }

    /// Total range covered by this cascade (half-extent)
    pub fn range(&self) -> f32 {
        self.grid_size.x as f32 * self.spacing * 0.5
    }
}

/// Probe Grid Uniform (for GPU)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ProbeGridUniform {
    /// Grid origin (world space, snapped to probe spacing)
    pub origin: [f32; 3],
    pub spacing: f32,

    /// Grid dimensions
    pub grid_size: [u32; 3],
    pub atlas_offset: u32,

    /// Inverse spacing for fast lookup
    pub inv_spacing: f32,
    pub _pad: [f32; 7],
}

/// Probe Grid
pub struct ProbeGrid {
    /// Configuration
    pub config: ProbeGridConfig,

    /// Current grid origin (snapped to spacing)
    pub origin: Vec3,

    /// Uniform buffer
    pub uniform_buffer: wgpu::Buffer,
}

impl ProbeGrid {
    pub fn new(device: &wgpu::Device, config: ProbeGridConfig) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("DDGI {:?} Uniform", config.level)),
            size: std::mem::size_of::<ProbeGridUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            config,
            origin: Vec3::ZERO,
            uniform_buffer,
        }
    }

    /// Total number of probes in this grid
    pub fn total_probes(&self) -> u32 {
        (self.config.grid_size.x * self.config.grid_size.y * self.config.grid_size.z) as u32
    }

    /// Update grid origin when camera moves
    pub fn update_center(&mut self, center: Vec3) {
        // Snap to probe spacing
        let spacing = self.config.spacing;
        self.origin = Vec3::new(
            (center.x / spacing).floor() * spacing - self.config.range(),
            (center.y / spacing).floor() * spacing - self.config.range(),
            (center.z / spacing).floor() * spacing - self.config.grid_size.z as f32 * spacing * 0.5,
        );
    }

    /// Get world position of a probe
    pub fn probe_position(&self, index: IVec3) -> Vec3 {
        self.origin + Vec3::new(
            index.x as f32 * self.config.spacing,
            index.y as f32 * self.config.spacing,
            index.z as f32 * self.config.spacing,
        )
    }

    /// Get probe index from world position (returns None if outside grid)
    pub fn world_to_probe(&self, world_pos: Vec3) -> Option<IVec3> {
        let local = (world_pos - self.origin) / self.config.spacing;
        let index = IVec3::new(
            local.x.floor() as i32,
            local.y.floor() as i32,
            local.z.floor() as i32,
        );

        if index.x >= 0 && index.x < self.config.grid_size.x &&
           index.y >= 0 && index.y < self.config.grid_size.y &&
           index.z >= 0 && index.z < self.config.grid_size.z {
            Some(index)
        } else {
            None
        }
    }

    /// Linear index in atlas from 3D probe index
    pub fn probe_to_atlas_index(&self, index: IVec3) -> u32 {
        let linear = index.x +
                     index.y * self.config.grid_size.x +
                     index.z * self.config.grid_size.x * self.config.grid_size.y;
        self.config.atlas_offset + linear as u32
    }

    /// Upload uniform to GPU
    pub fn upload_uniform(&self, queue: &wgpu::Queue) {
        let uniform = ProbeGridUniform {
            origin: self.origin.to_array(),
            spacing: self.config.spacing,
            grid_size: [
                self.config.grid_size.x as u32,
                self.config.grid_size.y as u32,
                self.config.grid_size.z as u32,
            ],
            atlas_offset: self.config.atlas_offset,
            inv_spacing: 1.0 / self.config.spacing,
            _pad: [0.0; 7],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }
}
