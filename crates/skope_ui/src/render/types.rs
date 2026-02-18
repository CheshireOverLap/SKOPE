//! Render types for RSlate UI

/// UI 정점
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SlateVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl SlateVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // uv
        2 => Float32x4,  // color
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SlateVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// SDF RoundedBox 정점
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SlateRoundedVertex {
    pub position: [f32; 2],       // 화면 좌표
    pub local_pos: [f32; 2],      // 엘리먼트 내 로컬 좌표 (0,0 ~ size)
    pub color: [f32; 4],          // fill color
    pub rect_size: [f32; 2],      // 엘리먼트 크기 (px) — SDF 계산용
    pub corner_radii: [f32; 4],   // TL, TR, BR, BL
    pub outline_color: [f32; 4],  // 아웃라인 색상
    pub outline_width: f32,       // 아웃라인 두께
    pub _pad: f32,                // 16바이트 정렬
}

impl SlateRoundedVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // local_pos
        2 => Float32x4,  // color
        3 => Float32x2,  // rect_size
        4 => Float32x4,  // corner_radii
        5 => Float32x4,  // outline_color
        6 => Float32x2,  // outline_width + pad
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SlateRoundedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// UI 유니폼
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SlateUniforms {
    pub screen_size: [f32; 2],
    pub _padding: [f32; 2],
}

/// 텍스처 정보
pub struct SlateTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub size: (u32, u32),
}

/// 드로우 콜 정보
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct DrawCall {
    pub index_start: u32,
    pub index_end: u32,
    pub texture_id: Option<u32>,
}

/// 텍스트 드로우 콜
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct TextDrawCall {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: [f32; 4],
}

// ============================================================================
// Instanced Rendering — 인스턴스 버텍스 데이터 (P2#21)
// ============================================================================

/// 인스턴스 데이터 (per-instance vertex 속성)
///
/// 동일 지오메트리를 다른 위치/크기/색상으로 반복 렌더링할 때 사용합니다.
/// 2D UI에 최적화된 구조로, 3x2 아핀 변환 + 컬러 틴트 + UV 스케일/오프셋.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SlateInstanceData {
    /// 2D 아핀 변환 (2x3 행렬, row-major)
    /// [m00, m01, m02, m10, m11, m12]
    /// position' = [m00*x + m01*y + m02, m10*x + m11*y + m12]
    pub transform: [f32; 6],
    /// 컬러 틴트 (RGBA, 1.0 = no tint)
    pub color_tint: [f32; 4],
    /// UV 스케일 및 오프셋 [u_scale, v_scale, u_offset, v_offset]
    pub uv_transform: [f32; 4],
    /// 불투명도 (0.0 ~ 1.0)
    pub opacity: f32,
    /// 패딩 (16바이트 정렬)
    pub _padding: [f32; 1],
}

#[allow(dead_code)]
impl SlateInstanceData {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        // 슬롯 3부터 시작 (슬롯 0-2는 SlateVertex가 사용)
        3 => Float32x3,  // transform part 1 (m00, m01, m02)
        4 => Float32x3,  // transform part 2 (m10, m11, m12)
        5 => Float32x4,  // color_tint
        6 => Float32x4,  // uv_transform
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SlateInstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    /// 단위 변환 (위치 (0,0), 크기 (1,1), 회전 없음)
    pub fn identity() -> Self {
        Self {
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            color_tint: [1.0, 1.0, 1.0, 1.0],
            uv_transform: [1.0, 1.0, 0.0, 0.0],
            opacity: 1.0,
            _padding: [0.0],
        }
    }

    /// 위치 + 크기 변환
    pub fn translate_scale(x: f32, y: f32, scale_x: f32, scale_y: f32) -> Self {
        Self {
            transform: [scale_x, 0.0, x, 0.0, scale_y, y],
            color_tint: [1.0, 1.0, 1.0, 1.0],
            uv_transform: [1.0, 1.0, 0.0, 0.0],
            opacity: 1.0,
            _padding: [0.0],
        }
    }

