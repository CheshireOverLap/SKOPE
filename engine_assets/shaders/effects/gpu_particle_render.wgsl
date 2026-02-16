// GPU Particle Render Shader
// SKOPE Engine
// Storage buffer에서 직접 파티클 읽어서 빌보드 렌더링

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
}

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

struct RenderConfig {
    particle_count: u32,
    soft_particle: u32,
    depth_fade_distance: f32,
    emission_strength: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<storage, read> particles: array<GpuParticle>;
@group(1) @binding(1) var<uniform> render_config: RenderConfig;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) emission: f32,
}

// 쿼드 정점 (6개 = 2 triangles)
const QUAD_VERTICES = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0,  1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // 파티클 인덱스와 쿼드 정점 인덱스 계산
    let particle_idx = vertex_index / 6u;
    let corner_idx = vertex_index % 6u;

    // 범위 체크
    if particle_idx >= render_config.particle_count {
        out.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
        out.uv = vec2<f32>(0.0);
        out.color = vec4<f32>(0.0);
        out.emission = 0.0;
        return out;
    }

    let particle = particles[particle_idx];

    // 죽은 파티클은 렌더링 안함
    if particle.alive == 0u {
        out.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
        out.uv = vec2<f32>(0.0);
        out.color = vec4<f32>(0.0);
        out.emission = 0.0;
        return out;
    }

    let corner = QUAD_VERTICES[corner_idx];

    // Billboard: 카메라를 향하도록
    let right = vec3<f32>(camera.view[0][0], camera.view[1][0], camera.view[2][0]);
    let up = vec3<f32>(camera.view[0][1], camera.view[1][1], camera.view[2][1]);

    // 회전 적용
    let cos_r = cos(particle.rotation);
    let sin_r = sin(particle.rotation);
    let rotated_corner = vec2<f32>(
        corner.x * cos_r - corner.y * sin_r,
        corner.x * sin_r + corner.y * cos_r
    );

    // 월드 위치 계산
    let world_pos = particle.position
        + right * rotated_corner.x * particle.size
        + up * rotated_corner.y * particle.size;

    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = corner * 0.5 + 0.5; // [-1,1] -> [0,1]
    out.color = particle.color;
    out.emission = render_config.emission_strength;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Soft circle falloff
    let dist = length(in.uv - vec2<f32>(0.5));
    let alpha = 1.0 - smoothstep(0.3, 0.5, dist);

    if alpha < 0.01 {
        discard;
    }

    // Emission (additive glow)
    let color = in.color.rgb * (1.0 + in.emission * 0.5);

    return vec4<f32>(color, in.color.a * alpha);
}
