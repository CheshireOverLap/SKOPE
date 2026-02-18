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
        // inner_cos > outer_cos (tighter cone = larger cosine value)
        let inner_cos = 0.4f32.cos();  // ~23 degrees (inner cone, tighter)
        let outer_cos = 0.8f32.cos();  // ~46 degrees (outer cone, wider)

        // Center of cone (light pointing opposite to spot direction)
        let center_atten = spot_attenuation(Vec3::new(0.0, 1.0, 0.0), spot_dir, inner_cos, outer_cos);
        assert!((center_atten - 1.0).abs() < 0.1, "Center attenuation was {}", center_atten);

        // Outside cone (perpendicular direction)
        let outside = spot_attenuation(Vec3::new(1.0, 0.0, 0.0), spot_dir, inner_cos, outer_cos);
        assert!(outside < 0.01, "Outside attenuation was {}", outside);
    }
}
