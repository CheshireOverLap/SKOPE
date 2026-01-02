//! Particle Force Fields
//!
//! 파티클에 적용되는 힘 필드들
//! - Turbulence: 노이즈 기반 난류
//! - Vortex: 축을 중심으로 회전하는 소용돌이
//! - Attractor: 특정 지점으로 끌어당기는 힘
//! - Wind: 일정 방향의 바람

use glam::Vec3;

/// 3D Simplex Noise (간단한 구현)
fn simplex_noise_3d(x: f32, y: f32, z: f32) -> f32 {
    // 간단한 해시 기반 노이즈 (실제 심플렉스 노이즈 근사)
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;

    let xf = x - x.floor();
    let yf = y - y.floor();
    let zf = z - z.floor();

    // Smoothstep
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let w = zf * zf * (3.0 - 2.0 * zf);

    // Hash corners
    fn hash(x: i32, y: i32, z: i32) -> f32 {
        let n = x.wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(z.wrapping_mul(1274126177));
        let n = (n ^ (n >> 13)).wrapping_mul(1103515245);
        ((n & 0x7fffffff) as f32) / (0x7fffffff as f32) * 2.0 - 1.0
    }

    // Trilinear interpolation
    let n000 = hash(xi, yi, zi);
    let n100 = hash(xi + 1, yi, zi);
    let n010 = hash(xi, yi + 1, zi);
    let n110 = hash(xi + 1, yi + 1, zi);
    let n001 = hash(xi, yi, zi + 1);
    let n101 = hash(xi + 1, yi, zi + 1);
    let n011 = hash(xi, yi + 1, zi + 1);
    let n111 = hash(xi + 1, yi + 1, zi + 1);

    let nx00 = n000 + u * (n100 - n000);
    let nx10 = n010 + u * (n110 - n010);
    let nx01 = n001 + u * (n101 - n001);
    let nx11 = n011 + u * (n111 - n011);

    let nxy0 = nx00 + v * (nx10 - nx00);
    let nxy1 = nx01 + v * (nx11 - nx01);

    nxy0 + w * (nxy1 - nxy0)
}

/// Curl noise for turbulence (divergence-free)
fn curl_noise(pos: Vec3, frequency: f32, time: f32) -> Vec3 {
    let eps = 0.01;
    let p = pos * frequency + Vec3::new(time * 0.1, 0.0, 0.0);

    // Compute curl using finite differences
    let dx = (simplex_noise_3d(p.x + eps, p.y, p.z) - simplex_noise_3d(p.x - eps, p.y, p.z)) / (2.0 * eps);
    let dy = (simplex_noise_3d(p.x, p.y + eps, p.z) - simplex_noise_3d(p.x, p.y - eps, p.z)) / (2.0 * eps);
    let dz = (simplex_noise_3d(p.x, p.y, p.z + eps) - simplex_noise_3d(p.x, p.y, p.z - eps)) / (2.0 * eps);

    // Curl = nabla x F (simplified for scalar noise)
    Vec3::new(dy - dz, dz - dx, dx - dy)
}

/// Force field types
#[derive(Clone, Debug)]
pub enum ForceFieldType {
    /// Turbulence - 노이즈 기반 난류
    Turbulence {
        /// 강도
        strength: f32,
        /// 주파수 (노이즈 스케일, 작을수록 큰 소용돌이)
        frequency: f32,
        /// 옥타브 수 (디테일 레벨)
        octaves: u32,
        /// 옥타브 가중치 감소율
        persistence: f32,
        /// 시간에 따른 변화 속도
        scroll_speed: f32,
    },

    /// Vortex - 축 기반 회전력
    Vortex {
        /// 회전축 (정규화됨)
        axis: Vec3,
        /// 축의 위치
        position: Vec3,
        /// 회전 강도 (양수: 반시계, 음수: 시계)
        strength: f32,
        /// 최대 영향 반경
        radius: f32,
        /// 반경 감쇠 (0 = 일정, 1 = 선형, 2 = 제곱)
        falloff: f32,
        /// 축 방향 끌어당기는 힘
        pull_strength: f32,
    },

