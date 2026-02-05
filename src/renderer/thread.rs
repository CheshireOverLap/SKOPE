// SKOPE Engine - Multi-Thread Rendering Module
//
// Provides infrastructure for separating game logic from GPU submission:
//
// Main Thread:                     Render Thread:
//   Game logic / Physics / Anim     Receives RenderFrameData
//   Extract RenderFrameData         Encode command buffers
//   Send to render thread           Submit to GPU + Present
//   Start next frame immediately
//
// Also provides rayon-based parallel CPU utilities for:
// - Frustum culling
// - Mesh sorting by material (reduces pipeline switches)
// - Draw call batching
//
// Integration:
//   let rt = RenderThread::spawn(device, queue, renderer);
//   // Each frame:
//   rt.send(RenderCommand::Render(frame_data));
//   // Shutdown:
//   rt.shutdown();

use glam::{Vec3, Vec4, Mat4};
use std::sync::mpsc;
use std::sync::Arc;

use super::types::RenderSettings;

// ═══════════════════════════════════════════════════════════════════
// Frame Data (Send-safe, no lifetimes)
// ═══════════════════════════════════════════════════════════════════

/// Per-mesh data captured from ECS for rendering.
/// Lightweight, owned (no GPU references), `Send`-safe.
#[derive(Clone, Debug)]
pub struct RenderMeshData {
    /// World transformation matrix (model → world)
    pub model_matrix: [[f32; 4]; 4],
    /// Offset into the unified vertex buffer
    pub vertex_offset: u32,
    /// Offset into the unified index buffer
    pub index_offset: u32,
    /// Number of indices to draw
    pub index_count: u32,
    /// Material index in the material array
    pub material_index: u32,
    /// Mesh asset ID (for geometry buffer lookup)
    pub mesh_id: u32,
    /// Current LOD level
    pub lod_level: u32,
    /// Mesh flags (opaque, masked, dynamic, occluder, etc.)
    pub flags: u32,
    /// Bounding sphere center (world space)
    pub bounds_center: [f32; 3],
    /// Bounding sphere radius (world space)
    pub bounds_radius: f32,
}

/// Mesh classification flags (for draw call sorting)
pub mod mesh_flags {
    pub const OPAQUE: u32 = 1 << 0;
    pub const MASKED: u32 = 1 << 1;
    pub const TRANSLUCENT: u32 = 1 << 2;
    pub const DYNAMIC: u32 = 1 << 3;
    pub const CASTS_SHADOW: u32 = 1 << 4;
    pub const SKINNED: u32 = 1 << 5;
}

/// Complete frame data captured from the main thread for async rendering.
/// All fields are `Send`-safe (no lifetime references, no non-Send types).
pub struct RenderFrameData {
    // Camera
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub camera_pos: Vec3,
    pub jitter: [f32; 2],

    // Lighting
    pub sun_direction: Vec3,
    pub sun_color: Vec3,

    // Meshes
    pub meshes: Vec<RenderMeshData>,

    // Settings
    pub settings: RenderSettings,

    // Viewport
    pub viewport_width: u32,
    pub viewport_height: u32,

    // Time
    pub delta_time: f32,
    pub frame_index: u64,
}

impl RenderFrameData {
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
}

// ═══════════════════════════════════════════════════════════════════
// Render Commands
// ═══════════════════════════════════════════════════════════════════

/// Commands sent from the main thread to the render thread.
pub enum RenderCommand {
    /// Render a frame with the given data.
    Render(RenderFrameData),
    /// Resize the swap chain / render targets.
    Resize { width: u32, height: u32 },
    /// Shutdown the render thread gracefully.
    Shutdown,
}

// ═══════════════════════════════════════════════════════════════════
// Render Thread
// ═══════════════════════════════════════════════════════════════════

