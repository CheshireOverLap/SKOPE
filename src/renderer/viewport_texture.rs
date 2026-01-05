//! Viewport Texture
//!
//! 오프스크린 렌더 타겟으로 씬을 렌더링하고 egui에 표시

/// 뷰포트 렌더 타겟
pub struct ViewportTexture {
    /// 렌더 타겟 텍스처
    pub texture: wgpu::Texture,
    /// 텍스처 뷰 (렌더 패스용)
    pub view: wgpu::TextureView,
    /// Depth 텍스처
    pub depth_texture: wgpu::Texture,
    /// Depth 뷰
    pub depth_view: wgpu::TextureView,
    /// egui 텍스처 ID (UI에서 표시용)
    pub egui_texture_id: egui::TextureId,
    /// 현재 크기
    pub size: (u32, u32),
    /// 텍스처 포맷
    format: wgpu::TextureFormat,
}

impl ViewportTexture {
    /// 새 뷰포트 텍스처 생성
    pub fn new(
        device: &wgpu::Device,
        egui_renderer: &mut egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
        let (texture, view) = Self::create_color_texture(device, format, size);
        let (depth_texture, depth_view) = Self::create_depth_texture(device, size);

        // egui에 텍스처 등록
        let egui_texture_id = egui_renderer.register_native_texture(
            device,
            &view,
            wgpu::FilterMode::Linear,
        );

        log::info!(
            "[ViewportTexture] Created {}x{} (format: {:?})",
            size.0, size.1, format
        );

        Self {
            texture,
            view,
            depth_texture,
            depth_view,
            egui_texture_id,
            size,
            format,
        }
    }

    /// 크기 변경
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut egui_wgpu::Renderer,
        new_size: (u32, u32),
    ) {
        // 크기가 같으면 무시
        if self.size == new_size || new_size.0 == 0 || new_size.1 == 0 {
            return;
        }

        // 새 텍스처 생성
        let (texture, view) = Self::create_color_texture(device, self.format, new_size);
        let (depth_texture, depth_view) = Self::create_depth_texture(device, new_size);

        // egui 텍스처 업데이트
        egui_renderer.update_egui_texture_from_wgpu_texture(
            device,
            &view,
            wgpu::FilterMode::Linear,
            self.egui_texture_id,
        );

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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// 렌더 타겟 뷰 가져오기
    pub fn render_target(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Depth 뷰 가져오기
    pub fn depth_target(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    /// egui 텍스처 ID 가져오기
    pub fn texture_id(&self) -> egui::TextureId {
        self.egui_texture_id
    }
}
