// Lumen GI Composite Shader
//
// Applies filtered probe irradiance to the final HDR buffer.
// Reads the probe grid and interpolates between nearby probes
// with depth-aware rejection and albedo modulation for physically
// correct diffuse GI contribution.

// ── Structs ────────────────────────────────────────────────────────

struct CompositeParams {
    probe_spacing: u32,
    probes_x: u32,
    probes_y: u32,
    screen_width: u32,
    screen_height: u32,
    gi_intensity: f32,
    _pad0: u32,
    _pad1: u32,
}

// ── Bindings ───────────────────────────────────────────────────────

@group(0) @binding(0) var<uniform> params: CompositeParams;

// Input: filtered irradiance grid (vec4: irradiance.rgb, depth)
@group(1) @binding(0) var<storage, read> probe_irradiance: array<vec4<f32>>;

// Input/Output: HDR color buffer (read-write storage texture)
@group(2) @binding(0) var hdr_buffer: texture_storage_2d<rgba16float, read_write>;

// Input: depth buffer for depth-aware probe interpolation
@group(3) @binding(0) var depth_tex: texture_2d<f32>;

// Input: albedo G-Buffer for physically correct modulation (albedo.rgb, metallic)
@group(3) @binding(1) var albedo_tex: texture_2d<f32>;

// ── Helper: probe depth from irradiance W channel ───────────────

fn get_probe_depth(ix: u32, iy: u32) -> f32 {
    let idx = iy * params.probes_x + ix;
    return probe_irradiance[idx].w;
}

// ── Main: one thread per pixel ─────────────────────────────────────

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= params.screen_width || py >= params.screen_height {
        return;
    }

    // Read pixel depth — skip sky pixels
    let pixel_depth = textureLoad(depth_tex, vec2<i32>(i32(px), i32(py)), 0).r;
    if pixel_depth >= 1.0 {
        return;
    }

    // Read albedo and metallic from G-Buffer
    let albedo_sample = textureLoad(albedo_tex, vec2<i32>(i32(px), i32(py)), 0);
    let albedo = albedo_sample.rgb;
    let metallic = albedo_sample.a;

    // Compute probe grid coordinates (floating point for interpolation)
    let spacing = f32(params.probe_spacing);
    let probe_fx = (f32(px) + 0.5) / spacing - 0.5;
    let probe_fy = (f32(py) + 0.5) / spacing - 0.5;

    // Bilinear probe indices
    let ix0 = u32(max(floor(probe_fx), 0.0));
    let iy0 = u32(max(floor(probe_fy), 0.0));
    let ix1 = min(ix0 + 1u, params.probes_x - 1u);
    let iy1 = min(iy0 + 1u, params.probes_y - 1u);

    let fx = fract(probe_fx);
    let fy = fract(probe_fy);

    // Depth-aware bilinear interpolation
    // Reject probes whose depth differs significantly from the pixel depth
    let depth_threshold = 0.05; // 5% relative depth difference tolerance

    var total_irradiance = vec3<f32>(0.0);
    var total_weight = 0.0;

    // 4 corner probes with depth-aware weighting
    let bilinear_weights = array<f32, 4>(
        (1.0 - fx) * (1.0 - fy),
        fx * (1.0 - fy),
        (1.0 - fx) * fy,
        fx * fy,
    );
    let probe_ix = array<u32, 4>(ix0, ix1, ix0, ix1);
    let probe_iy = array<u32, 4>(iy0, iy0, iy1, iy1);

    for (var i = 0u; i < 4u; i++) {
        let pidx = probe_iy[i] * params.probes_x + probe_ix[i];
        let probe_data = probe_irradiance[pidx];
        let probe_depth = probe_data.w;
        let irr = probe_data.xyz;

        var w = bilinear_weights[i];

        // Depth-aware rejection: reduce weight for probes at very different depths
        if probe_depth > 0.0 {
            let depth_error = abs(probe_depth - pixel_depth) / max(pixel_depth, 0.001);
            if depth_error > depth_threshold {
                w *= max(1.0 - (depth_error - depth_threshold) * 10.0, 0.0);
            }
        }

        total_irradiance += irr * w;
        total_weight += w;
    }

    if total_weight <= 0.0 {
        return;
    }

    var irradiance = total_irradiance / total_weight;

    // Firefly clamp: prevent bright probe outliers from causing visible sparkle.
    // Limit irradiance luminance to a reasonable maximum.
    let irr_lum = dot(irradiance, vec3<f32>(0.2126, 0.7152, 0.0722));
    let max_irr_lum = 8.0; // Maximum allowed irradiance luminance
    if irr_lum > max_irr_lum {
        irradiance *= max_irr_lum / irr_lum;
    }

    // Read current HDR color
    let current = textureLoad(hdr_buffer, vec2<i32>(i32(px), i32(py)));

    // Physically correct GI: irradiance * albedo * (1 - metallic) / PI * intensity
    // Only diffuse surfaces receive GI contribution (metals reflect, no diffuse)
    let gi = irradiance * albedo * (1.0 - metallic) * (1.0 / 3.14159265) * params.gi_intensity;
    let result = vec4<f32>(current.rgb + gi, current.a);

    textureStore(hdr_buffer, vec2<i32>(i32(px), i32(py)), result);
}
