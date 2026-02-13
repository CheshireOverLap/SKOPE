//! Sampler Cache
//!
//! WrapMode/FilterMode 조합별 wgpu::Sampler 캐싱

use std::collections::HashMap;
use crate::gltf_loader::intermediate::{SamplerDesc, WrapMode, FilterMode};

/// 샘플러 캐시 키
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerKey {
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: FilterMode,
}

impl From<&SamplerDesc> for SamplerKey {
    fn from(desc: &SamplerDesc) -> Self {
        Self {
            wrap_u: desc.wrap_u,
            wrap_v: desc.wrap_v,
            mag_filter: desc.mag_filter,
            min_filter: desc.min_filter,
            mipmap_filter: desc.mipmap_filter,
        }
    }
}

/// 샘플러 캐시
/// WrapMode/FilterMode 조합별로 wgpu::Sampler를 캐싱
pub struct SamplerCache {
    samplers: Vec<wgpu::Sampler>,
    desc_to_index: HashMap<SamplerKey, usize>,
}

impl SamplerCache {
    pub fn new() -> Self {
        Self {
            samplers: Vec::new(),
            desc_to_index: HashMap::new(),
        }
    }

    /// 주어진 SamplerDesc에 대응하는 샘플러 인덱스 반환 (없으면 생성)
    pub fn get_or_create(&mut self, device: &wgpu::Device, desc: &SamplerDesc) -> usize {
        let key = SamplerKey::from(desc);

        if let Some(&index) = self.desc_to_index.get(&key) {
            return index;
        }

        let wrap_mode = |w: WrapMode| -> wgpu::AddressMode {
            match w {
                WrapMode::Repeat => wgpu::AddressMode::Repeat,
                WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            }
        };

        let filter = |f: FilterMode| -> wgpu::FilterMode {
            match f {
                FilterMode::Nearest => wgpu::FilterMode::Nearest,
                FilterMode::Linear => wgpu::FilterMode::Linear,
            }
        };

        let mipmap_filter = |f: FilterMode| -> wgpu::MipmapFilterMode {
            match f {
                FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
                FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
            }
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("Cached Sampler {}", self.samplers.len())),
            address_mode_u: wrap_mode(desc.wrap_u),
            address_mode_v: wrap_mode(desc.wrap_v),
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter(desc.mag_filter),
            min_filter: filter(desc.min_filter),
            mipmap_filter: mipmap_filter(desc.mipmap_filter),
            anisotropy_clamp: 16,
            ..Default::default()
        });

        let index = self.samplers.len();
        self.samplers.push(sampler);
        self.desc_to_index.insert(key, index);

        log::debug!("[SamplerCache] Created sampler {} for {:?}", index, key);
        index
    }

    /// 인덱스로 샘플러 조회
    pub fn get(&self, index: usize) -> Option<&wgpu::Sampler> {
        self.samplers.get(index)
    }

    /// 캐시된 샘플러 수
    pub fn count(&self) -> usize {
        self.samplers.len()
    }
}
