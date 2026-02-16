// ========== SDF Primitives for Magic Circle ==========
// 마법진 렌더링을 위한 SDF 기본 함수들

// ========== Basic Shapes ==========

/// 원 SDF
fn sdf_circle(p: vec2<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

/// 링 (도넛) SDF
fn sdf_ring(p: vec2<f32>, inner: f32, outer: f32) -> f32 {
    let d = length(p);
    return max(inner - d, d - outer);
}

/// 선분 SDF
fn sdf_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, thickness: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - thickness;
}

/// 정다각형 SDF
fn sdf_polygon(p: vec2<f32>, radius: f32, sides: u32) -> f32 {
    let angle = atan2(p.y, p.x);
    let segment_angle = 6.283185 / f32(sides);
    let half_segment = segment_angle * 0.5;

    let sector_angle = ((angle + half_segment) % segment_angle) - half_segment;
    let rotated = vec2<f32>(cos(sector_angle), abs(sin(sector_angle))) * length(p);

    return rotated.x - radius * cos(half_segment);
}

/// 별 SDF
fn sdf_star(p: vec2<f32>, outer_r: f32, inner_r: f32, points: u32) -> f32 {
    let angle = atan2(p.y, p.x);
    let segment = 3.14159 / f32(points);
    let a = ((angle + segment) % (2.0 * segment)) - segment;

    let r = length(p);

    // 선형 보간으로 별 모양 생성
    let t = abs(a) / segment;
    let target_r = mix(outer_r, inner_r, t);

    return r - target_r;
}

/// 호 (Arc) SDF
fn sdf_arc(p: vec2<f32>, radius: f32, angle_start: f32, angle_end: f32, thickness: f32) -> f32 {
    let angle = atan2(p.y, p.x);
    let r = length(p);

    // 호 범위 체크
    var in_range = false;
    if angle_start < angle_end {
        in_range = angle >= angle_start && angle <= angle_end;
    } else {
        in_range = angle >= angle_start || angle <= angle_end;
    }

    if in_range {
        return abs(r - radius) - thickness;
    } else {
        // 호 끝점까지의 거리
        let end1 = vec2<f32>(cos(angle_start), sin(angle_start)) * radius;
        let end2 = vec2<f32>(cos(angle_end), sin(angle_end)) * radius;
        return min(length(p - end1), length(p - end2)) - thickness;
    }
}

/// 박스 SDF (둥근 모서리)
fn sdf_rounded_box(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// ========== Boolean Operators ==========

/// Union (합집합)
fn op_union(d1: f32, d2: f32) -> f32 {
    return min(d1, d2);
}

/// Subtraction (차집합)
fn op_subtract(d1: f32, d2: f32) -> f32 {
    return max(d1, -d2);
}

/// Intersection (교집합)
fn op_intersect(d1: f32, d2: f32) -> f32 {
    return max(d1, d2);
}

/// Smooth Union (부드러운 합집합)
fn op_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

/// Smooth Subtraction
fn op_smooth_subtract(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (d2 + d1) / k, 0.0, 1.0);
    return mix(d1, -d2, h) + k * h * (1.0 - h);
}

/// Smooth Intersection
fn op_smooth_intersect(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) + k * h * (1.0 - h);
}

// ========== Transforms ==========

/// 2D 회전
fn rotate2d(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

/// 극좌표 반복 (n등분)
fn polar_repeat(p: vec2<f32>, count: f32) -> vec2<f32> {
    let angle = atan2(p.y, p.x);
    let segment = 6.283185 / count;
    let a = ((angle + segment * 0.5) % segment) - segment * 0.5;
    return vec2<f32>(cos(a), sin(a)) * length(p);
}

/// 극좌표 반복 (인덱스 반환)
fn polar_repeat_index(p: vec2<f32>, count: f32) -> u32 {
    let angle = atan2(p.y, p.x);
    let segment = 6.283185 / count;
    let idx = floor((angle + 3.14159) / segment);
    return u32(idx) % u32(count);
}

/// 거울 반사 (X축 기준)
fn mirror_x(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(abs(p.x), p.y);
}

/// 거울 반사 (Y축 기준)
fn mirror_y(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x, abs(p.y));
}

// ========== Utility Functions ==========

/// 글로우 효과 (거리 기반)
fn glow(d: f32, intensity: f32, falloff: f32) -> f32 {
    return intensity * exp(-abs(d) * falloff);
}

/// 부드러운 엣지
fn smooth_edge(d: f32, smoothness: f32) -> f32 {
    return 1.0 - smoothstep(0.0, smoothness, d);
}

/// 안티에일리어싱 엣지
fn aa_edge(d: f32) -> f32 {
    return 1.0 - smoothstep(-0.005, 0.005, d);
}

/// 펄스 애니메이션
fn pulse(t: f32, speed: f32, amplitude: f32) -> f32 {
    return 1.0 + sin(t * speed) * amplitude;
}

/// 극좌표 UV 계산
fn to_polar_uv(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(length(p), atan2(p.y, p.x));
}

/// 직교좌표로 변환
fn from_polar_uv(polar: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(polar.x * cos(polar.y), polar.x * sin(polar.y));
}

// ========== Pattern Functions ==========

/// 동심원 패턴
fn concentric_rings(p: vec2<f32>, spacing: f32, thickness: f32) -> f32 {
    let r = length(p);
    let ring = abs(fract(r / spacing) - 0.5) * spacing;
    return smooth_edge(ring - thickness * 0.5, 0.01);
}

/// 방사형 패턴
fn radial_lines(p: vec2<f32>, count: f32, thickness: f32) -> f32 {
    let angle = atan2(p.y, p.x);
    let segment = 6.283185 / count;
    let line_angle = abs(fract(angle / segment + 0.5) - 0.5) * segment;
    return smooth_edge(line_angle - thickness, 0.02);
}

/// 소용돌이 패턴
fn spiral(p: vec2<f32>, arms: f32, twist: f32) -> f32 {
    let angle = atan2(p.y, p.x);
    let r = length(p);
    let spiral_angle = angle + r * twist;
    return fract(spiral_angle * arms / 6.283185);
}
