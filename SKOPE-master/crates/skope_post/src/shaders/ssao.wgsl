// SKOPE Engine - Screen Space Ambient Occlusion Shader

struct SSAOParams {
    radius: f32,
    intensity: f32,
    bias: f32,
    sample_count: u32,
    blur_passes: u32,
    blur_sharpness: f32,
    near_plane: f32,
    far_plane: f32,
}

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var normal_tex: texture_2d<f32>;
@group(0) @binding(2) var noise_tex: texture_2d<f32>;
@group(0) @binding(3) var output_tex: texture_storage_2d<r32float, write>;  // r8unorm은 storage 미지원
@group(0) @binding(4) var<uniform> params: SSAOParams;

// Depth를 선형 거리로 변환
fn linearize_depth(depth: f32) -> f32 {
    return params.near_plane * params.far_plane / (params.far_plane - depth * (params.far_plane - params.near_plane));
}

// View-space 위치 재구성 (간소화)
fn reconstruct_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let linear_depth = linearize_depth(depth);
    let x = (uv.x * 2.0 - 1.0) * linear_depth;
    let y = (uv.y * 2.0 - 1.0) * linear_depth;
    return vec3<f32>(x, y, linear_depth);
}

// 랜덤 반구 샘플 (미리 계산된 커널 사용 대신 런타임 생성)
fn get_sample_offset(index: u32, noise: vec2<f32>) -> vec3<f32> {
    let golden_ratio = 1.618033988749;
    let i = f32(index);

    // 피보나치 격자 기반 분포
    let theta = i * golden_ratio * 6.28318;
    let phi = acos(1.0 - 2.0 * (i + 0.5) / f32(params.sample_count));

    var offset = vec3<f32>(
        sin(phi) * cos(theta + noise.x * 6.28318),
        sin(phi) * sin(theta + noise.y * 6.28318),
        cos(phi)
    );

    // 반구 안쪽 분포 (가까운 샘플에 가중치)
    let scale = f32(index + 1u) / f32(params.sample_count);
    offset = offset * mix(0.1, 1.0, scale * scale);

    return offset;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let tex_size = textureDimensions(depth_tex);

    if (u32(pixel.x) >= tex_size.x || u32(pixel.y) >= tex_size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(tex_size);

    // 현재 픽셀 깊이
    let depth = textureLoad(depth_tex, pixel, 0);
    if (depth >= 1.0) {
        textureStore(output_tex, pixel, vec4<f32>(1.0));
        return;
    }

    let position = reconstruct_position(uv, depth);

    // Normal (G-Buffer에서)
    let normal_raw = textureLoad(normal_tex, pixel, 0).xyz;
    let normal = normalize(normal_raw * 2.0 - 1.0);

    // 노이즈 (타일링)
    let noise_uv = vec2<f32>(gid.xy) / 4.0;
    let noise_pixel = vec2<i32>(i32(gid.x) % 4, i32(gid.y) % 4);
    let noise = textureLoad(noise_tex, noise_pixel, 0).xy;

    // SSAO 샘플링
    var occlusion = 0.0;

    for (var i = 0u; i < params.sample_count; i++) {
        // 샘플 오프셋 (반구 내)
        var sample_offset = get_sample_offset(i, noise);

        // 노멀 방향으로 정렬
        if (dot(sample_offset, normal) < 0.0) {
            sample_offset = -sample_offset;
        }

        // 샘플 위치
        let sample_pos = position + sample_offset * params.radius;

        // 스크린 스페이스로 변환 (간소화)
        let sample_uv = vec2<f32>(
            sample_pos.x / sample_pos.z * 0.5 + 0.5,
            sample_pos.y / sample_pos.z * 0.5 + 0.5
        );

        // 범위 체크
        if (sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0) {
            continue;
        }

        // 샘플 깊이
        let sample_pixel = vec2<i32>(sample_uv * vec2<f32>(tex_size));
        let sample_depth = textureLoad(depth_tex, sample_pixel, 0);
        let sample_linear_depth = linearize_depth(sample_depth);

        // Range check & 차폐 계산
        let range_check = smoothstep(0.0, 1.0, params.radius / abs(position.z - sample_linear_depth));
        occlusion += select(0.0, 1.0, sample_linear_depth < sample_pos.z - params.bias) * range_check;
    }

    occlusion = 1.0 - (occlusion / f32(params.sample_count)) * params.intensity;

    textureStore(output_tex, pixel, vec4<f32>(occlusion));
}

// Edge-aware 블러 (별도 패스)
@compute @workgroup_size(8, 8)
fn blur_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // 이 패스는 raw_ao_texture를 입력으로 받아 output_texture에 블러된 결과를 씀
    // 실제 구현에서는 별도 바인딩 필요
}
