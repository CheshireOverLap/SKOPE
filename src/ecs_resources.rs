// ECS Resources for SKOPE Engine
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

// ============ GPU Resources ============

use std::sync::Arc;

/// GPU context (Device, Queue)
#[derive(Resource, Clone)]
pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

/// Surface context (Surface, Config, Depth texture)
#[derive(Resource)]
pub struct SurfaceContext {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::TextureView,
}

/// Render pipeline and bind group layouts
#[derive(Resource)]
pub struct RenderPipelineRes {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
}

// ============ Asset Resources ============

/// Mesh GPU data
pub struct MeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

/// Mesh assets (all loaded meshes)
#[derive(Resource, Default)]
pub struct MeshAssets {
    pub meshes: Vec<MeshGpuData>,
}

/// Material GPU data
pub struct MaterialGpuData {
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
}

/// Material assets (all loaded materials)
#[derive(Resource, Default)]
pub struct MaterialAssets {
    pub materials: Vec<MaterialGpuData>,
}

/// Uniform buffer for MVP matrices
#[derive(Resource)]
pub struct UniformBuffer {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

// ============ Input Resources ============

/// Keyboard input state
#[derive(Resource, Default)]
pub struct KeyboardInput {
    pub keys_pressed: HashSet<KeyCode>,
}

/// Mouse input state
#[derive(Resource, Default)]
pub struct MouseInput {
    pub is_pressed: bool,
    pub last_pos: Option<(f64, f64)>,
}

// ============ Time Resource ============

/// Delta time and elapsed time
#[derive(Resource)]
pub struct Time {
    pub delta_seconds: f32,
    pub elapsed_seconds: f64,
    last_update: std::time::Instant,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            delta_seconds: 0.016,  // 60 FPS 초기값
            elapsed_seconds: 0.0,
            last_update: std::time::Instant::now(),
        }
    }
}

impl Time {
    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.delta_seconds = (now - self.last_update).as_secs_f32();
        self.elapsed_seconds += self.delta_seconds as f64;
        self.last_update = now;
    }
}

// ============ Window Resource ============

/// Window size
#[derive(Resource)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}
