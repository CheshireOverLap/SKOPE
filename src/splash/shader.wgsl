// SKOPE Splash Screen Shader
// 로고, 프로그레스 바, 배경 그라데이션, 텍스트 렌더링

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

// ============== 픽셀 폰트 시스템 ==============
// 4x5 비트맵 폰트 (각 문자는 20비트로 인코딩)
// 비트 레이아웃: 상단부터 하단으로, 왼쪽부터 오른쪽으로

fn get_char_bitmap(c: u32) -> u32 {
    // 0-9 숫자
    if (c == 48u) { return 0x69996u; }  // 0: 0110 1001 1001 1001 0110
    if (c == 49u) { return 0x26227u; }  // 1: 0010 0110 0010 0010 0111
    if (c == 50u) { return 0x69127u; }  // 2: 0110 1001 0001 0010 0111
    if (c == 51u) { return 0x69169u; }  // 3: 0110 1001 0001 0110 1001
    if (c == 52u) { return 0x99711u; }  // 4: 1001 1001 0111 0001 0001
    if (c == 53u) { return 0xf8e1eu; }  // 5: 1111 1000 1110 0001 1110
    if (c == 54u) { return 0x68e9eu; }  // 6: 0110 1000 1110 1001 1110
    if (c == 55u) { return 0xf1244u; }  // 7: 1111 0001 0010 0100 0100
    if (c == 56u) { return 0x6969eu; }  // 8: 0110 1001 0110 1001 1110
    if (c == 57u) { return 0x69716u; }  // 9: 0110 1001 0111 0001 0110

    // 대문자
    if (c == 67u) { return 0x78886u; }  // C: 0111 1000 1000 1000 0110
    if (c == 70u) { return 0xf8e88u; }  // F: 1111 1000 1110 1000 1000
    if (c == 76u) { return 0x8888fu; }  // L: 1000 1000 1000 1000 1111
    if (c == 82u) { return 0xe9e98u; }  // R: 1110 1001 1110 1001 1000

    // 소문자
    if (c == 97u) { return 0x06f9fu; }   // a: 0000 0110 1111 1001 1111
    if (c == 99u) { return 0x0688eu; }   // c: 0000 0110 1000 1000 1110
    if (c == 100u) { return 0x11f99fu; } // d: 0001 0001 1111 1001 1111
    if (c == 101u) { return 0x069f8eu; } // e: 0000 0110 1001 1111 1000 1110
    if (c == 103u) { return 0x06996u; }  // g: 0000 0110 1001 1001 0110
    if (c == 104u) { return 0x88e99u; }  // h: 1000 1000 1110 1001 1001
    if (c == 105u) { return 0x20222u; }  // i: 0010 0000 0010 0010 0010
    if (c == 108u) { return 0xc4447u; }  // l: 1100 0100 0100 0100 0111
    if (c == 110u) { return 0x0e999u; }  // n: 0000 1110 1001 1001 1001
    if (c == 111u) { return 0x06996u; }  // o: 0000 0110 1001 1001 0110
    if (c == 114u) { return 0x05888u; }  // r: 0000 0101 1000 1000 1000
    if (c == 115u) { return 0x07861eu; } // s: 0000 0111 1000 0110 0001 1110
    if (c == 116u) { return 0x4e442u; }  // t: 0100 1110 0100 0100 0010
    if (c == 117u) { return 0x09996u; }  // u: 0000 1001 1001 1001 0110
    if (c == 120u) { return 0x09669u; }  // x: 0000 1001 0110 0110 1001
    if (c == 121u) { return 0x09971u; }  // y: 0000 1001 1001 0111 0001
    if (c == 122u) { return 0x0f24fu; }  // z: 0000 1111 0010 0100 1111
    if (c == 109u) { return 0x0d999u; }  // m: 0000 1101 1001 1001 1001

    // 특수문자
    if (c == 46u) { return 0x00002u; }   // .: 0000 0000 0000 0000 0010
    if (c == 33u) { return 0x22202u; }   // !: 0010 0010 0010 0000 0010
    if (c == 37u) { return 0x91249u; }   // %: 1001 0001 0010 0100 1001
    if (c == 32u) { return 0x00000u; }   // space

    return 0u;
}

