// SKOPE Engine - Light Probes
// Spherical Harmonics (SH9) based Global Illumination

use glam::Vec3;
use bytemuck::{Pod, Zeroable};
use std::f32::consts::PI;

/// SH L2 (9 coefficients per channel = 27 floats)
/// RGB 각각 9개의 계수
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SphericalHarmonicsL2 {
    // Band 0 (1 coefficient)
    pub r0: [f32; 4],  // xyz = L0 RGB, w = L1_-1 R
    // Band 1 (3 coefficients)
    pub r1: [f32; 4],  // x = L1_-1 G, y = L1_-1 B, z = L1_0 R, w = L1_0 G
    pub r2: [f32; 4],  // x = L1_0 B, y = L1_1 R, z = L1_1 G, w = L1_1 B
    // Band 2 (5 coefficients)
    pub r3: [f32; 4],  // x = L2_-2 R, y = L2_-2 G, z = L2_-2 B, w = L2_-1 R
    pub r4: [f32; 4],  // x = L2_-1 G, y = L2_-1 B, z = L2_0 R, w = L2_0 G
    pub r5: [f32; 4],  // x = L2_0 B, y = L2_1 R, z = L2_1 G, w = L2_1 B
    pub r6: [f32; 4],  // x = L2_2 R, y = L2_2 G, z = L2_2 B, w = unused
}

impl Default for SphericalHarmonicsL2 {
    fn default() -> Self {
        Self {
            r0: [0.0; 4],
            r1: [0.0; 4],
            r2: [0.0; 4],
            r3: [0.0; 4],
            r4: [0.0; 4],
            r5: [0.0; 4],
            r6: [0.0; 4],
        }
    }
}

impl SphericalHarmonicsL2 {
    /// 단색 환경광으로 초기화
    pub fn from_ambient(color: Vec3) -> Self {
        // L0 (DC term) = sqrt(4*PI) * color / PI = 2 * color
        let scale = 2.0 * (4.0 * PI).sqrt() / (4.0 * PI);
        let l0 = color * scale;

        Self {
            r0: [l0.x, l0.y, l0.z, 0.0],
            ..Default::default()
        }
    }

    /// 방향성 광원에서 SH 계수 추가
    pub fn add_directional_light(&mut self, direction: Vec3, color: Vec3, intensity: f32) {
        let radiance = color * intensity;
        let dir = direction.normalize();

        // SH 기저 함수 계산
        let y00 = 0.282095;  // 1 / (2 * sqrt(PI))
        let y1_1 = 0.488603 * dir.y;
        let y10 = 0.488603 * dir.z;
        let y11 = 0.488603 * dir.x;
        let y2_2 = 1.092548 * dir.x * dir.y;
        let y2_1 = 1.092548 * dir.y * dir.z;
        let y20 = 0.315392 * (3.0 * dir.z * dir.z - 1.0);
        let y21 = 1.092548 * dir.x * dir.z;
        let y22 = 0.546274 * (dir.x * dir.x - dir.y * dir.y);

        // 각 채널에 추가
        self.r0[0] += radiance.x * y00;
        self.r0[1] += radiance.y * y00;
        self.r0[2] += radiance.z * y00;

        // L1
        self.r0[3] += radiance.x * y1_1;
        self.r1[0] += radiance.y * y1_1;
        self.r1[1] += radiance.z * y1_1;

        self.r1[2] += radiance.x * y10;
        self.r1[3] += radiance.y * y10;
        self.r2[0] += radiance.z * y10;

        self.r2[1] += radiance.x * y11;
        self.r2[2] += radiance.y * y11;
        self.r2[3] += radiance.z * y11;

        // L2
        self.r3[0] += radiance.x * y2_2;
        self.r3[1] += radiance.y * y2_2;
        self.r3[2] += radiance.z * y2_2;

        self.r3[3] += radiance.x * y2_1;
        self.r4[0] += radiance.y * y2_1;
        self.r4[1] += radiance.z * y2_1;

        self.r4[2] += radiance.x * y20;
        self.r4[3] += radiance.y * y20;
        self.r5[0] += radiance.z * y20;

        self.r5[1] += radiance.x * y21;
        self.r5[2] += radiance.y * y21;
        self.r5[3] += radiance.z * y21;

        self.r6[0] += radiance.x * y22;
        self.r6[1] += radiance.y * y22;
        self.r6[2] += radiance.z * y22;
    }

