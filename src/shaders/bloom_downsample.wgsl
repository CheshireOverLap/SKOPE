// SKOPE Engine - Bloom Downsample Shader
// Dual Kawase Downsample

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let output_size = textureDimensions(output_tex);
    if (gid.x >= output_size.x || gid.y >= output_size.y) {
        return;
    }

    let input_size = vec2<f32>(textureDimensions(input_tex));
    let texel_size = 1.0 / input_size;

    // 출력 픽셀의 UV (입력 텍스처 기준)
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(output_size);

    // Dual Kawase Downsample: 5개 샘플
    let offset = texel_size * 0.5;

    var color = textureSampleLevel(input_tex, tex_sampler, uv, 0.0).rgb * 4.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x, -offset.y), 0.0).rgb;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x, -offset.y), 0.0).rgb;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x,  offset.y), 0.0).rgb;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x,  offset.y), 0.0).rgb;
    color = color / 8.0;

    textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(color, 1.0));
}
