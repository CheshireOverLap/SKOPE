// SKOPE Engine — Hair Environment Lighting Integration
//
// Integrates Lumen GI / IBL with Marschner hair shading.
// Samples screen-space irradiance from the Lumen screen probes
// (or fallback SH) and applies to hair diffuse + specular.
//
// Reference: UE5 HairStrandsEnvironmentLighting.usf

struct HairEnvParams {
    view_proj:       mat4x4<f32>,
    inv_view_proj:   mat4x4<f32>,
    camera_pos:      vec3<f32>,
    gi_intensity:    f32,
    screen_width:    u32,
    screen_height:   u32,
    use_lumen:       u32,       // 1 = use Lumen probes, 0 = fallback SH
    _pad:            u32,
    // Fallback SH coefficients (L0 + L1 = 4 vec3)
    sh_r:            vec4<f32>,
    sh_g:            vec4<f32>,
    sh_b:            vec4<f32>,
};

struct MarschnerParams {
    sigma_a:       vec3<f32>,
    alpha:         f32,
    beta_r:        f32,
    beta_tt:       f32,
    beta_trt:      f32,
    eta:           f32,
    r_intensity:   f32,
    tt_intensity:  f32,
    trt_intensity: f32,
    _pad:          f32,
};

@group(0) @binding(0) var<uniform> params: HairEnvParams;
@group(0) @binding(1) var<uniform> marschner: MarschnerParams;
@group(0) @binding(2) var hair_color_tex: texture_2d<f32>;       // Hair shading (direct light only)
@group(0) @binding(3) var hair_normal_tex: texture_2d<f32>;      // Hair tangent/normal
@group(0) @binding(4) var hair_depth_tex: texture_depth_2d;      // Hair depth
@group(0) @binding(5) var lumen_irradiance: texture_2d<f32>;     // Lumen screen irradiance
@group(0) @binding(6) var env_sampler: sampler;
@group(0) @binding(7) var output: texture_storage_2d<rgba16float, write>;

fn reconstruct_world_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_h = params.inv_view_proj * ndc;
    return world_h.xyz / world_h.w;
}

// Evaluate SH irradiance (L0 + L1 band)
fn eval_sh(normal: vec3<f32>) -> vec3<f32> {
    let r = params.sh_r.x + params.sh_r.y * normal.y + params.sh_r.z * normal.z + params.sh_r.w * normal.x;
    let g = params.sh_g.x + params.sh_g.y * normal.y + params.sh_g.z * normal.z + params.sh_g.w * normal.x;
    let b = params.sh_b.x + params.sh_b.y * normal.y + params.sh_b.z * normal.z + params.sh_b.w * normal.x;
    return max(vec3<f32>(r, g, b), vec3<f32>(0.0));
}

// Hair diffuse wrap lighting (softer than Lambertian)
fn hair_diffuse_wrap(normal: vec3<f32>, light_dir: vec3<f32>, wrap: f32) -> f32 {
    let ndotl = dot(normal, light_dir);
    return max((ndotl + wrap) / (1.0 + wrap), 0.0);
}

// Simplified Marschner for environment (hemisphere integral approximation)
fn marschner_env_specular(view_dir: vec3<f32>, tangent: vec3<f32>, roughness: f32) -> f32 {
    // For environment lighting, use a simplified specular response
    // based on the tangent-space half angle
    let t_dot_v = abs(dot(tangent, view_dir));
    let sin_theta = sqrt(max(1.0 - t_dot_v * t_dot_v, 0.0));
    let spec = pow(sin_theta, 1.0 / max(roughness, 0.01));
    return spec * 0.3; // Scale down for energy conservation
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(f32(params.screen_width), f32(params.screen_height));

    // Read hair data
    let direct_color = textureLoad(hair_color_tex, pixel, 0).rgb;
    let depth = textureLoad(hair_depth_tex, pixel, 0);

    // Skip sky pixels
    if depth >= 1.0 {
        textureStore(output, pixel, vec4<f32>(direct_color, 1.0));
        return;
    }

    let tangent_packed = textureLoad(hair_normal_tex, pixel, 0).rgb;
    let tangent = normalize(tangent_packed * 2.0 - 1.0);

    // Construct a pseudo-normal from tangent (perpendicular to tangent, facing camera)
    let world_pos = reconstruct_world_pos(uv, depth);
    let view_dir = normalize(params.camera_pos - world_pos);
    let bitangent = normalize(cross(tangent, view_dir));
    let normal = normalize(cross(bitangent, tangent));

    // Get irradiance
    var irradiance: vec3<f32>;
    if params.use_lumen == 1u {
        // Sample Lumen screen irradiance at this pixel
        irradiance = textureSampleLevel(lumen_irradiance, env_sampler, uv, 0.0).rgb;
    } else {
        // Fallback: evaluate SH
        irradiance = eval_sh(normal);
    }

    // Hair diffuse (wrapped)
    let hair_diffuse = irradiance * 0.6; // Hair diffuse is weaker than opaque surfaces

    // Hair specular (environment)
    let roughness = marschner.beta_r / 90.0; // Normalize to [0, 1]
    let env_spec = marschner_env_specular(view_dir, tangent, roughness);
    let hair_specular = irradiance * env_spec;

    // Absorption tint
    let absorption = exp(-vec3<f32>(marschner.sigma_a) * 0.5);

    // Combine
    let indirect = (hair_diffuse * absorption + hair_specular) * params.gi_intensity;
    let final_color = direct_color + indirect;

    textureStore(output, pixel, vec4<f32>(final_color, 1.0));
}
