// SKOPE Engine - Magic Circle SDF Shader
//
// Procedural magic circle rendering using Signed Distance Fields.
// Creates animated runes, rings, and glowing effects.

// ============================================================
// Structures
// ============================================================

struct CameraData {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    screen_size: vec2<f32>,
    time: f32,
    _pad: f32,
}

struct MagicCircleParams {
    position_scale: vec4<f32>,  // xyz: position, w: scale
    rotation: f32,
    time: f32,
    ring_count: u32,
    symbol_count: u32,
    color_primary: vec4<f32>,   // rgb: color, a: intensity
    color_secondary: vec4<f32>, // rgb: color, a: glow intensity
    anim_speeds: vec4<f32>,     // outer, inner, symbols, pulse
    rune_style: u32,
    opacity: f32,
    fade_distance: f32,
    _pad: f32,
}

struct MagicCircleInstance {
    model: mat4x4<f32>,
    params: MagicCircleParams,
}

struct InstanceCount {
    count: u32,
    _pad: vec3<u32>,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> camera: CameraData;
@group(0) @binding(1) var<storage, read> instances: array<MagicCircleInstance>;
@group(0) @binding(2) var noise_texture: texture_2d<f32>;
@group(0) @binding(3) var noise_sampler: sampler;
@group(0) @binding(4) var<uniform> instance_count: InstanceCount;

// ============================================================
// Constants
// ============================================================

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

// ============================================================
// SDF Primitives
// ============================================================

fn sd_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sd_ring(p: vec2<f32>, r: f32, width: f32) -> f32 {
    return abs(length(p) - r) - width;
}

fn sd_box(p: vec2<f32>, size: vec2<f32>) -> f32 {
    let d = abs(p) - size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_triangle(p: vec2<f32>, r: f32) -> f32 {
    let k = sqrt(3.0);
    var q = p;
    q.x = abs(q.x) - r;
    q.y = q.y + r / k;
    if (q.x + k * q.y > 0.0) {
        q = vec2<f32>(q.x - k * q.y, -k * q.x - q.y) / 2.0;
    }
    q.x -= clamp(q.x, -2.0 * r, 0.0);
    return -length(q) * sign(q.y);
}

fn sd_pentagon(p: vec2<f32>, r: f32) -> f32 {
    let k = vec3<f32>(0.809016994, 0.587785252, 0.726542528);
    var q = vec2<f32>(abs(p.x), p.y);
    q = q - 2.0 * min(dot(vec2<f32>(-k.x, k.y), q), 0.0) * vec2<f32>(-k.x, k.y);
    q = q - 2.0 * min(dot(vec2<f32>(k.x, k.y), q), 0.0) * vec2<f32>(k.x, k.y);
    q = q - vec2<f32>(clamp(q.x, -r * k.z, r * k.z), r);
    return length(q) * sign(q.y);
}

// ============================================================
// SDF Operations
// ============================================================

fn op_union(d1: f32, d2: f32) -> f32 {
    return min(d1, d2);
}

fn op_subtract(d1: f32, d2: f32) -> f32 {
    return max(d1, -d2);
}

fn op_intersect(d1: f32, d2: f32) -> f32 {
    return max(d1, d2);
}

fn op_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

fn op_round(d: f32, r: f32) -> f32 {
    return d - r;
}

fn rotate2d(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

// ============================================================
// Rune Symbols (Procedural)
// ============================================================

fn rune_runic(p: vec2<f32>, scale: f32, seed: u32) -> f32 {
    // Nordic-style angular runes
    var d = 999.0;
    let s = scale * 0.5;

    // Vertical line
    d = op_union(d, sd_segment(p, vec2<f32>(0.0, -s), vec2<f32>(0.0, s)));

    // Diagonal lines based on seed
    if ((seed & 1u) != 0u) {
        d = op_union(d, sd_segment(p, vec2<f32>(-s * 0.5, -s * 0.5), vec2<f32>(s * 0.5, 0.0)));
    }
    if ((seed & 2u) != 0u) {
        d = op_union(d, sd_segment(p, vec2<f32>(-s * 0.5, s * 0.5), vec2<f32>(s * 0.5, 0.0)));
    }
    if ((seed & 4u) != 0u) {
        d = op_union(d, sd_segment(p, vec2<f32>(0.0, 0.0), vec2<f32>(s * 0.7, -s * 0.7)));
    }

    return d - scale * 0.02;
}

fn rune_arcane(p: vec2<f32>, scale: f32, seed: u32) -> f32 {
    // Circular/curved arcane symbols
    var d = 999.0;
    let s = scale * 0.4;

    // Base circle
    d = op_union(d, sd_ring(p, s * 0.8, s * 0.02));

    // Inner shapes based on seed
    let inner_count = 3u + (seed % 4u);
    for (var i = 0u; i < inner_count; i++) {
        let angle = f32(i) * TWO_PI / f32(inner_count) + f32(seed) * 0.5;
        let offset = rotate2d(vec2<f32>(s * 0.4, 0.0), angle);
        d = op_union(d, sd_circle(p - offset, s * 0.1));
    }

    // Connecting lines
    if ((seed & 1u) != 0u) {
        d = op_union(d, sd_segment(p, vec2<f32>(-s, 0.0), vec2<f32>(s, 0.0)));
    }

    return d - scale * 0.015;
}

fn rune_demonic(p: vec2<f32>, scale: f32, seed: u32) -> f32 {
    // Sharp, aggressive demonic symbols
    var d = 999.0;
    let s = scale * 0.45;

    // Inverted triangle/pentagram base
    d = op_union(d, sd_triangle(p * vec2<f32>(1.0, -1.0), s * 0.9));

    // Spikes
    let spike_count = 3u + (seed % 3u);
    for (var i = 0u; i < spike_count; i++) {
        let angle = f32(i) * TWO_PI / f32(spike_count) + PI * 0.5;
        let dir = vec2<f32>(cos(angle), sin(angle));
        d = op_union(d, sd_segment(p, vec2<f32>(0.0), dir * s * 1.2));
    }

    return d - scale * 0.02;
}

fn rune_divine(p: vec2<f32>, scale: f32, seed: u32) -> f32 {
    // Soft, radiant divine symbols
    var d = 999.0;
    let s = scale * 0.4;

    // Central circle
    d = op_union(d, sd_circle(p, s * 0.3));

    // Radiating lines (star pattern)
    let ray_count = 4u + (seed % 4u) * 2u;
    for (var i = 0u; i < ray_count; i++) {
        let angle = f32(i) * TWO_PI / f32(ray_count);
        let dir = vec2<f32>(cos(angle), sin(angle));
        d = op_union(d, sd_segment(p, dir * s * 0.4, dir * s * 0.9));
    }

    // Outer ring
    d = op_union(d, sd_ring(p, s * 0.95, s * 0.03));

    return d - scale * 0.02;
}

fn draw_rune(p: vec2<f32>, scale: f32, style: u32, seed: u32) -> f32 {
    switch (style) {
        case 0u: { return rune_runic(p, scale, seed); }
        case 1u: { return rune_arcane(p, scale, seed); }
        case 2u: { return rune_demonic(p, scale, seed); }
        case 3u: { return rune_divine(p, scale, seed); }
        default: { return rune_arcane(p, scale, seed); }
    }
}

// ============================================================
// Magic Circle SDF
// ============================================================

fn magic_circle_sdf(uv: vec2<f32>, params: MagicCircleParams, time: f32) -> vec3<f32> {
    var total_d = 999.0;
    var glow_d = 999.0;

    let outer_rot = time * params.anim_speeds.x;
    let inner_rot = time * params.anim_speeds.y;
    let symbol_rot = time * params.anim_speeds.z;
    let pulse = sin(time * params.anim_speeds.w * TWO_PI) * 0.5 + 0.5;

    // Draw rings
    for (var ring = 0u; ring < params.ring_count; ring++) {
        let ring_radius = 0.3 + f32(ring) * 0.15;
        let ring_width = 0.01 + 0.005 * f32(ring);
        let ring_rot = select(inner_rot, outer_rot, ring % 2u == 0u);

        let rotated_uv = rotate2d(uv, ring_rot + f32(ring) * 0.5);

        // Main ring
        let ring_d = sd_ring(rotated_uv, ring_radius, ring_width);
        total_d = op_union(total_d, ring_d);
        glow_d = op_union(glow_d, ring_d);

        // Decorative segments on ring
        let seg_count = 8u + ring * 4u;
        for (var seg = 0u; seg < seg_count; seg++) {
            let seg_angle = f32(seg) * TWO_PI / f32(seg_count);
            let seg_uv = rotate2d(rotated_uv, seg_angle);

            // Small tick marks
            if (seg % 2u == 0u) {
                let tick_start = vec2<f32>(ring_radius - 0.02, 0.0);
                let tick_end = vec2<f32>(ring_radius + 0.02, 0.0);
                let tick_d = sd_segment(seg_uv, tick_start, tick_end) - 0.003;
                total_d = op_union(total_d, tick_d);
            }
        }
    }

    // Draw runes/symbols around outer ring
    let symbol_radius = 0.3 + f32(params.ring_count - 1u) * 0.15 + 0.08;
    for (var sym = 0u; sym < params.symbol_count; sym++) {
        let sym_angle = f32(sym) * TWO_PI / f32(params.symbol_count) + symbol_rot;
        let sym_pos = vec2<f32>(cos(sym_angle), sin(sym_angle)) * symbol_radius;

        let local_uv = uv - sym_pos;
        let rotated_local = rotate2d(local_uv, -sym_angle + PI * 0.5);

        let rune_d = draw_rune(rotated_local, 0.06, params.rune_style, sym + 12345u);
        total_d = op_union(total_d, rune_d);
        glow_d = op_union(glow_d, rune_d * 0.5);
    }

    // Central symbol
    let center_rot = time * params.anim_speeds.y * 0.5;
    let center_uv = rotate2d(uv, center_rot);
    let center_d = draw_rune(center_uv, 0.15, params.rune_style, 99999u);
    total_d = op_union(total_d, center_d);
    glow_d = op_union(glow_d, center_d * 0.3);

    // Inner decorative ring
    let inner_ring_d = sd_ring(rotate2d(uv, -inner_rot * 1.5), 0.12, 0.005);
    total_d = op_union(total_d, inner_ring_d);

    // Pulse effect on glow
    glow_d = glow_d * (0.8 + 0.2 * pulse);

    return vec3<f32>(total_d, glow_d, pulse);
}

// ============================================================
// Vertex Shader
// ============================================================

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) instance_id: u32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Quad vertices (triangle strip)
    let positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let local_pos = positions[vertex_idx];

    // Get instance data
    let instance = instances[instance_idx];
    let scale = instance.params.position_scale.w;

    // Create billboard facing up (XZ plane)
    let world_offset = vec3<f32>(local_pos.x * scale, 0.0, local_pos.y * scale);
    let world_pos = (instance.model * vec4<f32>(world_offset, 1.0)).xyz;

    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.uv = local_pos * 0.5 + 0.5;  // [0, 1] UV
    out.instance_id = instance_idx;

    return out;
}

// ============================================================
// Fragment Shader
// ============================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let instance = instances[in.instance_id];
    let params = instance.params;