    /// Attractor - 점 끌어당김/밀어냄
    Attractor {
        /// 중심 위치
        position: Vec3,
        /// 강도 (양수: 끌어당김, 음수: 밀어냄)
        strength: f32,
        /// 최대 영향 반경
        radius: f32,
        /// 데드존 반경 (이 안에서는 힘 없음)
        dead_zone: f32,
    },

    /// Wind - 일정 방향 바람
    Wind {
        /// 바람 방향과 강도
        direction: Vec3,
        /// 바람 흔들림 강도
        turbulence: f32,
        /// 흔들림 주파수
        turbulence_frequency: f32,
    },

    /// Drag - 속도 감쇠
    Drag {
        /// 감쇠 계수 (0 = 없음, 1 = 즉시 정지)
        coefficient: f32,
    },
}

/// Force field instance
#[derive(Clone, Debug)]
pub struct ForceField {
    pub field_type: ForceFieldType,
    pub enabled: bool,
    /// 시간 축적 (애니메이션용)
    time: f32,
}

impl ForceField {
    /// Create turbulence force field
    pub fn turbulence(strength: f32, frequency: f32) -> Self {
        Self {
            field_type: ForceFieldType::Turbulence {
                strength,
                frequency,
                octaves: 3,
                persistence: 0.5,
                scroll_speed: 1.0,
            },
            enabled: true,
            time: 0.0,
        }
    }

    /// Create vortex force field
    pub fn vortex(axis: Vec3, position: Vec3, strength: f32, radius: f32) -> Self {
        Self {
            field_type: ForceFieldType::Vortex {
                axis: axis.normalize(),
                position,
                strength,
                radius,
                falloff: 1.0,
                pull_strength: 0.0,
            },
            enabled: true,
            time: 0.0,
        }
    }

    /// Create vortex with pull (tornado-like)
    pub fn tornado(position: Vec3, strength: f32, radius: f32, pull: f32) -> Self {
        Self {
            field_type: ForceFieldType::Vortex {
                axis: Vec3::Y,
                position,
                strength,
                radius,
                falloff: 1.5,
                pull_strength: pull,
            },
            enabled: true,
            time: 0.0,
        }
    }

    /// Create attractor
    pub fn attractor(position: Vec3, strength: f32, radius: f32) -> Self {
        Self {
            field_type: ForceFieldType::Attractor {
                position,
                strength,
                radius,
                dead_zone: 0.1,
            },
            enabled: true,
            time: 0.0,
        }
    }

    /// Create repulsor (negative attractor)
    pub fn repulsor(position: Vec3, strength: f32, radius: f32) -> Self {
        Self {
            field_type: ForceFieldType::Attractor {
                position,
                strength: -strength,
                radius,
                dead_zone: 0.0,
            },
            enabled: true,
            time: 0.0,
        }
    }

    /// Create wind
    pub fn wind(direction: Vec3) -> Self {
        Self {
            field_type: ForceFieldType::Wind {
                direction,
                turbulence: 0.2,
                turbulence_frequency: 2.0,
            },
            enabled: true,
            time: 0.0,
        }
    }

    /// Create drag
    pub fn drag(coefficient: f32) -> Self {
        Self {
            field_type: ForceFieldType::Drag { coefficient },
            enabled: true,
            time: 0.0,
        }
    }

