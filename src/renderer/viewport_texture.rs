//! Viewport Texture
//!
//! 오프스크린 렌더 타겟으로 씬을 렌더링하고 ImGui에 표시

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
    /// ImGui 텍스처 ID (dear-imgui-rs에서 표시용)
    pub imgui_texture_id: Option<u64>,
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
            imgui_texture_id: None,
            size,
            format,
        }
    }

    /// ImGui 텍스처 등록 (dear-imgui-wgpu)
    pub fn register_imgui_texture(
        &mut self,
        imgui_renderer: &mut dear_imgui_wgpu::WgpuRenderer,
    ) {
        let id = imgui_renderer.register_external_texture(&self.texture, &self.view);
        self.imgui_texture_id = Some(id);
        log::info!("[ViewportTexture] ImGui texture registered (id={})", id);
    }

    /// ImGui 텍스처 업데이트 (리사이즈 후 - 새 텍스처로 다시 등록)
    pub fn update_imgui_texture(
        &mut self,
        imgui_renderer: &mut dear_imgui_wgpu::WgpuRenderer,
    ) {
        // 텍스처가 새로 생성되었으므로 다시 등록
        let new_id = imgui_renderer.register_external_texture(&self.texture, &self.view);
        self.imgui_texture_id = Some(new_id);
        log::info!("[ViewportTexture] ImGui texture re-registered (new_id={})", new_id);
    }

    /// ImGui 텍스처 ID 반환
    pub fn imgui_texture_id(&self) -> Option<u64> {
        self.imgui_texture_id
    }

    /// 크기 변경
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        new_size: (u32, u32),
    ) {
        // 크기가 같으면 무시
        if self.size == new_size || new_size.0 == 0 || new_size.1 == 0 {
            return;
        }

        // 새 텍스처 생성
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

    /// 렌더 타겟 뷰 가져오기
    pub fn render_target(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Depth 뷰 가져오기
    pub fn depth_target(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    /// 현재 크기 반환
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// 텍스처 뷰 반환 (ImGui 재등록용)
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// ImGui 텍스처 ID 업데이트
    pub fn update_imgui_id(&mut self, id: u64) {
        self.imgui_texture_id = Some(id);
    }

    /// ImGui 텍스처 ID 반환 (unwrap 버전)
    pub fn imgui_texture_id_unwrap(&self) -> u64 {
        self.imgui_texture_id.unwrap_or(0)
    }
}
