//! Transform quantization for bandwidth reduction.
//!
//! Provides compact representations of Vec3 and Quat:
//! - QVec3: 6 bytes (3 × i16, scale=100, precision 0.01)
//! - QQuat: 8 bytes (smallest-three encoding)

use glam::{Vec3, Quat};
use serde::{Serialize, Deserialize};

/// Quantized Vec3: 6 bytes (3 × i16).
/// Scale factor of 100 gives 0.01 unit precision over range [-327.67, 327.67].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QVec3 {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

const QVEC3_SCALE: f32 = 100.0;

impl QVec3 {
    /// Encode a Vec3 into a QVec3.
    pub fn encode(v: Vec3) -> Self {
        Self {
            x: (v.x * QVEC3_SCALE).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
            y: (v.y * QVEC3_SCALE).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
            z: (v.z * QVEC3_SCALE).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
        }
    }

    /// Decode back to Vec3.
    pub fn decode(self) -> Vec3 {
        Vec3::new(
            self.x as f32 / QVEC3_SCALE,
            self.y as f32 / QVEC3_SCALE,
            self.z as f32 / QVEC3_SCALE,
        )
    }
}

/// Quantized Quat: 8 bytes using smallest-three encoding.
/// Stores the index of the largest component (2 bits) and three
/// smallest components as i16 (each scaled to [-1, 1] range).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QQuat {
    /// Packed: bits[0..2] = largest component index, bits[2..16] = first smallest
    pub a: u16,
    pub b: i16,
    pub c: i16,
}

const QQUAT_SCALE: f32 = 16383.0; // max for 14-bit signed

impl QQuat {
    /// Encode a Quat using smallest-three encoding.
    pub fn encode(q: Quat) -> Self {
        let q = if q.w < 0.0 { -q } else { q }; // ensure w >= 0
        let components = [q.x, q.y, q.z, q.w];

        // Find the largest component
        let mut largest_idx = 0;
        let mut largest_val = components[0].abs();
        for i in 1..4 {
            let abs_val = components[i].abs();
            if abs_val > largest_val {
                largest_idx = i;
                largest_val = abs_val;
            }
        }

        // Get the three smallest components
        let mut small = [0.0f32; 3];
        let mut si = 0;
        for i in 0..4 {
            if i != largest_idx {
                small[si] = components[i];
                si += 1;
            }
        }

        // Quantize: range is [-1/√2, 1/√2] ≈ [-0.707, 0.707]
        let scale = QQUAT_SCALE / 0.70710678; // sqrt(2)/2
        let a_val = (small[0] * scale).round().clamp(-16383.0, 16383.0) as i16;
        let b_val = (small[1] * scale).round().clamp(-16383.0, 16383.0) as i16;
        let c_val = (small[2] * scale).round().clamp(-16383.0, 16383.0) as i16;

        // Pack largest_idx (2 bits) + first component (14 bits) into u16
        let a_packed = ((largest_idx as u16) << 14) | ((a_val as u16) & 0x3FFF);

        Self {
            a: a_packed,
            b: b_val,
            c: c_val,
        }
    }

    /// Decode back to Quat.
    pub fn decode(self) -> Quat {
        let largest_idx = (self.a >> 14) as usize;
        // Sign-extend the 14-bit value: if bit 13 is set, the original was negative
        let raw_a = (self.a & 0x3FFF) as i16;
        let a_val = if raw_a >= 0x2000 { raw_a - 0x4000 } else { raw_a } as f32;
        let b_val = self.b as f32;
        let c_val = self.c as f32;

        let scale = 0.70710678 / QQUAT_SCALE;
        let small = [a_val * scale, b_val * scale, c_val * scale];

        // Reconstruct largest component
        let sum_sq: f32 = small.iter().map(|x| x * x).sum();
        let largest_val = (1.0 - sum_sq).max(0.0).sqrt();

        let mut components = [0.0f32; 4];
        let mut si = 0;
        for i in 0..4 {
            if i == largest_idx {
                components[i] = largest_val;
            } else {
                components[i] = small[si];
                si += 1;
            }
        }

        Quat::from_xyzw(components[0], components[1], components[2], components[3]).normalize()
    }
}

/// Quantized Transform: 18 bytes total (vs 40 bytes for full f32 Transform).
/// translation: QVec3(6B) + rotation: QQuat(6B) + scale: QVec3(6B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedTransform {
    pub translation: QVec3,
    pub rotation: QQuat,
    pub scale: QVec3,
}

