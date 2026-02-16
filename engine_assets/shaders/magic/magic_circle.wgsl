// ========== Magic Circle Main Shader ==========
// SDF 기반 마법진 렌더링

// Include SDF primitives (will be concatenated at build time)
// #include "sdf_primitives.wgsl"

// ========== Constants ==========

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

// Layer Types
const LAYER_CORE: u32 = 0u;
const LAYER_INNER_RING: u32 = 1u;
const LAYER_OUTER_RING: u32 = 2u;
const LAYER_RUNES: u32 = 3u;
const LAYER_NODES: u32 = 4u;
const LAYER_CONNECTIONS: u32 = 5u;

// Element Types
const ELEMENT_FIRE: u32 = 0u;
const ELEMENT_WATER: u32 = 1u;
const ELEMENT_LIGHTNING: u32 = 2u;
const ELEMENT_WIND: u32 = 3u;
const ELEMENT_EARTH: u32 = 4u;
const ELEMENT_VOID: u32 = 5u;

// ========== Uniforms ==========

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
}

struct CircleUniform {
    // Transform
    world_position: vec3<f32>,
    scale: f32,
    normal: vec3<f32>,
    base_rotation: f32,

    // Appearance
    color: vec4<f32>,
    opacity: f32,
    time: f32,
    spawn_progress: f32,
    activate_progress: f32,
}

struct LayerData {
    rotation: f32,
    radius_min: f32,
    radius_max: f32,
    glow_intensity: f32,
    pulse: f32,
    segments: u32,
    layer_type: u32,
    _pad: u32,
}

struct NodeData {
    position: vec2<f32>,  // Cartesian (극좌표에서 변환됨)
    element: u32,
    activation: f32,
    size: f32,
    _pad: vec3<f32>,
}

