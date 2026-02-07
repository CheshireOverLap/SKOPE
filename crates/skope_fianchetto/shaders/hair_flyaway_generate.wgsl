// SKOPE Engine - Flyaway Strand Generation Shader
// Phase 12: Generate wispy flyaway hair strands

struct FlyawayParams {
    length_min: f32,
    length_max: f32,
    curl_amount: f32,
    curl_frequency: f32,
    spread_angle: f32,
    root_offset: f32,
    _pad: vec2<f32>,
}

struct StrandVertex {
    position: vec3<f32>,
    thickness: f32,
    velocity: vec3<f32>,
    _pad: f32,
}

@group(0) @binding(0) var<uniform> params: FlyawayParams;
@group(0) @binding(1) var<storage, read> scalp_points: array<vec4<f32>>;  // xyz + normal_packed
@group(0) @binding(2) var<storage, read_write> strand_vertices: array<StrandVertex>;
@group(0) @binding(3) var<uniform> segments_per_strand: u32;
@group(0) @binding(4) var<uniform> time: f32;
@group(0) @binding(5) var<uniform> strand_count: u32;

// 간단한 해시 함수
fn hash(seed: u32) -> f32 {
    var x = seed;
    x = x * 0x85ebca6bu;
    x = x ^ (x >> 13u);
    x = x * 0xc2b2ae35u;
    x = x ^ (x >> 16u);
    return f32(x) / f32(0xffffffffu);
}

fn hash3(seed: u32) -> vec3<f32> {
    return vec3<f32>(
        hash(seed),
        hash(seed + 1u),
        hash(seed + 2u)
    );
}

// Unpack normal from float
fn unpack_normal(packed: f32) -> vec3<f32> {
    let bits = bitcast<u32>(packed);
    let x = f32((bits >> 20u) & 0x3ffu) / 1023.0 * 2.0 - 1.0;
    let y = f32((bits >> 10u) & 0x3ffu) / 1023.0 * 2.0 - 1.0;
    let z = f32(bits & 0x3ffu) / 1023.0 * 2.0 - 1.0;
    return normalize(vec3<f32>(x, y, z));
}

@compute @workgroup_size(64)
fn generate_flyaway(@builtin(global_invocation_id) gid: vec3<u32>) {
    let strand_id = gid.x;
    if (strand_id >= strand_count) {
        return;
    }

    let scalp_data = scalp_points[strand_id];
    let root_pos = scalp_data.xyz;

    // Normal: Y-up 기본 (또는 packed에서 unpack)
    var normal = vec3<f32>(0.0, 1.0, 0.0);
    if (scalp_data.w != 0.0) {
        normal = unpack_normal(scalp_data.w);
    }

    // 랜덤 파라미터
    let rand = hash3(strand_id);
    let length = mix(params.length_min, params.length_max, rand.x);
    let spread = (rand.y - 0.5) * 2.0 * params.spread_angle;
    let curl_phase = rand.z * 6.28318;

    // 루트 위치에 약간의 랜덤 오프셋
    let root_offset = (hash3(strand_id + 1000u) - 0.5) * params.root_offset;
    var pos = root_pos + root_offset;

    // 기본 방향 (normal + spread)
    let cos_spread = cos(spread);
    let sin_spread = sin(spread);

    // Tangent 계산 (normal에 수직인 방향)
    var tangent = vec3<f32>(1.0, 0.0, 0.0);
    if (abs(normal.x) < 0.9) {
        tangent = normalize(cross(normal, vec3<f32>(1.0, 0.0, 0.0)));
    } else {
        tangent = normalize(cross(normal, vec3<f32>(0.0, 0.0, 1.0)));
    }

    var dir = normalize(normal * cos_spread + tangent * sin_spread);

    // 세그먼트별 위치 계산
    let segment_length = length / f32(segments_per_strand);
    let vertices_per_strand = segments_per_strand + 1u;

    for (var seg = 0u; seg <= segments_per_strand; seg++) {
        let t = f32(seg) / f32(segments_per_strand);
        let vertex_idx = strand_id * vertices_per_strand + seg;

        // Curl 적용
        let curl_angle = t * params.curl_frequency * 6.28318 + curl_phase;
        let curl_offset = tangent * cos(curl_angle) * params.curl_amount * t
                        + cross(normal, tangent) * sin(curl_angle) * params.curl_amount * t;

        // 바람 영향
        let wind_strength = sin(time * 2.0 + f32(strand_id) * 0.1) * 0.01 * t;
        let wind = vec3<f32>(wind_strength, 0.0, wind_strength * 0.5);

        // 중력 영향 (약간)
        let gravity = vec3<f32>(0.0, -0.02 * t * t, 0.0);

        // 최종 위치
        let segment_pos = pos + dir * segment_length * f32(seg);
        let final_pos = segment_pos + curl_offset + wind + gravity;

        // 두께: 루트에서 팁으로 갈수록 얇아짐
        let thickness = mix(0.0008, 0.0002, t);

        strand_vertices[vertex_idx] = StrandVertex(
            final_pos,
            thickness,
            vec3<f32>(0.0),
            0.0
        );
    }
}
