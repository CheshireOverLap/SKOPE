//! SKOPE Engine - Color Manipulation
//! NetEase 방식 기반 스타일라이즈드 색 조작

use glam::Vec3;

/// RGB → HSV 변환
pub fn rgb_to_hsv(rgb: Vec3) -> Vec3 {
    let max = rgb.x.max(rgb.y).max(rgb.z);
    let min = rgb.x.min(rgb.y).min(rgb.z);
    let delta = max - min;

    let v = max;
    let s = if max > 0.0 { delta / max } else { 0.0 };

    let h = if delta == 0.0 {
        0.0
    } else if max == rgb.x {
        60.0 * (((rgb.y - rgb.z) / delta) % 6.0)
    } else if max == rgb.y {
        60.0 * ((rgb.z - rgb.x) / delta + 2.0)
    } else {
        60.0 * ((rgb.x - rgb.y) / delta + 4.0)
    };

    let h_normalized = if h < 0.0 { (h + 360.0) / 360.0 } else { h / 360.0 };

    Vec3::new(h_normalized, s, v)
}

/// HSV → RGB 변환
pub fn hsv_to_rgb(hsv: Vec3) -> Vec3 {
    let h = hsv.x * 360.0;
    let s = hsv.y;
    let v = hsv.z;

    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Vec3::new(r + m, g + m, b + m)
}

/// 스타일라이즈드 피부/얼굴 색 조작 적용
pub fn apply_skin_color_manipulation(
    base_color: Vec3,
    shadow_factor: f32,
    saturation_boost: f32,
    hue_shift: f32,
    highlight_saturation_reduce: f32,
) -> Vec3 {
    let mut hsv = rgb_to_hsv(base_color);

    hsv.y += shadow_factor * saturation_boost;
    hsv.y = hsv.y.clamp(0.0, 1.0);

    hsv.x += shadow_factor * hue_shift;
    if hsv.x > 1.0 {
        hsv.x -= 1.0;
    }
    if hsv.x < 0.0 {
        hsv.x += 1.0;
    }

    let highlight_factor = (1.0 - shadow_factor).powi(2);
    hsv.y -= highlight_factor * highlight_saturation_reduce;
    hsv.y = hsv.y.clamp(0.0, 1.0);

    hsv_to_rgb(hsv)
}

/// 그림자 색상 직접 지정 방식
pub fn apply_shadow_color_ramp(
    base_color: Vec3,
    shadow_color: Vec3,
    shadow_factor: f32,
) -> Vec3 {
    base_color.lerp(shadow_color, shadow_factor)
}

/// 색온도 조정 (따뜻하게/차갑게)
pub fn adjust_color_temperature(color: Vec3, temperature: f32) -> Vec3 {
    let warm_shift = Vec3::new(0.1, 0.0, -0.1) * temperature;
    (color + warm_shift).clamp(Vec3::ZERO, Vec3::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_hsv_roundtrip() {
        let colors = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(1.0, 0.5, 0.25),
        ];

        for color in colors {
            let hsv = rgb_to_hsv(color);
            let rgb = hsv_to_rgb(hsv);

            assert!((color.x - rgb.x).abs() < 0.01, "Red channel mismatch");
            assert!((color.y - rgb.y).abs() < 0.01, "Green channel mismatch");
            assert!((color.z - rgb.z).abs() < 0.01, "Blue channel mismatch");
        }
    }
}
