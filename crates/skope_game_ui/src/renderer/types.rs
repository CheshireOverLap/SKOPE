//! UI Renderer Types
//!
//! Data types for the UI rendering system

/// UI 텍스처
pub struct UiTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub size: (u32, u32),
}

/// UI 정점
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl UiVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // uv
        2 => Float32x4,  // color
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// UI 유니폼
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiUniforms {
    pub screen_size: [f32; 2],
    pub _padding: [f32; 2],
}

/// 드로우 콜 정보
pub(crate) struct DrawCall {
    pub texture: Option<String>,
    pub index_start: u32,
    pub index_end: u32,
    /// 클리핑 영역 (ScrollView용)
    pub scissor_rect: Option<ScissorRect>,
}

/// 시저 렉트 (클리핑 영역)
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// 텍스트 드로우 콜 정보
pub(crate) struct TextDrawCall {
    pub content: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: [f32; 4],
    pub max_width: f32,
    pub max_height: f32,
    /// 클리핑 영역 (ScrollView용)
    pub scissor_rect: Option<ScissorRect>,
}
