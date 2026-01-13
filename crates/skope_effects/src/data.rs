// SKOPE Engine - Effect Data Structures
// Phase E1: FlipbookMeta, VatMeta, GPU Instance structures

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

/// Flipbook 루프 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoopMode {
    /// 한 번 재생 후 삭제
    #[default]
    Once,
    /// 무한 반복
    Loop,
    /// 끝에서 정지
    Hold,
    /// 핑퐁 (왕복)
    PingPong,
}

/// 블렌드 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlendMode {
    /// 일반 알파 블렌딩
    #[default]
    Alpha,
    /// 가산 블렌딩 (불, 폭발)
    Additive,
    /// 소프트 가산
    SoftAdditive,
    /// 곱셈 블렌딩
    Multiply,
}

/// VAT 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VatType {
    /// 소프트 바디 (천, 물)
    #[default]
    Soft,
    /// 리지드 바디 (파편)
    Rigid,
    /// 유체 (스플래시)
    Fluid,
}

/// 빌보드 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BillboardType {
    /// 항상 카메라를 향함 (Y-up 유지)
    #[default]
    CameraFacing,
    /// 완전히 카메라를 향함 (roll 포함)
    FullCameraFacing,
    /// Y축 고정 빌보드
    AxisY,
    /// 빌보드 없음 (월드 회전)
    None,
}

/// Flipbook 셰이더 파라미터
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FlipbookShaderParams {
    /// 이미션 강도 (0.0 = 없음, 10.0 = 매우 밝음)
    pub emission_strength: f32,
    /// 색상 틴트 RGB
    pub color_tint: [f32; 3],
    /// 디스토션 강도 (0.0 = 없음)
    pub distortion_strength: f32,
    /// 컬러 랜덤 범위
    pub color_random: f32,
}

impl Default for FlipbookShaderParams {
    fn default() -> Self {
        Self {
            emission_strength: 1.0,
            color_tint: [1.0, 1.0, 1.0],
            distortion_strength: 0.0,
            color_random: 0.0,
        }
    }
}

/// Flipbook 메타데이터 (RON 파일에서 로드)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlipbookMeta {
    /// 텍스처 파일 경로 (PNG)
    pub texture: String,
    /// 그리드 크기 (columns, rows)
    pub grid: (u32, u32),
    /// 총 프레임 수
    pub frame_count: u32,
    /// 초당 프레임 수
    pub fps: f32,
    /// 루프 모드
    #[serde(default)]
    pub loop_mode: LoopMode,
    /// 블렌드 모드
    #[serde(default)]
    pub blend_mode: BlendMode,
    /// 빌보드 타입
    #[serde(default)]
    pub billboard: BillboardType,
    /// 소프트 파티클 활성화
    #[serde(default)]
    pub soft_particle: bool,
    /// 소프트 파티클 페이드 거리
    #[serde(default = "default_depth_fade")]
    pub depth_fade_distance: f32,
    /// 기본 크기 (width, height)
    #[serde(default = "default_size")]
    pub size: (f32, f32),
    /// 셰이더 파라미터
    #[serde(default)]
    pub shader_params: FlipbookShaderParams,
}

fn default_depth_fade() -> f32 {
    0.5
}

fn default_size() -> (f32, f32) {
    (1.0, 1.0)
}

impl Default for FlipbookMeta {
    fn default() -> Self {
        Self {
            texture: String::new(),
            grid: (8, 8),
            frame_count: 64,
            fps: 30.0,
            loop_mode: LoopMode::Once,
            blend_mode: BlendMode::Additive,
            billboard: BillboardType::CameraFacing,
            soft_particle: true,
            depth_fade_distance: 0.5,
            size: (1.0, 1.0),
            shader_params: FlipbookShaderParams::default(),
        }
    }
}

/// VAT 메타데이터 (RON 파일에서 로드)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VatMeta {
    /// 메시 파일 경로 (glTF)
    pub mesh: String,
    /// Position 텍스처 (EXR)
    pub position_texture: String,
    /// Normal 텍스처 (EXR, 선택)
    pub normal_texture: Option<String>,
    /// VAT 타입
    #[serde(default)]
    pub vat_type: VatType,
    /// 바운딩 박스 최소값 (정규화 복원용)
    pub bbox_min: [f32; 3],
    /// 바운딩 박스 최대값 (정규화 복원용)
    pub bbox_max: [f32; 3],
    /// 총 프레임 수
    pub frame_count: u32,
    /// 초당 프레임 수
    pub fps: f32,
    /// 루프 모드
    #[serde(default)]
    pub loop_mode: LoopMode,
    /// 버텍스 수 (텍스처 width와 동일해야 함)
    pub vertex_count: u32,
}

