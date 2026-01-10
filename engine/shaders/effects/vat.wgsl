// SKOPE Engine - VAT (Vertex Animation Texture) Shader
// Supports Soft, Rigid, and Fluid animation types

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
}

struct ModelUniform {
    model: mat4x4<f32>,
    model_inv_transpose: mat4x4<f32>,
}

struct VatUniforms {
    bbox_min: vec3<f32>,
    _pad0: f32,
    bbox_max: vec3<f32>,
    _pad1: f32,
    frame_count: u32,
    vertex_count: u32,
    vat_type: u32,    // 0=Soft, 1=Rigid, 2=Fluid
    _pad2: u32,
    current_frame: f32,
    frame_blend: f32,
    _pad3: vec2<f32>,
}

struct LightUniform {
    sun_direction: vec3<f32>,
    _pad0: f32,
    sun_color: vec3<f32>,
    sun_intensity: f32,
    ambient_color: vec3<f32>,
    ambient_intensity: f32,
}

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) base_position: vec3<f32>,
    @location(1) base_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> model: ModelUniform;

@group(0) @binding(2)
var<uniform> vat: VatUniforms;

@group(0) @binding(3)
var<uniform> light: LightUniform;

@group(0) @binding(4)
var position_texture: texture_2d<f32>;

@group(0) @binding(5)
var normal_texture: texture_2d<f32>;

@group(0) @binding(6)
var vat_sampler: sampler;

// Sample VAT texture at specific vertex and frame
fn sample_vat_position(vertex_idx: u32, frame: f32) -> vec3<f32> {
    let tex_size = textureDimensions(position_texture);

    // X = vertex index, Y = frame
    let x = i32(vertex_idx % tex_size.x);
    let y = i32(frame) % i32(tex_size.y);

    let texel = textureLoad(position_texture, vec2<i32>(x, y), 0);

    // Denormalize from [0,1] to bbox
    let bbox_size = vat.bbox_max - vat.bbox_min;
    return vat.bbox_min + texel.rgb * bbox_size;
}

fn sample_vat_normal(vertex_idx: u32, frame: f32) -> vec3<f32> {
    let tex_size = textureDimensions(normal_texture);

    let x = i32(vertex_idx % tex_size.x);
    let y = i32(frame) % i32(tex_size.y);

    let texel = textureLoad(normal_texture, vec2<i32>(x, y), 0);

    // Normals stored as [0,1] -> [-1,1]
    return normalize(texel.rgb * 2.0 - 1.0);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let frame_current = floor(vat.current_frame);
    let frame_next = floor(vat.current_frame + 1.0);
    let blend = vat.frame_blend;

    var position: vec3<f32>;
    var normal: vec3<f32>;

    // Sample position from VAT
    if vat.vat_type == 0u {
        // Soft body: direct vertex displacement
        let pos_current = sample_vat_position(in.vertex_index, frame_current);
        let pos_next = sample_vat_position(in.vertex_index, frame_next);
        position = mix(pos_current, pos_next, blend);

        let norm_current = sample_vat_normal(in.vertex_index, frame_current);
        let norm_next = sample_vat_normal(in.vertex_index, frame_next);
        normal = normalize(mix(norm_current, norm_next, blend));

    } else if vat.vat_type == 1u {
        // Rigid body: base position + offset
        let offset_current = sample_vat_position(in.vertex_index, frame_current);
        let offset_next = sample_vat_position(in.vertex_index, frame_next);
        let offset = mix(offset_current, offset_next, blend);

        position = in.base_position + offset;
        normal = in.base_normal; // Rigid keeps original normal

    } else {
        // Fluid: full replacement
        let pos_current = sample_vat_position(in.vertex_index, frame_current);
        let pos_next = sample_vat_position(in.vertex_index, frame_next);
        position = mix(pos_current, pos_next, blend);

        let norm_current = sample_vat_normal(in.vertex_index, frame_current);
        let norm_next = sample_vat_normal(in.vertex_index, frame_next);
        normal = normalize(mix(norm_current, norm_next, blend));
    }

    // Transform to world space
    let world_pos = model.model * vec4<f32>(position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;

    // Transform normal
    out.world_normal = normalize((model.model_inv_transpose * vec4<f32>(normal, 0.0)).xyz);

    out.uv = in.uv;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple PBR-ish shading
    let view_dir = normalize(camera.camera_pos - in.world_position);
    let light_dir = normalize(-light.sun_direction);
    let half_dir = normalize(view_dir + light_dir);

    let normal = normalize(in.world_normal);

    // Diffuse
    let NdotL = max(dot(normal, light_dir), 0.0);
    let diffuse = NdotL * light.sun_color * light.sun_intensity;

    // Specular (simple Blinn-Phong)
    let NdotH = max(dot(normal, half_dir), 0.0);
    let specular = pow(NdotH, 32.0) * light.sun_color * 0.5;

    // Ambient
    let ambient = light.ambient_color * light.ambient_intensity;

    // Base color (could be from texture, for now just white)
    let base_color = vec3<f32>(0.8, 0.8, 0.8);

    let final_color = base_color * (ambient + diffuse) + specular;

    return vec4<f32>(final_color, 1.0);
}
