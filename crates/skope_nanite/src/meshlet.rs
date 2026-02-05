//! Meshlet (cluster) builder — converts a triangle mesh into meshlets.
//!
//! Each meshlet contains up to MAX_MESHLET_VERTICES vertices and
//! MAX_MESHLET_TRIANGLES triangles, grouped for spatial locality.

use crate::types::{Meshlet, MAX_MESHLET_VERTICES, MAX_MESHLET_TRIANGLES};

/// A complete Nanite mesh resource containing all LOD levels.
pub struct NaniteMesh {
    /// All meshlets across all LOD levels.
    pub meshlets: Vec<Meshlet>,
    /// Vertex data (application-defined vertex format).
    pub vertex_positions: Vec<[f32; 3]>,
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
            normal_cone: [0.0, 1.0, 0.0, -1.0], // TODO: compute proper normal cone
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