impl QuantizedTransform {
    /// Encode a full-precision Transform into a quantized representation.
    pub fn encode(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation: QVec3::encode(translation),
            rotation: QQuat::encode(rotation),
            scale: QVec3::encode(scale),
        }
    }

    /// Decode back to full-precision (translation, rotation, scale).
    pub fn decode(&self) -> (Vec3, Quat, Vec3) {
        (
            self.translation.decode(),
            self.rotation.decode(),
            self.scale.decode(),
        )
    }

    /// Serialize to bytes (bincode). Always succeeds for fixed-size types.
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("QuantizedTransform serialization should never fail")
    }

    /// Deserialize from bytes (bincode).
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        bincode::deserialize(data).map_err(|e| format!("QuantizedTransform decode: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qvec3_roundtrip() {
        let original = Vec3::new(1.23, -4.56, 7.89);
        let quantized = QVec3::encode(original);
        let decoded = quantized.decode();

        assert!((decoded.x - original.x).abs() < 0.01);
        assert!((decoded.y - original.y).abs() < 0.01);
        assert!((decoded.z - original.z).abs() < 0.01);
    }

    #[test]
    fn test_qvec3_zero() {
        let q = QVec3::encode(Vec3::ZERO);
        let d = q.decode();
        assert_eq!(d, Vec3::ZERO);
    }

    #[test]
    fn test_qvec3_clamp() {
        // Values beyond i16 range should be clamped
        let extreme = Vec3::new(500.0, -500.0, 0.0);
        let q = QVec3::encode(extreme);
        let d = q.decode();
        assert!((d.x - 327.67).abs() < 0.02); // clamped to max
    }

    #[test]
    fn test_qquat_roundtrip() {
        let original = Quat::from_rotation_z(1.23).normalize();
        let quantized = QQuat::encode(original);
        let decoded = quantized.decode();

        let dot = original.dot(decoded).abs();
        assert!(dot > 0.999, "dot product: {} (expected > 0.999)", dot);
    }

    #[test]
    fn test_qquat_identity() {
        let q = QQuat::encode(Quat::IDENTITY);
        let d = q.decode();
        let dot = Quat::IDENTITY.dot(d).abs();
        assert!(dot > 0.999);
    }

    #[test]
    fn test_qquat_various_rotations() {
        let rotations = [
            Quat::from_rotation_x(0.5),
            Quat::from_rotation_y(1.0),
            Quat::from_rotation_z(2.0),
            Quat::from_euler(glam::EulerRot::XYZ, 0.3, 0.7, 1.2),
        ];

        for original in &rotations {
            let q = QQuat::encode(*original);
            let d = q.decode();
            let dot = original.dot(d).abs();
            assert!(dot > 0.999, "dot product: {} for {:?}", dot, original);
        }
    }

    #[test]
    fn test_quantized_transform_roundtrip() {
        let translation = Vec3::new(1.23, -4.56, 7.89);
        let rotation = Quat::from_euler(glam::EulerRot::XYZ, 0.3, 0.7, 1.2);
        let scale = Vec3::new(1.0, 2.0, 0.5);

        let qt = QuantizedTransform::encode(translation, rotation, scale);

        // Verify bincode size: 6 + 6 + 6 = 18 bytes
        let bytes = bincode::serialize(&qt).unwrap();
        assert_eq!(bytes.len(), 18, "QuantizedTransform should be 18 bytes, got {}", bytes.len());

        // Roundtrip via bincode
        let qt2: QuantizedTransform = bincode::deserialize(&bytes).unwrap();
        let (t, r, s) = qt2.decode();

        assert!((t - translation).length() < 0.02);
        assert!(rotation.dot(r).abs() > 0.999);
        assert!((s - scale).length() < 0.02);
    }

    #[test]
    fn test_quantized_transform_vs_full_size() {
        // Full Transform (3×f32 + 4×f32 + 3×f32 = 40 bytes in bincode)
        // vs QuantizedTransform (3×i16 + u16+2×i16 + 3×i16 = 18 bytes)
        let qt = QuantizedTransform::encode(Vec3::ONE, Quat::IDENTITY, Vec3::ONE);
        let q_size = bincode::serialize(&qt).unwrap().len();
        assert_eq!(q_size, 18);
        // 55% reduction from 40 bytes
    }
}
