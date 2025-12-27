// SKOPE Engine - Skin Shading
// Pre-Integrated SSS + 색조작 기반 피부 렌더링

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// 피부 셰이딩 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SkinShadeParams {
    // === SSS ===
    /// SSS 강도 (0 = 없음, 1 = 최대)
    /// SKOPE 권장: 0.4 ~ 0.6
    pub sss_strength: f32,

    /// SSS 색상 (피부 톤에 맞춰 조정)
    pub sss_color: [f32; 3],

    /// SSS 반경 (빛이 퍼지는 정도)
    pub sss_radius: f32,

    // === 색 조작 ===
    /// 그림자 채도 부스트
    pub shadow_saturation_boost: f32,

    /// 그림자 Hue 시프트
    pub shadow_hue_shift: f32,

    /// 트랜지션 샤프니스
    pub transition_sharpness: f32,

    // === 디테일 ===
    /// 모공/피부결 강도
    pub detail_intensity: f32,

    /// 스펙큘러 강도
    pub specular_intensity: f32,

    /// 하이라이트 채도 감소
    pub highlight_saturation_reduce: f32,

    pub _pad: [f32; 2],
}

impl Default for SkinShadeParams {
    fn default() -> Self {
        Self {
            sss_strength: 0.5,
            sss_color: [1.0, 0.4, 0.25], // 따뜻한 피부톤
            sss_radius: 0.01,
            shadow_saturation_boost: 0.15,
            shadow_hue_shift: 0.02,
            transition_sharpness: 0.3,
            detail_intensity: 0.5,
            specular_intensity: 0.4,
            highlight_saturation_reduce: 0.1,
            _pad: [0.0; 2],
        }
    }
}

impl SkinShadeParams {
    /// 동양인 피부톤
    pub fn asian() -> Self {
        Self {
            sss_color: [1.0, 0.45, 0.3],
            ..Default::default()
        }
    }

    /// 백인 피부톤
    pub fn caucasian() -> Self {
        Self {
            sss_color: [1.0, 0.35, 0.2],
            ..Default::default()
        }
    }

    /// 어두운 피부톤
    pub fn dark() -> Self {
        Self {
            sss_color: [0.8, 0.35, 0.25],
            sss_strength: 0.35, // SSS가 덜 보임
            ..Default::default()
        }
    }
}

/// Pre-Integrated SSS LUT 생성
/// 크기: size x size (일반적으로 256x256)
/// U축: NdotL (-1 ~ 1)
/// V축: Curvature (0 ~ 1)
pub fn generate_sss_lut(size: u32) -> Vec<[f32; 4]> {
    let mut lut = Vec::with_capacity((size * size) as usize);

    for y in 0..size {
        for x in 0..size {
            let n_dot_l = (x as f32 / (size - 1) as f32) * 2.0 - 1.0; // -1 to 1
            let curvature = y as f32 / (size - 1) as f32; // 0 to 1

            // Diffusion profile integration
            let diffuse = integrate_diffusion_profile(n_dot_l, curvature);

            lut.push([diffuse.x, diffuse.y, diffuse.z, 1.0]);
        }
    }

    lut
}

/// Burley's normalized diffusion profile 적분
fn integrate_diffusion_profile(n_dot_l: f32, curvature: f32) -> Vec3 {
    // 곡률이 높을수록 빛이 덜 퍼짐
    let s = 1.0 / curvature.max(0.001);

    // RGB 채널별 다른 확산 거리
    // Red: 가장 멀리 확산 (피부의 붉은 톤)
    // Green: 중간
    // Blue: 가장 가까이 (거의 확산 안 함)
    let r = gaussian_integral(n_dot_l, s * 0.5);
    let g = gaussian_integral(n_dot_l, s * 0.25);
    let b = gaussian_integral(n_dot_l, s * 0.1);

    Vec3::new(r, g, b)
}

/// 가우시안 적분 (간단한 근사)
fn gaussian_integral(n_dot_l: f32, width: f32) -> f32 {
    // Half Lambert 기반
    let x = n_dot_l * 0.5 + 0.5;

    // 가우시안 falloff
    let falloff = (-x * x / (2.0 * width * width)).exp();

    // 확산된 결과
    (x + falloff * (1.0 - x)).clamp(0.0, 1.0)
}

/// SSS LUT 텍스처 생성 헬퍼
pub fn create_sss_lut_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: u32,
) -> wgpu::Texture {
    let lut_data = generate_sss_lut(size);

    // f32x4 → u8x4 변환
    let rgba_data: Vec<u8> = lut_data
        .iter()
        .flat_map(|pixel| {
            [
                (pixel[0] * 255.0) as u8,
                (pixel[1] * 255.0) as u8,
                (pixel[2] * 255.0) as u8,
                255u8,
            ]
        })
        .collect();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSS LUT Texture"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
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
        &rgba_data,
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
