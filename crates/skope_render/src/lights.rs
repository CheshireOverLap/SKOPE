//! SKOPE Light Types
//!
//! Directional, Point, Spot, and Area light definitions.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Light type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum LightType {
    #[default]
    Directional = 0,
    Point = 1,
    Spot = 2,
    AreaRect = 3,
    AreaDisk = 4,
}

/// Directional Light (sun, moon)
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub angular_diameter: f32,
    pub cast_shadows: bool,
    pub shadow_cascade_count: u32,
    pub shadow_distance: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::ONE,
            intensity: 1.0,
            angular_diameter: 0.53,
            cast_shadows: true,
            shadow_cascade_count: 4,
            shadow_distance: 100.0,
        }
    }
}

/// Point Light (light bulb, torch)
#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
    pub source_radius: f32,
    pub cast_shadows: bool,
    pub shadow_bias: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 1.0,
            radius: 10.0,
            source_radius: 0.0,
            cast_shadows: false,
            shadow_bias: 0.001,
        }
    }
}

/// Spot Light (flashlight, street lamp)
#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub source_radius: f32,
    pub cast_shadows: bool,
    pub shadow_bias: f32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::ONE,
            intensity: 1.0,
            radius: 15.0,
            inner_angle: 0.4,
            outer_angle: 0.6,
            source_radius: 0.0,
            cast_shadows: true,
            shadow_bias: 0.001,
        }
    }
}

/// Rectangular Area Light
#[derive(Debug, Clone, Copy)]
pub struct RectAreaLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub up: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub width: f32,
    pub height: f32,
    pub two_sided: bool,
}

impl Default for RectAreaLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::Y,
            color: Vec3::ONE,
            intensity: 1.0,
            width: 1.0,
            height: 1.0,
            two_sided: false,
        }
    }
}

/// Disk Area Light
#[derive(Debug, Clone, Copy)]
pub struct DiskAreaLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub disk_radius: f32,
}

impl Default for DiskAreaLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::new(0.0, 0.0, -1.0),
            color: Vec3::ONE,
            intensity: 1.0,
            disk_radius: 0.5,
        }
    }
}

/// GPU-compatible light data (unified structure)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuLight {
    pub position_type: [f32; 4],       // xyz: position, w: light_type
    pub direction_radius: [f32; 4],    // xyz: direction, w: radius
    pub color_intensity: [f32; 4],     // xyz: color, w: intensity
    pub params0: [f32; 4],             // spot: inner/outer cos, area: width/height
    pub params1: [f32; 4],             // source_radius, shadow_bias, shadows, etc.
}