struct ConnectionData {
    from_pos: vec2<f32>,
    to_pos: vec2<f32>,
    flow_progress: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

@group(1) @binding(0) var<uniform> circle: CircleUniform;
@group(1) @binding(1) var<storage, read> layers: array<LayerData>;
@group(1) @binding(2) var<storage, read> nodes: array<NodeData>;
@group(1) @binding(3) var<storage, read> connections: array<ConnectionData>;
@group(1) @binding(4) var<uniform> counts: vec4<u32>; // layer_count, node_count, connection_count, _

// ========== SDF Primitives (inlined) ==========

fn sdf_circle(p: vec2<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

fn sdf_ring(p: vec2<f32>, inner: f32, outer: f32) -> f32 {
    let d = length(p);
    return max(inner - d, d - outer);
}

fn sdf_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, thickness: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - thickness;
}

fn rotate2d(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

fn polar_repeat(p: vec2<f32>, count: f32) -> vec2<f32> {
    let angle = atan2(p.y, p.x);
    let segment = TAU / count;
    let a = ((angle + segment * 0.5) % segment) - segment * 0.5;
    return vec2<f32>(cos(a), sin(a)) * length(p);
}

// ========== Element Colors ==========

fn element_color(element: u32) -> vec3<f32> {
    switch element {
        case ELEMENT_FIRE: { return vec3<f32>(1.0, 0.3, 0.1); }
        case ELEMENT_WATER: { return vec3<f32>(0.2, 0.5, 1.0); }
        case ELEMENT_LIGHTNING: { return vec3<f32>(1.0, 0.9, 0.2); }
        case ELEMENT_WIND: { return vec3<f32>(0.3, 0.9, 0.4); }
        case ELEMENT_EARTH: { return vec3<f32>(0.6, 0.4, 0.2); }
        case ELEMENT_VOID: { return vec3<f32>(0.6, 0.2, 0.9); }
        default: { return vec3<f32>(1.0, 1.0, 1.0); }
    }
}

// ========== Rendering Functions ==========

fn render_core(uv: vec2<f32>, layer: LayerData, time: f32) -> vec4<f32> {
    let rotated_uv = rotate2d(uv, layer.rotation);

    // 코어 원
    let d = sdf_circle(rotated_uv, layer.radius_max);
    let glow = exp(-abs(d) * 10.0) * layer.glow_intensity;

    // 펄스 효과
    let pulse = 1.0 + sin(time * 3.0) * 0.2 * layer.pulse;

    // 내부 패턴 (동심원)
    let inner_pattern = exp(-abs(sdf_circle(rotated_uv, layer.radius_max * 0.5)) * 20.0) * 0.5;

    let alpha = (glow + inner_pattern) * pulse;

    return vec4<f32>(circle.color.rgb, alpha * circle.opacity);
}

fn render_ring(uv: vec2<f32>, layer: LayerData, time: f32) -> vec4<f32> {
    let rotated_uv = rotate2d(uv, layer.rotation);

    // 링 자체
    let d = sdf_ring(rotated_uv, layer.radius_min, layer.radius_max);
    let edge = 1.0 - smoothstep(0.0, 0.01, abs(d));

    var alpha = edge * 0.3;

    // 세그먼트 패턴
    if layer.segments > 0u {
        let seg_uv = polar_repeat(rotated_uv, f32(layer.segments));
        let thickness = (layer.radius_max - layer.radius_min) * 0.3;

        // 방사형 선
        let seg_d = sdf_segment(
            seg_uv,
            vec2<f32>(layer.radius_min, 0.0),
            vec2<f32>(layer.radius_max, 0.0),
            0.003
        );
        let seg_alpha = 1.0 - smoothstep(0.0, 0.01, seg_d);

        // 장식용 점
        let dot_pos = vec2<f32>((layer.radius_min + layer.radius_max) * 0.5, 0.0);
        let dot_d = sdf_circle(seg_uv - dot_pos, 0.015);
        let dot_alpha = 1.0 - smoothstep(0.0, 0.01, dot_d);

        alpha = max(alpha, max(seg_alpha * 0.8, dot_alpha));
    }

    // 글로우
    alpha *= layer.glow_intensity;

    return vec4<f32>(circle.color.rgb, alpha * circle.opacity);
}

fn render_node(uv: vec2<f32>, node: NodeData, time: f32) -> vec4<f32> {
    let node_pos = node.position;
    let d = sdf_circle(uv - node_pos, node.size * 0.04);

    // 글로우 + 펄스
    let glow = exp(-abs(d) * 15.0);
    let pulse = 1.0 + sin(time * 4.0 + node.position.x * 10.0) * 0.3;

    // 활성화 상태에 따른 강도
    let intensity = node.activation;

    // 코어
    let core_d = sdf_circle(uv - node_pos, node.size * 0.02);
    let core_alpha = (1.0 - smoothstep(0.0, 0.01, core_d)) * intensity;

    // 외부 링
    let ring_d = sdf_ring(uv - node_pos, node.size * 0.035, node.size * 0.045);
    let ring_alpha = (1.0 - smoothstep(0.0, 0.005, abs(ring_d))) * intensity * 0.5;

    let alpha = (glow * pulse + core_alpha + ring_alpha) * intensity;

    let col = element_color(node.element);
    return vec4<f32>(col, alpha);
}

fn render_connection(uv: vec2<f32>, conn: ConnectionData, time: f32) -> vec4<f32> {
    let d = sdf_segment(uv, conn.from_pos, conn.to_pos, 0.002);
    let line_alpha = (1.0 - smoothstep(0.0, 0.01, d)) * 0.3;

    // Flow 애니메이션 (에너지 이동)
    let dir = normalize(conn.to_pos - conn.from_pos);
    let total_len = length(conn.to_pos - conn.from_pos);

    // 프로젝션 계산
    let to_point = uv - conn.from_pos;
    let proj = dot(to_point, dir);

    // Flow 위치 (0 ~ total_len)
    let flow_pos = conn.flow_progress * total_len;

    // 에너지 파티클 글로우
    let flow_glow = exp(-abs(proj - flow_pos) * 30.0) * 0.8;

    // 추가 파티클들 (역방향)
    let flow_glow2 = exp(-abs(proj - (total_len - flow_pos)) * 30.0) * 0.4;

    return vec4<f32>(circle.color.rgb, (line_alpha + flow_glow + flow_glow2) * circle.opacity);
}

// ========== Layer Rendering Dispatcher ==========

fn render_layer(uv: vec2<f32>, layer: LayerData, time: f32) -> vec4<f32> {
    switch layer.layer_type {
        case LAYER_CORE: {
            return render_core(uv, layer, time);
        }
        case LAYER_INNER_RING, LAYER_OUTER_RING: {
            return render_ring(uv, layer, time);
        }
        default: {
            return vec4<f32>(0.0);
        }
    }
}

// ========== Vertex Shader ==========

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // 빌보드 쿼드 (-1 ~ 1)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );

    let pos2d = positions[idx];

    // 빌보드 회전 (카메라를 향함)
    let right = vec3<f32>(camera.view[0][0], camera.view[1][0], camera.view[2][0]);
    let up = vec3<f32>(camera.view[0][1], camera.view[1][1], camera.view[2][1]);

    let world_pos = circle.world_position
        + right * pos2d.x * circle.scale
        + up * pos2d.y * circle.scale;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = pos2d;
    return out;
}

// ========== Fragment Shader ==========

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var final_color = vec4<f32>(0.0);
    let uv = rotate2d(in.uv, circle.base_rotation);

