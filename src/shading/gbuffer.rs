// SKOPE Engine - G-Buffer Layout
// Deferred Rendering용 데이터 구조

use bytemuck::{Pod, Zeroable};

use super::{EyeShadeParams, FaceShadeParams, HairShadowProxyParams, SkinShadeParams};

/// G-Buffer 레이아웃 (128 bits per pixel, Mali 호환)
/// RT0: RGBA8 - Albedo.rgb + Metallic
/// RT1: RGBA8 - Normal.xy (octahedron) + Roughness + ShadingModelID
/// RT2: RGBA8 - Custom Data (모델별 다름)
/// RT3: Depth32Float
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GBufferPixel {
    // RT0
    pub albedo: [f32; 3],
    pub metallic: f32,

    // RT1
    pub normal_encoded: [f32; 2], // Octahedron encoded
    pub roughness: f32,
    pub shading_model_id: f32, // 0~1로 정규화된 ID

    // RT2 (모델별 해석 다름)
    pub custom_data: [f32; 4],
}

/// Face 모델의 Custom Data (RT2) 해석
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FaceCustomData {
    /// 노멀 시프트 마스크 (텍스처에서)
    pub normal_shift_mask: f32,
    /// SSS용 Curvature
    pub curvature: f32,
    /// 프리컴퓨트된 그림자 (hair shadow proxy 등)
    pub shadow_factor: f32,
    /// Ambient Occlusion
    pub ao: f32,
}

/// Skin 모델의 Custom Data (RT2) 해석
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SkinCustomData {
    /// SSS용 Curvature
    pub curvature: f32,
    /// 피부 두께 (SSS 강도 조절)
    pub thickness: f32,
    /// 그림자 팩터
    pub shadow_factor: f32,
    /// AO
    pub ao: f32,
}

/// Eye 모델의 Custom Data (RT2) 해석
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EyeCustomData {
    /// Parallax 적용된 UV
    pub iris_uv: [f32; 2],
    /// 동공 마스크
    pub pupil_mask: f32,
    /// 각막 마스크
    pub cornea_mask: f32,
}

/// Octahedron Normal Encoding (GPU에서도 동일 구현)
pub fn encode_normal_octahedron(n: [f32; 3]) -> [f32; 2] {
    let sum = n[0].abs() + n[1].abs() + n[2].abs();
    let mut p = [n[0] / sum, n[1] / sum];

    if n[2] < 0.0 {
        let sign_x = if p[0] >= 0.0 { 1.0 } else { -1.0 };
        let sign_y = if p[1] >= 0.0 { 1.0 } else { -1.0 };
        p = [(1.0 - p[1].abs()) * sign_x, (1.0 - p[0].abs()) * sign_y];
    }

    // 0~1 범위로 변환
    [p[0] * 0.5 + 0.5, p[1] * 0.5 + 0.5]
}

/// Octahedron Normal Decoding
pub fn decode_normal_octahedron(encoded: [f32; 2]) -> [f32; 3] {
    // 0~1에서 -1~1로 변환
    let p = [encoded[0] * 2.0 - 1.0, encoded[1] * 2.0 - 1.0];

    let mut n = [p[0], p[1], 1.0 - p[0].abs() - p[1].abs()];

    if n[2] < 0.0 {
        let sign_x = if n[0] >= 0.0 { 1.0 } else { -1.0 };
        let sign_y = if n[1] >= 0.0 { 1.0 } else { -1.0 };
        n[0] = (1.0 - n[1].abs()) * sign_x;
        n[1] = (1.0 - n[0].abs()) * sign_y;
    }

    // Normalize
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    [n[0] / len, n[1] / len, n[2] / len]
}

/// 캐릭터 전체 셰이딩 파라미터 (통합)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CharacterShadeData {
    pub face: FaceShadeParams,
    pub skin: SkinShadeParams,
    pub eye: EyeShadeParams,
    pub hair_shadow: HairShadowProxyParams,
}

impl Default for CharacterShadeData {
    fn default() -> Self {
        Self {
            face: FaceShadeParams::default(),
            skin: SkinShadeParams::default(),
            eye: EyeShadeParams::default(),
            hair_shadow: HairShadowProxyParams::default(),
        }
    }
}

/// G-Buffer 텍스처 포맷 정의
pub struct GBufferFormats;

impl GBufferFormats {
    /// RT0: Albedo + Metallic
    pub const RT0: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    /// RT1: Normal + Roughness + ModelID
    pub const RT1: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    /// RT2: Custom Data
    pub const RT2: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    /// Depth
    pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// 모든 Color RT 포맷
    pub fn color_formats() -> [wgpu::TextureFormat; 3] {
        [Self::RT0, Self::RT1, Self::RT2]
    }
}

/// G-Buffer 리소스 생성 헬퍼
pub struct GBufferResources {
    pub rt0: wgpu::Texture,
    pub rt1: wgpu::Texture,
    pub rt2: wgpu::Texture,
    pub depth: wgpu::Texture,

    pub rt0_view: wgpu::TextureView,
    pub rt1_view: wgpu::TextureView,
    pub rt2_view: wgpu::TextureView,
    pub depth_view: wgpu::TextureView,
}

impl GBufferResources {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let create_texture = |label: &str, format: wgpu::TextureFormat| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        let rt0 = create_texture("GBuffer RT0", GBufferFormats::RT0);
        let rt1 = create_texture("GBuffer RT1", GBufferFormats::RT1);
        let rt2 = create_texture("GBuffer RT2", GBufferFormats::RT2);
        let depth = create_texture("GBuffer Depth", GBufferFormats::DEPTH);

        let rt0_view = rt0.create_view(&wgpu::TextureViewDescriptor::default());
        let rt1_view = rt1.create_view(&wgpu::TextureViewDescriptor::default());
        let rt2_view = rt2.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            rt0,
            rt1,
            rt2,
            depth,
            rt0_view,
            rt1_view,
            rt2_view,
            depth_view,
        }
    }

    /// 사이즈 변경 시 재생성
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_encoding_roundtrip() {
        let normals = [
            [0.0, 0.0, 1.0],  // Forward
            [0.0, 0.0, -1.0], // Back
            [1.0, 0.0, 0.0],  // Right
            [0.0, 1.0, 0.0],  // Up
            [0.577, 0.577, 0.577], // Diagonal
        ];

        for n in normals {
            let encoded = encode_normal_octahedron(n);
            let decoded = decode_normal_octahedron(encoded);

            let diff = (n[0] - decoded[0]).abs()
                + (n[1] - decoded[1]).abs()
                + (n[2] - decoded[2]).abs();

            assert!(diff < 0.01, "Normal encoding roundtrip failed for {:?}", n);
        }
    }
}