    /// 위치만 설정
    pub fn translate(x: f32, y: f32) -> Self {
        Self::translate_scale(x, y, 1.0, 1.0)
    }

    /// 컬러 틴트 설정
    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color_tint = [r, g, b, a];
        self
    }

    /// UV 변환 설정
    pub fn with_uv(mut self, u_scale: f32, v_scale: f32, u_offset: f32, v_offset: f32) -> Self {
        self.uv_transform = [u_scale, v_scale, u_offset, v_offset];
        self
    }

    /// 불투명도 설정
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

/// 인스턴스 배치 — 동일 지오메트리의 인스턴스 모음
#[derive(Debug)]
#[allow(dead_code)]
pub struct InstanceBatch {
    /// 기본 지오메트리의 인덱스 범위
    pub index_start: u32,
    pub index_count: u32,
    /// 인스턴스 데이터
    pub instances: Vec<SlateInstanceData>,
    /// 텍스처 ID (옵션)
    pub texture_id: Option<u32>,
}

#[allow(dead_code)]
impl InstanceBatch {
    pub fn new(index_start: u32, index_count: u32) -> Self {
        Self {
            index_start,
            index_count,
            instances: Vec::new(),
            texture_id: None,
        }
    }

    /// 인스턴스 추가
    pub fn add_instance(&mut self, instance: SlateInstanceData) {
        self.instances.push(instance);
    }

    /// 인스턴스 수
    pub fn instance_count(&self) -> u32 {
        self.instances.len() as u32
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// 텍스처 설정
    pub fn with_texture(mut self, texture_id: u32) -> Self {
        self.texture_id = Some(texture_id);
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slate_instance_data_identity() {
        let inst = SlateInstanceData::identity();
        assert_eq!(inst.transform[0], 1.0); // m00
        assert_eq!(inst.transform[4], 1.0); // m11
        assert_eq!(inst.opacity, 1.0);
    }

    #[test]
    fn test_slate_instance_data_translate() {
        let inst = SlateInstanceData::translate(100.0, 200.0);
        assert_eq!(inst.transform[2], 100.0); // tx
        assert_eq!(inst.transform[5], 200.0); // ty
        assert_eq!(inst.transform[0], 1.0); // sx
    }

    #[test]
    fn test_slate_instance_data_translate_scale() {
        let inst = SlateInstanceData::translate_scale(10.0, 20.0, 2.0, 3.0);
        assert_eq!(inst.transform[0], 2.0); // sx
        assert_eq!(inst.transform[4], 3.0); // sy
        assert_eq!(inst.transform[2], 10.0); // tx
        assert_eq!(inst.transform[5], 20.0); // ty
    }

    #[test]
    fn test_slate_instance_data_builder() {
        let inst = SlateInstanceData::identity()
            .with_color(1.0, 0.0, 0.0, 1.0)
            .with_opacity(0.5)
            .with_uv(0.5, 0.5, 0.25, 0.25);

        assert_eq!(inst.color_tint, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(inst.opacity, 0.5);
        assert_eq!(inst.uv_transform, [0.5, 0.5, 0.25, 0.25]);
    }

    #[test]
    fn test_instance_batch() {
        let mut batch = InstanceBatch::new(0, 6);
        assert!(batch.is_empty());

        batch.add_instance(SlateInstanceData::translate(0.0, 0.0));
        batch.add_instance(SlateInstanceData::translate(100.0, 0.0));
        batch.add_instance(SlateInstanceData::translate(200.0, 0.0));

        assert_eq!(batch.instance_count(), 3);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_slate_instance_data_size() {
        // 6 + 4 + 4 + 1 + 1 = 16 floats = 64 bytes
        assert_eq!(std::mem::size_of::<SlateInstanceData>(), 64);
    }
}