    // 스폰 애니메이션: 바깥에서 안쪽으로 펼쳐지기
    let spawn_mask = 1.0 - smoothstep(circle.spawn_progress - 0.1, circle.spawn_progress, length(uv));
    if circle.spawn_progress < 1.0 && length(uv) > circle.spawn_progress {
        // 스폰 중 외곽은 숨김
        discard;
    }

    // 1. 레이어 렌더링 (Core, Rings)
    for (var i = 0u; i < counts.x; i++) {
        let layer = layers[i];

        // Nodes와 Connections 레이어는 별도 처리
        if layer.layer_type == LAYER_NODES || layer.layer_type == LAYER_CONNECTIONS {
            continue;
        }

        let layer_color = render_layer(uv, layer, circle.time);

        // 블렌딩 (additive)
        final_color = vec4<f32>(
            final_color.rgb + layer_color.rgb * layer_color.a,
            max(final_color.a, layer_color.a)
        );
    }

    // 2. 연결선 렌더링
    for (var i = 0u; i < counts.z; i++) {
        let conn_color = render_connection(uv, connections[i], circle.time);
        final_color = vec4<f32>(
            final_color.rgb + conn_color.rgb * conn_color.a,
            max(final_color.a, conn_color.a)
        );
    }

    // 3. 노드 렌더링
    for (var i = 0u; i < counts.y; i++) {
        let node_color = render_node(uv, nodes[i], circle.time);
        final_color = vec4<f32>(
            final_color.rgb + node_color.rgb * node_color.a,
            max(final_color.a, node_color.a)
        );
    }

    // 4. 외곽 글로우
    let outer_d = sdf_circle(uv, 1.0);
    let outer_glow = exp(-outer_d * 5.0) * 0.15;
    final_color = vec4<f32>(final_color.rgb + circle.color.rgb * outer_glow, final_color.a);

    // 5. 발동 애니메이션
    if circle.activate_progress > 0.0 {
        let activate_pulse = sin(circle.activate_progress * PI) * 0.5;
        final_color = vec4<f32>(final_color.rgb + circle.color.rgb * activate_pulse, final_color.a);

        // 중심으로 수렴하는 효과
        if circle.activate_progress < 0.5 {
            let converge = 1.0 - circle.activate_progress * 2.0;
            let converge_glow = exp(-length(uv) * 10.0 * converge) * converge * 0.5;
            final_color = vec4<f32>(final_color.rgb + circle.color.rgb * converge_glow, final_color.a);
        }
    }

    // 6. 최종 불투명도 적용
    final_color = vec4<f32>(final_color.rgb, final_color.a * circle.opacity);

    // 알파가 너무 낮으면 폐기
    if final_color.a < 0.01 {
        discard;
    }

    return final_color;
}
