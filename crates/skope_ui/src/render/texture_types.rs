//! Texture Types — 텍스처 관련 확장 타입
//!
//! 아이콘, 업데이트 가능 텍스처, 비아틀라스 텍스처 등
//! UE5 Slate의 텍스처 인프라에 해당합니다.

use glam::Vec2;

use crate::core::{Color, TextureId};

// ============================================================================
// TextureFormat
// ============================================================================

/// 텍스처 포맷
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SlateTextureFormat {
    /// RGBA 8비트 (기본)
    #[default]
    Rgba8,
    /// BGRA 8비트
    Bgra8,
    /// 단일 채널 (알파 맵)
    R8,
    /// 단일 채널 16비트 (SDF용)
    R16,
    /// 16비트 float RGBA (HDR)
    Rgba16Float,
}

impl SlateTextureFormat {
    /// 픽셀당 바이트 수
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            Self::Rgba8 | Self::Bgra8 => 4,
            Self::R8 => 1,
            Self::R16 => 2,
            Self::Rgba16Float => 8,
        }
    }

    /// 채널 수
    pub fn channel_count(&self) -> u32 {
        match self {
            Self::Rgba8 | Self::Bgra8 | Self::Rgba16Float => 4,
            Self::R8 | Self::R16 => 1,
        }
    }
}

// ============================================================================
// SlateIcon — UI 아이콘
// ============================================================================

/// UI 아이콘 (UE5 FSlateIcon)
///
/// 스타일셋에서 아이콘을 참조하는 구조체입니다.
#[derive(Debug, Clone)]
pub struct SlateIcon {
    /// 스타일셋 이름 (예: "EditorStyle")
    pub style_set_name: String,
    /// 아이콘 이름 (예: "Icons.Plus")
    pub icon_name: String,
    /// 소형 아이콘 이름 (선택)
    pub small_icon_name: Option<String>,
    /// 오버라이드 틴트 색상
    pub tint_override: Option<Color>,
}

impl SlateIcon {
    /// 새 아이콘 참조
    pub fn new(style_set: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            style_set_name: style_set.into(),
            icon_name: icon.into(),
            small_icon_name: None,
            tint_override: None,
        }
    }

    /// 소형 아이콘 설정
    pub fn with_small_icon(mut self, name: impl Into<String>) -> Self {
        self.small_icon_name = Some(name.into());
        self
    }

    /// 틴트 오버라이드
    pub fn with_tint(mut self, color: Color) -> Self {
        self.tint_override = Some(color);
        self
    }

    /// 소형 아이콘 이름 (없으면 일반 아이콘 반환)
    pub fn get_small_icon(&self) -> &str {
        self.small_icon_name.as_deref().unwrap_or(&self.icon_name)
    }
}

// ============================================================================
// SlateUpdatableTexture — 업데이트 가능 텍스처
// ============================================================================

/// 업데이트 가능 텍스처 (UE5 FSlateUpdatableTexture)
///
/// 매 프레임 CPU 데이터로 갱신할 수 있는 텍스처입니다.
/// 비디오, 동적 프레뷰 등에 사용됩니다.
#[derive(Debug)]
pub struct SlateUpdatableTexture {
    /// 텍스처 식별자
    pub texture_id: TextureId,
    /// 텍스처 크기
    pub size: (u32, u32),
    /// 포맷
    pub format: SlateTextureFormat,
    /// CPU 측 데이터 버퍼
    data: Vec<u8>,
    /// 갱신 필요 플래그
    dirty: bool,
    /// 생성 세대 (갱신 추적용)
    generation: u64,
}

impl SlateUpdatableTexture {
    /// 새 업데이트 가능 텍스처 생성
    pub fn new(texture_id: TextureId, width: u32, height: u32, format: SlateTextureFormat) -> Self {
        let data_size = (width * height * format.bytes_per_pixel()) as usize;
        Self {
            texture_id,
            size: (width, height),
            format,
            data: vec![0u8; data_size],
            dirty: true,
            generation: 0,
        }
    }

    /// 전체 데이터 갱신
    pub fn update_data(&mut self, data: &[u8]) -> bool {
        let expected = (self.size.0 * self.size.1 * self.format.bytes_per_pixel()) as usize;
        if data.len() != expected {
            return false;
        }
        self.data.copy_from_slice(data);
        self.dirty = true;
        self.generation += 1;
        true
    }

