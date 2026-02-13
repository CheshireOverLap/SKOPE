//! Mesh Validator — UE5.7 ValidateAndFixData() 대응
//!
//! IntermediatePrimitive를 검증하고 누락된 데이터를 자동 생성

use crate::intermediate::{IntermediatePrimitive, MeshValidationReport};

/// 메시 검증기
pub struct MeshValidator;

impl MeshValidator {
    /// 프리미티브를 검증하고 누락된 데이터를 자동 생성
    pub fn validate_and_fix(primitive: &mut IntermediatePrimitive) -> MeshValidationReport {
        let mut report = MeshValidationReport::default();

        // 1. 인덱스 자동 생성 (없으면 0..N)
        Self::ensure_indices(primitive, &mut report);

        // 2. 노멀 자동 생성 (없으면 면 노멀)
        Self::ensure_normals(primitive, &mut report);

        // 3. 탄젠트 자동 생성 (없으면 계산)
        Self::ensure_tangents(primitive, &mut report);

        // 4. 디제너레이트 삼각형 제거
        Self::remove_degenerate_triangles(primitive, &mut report);

        report
    }

    /// 인덱스 자동 생성
    fn ensure_indices(primitive: &mut IntermediatePrimitive, report: &mut MeshValidationReport) {
        if primitive.indices.is_none() {
            let count = primitive.positions.len() as u32;
            primitive.indices = Some((0..count).collect());
            report.auto_generated_indices = true;
            log::debug!("[Validator] Auto-generated {} sequential indices", count);
        }
    }

    /// 면 노멀 자동 생성
    fn ensure_normals(primitive: &mut IntermediatePrimitive, report: &mut MeshValidationReport) {
        if primitive.normals.is_some() {
            return;
        }

        let positions = &primitive.positions;
        let indices = primitive.indices.as_ref().expect("indices should be ensured before normals");
        let mut normals = vec![[0.0f32; 3]; positions.len()];

        // 삼각형별 면 노멀 누적
        for tri in indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                continue;
            }

