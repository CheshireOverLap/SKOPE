//! Texture Atlas — UI 이미지를 단일 텍스처에 패킹 (Shelf-packing)
//!
//! 여러 UI 이미지를 하나의 GPU 텍스처에 합쳐서
//! 드로우콜 배칭 효율을 높입니다.

use std::collections::HashMap;

/// 아틀라스 슬롯 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasSlotId(pub u32);

/// 아틀라스 내 할당된 영역
#[derive(Debug, Clone)]
pub struct AtlasSlot {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// UV 좌표 (u_min, v_min, u_max, v_max) — 0.0~1.0
    pub uv: [f32; 4],
}

/// Shelf-packing 기반 텍스처 아틀라스
pub struct SlateTextureAtlas {
    /// GPU 텍스처
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    /// 아틀라스 크기 (정사각형)
    size: u32,
    /// 할당된 슬롯들
    slots: HashMap<AtlasSlotId, AtlasSlot>,
    /// 이름 → 슬롯 ID 매핑
    name_to_id: HashMap<String, AtlasSlotId>,
    /// 다음 슬롯 ID
    next_id: u32,
    /// Shelf 상태: (현재 x, 현재 y, 현재 선반 높이)
    shelf_cursor: (u32, u32, u32),
    /// dirty 플래그 (CPU → GPU 업로드 필요)
    dirty: bool,
}

impl SlateTextureAtlas {
    pub const DEFAULT_SIZE: u32 = 2048;

    /// 새 아틀라스 생성
    pub fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        size: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Slate UI Atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Slate UI Atlas Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            texture_view,
            bind_group,
            size,
            slots: HashMap::new(),
            name_to_id: HashMap::new(),
            next_id: 0,
            shelf_cursor: (0, 0, 0),
            dirty: false,
        }
    }

    /// 텍스처 추가 (RGBA 데이터, 즉시 GPU 업로드)
    ///
    /// Shelf-packing: 왼쪽→오른쪽으로 채우고, 행이 차면 다음 행으로.
    pub fn add_texture(
        &mut self,
        queue: &wgpu::Queue,
        name: impl Into<String>,
        width: u32,
        height: u32,
        rgba_data: &[u8],
    ) -> Option<AtlasSlotId> {
        let name = name.into();

        // 이미 등록된 경우
        if let Some(&id) = self.name_to_id.get(&name) {
            return Some(id);
        }

        let padding = 1u32;
        let padded_w = width + padding * 2;
        let padded_h = height + padding * 2;

        // 아틀라스에 맞는지 확인
        if padded_w > self.size || padded_h > self.size {
            return None;
        }

        // 현재 행에 맞는지
        if self.shelf_cursor.0 + padded_w > self.size {
            // 다음 행으로
            self.shelf_cursor.0 = 0;
            self.shelf_cursor.1 += self.shelf_cursor.2;
            self.shelf_cursor.2 = 0;
        }

        // 높이 초과 확인
        if self.shelf_cursor.1 + padded_h > self.size {
            return None; // 아틀라스 꽉 참
        }

        let x = self.shelf_cursor.0 + padding;
        let y = self.shelf_cursor.1 + padding;

        // 커서 전진
        self.shelf_cursor.0 += padded_w;
        self.shelf_cursor.2 = self.shelf_cursor.2.max(padded_h);

        // GPU 업로드
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_f = self.size as f32;
        let slot = AtlasSlot {
            x,
            y,
            width,
            height,
            uv: [
                x as f32 / atlas_f,
                y as f32 / atlas_f,
                (x + width) as f32 / atlas_f,
                (y + height) as f32 / atlas_f,
            ],
        };

        let id = AtlasSlotId(self.next_id);
        self.next_id += 1;
        self.slots.insert(id, slot);
        self.name_to_id.insert(name, id);

        Some(id)
    }

    /// 이름으로 슬롯 조회
    pub fn get_slot_by_name(&self, name: &str) -> Option<&AtlasSlot> {
        self.name_to_id.get(name).and_then(|id| self.slots.get(id))
    }

    /// ID로 슬롯 조회
    pub fn get_slot(&self, id: AtlasSlotId) -> Option<&AtlasSlot> {
        self.slots.get(&id)
    }

    /// 바인드 그룹 참조
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// 텍스처 뷰 참조
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    /// 등록된 슬롯 수
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// 이름이 등록되어 있는지
    pub fn contains(&self, name: &str) -> bool {
        self.name_to_id.contains_key(name)
    }

    /// 아틀라스 크기
    pub fn size(&self) -> u32 {
        self.size
    }
}
