// SKOPE Engine - Screen-Space Composite Shader
//
// Applies screen-space effects to HDR color buffer:
// - GTAO (Ground Truth Ambient Occlusion)
// - Contact Shadows
// - SSR (Screen-Space Reflections)

// ============================================================
// Structures
// ============================================================

struct CompositeParams {
    screen_size: vec2<f32>,
    ao_strength: f32,
    contact_shadow_strength: f32,
    ssr_strength: f32,
    _pad: vec3<f32>,
}

// ============================================================
// Bindings
// ============================================================

@group(0) @binding(0) var<uniform> params: CompositeParams;
@group(0) @binding(1) var hdr_input: texture_2d<f32>;
@group(0) @binding(2) var gtao_texture: texture_2d<f32>;
@group(0) @binding(3) var contact_shadow_texture: texture_2d<f32>;
@group(0) @binding(4) var ssr_texture: texture_2d<f32>;
@group(0) @binding(5) var linear_sampler: sampler;
@group(0) @binding(6) var output: texture_storage_2d<rgba16float, write>;

// ============================================================
// Main
// ============================================================

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(global_id.xy);
    let screen_size = vec2<i32>(params.screen_size);

    // Bounds check
    if (pixel.x >= screen_size.x || pixel.y >= screen_size.y) {
        return;
    }

    let uv = (vec2<f32>(pixel) + 0.5) / params.screen_size;

    // Sample input textures
    let hdr_color = textureLoad(hdr_input, pixel, 0);
    let ao = textureLoad(gtao_texture, pixel, 0).r;
    let contact_shadow = textureLoad(contact_shadow_texture, pixel, 0).r;
    let ssr = textureSampleLevel(ssr_texture, linear_sampler, uv, 0.0);

    // Apply AO - affects indirect/ambient lighting
    // AO ranges from 0 (fully occluded) to 1 (no occlusion)
    let ao_factor = mix(1.0, ao, params.ao_strength);

    // Apply contact shadows - affects direct lighting
    // Contact shadow ranges from 0 (in shadow) to 1 (lit)
    let shadow_factor = mix(1.0, contact_shadow, params.contact_shadow_strength);

    // Base color with AO and contact shadows
    // AO is applied as a multiplier to the entire color (approximation)
    // In a more accurate implementation, AO would only affect ambient/indirect
    var result = hdr_color.rgb * ao_factor * shadow_factor;

    // Add SSR contribution
    // SSR alpha channel indicates reflection confidence/validity
    let ssr_weight = ssr.a * params.ssr_strength;
    result = mix(result, ssr.rgb, ssr_weight);

    // Output
    textureStore(output, pixel, vec4<f32>(result, hdr_color.a));
}
