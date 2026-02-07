//! Meshlet (cluster) builder — converts a triangle mesh into meshlets.
//!
//! Each meshlet contains up to MAX_MESHLET_VERTICES vertices and
//! MAX_MESHLET_TRIANGLES triangles, grouped for spatial locality.

use crate::types::{Meshlet, NaniteFullVertex, MAX_MESHLET_VERTICES, MAX_MESHLET_TRIANGLES};

/// A complete Nanite mesh resource containing all LOD levels.
pub struct NaniteMesh {
    /// All meshlets across all LOD levels.
    pub meshlets: Vec<Meshlet>,
    /// Vertex positions (culling/rasterize用).
    pub vertex_positions: Vec<[f32; 3]>,
    /// Full vertex data for material evaluation (normal, tangent, UV).
    /// If empty, fallback vertices are generated from positions during upload.
    pub vertex_data: Vec<NaniteFullVertex>,
    /// Meshlet-local triangle indices (3 bytes per triangle: v0, v1, v2 as u8 offsets).
    pub meshlet_triangles: Vec<u8>,
    /// Number of LOD levels.
    pub lod_levels: u32,
    /// Total triangle count across all meshlets.
    pub total_triangles: u32,
}

/// Build meshlets from raw vertex positions and triangle indices.
///
/// This is a simplified greedy algorithm. For production quality,
/// consider using the `meshopt` crate which wraps meshoptimizer.
pub fn build_meshlets(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> (Vec<Meshlet>, Vec<u8>) {
    let triangle_count = indices.len() / 3;
    let mut meshlets = Vec::new();
    let mut meshlet_triangles = Vec::new();

    let mut tri_idx = 0;
    while tri_idx < triangle_count {
        let mut meshlet_vertex_map: Vec<u32> = Vec::new();
        let mut local_triangles: Vec<u8> = Vec::new();
        let mut meshlet_tri_count = 0u32;
        let tri_start = meshlet_triangles.len() as u32;

        while tri_idx < triangle_count
            && meshlet_tri_count < MAX_MESHLET_TRIANGLES
        {
            let i0 = indices[tri_idx * 3];
            let i1 = indices[tri_idx * 3 + 1];
            let i2 = indices[tri_idx * 3 + 2];

            // Map global vertex indices to meshlet-local indices.
            let l0 = get_or_add_vertex(&mut meshlet_vertex_map, i0);
            let l1 = get_or_add_vertex(&mut meshlet_vertex_map, i1);
            let l2 = get_or_add_vertex(&mut meshlet_vertex_map, i2);

            // Check vertex limit.
            if meshlet_vertex_map.len() > MAX_MESHLET_VERTICES as usize {
                // Revert: remove the vertices we just added.
                while meshlet_vertex_map.len() > l0.max(l1).max(l2) as usize + 1 {
                    meshlet_vertex_map.pop();
                }
                break;
            }

            local_triangles.push(l0 as u8);
            local_triangles.push(l1 as u8);
            local_triangles.push(l2 as u8);
            meshlet_tri_count += 1;
            tri_idx += 1;
        }

        if meshlet_tri_count == 0 {
            break;
        }

        // Compute bounding sphere (simple: average center + max radius).
        let (center, radius) = compute_bounding_sphere(positions, &meshlet_vertex_map);

        let meshlet = Meshlet {
            vertex_offset: meshlet_vertex_map.first().copied().unwrap_or(0),
            vertex_count: meshlet_vertex_map.len() as u32,
            triangle_offset: tri_start,
            triangle_count: meshlet_tri_count,
            bounding_sphere: [center[0], center[1], center[2], radius],
            normal_cone: compute_normal_cone(positions, &meshlet_vertex_map, &local_triangles, meshlet_tri_count),
            lod_error: 0.0,
            parent_error: f32::MAX,
            group_id: meshlets.len() as u32,
            lod_level: 0,
        };

        meshlet_triangles.extend_from_slice(&local_triangles);
        meshlets.push(meshlet);
    }

    log::info!(
        "Built {} meshlets from {} triangles ({} vertices)",
        meshlets.len(),
        triangle_count,
        positions.len()
    );

    (meshlets, meshlet_triangles)
}

fn get_or_add_vertex(map: &mut Vec<u32>, global_idx: u32) -> u32 {
    if let Some(pos) = map.iter().position(|&v| v == global_idx) {
        pos as u32
    } else {
        let local = map.len() as u32;
        map.push(global_idx);
        local
    }
}

/// Compute the normal cone for a meshlet (axis + cos(half_angle)).
///
/// The normal cone is the tightest cone that contains all face normals
/// of the meshlet. If the camera view direction lies outside this cone,
/// the entire meshlet is backfacing and can be culled.
///
/// Returns `[axis.x, axis.y, axis.z, cos(half_angle)]`.
/// A `cos(half_angle)` of `-1.0` means the cone spans the full sphere
/// (no backface culling possible for this meshlet).
fn compute_normal_cone(
    positions: &[[f32; 3]],
    vertex_map: &[u32],
    local_triangles: &[u8],
    tri_count: u32,
) -> [f32; 4] {
    if tri_count == 0 {
        return [0.0, 1.0, 0.0, -1.0];
    }

    // Step 1: Compute all face normals and accumulate for average.
    let mut normals = Vec::with_capacity(tri_count as usize);
    let mut sum = [0.0f32; 3];

    for t in 0..tri_count as usize {
        let li0 = local_triangles[t * 3] as usize;
        let li1 = local_triangles[t * 3 + 1] as usize;
        let li2 = local_triangles[t * 3 + 2] as usize;

        if li0 >= vertex_map.len() || li1 >= vertex_map.len() || li2 >= vertex_map.len() {
            return [0.0, 1.0, 0.0, -1.0];
        }

        let p0 = positions[vertex_map[li0] as usize];
        let p1 = positions[vertex_map[li1] as usize];
        let p2 = positions[vertex_map[li2] as usize];

        // Edge vectors
        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        // Cross product = face normal (unnormalized)
        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let nz = e1[0] * e2[1] - e1[1] * e2[0];

        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len < 1e-12 {
            // Degenerate triangle (zero area) — disable cone culling.
            return [0.0, 1.0, 0.0, -1.0];
        }

        let inv = 1.0 / len;
        let n = [nx * inv, ny * inv, nz * inv];
        normals.push(n);
        sum[0] += n[0];
        sum[1] += n[1];
        sum[2] += n[2];
    }

    // Step 2: Average normal → cone axis (normalized).
    let axis_len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    if axis_len < 1e-12 {
        // Normals cancel out (e.g. opposing faces) — no valid cone.
        return [0.0, 1.0, 0.0, -1.0];
    }
    let inv = 1.0 / axis_len;
    let axis = [sum[0] * inv, sum[1] * inv, sum[2] * inv];

    // Step 3: Find the minimum dot product between each normal and the axis.
    // This gives cos(half_angle) of the tightest enclosing cone.
    let mut min_dot = 1.0f32;
    for n in &normals {
        let d = axis[0] * n[0] + axis[1] * n[1] + axis[2] * n[2];
        if d < min_dot {
            min_dot = d;
        }
    }

    // If cone spread exceeds hemisphere (cos < 0), normals spread > 90 degrees
    // from axis. Backface culling is unreliable in this case.
    if min_dot < 0.0 {
        return [axis[0], axis[1], axis[2], -1.0];
    }

    [axis[0], axis[1], axis[2], min_dot]
}

fn compute_bounding_sphere(positions: &[[f32; 3]], vertex_map: &[u32]) -> ([f32; 3], f32) {
    if vertex_map.is_empty() {
        return ([0.0; 3], 0.0);
    }

    // Average center.
    let mut center = [0.0f32; 3];
    for &idx in vertex_map {
        let p = positions[idx as usize];
        center[0] += p[0];
        center[1] += p[1];
        center[2] += p[2];
    }
    let n = vertex_map.len() as f32;
    center[0] /= n;
    center[1] /= n;
    center[2] /= n;

    // Max radius.
    let mut max_r2 = 0.0f32;
    for &idx in vertex_map {
        let p = positions[idx as usize];
        let dx = p[0] - center[0];
        let dy = p[1] - center[1];
        let dz = p[2] - center[2];
        max_r2 = max_r2.max(dx * dx + dy * dy + dz * dz);
    }

    (center, max_r2.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_cone_flat_quad() {
        // All faces point +Z → axis=(0,0,1), cos(half_angle)=1.0
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let vertex_map: Vec<u32> = vec![0, 1, 2, 3];
        let local_tris: Vec<u8> = vec![0, 1, 2, 0, 2, 3];
        let cone = compute_normal_cone(&positions, &vertex_map, &local_tris, 2);
        // Both triangles face +Z
        assert!((cone[2] - 1.0).abs() < 0.01, "axis.z should be ~1.0, got {}", cone[2]);
        assert!((cone[3] - 1.0).abs() < 0.01, "cos should be ~1.0, got {}", cone[3]);
    }

    #[test]
    fn test_normal_cone_opposing_faces() {
        // Two triangles facing opposite directions → should return cos=-1.0
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.5, 1.0, 0.0],
            [0.5, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let vertex_map: Vec<u32> = vec![0, 1, 2, 3, 4, 5];
        // First triangle: 0,1,2 → normal +Z; Second: wound opposite → normal -Z
        let local_tris: Vec<u8> = vec![0, 1, 2, 2, 1, 0];
        let cone = compute_normal_cone(&positions, &vertex_map, &local_tris, 2);
        assert_eq!(cone[3], -1.0, "opposing normals should give cos=-1.0");
    }

    #[test]
    fn test_normal_cone_degenerate_triangle() {
        // Degenerate triangle (zero area) → cos=-1.0
        let positions = vec![
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let vertex_map: Vec<u32> = vec![0, 1, 2];
        let local_tris: Vec<u8> = vec![0, 1, 2];
        let cone = compute_normal_cone(&positions, &vertex_map, &local_tris, 1);
        assert_eq!(cone[3], -1.0, "degenerate triangle should disable culling");
    }

    #[test]
    fn test_normal_cone_built_meshlets() {
        // Verify build_meshlets produces valid normal cones (not placeholders)
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let indices = vec![0, 1, 2, 0, 2, 3];
        let (meshlets, _) = build_meshlets(&positions, &indices);
        assert_eq!(meshlets.len(), 1);
        let cone = meshlets[0].normal_cone;
        // Should be a valid cone, not placeholder [0, 1, 0, -1]
        assert!(cone[3] > 0.0, "flat quad should have positive cos(half_angle)");
    }

    #[test]
    fn test_build_simple_meshlets() {
        // A simple quad (2 triangles, 4 vertices).
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let indices = vec![0, 1, 2, 0, 2, 3];

        let (meshlets, triangles) = build_meshlets(&positions, &indices);

        assert_eq!(meshlets.len(), 1);
        assert_eq!(meshlets[0].triangle_count, 2);
        assert_eq!(meshlets[0].vertex_count, 4);
        assert_eq!(triangles.len(), 6); // 2 triangles * 3 indices
    }
}
