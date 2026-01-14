// SKOPE Splash Screen Shader
// 로고, 프로그레스 바, 배경 그라데이션 렌더링
// (텍스트는 별도 텍스트 렌더러에서 처리)

struct Uniforms {
    progress: f32,      // 로딩 진행률 (0.0 ~ 1.0)
    time: f32,          // 시간 (애니메이션용)
    aspect: f32,        // 화면 종횡비
    stage: f32,         // 현재 단계 (0-6)
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var logo_tex: texture_2d<f32>;
@group(0) @binding(2) var logo_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // 풀스크린 쿼드 (6 vertices, 2 triangles)
    var positions = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(0.0, 0.0),
        vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(1.0, 0.0),
    );

    var out: VertexOutput;
    out.position = vec4(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // === 배경 그라데이션 (어두운 테마) ===
    let bg_top = vec3(0.05, 0.05, 0.08);      // 상단: 어두운 파랑
    let bg_bottom = vec3(0.02, 0.02, 0.03);   // 하단: 거의 검정
    var color = mix(bg_top, bg_bottom, uv.y);

    // === 로고 렌더링 (중앙 배치) ===
    let logo_scale = 0.25;
    let logo_aspect = 1.0;  // 로고 종횡비 (정사각형 가정)

    // 화면 종횡비 보정
    var centered_uv = uv - 0.5;
    centered_uv.x *= uniforms.aspect;

    // 로고 UV 계산
    let logo_uv = centered_uv / logo_scale + 0.5;

    // 로고 영역 내에서만 샘플링
    if (logo_uv.x >= 0.0 && logo_uv.x <= 1.0 && logo_uv.y >= 0.0 && logo_uv.y <= 1.0) {
        let logo = textureSample(logo_tex, logo_sampler, logo_uv);
        // 알파 블렌딩
        color = mix(color, logo.rgb, logo.a);
    }

    // === 프로그레스 바 ===
    let bar_y = 0.78;           // Y 위치 (하단에서 22%)
    let bar_height = 0.008;     // 바 높이
    let bar_width = 0.5;        // 바 너비 (화면 50%)
    let bar_x_start = (1.0 - bar_width) / 2.0;
    let bar_x_end = bar_x_start + bar_width;

    // 종횡비 보정된 UV
    let bar_uv_x = uv.x;
    let bar_uv_y = uv.y;

    // 바 영역 체크
    if (bar_uv_y > bar_y && bar_uv_y < bar_y + bar_height) {
        if (bar_uv_x >= bar_x_start && bar_uv_x <= bar_x_end) {
            // 바 내 상대 위치 (0.0 ~ 1.0)
            let bar_x = (bar_uv_x - bar_x_start) / bar_width;

            // 배경 바 (어두운 회색)
            color = vec3(0.12, 0.12, 0.15);

            // 진행 바
            if (bar_x <= uniforms.progress) {
                // 파란색 그라데이션 + 미세한 글로우 애니메이션
                let glow = sin(uniforms.time * 2.0 + bar_x * 8.0) * 0.08 + 0.92;
                let bar_color = mix(
                    vec3(0.15, 0.4, 0.9),   // 진한 파랑
                    vec3(0.3, 0.6, 1.0),    // 밝은 파랑
                    bar_x
                );
                color = bar_color * glow;

                // 끝부분 하이라이트
                let edge_dist = abs(bar_x - uniforms.progress);
                if (edge_dist < 0.02) {
                    let highlight = 1.0 - edge_dist / 0.02;
                    color = mix(color, vec3(0.5, 0.8, 1.0), highlight * 0.5);
                }
            }

            // 바 테두리 (상하 1픽셀)
            let border_y = (bar_uv_y - bar_y) / bar_height;
            if (border_y < 0.15 || border_y > 0.85) {
                color = mix(color, vec3(0.2, 0.25, 0.35), 0.5);
            }
        }
    }

    // === 바깥쪽 글로우 (미세한 비네팅) ===
    let vignette = 1.0 - length(uv - 0.5) * 0.3;
    color *= vignette;

    return vec4(color, 1.0);
}