            let v0 = positions[i0];
            let v1 = positions[i1];
            let v2 = positions[i2];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            // Cross product
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];

            for &idx in &[i0, i1, i2] {
                normals[idx][0] += n[0];
                normals[idx][1] += n[1];
                normals[idx][2] += n[2];
            }
        }

        // 정규화
        for normal in &mut normals {
            let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            if len > 1e-6 {
                normal[0] /= len;
                normal[1] /= len;
                normal[2] /= len;
            } else {
                *normal = [0.0, 1.0, 0.0]; // 기본 UP 방향
            }
        }

        primitive.normals = Some(normals);
        report.auto_generated_normals = true;
        log::debug!("[Validator] Auto-generated face normals for {} vertices", positions.len());
    }

    /// 탄젠트 자동 생성 (MikkTSpace 기반)
    fn ensure_tangents(primitive: &mut IntermediatePrimitive, report: &mut MeshValidationReport) {
        if primitive.tangents.is_some() {
            return;
        }

        let positions = &primitive.positions;
        let normals = primitive.normals.as_ref().expect("normals should be ensured before tangents");
        let uvs = primitive.tex_coords_0.as_ref()
            .cloned()
            .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
        let indices = primitive.indices.as_ref().expect("indices should be ensured before tangents");

        let tangents = Self::calculate_tangents(positions, normals, &uvs, indices);
        primitive.tangents = Some(tangents);
        report.auto_generated_tangents = true;
        log::debug!("[Validator] Auto-generated tangents for {} vertices", positions.len());
    }

    /// 디제너레이트 삼각형 제거 (면적 = 0)
    fn remove_degenerate_triangles(primitive: &mut IntermediatePrimitive, report: &mut MeshValidationReport) {
        let indices = match primitive.indices.as_mut() {
            Some(idx) => idx,
            None => return,
        };

        let positions = &primitive.positions;
        let original_count = indices.len() / 3;
        let mut valid_indices = Vec::with_capacity(indices.len());

        for tri in indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                continue;
            }

            // 면적 체크 (cross product magnitude)
            let v0 = positions[i0];
            let v1 = positions[i1];
            let v2 = positions[i2];

            let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];

            let area_sq = cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2];

            if area_sq > 1e-12 {
                valid_indices.push(tri[0]);
                valid_indices.push(tri[1]);
                valid_indices.push(tri[2]);
            }
        }

        let removed = original_count - valid_indices.len() / 3;
        if removed > 0 {
            report.degenerate_triangles_removed = removed as u32;
            *indices = valid_indices;
            log::debug!("[Validator] Removed {} degenerate triangles", removed);
        }
    }

    /// 탄젠트 계산 (Gram-Schmidt orthogonalization)
    fn calculate_tangents(
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        uvs: &[[f32; 2]],
        indices: &[u32],
    ) -> Vec<[f32; 4]> {
        let mut tangents = vec![[0.0f32; 3]; positions.len()];
        let mut bitangents = vec![[0.0f32; 3]; positions.len()];

        for tri in indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
                continue;
            }

            let v0 = positions[i0];
            let v1 = positions[i1];
            let v2 = positions[i2];

            let uv0 = uvs[i0];
            let uv1 = uvs[i1];
            let uv2 = uvs[i2];

            let delta_pos1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let delta_pos2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

            let delta_uv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
            let delta_uv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];

            let r = 1.0 / (delta_uv1[0] * delta_uv2[1] - delta_uv1[1] * delta_uv2[0] + 1e-5);

            let tangent = [
                r * (delta_uv2[1] * delta_pos1[0] - delta_uv1[1] * delta_pos2[0]),
                r * (delta_uv2[1] * delta_pos1[1] - delta_uv1[1] * delta_pos2[1]),
                r * (delta_uv2[1] * delta_pos1[2] - delta_uv1[1] * delta_pos2[2]),
            ];

            let bitangent = [
                r * (-delta_uv2[0] * delta_pos1[0] + delta_uv1[0] * delta_pos2[0]),
                r * (-delta_uv2[0] * delta_pos1[1] + delta_uv1[0] * delta_pos2[1]),
                r * (-delta_uv2[0] * delta_pos1[2] + delta_uv1[0] * delta_pos2[2]),
            ];

            for &idx in &[i0, i1, i2] {
                tangents[idx][0] += tangent[0];
                tangents[idx][1] += tangent[1];
                tangents[idx][2] += tangent[2];
                bitangents[idx][0] += bitangent[0];
                bitangents[idx][1] += bitangent[1];
                bitangents[idx][2] += bitangent[2];
            }
        }

        // Gram-Schmidt orthogonalize + handedness
        tangents.iter()
            .zip(normals.iter())
            .zip(bitangents.iter())
            .map(|((t, n), b)| {
                let n_dot_t = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
                let ortho = [
                    t[0] - n[0] * n_dot_t,
                    t[1] - n[1] * n_dot_t,
                    t[2] - n[2] * n_dot_t,
                ];

                let len = (ortho[0] * ortho[0] + ortho[1] * ortho[1] + ortho[2] * ortho[2]).sqrt();
                let normalized = if len > 1e-5 {
                    [ortho[0] / len, ortho[1] / len, ortho[2] / len]
                } else {
                    [1.0, 0.0, 0.0]
                };

                let cross = [
                    n[1] * normalized[2] - n[2] * normalized[1],
                    n[2] * normalized[0] - n[0] * normalized[2],
                    n[0] * normalized[1] - n[1] * normalized[0],
                ];
                let handedness = if cross[0] * b[0] + cross[1] * b[1] + cross[2] * b[2] < 0.0 {
                    -1.0
                } else {
                    1.0
                };

                [normalized[0], normalized[1], normalized[2], handedness]
            })
            .collect()
    }
}
