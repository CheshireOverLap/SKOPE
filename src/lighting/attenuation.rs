// SKOPE Engine - Light Attenuation Functions
// Distance and Angular Falloff

use glam::Vec3;

/// 물리 기반 거리 감쇠 (Inverse Square with Smooth Falloff)
/// UE4/Unity 스타일
pub fn distance_attenuation(distance: f32, radius: f32) -> f32 {
    if distance >= radius {
        return 0.0;
    }

    let ratio = distance / radius;
    let ratio2 = ratio * ratio;
    let ratio4 = ratio2 * ratio2;

    // Smooth falloff: (1 - (d/r)^4)^2 / (1 + d^2)
    let numerator = (1.0 - ratio4).max(0.0);
    let denominator = 1.0 + distance * distance;

    (numerator * numerator) / denominator
}

/// Spotlight Angular Attenuation
/// inner_angle, outer_angle: cos(angle) 값
pub fn spot_attenuation(
    light_dir: Vec3,
    spot_dir: Vec3,
    inner_cos: f32,
    outer_cos: f32,
) -> f32 {
    let cos_angle = light_dir.dot(-spot_dir);

    // Smooth falloff between inner and outer cone
    let t = (cos_angle - outer_cos) / (inner_cos - outer_cos);
    t.clamp(0.0, 1.0).powf(2.0)  // 제곱으로 부드럽게
}

/// Rectangular Area Light Attenuation (LTC 기반 근사)
pub fn rect_area_attenuation(
    p: Vec3,           // Surface point
    light_pos: Vec3,
    light_dir: Vec3,
    light_up: Vec3,
    width: f32,
    height: f32,
    two_sided: bool,
) -> f32 {
    let to_light = light_pos - p;
    let dist = to_light.length();

    if dist < 0.001 {
        return 1.0;
    }

    let dir_to_light = to_light / dist;

    // Backface culling (one-sided)
    if !two_sided && dir_to_light.dot(light_dir) > 0.0 {
        return 0.0;
    }

    // Solid angle 근사
    let half_width = width * 0.5;
    let half_height = height * 0.5;
    let area = width * height;
    let solid_angle = area / (dist * dist);

    // Orientation factor
    let cos_theta = dir_to_light.dot(-light_dir).abs();

    solid_angle * cos_theta
}

/// Disk Area Light Attenuation
pub fn disk_area_attenuation(
    p: Vec3,
    light_pos: Vec3,
    light_dir: Vec3,
    disk_radius: f32,
) -> f32 {
    let to_light = light_pos - p;
    let dist = to_light.length();

    if dist < 0.001 {
        return 1.0;
    }

    let dir_to_light = to_light / dist;

    // Solid angle for disk
    let area = std::f32::consts::PI * disk_radius * disk_radius;
    let solid_angle = area / (dist * dist);

    let cos_theta = dir_to_light.dot(-light_dir).max(0.0);

    solid_angle * cos_theta
}

/// IES Profile 감쇠 (미리 계산된 LUT 사용)
pub struct IESProfile {
    // 수평/수직 각도별 강도
    pub horizontal_angles: Vec<f32>,  // degrees
    pub vertical_angles: Vec<f32>,    // degrees
    pub candela_values: Vec<Vec<f32>>,  // [horizontal][vertical]
    max_candela: f32,
}

impl IESProfile {
    pub fn new(
        horizontal_angles: Vec<f32>,
        vertical_angles: Vec<f32>,
        candela_values: Vec<Vec<f32>>,
    ) -> Self {
        let max_candela = candela_values
            .iter()
            .flat_map(|row| row.iter())
            .cloned()
            .fold(0.0f32, f32::max);

        Self {
            horizontal_angles,
            vertical_angles,
            candela_values,
            max_candela,
        }
    }

    /// 특정 방향의 IES 강도 샘플링
    pub fn sample(&self, direction: Vec3) -> f32 {
        if self.candela_values.is_empty() {
            return 1.0;
        }

        // 방향을 구면 좌표로 변환
        let horizontal = direction.x.atan2(direction.z).to_degrees();
        let horizontal = if horizontal < 0.0 { horizontal + 360.0 } else { horizontal };

        let vertical = direction.y.acos().to_degrees();

        // Bilinear interpolation
        let (h_idx, h_t) = find_lerp_indices(&self.horizontal_angles, horizontal);
        let (v_idx, v_t) = find_lerp_indices(&self.vertical_angles, vertical);

        let h0v0 = self.candela_values.get(h_idx).and_then(|row| row.get(v_idx)).copied().unwrap_or(0.0);
        let h1v0 = self.candela_values.get(h_idx + 1).and_then(|row| row.get(v_idx)).copied().unwrap_or(h0v0);
        let h0v1 = self.candela_values.get(h_idx).and_then(|row| row.get(v_idx + 1)).copied().unwrap_or(h0v0);
        let h1v1 = self.candela_values.get(h_idx + 1).and_then(|row| row.get(v_idx + 1)).copied().unwrap_or(h0v0);

        let v0 = h0v0 + (h1v0 - h0v0) * h_t;
        let v1 = h0v1 + (h1v1 - h0v1) * h_t;
        let result = v0 + (v1 - v0) * v_t;

        result / self.max_candela.max(1.0)
    }
}

fn find_lerp_indices(angles: &[f32], value: f32) -> (usize, f32) {
    if angles.is_empty() {
        return (0, 0.0);
    }

    for i in 0..angles.len() - 1 {
        if value >= angles[i] && value < angles[i + 1] {
            let t = (value - angles[i]) / (angles[i + 1] - angles[i]);
            return (i, t);
        }
    }

    (angles.len().saturating_sub(2), 1.0)
}

/// 조합된 감쇠 계산
pub fn combined_attenuation(
    distance: f32,
    radius: f32,
    spot_cos: Option<(f32, f32, Vec3, Vec3)>,  // (inner_cos, outer_cos, light_dir, spot_dir)
    ies_profile: Option<(&IESProfile, Vec3)>,
) -> f32 {
    let mut atten = distance_attenuation(distance, radius);

    if let Some((inner, outer, light_dir, spot_dir)) = spot_cos {
        atten *= spot_attenuation(light_dir, spot_dir, inner, outer);
    }

    if let Some((profile, dir)) = ies_profile {
        atten *= profile.sample(dir);
    }

    atten
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_attenuation() {
        let radius = 10.0;

        // At center
        assert!((distance_attenuation(0.0, radius) - 1.0).abs() < 0.01);

        // At boundary
        assert!(distance_attenuation(radius, radius) < 0.01);

        // Beyond radius
        assert_eq!(distance_attenuation(radius + 1.0, radius), 0.0);

        // Monotonically decreasing
        let a1 = distance_attenuation(2.0, radius);
        let a2 = distance_attenuation(5.0, radius);
        let a3 = distance_attenuation(8.0, radius);
        assert!(a1 > a2);
        assert!(a2 > a3);
    }

    #[test]
    fn test_spot_attenuation() {
        let spot_dir = Vec3::new(0.0, -1.0, 0.0);
        let inner_cos = 0.9f32.cos();  // ~25 degrees
        let outer_cos = 0.7f32.cos();  // ~45 degrees

        // Center of cone
        let center_atten = spot_attenuation(Vec3::new(0.0, 1.0, 0.0), spot_dir, inner_cos, outer_cos);
        assert!((center_atten - 1.0).abs() < 0.1);

        // Outside cone
        let outside = spot_attenuation(Vec3::new(1.0, 0.0, 0.0), spot_dir, inner_cos, outer_cos);
        assert!(outside < 0.01);
    }
}
