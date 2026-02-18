// SKOPE Engine - Depth of Field Shader

struct DOFParams {
    focus_distance: f32,
    focus_range: f32,
    max_blur_size: f32,
    bokeh_intensity: f32,
    aperture_shape: u32,
    aperture_blades: u32,
    near_plane: f32,
    far_plane: f32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> params: DOFParams;

// Depth를 선형 거리로 변환
fn linearize_depth(depth: f32, near: f32, far: f32) -> f32 {
    return near * far / (far - depth * (far - near));
}

// Circle of Confusion 계산
fn calculate_coc(depth: f32) -> f32 {
    let linear_depth = linearize_depth(depth, params.near_plane, params.far_plane);
    let dist_from_focus = abs(linear_depth - params.focus_distance);
    let coc = smoothstep(0.0, params.focus_range, dist_from_focus);
    return coc * params.max_blur_size;
}

// Disk 블러 샘플링
fn disk_blur(uv: vec2<f32>, coc: f32, tex_size: vec2<f32>) -> vec3<f32> {
    if (coc < 0.5) {
        return textureSampleLevel(input_tex, tex_sampler, uv, 0.0).rgb;
    }

    let sample_count = 16;
    let golden_angle = 2.39996323;

    var color = vec3<f32>(0.0);
    var total_weight = 0.0;

    for (var i = 0; i < sample_count; i++) {
        let r = sqrt(f32(i) / f32(sample_count)) * coc;
        let theta = f32(i) * golden_angle;

        let offset = vec2<f32>(cos(theta), sin(theta)) * r / tex_size;
        let sample_uv = uv + offset;

        let sample_color = textureSampleLevel(input_tex, tex_sampler, sample_uv, 0.0).rgb;
        // Depth 텍스처는 textureLoad 사용 (textureSampleLevel 불가)
        let sample_pixel = vec2<i32>(sample_uv * tex_size);
        let sample_depth = textureLoad(depth_tex, sample_pixel, 0);
        let sample_coc = calculate_coc(sample_depth);

        // 밝은 픽셀 Bokeh 부각
        let brightness = max(max(sample_color.r, sample_color.g), sample_color.b);
        let bokeh_weight = 1.0 + brightness * params.bokeh_intensity;

        // 샘플 CoC가 충분히 큰 경우만 기여
        let weight = step(r, sample_coc + 0.5) * bokeh_weight;

        color += sample_color * weight;
        total_weight += weight;
    }

    return color / max(total_weight, 1.0);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = vec2<f32>(textureDimensions(input_tex));

    if (f32(pixel.x) >= tex_size.x || f32(pixel.y) >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / tex_size;

    // Depth 텍스처는 textureLoad 사용
    let depth = textureLoad(depth_tex, pixel, 0);
    let coc = calculate_coc(depth);

    let color = disk_blur(uv, coc, tex_size);

    textureStore(output_tex, pixel, vec4<f32>(color, 1.0));
}
