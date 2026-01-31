//! GPU Particle System Data Structures
//!
//! GPU compute shader에서 사용되는 파티클 데이터 구조

use bytemuck::{Pod, Zeroable};

/// GPU에서 처리되는 개별 파티클 (80 bytes, 16-byte aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuParticle {
    /// 월드 좌표 위치 (x, y, z) + lifetime
    pub position: [f32; 3],
    pub lifetime: f32,

    /// 속도 (x, y, z) + max_lifetime
    pub velocity: [f32; 3],
    pub max_lifetime: f32,

    /// 색상 (r, g, b, a)
    pub color: [f32; 4],

    /// 크기, 회전, 회전 속도, alive 플래그
    pub size: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub alive: u32, // 1 = alive, 0 = dead

    /// 랜덤 시드 (노이즈 변형용) + padding
    pub seed: u32,
    pub _pad: [f32; 3],
}

impl Default for GpuParticle {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            lifetime: 0.0,
            velocity: [0.0; 3],
            max_lifetime: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            size: 0.1,
            rotation: 0.0,
            rotation_speed: 0.0,
            alive: 0,
            seed: 0,
            _pad: [0.0; 3],
        }
    }
}

/// GPU 이미터 설정 (80 bytes, 16-byte aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuEmitterConfig {
    /// 이미터 위치 (x, y, z) + delta_time
    pub emitter_position: [f32; 3],
    pub delta_time: f32,

    /// 중력 (x, y, z) + total_time
    pub gravity: [f32; 3],
    pub total_time: f32,

    /// 초기 색상 (start)
    pub color_start: [f32; 4],

    /// 끝 색상 (end)
    pub color_end: [f32; 4],

    /// 파티클 수 + 활성 force field 수 + padding
    pub particle_count: u32,
    pub force_field_count: u32,
    pub size_start: f32,
    pub size_end: f32,
}

impl Default for GpuEmitterConfig {
    fn default() -> Self {
        Self {
            emitter_position: [0.0; 3],
            delta_time: 0.016,
            gravity: [0.0, -9.8, 0.0],
            total_time: 0.0,
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            particle_count: 1024,
            force_field_count: 0,
            size_start: 0.1,
            size_end: 0.01,
        }
    }
}

/// GPU Force Field 타입 상수
pub mod force_field_type {
    pub const TURBULENCE: u32 = 0;
    pub const VORTEX: u32 = 1;
    pub const ATTRACTOR: u32 = 2;
    pub const WIND: u32 = 3;
    pub const DRAG: u32 = 4;
}

/// GPU Force Field 데이터 (64 bytes, 16-byte aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuForceField {
    /// 타입 (0=Turbulence, 1=Vortex, 2=Attractor, 3=Wind, 4=Drag)
    pub field_type: u32,
    /// 강도
    pub strength: f32,
    /// 반경
    pub radius: f32,
    /// 감쇠 (falloff)
    pub falloff: f32,

    /// 위치 또는 방향 (x, y, z) + 추가 파라미터
    pub position: [f32; 3],
    pub param1: f32,

    /// 축 또는 추가 벡터 (x, y, z) + 추가 파라미터
    pub axis: [f32; 3],
    pub param2: f32,

    /// Turbulence용: frequency, octaves, persistence, scroll_speed
    pub frequency: f32,
    pub octaves: u32,
    pub persistence: f32,
    pub scroll_speed: f32,
}

impl Default for GpuForceField {
    fn default() -> Self {
        Self {
            field_type: force_field_type::WIND,
            strength: 1.0,
            radius: 10.0,
            falloff: 1.0,
            position: [0.0; 3],
            param1: 0.0,
            axis: [0.0, 1.0, 0.0],
            param2: 0.0,
            frequency: 1.0,
            octaves: 3,
            persistence: 0.5,
            scroll_speed: 0.1,
        }
    }
}

impl GpuForceField {
    /// Turbulence force field 생성
    pub fn turbulence(strength: f32, frequency: f32, octaves: u32) -> Self {
        Self {
            field_type: force_field_type::TURBULENCE,
            strength,
            frequency,
            octaves,
            persistence: 0.5,
            scroll_speed: 0.1,
            ..Default::default()
        }
    }

