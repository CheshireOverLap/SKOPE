// SKOPE Engine — SDF Voxelization Compute Shader
//
// Generates a global distance field by computing the minimum signed distance
// from each voxel center to all instance bounding spheres in the GPU Scene.
//
// Phase 1 approximation: uses bounding spheres only. Accurate mesh SDFs
// are deferred to Phase 2.
//
// Input:  GpuInstance array (bounds_center, bounds_radius)
// Output: R32Float 3D storage texture (signed distance per voxel)

struct VoxelizeParams {
    volume_origin: vec3<f32>,
    voxel_size: f32,
    resolution: u32,
    instance_count: u32,
    _pad: vec2<u32>,
}

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
}

@group(0) @binding(0) var<uniform> params: VoxelizeParams;
@group(0) @binding(1) var<storage, read> instances: array<GpuInstance>;
@group(0) @binding(2) var sdf_output: texture_storage_3d<r32float, write>;

const FLAG_VISIBLE: u32 = 1u;

@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.resolution || gid.y >= params.resolution || gid.z >= params.resolution {
        return;
    }

    // Voxel center in world space
    let world_pos = params.volume_origin + vec3<f32>(
        f32(gid.x) + 0.5,
        f32(gid.y) + 0.5,
        f32(gid.z) + 0.5,
    ) * params.voxel_size;

    // Compute min signed distance to all visible instance bounding spheres
    var min_dist = params.voxel_size * f32(params.resolution); // Start at max extent

    for (var i = 0u; i < params.instance_count; i++) {
        let inst = instances[i];
        // Only consider visible instances
        if (inst.flags & FLAG_VISIBLE) == 0u {
            continue;
        }
        let d = length(world_pos - inst.bounds_center) - inst.bounds_radius;
        min_dist = min(min_dist, d);
    }

    // Normalize: store distance relative to volume extent for SDF tracing
    // Positive = outside, negative = inside
    let normalized = min_dist / (params.voxel_size * f32(params.resolution));

    textureStore(sdf_output, gid, vec4<f32>(normalized, 0.0, 0.0, 0.0));
}
