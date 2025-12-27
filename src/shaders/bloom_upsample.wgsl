// SKOPE Engine - Bloom Upsample Shader
// Dual Kawase Upsample

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var blend_tex: texture_2d<f32>;  // 이전 MIP
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> blend_factor: f32;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let output_size = textureDimensions(output_tex);
    if (gid.x >= output_size.x || gid.y >= output_size.y) {
        return;
    }

    let input_size = vec2<f32>(textureDimensions(input_tex));
    let texel_size = 1.0 / input_size;
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(output_size);

    // Dual Kawase Upsample: 9개 샘플 (3x3 tent)
    let offset = texel_size;

    var color = vec3<f32>(0.0);
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x, -offset.y), 0.0).rgb;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 0.0,      -offset.y), 0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x, -offset.y), 0.0).rgb;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x,  0.0),      0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv,                                   0.0).rgb * 4.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x,  0.0),      0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x,  offset.y), 0.0).rgb;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 0.0,       offset.y), 0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x,  offset.y), 0.0).rgb;
    color = color / 16.0;

    // 이전 MIP와 블렌드
    let prev_color = textureSampleLevel(blend_tex, tex_sampler, uv, 0.0).rgb;
    color = mix(prev_color, color, blend_factor);

    textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(color, 1.0));
}
