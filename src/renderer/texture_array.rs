// SKOPE Engine - Texture Array Manager
// glTF 텍스처들을 유형별 텍스처 배열로 통합 관리
//
// 텍스처 배열 구조:
// - Albedo Array (Rgba8UnormSrgb): 각 머티리얼의 base color 텍스처
// - Normal Array (Rgba8Unorm): 각 머티리얼의 normal map
// - MetallicRoughness Array (Rgba8Unorm): 각 머티리얼의 metallic-roughness 텍스처

use std::collections::HashMap;
use std::path::Path;
use wgpu;

use crate::gltf_loader::{TextureData, Material};

/// 이미지 파일을 TextureData로 로드 (PNG, JPG 등 지원)
pub fn load_image_texture(path: &Path) -> Option<TextureData> {
    let img = image::open(path).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Some(TextureData {
        data: rgba.into_raw(),
        width,
        height,
    })
}

/// 텍스처 배열 정보
#[allow(dead_code)]
pub struct TextureArrayInfo {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub layer_count: u32,
    pub size: u32,  // width == height (정사각형)
    pub format: wgpu::TextureFormat,
}

/// 텍스처 배열 관리자
/// glTF에서 로드한 텍스처들을 유형별로 D2Array 텍스처로 통합
#[allow(dead_code)]
pub struct TextureArrayManager {
    pub albedo_array: TextureArrayInfo,
    pub normal_array: TextureArrayInfo,
    pub metallic_roughness_array: TextureArrayInfo,
    pub sampler: wgpu::Sampler,

    // 원본 텍스처 인덱스 → 배열 레이어 매핑
    albedo_layer_map: HashMap<usize, u32>,
    normal_layer_map: HashMap<usize, u32>,
    mr_layer_map: HashMap<usize, u32>,

    // 경로 기반 텍스처 매핑 (독립 머티리얼용)
    albedo_path_map: HashMap<String, u32>,
    normal_path_map: HashMap<String, u32>,
    mr_path_map: HashMap<String, u32>,
}

