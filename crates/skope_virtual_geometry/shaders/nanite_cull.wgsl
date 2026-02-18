// Nanite GPU Culling Compute Shader
//
// Per-cluster frustum + occlusion culling + LOD selection.
// One thread per meshlet/cluster.
//
// Pipeline:
//   1. Load instance + meshlet data
//   2. Transform bounding sphere to world space
//   3. Frustum test (6 plane half-space)
//   4. LOD selection (screen-space error threshold)
//   5. Occlusion test (HZB lookup)
//   6. Classify: HW vs SW rasterization
//   7. Append to visible cluster list + update indirect args
//
// Bind Groups:
//   G0: CullParams (uniform) + Instances (storage)
//   G1: Meshlets (storage) + MeshletTriangles (storage)
//   G2: HZB texture + sampler
//   G3: Outputs (visible clusters, HW args, SW args, counters)

// ── Struct definitions ─────────────────────────────────────────────

struct CullParams {
    view_proj: mat4x4<f32>,
    frustum_planes: array<vec4<f32>, 6>,
    camera_pos: vec3<f32>,
    screen_height: f32,
    fov_y: f32,
    hzb_width: u32,
    hzb_height: u32,
    lod_scale: f32,
    instance_count: u32,
    total_meshlet_count: u32,
    enable_occlusion_cull: u32,
    _pad: u32,
}

struct NaniteInstance {
    world_matrix: mat4x4<f32>,
    prev_world_matrix: mat4x4<f32>,
    mesh_id: u32,
    material_id: u32,
    lod_bias: f32,
    flags: u32,
}

struct Meshlet {
    vertex_offset: u32,
    vertex_count: u32,
    triangle_offset: u32,
    triangle_count: u32,
    bounding_sphere: vec4<f32>,   // xyz = center, w = radius
    normal_cone: vec4<f32>,       // xyz = axis, w = cos(half_angle)
    lod_error: f32,
    parent_error: f32,
    group_id: u32,
    lod_level: u32,
}

/// Visible cluster entry appended to the output buffer.
struct VisibleCluster {
    instance_id: u32,
    meshlet_id: u32,
    material_id: u32,
    flags: u32,   // bit 0: hw(0) / sw(1) rasterization
}

/// Indirect draw arguments (wgpu DrawIndirect format).
struct IndirectDrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

/// Indirect dispatch arguments.
struct IndirectDispatchArgs {
    x: u32,
    y: u32,
    z: u32,
    _pad: u32,
}

// Instance flags
const INSTANCE_VISIBLE: u32 = 1u;
const INSTANCE_SHADOW_CASTER: u32 = 2u;
const INSTANCE_MOVABLE: u32 = 4u;

// Visible cluster flags
const CLUSTER_SW_RASTER: u32 = 1u;

// ── Bindings ───────────────────────────────────────────────────────

// Group 0: Cull params + Instances
@group(0) @binding(0) var<uniform> params: CullParams;
@group(0) @binding(1) var<storage, read> instances: array<NaniteInstance>;

struct MeshMeshletRange {
    meshlet_offset: u32,
    meshlet_count: u32,
}

// Group 1: Meshlets + MeshRanges
@group(1) @binding(0) var<storage, read> meshlets: array<Meshlet>;
@group(1) @binding(1) var<storage, read> mesh_ranges: array<MeshMeshletRange>;

// Group 2: HZB for occlusion culling
@group(2) @binding(0) var hzb_texture: texture_2d<f32>;
@group(2) @binding(1) var hzb_sampler: sampler;

// Group 3: Outputs
@group(3) @binding(0) var<storage, read_write> visible_clusters: array<VisibleCluster>;
@group(3) @binding(1) var<storage, read_write> hw_indirect: IndirectDrawArgs;
@group(3) @binding(2) var<storage, read_write> sw_indirect: IndirectDispatchArgs;
@group(3) @binding(3) var<storage, read_write> counters: array<atomic<u32>>;
// counters[0] = total visible cluster count
// counters[1] = HW cluster count
// counters[2] = SW cluster count

// ── Helper functions ───────────────────────────────────────────────

/// Test a world-space bounding sphere against the view frustum.
fn frustum_test(center: vec3<f32>, radius: f32) -> bool {
    for (var i = 0u; i < 6u; i++) {
        let plane = params.frustum_planes[i];
        let dist = dot(plane.xyz, center) + plane.w;
        if dist < -radius {
            return false;
        }
    }
    return true;
}

/// Compute the screen-space projected pixel size of a sphere.
/// Returns the diameter in pixels.
fn screen_space_size(center: vec3<f32>, radius: f32) -> f32 {
    let d = distance(params.camera_pos, center);
    if d < 0.001 {
        return params.screen_height;
    }
    // Project sphere radius to screen pixels
    // pixel_size = (radius / (d * tan(fov/2))) * screen_height
    let half_fov_tan = tan(params.fov_y * 0.5);
    return (radius / (d * half_fov_tan)) * params.screen_height;
}

/// LOD selection: returns true if this cluster should be rendered at the current LOD.
/// A cluster is selected when its error is below the threshold but its parent's error is above.
fn lod_select(meshlet: Meshlet, world_center: vec3<f32>, lod_bias: f32) -> bool {
    let d = distance(params.camera_pos, world_center);
    if d < 0.001 {
        // Very close: always render finest LOD
        return meshlet.lod_level == 0u;
    }

    let half_fov_tan = tan(params.fov_y * 0.5);
    let pixel_per_unit = params.screen_height / (2.0 * d * half_fov_tan);
    let screen_error = meshlet.lod_error * pixel_per_unit * params.lod_scale + lod_bias;
    let parent_screen_error = meshlet.parent_error * pixel_per_unit * params.lod_scale + lod_bias;

    // This cluster is selected if:
    // - Its own error is acceptable (below 1 pixel threshold)
    // - Its parent's error is NOT acceptable (above threshold)
    // This ensures exactly one LOD level per group is selected.
    let threshold = 1.0; // 1 pixel error threshold
    return screen_error <= threshold && parent_screen_error > threshold;
}

