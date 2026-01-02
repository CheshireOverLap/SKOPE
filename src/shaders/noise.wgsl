// SKOPE Engine - Noise Functions
// Phase 13.4: Perlin Noise 및 FBM for Caustics

// ============================================
// Pseudo-Random Hash Functions
// ============================================

/// 2D 벡터를 입력으로 받아 0~1 범위의 랜덤 값 반환
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

/// 2D 벡터를 입력으로 받아 2D 랜덤 벡터 반환
fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

/// 3D 벡터를 입력으로 받아 0~1 범위의 랜덤 값 반환
fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// ============================================
// Gradient Noise (Perlin-like)
// ============================================

/// 2D Gradient 생성
fn gradient2d(i: vec2<f32>) -> vec2<f32> {
    let h = hash21(i);
    let angle = h * 6.283185307;  // 2 * PI
    return vec2<f32>(cos(angle), sin(angle));
}

/// 2D Perlin Noise (-1 ~ 1 범위)
fn perlin_noise_2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Hermite 보간을 위한 smooth curve
    let u = f * f * (3.0 - 2.0 * f);

    // 4개 코너의 그라디언트
    let g00 = gradient2d(i + vec2<f32>(0.0, 0.0));
    let g10 = gradient2d(i + vec2<f32>(1.0, 0.0));
    let g01 = gradient2d(i + vec2<f32>(0.0, 1.0));
    let g11 = gradient2d(i + vec2<f32>(1.0, 1.0));

    // 각 코너에서의 방향 벡터
    let d00 = f - vec2<f32>(0.0, 0.0);
    let d10 = f - vec2<f32>(1.0, 0.0);
    let d01 = f - vec2<f32>(0.0, 1.0);
    let d11 = f - vec2<f32>(1.0, 1.0);

    // 내적 계산
    let n00 = dot(g00, d00);
    let n10 = dot(g10, d10);
    let n01 = dot(g01, d01);
    let n11 = dot(g11, d11);

    // 보간
    let nx0 = mix(n00, n10, u.x);
    let nx1 = mix(n01, n11, u.x);

    return mix(nx0, nx1, u.y);
}

/// 간단한 Value Noise (더 빠름)
fn value_noise_2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Hermite 보간
    let u = f * f * (3.0 - 2.0 * f);

    // 4개 코너의 랜덤 값
    let n00 = hash21(i + vec2<f32>(0.0, 0.0));
    let n10 = hash21(i + vec2<f32>(1.0, 0.0));
    let n01 = hash21(i + vec2<f32>(0.0, 1.0));
    let n11 = hash21(i + vec2<f32>(1.0, 1.0));

    // 보간
    let nx0 = mix(n00, n10, u.x);
    let nx1 = mix(n01, n11, u.x);

    return mix(nx0, nx1, u.y);
}

// ============================================
// Fractal Brownian Motion (FBM)
// ============================================

/// 2D FBM Noise (여러 옥타브의 노이즈 합성)
/// - octaves: 레이어 수 (보통 4-6)
/// - lacunarity: 주파수 증가율 (보통 2.0)
/// - persistence: 진폭 감소율 (보통 0.5)
fn fbm_2d(p: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var max_value = 0.0;

    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * perlin_noise_2d(p * frequency);
        max_value = max_value + amplitude;
        amplitude = amplitude * persistence;
        frequency = frequency * lacunarity;
    }

    // 정규화 (-1 ~ 1)
    return value / max_value;
}

/// 빠른 FBM (Value noise 기반, 0 ~ 1 범위)
fn fbm_value_2d(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * value_noise_2d(p * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}

// ============================================
// Caustics 전용 노이즈
// ============================================

/// 코스틱 패턴 생성
/// 물의 굴절로 인한 빛의 집중/분산 효과를 시뮬레이션
fn caustics_noise(uv: vec2<f32>, time: f32) -> f32 {
    // 두 개의 이동하는 노이즈 레이어 합성
    let uv1 = uv * 3.0 + vec2<f32>(time * 0.1, time * 0.05);
    let uv2 = uv * 4.0 - vec2<f32>(time * 0.08, time * 0.12);

    let noise1 = fbm_value_2d(uv1, 3);
    let noise2 = fbm_value_2d(uv2, 3);

    // 두 노이즈의 차이로 집광 효과 생성
    let caustic = abs(noise1 - noise2);

    // 대비 증가
    return pow(caustic, 1.5) * 2.0;
}

/// Voronoi 기반 코스틱 (더 날카로운 패턴)
fn voronoi_caustics(uv: vec2<f32>, time: f32) -> f32 {
    let p = uv * 4.0;
    let i = floor(p);
    let f = fract(p);

    var min_dist = 1.0;
    var second_min = 1.0;

    // 3x3 이웃 검색
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell = i + neighbor;

            // 시간에 따라 움직이는 포인트
            var point = hash22(cell);
            point = 0.5 + 0.5 * sin(time * 0.5 + 6.283185307 * point);

            let diff = neighbor + point - f;
            let dist = length(diff);

            if (dist < min_dist) {
                second_min = min_dist;
                min_dist = dist;
            } else if (dist < second_min) {
                second_min = dist;
            }
        }
    }

    // 가장자리 강조 (코스틱 효과)
    return pow(second_min - min_dist, 0.5);
}

// ============================================
// 유틸리티 함수
// ============================================

/// Turbulence (절대값 FBM)
fn turbulence_2d(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * abs(perlin_noise_2d(p * frequency));
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}

/// Ridged Noise (능선 효과)
fn ridged_noise_2d(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var weight = 1.0;

    for (var i = 0; i < octaves; i = i + 1) {
        var signal = 1.0 - abs(perlin_noise_2d(p * frequency));
        signal = signal * signal * weight;
        weight = clamp(signal * 2.0, 0.0, 1.0);
        value = value + amplitude * signal;
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}
