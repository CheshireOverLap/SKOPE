//! Coordinate System Conversion
//!
//! Y-up → Z-up coordinate system conversion functions
//! glTF (Y-up) → Blender/SKOPE (Z-up)
//! Transformation: (x, y, z) → (x, -z, y)

/// Position/Vector 변환: (x, y, z) → (x, -z, y)
#[inline]
pub fn convert_vec3(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[2], v[1]]
}

/// Tangent 변환: xyz는 벡터처럼, w(handedness)는 부호 반전
#[inline]
pub fn convert_tangent(t: [f32; 4]) -> [f32; 4] {
    [t[0], -t[2], t[1], -t[3]]
}

/// Quaternion 변환: (x, y, z, w) → (x, -z, y, w)
#[inline]
pub fn convert_quat(q: [f32; 4]) -> [f32; 4] {
    [q[0], -q[2], q[1], q[3]]
}

/// 4x4 행렬 변환 (inverse bind matrix 등)
pub fn convert_matrix(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // 좌표계 변환 행렬 S와 그 역행렬 S^-1
    // S: Y-up → Z-up 변환
    // result = S * M * S^-1
    let s = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let s_inv = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    let temp = mat4_mul(s, m);
    mat4_mul(temp, s_inv)
}

/// 4x4 행렬 곱셈 헬퍼
fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}