/// Handle for communicating with the render thread.
///
/// The render thread owns the `Renderer` and GPU resources.
/// The main thread sends `RenderCommand`s through a bounded channel
/// (capacity 2: double-buffering, main thread can prepare frame N+1
///  while render thread processes frame N).
pub struct RenderThread {
    sender: mpsc::SyncSender<RenderCommand>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RenderThread {
    /// Spawn a render thread.
    ///
    /// The render thread takes ownership of the device/queue arcs and
    /// will call the provided `render_fn` for each frame.
    ///
    /// `render_fn` is called on the render thread with the device, queue,
    /// and frame data. It should encode all GPU commands and submit them.
    pub fn spawn<F>(render_fn: F) -> Self
    where
        F: FnMut(RenderCommand) -> bool + Send + 'static,
    {
        // Bounded channel: 2 frames in flight (double buffering)
        let (sender, receiver) = mpsc::sync_channel::<RenderCommand>(2);

        let thread = std::thread::Builder::new()
            .name("skope-render".into())
            .spawn(move || {
                let mut render_fn = render_fn;
                log::info!("[RenderThread] Started");

                loop {
                    match receiver.recv() {
                        Ok(cmd) => {
                            let should_continue = render_fn(cmd);
                            if !should_continue {
                                break;
                            }
                        }
                        Err(_) => {
                            log::info!("[RenderThread] Channel closed, shutting down");
                            break;
                        }
                    }
                }

                log::info!("[RenderThread] Exited");
            })
            .expect("Failed to spawn render thread");

        Self {
            sender,
            thread: Some(thread),
        }
    }

    /// Send a render command to the render thread.
    /// Blocks if the channel is full (backpressure).
    pub fn send(&self, cmd: RenderCommand) -> Result<(), mpsc::SendError<RenderCommand>> {
        self.sender.send(cmd)
    }

    /// Try to send without blocking. Returns false if the channel is full.
    pub fn try_send(&self, cmd: RenderCommand) -> Result<(), mpsc::TrySendError<RenderCommand>> {
        self.sender.try_send(cmd)
    }

    /// Shutdown the render thread and wait for it to finish.
    pub fn shutdown(&mut self) {
        let _ = self.sender.send(RenderCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("Render thread panicked");
        }
    }

    /// Check if the render thread is still alive.
    pub fn is_alive(&self) -> bool {
        self.thread
            .as_ref()
            .map(|t| !t.is_finished())
            .unwrap_or(false)
    }
}

impl Drop for RenderThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ═══════════════════════════════════════════════════════════════════
// Parallel Culling (rayon-based)
// ═══════════════════════════════════════════════════════════════════

/// Frustum planes extracted from a view-projection matrix.
/// Used for fast AABB/sphere culling on CPU.
#[derive(Clone, Debug)]
pub struct FrustumPlanes {
    /// Six planes: Left, Right, Bottom, Top, Near, Far
    /// Each plane is (a, b, c, d) where ax + by + cz + d = 0
    pub planes: [Vec4; 6],
}

impl FrustumPlanes {
    /// Extract frustum planes from a view-projection matrix.
    /// Uses the Gribb-Hartmann method.
    pub fn from_view_proj(vp: &Mat4) -> Self {
        let m = vp.to_cols_array_2d();

        // Row extraction for plane computation
        let row0 = Vec4::new(m[0][0], m[1][0], m[2][0], m[3][0]);
        let row1 = Vec4::new(m[0][1], m[1][1], m[2][1], m[3][1]);
        let row2 = Vec4::new(m[0][2], m[1][2], m[2][2], m[3][2]);
        let row3 = Vec4::new(m[0][3], m[1][3], m[2][3], m[3][3]);

        let mut planes = [
            row3 + row0, // Left
            row3 - row0, // Right
            row3 + row1, // Bottom
            row3 - row1, // Top
            row3 + row2, // Near
            row3 - row2, // Far
        ];

        // Normalize plane normals
        for p in &mut planes {
            let len = Vec3::new(p.x, p.y, p.z).length();
            if len > 1e-6 {
                *p /= len;
            }
        }

        Self { planes }
    }

    /// Test a bounding sphere against the frustum.
    /// Returns true if the sphere is at least partially inside.
    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            let dist = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
            if dist < -radius {
                return false;
            }
        }
        true
    }
}

