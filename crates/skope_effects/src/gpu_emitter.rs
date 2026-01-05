//! GPU Particle Emitter
//!
//! GPU compute shader 기반 파티클 이미터 컴포넌트

use glam::Vec3;
use rand::Rng;

use crate::gpu_particle::{GpuEmitterConfig, GpuForceField, GpuForceFieldArray, GpuParticle, MAX_FORCE_FIELDS};
use crate::gpu_particle_pipeline::GpuParticlePipeline;
use crate::{EmitterConfig, EmitterShape, ForceField, ForceFieldSystem};

/// GPU 파티클 이미터
pub struct GpuParticleEmitter {
    /// Compute 파이프라인
    pipeline: GpuParticlePipeline,
    /// CPU 측 이미터 설정
    config: EmitterConfig,
    /// GPU 이미터 설정
    gpu_config: GpuEmitterConfig,
    /// Force field 시스템
    force_fields: ForceFieldSystem,
    /// GPU force field 배열
    gpu_force_fields: GpuForceFieldArray,
    /// 이미터 위치
    position: Vec3,
    /// 총 경과 시간
    total_time: f32,
    /// 스폰 축적기
    #[allow(dead_code)]
    spawn_accumulator: f32,
    /// 활성화 상태
    active: bool,
}

impl GpuParticleEmitter {
    /// 새 GPU 이미터 생성
    pub fn new(device: &wgpu::Device, config: EmitterConfig, position: Vec3) -> Self {
        let particle_count = config.max_particles as u32;
        let pipeline = GpuParticlePipeline::new(device, particle_count);

        let gpu_config = GpuEmitterConfig {
            emitter_position: position.to_array(),
            delta_time: 0.016,
            gravity: config.gravity,
            total_time: 0.0,
            color_start: config.color.start_color(),
            color_end: config.color.end_color(),
            particle_count,
            force_field_count: 0,
            size_start: config.size_over_lifetime.start_size(),
            size_end: config.size_over_lifetime.end_size(),
        };

        Self {
            pipeline,
            config,
            gpu_config,
            force_fields: ForceFieldSystem::new(),
            gpu_force_fields: GpuForceFieldArray::default(),
            position,
            total_time: 0.0,
            spawn_accumulator: 0.0,
            active: true,
        }
    }

