// SKOPE Particle System
// CPU simulation + GPU instanced billboard rendering
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use glam::{Vec3, Vec4};

// ============ Particle Data ============

/// Individual particle state
#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub color: Vec4,
    pub size: f32,
    pub rotation: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub alive: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            color: Vec4::ONE,
            size: 0.1,
            rotation: 0.0,
            lifetime: 0.0,
            max_lifetime: 1.0,
            alive: false,
        }
    }
}

impl Particle {
    /// Get normalized lifetime (0.0 = just born, 1.0 = about to die)
    pub fn normalized_age(&self) -> f32 {
        if self.max_lifetime > 0.0 {
            self.lifetime / self.max_lifetime
        } else {
            1.0
        }
    }

    /// Update particle state
    pub fn update(&mut self, dt: f32) {
        if !self.alive {
            return;
        }

        self.lifetime += dt;
        if self.lifetime >= self.max_lifetime {
            self.alive = false;
            return;
        }

        self.velocity += self.acceleration * dt;
        self.position += self.velocity * dt;
    }
}

// ============ Emitter Configuration ============

/// Particle emitter shape
#[derive(Clone, Debug)]
pub enum EmitterShape {
    /// Point emission
    Point,
    /// Sphere surface emission
    Sphere { radius: f32 },
    /// Sphere volume emission
    SphereVolume { radius: f32 },
    /// Cone emission
    Cone { angle: f32, radius: f32 },
    /// Box emission
    Box { half_extents: Vec3 },
}

impl Default for EmitterShape {
    fn default() -> Self {
        Self::Point
    }
}

/// Color over lifetime configuration
#[derive(Clone, Debug)]
pub enum ColorOverLifetime {
    Constant(Vec4),
    Gradient { start: Vec4, end: Vec4 },
    Curve(Vec<(f32, Vec4)>), // (time, color) pairs
}

impl Default for ColorOverLifetime {
    fn default() -> Self {
        Self::Constant(Vec4::ONE)
    }
}

/// Size over lifetime configuration
#[derive(Clone, Debug)]
pub enum SizeOverLifetime {
    Constant(f32),
    Linear { start: f32, end: f32 },
    Curve(Vec<(f32, f32)>),
}

impl Default for SizeOverLifetime {
    fn default() -> Self {
        Self::Constant(0.1)
    }
}

/// Particle emitter configuration
#[derive(Clone, Debug)]
pub struct EmitterConfig {
    /// Maximum particles this emitter can have
    pub max_particles: usize,
    /// Particles spawned per second
    pub emission_rate: f32,
    /// Initial velocity range
    pub velocity_min: Vec3,
    pub velocity_max: Vec3,
    /// Gravity/acceleration
    pub gravity: Vec3,
    /// Lifetime range (seconds)
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    /// Initial size range
    pub size_min: f32,
    pub size_max: f32,
    /// Emission shape
    pub shape: EmitterShape,
    /// Color over lifetime
    pub color: ColorOverLifetime,
    /// Size over lifetime
    pub size_over_lifetime: SizeOverLifetime,
    /// Spawn in world space (true) or local space (false)
    pub world_space: bool,
    /// Burst count (0 = continuous emission)
    pub burst_count: u32,
    /// Loop emission
    pub looping: bool,
    /// Emitter duration (0 = infinite)
    pub duration: f32,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            max_particles: 100,
            emission_rate: 10.0,
            velocity_min: Vec3::new(-0.5, 1.0, -0.5),
            velocity_max: Vec3::new(0.5, 3.0, 0.5),
            gravity: Vec3::new(0.0, -9.8, 0.0),
            lifetime_min: 1.0,
            lifetime_max: 2.0,
            size_min: 0.05,
            size_max: 0.15,
            shape: EmitterShape::Point,
            color: ColorOverLifetime::Gradient {
                start: Vec4::new(1.0, 0.8, 0.2, 1.0),
                end: Vec4::new(1.0, 0.2, 0.0, 0.0),
            },
            size_over_lifetime: SizeOverLifetime::Linear { start: 1.0, end: 0.0 },
            world_space: true,
            burst_count: 0,
            looping: true,
            duration: 0.0,
        }
    }
}

