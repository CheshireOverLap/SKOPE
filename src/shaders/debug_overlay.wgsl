// SKOPE Engine — Debug Visualization Overlay
//
// Fullscreen compute shader that reads from various render targets
// and produces a debug visualization overlay. The debug_mode uniform
// selects which buffer to visualize.
//
// Modes:
//   0 = Off (passthrough)
//   1 = Depth (linear grayscale)
//   2 = Normals (RGB = XYZ)
//   3 = Motion Vectors (RG direction, B = magnitude)
//   4 = VSM Shadow Factor
//   5 = VSM Clipmap Level (color-coded)
//   6 = MegaLights Tile Count (heatmap)
//   7 = TSR Rejection Mask
//   8 = TSR Thin Geometry
//   9 = DF Shadows
//  10 = DF AO
//  11 = DBuffer Albedo
//  12 = DBuffer Normal
//  13 = Lumen Screen Probes
//  14 = Aerial Perspective
//  15 = Profiler Overlay (timing bars)

struct DebugParams {
    screen_width:  u32,
    screen_height: u32,
    debug_mode:    u32,
    near_plane:    f32,
    far_plane:     f32,
    _pad:          vec3<f32>,
};

@group(0) @binding(0) var<uniform> params: DebugParams;
@group(0) @binding(1) var input_color: texture_2d<f32>;
@group(0) @binding(2) var debug_tex_a: texture_2d<f32>;    // Primary debug texture
@group(0) @binding(3) var debug_tex_b: texture_depth_2d;   // Depth texture
@group(0) @binding(4) var output: texture_storage_2d<rgba8unorm, write>;

// Turbo colormap (blue → cyan → green → yellow → red)
fn turbo_colormap(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    let r = clamp(1.0 - abs(x - 0.75) * 4.0, 0.0, 1.0);
    let g = clamp(1.0 - abs(x - 0.5) * 4.0, 0.0, 1.0);
    let b = clamp(1.0 - abs(x - 0.25) * 4.0, 0.0, 1.0);
    return vec3<f32>(r, g, b);
}

// Heat map (black → blue → red → yellow → white)
fn heatmap(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    if x < 0.25 {
        return mix(vec3<f32>(0.0), vec3<f32>(0.0, 0.0, 1.0), x * 4.0);
    } else if x < 0.5 {
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), (x - 0.25) * 4.0);
    } else if x < 0.75 {
        return mix(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), (x - 0.5) * 4.0);
    } else {
        return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 1.0), (x - 0.75) * 4.0);
    }
}

// Color per index (for clipmap level, cluster ID, etc.)
fn index_color(idx: u32) -> vec3<f32> {
    let colors = array<vec3<f32>, 8>(
        vec3<f32>(1.0, 0.2, 0.2),  // Red
        vec3<f32>(0.2, 1.0, 0.2),  // Green
        vec3<f32>(0.2, 0.2, 1.0),  // Blue
        vec3<f32>(1.0, 1.0, 0.2),  // Yellow
        vec3<f32>(1.0, 0.2, 1.0),  // Magenta
        vec3<f32>(0.2, 1.0, 1.0),  // Cyan
        vec3<f32>(1.0, 0.6, 0.2),  // Orange
        vec3<f32>(0.6, 0.2, 1.0),  // Purple
    );
    return colors[idx % 8u];
}

