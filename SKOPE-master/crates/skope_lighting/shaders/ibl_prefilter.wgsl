// SKOPE Engine - IBL Prefilter Compute Shader
// Prefiltered Environment Map + Irradiance Convolution

struct PrefilterParams {
    roughness: f32,
    face_size: f32,
    sample_count: u32,
    mip_level: u32,
}

@group(0) @binding(0) var input_cube: texture_cube<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var<uniform> params: PrefilterParams;

const PI: f32 = 3.14159265359;

// =============================================================================
// Helper Functions
// =============================================================================

fn radical_inverse_vdc(bits: u32) -> f32 {
    var b = bits;
    b = (b << 16u) | (b >> 16u);
    b = ((b & 0x55555555u) << 1u) | ((b & 0xAAAAAAAAu) >> 1u);
    b = ((b & 0x33333333u) << 2u) | ((b & 0xCCCCCCCCu) >> 2u);
    b = ((b & 0x0F0F0F0Fu) << 4u) | ((b & 0xF0F0F0F0u) >> 4u);
    b = ((b & 0x00FF00FFu) << 8u) | ((b & 0xFF00FF00u) >> 8u);
    return f32(b) * 2.3283064365386963e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

fn importance_sample_ggx(xi: vec2<f32>, n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;

    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    // Tangent space
    let h = vec3<f32>(
        cos(phi) * sin_theta,
        sin(phi) * sin_theta,
        cos_theta
    );

    // Tangent to world
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);

    return normalize(tangent * h.x + bitangent * h.y + n * h.z);
}

fn cube_direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let u = uv.x * 2.0 - 1.0;
    let v = uv.y * 2.0 - 1.0;

    switch (face) {
        case 0u: { return normalize(vec3<f32>(1.0, -v, -u)); }  // +X
        case 1u: { return normalize(vec3<f32>(-1.0, -v, u)); }  // -X
        case 2u: { return normalize(vec3<f32>(u, 1.0, v)); }    // +Y
        case 3u: { return normalize(vec3<f32>(u, -1.0, -v)); }  // -Y
        case 4u: { return normalize(vec3<f32>(u, -v, 1.0)); }   // +Z
        default: { return normalize(vec3<f32>(-u, -v, -1.0)); } // -Z
    }
}

// =============================================================================
// Specular Prefilter
// =============================================================================

@compute @workgroup_size(8, 8, 1)
fn prefilter_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let face = gid.z;
    let size = u32(params.face_size);

    if (gid.x >= size || gid.y >= size || face >= 6u) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / f32(size);
    let n = cube_direction(face, uv);
    let r = n;
    let v = r;

    var prefiltered_color = vec3<f32>(0.0);
    var total_weight = 0.0;

    let roughness = params.roughness;

    // Roughness 0 = mirror reflection
    if (roughness < 0.01) {
        prefiltered_color = textureSampleLevel(input_cube, tex_sampler, n, 0.0).rgb;
    } else {
        for (var i = 0u; i < params.sample_count; i++) {
            let xi = hammersley(i, params.sample_count);
            let h = importance_sample_ggx(xi, n, roughness);
            let l = normalize(2.0 * dot(v, h) * h - v);

            let n_dot_l = max(dot(n, l), 0.0);

            if (n_dot_l > 0.0) {
                // Mipmap level based on PDF
                let n_dot_h = max(dot(n, h), 0.0);
                let h_dot_v = max(dot(h, v), 0.0);

                let a = roughness * roughness;
                let d = (n_dot_h * n_dot_h * (a * a - 1.0) + 1.0);
                let pdf = (a * a * n_dot_h) / (PI * d * d * 4.0 * h_dot_v) + 0.0001;

                let sa_sample = 1.0 / (f32(params.sample_count) * pdf + 0.0001);
                let sa_texel = 4.0 * PI / (6.0 * params.face_size * params.face_size);
                let mip = select(0.5 * log2(sa_sample / sa_texel), 0.0, roughness == 0.0);

                prefiltered_color += textureSampleLevel(input_cube, tex_sampler, l, mip).rgb * n_dot_l;
                total_weight += n_dot_l;
            }
        }

        prefiltered_color = prefiltered_color / max(total_weight, 0.001);
    }

    textureStore(output_tex, vec2<i32>(gid.xy), i32(face), vec4<f32>(prefiltered_color, 1.0));
}

// =============================================================================
// Irradiance Convolution
// =============================================================================

@compute @workgroup_size(8, 8, 1)
fn irradiance_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let face = gid.z;
    let size = u32(params.face_size);

    if (gid.x >= size || gid.y >= size || face >= 6u) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + 0.5) / f32(size);
    let n = cube_direction(face, uv);

    // Tangent space
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let right = normalize(cross(up, n));
    let up_vec = cross(n, right);

    var irradiance = vec3<f32>(0.0);
    var sample_count = 0.0;

    // Hemisphere sampling
    let sample_delta = 0.025;

    var phi = 0.0;
    while (phi < 2.0 * PI) {
        var theta = 0.0;
        while (theta < 0.5 * PI) {
            // Spherical to cartesian (tangent space)
            let tangent_sample = vec3<f32>(
                sin(theta) * cos(phi),
                sin(theta) * sin(phi),
                cos(theta)
            );

            // Tangent to world
            let sample_vec = tangent_sample.x * right + tangent_sample.y * up_vec + tangent_sample.z * n;

            irradiance += textureSampleLevel(input_cube, tex_sampler, sample_vec, 0.0).rgb * cos(theta) * sin(theta);
            sample_count += 1.0;

            theta += sample_delta;
        }
        phi += sample_delta;
    }

    irradiance = PI * irradiance / sample_count;

    textureStore(output_tex, vec2<i32>(gid.xy), i32(face), vec4<f32>(irradiance, 1.0));
}
