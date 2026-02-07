// SKOPE Engine — V-Buffer Resolve (Nanite + Standard merge)
//
// Merges Nanite V-Buffer output (mesh shader HW + compute SW) with the
// standard V-Buffer from traditional rasterization. The closer fragment
// (by depth) wins for each pixel.
//
// Nanite visibility entries use a different triangle_id encoding:
//   cluster_id(20 bits) | triangle_id(7 bits) | material_id(5 bits)
// Standard V-Buffer triangle_id encoding:
//   mesh_index(16 bits) | primitive_index(16 bits)
//
// To distinguish them in the merged output, we set bit 31 for Nanite entries.

const NANITE_FLAG: u32 = 0x80000000u;

struct ResolveParams {
    width:  u32,
    height: u32,
    enable_nanite: u32,
    _pad:   u32,
};

// Bind group 0: Standard V-Buffer (read)
@group(0) @binding(0) var std_triangle_id: texture_2d<u32>;
@group(0) @binding(1) var std_barycentrics: texture_2d<f32>;
@group(0) @binding(2) var std_depth: texture_depth_2d;

// Bind group 1: Nanite V-Buffer (read)
@group(1) @binding(0) var nanite_triangle_id: texture_2d<u32>;
@group(1) @binding(1) var nanite_barycentrics: texture_2d<f32>;
@group(1) @binding(2) var nanite_depth: texture_depth_2d;

// Bind group 2: Output merged V-Buffer (write)
@group(2) @binding(0) var merged_triangle_id: texture_storage_2d<r32uint, write>;
@group(2) @binding(1) var merged_barycentrics: texture_storage_2d<rgba16float, write>;
@group(2) @binding(2) var merged_depth: texture_storage_2d<r32float, write>;
@group(2) @binding(3) var<uniform> params: ResolveParams;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));

    if gid.x >= params.width || gid.y >= params.height {
        return;
    }

    // Read standard V-Buffer
    let std_tid = textureLoad(std_triangle_id, pixel, 0).r;
    let std_bary = textureLoad(std_barycentrics, pixel, 0).rg;
    let std_d = textureLoad(std_depth, pixel, 0);

    if params.enable_nanite == 0u {
        // No Nanite — pass through standard
        textureStore(merged_triangle_id, pixel, vec4<u32>(std_tid, 0u, 0u, 0u));
        textureStore(merged_barycentrics, pixel, vec4<f32>(std_bary, 0.0, 0.0));
        textureStore(merged_depth, pixel, vec4<f32>(std_d, 0.0, 0.0, 0.0));
        return;
    }

    // Read Nanite V-Buffer
    let nanite_tid = textureLoad(nanite_triangle_id, pixel, 0).r;
    let nanite_bary = textureLoad(nanite_barycentrics, pixel, 0).rg;
    let nanite_d = textureLoad(nanite_depth, pixel, 0);

    // Standard uses 1.0 for clear (far), Nanite uses 0.0 for clear (reverse-Z far)
    // Compare: closer fragment wins
    // Standard: LESS compare (lower depth = closer)
    // Nanite: GREATER_EQUAL reverse-Z (higher depth = closer)

    // Normalize both to "lower depth = closer" for comparison
    let std_has_geometry = std_d < 1.0;
    let nanite_has_geometry = nanite_d > 0.0 && nanite_tid != 0u;

    if std_has_geometry && nanite_has_geometry {
        // Both have geometry — compare depths
        // Standard: lower = closer; Nanite reverse-Z: 1.0 - nanite_d to normalize
        // Actually, just compare directly: standard uses [0,1] where 0=near,
        // nanite reverse-Z uses [0,1] where 1=near. We compare in standard space.
        let nanite_linear = 1.0 - nanite_d;

        if std_d <= nanite_linear {
            // Standard is closer
            textureStore(merged_triangle_id, pixel, vec4<u32>(std_tid, 0u, 0u, 0u));
            textureStore(merged_barycentrics, pixel, vec4<f32>(std_bary, 0.0, 0.0));
            textureStore(merged_depth, pixel, vec4<f32>(std_d, 0.0, 0.0, 0.0));
        } else {
            // Nanite is closer — tag with NANITE_FLAG
            textureStore(merged_triangle_id, pixel, vec4<u32>(nanite_tid | NANITE_FLAG, 0u, 0u, 0u));
            textureStore(merged_barycentrics, pixel, vec4<f32>(nanite_bary, 0.0, 0.0));
            textureStore(merged_depth, pixel, vec4<f32>(nanite_linear, 0.0, 0.0, 0.0));
        }
    } else if nanite_has_geometry {
        // Only Nanite has geometry
        let nanite_linear = 1.0 - nanite_d;
        textureStore(merged_triangle_id, pixel, vec4<u32>(nanite_tid | NANITE_FLAG, 0u, 0u, 0u));
        textureStore(merged_barycentrics, pixel, vec4<f32>(nanite_bary, 0.0, 0.0));
        textureStore(merged_depth, pixel, vec4<f32>(nanite_linear, 0.0, 0.0, 0.0));
    } else {
        // Only standard (or no geometry)
        textureStore(merged_triangle_id, pixel, vec4<u32>(std_tid, 0u, 0u, 0u));
        textureStore(merged_barycentrics, pixel, vec4<f32>(std_bary, 0.0, 0.0));
        textureStore(merged_depth, pixel, vec4<f32>(std_d, 0.0, 0.0, 0.0));
    }
}
