// Procedural primitive mesh generation (Cube, Sphere, Plane, etc.)

#![allow(clippy::type_complexity)]

use crate::gltf_loader::{Mesh, Vertex};

/// Generate a unit cube mesh (1x1x1, centered at origin)
pub fn create_cube() -> Mesh {
    let mut vertices = Vec::new();

    // Cube vertices (24 vertices, 4 per face for proper normals)
    #[rustfmt::skip]
    let vertex_data: &[([f32; 3], [f32; 3], [f32; 2], [f32; 4])] = &[
        // Front face (z = 0.5)
        ([-0.5, -0.5,  0.5], [0.0, 0.0, 1.0], [0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([ 0.5, -0.5,  0.5], [0.0, 0.0, 1.0], [1.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([ 0.5,  0.5,  0.5], [0.0, 0.0, 1.0], [1.0, 1.0], [1.0, 0.0, 0.0, 1.0]),
        ([-0.5,  0.5,  0.5], [0.0, 0.0, 1.0], [0.0, 1.0], [1.0, 0.0, 0.0, 1.0]),

        // Back face (z = -0.5)
        ([ 0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 0.0], [-1.0, 0.0, 0.0, 1.0]),
        ([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 0.0], [-1.0, 0.0, 0.0, 1.0]),
        ([-0.5,  0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 1.0], [-1.0, 0.0, 0.0, 1.0]),
        ([ 0.5,  0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 1.0], [-1.0, 0.0, 0.0, 1.0]),

        // Top face (y = 0.5)
        ([-0.5,  0.5,  0.5], [0.0, 1.0, 0.0], [0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([ 0.5,  0.5,  0.5], [0.0, 1.0, 0.0], [1.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([ 0.5,  0.5, -0.5], [0.0, 1.0, 0.0], [1.0, 1.0], [1.0, 0.0, 0.0, 1.0]),
        ([-0.5,  0.5, -0.5], [0.0, 1.0, 0.0], [0.0, 1.0], [1.0, 0.0, 0.0, 1.0]),

        // Bottom face (y = -0.5)
        ([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], [0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([ 0.5, -0.5, -0.5], [0.0, -1.0, 0.0], [1.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ([ 0.5, -0.5,  0.5], [0.0, -1.0, 0.0], [1.0, 1.0], [1.0, 0.0, 0.0, 1.0]),
        ([-0.5, -0.5,  0.5], [0.0, -1.0, 0.0], [0.0, 1.0], [1.0, 0.0, 0.0, 1.0]),

        // Right face (x = 0.5)
        ([ 0.5, -0.5,  0.5], [1.0, 0.0, 0.0], [0.0, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ([ 0.5, -0.5, -0.5], [1.0, 0.0, 0.0], [1.0, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ([ 0.5,  0.5, -0.5], [1.0, 0.0, 0.0], [1.0, 1.0], [0.0, 0.0, 1.0, 1.0]),
        ([ 0.5,  0.5,  0.5], [1.0, 0.0, 0.0], [0.0, 1.0], [0.0, 0.0, 1.0, 1.0]),

        // Left face (x = -0.5)
        ([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], [0.0, 0.0], [0.0, 0.0, -1.0, 1.0]),
        ([-0.5, -0.5,  0.5], [-1.0, 0.0, 0.0], [1.0, 0.0], [0.0, 0.0, -1.0, 1.0]),
        ([-0.5,  0.5,  0.5], [-1.0, 0.0, 0.0], [1.0, 1.0], [0.0, 0.0, -1.0, 1.0]),
        ([-0.5,  0.5, -0.5], [-1.0, 0.0, 0.0], [0.0, 1.0], [0.0, 0.0, -1.0, 1.0]),
    ];

    for (pos, norm, uv, tang) in vertex_data {
        vertices.push(Vertex {
            position: *pos,
            _pad1: 0.0,
            normal: *norm,
            _pad2: 0.0,
            tangent: *tang,
            tex_coords: *uv,
            _pad3: [0.0, 0.0],
        });
    }

    // Indices (6 faces * 2 triangles * 3 vertices = 36 indices)
    #[rustfmt::skip]
    let indices: Vec<u32> = vec![
        // Front
        0, 1, 2,  2, 3, 0,
        // Back
        4, 5, 6,  6, 7, 4,
        // Top
        8, 9, 10,  10, 11, 8,
        // Bottom
        12, 13, 14,  14, 15, 12,
        // Right
        16, 17, 18,  18, 19, 16,
        // Left
        20, 21, 22,  22, 23, 20,
    ];

    Mesh {
        vertices,
        indices,
        material_index: None,  // Will use default material
        morph_targets: None,
    }
}

/// Generate a plane mesh (XY plane, 1x1, centered at origin, facing +Z)
/// Z-up coordinate system: plane lies flat on XY, normal points up (+Z)
pub fn create_plane() -> Mesh {
    let vertices = vec![
        Vertex {
            position: [-0.5, -0.5, 0.0],
            _pad1: 0.0,
            normal: [0.0, 0.0, 1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [0.0, 0.0],
            _pad3: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.0],
            _pad1: 0.0,
            normal: [0.0, 0.0, 1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [1.0, 0.0],
            _pad3: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.5, 0.0],
            _pad1: 0.0,
            normal: [0.0, 0.0, 1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [1.0, 1.0],
            _pad3: [0.0, 0.0],
        },
        Vertex {
            position: [-0.5, 0.5, 0.0],
            _pad1: 0.0,
            normal: [0.0, 0.0, 1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [0.0, 1.0],
            _pad3: [0.0, 0.0],
        },
    ];

    let indices = vec![
        0, 1, 2,
        2, 3, 0,
    ];

    Mesh {
        vertices,
        indices,
        material_index: None,
        morph_targets: None,
    }
}

/// Generate a UV sphere mesh (radius 0.5, centered at origin)
pub fn create_sphere(segments: u32, rings: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let phi = v * std::f32::consts::PI; // 0 to PI (top to bottom)

        for seg in 0..=segments {
            let u = seg as f32 / segments as f32;
            let theta = u * std::f32::consts::TAU; // 0 to 2PI

            // Position on unit sphere, scaled to 0.5 radius
            let x = phi.sin() * theta.cos() * 0.5;
            let y = phi.cos() * 0.5;
            let z = phi.sin() * theta.sin() * 0.5;

            // Normal is just the normalized position (for unit sphere at origin)
            let nx = phi.sin() * theta.cos();
            let ny = phi.cos();
            let nz = phi.sin() * theta.sin();

            // Tangent (along theta direction)
            let tx = -theta.sin();
            let ty = 0.0;
            let tz = theta.cos();

            vertices.push(Vertex {
                position: [x, y, z],
                _pad1: 0.0,
                normal: [nx, ny, nz],
                _pad2: 0.0,
                tangent: [tx, ty, tz, 1.0],
                tex_coords: [u, v],
                _pad3: [0.0, 0.0],
            });
        }
    }

    // Generate indices
    for ring in 0..rings {
        for seg in 0..segments {
            let curr_row = ring * (segments + 1);
            let next_row = (ring + 1) * (segments + 1);

            // Two triangles per quad
            indices.push(curr_row + seg);
            indices.push(next_row + seg);
            indices.push(next_row + seg + 1);

            indices.push(curr_row + seg);
            indices.push(next_row + seg + 1);
            indices.push(curr_row + seg + 1);
        }
    }

    Mesh {
        vertices,
        indices,
        material_index: None,
        morph_targets: None,
    }
}

/// Generate a cylinder mesh (radius 0.5, height 1.0, centered at origin)
/// Z-up coordinate system: height along Z axis
pub fn create_cylinder(segments: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let radius = 0.5;
    let half_height = 0.5;

    // === Side vertices ===
    let side_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        // Bottom vertex (Z = -half_height)
        vertices.push(Vertex {
            position: [cos_t * radius, sin_t * radius, -half_height],
            _pad1: 0.0,
            normal: [cos_t, sin_t, 0.0],
            _pad2: 0.0,
            tangent: [-sin_t, cos_t, 0.0, 1.0],
            tex_coords: [u, 0.0],
            _pad3: [0.0, 0.0],
        });

        // Top vertex (Z = +half_height)
        vertices.push(Vertex {
            position: [cos_t * radius, sin_t * radius, half_height],
            _pad1: 0.0,
            normal: [cos_t, sin_t, 0.0],
            _pad2: 0.0,
            tangent: [-sin_t, cos_t, 0.0, 1.0],
            tex_coords: [u, 1.0],
            _pad3: [0.0, 0.0],
        });
    }

    // Side indices
    for i in 0..segments {
        let base = side_start + i * 2;
        // Two triangles per segment
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 3);

        indices.push(base);
        indices.push(base + 3);
        indices.push(base + 2);
    }

    // === Top cap (Z = +half_height, normal +Z) ===
    let top_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, half_height],
        _pad1: 0.0,
        normal: [0.0, 0.0, 1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
        _pad3: [0.0, 0.0],
    });

    let top_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * radius, sin_t * radius, half_height],
            _pad1: 0.0,
            normal: [0.0, 0.0, 1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
            _pad3: [0.0, 0.0],
        });
    }

    // Top cap indices (fan)
    for i in 0..segments {
        indices.push(top_center_idx);
        indices.push(top_start + i);
        indices.push(top_start + i + 1);
    }

    // === Bottom cap (Z = -half_height, normal -Z) ===
    let bottom_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, -half_height],
        _pad1: 0.0,
        normal: [0.0, 0.0, -1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
        _pad3: [0.0, 0.0],
    });

    let bottom_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * radius, sin_t * radius, -half_height],
            _pad1: 0.0,
            normal: [0.0, 0.0, -1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
            _pad3: [0.0, 0.0],
        });
    }

    // Bottom cap indices (fan, reversed winding)
    for i in 0..segments {
        indices.push(bottom_center_idx);
        indices.push(bottom_start + i + 1);
        indices.push(bottom_start + i);
    }

    Mesh {
        vertices,
        indices,
        material_index: None,
        morph_targets: None,
    }
}

/// Generate a cone mesh (radius 0.5, height 1.0, tip at top, centered at origin)
/// Z-up: base at z=-0.5, tip at z=0.5
pub fn create_cone(segments: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let radius = 0.5;
    let half_height = 0.5;

    // === Tip vertex (top) ===
    let tip_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, half_height],
        _pad1: 0.0,
        normal: [0.0, 0.0, 1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 1.0],
        _pad3: [0.0, 0.0],
    });

    // === Side vertices ===
    let slope = radius / (2.0 * half_height);
    let normal_z = slope / (1.0 + slope * slope).sqrt();
    let normal_xy = 1.0 / (1.0 + slope * slope).sqrt();

    let side_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * radius, sin_t * radius, -half_height],
            _pad1: 0.0,
            normal: [cos_t * normal_xy, sin_t * normal_xy, normal_z],
            _pad2: 0.0,
            tangent: [-sin_t, cos_t, 0.0, 1.0],
            tex_coords: [u, 0.0],
            _pad3: [0.0, 0.0],
        });
    }

    // Side indices (fan from tip)
    for i in 0..segments {
        indices.push(tip_idx);
        indices.push(side_start + i + 1);
        indices.push(side_start + i);
    }

    // === Bottom cap ===
    let bottom_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, -half_height],
        _pad1: 0.0,
        normal: [0.0, 0.0, -1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
        _pad3: [0.0, 0.0],
    });

    let bottom_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * radius, sin_t * radius, -half_height],
            _pad1: 0.0,
            normal: [0.0, 0.0, -1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
            _pad3: [0.0, 0.0],
        });
    }

    // Bottom cap indices
    for i in 0..segments {
        indices.push(bottom_center_idx);
        indices.push(bottom_start + i);
        indices.push(bottom_start + i + 1);
    }

    Mesh {
        vertices,
        indices,
        material_index: None,
        morph_targets: None,
    }
}

