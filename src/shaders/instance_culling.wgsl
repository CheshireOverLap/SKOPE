// SKOPE Engine — GPU Instance Culling
//
// Two-pass GPU-driven culling:
//   Pass 0: Frustum cull + occlusion cull against previous HZB
//   Pass 1: Re-cull against new HZB (catches objects that were occluded but are now visible)
//
// Output: a compact visible-instance list + indirect draw arguments.

// --- Include GPU Scene types (inlined for wgpu shader compilation) ---

const FLAG_VISIBLE: u32         = 1u;
const FLAG_SHADOW_CASTER: u32   = 2u;
const FLAG_MOVABLE: u32         = 4u;
const FLAG_NANITE: u32          = 8u;

struct GpuInstance {
    world_matrix:       mat4x4<f32>,
    prev_world_matrix:  mat4x4<f32>,
    bounds_center:      vec3<f32>,
    bounds_radius:      f32,
    mesh_id:            u32,
    material_id:        u32,
    flags:              u32,
    custom_data:        u32,
    vertex_offset:      u32,
    index_offset:       u32,
    index_count:        u32,
    lod_level:          u32,
    payload_offset:     u32,
    payload_stride:     u32,
    _reserved:          vec2<u32>,
};

struct GpuSceneParams {
    instance_count: u32,
    frame_index:    u32,
    _pad:           vec2<u32>,
};

// --- Culling-specific types ---

struct CullingParams {
    view_proj:          mat4x4<f32>,
    frustum_planes:     array<vec4<f32>, 6>,    // left, right, bottom, top, near, far
    camera_pos:         vec3<f32>,
    min_screen_size:    f32,                    // Minimum screen-space radius in pixels (0 = disabled)
    hzb_size:           vec2<f32>,
    near_plane:         f32,
    far_plane:          f32,
    pass_index:         u32,    // 0 = first pass (prev HZB), 1 = second pass (new HZB)
    instance_count:     u32,
    _pad:               vec2<u32>,
};

struct IndirectDrawArgs {
    vertex_count:   u32,
    instance_count: u32,
    first_vertex:   u32,
    first_instance: u32,
};

struct DrawCommand {
    index_count:    u32,
    instance_count: u32,
    first_index:    u32,
    base_vertex:    i32,
    first_instance: u32,
};

// --- Bindings ---

@group(0) @binding(0) var<storage, read> instances: array<GpuInstance>;
@group(0) @binding(1) var<uniform> params: CullingParams;
@group(0) @binding(2) var hzb_texture: texture_2d<f32>;
@group(0) @binding(3) var<storage, read_write> visible_indices: array<u32>;
@group(0) @binding(4) var<storage, read_write> draw_commands: array<DrawCommand>;
@group(0) @binding(5) var<storage, read_write> counters: array<atomic<u32>>;
// counters[0] = visible_count
// counters[1] = draw_command_count
// counters[2] = occluded_count (two-pass: instances that failed HZB in pass 0)
@group(0) @binding(6) var<storage, read_write> occluded_indices: array<u32>;

// --- Frustum culling ---

fn sphere_vs_plane(center: vec3<f32>, radius: f32, plane: vec4<f32>) -> bool {
    let dist = dot(plane.xyz, center) + plane.w;
    return dist >= -radius;
}

fn frustum_cull(center: vec3<f32>, radius: f32) -> bool {
    for (var i = 0u; i < 6u; i = i + 1u) {
        if !sphere_vs_plane(center, radius, params.frustum_planes[i]) {
            return false; // Outside this plane → culled
        }
    }
    return true; // Inside all planes → visible
}

// --- HZB occlusion test ---

fn hzb_occlusion_test(center: vec3<f32>, radius: f32) -> bool {
    // Project sphere center to clip space
    let clip = params.view_proj * vec4<f32>(center, 1.0);
    if clip.w <= 0.0 {
        return true; // Behind camera, skip occlusion test
    }

    let ndc = clip.xyz / clip.w;
    let screen_uv = ndc.xy * 0.5 + 0.5;

    // Estimate screen-space radius for HZB mip selection
    let projected_radius = radius / clip.w;
    let screen_radius = projected_radius * max(params.hzb_size.x, params.hzb_size.y) * 0.5;

    // Select HZB mip level based on screen-space size
    let mip_level = clamp(u32(log2(max(screen_radius * 2.0, 1.0))), 0u, 10u);

    // Sample HZB at the appropriate mip level
    let hzb_coord = vec2<i32>(
        i32(screen_uv.x * params.hzb_size.x) >> mip_level,
        i32(screen_uv.y * params.hzb_size.y) >> mip_level,
    );

    let hzb_depth = textureLoad(hzb_texture, hzb_coord, i32(mip_level)).r;

    // Object's closest depth (sphere center depth - radius in clip space)
    let obj_depth = ndc.z - projected_radius;

    // If object is behind HZB depth, it's occluded
    // (Reverse-Z: larger depth = closer)
    return obj_depth <= hzb_depth;
}

// --- Main compute kernel ---

@compute @workgroup_size(64)
fn cull_instances(@builtin(global_invocation_id) gid: vec3<u32>) {
    let instance_idx = gid.x;
    if instance_idx >= params.instance_count {
        return;
    }

    let inst = instances[instance_idx];

    // Skip invisible instances
    if (inst.flags & FLAG_VISIBLE) == 0u {
        return;
    }

    // Transform bounds to world space
    let world_center = (inst.world_matrix * vec4<f32>(inst.bounds_center, 1.0)).xyz;
    let world_radius = inst.bounds_radius; // Assumes uniform scale for now

    // Frustum cull (same for both passes)
    if !frustum_cull(world_center, world_radius) {
        return;
    }

    // Screen-size cull: skip objects smaller than min_screen_size pixels
    if params.min_screen_size > 0.0 {
        let dist = distance(params.camera_pos, world_center);
        if dist > 0.0 {
            let screen_radius = (world_radius / dist) * max(params.hzb_size.x, params.hzb_size.y) * 0.5;
            if screen_radius < params.min_screen_size {
                return;
            }
        }
    }

    // HZB occlusion cull
    let hzb_visible = hzb_occlusion_test(world_center, world_radius);

    if params.pass_index == 0u {
        // Pass 0: Cull against previous frame HZB.
        // Visible → emit to visible list.
        // Occluded → write to occluded list for Pass 1 re-test.
        if !hzb_visible {
            let occ_idx = atomicAdd(&counters[2], 1u);
            occluded_indices[occ_idx] = instance_idx;
            return;
        }
    } else {
        // Pass 1: Re-test occluded instances against current frame HZB.
        // Only instances from the occluded list are dispatched for pass 1.
        if !hzb_visible {
            return; // Still occluded with new HZB
        }
    }

    // Instance passed all tests — add to visible list
    let out_idx = atomicAdd(&counters[0], 1u);
    visible_indices[out_idx] = instance_idx;

    // Write indirect draw command
    let cmd_idx = atomicAdd(&counters[1], 1u);
    draw_commands[cmd_idx] = DrawCommand(
        inst.index_count,
        1u,
        inst.index_offset,
        i32(inst.vertex_offset),
        out_idx,
    );
}
