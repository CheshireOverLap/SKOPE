//! SKOPE Render Uniforms
//!
//! GPU-compatible uniform buffer structures for rendering.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Camera uniform data for shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    pub view_projection: [[f32; 4]; 4],
    pub inv_view_projection: [[f32; 4]; 4],
    pub camera_position: [f32; 4],
    pub screen_size: [f32; 2],
    pub near: f32,
    pub far: f32,
}

impl CameraUniform {
    pub fn new(
        view: Mat4,
        projection: Mat4,
        position: Vec3,
        screen_size: (u32, u32),
        near: f32,
        far: f32,
    ) -> Self {
        let view_projection = projection * view;
        let inv_view_projection = view_projection.inverse();

        Self {
            view: view.to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            view_projection: view_projection.to_cols_array_2d(),
            inv_view_projection: inv_view_projection.to_cols_array_2d(),
            camera_position: [position.x, position.y, position.z, 1.0],
            screen_size: [screen_size.0 as f32, screen_size.1 as f32],
            near,
            far,
        }
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY.to_cols_array_2d(),
            projection: Mat4::IDENTITY.to_cols_array_2d(),
            view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            camera_position: [0.0, 0.0, 5.0, 1.0],
            screen_size: [1280.0, 720.0],
            near: 0.1,
            far: 1000.0,
        }
    }
}

/// Model transform uniform
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ModelUniform {
    pub model: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
}

impl ModelUniform {
    pub fn new(model: Mat4) -> Self {
        let normal_matrix = model.inverse().transpose();
        Self {
            model: model.to_cols_array_2d(),
            normal_matrix: normal_matrix.to_cols_array_2d(),
        }
    }
}

impl Default for ModelUniform {
    fn default() -> Self {
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            normal_matrix: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

/// Lighting uniform data for deferred shading
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightingUniform {
    // Camera
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_position: [f32; 4],

    // Sun/Directional light
    pub sun_direction: [f32; 4],
    pub sun_color: [f32; 4],

    // Ambient
    pub ambient_color: [f32; 4],

    // Settings
    pub screen_size: [f32; 2],
    pub time: f32,
    pub exposure: f32,

    // Debug/Tuning parameters
    pub intensity_scale: f32,
    pub d_ggx_max: f32,
    pub specular_max: f32,
    pub roughness_min: f32,

    // Debug visualization mode
    pub debug_mode: u32,
    pub _pad1: [u32; 3],
    pub _pad2: [u32; 4],
}

impl Default for LightingUniform {
    fn default() -> Self {
        Self {
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_position: [0.0, 0.0, 5.0, 1.0],
            sun_direction: [-0.5, -1.0, -0.3, 0.0],
            sun_color: [1.0, 0.98, 0.95, 1.0],
            ambient_color: [0.03, 0.03, 0.05, 1.0],
            screen_size: [1280.0, 720.0],
            time: 0.0,
            exposure: 1.0,
            intensity_scale: 0.2,
            d_ggx_max: 16.0,
            specular_max: 10.0,
            roughness_min: 0.1,
            debug_mode: 0,
            _pad1: [0, 0, 0],
            _pad2: [0, 0, 0, 0],
        }
    }
}

/// Debug visualization modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum DebugMode {
    #[default]
    None = 0,
    Albedo = 1,
    Normals = 2,
    Roughness = 3,
    Metallic = 4,
    Depth = 5,
    LightingOnly = 6,
    AO = 7,
    Emissive = 8,
    Velocity = 9,
    Clusters = 10,
}

/// Material uniform data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub emissive: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub ao: f32,
    pub _pad: f32,
}

impl Default for MaterialUniform {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            emissive: [0.0, 0.0, 0.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            _pad: 0.0,
        }
    }
}

/// V-Buffer instance data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VBufferInstanceData {
    pub model_matrix: [[f32; 4]; 4],
    pub material_id: u32,
    pub flags: u32,
    pub _pad: [u32; 2],
}

impl Default for VBufferInstanceData {
    fn default() -> Self {
        Self {
            model_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            material_id: 0,
            flags: 0,
            _pad: [0, 0],
        }
    }
}

/// Clustered lighting grid info
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ClusterGridUniform {
    pub cluster_count: [u32; 4],      // x, y, z, total
    pub cluster_size: [f32; 4],       // screen_x, screen_y, depth_slice, padding
    pub near_far: [f32; 4],           // near, far, log2(far/near), padding
    pub screen_size: [f32; 4],        // width, height, 1/width, 1/height
}

impl Default for ClusterGridUniform {
    fn default() -> Self {
        Self {
            cluster_count: [16, 9, 24, 16 * 9 * 24],
            cluster_size: [80.0, 80.0, 0.0, 0.0],
            near_far: [0.1, 1000.0, (1000.0_f32 / 0.1).log2(), 0.0],
            screen_size: [1280.0, 720.0, 1.0 / 1280.0, 1.0 / 720.0],
        }
    }
}

/// Shadow cascade data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowCascadeUniform {
    pub view_proj: [[f32; 4]; 4],
    pub split_depth: f32,
    pub _pad: [f32; 3],
}

impl Default for ShadowCascadeUniform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            split_depth: 0.0,
            _pad: [0.0, 0.0, 0.0],
        }
    }
}

/// Shadow uniform buffer (4 cascades)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowUniform {
    pub cascades: [ShadowCascadeUniform; 4],
    pub cascade_count: u32,
    pub shadow_bias: f32,
    pub normal_bias: f32,
    pub pcf_radius: f32,
}

impl Default for ShadowUniform {
    fn default() -> Self {
        Self {
            cascades: [ShadowCascadeUniform::default(); 4],
            cascade_count: 4,
            shadow_bias: 0.005,
            normal_bias: 0.02,
            pcf_radius: 1.0,
        }
    }
}
