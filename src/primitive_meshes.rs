// Procedural primitive mesh generation (Cube, Sphere, Plane, etc.)

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
            normal: *norm,
            tangent: *tang,
            tex_coords: *uv,
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
    }
}

/// Generate a plane mesh (XZ plane, 1x1, centered at origin, facing +Y)
#[allow(dead_code)]
pub fn create_plane() -> Mesh {
    let vertices = vec![
        Vertex {
            position: [-0.5, 0.0, -0.5],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.0, -0.5],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.0, 0.5],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.0, 0.5],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [0.0, 1.0],
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
                normal: [nx, ny, nz],
                tangent: [tx, ty, tz, 1.0],
                tex_coords: [u, v],
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
    }
}

/// Generate a cylinder mesh (radius 0.5, height 1.0, centered at origin)
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

        // Bottom vertex
        vertices.push(Vertex {
            position: [cos_t * radius, -half_height, sin_t * radius],
            normal: [cos_t, 0.0, sin_t],
            tangent: [-sin_t, 0.0, cos_t, 1.0],
            tex_coords: [u, 0.0],
        });

        // Top vertex
        vertices.push(Vertex {
            position: [cos_t * radius, half_height, sin_t * radius],
            normal: [cos_t, 0.0, sin_t],
            tangent: [-sin_t, 0.0, cos_t, 1.0],
            tex_coords: [u, 1.0],
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

    // === Top cap ===
    let top_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, half_height, 0.0],
        normal: [0.0, 1.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
    });

    let top_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * radius, half_height, sin_t * radius],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
        });
    }

    // Top cap indices (fan)
    for i in 0..segments {
        indices.push(top_center_idx);
        indices.push(top_start + i);
        indices.push(top_start + i + 1);
    }

    // === Bottom cap ===
    let bottom_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [0.0, -half_height, 0.0],
        normal: [0.0, -1.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
        tex_coords: [0.5, 0.5],
    });

    let bottom_start = vertices.len() as u32;
    for i in 0..=segments {
        let u = i as f32 / segments as f32;
        let theta = u * std::f32::consts::TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        vertices.push(Vertex {
            position: [cos_t * radius, -half_height, sin_t * radius],
            normal: [0.0, -1.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            tex_coords: [(cos_t + 1.0) * 0.5, (sin_t + 1.0) * 0.5],
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
    }
}