fn linearize_depth(d: f32, near: f32, far: f32) -> f32 {
    // Reverse-Z perspective depth to linear [0, 1]
    let z = d;
    if z <= 0.0 { return 1.0; }
    return near / (z * (far - near) + near);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    let color = textureLoad(input_color, pixel, 0);

    var out_color = color.rgb;

    switch params.debug_mode {
        // Mode 0: Passthrough
        case 0u: {
            out_color = color.rgb;
        }

        // Mode 1: Linear depth
        case 1u: {
            let d = textureLoad(debug_tex_b, pixel, 0);
            let linear = linearize_depth(d, params.near_plane, params.far_plane);
            out_color = vec3<f32>(linear);
        }

        // Mode 2: Normals (from debug_tex_a RG → world normal)
        case 2u: {
            let packed = textureLoad(debug_tex_a, pixel, 0).rg;
            let n = packed * 2.0 - 1.0;
            let z = sqrt(max(1.0 - n.x * n.x - n.y * n.y, 0.0));
            out_color = vec3<f32>(n.x, n.y, z) * 0.5 + 0.5;
        }

        // Mode 3: Motion vectors
        case 3u: {
            let mv = textureLoad(debug_tex_a, pixel, 0).rg;
            let mag = length(mv) * 10.0;
            out_color = vec3<f32>(abs(mv.x) * 10.0, abs(mv.y) * 10.0, mag);
        }

        // Mode 4: VSM shadow factor (grayscale from debug_tex_a.r)
        case 4u: {
            let shadow = textureLoad(debug_tex_a, pixel, 0).r;
            out_color = vec3<f32>(shadow);
        }

        // Mode 5: VSM clipmap level (index color from debug_tex_a.r)
        case 5u: {
            let level = u32(textureLoad(debug_tex_a, pixel, 0).r * 8.0);
            out_color = index_color(level);
        }

        // Mode 6: MegaLights tile count / DF shadows / general heatmap
        case 6u: {
            let val = textureLoad(debug_tex_a, pixel, 0).r;
            out_color = heatmap(val);
        }

        // Mode 7: TSR rejection mask (red = rejected)
        case 7u: {
            let rejection = textureLoad(debug_tex_a, pixel, 0).r;
            out_color = mix(color.rgb, vec3<f32>(1.0, 0.0, 0.0), rejection);
        }

        // Mode 8: TSR thin geometry (cyan = thin)
        case 8u: {
            let thin = textureLoad(debug_tex_a, pixel, 0).r;
            out_color = mix(color.rgb, vec3<f32>(0.0, 1.0, 1.0), thin);
        }

        // Mode 9: Grayscale from debug_tex_a.r (DF shadows, DF AO, etc.)
        case 9u: {
            let val = textureLoad(debug_tex_a, pixel, 0).r;
            out_color = vec3<f32>(val);
        }

        // Mode 10: DFAO
        case 10u: {
            let ao = textureLoad(debug_tex_a, pixel, 0).r;
            out_color = turbo_colormap(1.0 - ao);
        }

        // Mode 11: DBuffer albedo (RGB direct)
        case 11u: {
            let dbuf = textureLoad(debug_tex_a, pixel, 0);
            let alpha = dbuf.a;
            out_color = mix(color.rgb, dbuf.rgb, alpha);
        }

        // Mode 12: DBuffer normal (tangent-space → color)
        case 12u: {
            let n = textureLoad(debug_tex_a, pixel, 0).rgb;
            out_color = n * 0.5 + 0.5;
        }

        // Mode 13: Lumen screen probes (false color)
        case 13u: {
            let irr = textureLoad(debug_tex_a, pixel, 0).rgb;
            let lum = dot(irr, vec3<f32>(0.2126, 0.7152, 0.0722));
            out_color = turbo_colormap(lum);
        }

        // Mode 14: Aerial perspective (fog contribution)
        case 14u: {
            let fog = textureLoad(debug_tex_a, pixel, 0);
            out_color = fog.rgb + color.rgb * fog.a;
        }

        // Mode 15: Profiler overlay (draw timing bars)
        // This mode just tints the bottom of the screen as a placeholder;
        // the actual timing bar rendering happens on the CPU/UI side.
        case 15u: {
            let uv_y = f32(gid.y) / f32(params.screen_height);
            if uv_y > 0.9 {
                let bar_t = (uv_y - 0.9) * 10.0;
                let bar_color = heatmap(bar_t);
                out_color = mix(color.rgb * 0.3, bar_color, 0.7);
            } else {
                out_color = color.rgb;
            }
        }

        default: {
            out_color = color.rgb;
        }
    }

    textureStore(output, pixel, vec4<f32>(out_color, 1.0));
}
