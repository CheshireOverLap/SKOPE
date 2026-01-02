// SKOPE Engine - Skin Shading
// Pre-Integrated SSS + 색조작 기반 피부 렌더링

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// SSS LUT 타입 (피부 두께에 따른 프로파일)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SssLutType {
    /// 일반 피부 (볼, 팔 등)
    Normal = 0,
    /// 얇은 피부 (귀, 코, 손가락 끝)
    Thin = 1,
    /// 두꺼운 피부 (이마, 등)
    Thick = 2,
}

impl SssLutType {
    /// LUT 배열의 총 레이어 수
    pub const COUNT: u32 = 3;

    /// 피부 두께에 따른 확산 스케일 계수
    fn diffusion_scale(&self) -> f32 {
        match self {
            SssLutType::Normal => 1.0,
            SssLutType::Thin => 1.8,   // 더 많이 확산 (빛이 통과)
            SssLutType::Thick => 0.5,  // 덜 확산
        }
    }
}

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

    /// SSS LUT 타입 인덱스 (0=Normal, 1=Thin, 2=Thick)
    pub lut_type_index: u32,

    pub _pad: f32,
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
            lut_type_index: 0, // SssLutType::Normal
            _pad: 0.0,
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

/// Pre-Integrated SSS LUT 생성 (단일 타입)
/// 크기: size x size (일반적으로 256x256)
/// U축: NdotL (-1 ~ 1)
/// V축: Curvature (0 ~ 1)
pub fn generate_sss_lut(size: u32) -> Vec<[f32; 4]> {
    generate_sss_lut_for_type(size, SssLutType::Normal)
}

/// 특정 LUT 타입에 대한 SSS LUT 생성
pub fn generate_sss_lut_for_type(size: u32, lut_type: SssLutType) -> Vec<[f32; 4]> {
    let mut lut = Vec::with_capacity((size * size) as usize);
    let scale = lut_type.diffusion_scale();

    for y in 0..size {
        for x in 0..size {
            let n_dot_l = (x as f32 / (size - 1) as f32) * 2.0 - 1.0; // -1 to 1
            let curvature = y as f32 / (size - 1) as f32; // 0 to 1

            // Diffusion profile integration with type-specific scaling
            let diffuse = integrate_diffusion_profile_scaled(n_dot_l, curvature, scale);

            lut.push([diffuse.x, diffuse.y, diffuse.z, 1.0]);
        }
    }

    lut
}

/// 3개 레이어의 SSS LUT 배열 생성 (Normal, Thin, Thick)
/// 반환: size x size x 3 크기의 데이터
pub fn generate_sss_lut_array(size: u32) -> Vec<[f32; 4]> {
    let layer_size = (size * size) as usize;
    let mut lut_array = Vec::with_capacity(layer_size * SssLutType::COUNT as usize);

    // 각 LUT 타입별로 생성
    for lut_type in [SssLutType::Normal, SssLutType::Thin, SssLutType::Thick] {
        let layer_data = generate_sss_lut_for_type(size, lut_type);
        lut_array.extend(layer_data);
    }

    lut_array
}

/// Burley's normalized diffusion profile 적분
fn integrate_diffusion_profile(n_dot_l: f32, curvature: f32) -> Vec3 {
    integrate_diffusion_profile_scaled(n_dot_l, curvature, 1.0)
}

/// 스케일 적용된 diffusion profile 적분
fn integrate_diffusion_profile_scaled(n_dot_l: f32, curvature: f32, scale: f32) -> Vec3 {
    // 곡률이 높을수록 빛이 덜 퍼짐
    let s = 1.0 / curvature.max(0.001);

    // RGB 채널별 다른 확산 거리 (스케일 적용)
    // Red: 가장 멀리 확산 (피부의 붉은 톤)
    // Green: 중간
    // Blue: 가장 가까이 (거의 확산 안 함)
    let r = gaussian_integral(n_dot_l, s * 0.5 * scale);
    let g = gaussian_integral(n_dot_l, s * 0.25 * scale);
    let b = gaussian_integral(n_dot_l, s * 0.1 * scale);

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

/// SSS LUT 배열 텍스처 생성 (3 레이어: Normal, Thin, Thick)
/// texture_2d_array<f32>로 셰이더에서 사용
pub fn create_sss_lut_array_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: u32,
) -> wgpu::Texture {
    let lut_array_data = generate_sss_lut_array(size);

    // f32x4 → u8x4 변환
    let rgba_data: Vec<u8> = lut_array_data
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
        label: Some("SSS LUT Array Texture"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: SssLutType::COUNT,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // 각 레이어별로 데이터 업로드
    let bytes_per_layer = (size * size * 4) as usize;
    for layer in 0..SssLutType::COUNT {
        let offset = layer as usize * bytes_per_layer;
        let layer_data = &rgba_data[offset..offset + bytes_per_layer];

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            layer_data,
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
    }

    texture
}

/// SSS LUT 배열 텍스처 뷰 생성 헬퍼
pub fn create_sss_lut_array_view(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("SSS LUT Array View"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(SssLutType::COUNT),
    })
}