    /// 부분 영역 갱신
    pub fn update_region(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> bool {
        let bpp = self.format.bytes_per_pixel();
        let row_bytes = width * bpp;
        let expected_bytes = (width * height * bpp) as usize;

        if data.len() != expected_bytes {
            return false;
        }
        if x + width > self.size.0 || y + height > self.size.1 {
            return false;
        }

        let stride = self.size.0 * bpp;
        for row in 0..height {
            let src_start = (row * row_bytes) as usize;
            let dst_start = ((y + row) * stride + x * bpp) as usize;
            let len = row_bytes as usize;
            self.data[dst_start..dst_start + len]
                .copy_from_slice(&data[src_start..src_start + len]);
        }

        self.dirty = true;
        self.generation += 1;
        true
    }

    /// CPU 데이터 참조
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 갱신 필요 여부
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// dirty 플래그 클리어 (GPU 업로드 후 호출)
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// 갱신 세대
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// 텍스처 너비
    pub fn width(&self) -> u32 {
        self.size.0
    }

    /// 텍스처 높이
    pub fn height(&self) -> u32 {
        self.size.1
    }
}

// ============================================================================
// NonAtlasedTexture — 비아틀라스 텍스처
// ============================================================================

/// 비아틀라스 텍스처 (UE5에서 큰 이미지 등)
///
/// 아틀라스에 넣지 않고 독립적인 GPU 텍스처로 관리되는 이미지입니다.
/// 뷰포트 렌더 타겟, 큰 배경 이미지 등에 사용됩니다.
#[derive(Debug)]
pub struct NonAtlasedTexture {
    /// 텍스처 식별자
    pub texture_id: TextureId,
    /// 원본 크기
    pub size: (u32, u32),
    /// 포맷
    pub format: SlateTextureFormat,
    /// 이름/경로
    pub name: String,
    /// 렌더 타겟 여부
    pub is_render_target: bool,
}

impl NonAtlasedTexture {
    /// 새 비아틀라스 텍스처
    pub fn new(
        texture_id: TextureId,
        name: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            texture_id,
            size: (width, height),
            format: SlateTextureFormat::Rgba8,
            name: name.into(),
            is_render_target: false,
        }
    }

    /// 렌더 타겟으로 설정
    pub fn as_render_target(mut self) -> Self {
        self.is_render_target = true;
        self
    }

    /// 포맷 설정
    pub fn with_format(mut self, format: SlateTextureFormat) -> Self {
        self.format = format;
        self
    }

    /// UV 좌표 (항상 전체 텍스처)
    pub fn uv(&self) -> [f32; 4] {
        [0.0, 0.0, 1.0, 1.0]
    }

    /// 크기를 Vec2로
    pub fn size_vec2(&self) -> Vec2 {
        Vec2::new(self.size.0 as f32, self.size.1 as f32)
    }
}

// ============================================================================
// SlateResourceHandle — 리소스 핸들
// ============================================================================

/// 슬레이트 리소스 핸들
///
/// 텍스처/브러시 등 렌더 리소스의 간접 참조입니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlateResourceHandle {
    /// 핸들 타입
    pub resource_type: ResourceType,
    /// 리소스 인덱스
    pub index: u32,
    /// 세대 (재사용 감지)
    pub generation: u32,
}

/// 리소스 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// 아틀라스 텍스처 슬롯
    AtlasSlot,
    /// 비아틀라스 텍스처
    StandaloneTexture,
    /// 렌더 타겟
    RenderTarget,
    /// 머티리얼/셰이더
    Material,
}

impl SlateResourceHandle {
    /// 새 핸들
    pub fn new(resource_type: ResourceType, index: u32, generation: u32) -> Self {
        Self {
            resource_type,
            index,
            generation,
        }
    }

    /// 유효하지 않은 핸들
    pub const INVALID: Self = Self {
        resource_type: ResourceType::AtlasSlot,
        index: u32::MAX,
        generation: 0,
    };

    /// 유효한지 확인
    pub fn is_valid(&self) -> bool {
        self.index != u32::MAX
    }
}

impl Default for SlateResourceHandle {
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_format_bpp() {
        assert_eq!(SlateTextureFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(SlateTextureFormat::R8.bytes_per_pixel(), 1);
        assert_eq!(SlateTextureFormat::Rgba16Float.bytes_per_pixel(), 8);
    }

    #[test]
    fn test_slate_icon() {
        let icon = SlateIcon::new("EditorStyle", "Icons.Plus")
            .with_small_icon("Icons.Plus.Small")
            .with_tint(Color::WHITE);
        assert_eq!(icon.icon_name, "Icons.Plus");
        assert_eq!(icon.get_small_icon(), "Icons.Plus.Small");
    }

    #[test]
    fn test_updatable_texture() {
        let mut tex = SlateUpdatableTexture::new(TextureId(1), 4, 4, SlateTextureFormat::Rgba8);
        assert!(tex.is_dirty());
        assert_eq!(tex.data().len(), 64); // 4*4*4

        let data = vec![255u8; 64];
        assert!(tex.update_data(&data));
        assert_eq!(tex.generation(), 1);

        tex.clear_dirty();
        assert!(!tex.is_dirty());
    }

    #[test]
    fn test_updatable_texture_region() {
        let mut tex = SlateUpdatableTexture::new(TextureId(1), 4, 4, SlateTextureFormat::Rgba8);
        // 2x2 region at (1,1)
        let region = vec![128u8; 16]; // 2*2*4
        assert!(tex.update_region(1, 1, 2, 2, &region));
        assert_eq!(tex.generation(), 1);

        // Out of bounds
        assert!(!tex.update_region(3, 3, 2, 2, &region));
    }

    #[test]
    fn test_non_atlased_texture() {
        let tex = NonAtlasedTexture::new(TextureId(1), "background.png", 1920, 1080)
            .as_render_target();
        assert!(tex.is_render_target);
        assert_eq!(tex.uv(), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(tex.size_vec2(), Vec2::new(1920.0, 1080.0));
    }

    #[test]
    fn test_resource_handle() {
        let handle = SlateResourceHandle::new(ResourceType::AtlasSlot, 0, 1);
        assert!(handle.is_valid());

        let invalid = SlateResourceHandle::INVALID;
        assert!(!invalid.is_valid());
    }
}
