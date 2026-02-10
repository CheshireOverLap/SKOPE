//! SKOPE Render Configuration
//!
//! Rendering pipeline configuration and presets.

use glam::Vec3;

/// V-Buffer rendering configuration
#[derive(Debug, Clone)]
pub struct VBufferConfig {
    /// Enable multi-sample anti-aliasing
    pub msaa_samples: u32,

    /// Use 32-bit depth buffer (vs 24-bit)
    pub depth_32bit: bool,

    /// Enable velocity buffer for motion blur/TAA
    pub velocity_buffer: bool,

    /// Enable stencil buffer
    pub stencil_buffer: bool,
}

impl Default for VBufferConfig {
    fn default() -> Self {
        Self {
            msaa_samples: 1,
            depth_32bit: true,
            velocity_buffer: true,
            stencil_buffer: true,
        }
    }
}

/// Shadow rendering configuration
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// Shadow map resolution
    pub resolution: u32,

    /// Number of cascade splits
    pub cascade_count: u32,

    /// Maximum shadow distance
    pub max_distance: f32,

    /// Split distribution lambda (0 = linear, 1 = logarithmic)
    pub split_lambda: f32,

    /// Depth bias
    pub depth_bias: f32,

    /// Normal bias
    pub normal_bias: f32,

    /// PCF filtering radius
    pub pcf_radius: f32,

    /// Enable soft shadows
    pub soft_shadows: bool,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            resolution: 2048,
            cascade_count: 4,
            max_distance: 100.0,
            split_lambda: 0.5,
            depth_bias: 0.005,
            normal_bias: 0.02,
            pcf_radius: 1.0,
            soft_shadows: true,
        }
    }
}

/// Clustered lighting configuration
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Number of clusters in X dimension
    pub clusters_x: u32,

    /// Number of clusters in Y dimension
    pub clusters_y: u32,

    /// Number of depth slices
    pub clusters_z: u32,

    /// Maximum lights per cluster
    pub max_lights_per_cluster: u32,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            clusters_x: 16,
            clusters_y: 9,
            clusters_z: 24,
            max_lights_per_cluster: 256,
        }
    }
}

impl ClusterConfig {
    pub fn total_clusters(&self) -> u32 {
        self.clusters_x * self.clusters_y * self.clusters_z
    }
}

/// Environment/IBL configuration
#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    /// Environment intensity multiplier
    pub intensity: f32,

    /// Environment rotation (radians around Y)
    pub rotation: f32,

    /// Diffuse IBL contribution
    pub diffuse_intensity: f32,

    /// Specular IBL contribution
    pub specular_intensity: f32,

    /// Sky color for procedural sky
    pub sky_color: Vec3,

    /// Ground color for procedural sky
    pub ground_color: Vec3,

    /// Use procedural sky vs cubemap
    pub procedural_sky: bool,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            rotation: 0.0,
            diffuse_intensity: 1.0,
            specular_intensity: 1.0,
            sky_color: Vec3::new(0.5, 0.7, 1.0),
            ground_color: Vec3::new(0.1, 0.1, 0.1),
            procedural_sky: true,
        }
    }
}

/// Complete render configuration
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub vbuffer: VBufferConfig,
    pub shadow: ShadowConfig,
    pub cluster: ClusterConfig,
    pub environment: EnvironmentConfig,

    /// Target framebuffer width
    pub width: u32,

    /// Target framebuffer height
    pub height: u32,

    /// Render scale (0.5 = half resolution)
    pub render_scale: f32,

    /// HDR rendering
    pub hdr: bool,

    /// Maximum lights in scene
    pub max_lights: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            vbuffer: VBufferConfig::default(),
            shadow: ShadowConfig::default(),
            cluster: ClusterConfig::default(),
            environment: EnvironmentConfig::default(),
            width: 1920,
            height: 1080,
            render_scale: 1.0,
            hdr: true,
            max_lights: 1024,
        }
    }
}

impl RenderConfig {
    pub fn scaled_width(&self) -> u32 {
        ((self.width as f32) * self.render_scale).max(1.0) as u32
    }

    pub fn scaled_height(&self) -> u32 {
        ((self.height as f32) * self.render_scale).max(1.0) as u32
    }

    /// Low quality preset for weak hardware
    pub fn low() -> Self {
        Self {
            vbuffer: VBufferConfig {
                msaa_samples: 1,
                depth_32bit: false,
                velocity_buffer: false,
                stencil_buffer: false,
            },
            shadow: ShadowConfig {
                resolution: 1024,
                cascade_count: 2,
                soft_shadows: false,
                ..Default::default()
            },
            cluster: ClusterConfig {
                clusters_x: 8,
                clusters_y: 6,
                clusters_z: 16,
                max_lights_per_cluster: 64,
            },
            render_scale: 0.75,
            max_lights: 256,
            ..Default::default()
        }
    }

    /// High quality preset
    pub fn high() -> Self {
        Self {
            vbuffer: VBufferConfig {
                msaa_samples: 4,
                depth_32bit: true,
                velocity_buffer: true,
                stencil_buffer: true,
            },
            shadow: ShadowConfig {
                resolution: 4096,
                cascade_count: 4,
                soft_shadows: true,
                ..Default::default()
            },
            cluster: ClusterConfig {
                clusters_x: 32,
                clusters_y: 18,
                clusters_z: 32,
                max_lights_per_cluster: 512,
            },
            render_scale: 1.0,
            max_lights: 2048,
            ..Default::default()
        }
    }
}