    /// 주어진 방향의 irradiance 평가
    pub fn evaluate(&self, normal: Vec3) -> Vec3 {
        let n = normal.normalize();

        // Convolve SH with cosine lobe for irradiance
        let c1 = 0.429043;
        let c2 = 0.511664;
        let c3 = 0.743125;
        let c4 = 0.886227;
        let c5 = 0.247708;

        // L0
        let l00_r = self.r0[0];
        let l00_g = self.r0[1];
        let l00_b = self.r0[2];

        // L1
        let l1_1_r = self.r0[3];
        let l1_1_g = self.r1[0];
        let l1_1_b = self.r1[1];
        let l10_r = self.r1[2];
        let l10_g = self.r1[3];
        let l10_b = self.r2[0];
        let l11_r = self.r2[1];
        let l11_g = self.r2[2];
        let l11_b = self.r2[3];

        // L2
        let l2_2_r = self.r3[0];
        let l2_2_g = self.r3[1];
        let l2_2_b = self.r3[2];
        let l2_1_r = self.r3[3];
        let l2_1_g = self.r4[0];
        let l2_1_b = self.r4[1];
        let l20_r = self.r4[2];
        let l20_g = self.r4[3];
        let l20_b = self.r5[0];
        let l21_r = self.r5[1];
        let l21_g = self.r5[2];
        let l21_b = self.r5[3];
        let l22_r = self.r6[0];
        let l22_g = self.r6[1];
        let l22_b = self.r6[2];

        // Irradiance reconstruction
        let r = c4 * l00_r
            + 2.0 * c2 * (l11_r * n.x + l1_1_r * n.y + l10_r * n.z)
            + 2.0 * c1 * l2_2_r * n.x * n.y
            + 2.0 * c1 * l2_1_r * n.y * n.z
            + c3 * l20_r * n.z * n.z - c5 * l20_r
            + 2.0 * c1 * l21_r * n.x * n.z
            + c1 * l22_r * (n.x * n.x - n.y * n.y);

        let g = c4 * l00_g
            + 2.0 * c2 * (l11_g * n.x + l1_1_g * n.y + l10_g * n.z)
            + 2.0 * c1 * l2_2_g * n.x * n.y
            + 2.0 * c1 * l2_1_g * n.y * n.z
            + c3 * l20_g * n.z * n.z - c5 * l20_g
            + 2.0 * c1 * l21_g * n.x * n.z
            + c1 * l22_g * (n.x * n.x - n.y * n.y);

        let b = c4 * l00_b
            + 2.0 * c2 * (l11_b * n.x + l1_1_b * n.y + l10_b * n.z)
            + 2.0 * c1 * l2_2_b * n.x * n.y
            + 2.0 * c1 * l2_1_b * n.y * n.z
            + c3 * l20_b * n.z * n.z - c5 * l20_b
            + 2.0 * c1 * l21_b * n.x * n.z
            + c1 * l22_b * (n.x * n.x - n.y * n.y);

        Vec3::new(r.max(0.0), g.max(0.0), b.max(0.0))
    }

    /// 두 SH 보간
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let lerp_arr = |a: [f32; 4], b: [f32; 4], t: f32| -> [f32; 4] {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
                a[3] + (b[3] - a[3]) * t,
            ]
        };

        Self {
            r0: lerp_arr(a.r0, b.r0, t),
            r1: lerp_arr(a.r1, b.r1, t),
            r2: lerp_arr(a.r2, b.r2, t),
            r3: lerp_arr(a.r3, b.r3, t),
            r4: lerp_arr(a.r4, b.r4, t),
            r5: lerp_arr(a.r5, b.r5, t),
            r6: lerp_arr(a.r6, b.r6, t),
        }
    }

    /// 스칼라 곱
    pub fn scale(&self, s: f32) -> Self {
        let scale_arr = |a: [f32; 4], s: f32| -> [f32; 4] {
            [a[0] * s, a[1] * s, a[2] * s, a[3] * s]
        };

        Self {
            r0: scale_arr(self.r0, s),
            r1: scale_arr(self.r1, s),
            r2: scale_arr(self.r2, s),
            r3: scale_arr(self.r3, s),
            r4: scale_arr(self.r4, s),
            r5: scale_arr(self.r5, s),
            r6: scale_arr(self.r6, s),
        }
    }
}