    /// Vortex force field 생성
    pub fn vortex(position: [f32; 3], axis: [f32; 3], strength: f32, radius: f32) -> Self {
        Self {
            field_type: force_field_type::VORTEX,
            strength,
            radius,
            position,
            axis,
            falloff: 1.0,
            ..Default::default()
        }
    }

    /// Attractor force field 생성
    pub fn attractor(position: [f32; 3], strength: f32, radius: f32) -> Self {
        Self {
            field_type: force_field_type::ATTRACTOR,
            strength,
            radius,
            position,
            falloff: 2.0, // inverse square
            ..Default::default()
        }
    }

    /// Wind force field 생성
    pub fn wind(direction: [f32; 3], strength: f32) -> Self {
        Self {
            field_type: force_field_type::WIND,
            strength,
            axis: direction, // direction stored in axis
            ..Default::default()
        }
    }

    /// Drag force field 생성
    pub fn drag(coefficient: f32) -> Self {
        Self {
            field_type: force_field_type::DRAG,
            strength: coefficient,
            ..Default::default()
        }
    }
}

/// GPU 스폰 설정 (144 bytes, 16-byte aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuSpawnConfig {
    /// 이미터 위치 (x, y, z) + spawn_count
    pub emitter_position: [f32; 3],
    pub spawn_count: u32,

    /// 초기 속도 최소값 (x, y, z) + spawn_start_index
    pub velocity_min: [f32; 3],
    pub spawn_start_index: u32,

    /// 초기 속도 최대값 (x, y, z) + total_particles
    pub velocity_max: [f32; 3],
    pub total_particles: u32,

    /// 수명 범위
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    /// 크기 범위
    pub size_min: f32,
    pub size_max: f32,

    /// 회전 속도 범위 + 랜덤 시드 + 스폰 형태
    pub rotation_speed_min: f32,
    pub rotation_speed_max: f32,
    pub random_seed: f32,
    pub spawn_shape: u32, // 0=Point, 1=Box, 2=Sphere, 3=Cone

    /// 스폰 영역 크기 + 원뿔 각도
    pub spawn_extent: [f32; 3],
    pub cone_angle: f32,

    /// 초기 색상
    pub color_start: [f32; 4],
}

impl Default for GpuSpawnConfig {
    fn default() -> Self {
        Self {
            emitter_position: [0.0; 3],
            spawn_count: 0,
            velocity_min: [-1.0, 2.0, -1.0],
            spawn_start_index: 0,
            velocity_max: [1.0, 5.0, 1.0],
            total_particles: 1024,
            lifetime_min: 1.0,
            lifetime_max: 3.0,
            size_min: 0.05,
            size_max: 0.15,
            rotation_speed_min: -1.0,
            rotation_speed_max: 1.0,
            random_seed: 0.0,
            spawn_shape: 0, // Point
            spawn_extent: [1.0, 1.0, 1.0],
            cone_angle: 0.5,
            color_start: [1.0, 0.8, 0.3, 1.0],
        }
    }
}

/// GPU 렌더 설정 (16 bytes, 16-byte aligned)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuRenderConfig {
    pub particle_count: u32,
    pub soft_particle: u32,
    pub depth_fade_distance: f32,
    pub emission_strength: f32,
}

impl Default for GpuRenderConfig {
    fn default() -> Self {
        Self {
            particle_count: 1024,
            soft_particle: 0,
            depth_fade_distance: 0.5,
            emission_strength: 1.0,
        }
    }
}

/// Force Field 배열 (최대 8개)
pub const MAX_FORCE_FIELDS: usize = 8;

/// GPU Force Field 배열 (512 bytes = 8 * 64)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuForceFieldArray {
    pub fields: [GpuForceField; MAX_FORCE_FIELDS],
}

impl Default for GpuForceFieldArray {
    fn default() -> Self {
        Self {
            fields: [GpuForceField::default(); MAX_FORCE_FIELDS],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_particle_size() {
        assert_eq!(std::mem::size_of::<GpuParticle>(), 80);
    }

    #[test]
    fn test_gpu_emitter_config_size() {
        assert_eq!(std::mem::size_of::<GpuEmitterConfig>(), 80);
    }

    #[test]
    fn test_gpu_force_field_size() {
        assert_eq!(std::mem::size_of::<GpuForceField>(), 64);
    }

    #[test]
    fn test_gpu_force_field_array_size() {
        assert_eq!(std::mem::size_of::<GpuForceFieldArray>(), 512);
    }
}
