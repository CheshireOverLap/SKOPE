// SKOPE Engine - Effect Asset Loader
// Phase E1: RON/PNG/EXR Loading

use super::data::*;
use std::path::Path;

/// Effect 로더
pub struct EffectLoader;

impl EffectLoader {
    /// Flipbook RON 파일 로드
    pub fn load_flipbook_meta(path: &Path) -> Result<FlipbookMeta, EffectLoadError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| EffectLoadError::IoError(e.to_string()))?;

        ron::from_str(&content)
            .map_err(|e| EffectLoadError::ParseError(format!("RON parse error: {}", e)))
    }

    /// VAT RON 파일 로드
    pub fn load_vat_meta(path: &Path) -> Result<VatMeta, EffectLoadError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| EffectLoadError::IoError(e.to_string()))?;

        ron::from_str(&content)
            .map_err(|e| EffectLoadError::ParseError(format!("RON parse error: {}", e)))
    }

    /// PNG 텍스처 로드 (Flipbook용)
    pub fn load_png_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
        label: &str,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), EffectLoadError> {
        let img = image::open(path)
            .map_err(|e| EffectLoadError::TextureError(format!("Failed to open image: {}", e)))?
            .to_rgba8();

        let dimensions = img.dimensions();

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok((texture, view))
    }

    /// EXR 텍스처 로드 (VAT Position/Normal용)
    pub fn load_exr_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
        label: &str,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), EffectLoadError> {
        use exr::prelude::*;

        // EXR 파일 읽기
        let image = read_first_rgba_layer_from_file(
            path,
            |resolution, _| {
                vec![(0.0f32, 0.0f32, 0.0f32, 1.0f32); resolution.width() * resolution.height()]
            },
            |pixels, position, (r, g, b, a): (f32, f32, f32, f32)| {
                pixels[position.y() * position.width() + position.x()] = (r, g, b, a);
            },
        ).map_err(|e| EffectLoadError::TextureError(format!("Failed to read EXR: {}", e)))?;

        let width = image.layer_data.size.width();
        let height = image.layer_data.size.height();
        let pixels = &image.layer_data.channel_data.pixels;

        // f32 튜플을 바이트로 변환
        let pixel_bytes: Vec<u8> = pixels
            .iter()
            .flat_map(|&(r, g, b, a)| {
                let mut bytes = Vec::with_capacity(16);
                bytes.extend_from_slice(&r.to_le_bytes());
                bytes.extend_from_slice(&g.to_le_bytes());
                bytes.extend_from_slice(&b.to_le_bytes());
                bytes.extend_from_slice(&a.to_le_bytes());
                bytes
            })
            .collect();

        let texture_size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixel_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16 * width as u32), // 4 * f32 * width
                rows_per_image: Some(height as u32),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok((texture, view))
    }

    /// Flipbook 에셋 전체 로드
    pub fn load_flipbook_asset(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ron_path: &Path,
    ) -> Result<EffectAsset, EffectLoadError> {
        let meta = Self::load_flipbook_meta(ron_path)?;

        // 텍스처 경로 (RON 파일 기준 상대 경로)
        let texture_path = ron_path.parent()
            .unwrap_or(Path::new("."))
            .join(&meta.texture);

        let (texture, texture_view) = Self::load_png_texture(
            device,
            queue,
            &texture_path,
            &format!("flipbook_{}", meta.texture),
        )?;

        Ok(EffectAsset::Flipbook {
            meta,
            texture,
            texture_view,
        })
    }

    /// VAT 에셋 전체 로드 (메시는 별도)
    pub fn load_vat_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ron_path: &Path,
    ) -> Result<(VatMeta, wgpu::Texture, wgpu::TextureView, Option<wgpu::Texture>, Option<wgpu::TextureView>), EffectLoadError> {
        let meta = Self::load_vat_meta(ron_path)?;

        let base_path = ron_path.parent().unwrap_or(Path::new("."));

        // Position 텍스처 (필수)
        let pos_path = base_path.join(&meta.position_texture);
        let (pos_tex, pos_view) = Self::load_exr_texture(
            device,
            queue,
            &pos_path,
            &format!("vat_position_{}", meta.position_texture),
        )?;

        // Normal 텍스처 (선택)
        let (normal_tex, normal_view) = if let Some(ref normal_path) = meta.normal_texture {
            let np = base_path.join(normal_path);
            let (tex, view) = Self::load_exr_texture(
                device,
                queue,
                &np,
                &format!("vat_normal_{}", normal_path),
            )?;
            (Some(tex), Some(view))
        } else {
            (None, None)
        };

        Ok((meta, pos_tex, pos_view, normal_tex, normal_view))
    }
}

/// Effect 로드 에러
#[derive(Debug)]
pub enum EffectLoadError {
    IoError(String),
    ParseError(String),
    TextureError(String),
    MeshError(String),
}

impl std::fmt::Display for EffectLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectLoadError::IoError(s) => write!(f, "IO Error: {}", s),
            EffectLoadError::ParseError(s) => write!(f, "Parse Error: {}", s),
            EffectLoadError::TextureError(s) => write!(f, "Texture Error: {}", s),
            EffectLoadError::MeshError(s) => write!(f, "Mesh Error: {}", s),
        }
    }
}

impl std::error::Error for EffectLoadError {}
