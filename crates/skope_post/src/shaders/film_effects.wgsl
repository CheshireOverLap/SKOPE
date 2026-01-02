// SKOPE Engine - Film Effects Shader
// Film Grain, Vignette, Chromatic Aberration

struct FilmEffectsParams {
    grain_intensity: f32,
    grain_size: f32,
    grain_colored: f32,
    grain_luminance_linked: f32,

    vignette_intensity: f32,
    vignette_roundness: f32,
    vignette_smoothness: f32,
    _pad0: f32,

    vignette_color: vec3<f32>,
    _pad1: f32,

    chromatic_intensity: f32,
    time: f32,
    _pad2: vec2<f32>,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var<uniform> params: FilmEffectsParams;

// 노이즈 함수
fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn film_grain(uv: vec2<f32>, luminance: f32) -> vec3<f32> {
    let grain_uv = uv * params.grain_size + params.time * 0.1;

    // 기본 그레인
    var grain = hash(grain_uv) * 2.0 - 1.0;

    // 휘도 연동 (어두운 영역에 더 강하게)
    let luma_factor = mix(1.0, 1.0 - luminance, params.grain_luminance_linked);
    grain = grain * luma_factor;

    // 컬러 그레인
    var grain_color: vec3<f32>;
    if (params.grain_colored > 0.0) {
        let r = hash(grain_uv + vec2<f32>(0.0, 0.0)) * 2.0 - 1.0;
        let g = hash(grain_uv + vec2<f32>(1.0, 0.0)) * 2.0 - 1.0;
        let b = hash(grain_uv + vec2<f32>(0.0, 1.0)) * 2.0 - 1.0;
        grain_color = mix(vec3<f32>(grain), vec3<f32>(r, g, b), params.grain_colored);
    } else {
        grain_color = vec3<f32>(grain);
    }

    return grain_color * params.grain_intensity * luma_factor;
}

fn vignette(uv: vec2<f32>) -> f32 {
    let center = uv - 0.5;
    let dist = length(center * vec2<f32>(1.0, params.vignette_roundness));
    let vig = smoothstep(0.5, 0.5 - params.vignette_smoothness, dist);
    return mix(1.0, vig, params.vignette_intensity);
}

fn chromatic_aberration(uv: vec2<f32>, tex_size: vec2<f32>) -> vec3<f32> {
    if (params.chromatic_intensity < 0.001) {
        return textureSampleLevel(input_tex, tex_sampler, uv, 0.0).rgb;
    }

    let center = uv - 0.5;
    let dist = length(center);
    let offset = center * dist * params.chromatic_intensity * 0.01;

    let r = textureSampleLevel(input_tex, tex_sampler, uv + offset, 0.0).r;
    let g = textureSampleLevel(input_tex, tex_sampler, uv, 0.0).g;
    let b = textureSampleLevel(input_tex, tex_sampler, uv - offset, 0.0).b;

    return vec3<f32>(r, g, b);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = vec2<f32>(textureDimensions(input_tex));

    if (f32(pixel.x) >= tex_size.x || f32(pixel.y) >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / tex_size;

    // 색수차 (또는 일반 샘플링)
    var color = chromatic_aberration(uv, tex_size);

    // 그레인
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = color + film_grain(uv, luminance);

    // 비네트
    let vig = vignette(uv);
    color = mix(params.vignette_color, color, vig);

    textureStore(output_tex, pixel, vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
