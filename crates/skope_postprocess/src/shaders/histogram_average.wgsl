// SKOPE Engine - Histogram Average Shader
// Calculates weighted average luminance and target exposure

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

struct ExposureResult {
    current_exposure: f32,
    target_exposure: f32,
    average_luminance: f32,
    _pad: f32,
}

@group(0) @binding(0) var<storage, read> histogram: array<u32, 256>;
@group(0) @binding(1) var<storage, read_write> result: ExposureResult;
@group(0) @binding(2) var<uniform> params: AutoExposureParams;

// Convert bin index to luminance
fn bin_to_luminance(bin: u32) -> f32 {
    let min_log = -10.0;
    let max_log = 10.0;

    let normalized = f32(bin) / 255.0;
    let log_lum = normalized * (max_log - min_log) + min_log;
    return pow(2.0, log_lum);
}

// Single workgroup processes entire histogram
@compute @workgroup_size(256)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    // Only thread 0 does the work (could parallelize but histogram is small)
    if (lid.x != 0u) {
        return;
    }

    // Skip if disabled
    if (params.enabled == 0u) {
        result.current_exposure = 1.0;
        result.target_exposure = 1.0;
        result.average_luminance = 0.18;
        return;
    }

    // Count total pixels
    var total_count = 0u;
    for (var i = 0u; i < 256u; i = i + 1u) {
        total_count = total_count + histogram[i];
    }

    if (total_count == 0u) {
        return;
    }

    // Calculate percentile thresholds
    let low_threshold = u32(f32(total_count) * params.low_percentile);
    let high_threshold = u32(f32(total_count) * params.high_percentile);

    // Find valid range (skip low and high percentiles)
    var cumulative = 0u;
    var start_bin = 0u;
    var end_bin = 255u;

    // Find start bin (skip low percentile)
    for (var i = 0u; i < 256u; i = i + 1u) {
        cumulative = cumulative + histogram[i];
        if (cumulative >= low_threshold) {
            start_bin = i;
            break;
        }
    }

    // Find end bin (skip high percentile)
    cumulative = 0u;
    for (var i = 0u; i < 256u; i = i + 1u) {
        cumulative = cumulative + histogram[i];
        if (cumulative >= high_threshold) {
            end_bin = i;
            break;
        }
    }

    // Calculate weighted average luminance (log space)
    var weighted_sum = 0.0;
    var valid_count = 0u;

    for (var i = start_bin; i <= end_bin; i = i + 1u) {
        let count = histogram[i];
        if (count > 0u) {
            let lum = bin_to_luminance(i);
            // Weight by log luminance for perceptual averaging
            weighted_sum = weighted_sum + log2(max(lum, 0.0001)) * f32(count);
            valid_count = valid_count + count;
        }
    }

    // Average luminance
    var avg_luminance = 0.18;  // Default key value
    if (valid_count > 0u) {
        let avg_log_lum = weighted_sum / f32(valid_count);
        avg_luminance = pow(2.0, avg_log_lum);
    }

    // Calculate target exposure
    // E = key_value / average_luminance
    let target_exposure = params.key_value / max(avg_luminance, 0.0001);

    // Apply compensation and clamp
    let compensated = target_exposure * pow(2.0, params.exposure_compensation);
    let clamped_exposure = clamp(compensated, params.min_exposure, params.max_exposure);

    // Smooth temporal adaptation
    // dt is not available here, so we assume fixed rate
    let dt = 0.016;  // ~60fps
    let adaptation = 1.0 - exp(-params.adaptation_speed * dt);
    let new_exposure = mix(result.current_exposure, clamped_exposure, adaptation);

    // Store results
    result.target_exposure = clamped_exposure;
    result.current_exposure = new_exposure;
    result.average_luminance = avg_luminance;
}