// 문자 렌더링 (픽셀 좌표 기반) - 안티앨리어싱 적용
fn render_char(px: f32, py: f32, char_x: f32, char_y: f32, char_code: u32) -> f32 {
    let char_w = 4.0;
    let char_h = 5.0;

    // 문자 내 상대 위치
    let lx = px - char_x;
    let ly = char_y + char_h - py;  // Y 반전

    // 범위 체크 (패딩 포함)
    if (lx < -0.5 || lx >= char_w + 0.5 || ly < -0.5 || ly >= char_h + 0.5) {
        return 0.0;
    }

    let bitmap = get_char_bitmap(char_code);

    // 서브픽셀 샘플링으로 안티앨리어싱
    var total = 0.0;
    let samples = 4.0;
    let step = 1.0 / samples;

    for (var sy = 0.0; sy < 1.0; sy = sy + step) {
        for (var sx = 0.0; sx < 1.0; sx = sx + step) {
            let sample_x = lx + sx * 0.5 - 0.25;
            let sample_y = ly + sy * 0.5 - 0.25;

            if (sample_x >= 0.0 && sample_x < char_w && sample_y >= 0.0 && sample_y < char_h) {
                let bit_idx = u32(sample_y) * 4u + u32(sample_x);
                if (bit_idx < 20u && ((bitmap >> (19u - bit_idx)) & 1u) == 1u) {
                    total = total + 1.0;
                }
            }
        }
    }

    return total / (samples * samples);
}