impl EmitterConfig {
    /// Create fire-like particles
    pub fn fire() -> Self {
        Self {
            max_particles: 200,
            emission_rate: 50.0,
            velocity_min: Vec3::new(-0.3, 2.0, -0.3),
            velocity_max: Vec3::new(0.3, 4.0, 0.3),
            gravity: Vec3::new(0.0, 1.0, 0.0), // Fire rises
            lifetime_min: 0.5,
            lifetime_max: 1.5,
            size_min: 0.1,
            size_max: 0.3,
            shape: EmitterShape::Sphere { radius: 0.2 },
            color: ColorOverLifetime::Gradient {
                start: Vec4::new(1.0, 0.9, 0.3, 1.0),
                end: Vec4::new(1.0, 0.2, 0.0, 0.0),
            },
            size_over_lifetime: SizeOverLifetime::Linear { start: 1.0, end: 0.2 },
            ..Default::default()
        }
    }

    /// Create smoke particles
    pub fn smoke() -> Self {
        Self {
            max_particles: 100,
            emission_rate: 15.0,
            velocity_min: Vec3::new(-0.5, 1.0, -0.5),
            velocity_max: Vec3::new(0.5, 2.5, 0.5),
            gravity: Vec3::new(0.0, 0.5, 0.0),
            lifetime_min: 2.0,
            lifetime_max: 4.0,
            size_min: 0.2,
            size_max: 0.5,
            shape: EmitterShape::Sphere { radius: 0.1 },
            color: ColorOverLifetime::Gradient {
                start: Vec4::new(0.5, 0.5, 0.5, 0.8),
                end: Vec4::new(0.3, 0.3, 0.3, 0.0),
            },
            size_over_lifetime: SizeOverLifetime::Linear { start: 0.5, end: 2.0 },
            ..Default::default()
        }
    }

    /// Create explosion burst
    pub fn explosion() -> Self {
        Self {
            max_particles: 50,
            emission_rate: 0.0,
            velocity_min: Vec3::new(-5.0, -5.0, -5.0),
            velocity_max: Vec3::new(5.0, 5.0, 5.0),
            gravity: Vec3::new(0.0, -5.0, 0.0),
            lifetime_min: 0.3,
            lifetime_max: 0.8,
            size_min: 0.1,
            size_max: 0.3,
            shape: EmitterShape::Point,
            color: ColorOverLifetime::Gradient {
                start: Vec4::new(1.0, 0.8, 0.2, 1.0),
                end: Vec4::new(1.0, 0.3, 0.0, 0.0),
            },
            size_over_lifetime: SizeOverLifetime::Linear { start: 1.0, end: 0.0 },
            burst_count: 50,
            looping: false,
            ..Default::default()
        }
    }

    /// Create sparkle particles
    pub fn sparkle() -> Self {
        Self {
            max_particles: 30,
            emission_rate: 5.0,
            velocity_min: Vec3::new(-0.2, 0.5, -0.2),
            velocity_max: Vec3::new(0.2, 1.5, 0.2),
            gravity: Vec3::new(0.0, -1.0, 0.0),
            lifetime_min: 0.5,
            lifetime_max: 1.0,
            size_min: 0.02,
            size_max: 0.05,
            shape: EmitterShape::Sphere { radius: 0.3 },
            color: ColorOverLifetime::Gradient {
                start: Vec4::new(1.0, 1.0, 0.8, 1.0),
                end: Vec4::new(1.0, 1.0, 1.0, 0.0),
            },
            size_over_lifetime: SizeOverLifetime::Constant(1.0),
            ..Default::default()
        }
    }
}

// ============ Emitter Component ============

/// Particle emitter component - attach to entities
#[derive(Component, Clone)]
pub struct ParticleEmitter {
    pub config: EmitterConfig,
    pub enabled: bool,
    pub particles: Vec<Particle>,
    emission_accumulator: f32,
    elapsed_time: f32,
    has_burst: bool,
}

