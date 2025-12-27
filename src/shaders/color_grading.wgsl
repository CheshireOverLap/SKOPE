// SKOPE Engine - Color Grading Shader
// LUT 및 수학적 색상 조정

struct ColorGradingParams {
    brightness: f32,
    contrast: f32,
    saturation: f32,
    hue_shift: f32,

    lift: vec3<f32>,
    _pad0: f32,
    gamma: vec3<f32>,
    _pad1: f32,
    gain: vec3<f32>,
    _pad2: f32,

    shadow_tint: vec3<f32>,
    shadow_tint_strength: f32,
    highlight_tint: vec3<f32>,
    highlight_tint_strength: f32,

    temperature: f32,
    tint: f32,

    lut_intensity: f32,
    lut_size: f32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var lut_tex: texture_3d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> params: ColorGradingParams;

// 색온도 변환 (Kelvin → RGB 멀티플라이어)
fn temperature_to_rgb(kelvin: f32) -> vec3<f32> {
    let temp = kelvin / 100.0;
    var color: vec3<f32>;

    // Red
    if (temp <= 66.0) {
        color.r = 1.0;
    } else {
        color.r = clamp(pow(temp - 60.0, -0.1332047592) * 329.698727446 / 255.0, 0.0, 1.0);
    }

    // Green
    if (temp <= 66.0) {
        color.g = clamp((log(temp) * 99.4708025861 - 161.1195681661) / 255.0, 0.0, 1.0);
    } else {
        color.g = clamp(pow(temp - 60.0, -0.0755148492) * 288.1221695283 / 255.0, 0.0, 1.0);
    }

    // Blue
    if (temp >= 66.0) {
        color.b = 1.0;
    } else if (temp <= 19.0) {
        color.b = 0.0;
    } else {
        color.b = clamp((log(temp - 10.0) * 138.5177312231 - 305.0447927307) / 255.0, 0.0, 1.0);
    }

    return color;
}

// HSV 변환
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(rgb.r, rgb.g), rgb.b);
    let min_c = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_c - min_c;

    var h = 0.0;
    var s = 0.0;
    if (max_c > 0.0) {
        s = delta / max_c;
    }
    let v = max_c;

    if (delta > 0.0) {
        if (max_c == rgb.r) {
            h = (rgb.g - rgb.b) / delta;
        } else if (max_c == rgb.g) {
            h = 2.0 + (rgb.b - rgb.r) / delta;
        } else {
            h = 4.0 + (rgb.r - rgb.g) / delta;
        }
        h = h / 6.0;
        if (h < 0.0) { h = h + 1.0; }
    }

    return vec3<f32>(h, s, v);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let idx = i32(i) % 6;
    if (idx == 0) { return vec3<f32>(v, t, p); }
    if (idx == 1) { return vec3<f32>(q, v, p); }
    if (idx == 2) { return vec3<f32>(p, v, t); }
    if (idx == 3) { return vec3<f32>(p, q, v); }
    if (idx == 4) { return vec3<f32>(t, p, v); }
    return vec3<f32>(v, p, q);
}

// Lift/Gamma/Gain
fn apply_lgg(color: vec3<f32>, lift: vec3<f32>, gamma: vec3<f32>, gain: vec3<f32>) -> vec3<f32> {
    // Lift: 그림자 조정
    var result = color * (vec3<f32>(1.0) - lift) + lift;

    // Gamma: 중간톤 조정 (0 방지)
    let safe_gamma = max(gamma, vec3<f32>(0.001));
    result = pow(max(result, vec3<f32>(0.0)), vec3<f32>(1.0) / safe_gamma);

    // Gain: 하이라이트 조정
    result = result * gain;

    return result;
}

// Split Toning
fn apply_split_toning(
    color: vec3<f32>,
    shadow_tint: vec3<f32>,
    shadow_strength: f32,
    highlight_tint: vec3<f32>,
    highlight_strength: f32
) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));

    // 그림자/하이라이트 마스크
    let shadow_mask = 1.0 - smoothstep(0.0, 0.5, luma);
    let highlight_mask = smoothstep(0.5, 1.0, luma);

    var result = color;
    result = mix(result, result * shadow_tint, shadow_mask * shadow_strength);
    result = mix(result, result * highlight_tint, highlight_mask * highlight_strength);

    return result;
}

// LUT 샘플링
fn sample_lut(color: vec3<f32>, lut_size: f32) -> vec3<f32> {
    let scale = (lut_size - 1.0) / lut_size;
    let offset = 0.5 / lut_size;
    let uvw = color * scale + offset;

    return textureSampleLevel(lut_tex, tex_sampler, uvw, 0.0).rgb;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(input_tex);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    var color = textureLoad(input_tex, pixel, 0).rgb;

    // 1. 색온도
    let temp_rgb = temperature_to_rgb(params.temperature);
    let base_rgb = temperature_to_rgb(6500.0);
    color = color * (temp_rgb / base_rgb);

    // 2. 밝기
    color = color * params.brightness;

    // 3. 대비 (중간 회색 기준)
    color = (color - 0.5) * params.contrast + 0.5;

    // 4. Lift/Gamma/Gain
    color = apply_lgg(color, params.lift, params.gamma, params.gain);

    // 5. Split Toning
    color = apply_split_toning(
        color,
        params.shadow_tint, params.shadow_tint_strength,
        params.highlight_tint, params.highlight_tint_strength
    );

    // 6. 채도 & Hue
    var hsv = rgb_to_hsv(max(color, vec3<f32>(0.0)));
    hsv.x = fract(hsv.x + params.hue_shift / 360.0);
    hsv.y = hsv.y * params.saturation;
    color = hsv_to_rgb(hsv);

    // 7. LUT 적용
    if (params.lut_intensity > 0.0) {
        let lut_color = sample_lut(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), params.lut_size);
        color = mix(color, lut_color, params.lut_intensity);
    }

    textureStore(output_tex, pixel, vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