    // UV centered at origin [-1, 1]
    let uv = (in.uv - 0.5) * 2.0;

    // Rotate UV
    let rotated_uv = rotate2d(uv, params.rotation + camera.time * params.anim_speeds.x * 0.1);

    // Get SDF values
    let sdf_result = magic_circle_sdf(rotated_uv, params, camera.time + params.time);
    let main_d = sdf_result.x;
    let glow_d = sdf_result.y;
    let pulse = sdf_result.z;

    // Calculate alpha from SDF
    let aa = fwidth(main_d) * 1.5;
    let main_alpha = 1.0 - smoothstep(-aa, aa, main_d);

    // Glow (larger, softer)
    let glow_strength = params.color_secondary.a;
    let glow_alpha = exp(-max(glow_d, 0.0) * 15.0 / glow_strength);

    // Sample noise for shimmer
    let noise_uv = rotated_uv * 2.0 + vec2<f32>(camera.time * 0.1, 0.0);
    let noise = textureSample(noise_texture, noise_sampler, noise_uv).r;
    let shimmer = 0.8 + 0.2 * noise;

    // Distance fade
    let dist_to_camera = length(in.world_pos - camera.camera_pos.xyz);
    let distance_fade = 1.0 - smoothstep(params.fade_distance * 0.5, params.fade_distance, dist_to_camera);

    // Circular edge fade
    let edge_dist = length(uv);
    let edge_fade = 1.0 - smoothstep(0.85, 1.0, edge_dist);

    // Combine colors
    let primary_color = params.color_primary.rgb * params.color_primary.a;
    let secondary_color = params.color_secondary.rgb * glow_strength;

    var final_color = primary_color * main_alpha * shimmer;
    final_color += secondary_color * glow_alpha * (0.5 + 0.5 * pulse);

    // Final alpha
    let final_alpha = max(main_alpha, glow_alpha * 0.5) * params.opacity * distance_fade * edge_fade;

    return vec4<f32>(final_color, final_alpha);
}