impl ParticleEmitter {
    pub fn new(config: EmitterConfig) -> Self {
        let max = config.max_particles;
        Self {
            config,
            enabled: true,
            particles: Vec::with_capacity(max),
            emission_accumulator: 0.0,
            elapsed_time: 0.0,
            has_burst: false,
        }
    }

    pub fn fire() -> Self {
        Self::new(EmitterConfig::fire())
    }

    pub fn smoke() -> Self {
        Self::new(EmitterConfig::smoke())
    }

    pub fn explosion() -> Self {
        Self::new(EmitterConfig::explosion())
    }

    pub fn sparkle() -> Self {
        Self::new(EmitterConfig::sparkle())
    }

    /// Update particles and spawn new ones
    pub fn update(&mut self, dt: f32, emitter_position: Vec3) {
        if !self.enabled {
            return;
        }

        self.elapsed_time += dt;

        // Check duration
        if self.config.duration > 0.0 && self.elapsed_time >= self.config.duration {
            if self.config.looping {
                self.elapsed_time = 0.0;
                self.has_burst = false;
            } else {
                self.enabled = false;
            }
        }

        // Update existing particles
        let gravity = self.config.gravity;
        let color_config = &self.config.color;
        let size_config = &self.config.size_over_lifetime;

        for particle in &mut self.particles {
            if particle.alive {
                particle.acceleration = gravity;
                particle.update(dt);

                // Apply color over lifetime
                let t = particle.normalized_age();
                particle.color = sample_color(color_config, t);
                particle.size *= sample_size(size_config, t);
            }
        }

        // Remove dead particles
        self.particles.retain(|p| p.alive);

        // Spawn new particles
        if self.config.burst_count > 0 {
            // Burst mode
            if !self.has_burst {
                for _ in 0..self.config.burst_count {
                    if self.particles.len() < self.config.max_particles {
                        self.spawn_particle(emitter_position);
                    }
                }
                self.has_burst = true;
            }
        } else {
            // Continuous emission
            self.emission_accumulator += self.config.emission_rate * dt;
            while self.emission_accumulator >= 1.0 && self.particles.len() < self.config.max_particles {
                self.spawn_particle(emitter_position);
                self.emission_accumulator -= 1.0;
            }
        }
    }

    fn spawn_particle(&mut self, emitter_position: Vec3) {
        let mut particle = Particle::default();
        particle.alive = true;

        // Position based on shape
        let local_pos = self.sample_shape_position();
        particle.position = if self.config.world_space {
            emitter_position + local_pos
        } else {
            local_pos
        };

        // Random velocity within range
        particle.velocity = Vec3::new(
            rand_range(self.config.velocity_min.x, self.config.velocity_max.x),
            rand_range(self.config.velocity_min.y, self.config.velocity_max.y),
            rand_range(self.config.velocity_min.z, self.config.velocity_max.z),
        );

        // Random lifetime
        particle.max_lifetime = rand_range(self.config.lifetime_min, self.config.lifetime_max);

        // Initial size
        let base_size = rand_range(self.config.size_min, self.config.size_max);
        particle.size = base_size * sample_size(&self.config.size_over_lifetime, 0.0);

        // Initial color
        particle.color = sample_color(&self.config.color, 0.0);

        self.particles.push(particle);
    }

    fn sample_shape_position(&self) -> Vec3 {
        match &self.config.shape {
            EmitterShape::Point => Vec3::ZERO,
            EmitterShape::Sphere { radius } => {
                random_on_sphere() * *radius
            }
            EmitterShape::SphereVolume { radius } => {
                random_in_sphere() * *radius
            }
            EmitterShape::Cone { angle, radius } => {
                let theta = rand_range(0.0, std::f32::consts::TAU);
                let r = rand_range(0.0, *radius);
                let h = rand_range(0.0, 1.0);
                let spread = h * angle.tan();
                Vec3::new(
                    theta.cos() * r * spread,
                    h,
                    theta.sin() * r * spread,
                )
            }
            EmitterShape::Box { half_extents } => {
                Vec3::new(
                    rand_range(-half_extents.x, half_extents.x),
                    rand_range(-half_extents.y, half_extents.y),
                    rand_range(-half_extents.z, half_extents.z),
                )
            }
        }
    }

