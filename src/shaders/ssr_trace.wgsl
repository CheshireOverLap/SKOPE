// SKOPE Engine - SSR Ray Trace Shader
//
// Hi-Z ray marching using Hierarchical Z-Buffer for efficient screen-space reflections.
// Reference: "Efficient GPU Screen-Space Ray Tracing" (JCGT 2014)

// ============================================================
// Structures
// ============================================================

struct SsrParams {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    max_distance: f32,
    thickness: f32,
    max_steps: u32,
    hzb_mip_count: u32,
    roughness_threshold: f32,
    temporal_weight: f32,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: SsrParams;
@group(0) @binding(1) var hzb_texture: texture_2d<f32>;
@group(0) @binding(2) var normal_roughness: texture_2d<f32>;
@group(0) @binding(3) var depth_texture: texture_2d<f32>;
@group(0) @binding(4) var point_sampler: sampler;
@group(0) @binding(5) var hit_output: texture_storage_2d<rgba32float, write>;

// ============================================================
// Helper Functions
// ============================================================

fn get_screen_uv(pixel: vec2<f32>) -> vec2<f32> {
    return (pixel + 0.5) / params.screen_size;
}

fn screen_to_world(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let clip = ndc;
    let clip_y_flipped = vec4<f32>(clip.x, -clip.y, clip.z, clip.w);
    let world_pos = params.inv_view_proj * clip_y_flipped;
    return world_pos.xyz / world_pos.w;
}

fn world_to_screen(world_pos: vec3<f32>) -> vec3<f32> {
    let clip = params.view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x, -ndc.y) * 0.5 + 0.5;
    return vec3<f32>(uv, ndc.z);
}

fn sample_depth(uv: vec2<f32>, mip: i32) -> f32 {
    let dims = vec2<f32>(textureDimensions(hzb_texture, mip));
    let texel = vec2<i32>(uv * dims);
    return textureLoad(hzb_texture, texel, mip).r;
}

fn decode_normal(encoded: vec2<f32>) -> vec3<f32> {
    let n = encoded * 2.0 - 1.0;
    let z = sqrt(max(0.0, 1.0 - n.x * n.x - n.y * n.y));
    return normalize(vec3<f32>(n.x, n.y, z));
}

// ============================================================
// Hi-Z Ray March
// ============================================================

struct RayHit {
    hit: bool,
    uv: vec2<f32>,
    depth: f32,
    pdf: f32,
}

fn hi_z_trace(
    ray_origin: vec3<f32>,
    ray_dir: vec3<f32>,
) -> RayHit {
    var result: RayHit;
    result.hit = false;
    result.uv = vec2<f32>(0.0);
    result.depth = 0.0;
    result.pdf = 0.0;

    // Project ray origin to screen
    let origin_screen = world_to_screen(ray_origin);

    // Check if origin is valid
    if (origin_screen.x < 0.0 || origin_screen.x > 1.0 ||
        origin_screen.y < 0.0 || origin_screen.y > 1.0) {
        return result;
    }

    // Project ray end point
    let ray_end = ray_origin + ray_dir * params.max_distance;
    let end_screen = world_to_screen(ray_end);

    // Screen-space ray direction
    var ray_screen = end_screen - origin_screen;
    let ray_length = length(ray_screen.xy * params.screen_size);

    if (ray_length < 1.0) {
        return result;
    }

    // Normalize for step
    ray_screen = ray_screen / ray_length;

    // Current position
    var pos = origin_screen;
    var mip = 0;
    let max_mip = i32(params.hzb_mip_count) - 1;

    // March through mip levels
    for (var i = 0u; i < params.max_steps; i++) {
        // Check bounds
        if (pos.x < 0.0 || pos.x > 1.0 || pos.y < 0.0 || pos.y > 1.0) {
            break;
        }

        // Sample depth at current mip level
        let sampled_depth = sample_depth(pos.xy, mip);

        // Compare depths
        let depth_diff = pos.z - sampled_depth;

        if (depth_diff > 0.0 && depth_diff < params.thickness) {
            // Hit! Refine position
            if (mip == 0) {
                result.hit = true;
                result.uv = pos.xy;
                result.depth = sampled_depth;
                result.pdf = 1.0;
                return result;
            } else {
                // Go down mip level for refinement
                mip = max(0, mip - 1);
            }
        } else if (depth_diff > params.thickness) {
            // Behind surface, go up mip level
            mip = min(max_mip, mip + 1);
        }

        // Step size based on mip level
        let step_size = pow(2.0, f32(mip));
        pos = pos + ray_screen * step_size;
    }

    return result;
}

// ============================================================
// Importance Sampling GGX
// ============================================================

fn importance_sample_ggx(xi: vec2<f32>, roughness: f32, n: vec3<f32>) -> vec3<f32> {
    let a = roughness * roughness;
    let a2 = a * a;

    let phi = 2.0 * 3.14159265359 * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a2 - 1.0) * xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    // Tangent space half vector
    let h = vec3<f32>(
        sin_theta * cos(phi),
        sin_theta * sin(phi),
        cos_theta
    );

    // Build TBN matrix
    var up = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(n.y) > 0.999) {
        up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);

    // Transform to world space
    return normalize(tangent * h.x + bitangent * h.y + n * h.z);
}

fn radical_inverse_vdc(bits: u32) -> f32 {
    var b = bits;
    b = (b << 16u) | (b >> 16u);
    b = ((b & 0x55555555u) << 1u) | ((b >> 1u) & 0x55555555u);
    b = ((b & 0x33333333u) << 2u) | ((b >> 2u) & 0x33333333u);
    b = ((b & 0x0F0F0F0Fu) << 4u) | ((b >> 4u) & 0x0F0F0F0Fu);
    b = ((b & 0x00FF00FFu) << 8u) | ((b >> 8u) & 0x00FF00FFu);
    return f32(b) * 2.3283064365386963e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<f32>(global_id.xy);

    // Check bounds
    if (pixel.x >= params.screen_size.x || pixel.y >= params.screen_size.y) {
        return;
    }

    let uv = get_screen_uv(pixel);
    let pixel_i = vec2<i32>(global_id.xy);

    // Sample depth
    let depth = textureLoad(depth_texture, pixel_i, 0).r;

    // Skip sky
    if (depth >= 1.0) {
        textureStore(hit_output, pixel_i, vec4<f32>(0.0, 0.0, -1.0, 0.0));
        return;
    }

    // Get normal and roughness
    let nr = textureLoad(normal_roughness, pixel_i, 0);
    let normal = decode_normal(nr.xy);
    let roughness = nr.z;

    // Skip rough surfaces
    if (roughness > params.roughness_threshold) {
        textureStore(hit_output, pixel_i, vec4<f32>(0.0, 0.0, -1.0, 0.0));
        return;
    }

    // Reconstruct world position
    let world_pos = screen_to_world(uv, depth);

    // Generate reflection ray
    let view_dir = normalize(world_pos); // Assumes camera at origin

    // Simple reflection for now (can add importance sampling for rough surfaces)
    let reflect_dir = reflect(view_dir, normal);

    // Trace ray
    let hit = hi_z_trace(world_pos + normal * 0.01, reflect_dir);

    if (hit.hit) {
        textureStore(hit_output, pixel_i, vec4<f32>(hit.uv, hit.depth, hit.pdf));
    } else {
        textureStore(hit_output, pixel_i, vec4<f32>(0.0, 0.0, -1.0, 0.0));
    }
}