    /// Update time
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
    }

    /// Calculate force at a given position with velocity
    pub fn calculate_force(&self, position: Vec3, velocity: Vec3) -> Vec3 {
        if !self.enabled {
            return Vec3::ZERO;
        }

        match &self.field_type {
            ForceFieldType::Turbulence {
                strength,
                frequency,
                octaves,
                persistence,
                scroll_speed,
            } => {
                let mut force = Vec3::ZERO;
                let mut amp = 1.0;
                let mut freq = *frequency;
                let mut total_amp = 0.0;

                for _ in 0..*octaves {
                    force += curl_noise(position, freq, self.time * scroll_speed) * amp;
                    total_amp += amp;
                    amp *= persistence;
                    freq *= 2.0;
                }

                force / total_amp * *strength
            }

            ForceFieldType::Vortex {
                axis,
                position: vortex_pos,
                strength,
                radius,
                falloff,
                pull_strength,
            } => {
                // Vector from vortex axis to particle
                let to_particle = position - *vortex_pos;

                // Project onto plane perpendicular to axis
                let along_axis = axis.dot(to_particle);
                let radial = to_particle - *axis * along_axis;
                let dist = radial.length();

                if dist < 0.001 || dist > *radius {
                    return Vec3::ZERO;
                }

                // Tangent direction (perpendicular to radial and axis)
                let tangent = axis.cross(radial.normalize());

                // Distance falloff
                let falloff_factor = (1.0 - (dist / radius).powf(*falloff)).max(0.0);

                // Rotational force
                let rotation_force = tangent * *strength * falloff_factor;

                // Pull toward axis
                let pull_force = -radial.normalize() * *pull_strength * falloff_factor;

                rotation_force + pull_force
            }

            ForceFieldType::Attractor {
                position: attr_pos,
                strength,
                radius,
                dead_zone,
            } => {
                let to_attractor = *attr_pos - position;
                let dist = to_attractor.length();

                if dist < *dead_zone || dist > *radius {
                    return Vec3::ZERO;
                }

                // Inverse square falloff
                let falloff = 1.0 - (dist / radius);
                let direction = to_attractor.normalize();

                direction * *strength * falloff * falloff
            }

            ForceFieldType::Wind {
                direction,
                turbulence,
                turbulence_frequency,
            } => {
                let noise = curl_noise(position, *turbulence_frequency, self.time);
                *direction + noise * *turbulence
            }

            ForceFieldType::Drag { coefficient } => {
                // Drag force opposes velocity, proportional to velocity squared
                let speed = velocity.length();
                if speed < 0.001 {
                    return Vec3::ZERO;
                }

                -velocity.normalize() * speed * speed * *coefficient
            }
        }
    }
}

/// Collection of force fields
#[derive(Clone, Debug, Default)]
pub struct ForceFieldSystem {
    pub fields: Vec<ForceField>,
}

impl ForceFieldSystem {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a force field
    pub fn add(&mut self, field: ForceField) {
        self.fields.push(field);
    }

    /// Update all fields
    pub fn update(&mut self, dt: f32) {
        for field in &mut self.fields {
            field.update(dt);
        }
    }

    /// Calculate combined force at position
    pub fn calculate_force(&self, position: Vec3, velocity: Vec3) -> Vec3 {
        let mut total_force = Vec3::ZERO;
        for field in &self.fields {
            total_force += field.calculate_force(position, velocity);
        }
        total_force
    }

    /// Clear all fields
    pub fn clear(&mut self) {
        self.fields.clear();
    }

    /// Remove disabled fields
    pub fn cleanup(&mut self) {
        self.fields.retain(|f| f.enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turbulence() {
        let field = ForceField::turbulence(5.0, 1.0);
        let force = field.calculate_force(Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO);
        assert!(force.length() > 0.0);
        assert!(force.length() < 10.0);
    }

    #[test]
    fn test_vortex() {
        let field = ForceField::vortex(Vec3::Y, Vec3::ZERO, 10.0, 5.0);

        // Particle at (1, 0, 0) should get force in Z direction
        let force = field.calculate_force(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO);
        assert!(force.z.abs() > force.x.abs());
    }

    #[test]
    fn test_attractor() {
        let field = ForceField::attractor(Vec3::ZERO, 10.0, 5.0);

        // Particle at (1, 0, 0) should be pulled toward origin
        let force = field.calculate_force(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO);
        assert!(force.x < 0.0); // Force points toward origin
    }

    #[test]
    fn test_repulsor() {
        let field = ForceField::repulsor(Vec3::ZERO, 10.0, 5.0);

        let force = field.calculate_force(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO);
        assert!(force.x > 0.0); // Force points away from origin
    }

    #[test]
    fn test_drag() {
        let field = ForceField::drag(0.5);

        let velocity = Vec3::new(10.0, 0.0, 0.0);
        let force = field.calculate_force(Vec3::ZERO, velocity);
        assert!(force.x < 0.0); // Force opposes velocity
    }

    #[test]
    fn test_force_field_system() {
        let mut system = ForceFieldSystem::new();
        system.add(ForceField::turbulence(1.0, 1.0));
        system.add(ForceField::wind(Vec3::new(0.0, 1.0, 0.0)));

        system.update(0.1);

        let force = system.calculate_force(Vec3::ZERO, Vec3::ZERO);
        assert!(force.y > 0.0); // Wind pushes up
    }
}