    /// Get alive particle count
    pub fn alive_count(&self) -> usize {
        self.particles.iter().filter(|p| p.alive).count()
    }

    /// Reset emitter
    pub fn reset(&mut self) {
        self.particles.clear();
        self.emission_accumulator = 0.0;
        self.elapsed_time = 0.0;
        self.has_burst = false;
        self.enabled = true;
    }

    /// Trigger burst (for non-looping emitters)
    pub fn burst(&mut self) {
        self.has_burst = false;
        self.elapsed_time = 0.0;
        self.enabled = true;
    }
}

// ============ GPU Data ============

/// Particle instance data for GPU rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
    pub rotation: f32,
    pub _padding: [f32; 3],
}

impl ParticleInstance {
    pub fn from_particle(p: &Particle) -> Self {
        Self {
            position: [p.position.x, p.position.y, p.position.z],
            size: p.size,
            color: [p.color.x, p.color.y, p.color.z, p.color.w],
            rotation: p.rotation,
            _padding: [0.0; 3],
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // size
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // rotation
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

// ============ Helper Functions ============

/// Simple LCG random number generator (no external crate needed)
fn rand_u32() -> u32 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u32> = Cell::new(12345);
    }
    SEED.with(|s| {
        let val = s.get().wrapping_mul(1103515245).wrapping_add(12345);
        s.set(val);
        val
    })
}

fn rand_f32() -> f32 {
    (rand_u32() as f32) / (u32::MAX as f32)
}

fn rand_range(min: f32, max: f32) -> f32 {
    min + rand_f32() * (max - min)
}

fn random_on_sphere() -> Vec3 {
    let theta = rand_range(0.0, std::f32::consts::TAU);
    let phi = rand_range(0.0, std::f32::consts::PI);
    Vec3::new(
        phi.sin() * theta.cos(),
        phi.cos(),
        phi.sin() * theta.sin(),
    )
}

fn random_in_sphere() -> Vec3 {
    random_on_sphere() * rand_f32().cbrt()
}

/// Sample color from ColorOverLifetime config
fn sample_color(config: &ColorOverLifetime, t: f32) -> Vec4 {
    match config {
        ColorOverLifetime::Constant(c) => *c,
        ColorOverLifetime::Gradient { start, end } => start.lerp(*end, t),
        ColorOverLifetime::Curve(points) => {
            if points.is_empty() {
                return Vec4::ONE;
            }
            if t <= points[0].0 {
                return points[0].1;
            }
            if t >= points.last().unwrap().0 {
                return points.last().unwrap().1;
            }
            for i in 0..points.len() - 1 {
                if t >= points[i].0 && t < points[i + 1].0 {
                    let local_t = (t - points[i].0) / (points[i + 1].0 - points[i].0);
                    return points[i].1.lerp(points[i + 1].1, local_t);
                }
            }
            Vec4::ONE
        }
    }
}

/// Sample size from SizeOverLifetime config
fn sample_size(config: &SizeOverLifetime, t: f32) -> f32 {
    match config {
        SizeOverLifetime::Constant(s) => *s,
        SizeOverLifetime::Linear { start, end } => start + (end - start) * t,
        SizeOverLifetime::Curve(points) => {
            if points.is_empty() {
                return 1.0;
            }
            if t <= points[0].0 {
                return points[0].1;
            }
            if t >= points.last().unwrap().0 {
                return points.last().unwrap().1;
            }
            for i in 0..points.len() - 1 {
                if t >= points[i].0 && t < points[i + 1].0 {
                    let local_t = (t - points[i].0) / (points[i + 1].0 - points[i].0);
                    return points[i].1 + (points[i + 1].1 - points[i].1) * local_t;
                }
            }
            1.0
        }
    }
}

// ============ Particle Renderer ============

/// GPU particle renderer using instanced billboard quads
pub struct ParticleRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    max_particles: usize,
}

