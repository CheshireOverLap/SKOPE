// SKOPE Engine - Outline Buffers
// 아웃라인 렌더링용 버퍼 및 텍스처

/// 아웃라인 전용 버퍼들
pub struct OutlineBuffers {
    /// 스무딩된 노멀 (Inverted Hull용)
    pub smooth_normal_buffer: wgpu::Buffer,

    /// 버텍스별 두께 스케일
    pub thickness_buffer: wgpu::Buffer,

    /// Edge Detection 결과 텍스처 (R8)
    pub edge_mask_texture: wgpu::Texture,
    pub edge_mask_view: wgpu::TextureView,

    /// Hull 결과 텍스처 (RGBA8)
    pub hull_texture: wgpu::Texture,
    pub hull_view: wgpu::TextureView,

    /// 최종 아웃라인 텍스처
    pub outline_texture: wgpu::Texture,
    pub outline_view: wgpu::TextureView,

    /// 버텍스 개수
    pub vertex_count: u32,

    /// 스크린 사이즈
    pub screen_size: (u32, u32),
}

impl OutlineBuffers {
    pub fn new(
        device: &wgpu::Device,
        vertex_count: u32,
        screen_size: (u32, u32),
    ) -> Self {
        // Smooth normal buffer (vec4 per vertex for alignment)
        let smooth_normal_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Outline Smooth Normal Buffer"),
            size: (vertex_count as u64) * 16, // vec4<f32>
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Thickness buffer (f32 per vertex)
        let thickness_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Outline Thickness Buffer"),
            size: (vertex_count as u64) * 4,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Edge mask texture
        let edge_mask_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Edge Mask Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let edge_mask_view = edge_mask_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Hull texture
        let hull_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Hull Outline Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hull_view = hull_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Final outline texture
        let outline_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Final Outline Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let outline_view = outline_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            smooth_normal_buffer,
            thickness_buffer,
            edge_mask_texture,
            edge_mask_view,
            hull_texture,
            hull_view,
            outline_texture,
            outline_view,
            vertex_count,
            screen_size,
        }
    }

    /// 스크린 사이즈 변경 시 텍스처 재생성
    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }

        self.screen_size = new_size;

        // Edge mask
        self.edge_mask_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Edge Mask Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.edge_mask_view = self
            .edge_mask_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Hull
        self.hull_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Hull Outline Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.hull_view = self
            .hull_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Final outline
        self.outline_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Final Outline Texture"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.outline_view = self
            .outline_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    /// 스무딩된 노멀 업로드
    pub fn upload_smooth_normals(&self, queue: &wgpu::Queue, normals: &[[f32; 4]]) {
        queue.write_buffer(
            &self.smooth_normal_buffer,
            0,
            bytemuck::cast_slice(normals),
        );
    }

    /// 두께 스케일 업로드
    pub fn upload_thickness_scales(&self, queue: &wgpu::Queue, scales: &[f32]) {
        queue.write_buffer(&self.thickness_buffer, 0, bytemuck::cast_slice(scales));
    }
}

/// 버텍스별 데이터를 vec4로 패딩
pub fn pad_normals_to_vec4(normals: &[[f32; 3]], scales: &[f32]) -> Vec<[f32; 4]> {
    normals
        .iter()
        .zip(scales.iter().chain(std::iter::repeat(&1.0)))
        .map(|(n, &s)| [n[0], n[1], n[2], s])
        .collect()
}