/// Light Probe (공간상 위치 + SH)
#[derive(Debug, Clone)]
pub struct LightProbe {
    pub position: Vec3,
    pub sh: SphericalHarmonicsL2,
    pub influence_radius: f32,
    pub priority: i32,  // 높을수록 우선
}

impl LightProbe {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            sh: SphericalHarmonicsL2::default(),
            influence_radius: 10.0,
            priority: 0,
        }
    }

    pub fn with_ambient(mut self, color: Vec3) -> Self {
        self.sh = SphericalHarmonicsL2::from_ambient(color);
        self
    }
}

/// GPU용 Light Probe 데이터
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuLightProbe {
    pub position_radius: [f32; 4],  // xyz = position, w = radius
    pub sh: SphericalHarmonicsL2,
}

/// Light Probe Grid (균일 그리드)
pub struct LightProbeGrid {
    probes: Vec<LightProbe>,
    grid_min: Vec3,
    grid_max: Vec3,
    grid_size: [u32; 3],
    cell_size: Vec3,

    // GPU
    probe_buffer: Option<wgpu::Buffer>,
    grid_info_buffer: Option<wgpu::Buffer>,
}

impl LightProbeGrid {
    pub fn new(min: Vec3, max: Vec3, size: [u32; 3]) -> Self {
        let extent = max - min;
        let cell_size = Vec3::new(
            extent.x / size[0] as f32,
            extent.y / size[1] as f32,
            extent.z / size[2] as f32,
        );

        // 각 셀 중심에 probe 생성
        let mut probes = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);

        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let pos = min + Vec3::new(
                        (x as f32 + 0.5) * cell_size.x,
                        (y as f32 + 0.5) * cell_size.y,
                        (z as f32 + 0.5) * cell_size.z,
                    );
                    probes.push(LightProbe::new(pos));
                }
            }
        }

        Self {
            probes,
            grid_min: min,
            grid_max: max,
            grid_size: size,
            cell_size,
            probe_buffer: None,
            grid_info_buffer: None,
        }
    }

    /// 위치를 그리드 인덱스로 변환
    pub fn position_to_index(&self, pos: Vec3) -> Option<[u32; 3]> {
        let local = pos - self.grid_min;
        let idx = [
            (local.x / self.cell_size.x) as u32,
            (local.y / self.cell_size.y) as u32,
            (local.z / self.cell_size.z) as u32,
        ];

        if idx[0] < self.grid_size[0] && idx[1] < self.grid_size[1] && idx[2] < self.grid_size[2] {
            Some(idx)
        } else {
            None
        }
    }

    /// 그리드 인덱스를 flat 인덱스로
    pub fn flatten_index(&self, idx: [u32; 3]) -> usize {
        (idx[2] * self.grid_size[1] * self.grid_size[0]
            + idx[1] * self.grid_size[0]
            + idx[0]) as usize
    }

    /// 특정 위치의 probe 가져오기
    pub fn get_probe_at(&self, pos: Vec3) -> Option<&LightProbe> {
        let idx = self.position_to_index(pos)?;
        let flat = self.flatten_index(idx);
        self.probes.get(flat)
    }

    /// 특정 위치의 probe 수정
    pub fn get_probe_at_mut(&mut self, pos: Vec3) -> Option<&mut LightProbe> {
        let idx = self.position_to_index(pos)?;
        let flat = self.flatten_index(idx);
        self.probes.get_mut(flat)
    }

    /// Trilinear 보간으로 SH 얻기
    pub fn sample_sh(&self, pos: Vec3) -> SphericalHarmonicsL2 {
        let local = (pos - self.grid_min) / self.cell_size;

        // Clamp to grid bounds
        let x = local.x.clamp(0.0, self.grid_size[0] as f32 - 1.0);
        let y = local.y.clamp(0.0, self.grid_size[1] as f32 - 1.0);
        let z = local.z.clamp(0.0, self.grid_size[2] as f32 - 1.0);

        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let z0 = z.floor() as usize;
        let x1 = (x0 + 1).min(self.grid_size[0] as usize - 1);
        let y1 = (y0 + 1).min(self.grid_size[1] as usize - 1);
        let z1 = (z0 + 1).min(self.grid_size[2] as usize - 1);

        let fx = x.fract();
        let fy = y.fract();
        let fz = z.fract();

        // 8 corners
        let get_sh = |ix: usize, iy: usize, iz: usize| -> SphericalHarmonicsL2 {
            let flat = iz * (self.grid_size[1] * self.grid_size[0]) as usize
                + iy * self.grid_size[0] as usize
                + ix;
            self.probes.get(flat).map_or(SphericalHarmonicsL2::default(), |p| p.sh)
        };

        let c000 = get_sh(x0, y0, z0);
        let c100 = get_sh(x1, y0, z0);
        let c010 = get_sh(x0, y1, z0);
        let c110 = get_sh(x1, y1, z0);
        let c001 = get_sh(x0, y0, z1);
        let c101 = get_sh(x1, y0, z1);
        let c011 = get_sh(x0, y1, z1);
        let c111 = get_sh(x1, y1, z1);

        // Trilinear interpolation
        let c00 = SphericalHarmonicsL2::lerp(&c000, &c100, fx);
        let c10 = SphericalHarmonicsL2::lerp(&c010, &c110, fx);
        let c01 = SphericalHarmonicsL2::lerp(&c001, &c101, fx);
        let c11 = SphericalHarmonicsL2::lerp(&c011, &c111, fx);

        let c0 = SphericalHarmonicsL2::lerp(&c00, &c10, fy);
        let c1 = SphericalHarmonicsL2::lerp(&c01, &c11, fy);

        SphericalHarmonicsL2::lerp(&c0, &c1, fz)
    }

    /// GPU 버퍼 업데이트
    pub fn update_gpu_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let gpu_probes: Vec<GpuLightProbe> = self.probes.iter().map(|p| {
            GpuLightProbe {
                position_radius: [
                    p.position.x,
                    p.position.y,
                    p.position.z,
                    p.influence_radius,
                ],
                sh: p.sh,
            }
        }).collect();

        let probe_data = bytemuck::cast_slice(&gpu_probes);

        if self.probe_buffer.is_none() {
            self.probe_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Light Probe Buffer"),
                size: probe_data.len() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        queue.write_buffer(self.probe_buffer.as_ref().unwrap(), 0, probe_data);

        // Grid info
        #[repr(C)]
        #[derive(Pod, Zeroable, Clone, Copy)]
        struct GridInfo {
            min: [f32; 4],
            max: [f32; 4],
            size: [u32; 3],
            _pad: u32,
            cell_size: [f32; 4],
        }

        let grid_info = GridInfo {
            min: [self.grid_min.x, self.grid_min.y, self.grid_min.z, 0.0],
            max: [self.grid_max.x, self.grid_max.y, self.grid_max.z, 0.0],
            size: self.grid_size,
            _pad: 0,
            cell_size: [self.cell_size.x, self.cell_size.y, self.cell_size.z, 0.0],
        };

        if self.grid_info_buffer.is_none() {
            self.grid_info_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Light Probe Grid Info"),
                size: std::mem::size_of::<GridInfo>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        queue.write_buffer(
            self.grid_info_buffer.as_ref().unwrap(),
            0,
            bytemuck::bytes_of(&grid_info),
        );
    }

    pub fn probe_buffer(&self) -> Option<&wgpu::Buffer> {
        self.probe_buffer.as_ref()
    }

    pub fn grid_info_buffer(&self) -> Option<&wgpu::Buffer> {
        self.grid_info_buffer.as_ref()
    }

    pub fn probes(&self) -> &[LightProbe] {
        &self.probes
    }

    pub fn probes_mut(&mut self) -> &mut [LightProbe] {
        &mut self.probes
    }
}

