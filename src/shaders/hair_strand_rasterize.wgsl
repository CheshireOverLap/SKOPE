// SKOPE Engine - Strand Rasterization Shader
// Phase 12: GPU-based hair strand rendering with tube expansion

struct StrandVertex {
    position: vec3<f32>,
    thickness: f32,
    velocity: vec3<f32>,
    _pad: f32,
}

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
}

struct StrandShadeParams {
    base_color: vec3<f32>,
    roughness: f32,
    specular_intensity: f32,
    ambient_occlusion: f32,
    _pad: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_tangent: vec3<f32>,
    @location(2) t_coord: f32,  // 0=root, 1=tip
    @location(3) radial_coord: f32,  // 0-1 around tube
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> shade_params: StrandShadeParams;
@group(0) @binding(2) var<storage, read> strand_vertices: array<StrandVertex>;
@group(0) @binding(3) var<uniform> segments_per_strand: u32;
@group(0) @binding(4) var<uniform> strand_count: u32;

// Tube expansion: 각 strand segment를 quad로 확장
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Instance = strand, vertex = segment quad의 정점
    // 각 segment는 4개의 정점 (2 triangles)
    let vertices_per_strand = segments_per_strand + 1u;
    let quads_per_strand = segments_per_strand;
    let verts_per_quad = 6u;  // 2 triangles

    let strand_id = instance_idx;
    let local_vert = vertex_idx % (quads_per_strand * verts_per_quad);
    let quad_idx = local_vert / verts_per_quad;
    let vert_in_quad = local_vert % verts_per_quad;

    // Segment의 시작/끝 정점
    let seg_start_idx = strand_id * vertices_per_strand + quad_idx;
    let seg_end_idx = seg_start_idx + 1u;

    // Clamp
    if (strand_id >= strand_count || quad_idx >= quads_per_strand) {
        out.clip_position = vec4<f32>(0.0, 0.0, -2.0, 1.0);  // 클리핑
        return out;
    }

    let v0 = strand_vertices[seg_start_idx];
    let v1 = strand_vertices[seg_end_idx];

    // Quad 정점 위치 결정 (0-5: 두 삼각형)
    // Triangle 1: 0, 1, 2 -> left-bottom, right-bottom, left-top
    // Triangle 2: 3, 4, 5 -> right-bottom, right-top, left-top
    var t: f32;  // 0=start, 1=end
    var side: f32;  // -1=left, 1=right

    switch (vert_in_quad) {
        case 0u: { t = 0.0; side = -1.0; }
        case 1u: { t = 0.0; side = 1.0; }
        case 2u: { t = 1.0; side = -1.0; }
        case 3u: { t = 0.0; side = 1.0; }
        case 4u: { t = 1.0; side = 1.0; }
        case 5u: { t = 1.0; side = -1.0; }
        default: { t = 0.0; side = 0.0; }
    }

    // 위치 보간
    let pos = mix(v0.position, v1.position, t);
    let thickness = mix(v0.thickness, v1.thickness, t);

    // Tangent 계산
    let tangent = normalize(v1.position - v0.position);

    // Billboard: 카메라를 향하는 방향으로 확장
    let to_camera = normalize(camera.camera_pos - pos);
    let right = normalize(cross(tangent, to_camera));

    // 확장된 위치
    let expanded_pos = pos + right * side * thickness;

    out.clip_position = camera.view_proj * vec4<f32>(expanded_pos, 1.0);
    out.world_position = expanded_pos;
    out.world_tangent = tangent;
    out.t_coord = (f32(quad_idx) + t) / f32(quads_per_strand);
    out.radial_coord = (side + 1.0) * 0.5;

    return out;
}

// Kajiya-Kay diffuse
fn kajiya_diffuse(tangent: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    let TdotL = dot(tangent, light_dir);
    let sin_TL = sqrt(max(1.0 - TdotL * TdotL, 0.0));
    return sin_TL;
}

// Kajiya-Kay specular
fn kajiya_specular(
    tangent: vec3<f32>,
    view_dir: vec3<f32>,
    light_dir: vec3<f32>,
    exponent: f32,
) -> f32 {
    let TdotL = dot(tangent, light_dir);
    let TdotV = dot(tangent, view_dir);

    let sin_TL = sqrt(max(1.0 - TdotL * TdotL, 0.0));
    let sin_TV = sqrt(max(1.0 - TdotV * TdotV, 0.0));

    var spec = sin_TL * sin_TV - TdotL * TdotV;
    spec = pow(max(spec, 0.0), exponent);

    return spec;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 임시 조명 방향
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let view_dir = normalize(camera.camera_pos - in.world_position);
    let light_color = vec3<f32>(1.0, 0.98, 0.95);

    let tangent = normalize(in.world_tangent);

    // Diffuse
    let diff = kajiya_diffuse(tangent, light_dir);
    let diffuse = shade_params.base_color * diff * light_color * 0.6;

    // Specular (dual highlights)
    let spec1 = kajiya_specular(tangent, view_dir, light_dir, 80.0);
    let spec2 = kajiya_specular(tangent, view_dir, light_dir, 20.0);
    let specular = (spec1 * 0.8 + spec2 * 0.3) * shade_params.specular_intensity * light_color;

    // Root darkening (AO simulation)
    let ao = mix(shade_params.ambient_occlusion, 1.0, in.t_coord);

    // Ambient
    let ambient = shade_params.base_color * 0.1;

    // 최종 색상
    let final_color = (ambient + diffuse + specular) * ao;

    // Alpha: 팁으로 갈수록 페이드 아웃
    let alpha = smoothstep(1.0, 0.8, in.t_coord);

    // Radial gradient (튜브 가장자리 페이드)
    let radial_fade = 1.0 - pow(abs(in.radial_coord - 0.5) * 2.0, 2.0);
    let final_alpha = alpha * radial_fade;

    return vec4<f32>(final_color, final_alpha);
}