// 단계별 텍스트 렌더링
fn render_stage_text(px: f32, py: f32, stage: u32) -> f32 {
    var result = 0.0;
    let start_x = px;
    let char_spacing = 5.0;
    var x_offset = 0.0;

    // 각 단계별 텍스트 (ASCII 코드 시퀀스)
    if (stage == 0u) {
        // "Creating renderers..."
        let text = array<u32, 21>(67u, 114u, 101u, 97u, 116u, 105u, 110u, 103u, 32u,
                                   114u, 101u, 110u, 100u, 101u, 114u, 101u, 114u, 115u,
                                   46u, 46u, 46u);
        for (var i = 0u; i < 21u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    } else if (stage == 1u) {
        // "Loading textures..."
        let text = array<u32, 19>(76u, 111u, 97u, 100u, 105u, 110u, 103u, 32u,
                                   116u, 101u, 120u, 116u, 117u, 114u, 101u, 115u,
                                   46u, 46u, 46u);
        for (var i = 0u; i < 19u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    } else if (stage == 2u) {
        // "Loading meshes..."
        let text = array<u32, 17>(76u, 111u, 97u, 100u, 105u, 110u, 103u, 32u,
                                   109u, 101u, 115u, 104u, 101u, 115u,
                                   46u, 46u, 46u);
        for (var i = 0u; i < 17u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    } else if (stage == 3u) {
        // "Loading scene..."
        let text = array<u32, 16>(76u, 111u, 97u, 100u, 105u, 110u, 103u, 32u,
                                   115u, 99u, 101u, 110u, 101u,
                                   46u, 46u, 46u);
        for (var i = 0u; i < 16u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    } else if (stage == 4u) {
        // "Loading characters..."
        let text = array<u32, 21>(76u, 111u, 97u, 100u, 105u, 110u, 103u, 32u,
                                   99u, 104u, 97u, 114u, 97u, 99u, 116u, 101u, 114u, 115u,
                                   46u, 46u, 46u);
        for (var i = 0u; i < 21u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    } else if (stage == 5u) {
        // "Finalizing..."
        let text = array<u32, 14>(70u, 105u, 110u, 97u, 108u, 105u, 122u, 105u, 110u, 103u,
                                   46u, 46u, 46u, 0u);
        for (var i = 0u; i < 13u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    } else {
        // "Ready!"
        let text = array<u32, 6>(82u, 101u, 97u, 100u, 121u, 33u);
        for (var i = 0u; i < 6u; i = i + 1u) {
            result = max(result, render_char(px, py, x_offset, 0.0, text[i]));
            x_offset = x_offset + char_spacing;
        }
    }

    return result;
}

// 퍼센트 숫자 렌더링
fn render_percent(px: f32, py: f32, percent: u32) -> f32 {
    var result = 0.0;
    let char_spacing = 5.0;
    var x_offset = 0.0;

    // 100% 특수 처리
    if (percent >= 100u) {
        result = max(result, render_char(px, py, x_offset, 0.0, 49u));  // 1
        x_offset = x_offset + char_spacing;
        result = max(result, render_char(px, py, x_offset, 0.0, 48u));  // 0
        x_offset = x_offset + char_spacing;
        result = max(result, render_char(px, py, x_offset, 0.0, 48u));  // 0
        x_offset = x_offset + char_spacing;
    } else if (percent >= 10u) {
        // 두 자리수
        let tens = percent / 10u;
        let ones = percent % 10u;
        result = max(result, render_char(px, py, x_offset, 0.0, 48u + tens));
        x_offset = x_offset + char_spacing;
        result = max(result, render_char(px, py, x_offset, 0.0, 48u + ones));
        x_offset = x_offset + char_spacing;
    } else {
        // 한 자리수
        result = max(result, render_char(px, py, x_offset, 0.0, 48u + percent));
        x_offset = x_offset + char_spacing;
    }

    // % 기호
    result = max(result, render_char(px, py, x_offset, 0.0, 37u));

    return result;
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

    // === 텍스트 렌더링 (프로그레스 바 아래) ===
    let text_y = 0.82;          // 텍스트 Y 위치
    let text_height = 0.04;     // 텍스트 영역 높이
    let text_scale = 4.0;       // 텍스트 스케일 (픽셀 확대) - 더 크게

    if (uv.y > text_y && uv.y < text_y + text_height) {
        // 픽셀 좌표 계산 (화면 중앙 기준)
        let pixel_x = (uv.x - 0.32) * 1000.0 / text_scale;  // 스케일 적용
        let pixel_y = (uv.y - text_y) * 1000.0 / text_scale;

        // 단계 텍스트 렌더링
        let text_alpha = render_stage_text(pixel_x, pixel_y, u32(uniforms.stage));

        if (text_alpha > 0.01) {
            // 알파 블렌딩으로 부드러운 텍스트
            let text_color = vec3(0.75, 0.78, 0.85);
            color = mix(color, text_color, text_alpha);
        }
    }

    // === 퍼센트 표시 (텍스트 오른쪽) ===
    let percent_y = 0.82;
    let percent_height = 0.04;

    if (uv.y > percent_y && uv.y < percent_y + percent_height) {
        let pixel_x = (uv.x - 0.60) * 1000.0 / text_scale;
        let pixel_y = (uv.y - percent_y) * 1000.0 / text_scale;

        let percent = u32(uniforms.progress * 100.0);
        let percent_alpha = render_percent(pixel_x, pixel_y, percent);

        if (percent_alpha > 0.01) {
            // 알파 블렌딩으로 부드러운 퍼센트
            let percent_color = vec3(0.5, 0.7, 0.95);
            color = mix(color, percent_color, percent_alpha);
        }
    }

    // === 바깥쪽 글로우 (미세한 비네팅) ===
    let vignette = 1.0 - length(uv - 0.5) * 0.3;
    color *= vignette;

    return vec4(color, 1.0);
}