/// Billboard quad vertex
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex {
    corner: [f32; 2],
}

impl QuadVertex {
    const VERTICES: [QuadVertex; 6] = [
        QuadVertex { corner: [-1.0, -1.0] },
        QuadVertex { corner: [ 1.0, -1.0] },
        QuadVertex { corner: [ 1.0,  1.0] },
        QuadVertex { corner: [-1.0, -1.0] },
        QuadVertex { corner: [ 1.0,  1.0] },
        QuadVertex { corner: [-1.0,  1.0] },
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

impl ParticleRenderer {
    const MAX_PARTICLES: usize = 10000;

    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/particle.wgsl").into()),
        });

        // Create own bind group layout reference
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Camera BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline with alpha blending
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::desc(), ParticleInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None, // Billboards face camera
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Particles don't write depth
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create vertex buffer for quad
        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Vertex Buffer"),
            contents: bytemuck::cast_slice(&QuadVertex::VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create instance buffer
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Instance Buffer"),
            size: (std::mem::size_of::<ParticleInstance>() * Self::MAX_PARTICLES) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            instance_buffer,
            camera_bind_group_layout: camera_bgl,
            max_particles: Self::MAX_PARTICLES,
        }
    }

    /// Render all particle emitters
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        camera_bind_group: &'a wgpu::BindGroup,
        emitters: &[&ParticleEmitter],
    ) {
        // Collect all alive particles
        let mut instances: Vec<ParticleInstance> = Vec::with_capacity(self.max_particles);
        for emitter in emitters {
            for particle in &emitter.particles {
                if particle.alive && instances.len() < self.max_particles {
                    instances.push(ParticleInstance::from_particle(particle));
                }
            }
        }

        if instances.is_empty() {
            return;
        }

        // Update instance buffer
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        // Render
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..instances.len() as u32);
    }
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_update() {
        let mut p = Particle {
            position: Vec3::ZERO,
            velocity: Vec3::new(1.0, 0.0, 0.0),
            acceleration: Vec3::ZERO,
            max_lifetime: 1.0,
            alive: true,
            ..Default::default()
        };

        p.update(0.5);
        assert_eq!(p.position.x, 0.5);
        assert!(p.alive);

        p.update(0.6);
        assert!(!p.alive); // Should die after 1.0s
    }

    #[test]
    fn test_emitter_spawn() {
        let mut emitter = ParticleEmitter::new(EmitterConfig {
            emission_rate: 100.0,
            max_particles: 10,
            ..Default::default()
        });

        emitter.update(0.1, Vec3::ZERO);
        assert!(emitter.alive_count() > 0);
        assert!(emitter.alive_count() <= 10);
    }

    #[test]
    fn test_emitter_burst() {
        let mut emitter = ParticleEmitter::explosion();
        emitter.update(0.016, Vec3::ZERO);

        // Should spawn burst_count particles
        assert_eq!(emitter.alive_count(), 50);
    }

    #[test]
    fn test_preset_configs() {
        let fire = EmitterConfig::fire();
        assert!(fire.emission_rate > 0.0);

        let smoke = EmitterConfig::smoke();
        assert!(smoke.lifetime_max > smoke.lifetime_min);

        let explosion = EmitterConfig::explosion();
        assert_eq!(explosion.burst_count, 50);
    }

    #[test]
    fn test_color_sampling() {
        let emitter = ParticleEmitter::new(EmitterConfig {
            color: ColorOverLifetime::Gradient {
                start: Vec4::new(1.0, 0.0, 0.0, 1.0),
                end: Vec4::new(0.0, 0.0, 1.0, 0.0),
            },
            ..Default::default()
        });

        let c0 = sample_color(&emitter.config.color, 0.0);
        assert_eq!(c0.x, 1.0);

        let c1 = sample_color(&emitter.config.color, 1.0);
        assert_eq!(c1.z, 1.0);

        let c_mid = sample_color(&emitter.config.color, 0.5);
        assert!((c_mid.x - 0.5).abs() < 0.01);
    }
}