/// Generate an arrow mesh (shaft + head) pointing up (+Z)
/// Total height ~1.5, shaft radius 0.1, head radius 0.3
pub fn create_arrow() -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let shaft_radius = 0.1;
    let shaft_height = 1.0;
    let head_radius = 0.3;
    let head_height = 0.5;
    let segments = 12u32;

    let shaft_bottom = 0.0;
    let shaft_top = shaft_height;

    // === Shaft side ===
    let shaft_side_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        // Bottom
        vertices.push(Vertex {
            position: [cos_t * shaft_radius, sin_t * shaft_radius, shaft_bottom],
            _pad1: 0.0,
            normal: [cos_t, sin_t, 0.0],
            _pad2: 0.0,
            tangent: [-sin_t, cos_t, 0.0, 1.0],
            tex_coords: [u, 0.0],
            _pad3: [0.0, 0.0],
        });

        // Top
        vertices.push(Vertex {
            position: [cos_t * shaft_radius, sin_t * shaft_radius, shaft_top],
            _pad1: 0.0,
            normal: [cos_t, sin_t, 0.0],
            _pad2: 0.0,
            tangent: [-sin_t, cos_t, 0.0, 1.0],
            tex_coords: [u, 1.0],
            _pad3: [0.0, 0.0],
        });
    }

    for i in 0..segments {
        let base = shaft_side_start + i * 2;
        indices.push(base);
        indices.push(base + 2);
        indices.push(base + 3);
        indices.push(base);
        indices.push(base + 3);
        indices.push(base + 1);
    }

    // === Arrow Head ===
    let head_bottom = shaft_top;
    let head_tip = shaft_top + head_height;

    let tip_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, head_tip],
        _pad1: 0.0,
        normal: [0.0, 0.0, 1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 1.0],
        _pad3: [0.0, 0.0],
    });

    let slope = head_radius / head_height;
    let normal_z = slope / (1.0 + slope * slope).sqrt();
    let normal_xy = 1.0 / (1.0 + slope * slope).sqrt();

    let head_side_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * head_radius, sin_t * head_radius, head_bottom],
            _pad1: 0.0,
            normal: [cos_t * normal_xy, sin_t * normal_xy, normal_z],
            _pad2: 0.0,
            tangent: [-sin_t, cos_t, 0.0, 1.0],
            tex_coords: [u, 0.0],
            _pad3: [0.0, 0.0],
        });
    }

    for i in 0..segments {
        indices.push(tip_idx);
        indices.push(head_side_start + i + 1);
        indices.push(head_side_start + i);
    }

    // Head bottom cap
    let head_cap_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, head_bottom],
        _pad1: 0.0,
        normal: [0.0, 0.0, -1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
        _pad3: [0.0, 0.0],
    });

    let head_cap_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * head_radius, sin_t * head_radius, head_bottom],
            _pad1: 0.0,
            normal: [0.0, 0.0, -1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
            _pad3: [0.0, 0.0],
        });
    }

    for i in 0..segments {
        indices.push(head_cap_center_idx);
        indices.push(head_cap_start + i);
        indices.push(head_cap_start + i + 1);
    }

    // Shaft bottom cap
    let shaft_cap_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, 0.0, shaft_bottom],
        _pad1: 0.0,
        normal: [0.0, 0.0, -1.0],
        _pad2: 0.0,
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
        _pad3: [0.0, 0.0],
    });

    let shaft_cap_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * shaft_radius, sin_t * shaft_radius, shaft_bottom],
            _pad1: 0.0,
            normal: [0.0, 0.0, -1.0],
            _pad2: 0.0,
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
            _pad3: [0.0, 0.0],
        });
    }

    for i in 0..segments {
        indices.push(shaft_cap_center_idx);
        indices.push(shaft_cap_start + i);
        indices.push(shaft_cap_start + i + 1);
    }

    Mesh {
        vertices,
        indices,
        material_index: None,
        morph_targets: None,
    }
}
