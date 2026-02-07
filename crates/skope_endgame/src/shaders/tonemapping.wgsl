// SKOPE Engine - Tonemapping Shader
// HDR → LDR 변환
// COD:AW Style - Multiple Tonemapping Options
//
// Operators:
// - Reinhard: Simple, preserves colors
// - ACES Fitted: Film-like, industry standard
// - Uncharted 2: Game-friendly, good for HDR
// - AgX: Blender-style, neutral
// - Hejl 2015: Fast, good for games

struct TonemapParams {
    tonemap_type: u32,  // "operator"는 WGSL 예약어
    exposure: f32,
    white_point: f32,
    saturation_preserve: f32,
    gamma: f32,
    bloom_intensity: f32,  // 0.0 = no bloom, 1.0 = full bloom
    _pad: vec2<f32>,
}

@group(0) @binding(0) var hdr_input: texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var ldr_output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> params: TonemapParams;

const REINHARD: u32 = 0u;
const ACES: u32 = 1u;
const UNCHARTED2: u32 = 2u;
const AGX: u32 = 3u;
const HEJL: u32 = 4u;        // Hejl 2015
const PASSTHROUGH: u32 = 99u;  // Debug mode - no tonemapping, no gamma

// Reinhard
fn tonemap_reinhard(color: vec3<f32>, white: f32) -> vec3<f32> {
    return color * (1.0 + color / (white * white)) / (1.0 + color);
}

// ACES Filmic (Stephen Hill's fit)
fn tonemap_aces(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Uncharted 2 (Hable)
fn uncharted2_partial(x: vec3<f32>) -> vec3<f32> {
    let A = 0.15;
    let B = 0.50;
    let C = 0.10;
    let D = 0.20;
    let E = 0.02;
    let F = 0.30;
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;
}

fn tonemap_uncharted2(color: vec3<f32>, white: f32) -> vec3<f32> {
    let curr = uncharted2_partial(color);
    let white_scale = vec3<f32>(1.0) / uncharted2_partial(vec3<f32>(white));
    return curr * white_scale;
}

// AgX (Simplified)
fn agx_default_contrast_approx(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

fn tonemap_agx(color: vec3<f32>) -> vec3<f32> {
    let agx_mat = mat3x3<f32>(
        vec3<f32>(0.842479, 0.0423738, 0.0423738),
        vec3<f32>(0.0784336, 0.878368, 0.0784336),
        vec3<f32>(0.0792237, 0.0791661, 0.879143)
    );

    var val = agx_mat * color;
    val = clamp(log2(val + 0.00001), vec3<f32>(-12.47393), vec3<f32>(4.026069));
    val = (val - vec3<f32>(-12.47393)) / (4.026069 + 12.47393);
    val = agx_default_contrast_approx(val);

    return val;
}

// Hejl 2015 (Jim Hejl, optimized for games)
// Fast and includes sRGB gamma approximation
fn tonemap_hejl(color: vec3<f32>) -> vec3<f32> {
    let a = color * max(vec3<f32>(0.0), color - vec3<f32>(0.004));
    let b = (a * (6.2 * a + 0.5)) / (a * (6.2 * a + 1.7) + 0.06);
    return b;
}

// Hejl-Burgess-Dawson (alternative, more accurate)
fn tonemap_hejl_bd(color: vec3<f32>, white_point: f32) -> vec3<f32> {
    let x = max(vec3<f32>(0.0), color - vec3<f32>(0.004));
    let w = max(0.0, white_point - 0.004);

    let mapped = (x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06);
    let white_mapped = (w * (6.2 * w + 0.5)) / (w * (6.2 * w + 1.7) + 0.06);

    return mapped / white_mapped;
}

// 채도 보존 (ACES 보정)
fn preserve_saturation(original: vec3<f32>, tonemapped: vec3<f32>, strength: f32) -> vec3<f32> {
    let orig_luma = dot(original, vec3<f32>(0.2126, 0.7152, 0.0722));
    let tone_luma = dot(tonemapped, vec3<f32>(0.2126, 0.7152, 0.0722));

    if (orig_luma < 0.0001 || tone_luma < 0.0001) {
        return tonemapped;
    }

    // 원본 채도 복원
    let orig_chroma = original / orig_luma;
    let restored = tonemapped + (orig_chroma - tonemapped / tone_luma) * tone_luma * strength;

    return clamp(restored, vec3<f32>(0.0), vec3<f32>(1.0));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(hdr_input);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(tex_size);

    // HDR input
    var hdr_color = textureLoad(hdr_input, pixel, 0).rgb;

    // Add bloom if enabled (bloom_intensity > 0)
    if (params.bloom_intensity > 0.0) {
        let bloom = textureSampleLevel(bloom_tex, tex_sampler, uv, 0.0).rgb;
        hdr_color = hdr_color + bloom * params.bloom_intensity;
    }

    // 노출 적용
    hdr_color = hdr_color * params.exposure;

    // 톤매핑
    var ldr_color: vec3<f32>;
    var apply_gamma = true;

    switch (params.tonemap_type) {
        case REINHARD: {
            ldr_color = tonemap_reinhard(hdr_color, params.white_point);
        }
        case ACES: {
            ldr_color = tonemap_aces(hdr_color);
            // 채도 보존
            ldr_color = preserve_saturation(hdr_color, ldr_color, params.saturation_preserve);
        }
        case UNCHARTED2: {
            ldr_color = tonemap_uncharted2(hdr_color, params.white_point);
        }
        case AGX: {
            ldr_color = tonemap_agx(hdr_color);
        }
        case HEJL: {
            // Hejl 2015 includes gamma approximation
            ldr_color = tonemap_hejl(hdr_color);
            apply_gamma = false;  // Hejl includes gamma
        }
        case PASSTHROUGH: {
            // Debug mode: no tonemapping, no gamma, just clamp
            ldr_color = clamp(hdr_color, vec3<f32>(0.0), vec3<f32>(1.0));
            apply_gamma = false;
        }
        default: {
            ldr_color = tonemap_aces(hdr_color);
        }
    }

    // 감마 보정 (패스스루 모드에선 건너뜀)
    if (apply_gamma) {
        ldr_color = pow(ldr_color, vec3<f32>(1.0 / params.gamma));
    }

    textureStore(ldr_output, pixel, vec4<f32>(ldr_color, 1.0));
}
