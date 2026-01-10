// GPU Particle Spawn Compute Shader
// SKOPE Engine
// 새 파티클 스폰 및 초기화

struct GpuParticle {
    position: vec3<f32>,
    lifetime: f32,
    velocity: vec3<f32>,
    max_lifetime: f32,
    color: vec4<f32>,
    size: f32,
    rotation: f32,
    rotation_speed: f32,
    alive: u32,
    seed: u32,
    _pad: vec3<f32>,
}

struct SpawnConfig {
    // 스폰 위치 및 범위
    emitter_position: vec3<f32>,
    spawn_count: u32,        // 이번 프레임에 스폰할 파티클 수

    // 초기 속도 범위
    velocity_min: vec3<f32>,
    spawn_start_index: u32,  // 스폰 시작 인덱스
    velocity_max: vec3<f32>,
    total_particles: u32,    // 전체 파티클 버퍼 크기

    // 수명 범위
    lifetime_min: f32,
    lifetime_max: f32,

    // 크기 범위
    size_min: f32,
    size_max: f32,

    // 회전 속도 범위
    rotation_speed_min: f32,
    rotation_speed_max: f32,

    // 랜덤 시드
    random_seed: f32,
    spawn_shape: u32,        // 0=Point, 1=Box, 2=Sphere, 3=Cone

    // 스폰 영역 크기
    spawn_extent: vec3<f32>,
    cone_angle: f32,

    // 초기 색상
    color_start: vec4<f32>,
}

@group(0) @binding(0) var<storage, read_write> particles: array<GpuParticle>;
@group(0) @binding(1) var<uniform> config: SpawnConfig;

// Hash function for pseudo-random numbers
fn hash(seed: u32) -> f32 {
    var s = seed;
    s = s ^ (s >> 16u);
    s = s * 0x7feb352du;
    s = s ^ (s >> 15u);
    s = s * 0x846ca68bu;
    s = s ^ (s >> 16u);
    return f32(s) / f32(0xffffffffu);
}

fn random_range(seed: u32, min_val: f32, max_val: f32) -> f32 {
    return mix(min_val, max_val, hash(seed));
}

fn random_vec3_range(seed: u32, min_val: vec3<f32>, max_val: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        random_range(seed, min_val.x, max_val.x),
        random_range(seed + 1u, min_val.y, max_val.y),
        random_range(seed + 2u, min_val.z, max_val.z)
    );
}

// 스폰 위치 계산 (형태별)
fn calculate_spawn_position(seed: u32) -> vec3<f32> {
    switch config.spawn_shape {
        case 0u: { // Point
            return config.emitter_position;
        }
        case 1u: { // Box
            let offset = random_vec3_range(seed, -config.spawn_extent, config.spawn_extent);
            return config.emitter_position + offset;
        }
        case 2u: { // Sphere
            // 구면 균등 분포
            let theta = random_range(seed, 0.0, 6.28318);
            let phi = acos(random_range(seed + 1u, -1.0, 1.0));
            let r = pow(hash(seed + 2u), 1.0 / 3.0) * config.spawn_extent.x;
            return config.emitter_position + vec3<f32>(
                r * sin(phi) * cos(theta),
                r * sin(phi) * sin(theta),
                r * cos(phi)
            );
        }
        case 3u: { // Cone
            let angle = random_range(seed, 0.0, config.cone_angle);
            let theta = random_range(seed + 1u, 0.0, 6.28318);
            let dir = vec3<f32>(
                sin(angle) * cos(theta),
                cos(angle),
                sin(angle) * sin(theta)
            );
            return config.emitter_position + dir * random_range(seed + 2u, 0.0, config.spawn_extent.x);
        }
        default: {
            return config.emitter_position;
        }
    }
}

// 초기 속도 계산 (형태별)
fn calculate_initial_velocity(seed: u32, spawn_pos: vec3<f32>) -> vec3<f32> {
    switch config.spawn_shape {
        case 2u: { // Sphere - 방사형 속도
            let dir = normalize(spawn_pos - config.emitter_position);
            let speed = random_range(seed, length(config.velocity_min), length(config.velocity_max));
            return dir * speed;
        }
        case 3u: { // Cone - 원뿔 방향 속도
            let dir = normalize(spawn_pos - config.emitter_position);
            let speed = random_range(seed, length(config.velocity_min), length(config.velocity_max));
            return dir * speed;
        }
        default: { // Point, Box - 랜덤 속도
            return random_vec3_range(seed, config.velocity_min, config.velocity_max);
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let local_idx = global_id.x;

    // 스폰 범위 체크
    if local_idx >= config.spawn_count {
        return;
    }

    // 파티클 버퍼 인덱스 계산
    let particle_idx = (config.spawn_start_index + local_idx) % config.total_particles;

    // 랜덤 시드 생성
    let seed = u32(config.random_seed * 1000000.0) + local_idx * 7919u + particle_idx * 6997u;

    // 파티클 초기화
    var particle: GpuParticle;

    particle.position = calculate_spawn_position(seed);
    particle.velocity = calculate_initial_velocity(seed + 100u, particle.position);

    particle.lifetime = 0.0;
    particle.max_lifetime = random_range(seed + 200u, config.lifetime_min, config.lifetime_max);

    particle.color = config.color_start;
    particle.size = random_range(seed + 300u, config.size_min, config.size_max);
    particle.rotation = random_range(seed + 400u, 0.0, 6.28318);
    particle.rotation_speed = random_range(seed + 500u, config.rotation_speed_min, config.rotation_speed_max);

    particle.alive = 1u;
    particle.seed = seed;
    particle._pad = vec3<f32>(0.0);

    particles[particle_idx] = particle;
}
