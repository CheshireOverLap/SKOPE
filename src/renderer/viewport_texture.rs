//! Viewport Texture
//!
//! 오프스크린 렌더 타겟으로 씬을 렌더링 (skope_ui로 재구현 예정)

/// 뷰포트 렌더 타겟
#[allow(dead_code)]
pub struct ViewportTexture {
    /// 렌더 타겟 텍스처
    pub texture: wgpu::Texture,
    /// 텍스처 뷰 (렌더 패스용)
    pub view: wgpu::TextureView,
    /// Depth 텍스처
    pub depth_texture: wgpu::Texture,
    /// Depth 뷰
    pub depth_view: wgpu::TextureView,
    /// UI 텍스처 ID (skope_ui에서 표시용)
    pub ui_texture_id: Option<u64>,
    /// 현재 크기
    pub size: (u32, u32),
    /// 텍스처 포맷
    format: wgpu::TextureFormat,
}

impl ViewportTexture {
    /// 새 뷰포트 텍스처 생성
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
        let (texture, view) = Self::create_color_texture(device, format, size);
        let (depth_texture, depth_view) = Self::create_depth_texture(device, size);

        log::info!(
            "[ViewportTexture] Created {}x{} (format: {:?})",
            size.0, size.1, format
        );

        Self {
            texture,
            view,
            depth_texture,
            depth_view,
            ui_texture_id: None,
            size,
            format,
        }
    }

    /// 크기 변경
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        new_size: (u32, u32),
    ) {
        if self.size == new_size || new_size.0 == 0 || new_size.1 == 0 {
            return;
        }

        let (texture, view) = Self::create_color_texture(device, self.format, new_size);
        let (depth_texture, depth_view) = Self::create_depth_texture(device, new_size);

        self.texture = texture;
        self.view = view;
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.size = new_size;

        log::debug!("[ViewportTexture] Resized to {}x{}", new_size.0, new_size.1);
    }

    /// 컬러 텍스처 생성
    fn create_color_texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Color Texture"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Depth 텍스처 생성
    fn create_depth_texture(
        device: &wgpu::Device,
        size: (u32, u32),
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Depth Texture"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    pub fn render_target(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn depth_target(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    #[allow(dead_code)]
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    #[allow(dead_code)]
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    #[allow(dead_code)]
    pub fn update_ui_id(&mut self, id: u64) {
        self.ui_texture_id = Some(id);
    }

    #[allow(dead_code)]
    pub fn ui_texture_id(&self) -> Option<u64> {
        self.ui_texture_id
    }

}
