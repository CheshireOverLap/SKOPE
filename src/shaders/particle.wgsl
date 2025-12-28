// SKOPE Particle Billboard Shader
// GPU instanced billboard rendering

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    // Per-vertex (quad corner)
    @location(4) corner: vec2<f32>,
}

struct InstanceInput {
    // Per-instance
    @location(0) position: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
    @location(3) rotation: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Billboard: face camera
    let right = vec3<f32>(camera.view[0][0], camera.view[1][0], camera.view[2][0]);
    let up = vec3<f32>(camera.view[0][1], camera.view[1][1], camera.view[2][1]);

    // Apply rotation
    let cos_r = cos(instance.rotation);
    let sin_r = sin(instance.rotation);
    let rotated_corner = vec2<f32>(
        vertex.corner.x * cos_r - vertex.corner.y * sin_r,
        vertex.corner.x * sin_r + vertex.corner.y * cos_r
    );

    // Calculate world position
    let world_pos = instance.position
        + right * rotated_corner.x * instance.size
        + up * rotated_corner.y * instance.size;

    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = vertex.corner * 0.5 + 0.5; // [-1,1] -> [0,1]
    out.color = instance.color;

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

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