/// Parallel frustum culling using rayon.
/// Returns indices of visible meshes.
pub fn parallel_frustum_cull(
    meshes: &[RenderMeshData],
    frustum: &FrustumPlanes,
) -> Vec<usize> {
    use rayon::prelude::*;

    meshes
        .par_iter()
        .enumerate()
        .filter_map(|(i, mesh)| {
            let center = Vec3::from(mesh.bounds_center);
            if frustum.test_sphere(center, mesh.bounds_radius) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// Sort meshes by material index for minimal pipeline state changes.
/// Returns sorted indices (does not modify the original slice).
pub fn sort_by_material(meshes: &[RenderMeshData]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..meshes.len()).collect();
    indices.sort_unstable_by_key(|&i| meshes[i].material_index);
    indices
}

/// Sort meshes front-to-back by distance to camera (for early-Z efficiency).
/// Returns sorted indices.
pub fn sort_front_to_back(meshes: &[RenderMeshData], camera_pos: Vec3) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..meshes.len()).collect();
    indices.sort_unstable_by(|&a, &b| {
        let dist_a = (Vec3::from(meshes[a].bounds_center) - camera_pos).length_squared();
        let dist_b = (Vec3::from(meshes[b].bounds_center) - camera_pos).length_squared();
        dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
    });
    indices
}

