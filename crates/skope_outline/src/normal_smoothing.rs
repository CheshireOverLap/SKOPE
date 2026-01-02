// SKOPE Engine - Normal Smoothing for Outline
// 아웃라인이 끊기지 않도록 버텍스 노멀을 미리 스무딩

use glam::Vec3;
use std::collections::HashMap;

/// 버텍스 위치 기반 노멀 평균화
/// 같은 위치의 버텍스들의 노멀을 평균하여 실루엣이 연속되게 함
pub fn compute_smooth_normals(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    weld_threshold: f32,
) -> Vec<[f32; 3]> {
    if positions.len() != normals.len() {
        return normals.to_vec();
    }

    // 위치별 노멀 누적
    let mut position_normals: HashMap<[i32; 3], Vec<Vec3>> = HashMap::new();

    for (pos, normal) in positions.iter().zip(normals.iter()) {
        // 위치를 정수 그리드로 양자화 (welding)
        let key = quantize_position(pos, weld_threshold);

        position_normals
            .entry(key)
            .or_default()
            .push(Vec3::from(*normal));
    }

    // 평균 계산
    let averaged: HashMap<[i32; 3], Vec3> = position_normals
        .into_iter()
        .map(|(key, normals)| {
            let sum: Vec3 = normals.iter().copied().sum();
            let avg = if sum.length_squared() > 0.0001 {
                sum.normalize()
            } else {
                Vec3::Y
            };
            (key, avg)
        })
        .collect();

    // 원본 버텍스에 매핑
    positions
        .iter()
        .map(|pos| {
            let key = quantize_position(pos, weld_threshold);
            averaged.get(&key).copied().unwrap_or(Vec3::Y).into()
        })
        .collect()
}

/// 위치를 정수 그리드로 양자화
fn quantize_position(pos: &[f32; 3], threshold: f32) -> [i32; 3] {
    [
        (pos[0] / threshold).round() as i32,
        (pos[1] / threshold).round() as i32,
        (pos[2] / threshold).round() as i32,
    ]
}

/// 가중치 기반 노멀 스무딩
/// 면적이 큰 삼각형의 노멀에 더 큰 가중치
pub fn compute_area_weighted_smooth_normals(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
    weld_threshold: f32,
) -> Vec<[f32; 3]> {
    if positions.is_empty() || indices.is_empty() {
        return normals.to_vec();
    }

    // 위치별 가중 노멀 누적
    let mut position_normals: HashMap<[i32; 3], Vec3> = HashMap::new();

    // 삼각형별로 처리
    for tri in indices.chunks(3) {
        if tri.len() != 3 {
            continue;
        }

        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }

        let p0 = Vec3::from(positions[i0]);
        let p1 = Vec3::from(positions[i1]);
        let p2 = Vec3::from(positions[i2]);

        // 면적 계산 (cross product의 길이 / 2)
        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let cross = edge1.cross(edge2);
        let area = cross.length() * 0.5;

        // 면 노멀
        let face_normal = if cross.length_squared() > 0.0001 {
            cross.normalize()
        } else {
            continue;
        };

        // 가중 노멀 누적
        let weighted_normal = face_normal * area;

        for &idx in &[i0, i1, i2] {
            let key = quantize_position(&positions[idx], weld_threshold);
            *position_normals.entry(key).or_insert(Vec3::ZERO) += weighted_normal;
        }
    }

    // 정규화
    let averaged: HashMap<[i32; 3], Vec3> = position_normals
        .into_iter()
        .map(|(key, sum)| {
            let avg = if sum.length_squared() > 0.0001 {
                sum.normalize()
            } else {
                Vec3::Y
            };
            (key, avg)
        })
        .collect();

    // 원본 버텍스에 매핑
    positions
        .iter()
        .zip(normals.iter())
        .map(|(pos, original_normal)| {
            let key = quantize_position(pos, weld_threshold);
            averaged
                .get(&key)
                .copied()
                .unwrap_or(Vec3::from(*original_normal))
                .into()
        })
        .collect()
}

/// 버텍스별 두께 스케일 계산
/// 관절이나 특정 부위의 아웃라인 두께를 조절
pub fn compute_thickness_scale_from_vertex_colors(
    vertex_colors: &[[f32; 4]], // RGBA, R 채널 사용
) -> Vec<f32> {
    vertex_colors.iter().map(|c| c[0]).collect()
}

/// 기본 두께 스케일 (모든 버텍스 1.0)
pub fn default_thickness_scale(vertex_count: usize) -> Vec<f32> {
    vec![1.0; vertex_count]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smooth_normals() {
        // 같은 위치에 다른 노멀을 가진 버텍스들
        let positions = [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0], // 같은 위치
            [1.0, 0.0, 0.0],
        ];
        let normals = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];

        let smooth = compute_smooth_normals(&positions, &normals, 0.001);

        // 처음 두 버텍스는 같은 스무딩된 노멀을 가져야 함
        let n0 = Vec3::from(smooth[0]);
        let n1 = Vec3::from(smooth[1]);

        assert!(
            (n0 - n1).length() < 0.001,
            "Same position vertices should have same smoothed normal"
        );
    }
}
