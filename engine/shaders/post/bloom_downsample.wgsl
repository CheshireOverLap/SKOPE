// SKOPE Engine - Bloom Downsample Shader
// COD:AW Style 13-tap Karis Filter
// Reference: Call of Duty: Advanced Warfare (SIGGRAPH 2014)

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;

// Karis average to prevent fireflies (bright pixel pulsing)
// Uses luma-weighted average to reduce influence of extremely bright pixels
fn karis_average(color: vec3<f32>) -> f32 {
    // Rec. 709 luma coefficients
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    // Weight based on inverse luma (bright pixels get lower weight)
    return 1.0 / (1.0 + luma);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let output_size = textureDimensions(output_tex);
    if (gid.x >= output_size.x || gid.y >= output_size.y) {
        return;
    }

    let input_size = vec2<f32>(textureDimensions(input_tex));
    let texel_size = 1.0 / input_size;

    // Output pixel UV (in input texture space)
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(output_size);

    // 13-tap filter pattern (COD:AW)
    // Samples are arranged in a specific pattern for optimal quality
    //
    //     [a]   [b]
    //   [c] [d] [e] [f]
    //     [g]   [h]
    //   [i] [j] [k] [l]
    //     [m]   [n]  <- not used in 13-tap, but shows the pattern
    //
    // Actually, the 13-tap uses this pattern:
    //   a . b . c
    //   . d . e .
    //   f . g . h
    //   . i . j .
    //   k . l . m

    // Sample 13 points
    let a = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-2.0, -2.0) * texel_size, 0.0).rgb;
    let b = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 0.0, -2.0) * texel_size, 0.0).rgb;
    let c = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 2.0, -2.0) * texel_size, 0.0).rgb;
    let d = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-1.0, -1.0) * texel_size, 0.0).rgb;
    let e = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 1.0, -1.0) * texel_size, 0.0).rgb;
    let f = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-2.0,  0.0) * texel_size, 0.0).rgb;
    let g = textureSampleLevel(input_tex, tex_sampler, uv,                                      0.0).rgb;
    let h = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 2.0,  0.0) * texel_size, 0.0).rgb;
    let i = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-1.0,  1.0) * texel_size, 0.0).rgb;
    let j = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 1.0,  1.0) * texel_size, 0.0).rgb;
    let k = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>(-2.0,  2.0) * texel_size, 0.0).rgb;
    let l = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 0.0,  2.0) * texel_size, 0.0).rgb;
    let m = textureSampleLevel(input_tex, tex_sampler, uv + vec2<f32>( 2.0,  2.0) * texel_size, 0.0).rgb;

    // Apply Karis average weights to prevent fireflies
    // Group samples into 5 boxes and weight by luma
    // Center box (d, e, i, j) - weight 0.5
    let center_weight = karis_average(d + e + i + j);
    let center = (d + e + i + j) * 0.25 * center_weight;

    // Top-left box (a, b, d, g) - weight 0.125
    let tl_weight = karis_average(a + b + d + g);
    let top_left = (a + b + d + g) * 0.25 * tl_weight;

    // Top-right box (b, c, e, h) - weight 0.125
    let tr_weight = karis_average(b + c + e + h);
    let top_right = (b + c + e + h) * 0.25 * tr_weight;

    // Bottom-left box (f, i, k, l) - weight 0.125
    let bl_weight = karis_average(f + i + k + l);
    let bottom_left = (f + i + k + l) * 0.25 * bl_weight;

    // Bottom-right box (g, j, l, m) - weight 0.125
    let br_weight = karis_average(g + j + l + m);
    let bottom_right = (g + j + l + m) * 0.25 * br_weight;

    // Combine with normalized weights
    let total_weight = center_weight * 0.5 + (tl_weight + tr_weight + bl_weight + br_weight) * 0.125;
    var color = center * 0.5 + (top_left + top_right + bottom_left + bottom_right) * 0.125;
    color = color / max(total_weight, 0.0001);

    textureStore(output_tex, vec2<i32>(gid.xy), vec4<f32>(color, 1.0));
}
