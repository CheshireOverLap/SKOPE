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
pub(crate) struct DrawCall {
    pub index_start: u32,
    pub index_end: u32,
    pub texture_id: Option<u32>,
}

/// 텍스트 드로우 콜
#[derive(Debug)]
pub(crate) struct TextDrawCall {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: [f32; 4],
}
