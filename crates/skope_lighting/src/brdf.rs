// SKOPE Engine - PBR BRDF Functions
// Cook-Torrance with GGX/Trowbridge-Reitz

use glam::Vec3;
use std::f32::consts::PI;

/// PBR Material 파라미터
#[derive(Debug, Clone, Copy)]
pub struct PBRMaterial {
    pub base_color: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,  // 비금속 F0 조정
    pub ao: f32,
}

impl Default for PBRMaterial {
    fn default() -> Self {
        Self {
            base_color: Vec3::new(0.8, 0.8, 0.8),
            metallic: 0.0,
            roughness: 0.5,
            reflectance: 0.5,  // 약 4% F0 (플라스틱)
            ao: 1.0,
        }
    }
}

impl PBRMaterial {
    /// F0 계산 (Fresnel at normal incidence)
    pub fn f0(&self) -> Vec3 {
        // 비금속: reflectance 기반
        // 금속: base_color
        let dielectric_f0 = 0.16 * self.reflectance * self.reflectance;
        let dielectric = Vec3::splat(dielectric_f0);

        // Metallic lerp
        dielectric * (1.0 - self.metallic) + self.base_color * self.metallic
    }

    /// Diffuse color (금속은 없음)
    pub fn diffuse_color(&self) -> Vec3 {
        self.base_color * (1.0 - self.metallic)
    }
}

// =============================================================================
// CPU측 BRDF 함수 (베이킹, 프리컴퓨트용)
// =============================================================================

/// GGX/Trowbridge-Reitz Normal Distribution Function
pub fn d_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h2 = n_dot_h * n_dot_h;

    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    a2 / (PI * denom * denom)
}

/// Smith's Schlick-GGX Geometry Function
pub fn g_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;  // Direct lighting용

    n_dot_v / (n_dot_v * (1.0 - k) + k)
}

/// Smith's G2 (View + Light)
pub fn g_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let ggx_v = g_schlick_ggx(n_dot_v, roughness);
    let ggx_l = g_schlick_ggx(n_dot_l, roughness);
    ggx_v * ggx_l
}

/// Fresnel-Schlick
pub fn f_schlick(cos_theta: f32, f0: Vec3) -> Vec3 {
    let t = 1.0 - cos_theta;
    let t2 = t * t;
    let t5 = t2 * t2 * t;
    f0 + (Vec3::ONE - f0) * t5
}

/// Fresnel-Schlick with roughness (IBL용)
pub fn f_schlick_roughness(cos_theta: f32, f0: Vec3, roughness: f32) -> Vec3 {
    let t = 1.0 - cos_theta;
    let t2 = t * t;
    let t5 = t2 * t2 * t;
    let max_f0 = Vec3::splat(1.0 - roughness);
    f0 + (max_f0.max(f0) - f0) * t5
}

/// Cook-Torrance BRDF (Specular)
pub fn cook_torrance_specular(
    n_dot_h: f32,
    n_dot_v: f32,
    n_dot_l: f32,
    h_dot_v: f32,
    f0: Vec3,
    roughness: f32,
) -> Vec3 {
    let d = d_ggx(n_dot_h, roughness);
    let g = g_smith(n_dot_v, n_dot_l, roughness);
    let f = f_schlick(h_dot_v, f0);

    let denom = 4.0 * n_dot_v * n_dot_l + 0.0001;
    f * (d * g / denom)
}

/// Lambert Diffuse
pub fn lambert_diffuse(diffuse_color: Vec3) -> Vec3 {
    diffuse_color / PI
}

// =============================================================================
// BRDF LUT Generation (Split-Sum Approximation)
// =============================================================================

/// Hammersley 시퀀스
fn hammersley(i: u32, n: u32) -> (f32, f32) {
    let mut bits = i;
    bits = bits.rotate_right(16);
    bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
    bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
    bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
    bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);

    let radical_inverse = bits as f32 * 2.328_306_4e-10;
    (i as f32 / n as f32, radical_inverse)
}

/// Importance sample GGX
fn importance_sample_ggx(xi: (f32, f32), roughness: f32) -> Vec3 {
    let a = roughness * roughness;

    let phi = 2.0 * PI * xi.0;
    let cos_theta = ((1.0 - xi.1) / (1.0 + (a * a - 1.0) * xi.1)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

    Vec3::new(
        phi.cos() * sin_theta,
        phi.sin() * sin_theta,
        cos_theta,
    )
}

/// BRDF LUT 생성 (512x512)
pub fn generate_brdf_lut(size: usize) -> Vec<[f32; 2]> {
    let mut lut = vec![[0.0f32; 2]; size * size];
    let sample_count = 1024u32;

    for y in 0..size {
        let n_dot_v = (y as f32 + 0.5) / size as f32;
        let n_dot_v = n_dot_v.max(0.001);  // 0 방지

        let v = Vec3::new(
            (1.0 - n_dot_v * n_dot_v).sqrt(),
            0.0,
            n_dot_v,
        );
        for x in 0..size {
            let roughness = (x as f32 + 0.5) / size as f32;
            let roughness = roughness.max(0.01);  // 0 roughness 방지

            let mut a = 0.0f32;
            let mut b = 0.0f32;

            for i in 0..sample_count {
                let xi = hammersley(i, sample_count);
                let h = importance_sample_ggx(xi, roughness);
                let l = 2.0 * v.dot(h) * h - v;

                let n_dot_l = l.z.max(0.0);
                let n_dot_h = h.z.max(0.0);
                let v_dot_h = v.dot(h).max(0.0);

                if n_dot_l > 0.0 {
                    let g = g_smith(n_dot_v, n_dot_l, roughness);
                    let g_vis = g * v_dot_h / (n_dot_h * n_dot_v);
                    let fc = (1.0 - v_dot_h).powi(5);

                    a += (1.0 - fc) * g_vis;
                    b += fc * g_vis;
                }
            }

            let idx = y * size + x;
            lut[idx] = [a / sample_count as f32, b / sample_count as f32];
        }
    }

    lut
}

/// BRDF LUT 텍스처 생성
#[cfg(feature = "gpu")]
pub fn create_brdf_lut_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: u32,
) -> wgpu::Texture {
    let lut_data = generate_brdf_lut(size as usize);

    // RG16Float로 변환
    let mut texture_data: Vec<u8> = Vec::with_capacity(lut_data.len() * 4);
    for [r, g] in lut_data {
        // f32 -> f16 변환 (간단한 근사)
        let r_bits = half::f16::from_f32(r).to_bits();
        let g_bits = half::f16::from_f32(g).to_bits();
        texture_data.extend_from_slice(&r_bits.to_le_bytes());
        texture_data.extend_from_slice(&g_bits.to_le_bytes());
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("BRDF LUT"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texture_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size * 4),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );

    texture
}