impl TextureArrayManager {
    /// glTF 텍스처들로부터 텍스처 배열 생성
    pub fn from_gltf_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &[TextureData],
        materials: &[Material],
    ) -> Self {
        // 각 유형별로 사용되는 텍스처 인덱스 수집
        let mut albedo_indices: Vec<usize> = Vec::new();
        let mut normal_indices: Vec<usize> = Vec::new();
        let mut mr_indices: Vec<usize> = Vec::new();

        for mat in materials {
            if let Some(idx) = mat.base_color_texture {
                if !albedo_indices.contains(&idx) {
                    albedo_indices.push(idx);
                }
            }
            if let Some(idx) = mat.normal_texture {
                if !normal_indices.contains(&idx) {
                    normal_indices.push(idx);
                }
            }
            if let Some(idx) = mat.metallic_roughness_texture {
                if !mr_indices.contains(&idx) {
                    mr_indices.push(idx);
                }
            }
        }

        // 레이어 매핑 생성 (텍스처 인덱스 → 배열 레이어)
        let albedo_layer_map: HashMap<usize, u32> = albedo_indices
            .iter()
            .enumerate()
            .map(|(layer, &tex_idx)| (tex_idx, layer as u32))
            .collect();

        let normal_layer_map: HashMap<usize, u32> = normal_indices
            .iter()
            .enumerate()
            .map(|(layer, &tex_idx)| (tex_idx, layer as u32))
            .collect();

        let mr_layer_map: HashMap<usize, u32> = mr_indices
            .iter()
            .enumerate()
            .map(|(layer, &tex_idx)| (tex_idx, layer as u32))
            .collect();

        // 각 배열의 최대 크기 계산
        let albedo_size = Self::calculate_array_size(textures, &albedo_indices);
        let normal_size = Self::calculate_array_size(textures, &normal_indices);
        let mr_size = Self::calculate_array_size(textures, &mr_indices);

        log::info!(
            "[TextureArray] Creating arrays: Albedo {}x{} ({} layers), Normal {}x{} ({} layers), MR {}x{} ({} layers)",
            albedo_size, albedo_size, albedo_indices.len().max(1),
            normal_size, normal_size, normal_indices.len().max(1),
            mr_size, mr_size, mr_indices.len().max(1)
        );

        // 원본 텍스처 크기 로그 출력
        for &idx in &albedo_indices {
            if let Some(tex) = textures.get(idx) {
                log::info!("[TextureArray] Albedo texture[{}]: {}x{}, data_len={}, expected={}, target_size={}",
                    idx, tex.width, tex.height, tex.data.len(), tex.width * tex.height * 4, albedo_size);
                // 처음 16바이트 덤프 (픽셀 4개)
                if tex.data.len() >= 16 {
                    log::info!("[TextureArray] First 4 pixels: {:?}", &tex.data[0..16]);
                }
            }
        }

        // 텍스처 배열 생성
        let albedo_array = Self::create_texture_array(
            device,
            queue,
            textures,
            &albedo_indices,
            albedo_size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            "Albedo",
            &[255, 255, 255, 255], // 기본 흰색
        );

        let normal_array = Self::create_texture_array(
            device,
            queue,
            textures,
            &normal_indices,
            normal_size,
            wgpu::TextureFormat::Rgba8Unorm,
            "Normal",
            &[128, 128, 255, 255], // 기본 플랫 노멀
        );

        let metallic_roughness_array = Self::create_texture_array(
            device,
            queue,
            textures,
            &mr_indices,
            mr_size,
            wgpu::TextureFormat::Rgba8Unorm,
            "MetallicRoughness",
            &[255, 128, 0, 255], // AO=1, Roughness=0.5, Metallic=0
        );

        // 공유 샘플러 생성 (Anisotropic filtering 16x)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TextureArray Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,  // 비스듬한 각도에서 텍스처 품질 향상
            ..Default::default()
        });

        Self {
            albedo_array,
            normal_array,
            metallic_roughness_array,
            sampler,
            albedo_layer_map,
            normal_layer_map,
            mr_layer_map,
            albedo_path_map: HashMap::new(),
            normal_path_map: HashMap::new(),
            mr_path_map: HashMap::new(),
        }
    }

    /// glTF 텍스처 + 독립 텍스처 파일들로부터 텍스처 배열 생성
    pub fn from_gltf_and_standalone(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gltf_textures: &[TextureData],
        gltf_materials: &[Material],
        standalone_albedo_paths: &[std::path::PathBuf],
    ) -> Self {
        // 독립 텍스처 로드
        let mut standalone_textures: Vec<TextureData> = Vec::new();
        let mut standalone_path_to_idx: HashMap<String, usize> = HashMap::new();

        for path in standalone_albedo_paths {
            if let Some(tex_data) = load_image_texture(path) {
                let idx = gltf_textures.len() + standalone_textures.len();
                let path_str = path.to_string_lossy().to_string();
                log::info!("[TextureArray] Loaded standalone texture: {} (idx={})", path_str, idx);
                standalone_path_to_idx.insert(path_str, idx);
                standalone_textures.push(tex_data);
            } else {
                log::warn!("[TextureArray] Failed to load standalone texture: {:?}", path);
            }
        }

        // glTF + 독립 텍스처 결합
        let mut all_textures: Vec<TextureData> = Vec::new();
        for tex in gltf_textures {
            all_textures.push(TextureData {
                data: tex.data.clone(),
                width: tex.width,
                height: tex.height,
            });
        }
        all_textures.extend(standalone_textures);

        // 각 유형별로 사용되는 텍스처 인덱스 수집
        let mut albedo_indices: Vec<usize> = Vec::new();
        let mut normal_indices: Vec<usize> = Vec::new();
        let mut mr_indices: Vec<usize> = Vec::new();

        // glTF 머티리얼에서
        for mat in gltf_materials {
            if let Some(idx) = mat.base_color_texture {
                if !albedo_indices.contains(&idx) {
                    albedo_indices.push(idx);
                }
            }
            if let Some(idx) = mat.normal_texture {
                if !normal_indices.contains(&idx) {
                    normal_indices.push(idx);
                }
            }
            if let Some(idx) = mat.metallic_roughness_texture {
                if !mr_indices.contains(&idx) {
                    mr_indices.push(idx);
                }
            }
        }

        // 독립 albedo 텍스처 추가
        for &idx in standalone_path_to_idx.values() {
            if !albedo_indices.contains(&idx) {
                albedo_indices.push(idx);
            }
        }

        // 레이어 매핑 생성
        let albedo_layer_map: HashMap<usize, u32> = albedo_indices
            .iter()
            .enumerate()
            .map(|(layer, &tex_idx)| (tex_idx, layer as u32))
            .collect();

        let normal_layer_map: HashMap<usize, u32> = normal_indices
            .iter()
            .enumerate()
            .map(|(layer, &tex_idx)| (tex_idx, layer as u32))
            .collect();

        let mr_layer_map: HashMap<usize, u32> = mr_indices
            .iter()
            .enumerate()
            .map(|(layer, &tex_idx)| (tex_idx, layer as u32))
            .collect();

        // 경로 → 레이어 매핑
        let mut albedo_path_map: HashMap<String, u32> = HashMap::new();
        for (path, &tex_idx) in &standalone_path_to_idx {
            if let Some(&layer) = albedo_layer_map.get(&tex_idx) {
                albedo_path_map.insert(path.clone(), layer);
            }
        }

        // 배열 크기 계산
        let albedo_size = Self::calculate_array_size(&all_textures, &albedo_indices);
        let normal_size = Self::calculate_array_size(&all_textures, &normal_indices);
        let mr_size = Self::calculate_array_size(&all_textures, &mr_indices);

        log::info!(
            "[TextureArray] Creating arrays (with standalone): Albedo {}x{} ({} layers), Normal {}x{} ({} layers), MR {}x{} ({} layers)",
            albedo_size, albedo_size, albedo_indices.len().max(1),
            normal_size, normal_size, normal_indices.len().max(1),
            mr_size, mr_size, mr_indices.len().max(1)
        );

        // 텍스처 배열 생성
        let albedo_array = Self::create_texture_array(
            device,
            queue,
            &all_textures,
            &albedo_indices,
            albedo_size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            "Albedo",
            &[255, 255, 255, 255],
        );

        let normal_array = Self::create_texture_array(
            device,
            queue,
            &all_textures,
            &normal_indices,
            normal_size,
            wgpu::TextureFormat::Rgba8Unorm,
            "Normal",
            &[128, 128, 255, 255],
        );

        let metallic_roughness_array = Self::create_texture_array(
            device,
            queue,
            &all_textures,
            &mr_indices,
            mr_size,
            wgpu::TextureFormat::Rgba8Unorm,
            "MetallicRoughness",
            &[255, 128, 0, 255],
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TextureArray Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        Self {
            albedo_array,
            normal_array,
            metallic_roughness_array,
            sampler,
            albedo_layer_map,
            normal_layer_map,
            mr_layer_map,
            albedo_path_map,
            normal_path_map: HashMap::new(),
            mr_path_map: HashMap::new(),
        }
    }

    /// 원본 텍스처 인덱스로부터 albedo 배열 레이어 인덱스 가져오기
    pub fn get_albedo_layer(&self, tex_idx: usize) -> Option<u32> {
        self.albedo_layer_map.get(&tex_idx).copied()
    }

    /// 경로로부터 albedo 배열 레이어 인덱스 가져오기
    pub fn get_albedo_layer_by_path(&self, path: &str) -> Option<u32> {
        self.albedo_path_map.get(path).copied()
    }

    /// 원본 텍스처 인덱스로부터 normal 배열 레이어 인덱스 가져오기
    pub fn get_normal_layer(&self, tex_idx: usize) -> Option<u32> {
        self.normal_layer_map.get(&tex_idx).copied()
    }

    /// 원본 텍스처 인덱스로부터 metallic-roughness 배열 레이어 인덱스 가져오기
    pub fn get_mr_layer(&self, tex_idx: usize) -> Option<u32> {
        self.mr_layer_map.get(&tex_idx).copied()
    }

    /// 배열 크기 계산 (최대 크기, 2의 거듭제곱으로 올림)
    fn calculate_array_size(textures: &[TextureData], indices: &[usize]) -> u32 {
        if indices.is_empty() {
            return 1; // 최소 1x1
        }

        let max_dim = indices
            .iter()
            .filter_map(|&idx| textures.get(idx))
            .map(|t| t.width.max(t.height))
            .max()
            .unwrap_or(1);

        // 2의 거듭제곱으로 올림, 최대 2048
        Self::next_power_of_two(max_dim).min(2048)
    }

    /// 다음 2의 거듭제곱 계산
    fn next_power_of_two(n: u32) -> u32 {
        if n == 0 {
            return 1;
        }
        let mut v = n - 1;
        v |= v >> 1;
        v |= v >> 2;
        v |= v >> 4;
        v |= v >> 8;
        v |= v >> 16;
        v + 1
    }

    /// 밉맵 레벨 수 계산
    fn calculate_mip_levels(size: u32) -> u32 {
        (size as f32).log2().floor() as u32 + 1
    }

    /// 텍스처 배열 생성 (밉맵 포함)
    fn create_texture_array(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures: &[TextureData],
        indices: &[usize],
        size: u32,
        format: wgpu::TextureFormat,
        label: &str,
        fallback_color: &[u8; 4],
    ) -> TextureArrayInfo {
        let layer_count = indices.len().max(1) as u32;
        let mip_level_count = Self::calculate_mip_levels(size);

        log::info!(
            "[TextureArray] Creating {} array: {}x{}, {} layers, {} mip levels",
            label, size, size, layer_count, mip_level_count
        );

        // 텍스처 생성 (밉맵 지원)
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{} Texture Array", label)),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: layer_count,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 각 레이어에 텍스처 데이터 업로드 (모든 밉 레벨)
        if indices.is_empty() {
            // 기본 텍스처 (1x1)
            Self::write_fallback_layer_with_mips(queue, &texture, 0, size, mip_level_count, fallback_color);
        } else {
            for (layer, &tex_idx) in indices.iter().enumerate() {
                if let Some(tex_data) = textures.get(tex_idx) {
                    Self::write_texture_layer_with_mips(
                        queue,
                        &texture,
                        layer as u32,
                        tex_data,
                        size,
                        mip_level_count,
                        fallback_color,
                    );
                } else {
                    Self::write_fallback_layer_with_mips(queue, &texture, layer as u32, size, mip_level_count, fallback_color);
                }
            }
        }

        // D2Array 뷰 생성
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("{} Array View", label)),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        TextureArrayInfo {
            texture,
            view,
            layer_count,
            size,
            format,
        }
    }

    /// 텍스처 데이터를 특정 레이어에 업로드 (리사이징 포함)
    fn write_texture_layer(
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        layer: u32,
        tex_data: &TextureData,
        target_size: u32,
        fallback_color: &[u8; 4],
    ) {
        let resized_data = if tex_data.width == target_size && tex_data.height == target_size {
            // 크기가 같으면 그대로 사용
            tex_data.data.clone()
        } else {
            // 리사이징 필요
            Self::resize_texture(&tex_data.data, tex_data.width, tex_data.height, target_size, fallback_color)
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &resized_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(target_size * 4),
                rows_per_image: None,  // 중요: wgpu 자동 계산 (Some(target_size)는 줄무늬 발생)
            },
            wgpu::Extent3d {
                width: target_size,
                height: target_size,
                depth_or_array_layers: 1,
            },
        );
    }

    /// 기본 색상으로 레이어 채우기
    fn write_fallback_layer(
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        layer: u32,
        size: u32,
        color: &[u8; 4],
    ) {
        let pixel_count = (size * size) as usize;
        let mut data = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            data.extend_from_slice(color);
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: None,  // wgpu 자동 계산
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }

    /// CPU 기반 텍스처 리사이징 (Bilinear 보간)
    fn resize_texture(
        data: &[u8],
        src_width: u32,
        src_height: u32,
        target_size: u32,
        fallback_color: &[u8; 4],
    ) -> Vec<u8> {
        let target_pixels = (target_size * target_size) as usize;
        let mut result = vec![0u8; target_pixels * 4];

        if data.len() < (src_width * src_height * 4) as usize {
            // 데이터가 부족하면 fallback 색상으로 채움
            for i in 0..target_pixels {
                result[i * 4..i * 4 + 4].copy_from_slice(fallback_color);
            }
            return result;
        }

        let scale_x = src_width as f32 / target_size as f32;
        let scale_y = src_height as f32 / target_size as f32;

        for ty in 0..target_size {
            for tx in 0..target_size {
                let sx = tx as f32 * scale_x;
                let sy = ty as f32 * scale_y;

                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let x1 = (x0 + 1).min(src_width - 1);
                let y1 = (y0 + 1).min(src_height - 1);

                let fx = sx - x0 as f32;
                let fy = sy - y0 as f32;

                let get_pixel = |x: u32, y: u32| -> [u8; 4] {
                    let idx = ((y * src_width + x) * 4) as usize;
                    if idx + 3 < data.len() {
                        [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
                    } else {
                        *fallback_color
                    }
                };

                let p00 = get_pixel(x0, y0);
                let p10 = get_pixel(x1, y0);
                let p01 = get_pixel(x0, y1);
                let p11 = get_pixel(x1, y1);

                let target_idx = ((ty * target_size + tx) * 4) as usize;
                for c in 0..4 {
                    let v00 = p00[c] as f32;
                    let v10 = p10[c] as f32;
                    let v01 = p01[c] as f32;
                    let v11 = p11[c] as f32;

                    let v = v00 * (1.0 - fx) * (1.0 - fy)
                        + v10 * fx * (1.0 - fy)
                        + v01 * (1.0 - fx) * fy
                        + v11 * fx * fy;

                    result[target_idx + c] = v.round().clamp(0.0, 255.0) as u8;
                }
            }
        }

        result
    }

    /// 텍스처 데이터를 특정 레이어에 업로드 (모든 밉 레벨 생성)
    fn write_texture_layer_with_mips(
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        layer: u32,
        tex_data: &TextureData,
        target_size: u32,
        mip_level_count: u32,
        fallback_color: &[u8; 4],
    ) {
        // 기본 레벨 (mip 0) 데이터 준비
        let base_data = if tex_data.width == target_size && tex_data.height == target_size {
            tex_data.data.clone()
        } else {
            Self::resize_texture(&tex_data.data, tex_data.width, tex_data.height, target_size, fallback_color)
        };

        // 모든 밉 레벨 생성 및 업로드
        let mut current_data = base_data;
        let mut current_size = target_size;

        for mip_level in 0..mip_level_count {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &current_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(current_size * 4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: current_size,
                    height: current_size,
                    depth_or_array_layers: 1,
                },
            );

            // 다음 밉 레벨 준비 (2x2 박스 필터로 다운샘플)
            if mip_level + 1 < mip_level_count {
                let next_size = (current_size / 2).max(1);
                current_data = Self::downsample_2x2(&current_data, current_size, next_size);
                current_size = next_size;
            }
        }
    }

    /// 기본 색상으로 레이어 채우기 (모든 밉 레벨)
    fn write_fallback_layer_with_mips(
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        layer: u32,
        base_size: u32,
        mip_level_count: u32,
        color: &[u8; 4],
    ) {
        let mut current_size = base_size;

        for mip_level in 0..mip_level_count {
            let pixel_count = (current_size * current_size) as usize;
            let mut data = Vec::with_capacity(pixel_count * 4);
            for _ in 0..pixel_count {
                data.extend_from_slice(color);
            }

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(current_size * 4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: current_size,
                    height: current_size,
                    depth_or_array_layers: 1,
                },
            );

            current_size = (current_size / 2).max(1);
        }
    }

    /// 2x2 박스 필터로 다운샘플링 (밉맵 생성용)
    fn downsample_2x2(data: &[u8], src_size: u32, dst_size: u32) -> Vec<u8> {
        let dst_pixels = (dst_size * dst_size) as usize;
        let mut result = vec![0u8; dst_pixels * 4];

        for dy in 0..dst_size {
            for dx in 0..dst_size {
                let sx = dx * 2;
                let sy = dy * 2;

                // 2x2 블록의 4개 픽셀 평균
                let mut sum = [0u32; 4];
                let mut count = 0u32;

                for oy in 0..2 {
                    for ox in 0..2 {
                        let px = (sx + ox).min(src_size - 1);
                        let py = (sy + oy).min(src_size - 1);
                        let idx = ((py * src_size + px) * 4) as usize;

                        if idx + 3 < data.len() {
                            sum[0] += data[idx] as u32;
                            sum[1] += data[idx + 1] as u32;
                            sum[2] += data[idx + 2] as u32;
                            sum[3] += data[idx + 3] as u32;
                            count += 1;
                        }
                    }
                }

                let dst_idx = ((dy * dst_size + dx) * 4) as usize;
                if count > 0 {
                    result[dst_idx] = (sum[0] / count) as u8;
                    result[dst_idx + 1] = (sum[1] / count) as u8;
                    result[dst_idx + 2] = (sum[2] / count) as u8;
                    result[dst_idx + 3] = (sum[3] / count) as u8;
                }
            }
        }

        result
    }
}
