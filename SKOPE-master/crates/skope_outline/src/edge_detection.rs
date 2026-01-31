// SKOPE Engine - Edge Detection
// Sobel/Roberts 기반 엣지 검출


/// Sobel 커널 (3x3)
pub const SOBEL_X: [[f32; 3]; 3] = [[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]];

pub const SOBEL_Y: [[f32; 3]; 3] = [[-1.0, -2.0, -1.0], [0.0, 0.0, 0.0], [1.0, 2.0, 1.0]];

/// Roberts Cross 커널 (2x2, 더 얇은 라인)
pub const ROBERTS_X: [[f32; 2]; 2] = [[1.0, 0.0], [0.0, -1.0]];

pub const ROBERTS_Y: [[f32; 2]; 2] = [[0.0, 1.0], [-1.0, 0.0]];

/// Prewitt 커널 (3x3, Sobel보다 노이즈에 덜 민감)
pub const PREWITT_X: [[f32; 3]; 3] = [[-1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [-1.0, 0.0, 1.0]];

pub const PREWITT_Y: [[f32; 3]; 3] = [[-1.0, -1.0, -1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]];

/// 엣지 검출 소스 타입
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeSource {
    /// 깊이 차이 기반
    Depth,
    /// 노멀 방향 차이 기반
    Normal,
    /// 오브젝트/재질 ID 경계
    ObjectId,
    /// 색상 차이 기반
    Color,
}

/// 엣지 검출 연산자 타입
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EdgeOperator {
    /// Sobel (3x3) - 일반적인 선택
    #[default]
    Sobel,
    /// Roberts Cross (2x2) - 더 얇은 라인
    Roberts,
    /// Prewitt (3x3) - 노이즈에 덜 민감
    Prewitt,
}

/// CPU에서 Sobel 적용 (테스트/프리뷰용)
pub fn apply_sobel_depth(
    depth_buffer: &[f32],
    width: usize,
    height: usize,
) -> Vec<f32> {
    let mut result = vec![0.0; width * height];

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let mut gx = 0.0f32;
            let mut gy = 0.0f32;

            for ky in 0..3 {
                for kx in 0..3 {
                    let px = x + kx - 1;
                    let py = y + ky - 1;
                    let idx = py * width + px;
                    let depth = depth_buffer[idx];

                    gx += depth * SOBEL_X[ky][kx];
                    gy += depth * SOBEL_Y[ky][kx];
                }
            }

            result[y * width + x] = (gx * gx + gy * gy).sqrt();
        }
    }

    result
}

/// 노멀 불연속 검출 (CPU)
pub fn detect_normal_discontinuity(
    normals: &[[f32; 3]],
    width: usize,
    height: usize,
    threshold: f32,
) -> Vec<f32> {
    let mut result = vec![0.0; width * height];

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            let n_center = glam::Vec3::from(normals[idx]);

            let mut max_diff = 0.0f32;

            // 4방향 이웃
            let neighbors = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];

            for (nx, ny) in neighbors {
                if nx < width && ny < height {
                    let n_neighbor = glam::Vec3::from(normals[ny * width + nx]);
                    let diff = 1.0 - n_center.dot(n_neighbor);
                    max_diff = max_diff.max(diff);
                }
            }

            if max_diff > threshold {
                result[idx] = (max_diff - threshold) / (1.0 - threshold);
            }
        }
    }

    result
}

/// Object ID 경계 검출 (CPU)
pub fn detect_id_edges(ids: &[u32], width: usize, height: usize) -> Vec<f32> {
    let mut result = vec![0.0; width * height];

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let id_center = ids[idx];

            let mut is_edge = false;

            // 4방향 체크
            if x > 0 && ids[idx - 1] != id_center {
                is_edge = true;
            }
            if x + 1 < width && ids[idx + 1] != id_center {
                is_edge = true;
            }
            if y > 0 && ids[(y - 1) * width + x] != id_center {
                is_edge = true;
            }
            if y + 1 < height && ids[(y + 1) * width + x] != id_center {
                is_edge = true;
            }

            result[idx] = if is_edge { 1.0 } else { 0.0 };
        }
    }

    result
}

/// 엣지 결과 합성
pub fn combine_edges(
    depth_edges: &[f32],
    normal_edges: &[f32],
    id_edges: &[f32],
    depth_weight: f32,
    normal_weight: f32,
    id_weight: f32,
) -> Vec<f32> {
    let len = depth_edges.len().min(normal_edges.len()).min(id_edges.len());

    (0..len)
        .map(|i| {
            let combined = depth_edges[i] * depth_weight
                + normal_edges[i] * normal_weight
                + id_edges[i] * id_weight;
            combined.clamp(0.0, 1.0)
        })
        .collect()
}

/// 엣지 강도를 임계값으로 이진화
pub fn threshold_edges(edges: &[f32], threshold: f32) -> Vec<f32> {
    edges
        .iter()
        .map(|&e| if e > threshold { 1.0 } else { 0.0 })
        .collect()
}

/// 라인 두께 확장 (Dilation)
pub fn dilate_edges(edges: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let mut result = vec![0.0; width * height];

    for y in 0..height {
        for x in 0..width {
            let mut max_val = 0.0f32;

            for dy in 0..=radius * 2 {
                for dx in 0..=radius * 2 {
                    let nx = (x + dx).saturating_sub(radius);
                    let ny = (y + dy).saturating_sub(radius);

                    if nx < width && ny < height {
                        max_val = max_val.max(edges[ny * width + nx]);
                    }
                }
            }

            result[y * width + x] = max_val;
        }
    }

    result
}