    /// 위치 설정
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.gpu_config.emitter_position = position.to_array();
    }

    /// Force field 추가
    pub fn add_force_field(&mut self, force_field: ForceField) {
        self.force_fields.add(force_field.clone());
        self.sync_force_fields_to_gpu();
    }

    /// Force field 시스템 설정
    pub fn set_force_fields(&mut self, force_fields: ForceFieldSystem) {
        self.force_fields = force_fields;
        self.sync_force_fields_to_gpu();
    }

    /// Force field를 GPU 형식으로 변환
    fn sync_force_fields_to_gpu(&mut self) {
        let mut count = 0;
        for (i, ff) in self.force_fields.fields.iter().enumerate() {
            if i >= MAX_FORCE_FIELDS {
                break;
            }
            self.gpu_force_fields.fields[i] = force_field_to_gpu(ff);
            count += 1;
        }
        self.gpu_config.force_field_count = count as u32;
    }

    /// 이미터 활성화/비활성화
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// 활성화 상태 반환
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// 파티클 초기화 (burst 스폰)
    pub fn spawn_burst(&self, queue: &wgpu::Queue, count: u32) {
        let mut particles = vec![GpuParticle::default(); count as usize];
        let mut rng = rand::thread_rng();

        for (i, particle) in particles.iter_mut().enumerate() {
            // 스폰 위치
            let (spawn_pos, spawn_dir) = self.sample_spawn_point(&mut rng);
            particle.position = (self.position + spawn_pos).to_array();

            // 초기 속도 (velocity_min ~ velocity_max 사이 랜덤)
            let velocity = Vec3::new(
                rng.gen_range(self.config.velocity_min[0]..=self.config.velocity_max[0]),
                rng.gen_range(self.config.velocity_min[1]..=self.config.velocity_max[1]),
                rng.gen_range(self.config.velocity_min[2]..=self.config.velocity_max[2]),
            );
            // 방향이 있으면 방향 적용
            let final_velocity = if spawn_dir.length_squared() > 0.001 {
                spawn_dir * velocity.length()
            } else {
                velocity
            };
            particle.velocity = final_velocity.to_array();

            // 수명
            let lifetime = rng.gen_range(self.config.lifetime_min..=self.config.lifetime_max);
            particle.lifetime = 0.0;
            particle.max_lifetime = lifetime;

            // 색상
            particle.color = self.config.color.start_color();

            // 크기
            particle.size = rng.gen_range(self.config.size_min..=self.config.size_max);

            // 회전
            particle.rotation = rng.gen_range(self.config.rotation_min..=self.config.rotation_max);
            particle.rotation_speed = rng.gen_range(self.config.rotation_speed_min..=self.config.rotation_speed_max);

            // 활성화
            particle.alive = 1;

            // 랜덤 시드
            particle.seed = i as u32;
        }

        self.pipeline.init_particles(queue, &particles);
    }

    /// 이미터 형태에서 스폰 위치와 방향 샘플링
    fn sample_spawn_point(&self, rng: &mut impl Rng) -> (Vec3, Vec3) {
        match &self.config.shape {
            EmitterShape::Point => (Vec3::ZERO, Vec3::Y),
            EmitterShape::Sphere { radius } => {
                let dir = random_unit_sphere(rng);
                (dir * *radius, dir)
            }
            EmitterShape::SphereVolume { radius } => {
                let dir = random_unit_sphere(rng);
                let r = rng.gen::<f32>().cbrt() * *radius;
                (dir * r, dir)
            }
            EmitterShape::Cone { angle, radius } => {
                let angle_rad = angle.to_radians();
                let phi = rng.gen_range(0.0..std::f32::consts::TAU);
                let cos_theta = rng.gen_range(angle_rad.cos()..1.0);
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let dir = Vec3::new(sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin());
                let r = rng.gen::<f32>().sqrt() * *radius;
                let pos = Vec3::new(phi.cos() * r, 0.0, phi.sin() * r);
                (pos, dir)
            }
            EmitterShape::Box { half_extents } => {
                let pos = Vec3::new(
                    rng.gen_range(-half_extents[0]..half_extents[0]),
                    rng.gen_range(-half_extents[1]..half_extents[1]),
                    rng.gen_range(-half_extents[2]..half_extents[2]),
                );
                (pos, Vec3::Y)
            }
            EmitterShape::Hemisphere { radius } => {
                let dir = random_unit_hemisphere_y(rng);
                (dir * *radius, dir)
            }
            EmitterShape::Circle { radius } => {
                let theta = rng.gen_range(0.0..std::f32::consts::TAU);
                let r = rng.gen::<f32>().sqrt() * *radius;
                let pos = Vec3::new(theta.cos() * r, 0.0, theta.sin() * r);
                (pos, Vec3::Y)
            }
            EmitterShape::Ring { inner_radius, outer_radius } => {
                let theta = rng.gen_range(0.0..std::f32::consts::TAU);
                let r = rng.gen_range(*inner_radius..*outer_radius);
                let pos = Vec3::new(theta.cos() * r, 0.0, theta.sin() * r);
                (pos, Vec3::Y)
            }
        }
    }

    /// 프레임 업데이트
    pub fn update(&mut self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, delta_time: f32) {
        if !self.active {
            return;
        }

        self.total_time += delta_time;
        self.gpu_config.delta_time = delta_time;
        self.gpu_config.total_time = self.total_time;

        // Config 업데이트
        self.pipeline.update_config(queue, &self.gpu_config);
        self.pipeline.update_force_fields(queue, &self.gpu_force_fields);

        // Compute shader 실행
        self.pipeline.dispatch(encoder);
    }

    /// 파티클 버퍼 반환 (렌더링용)
    pub fn particle_buffer(&self) -> &wgpu::Buffer {
        self.pipeline.particle_buffer()
    }

    /// 파티클 수 반환
    pub fn particle_count(&self) -> u32 {
        self.pipeline.particle_count()
    }

    /// 설정 반환
    pub fn config(&self) -> &EmitterConfig {
        &self.config
    }

    /// GPU 설정 반환
    pub fn gpu_config(&self) -> &GpuEmitterConfig {
        &self.gpu_config
    }
}

