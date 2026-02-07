// Lumen GI Composite Shader
//
// Applies filtered probe irradiance to the final HDR buffer.
// Reads the probe grid and interpolates between nearby probes
// for smooth per-pixel irradiance.
//
// Phase 1: no separate albedo GBuffer, so irradiance is added directly
// scaled by gi_intensity. Albedo modulation deferred to Phase 2 when
// a dedicated albedo GBuffer is available.

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

// Input: filtered irradiance grid
@group(1) @binding(0) var<storage, read> probe_irradiance: array<vec4<f32>>;

// Input/Output: HDR color buffer (read-write storage texture)
@group(2) @binding(0) var hdr_buffer: texture_storage_2d<rgba16float, read_write>;

// ── Main: one thread per pixel ─────────────────────────────────────

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= params.screen_width || py >= params.screen_height {
        return;
    }

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

    // Bilinear interpolation of probe irradiance
    let i00 = probe_irradiance[iy0 * params.probes_x + ix0].xyz;
    let i10 = probe_irradiance[iy0 * params.probes_x + ix1].xyz;
    let i01 = probe_irradiance[iy1 * params.probes_x + ix0].xyz;
    let i11 = probe_irradiance[iy1 * params.probes_x + ix1].xyz;

    let irradiance = mix(
        mix(i00, i10, fx),
        mix(i01, i11, fx),
        fy,
    );

    // Read current HDR color
    let current = textureLoad(hdr_buffer, vec2<i32>(i32(px), i32(py)));

    // Add GI contribution: irradiance / PI * intensity
    // Phase 1: no albedo modulation (no separate albedo GBuffer yet)
    let gi = irradiance * (1.0 / 3.14159265) * params.gi_intensity;
    let result = vec4<f32>(current.rgb + gi, current.a);

    textureStore(hdr_buffer, vec2<i32>(i32(px), i32(py)), result);
}