/// HZB occlusion test: returns true if the sphere is potentially visible.
fn occlusion_test(center: vec3<f32>, radius: f32) -> bool {
    if params.enable_occlusion_cull == 0u {
        return true;
    }

    // Project sphere to screen-space AABB
    let clip_pos = params.view_proj * vec4<f32>(center, 1.0);
    if clip_pos.w <= 0.0 {
        // Behind camera — conservatively visible (might wrap around)
        return true;
    }

    let ndc = clip_pos.xyz / clip_pos.w;
    let screen_radius = radius / clip_pos.w;

    // Screen-space bounds (in [0, 1] UV space)
    let uv_center = ndc.xy * 0.5 + 0.5;
    let uv_radius = screen_radius * 0.5;

    let uv_min = clamp(uv_center - vec2<f32>(uv_radius), vec2<f32>(0.0), vec2<f32>(1.0));
    let uv_max = clamp(uv_center + vec2<f32>(uv_radius), vec2<f32>(0.0), vec2<f32>(1.0));

    // Choose HZB mip level based on projected size
    let pixel_size = uv_radius * 2.0 * f32(params.hzb_width);
    let mip = clamp(u32(ceil(log2(max(pixel_size, 1.0)))), 0u, 10u);

    // Sample HZB at the center (conservative single-point test)
    let hzb_depth = textureSampleLevel(hzb_texture, hzb_sampler, uv_center, f32(mip)).r;

    // Compare: object's nearest depth vs HZB (reverse-Z: closer = larger)
    let obj_depth = ndc.z; // NDC depth [0, 1] (reverse-Z: 1 = near, 0 = far)

    // If the object is farther than the HZB occluder, it's occluded
    return obj_depth >= hzb_depth;
}

// ── Main compute shader ────────────────────────────────────────────

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // 2D dispatch: gid.x = local meshlet index, gid.y = instance index
    let local_meshlet_idx = gid.x;
    let instance_id = gid.y;

    if instance_id >= params.instance_count {
        return;
    }

    let instance = instances[instance_id];

    // Look up which meshlets belong to this instance's mesh
    let range = mesh_ranges[instance.mesh_id];
    if local_meshlet_idx >= range.meshlet_count {
        return;
    }

    let meshlet_idx = range.meshlet_offset + local_meshlet_idx;
    if meshlet_idx >= params.total_meshlet_count {
        return;
    }

    let meshlet = meshlets[meshlet_idx];

    // Skip invisible instances
    if (instance.flags & INSTANCE_VISIBLE) == 0u {
        return;
    }

    // Transform bounding sphere to world space
    let local_center = meshlet.bounding_sphere.xyz;
    let local_radius = meshlet.bounding_sphere.w;
    let world_center = (instance.world_matrix * vec4<f32>(local_center, 1.0)).xyz;

    // Scale radius by maximum scale factor of the instance transform
    let sx = length(vec3<f32>(instance.world_matrix[0].x, instance.world_matrix[0].y, instance.world_matrix[0].z));
    let sy = length(vec3<f32>(instance.world_matrix[1].x, instance.world_matrix[1].y, instance.world_matrix[1].z));
    let sz = length(vec3<f32>(instance.world_matrix[2].x, instance.world_matrix[2].y, instance.world_matrix[2].z));
    let world_radius = local_radius * max(sx, max(sy, sz));

    // ── Frustum cull ──
    if !frustum_test(world_center, world_radius) {
        return;
    }

    // ── LOD selection ──
    if !lod_select(meshlet, world_center, instance.lod_bias) {
        return;
    }

    // ── Occlusion cull (HZB) ──
    if !occlusion_test(world_center, world_radius) {
        return;
    }

    // ── Normal cone backface cull ──
    // Skip clusters that face entirely away from the camera
    let cone_axis = (instance.world_matrix * vec4<f32>(meshlet.normal_cone.xyz, 0.0)).xyz;
    let cone_cos = meshlet.normal_cone.w;
    if cone_cos > -1.0 {
        let view_dir = normalize(world_center - params.camera_pos);
        if dot(view_dir, cone_axis) > cone_cos {
            return; // Entire cluster is backfacing
        }
    }

    // ── Classification: HW vs SW ──
    let pixel_size = screen_space_size(world_center, world_radius);
    let use_sw = pixel_size < 32.0; // SW threshold: 32 pixels

    // ── Append to visible cluster list ──
    let slot = atomicAdd(&counters[0], 1u);

    // Guard: skip write if buffer is full (prevents OOB storage writes)
    if slot >= arrayLength(&visible_clusters) { return; }

    var cluster: VisibleCluster;
    cluster.instance_id = instance_id;
    cluster.meshlet_id = meshlet_idx;
    cluster.material_id = instance.material_id;
    cluster.flags = select(0u, CLUSTER_SW_RASTER, use_sw);

    visible_clusters[slot] = cluster;

    // Update HW or SW counters and indirect args
    if use_sw {
        atomicAdd(&counters[2], 1u);
        // sw_indirect.x is set by a post-cull buffer copy from counters[2]
    } else {
        // HW (Mesh Shader): one mesh workgroup per cluster.
        // Task shader reads counters[1] to know how many clusters to dispatch.
        atomicAdd(&counters[1], 1u);
    }
}
