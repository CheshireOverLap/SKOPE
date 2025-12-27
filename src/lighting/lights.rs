// SKOPE Engine - Light Types
// Directional, Point, Spot, Area Lights

use glam::Vec3;
use bytemuck::{Pod, Zeroable};

/// 라이트 타입 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
    AreaRect = 3,
    AreaDisk = 4,
}

/// Directional Light (태양, 달)
#[derive(Debug, Clone, Copy)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub angular_diameter: f32,  // 태양 크기 (soft shadows)
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
            angular_diameter: 0.53,  // 태양 실제 각도
            cast_shadows: true,
            shadow_cascade_count: 4,
            shadow_distance: 100.0,
        }
    }
}

/// Point Light (전구, 횃불)
#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
    pub source_radius: f32,  // Area light 근사
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

/// Spot Light (손전등, 가로등)
#[derive(Debug, Clone, Copy)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
    pub inner_angle: f32,  // radians
    pub outer_angle: f32,  // radians
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
            inner_angle: 0.4,  // ~23 degrees
            outer_angle: 0.6,  // ~34 degrees
            source_radius: 0.0,
            cast_shadows: true,
            shadow_bias: 0.001,
        }
    }
}

/// Area Light - Rectangle
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

/// Area Light - Disk
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

/// GPU용 라이트 데이터 (통합 구조체)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuLight {
    pub position_type: [f32; 4],       // xyz: position, w: light_type
    pub direction_radius: [f32; 4],    // xyz: direction, w: radius
    pub color_intensity: [f32; 4],     // xyz: color, w: intensity
    pub params0: [f32; 4],             // spot: inner/outer angle, area: width/height
    pub params1: [f32; 4],             // source_radius, shadow_bias, etc.
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
            position_type: [light.position.x, light.position.y, light.position.z, LightType::Point as u32 as f32],
            direction_radius: [0.0, 0.0, 0.0, light.radius],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [0.0, 0.0, 0.0, 0.0],
            params1: [light.source_radius, light.shadow_bias, if light.cast_shadows { 1.0 } else { 0.0 }, 0.0],
        }
    }

    pub fn from_spot(light: &SpotLight) -> Self {
        Self {
            position_type: [light.position.x, light.position.y, light.position.z, LightType::Spot as u32 as f32],
            direction_radius: [light.direction.x, light.direction.y, light.direction.z, light.radius],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [light.inner_angle.cos(), light.outer_angle.cos(), 0.0, 0.0],
            params1: [light.source_radius, light.shadow_bias, if light.cast_shadows { 1.0 } else { 0.0 }, 0.0],
        }
    }

    pub fn from_rect_area(light: &RectAreaLight) -> Self {
        Self {
            position_type: [light.position.x, light.position.y, light.position.z, LightType::AreaRect as u32 as f32],
            direction_radius: [light.direction.x, light.direction.y, light.direction.z, 0.0],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [light.width, light.height, light.up.x, light.up.y],
            params1: [light.up.z, if light.two_sided { 1.0 } else { 0.0 }, 0.0, 0.0],
        }
    }

    pub fn from_disk_area(light: &DiskAreaLight) -> Self {
        Self {
            position_type: [light.position.x, light.position.y, light.position.z, LightType::AreaDisk as u32 as f32],
            direction_radius: [light.direction.x, light.direction.y, light.direction.z, light.disk_radius],
            color_intensity: [light.color.x, light.color.y, light.color.z, light.intensity],
            params0: [0.0, 0.0, 0.0, 0.0],
            params1: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// 라이트 매니저
pub struct LightManager {
    pub directional_lights: Vec<DirectionalLight>,
    pub point_lights: Vec<PointLight>,
    pub spot_lights: Vec<SpotLight>,
    pub rect_area_lights: Vec<RectAreaLight>,
    pub disk_area_lights: Vec<DiskAreaLight>,

    // GPU 버퍼
    light_buffer: Option<wgpu::Buffer>,
    light_count_buffer: Option<wgpu::Buffer>,

    dirty: bool,
}

impl LightManager {
    pub fn new() -> Self {
        Self {
            directional_lights: Vec::new(),
            point_lights: Vec::new(),
            spot_lights: Vec::new(),
            rect_area_lights: Vec::new(),
            disk_area_lights: Vec::new(),
            light_buffer: None,
            light_count_buffer: None,
            dirty: true,
        }
    }

    pub fn add_directional(&mut self, light: DirectionalLight) -> usize {
        self.dirty = true;
        self.directional_lights.push(light);
        self.directional_lights.len() - 1
    }

    pub fn add_point(&mut self, light: PointLight) -> usize {
        self.dirty = true;
        self.point_lights.push(light);
        self.point_lights.len() - 1
    }

    pub fn add_spot(&mut self, light: SpotLight) -> usize {
        self.dirty = true;
        self.spot_lights.push(light);
        self.spot_lights.len() - 1
    }

    pub fn add_rect_area(&mut self, light: RectAreaLight) -> usize {
        self.dirty = true;
        self.rect_area_lights.push(light);
        self.rect_area_lights.len() - 1
    }

    pub fn add_disk_area(&mut self, light: DiskAreaLight) -> usize {
        self.dirty = true;
        self.disk_area_lights.push(light);
        self.disk_area_lights.len() - 1
    }

    pub fn total_light_count(&self) -> usize {
        self.directional_lights.len()
            + self.point_lights.len()
            + self.spot_lights.len()
            + self.rect_area_lights.len()
            + self.disk_area_lights.len()
    }

    /// GPU 버퍼 업데이트
    pub fn update_gpu_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.dirty {
            return;
        }

        let mut gpu_lights: Vec<GpuLight> = Vec::with_capacity(self.total_light_count());

        // Directional lights
        for light in &self.directional_lights {
            gpu_lights.push(GpuLight::from_directional(light));
        }

        // Point lights
        for light in &self.point_lights {
            gpu_lights.push(GpuLight::from_point(light));
        }

        // Spot lights
        for light in &self.spot_lights {
            gpu_lights.push(GpuLight::from_spot(light));
        }

        // Area lights (Rect)
        for light in &self.rect_area_lights {
            gpu_lights.push(GpuLight::from_rect_area(light));
        }

        // Area lights (Disk)
        for light in &self.disk_area_lights {
            gpu_lights.push(GpuLight::from_disk_area(light));
        }

        // 최소 1개 보장 (빈 배열 방지)
        if gpu_lights.is_empty() {
            gpu_lights.push(GpuLight::from_directional(&DirectionalLight::default()));
        }

        // 버퍼 생성 또는 업데이트
        let light_data = bytemuck::cast_slice(&gpu_lights);

        if self.light_buffer.is_none() || self.light_buffer.as_ref().unwrap().size() < light_data.len() as u64 {
            self.light_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Light Buffer"),
                size: (light_data.len().max(1) * std::mem::size_of::<GpuLight>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        queue.write_buffer(self.light_buffer.as_ref().unwrap(), 0, light_data);

        // 라이트 카운트 버퍼
        let counts = [
            self.directional_lights.len() as u32,
            self.point_lights.len() as u32,
            self.spot_lights.len() as u32,
            self.rect_area_lights.len() as u32,
            self.disk_area_lights.len() as u32,
            0u32, 0u32, 0u32,  // padding
        ];

        if self.light_count_buffer.is_none() {
            self.light_count_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Light Count Buffer"),
                size: std::mem::size_of_val(&counts) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        queue.write_buffer(
            self.light_count_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&counts),
        );

        self.dirty = false;
    }

    pub fn light_buffer(&self) -> Option<&wgpu::Buffer> {
        self.light_buffer.as_ref()
    }

    pub fn light_count_buffer(&self) -> Option<&wgpu::Buffer> {
        self.light_count_buffer.as_ref()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

impl Default for LightManager {
    fn default() -> Self {
        Self::new()
    }
}
