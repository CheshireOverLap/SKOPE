// SKOPE Engine - Bloom Upsample Shader
// COD:AW Style 9-tap Tent Filter
// Reference: Call of Duty: Advanced Warfare (SIGGRAPH 2014)

struct BloomUpsampleParams {
    blend_factor: f32,
    radius: f32,      // Bloom radius multiplier (1.0 = standard)
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var blend_tex: texture_2d<f32>;  // Previous MIP (higher res)
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> params: BloomUpsampleParams;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let output_size = textureDimensions(output_tex);
    if (gid.x >= output_size.x || gid.y >= output_size.y) {
        return;
    }

    let input_size = vec2<f32>(textureDimensions(input_tex));
    let texel_size = 1.0 / input_size;
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(output_size);

    // Apply radius multiplier
    let offset = texel_size * params.radius;

    // 9-tap tent filter (3x3 weighted)
    // Weights:
    //   1  2  1
    //   2  4  2  / 16
    //   1  2  1
    var color = vec3<f32>(0.0);

    // Corner samples (weight 1)
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x, -offset.y), 0.0).rgb * 1.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x, -offset.y), 0.0).rgb * 1.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x,  offset.y), 0.0).rgb * 1.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x,  offset.y), 0.0).rgb * 1.0;

    // Edge samples (weight 2)
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 0.0,      -offset.y), 0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-offset.x,  0.0),      0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( offset.x,  0.0),      0.0).rgb * 2.0;
    color += textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 0.0,       offset.y), 0.0).rgb * 2.0;

    // Center sample (weight 4)
    color += textureSampleLevel(input_tex, tex_sampler, uv, 0.0).rgb * 4.0;

    // Normalize (1+2+1 + 2+4+2 + 1+2+1 = 16)
    color = color / 16.0;

    // Additive blend with previous MIP level (progressive accumulation)
    let prev_color = textureSampleLevel(blend_tex, tex_sampler, uv, 0.0).rgb;
    color = prev_color + color * params.blend_factor;

    textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(color, 1.0));
}
