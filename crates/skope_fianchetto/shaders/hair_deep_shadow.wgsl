// SKOPE Engine — Deep Shadow Maps for Hair
//
// Deep Shadow Maps (DSM) store opacity as a function of depth,
// allowing soft, volumetric shadows through semi-transparent hair.
// Each pixel stores multiple depth-opacity samples that are
// linearly interpolated during shadow lookup.
//
// Reference: UE5 HairStrandsDeepShadow.usf, Pixar Deep Shadow Maps (2000)

struct DeepShadowParams {
    light_view_proj: mat4x4<f32>,
    light_direction: vec3<f32>,
    shadow_bias:     f32,
    atlas_offset:    vec2<u32>,   // Offset into shadow atlas
    atlas_size:      u32,         // Size of this DSM tile
    max_layers:      u32,         // Max depth-opacity layers
    hair_opacity:    f32,         // Per-strand opacity contribution
    softness:        f32,         // Penumbra softness
    _pad:            vec2<f32>,
};

struct DeepShadowLayer {
    depth:   f32,
    opacity: f32,
};

@group(0) @binding(0) var<uniform> params: DeepShadowParams;
@group(0) @binding(1) var<storage, read> strand_vertices: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dsm_layers: array<DeepShadowLayer>;
@group(0) @binding(3) var<storage, read_write> dsm_count: array<atomic<u32>>;

// Maximum layers per pixel
const MAX_LAYERS_PER_PIXEL: u32 = 32u;

fn pixel_index(pixel: vec2<u32>) -> u32 {
    return pixel.y * params.atlas_size + pixel.x;
}

fn layer_base(pixel: vec2<u32>) -> u32 {
    return pixel_index(pixel) * MAX_LAYERS_PER_PIXEL;
}

// Insert a depth-opacity sample into the DSM for a pixel
fn insert_sample(pixel: vec2<u32>, depth: f32, opacity: f32) {
    let idx = pixel_index(pixel);
    let count = atomicAdd(&dsm_count[idx], 1u);

    if count < MAX_LAYERS_PER_PIXEL {
        let base = layer_base(pixel);
        dsm_layers[base + count] = DeepShadowLayer(depth, opacity);
    }
}

@compute @workgroup_size(64)
fn build_dsm(@builtin(global_invocation_id) gid: vec3<u32>) {
    let strand_idx = gid.x;
    let vertex = strand_vertices[strand_idx];
    let world_pos = vertex.xyz;
    let thickness = vertex.w;

    if thickness <= 0.0 {
        return;
    }

    // Project to light space
    let clip = params.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;

    // NDC to pixel coordinates
    let uv = ndc.xy * 0.5 + 0.5;
    let pixel = vec2<u32>(
        u32(uv.x * f32(params.atlas_size)),
        u32((1.0 - uv.y) * f32(params.atlas_size)),
    );

    // Bounds check
    if pixel.x >= params.atlas_size || pixel.y >= params.atlas_size {
        return;
    }

    // Depth with bias
    let depth = ndc.z + params.shadow_bias;

    // Opacity based on hair thickness and parameter
    let opacity = params.hair_opacity * clamp(thickness * 2.0, 0.0, 1.0);

    insert_sample(pixel, depth, opacity);
}

// Lookup shadow from DSM (called from hair shading)
// Returns visibility [0, 1]
fn lookup_dsm(pixel: vec2<u32>, query_depth: f32) -> f32 {
    let idx = pixel_index(pixel);
    let count = min(atomicLoad(&dsm_count[idx]), MAX_LAYERS_PER_PIXEL);

    if count == 0u {
        return 1.0;
    }

    // Accumulate opacity from all layers in front of query depth
    var total_opacity = 0.0;
    let base = layer_base(pixel);

    for (var i = 0u; i < count; i = i + 1u) {
        let layer = dsm_layers[base + i];
        if layer.depth < query_depth {
            total_opacity += layer.opacity;
        }
    }

    // Exponential transmittance
    return exp(-total_opacity);
}