/// Sort meshes back-to-front (for transparency).
/// Returns sorted indices.
pub fn sort_back_to_front(meshes: &[RenderMeshData], camera_pos: Vec3) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..meshes.len()).collect();
    indices.sort_unstable_by(|&a, &b| {
        let dist_a = (Vec3::from(meshes[a].bounds_center) - camera_pos).length_squared();
        let dist_b = (Vec3::from(meshes[b].bounds_center) - camera_pos).length_squared();
        dist_b.partial_cmp(&dist_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    indices
}

/// Classify meshes into opaque and translucent groups.
/// Returns (opaque_indices, translucent_indices).
pub fn classify_meshes(meshes: &[RenderMeshData]) -> (Vec<usize>, Vec<usize>) {
    let mut opaque = Vec::with_capacity(meshes.len());
    let mut translucent = Vec::new();

    for (i, mesh) in meshes.iter().enumerate() {
        if mesh.flags & mesh_flags::TRANSLUCENT != 0 {
            translucent.push(i);
        } else {
            opaque.push(i);
        }
    }

    (opaque, translucent)
}

/// Full CPU-side frame preparation pipeline (parallel where possible).
///
/// 1. Frustum cull (rayon parallel)
/// 2. Classify into opaque/translucent
/// 3. Sort opaque front-to-back (early-Z efficiency)
/// 4. Sort translucent back-to-front (correct blending)
///
/// Returns `PreparedDrawCalls` with sorted, culled mesh indices.
pub fn prepare_draw_calls(
    meshes: &[RenderMeshData],
    view_proj: &Mat4,
    camera_pos: Vec3,
) -> PreparedDrawCalls {
    let frustum = FrustumPlanes::from_view_proj(view_proj);

    // Step 1: Parallel frustum cull
    let visible = parallel_frustum_cull(meshes, &frustum);

    // Step 2: Classify
    let mut opaque_indices = Vec::with_capacity(visible.len());
    let mut translucent_indices = Vec::new();
    let mut shadow_indices = Vec::new();

    for &i in &visible {
        if meshes[i].flags & mesh_flags::TRANSLUCENT != 0 {
            translucent_indices.push(i);
        } else {
            opaque_indices.push(i);
        }
        if meshes[i].flags & mesh_flags::CASTS_SHADOW != 0 {
            shadow_indices.push(i);
        }
    }

    // Step 3: Sort opaque front-to-back
    opaque_indices.sort_unstable_by(|&a, &b| {
        let dist_a = (Vec3::from(meshes[a].bounds_center) - camera_pos).length_squared();
        let dist_b = (Vec3::from(meshes[b].bounds_center) - camera_pos).length_squared();
        dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Step 4: Sort translucent back-to-front
    translucent_indices.sort_unstable_by(|&a, &b| {
        let dist_a = (Vec3::from(meshes[a].bounds_center) - camera_pos).length_squared();
        let dist_b = (Vec3::from(meshes[b].bounds_center) - camera_pos).length_squared();
        dist_b.partial_cmp(&dist_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    PreparedDrawCalls {
        total_meshes: meshes.len(),
        visible_count: visible.len(),
        opaque: opaque_indices,
        translucent: translucent_indices,
        shadow_casters: shadow_indices,
    }
}

/// Result of CPU-side draw call preparation.
pub struct PreparedDrawCalls {
    /// Total mesh count before culling
    pub total_meshes: usize,
    /// Number of meshes that passed frustum culling
    pub visible_count: usize,
    /// Opaque mesh indices, sorted front-to-back
    pub opaque: Vec<usize>,
    /// Translucent mesh indices, sorted back-to-front
    pub translucent: Vec<usize>,
    /// Shadow-casting mesh indices
    pub shadow_casters: Vec<usize>,
}

impl PreparedDrawCalls {
    pub fn culled_count(&self) -> usize {
        self.total_meshes - self.visible_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frustum_planes_identity() {
        // Simple orthographic-like projection
        let vp = Mat4::IDENTITY;
        let frustum = FrustumPlanes::from_view_proj(&vp);
        // A point at origin should be inside the frustum
        assert!(frustum.test_sphere(Vec3::ZERO, 0.5));
    }

    #[test]
    fn test_parallel_frustum_cull_empty() {
        let meshes: Vec<RenderMeshData> = vec![];
        let frustum = FrustumPlanes::from_view_proj(&Mat4::IDENTITY);
        let visible = parallel_frustum_cull(&meshes, &frustum);
        assert!(visible.is_empty());
    }

    #[test]
    fn test_sort_by_material() {
        let meshes = vec![
            make_test_mesh(3),
            make_test_mesh(1),
            make_test_mesh(2),
            make_test_mesh(1),
        ];
        let sorted = sort_by_material(&meshes);
        assert_eq!(meshes[sorted[0]].material_index, 1);
        assert_eq!(meshes[sorted[1]].material_index, 1);
        assert_eq!(meshes[sorted[2]].material_index, 2);
        assert_eq!(meshes[sorted[3]].material_index, 3);
    }

    #[test]
    fn test_classify_meshes() {
        let meshes = vec![
            make_test_mesh_flags(mesh_flags::OPAQUE),
            make_test_mesh_flags(mesh_flags::TRANSLUCENT),
            make_test_mesh_flags(mesh_flags::OPAQUE),
            make_test_mesh_flags(mesh_flags::TRANSLUCENT),
            make_test_mesh_flags(mesh_flags::OPAQUE | mesh_flags::CASTS_SHADOW),
        ];
        let (opaque, translucent) = classify_meshes(&meshes);
        assert_eq!(opaque.len(), 3);
        assert_eq!(translucent.len(), 2);
    }

    #[test]
    fn test_prepare_draw_calls() {
        let meshes = vec![
            RenderMeshData {
                model_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                               [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
                vertex_offset: 0,
                index_offset: 0,
                index_count: 36,
                material_index: 0,
                mesh_id: 0,
                lod_level: 0,
                flags: mesh_flags::OPAQUE | mesh_flags::CASTS_SHADOW,
                bounds_center: [0.0, 0.0, 0.0],
                bounds_radius: 1.0,
            },
        ];

        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0)
            * Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);

        let result = prepare_draw_calls(&meshes, &vp, Vec3::new(0.0, 0.0, 5.0));
        assert_eq!(result.visible_count, 1);
        assert_eq!(result.opaque.len(), 1);
        assert_eq!(result.shadow_casters.len(), 1);
        assert!(result.translucent.is_empty());
    }

    fn make_test_mesh(mat_idx: u32) -> RenderMeshData {
        RenderMeshData {
            model_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                           [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            vertex_offset: 0,
            index_offset: 0,
            index_count: 0,
            material_index: mat_idx,
            mesh_id: 0,
            lod_level: 0,
            flags: mesh_flags::OPAQUE,
            bounds_center: [0.0, 0.0, 0.0],
            bounds_radius: 1.0,
        }
    }

    fn make_test_mesh_flags(flags: u32) -> RenderMeshData {
        RenderMeshData {
            model_matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0],
                           [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            vertex_offset: 0,
            index_offset: 0,
            index_count: 0,
            material_index: 0,
            mesh_id: 0,
            lod_level: 0,
            flags,
            bounds_center: [0.0, 0.0, 0.0],
            bounds_radius: 1.0,
        }
    }
}