// === Helper Functions ===

/// 단위 구 위의 랜덤 점
fn random_unit_sphere(rng: &mut impl Rng) -> Vec3 {
    let phi = rng.gen_range(0.0..std::f32::consts::TAU);
    let cos_theta = rng.gen_range(-1.0f32..1.0);
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    Vec3::new(sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin())
}

/// 위쪽 반구 위의 랜덤 점
fn random_unit_hemisphere_y(rng: &mut impl Rng) -> Vec3 {
    let phi = rng.gen_range(0.0..std::f32::consts::TAU);
    let cos_theta = rng.gen_range(0.0f32..1.0);
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
    Vec3::new(sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin())
}

/// CPU ForceField를 GPU 형식으로 변환
fn force_field_to_gpu(ff: &ForceField) -> GpuForceField {
    use crate::gpu_particle::force_field_type;
    use crate::force_fields::ForceFieldType;

    match &ff.field_type {
        ForceFieldType::Turbulence { strength, frequency, octaves, persistence, scroll_speed } => {
            GpuForceField {
                field_type: force_field_type::TURBULENCE,
                strength: *strength,
                frequency: *frequency,
                octaves: *octaves,
                persistence: *persistence,
                scroll_speed: *scroll_speed,
                ..Default::default()
            }
        }
        ForceFieldType::Vortex { axis, position, strength, radius, falloff, pull_strength } => {
            GpuForceField {
                field_type: force_field_type::VORTEX,
                strength: *strength,
                radius: *radius,
                falloff: *falloff,
                position: position.to_array(),
                axis: axis.to_array(),
                param1: *pull_strength,
                ..Default::default()
            }
        }
        ForceFieldType::Attractor { position, strength, radius, dead_zone } => {
            GpuForceField {
                field_type: force_field_type::ATTRACTOR,
                strength: *strength,
                radius: *radius,
                falloff: 2.0,
                position: position.to_array(),
                param1: *dead_zone,
                ..Default::default()
            }
        }
        ForceFieldType::Wind { direction, turbulence, turbulence_frequency } => {
            GpuForceField {
                field_type: force_field_type::WIND,
                strength: direction.length(),
                axis: direction.normalize_or_zero().to_array(),
                param1: *turbulence,
                frequency: *turbulence_frequency,
                ..Default::default()
            }
        }
        ForceFieldType::Drag { coefficient, .. } => {
            GpuForceField {
                field_type: force_field_type::DRAG,
                strength: *coefficient,
                ..Default::default()
            }
        }
    }
}

// === Color/Size Helpers for EmitterConfig ===

use crate::{ColorOverLifetime, SizeOverLifetime};

impl ColorOverLifetime {
    /// 시작 색상 반환
    pub fn start_color(&self) -> [f32; 4] {
        match self {
            ColorOverLifetime::Constant(c) => *c,
            ColorOverLifetime::Gradient { start, .. } => *start,
            ColorOverLifetime::Curve(points) => {
                if points.is_empty() {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    points[0].1
                }
            }
        }
    }

    /// 끝 색상 반환
    pub fn end_color(&self) -> [f32; 4] {
        match self {
            ColorOverLifetime::Constant(c) => *c,
            ColorOverLifetime::Gradient { end, .. } => *end,
            ColorOverLifetime::Curve(points) => {
                if points.is_empty() {
                    [1.0, 1.0, 1.0, 0.0]
                } else {
                    points.last().unwrap().1
                }
            }
        }
    }
}

impl SizeOverLifetime {
    /// 시작 크기 반환
    pub fn start_size(&self) -> f32 {
        match self {
            SizeOverLifetime::Constant(s) => *s,
            SizeOverLifetime::Linear { start, .. } => *start,
            SizeOverLifetime::Curve(points) => {
                if points.is_empty() {
                    0.1
                } else {
                    points[0].1
                }
            }
        }
    }

    /// 끝 크기 반환
    pub fn end_size(&self) -> f32 {
        match self {
            SizeOverLifetime::Constant(s) => *s,
            SizeOverLifetime::Linear { end, .. } => *end,
            SizeOverLifetime::Curve(points) => {
                if points.is_empty() {
                    0.01
                } else {
                    points.last().unwrap().1
                }
            }
        }
    }
}
