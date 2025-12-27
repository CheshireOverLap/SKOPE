// SKOPE Engine - Tonemapping Shader
// HDR → LDR 변환

struct TonemapParams {
    operator: u32,
    exposure: f32,
    white_point: f32,
    saturation_preserve: f32,
    gamma: f32,
    _pad: vec3<f32>,
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

    // HDR 씬 + 블룸
    var hdr_color = textureLoad(hdr_input, pixel, 0).rgb;
    let bloom = textureSampleLevel(bloom_tex, tex_sampler, uv, 0.0).rgb;
    hdr_color = hdr_color + bloom;

    // 노출 적용
    hdr_color = hdr_color * params.exposure;

    // 톤매핑
    var ldr_color: vec3<f32>;

    switch (params.operator) {
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
        default: {
            ldr_color = tonemap_aces(hdr_color);
        }
    }

    // 감마 보정
    ldr_color = pow(ldr_color, vec3<f32>(1.0 / params.gamma));

    textureStore(ldr_output, pixel, vec4<f32>(ldr_color, 1.0));
}