impl Default for GpuLight {
    fn default() -> Self {
        Self {
            position_type: [0.0, 0.0, 0.0, 0.0],
            direction_radius: [0.0, -1.0, 0.0, 0.0],
            color_intensity: [1.0, 1.0, 1.0, 1.0],
            params0: [0.0, 0.0, 0.0, 0.0],
            params1: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl GpuLight {
    pub fn from_directional(light: &DirectionalLight) -> Self {
        Self {
            position_type: [0.0, 0.0, 0.0, LightType::Directional as u32 as f32],
            direction_radius: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [light.angular_diameter, 0.0, 0.0, 0.0],
            params1: [
                if light.cast_shadows { 1.0 } else { 0.0 },
                light.shadow_cascade_count as f32,
                light.shadow_distance,
                0.0,
            ],
        }
    }

    pub fn from_point(light: &PointLight) -> Self {
        Self {
            position_type: [
                light.position.x,
                light.position.y,
                light.position.z,
                LightType::Point as u32 as f32,
            ],
            direction_radius: [0.0, 0.0, 0.0, light.radius],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [0.0, 0.0, 0.0, 0.0],
            params1: [
                light.source_radius,
                light.shadow_bias,
                if light.cast_shadows { 1.0 } else { 0.0 },
                0.0,
            ],
        }
    }

    pub fn from_spot(light: &SpotLight) -> Self {
        Self {
            position_type: [
                light.position.x,
                light.position.y,
                light.position.z,
                LightType::Spot as u32 as f32,
            ],
            direction_radius: [
                light.direction.x,
                light.direction.y,
                light.direction.z,
                light.radius,
            ],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [light.inner_angle.cos(), light.outer_angle.cos(), 0.0, 0.0],
            params1: [
                light.source_radius,
                light.shadow_bias,
                if light.cast_shadows { 1.0 } else { 0.0 },
                0.0,
            ],
        }
    }

    pub fn from_rect_area(light: &RectAreaLight) -> Self {
        Self {
            position_type: [
                light.position.x,
                light.position.y,
                light.position.z,
                LightType::AreaRect as u32 as f32,
            ],
            direction_radius: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [light.width, light.height, light.up.x, light.up.y],
            params1: [light.up.z, if light.two_sided { 1.0 } else { 0.0 }, 0.0, 0.0],
        }
    }

    pub fn from_disk_area(light: &DiskAreaLight) -> Self {
        Self {
            position_type: [
                light.position.x,
                light.position.y,
                light.position.z,
                LightType::AreaDisk as u32 as f32,
            ],
            direction_radius: [
                light.direction.x,
                light.direction.y,
                light.direction.z,
                light.disk_radius,
            ],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [0.0, 0.0, 0.0, 0.0],
            params1: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Light data collection for a scene
#[derive(Debug, Clone, Default)]
pub struct LightData {
    pub directional_lights: Vec<DirectionalLight>,
    pub point_lights: Vec<PointLight>,
    pub spot_lights: Vec<SpotLight>,
    pub rect_area_lights: Vec<RectAreaLight>,
    pub disk_area_lights: Vec<DiskAreaLight>,
}

impl LightData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_directional(&mut self, light: DirectionalLight) -> usize {
        self.directional_lights.push(light);
        self.directional_lights.len() - 1
    }

    pub fn add_point(&mut self, light: PointLight) -> usize {
        self.point_lights.push(light);
        self.point_lights.len() - 1
    }

    pub fn add_spot(&mut self, light: SpotLight) -> usize {
        self.spot_lights.push(light);
        self.spot_lights.len() - 1
    }

    pub fn add_rect_area(&mut self, light: RectAreaLight) -> usize {
        self.rect_area_lights.push(light);
        self.rect_area_lights.len() - 1
    }

    pub fn add_disk_area(&mut self, light: DiskAreaLight) -> usize {
        self.disk_area_lights.push(light);
        self.disk_area_lights.len() - 1
    }

    pub fn total_count(&self) -> usize {
        self.directional_lights.len()
            + self.point_lights.len()
            + self.spot_lights.len()
            + self.rect_area_lights.len()
            + self.disk_area_lights.len()
    }

    pub fn to_gpu_lights(&self) -> Vec<GpuLight> {
        let mut gpu_lights = Vec::with_capacity(self.total_count());

        for light in &self.directional_lights {
            gpu_lights.push(GpuLight::from_directional(light));
        }
        for light in &self.point_lights {
            gpu_lights.push(GpuLight::from_point(light));
        }
        for light in &self.spot_lights {
            gpu_lights.push(GpuLight::from_spot(light));
        }
        for light in &self.rect_area_lights {
            gpu_lights.push(GpuLight::from_rect_area(light));
        }
        for light in &self.disk_area_lights {
            gpu_lights.push(GpuLight::from_disk_area(light));
        }

        // Ensure at least one light
        if gpu_lights.is_empty() {
            gpu_lights.push(GpuLight::from_directional(&DirectionalLight::default()));
        }

        gpu_lights
    }

    pub fn light_counts(&self) -> [u32; 8] {
        [
            self.directional_lights.len() as u32,
            self.point_lights.len() as u32,
            self.spot_lights.len() as u32,
            self.rect_area_lights.len() as u32,
            self.disk_area_lights.len() as u32,
            0,
            0,
            0,
        ]
    }

    pub fn clear(&mut self) {
        self.directional_lights.clear();
        self.point_lights.clear();
        self.spot_lights.clear();
        self.rect_area_lights.clear();
        self.disk_area_lights.clear();
    }
}
