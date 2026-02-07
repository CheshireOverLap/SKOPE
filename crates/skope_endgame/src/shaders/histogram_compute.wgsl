// SKOPE Engine - Histogram Compute Shader
// Bins luminance values into 256 buckets for auto exposure

struct AutoExposureParams {
    min_exposure: f32,
    max_exposure: f32,
    adaptation_speed: f32,
    exposure_compensation: f32,
    low_percentile: f32,
    high_percentile: f32,
    key_value: f32,
    enabled: u32,
}

@group(0) @binding(0) var hdr_input: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> histogram: array<atomic<u32>, 256>;
@group(0) @binding(2) var<uniform> params: AutoExposureParams;

// Convert RGB to luminance (Rec. 709)
fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Convert luminance to histogram bin index
// Maps log2(luminance) to [0, 255] range
fn luminance_to_bin(lum: f32) -> u32 {
    if (lum < 0.0001) {
        return 0u;
    }

    // Log2 range: typically -10 to +10 EV
    let log_lum = log2(lum);
    let min_log = -10.0;
    let max_log = 10.0;

    // Normalize to [0, 1]
    let normalized = (log_lum - min_log) / (max_log - min_log);

    // Convert to bin index [0, 255]
    let bin = u32(clamp(normalized * 255.0, 0.0, 255.0));
    return bin;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tex_size = textureDimensions(hdr_input);

    if (gid.x >= tex_size.x || gid.y >= tex_size.y) {
        return;
    }

    // Skip if auto exposure is disabled
    if (params.enabled == 0u) {
        return;
    }

    // Load HDR color
    let color = textureLoad(hdr_input, vec2<i32>(gid.xy), 0).rgb;

    // Calculate luminance
    let lum = luminance(color);

    // Get histogram bin
    let bin = luminance_to_bin(lum);

    // Atomically increment the bin counter
    atomicAdd(&histogram[bin], 1u);
}