impl Default for VatMeta {
    fn default() -> Self {
        Self {
            mesh: String::new(),
            position_texture: String::new(),
            normal_texture: None,
            vat_type: VatType::Soft,
            bbox_min: [-1.0, -1.0, -1.0],
            bbox_max: [1.0, 1.0, 1.0],
            frame_count: 60,
            fps: 30.0,
            loop_mode: LoopMode::Once,
            vertex_count: 0,
        }
    }
}

/// Flipbook GPU 인스턴스 데이터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FlipbookInstance {
    /// 월드 위치
    pub position: [f32; 3],
    /// 회전 (Z축 라디안)
    pub rotation: f32,
    /// 크기 (width, height)
    pub size: [f32; 2],
    /// 현재 프레임
    pub frame: f32,
    /// 프레임 블렌드 (0.0 ~ 1.0)
    pub frame_blend: f32,
    /// 색상 틴트 RGBA
    pub color: [f32; 4],
    /// 이미션 강도
    pub emission: f32,
    /// 패딩 (16바이트 정렬)
    pub _pad: [f32; 3],
}

impl Default for FlipbookInstance {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: 0.0,
            size: [1.0, 1.0],
            frame: 0.0,
            frame_blend: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            emission: 1.0,
            _pad: [0.0; 3],
        }
    }
}

#[cfg(feature = "gpu")]
impl FlipbookInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FlipbookInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position + rotation
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4, // position.xyz + rotation
                },
                // size + frame + frame_blend
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // emission + pad
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// VAT GPU 인스턴스 데이터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct VatInstance {
    /// Model 행렬 (4x4)
    pub model: [[f32; 4]; 4],
    /// 현재 프레임
    pub frame: f32,
    /// 프레임 블렌드
    pub frame_blend: f32,
    /// 색상 틴트
    pub color: [f32; 4],
    /// 패딩
    pub _pad: [f32; 2],
}

impl Default for VatInstance {
    fn default() -> Self {
        Self {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            frame: 0.0,
            frame_blend: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            _pad: [0.0; 2],
        }
    }
}

/// Flipbook Uniform (셰이더용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FlipbookUniforms {
    /// 그리드 크기 (columns, rows)
    pub grid: [u32; 2],
    /// 총 프레임 수
    pub frame_count: u32,
    /// 소프트 파티클 활성화
    pub soft_particle: u32,
    /// 페이드 거리
    pub depth_fade_distance: f32,
    /// 이미션 강도
    pub emission_strength: f32,
    /// 패딩
    pub _pad: [f32; 2],
}

impl Default for FlipbookUniforms {
    fn default() -> Self {
        Self {
            grid: [8, 8],
            frame_count: 64,
            soft_particle: 1,
            depth_fade_distance: 0.5,
            emission_strength: 1.0,
            _pad: [0.0; 2],
        }
    }
}

/// VAT Uniform (셰이더용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct VatUniforms {
    /// 바운딩 박스 최소값
    pub bbox_min: [f32; 3],
    pub _pad0: f32,
    /// 바운딩 박스 최대값
    pub bbox_max: [f32; 3],
    pub _pad1: f32,
    /// 총 프레임 수
    pub frame_count: u32,
    /// 버텍스 수
    pub vertex_count: u32,
    /// VAT 타입 (0=Soft, 1=Rigid, 2=Fluid)
    pub vat_type: u32,
    pub _pad2: u32,
}

impl Default for VatUniforms {
    fn default() -> Self {
        Self {
            bbox_min: [-1.0, -1.0, -1.0],
            _pad0: 0.0,
            bbox_max: [1.0, 1.0, 1.0],
            _pad1: 0.0,
            frame_count: 60,
            vertex_count: 0,
            vat_type: 0,
            _pad2: 0,
        }
    }
}

/// 로드된 Effect 에셋 (GPU feature 필요)
#[cfg(feature = "gpu")]
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EffectAsset {
    Flipbook {
        meta: FlipbookMeta,
        texture: wgpu::Texture,
        texture_view: wgpu::TextureView,
    },
    Vat {
        meta: VatMeta,
        position_texture: wgpu::Texture,
        position_view: wgpu::TextureView,
        normal_texture: Option<wgpu::Texture>,
        normal_view: Option<wgpu::TextureView>,
        mesh_vertex_buffer: wgpu::Buffer,
        mesh_index_buffer: wgpu::Buffer,
        index_count: u32,
    },
}

/// Effect 에셋 컬렉션 (GPU feature 필요)
#[cfg(feature = "gpu")]
#[derive(Default)]
pub struct EffectAssets {
    pub flipbooks: std::collections::HashMap<String, usize>,
    pub vats: std::collections::HashMap<String, usize>,
    pub assets: Vec<EffectAsset>,
}

#[cfg(feature = "gpu")]
impl EffectAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_flipbook(&self, name: &str) -> Option<&EffectAsset> {
        self.flipbooks.get(name).map(|&idx| &self.assets[idx])
    }

    pub fn get_vat(&self, name: &str) -> Option<&EffectAsset> {
        self.vats.get(name).map(|&idx| &self.assets[idx])
    }
}
