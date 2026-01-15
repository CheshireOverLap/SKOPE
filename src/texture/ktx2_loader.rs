// SKOPE Engine - KTX2 Texture Loader
//
// Loads GPU-compressed textures in KTX2 format.
// Supports BCn (DXT), ASTC, and ETC2 compression.

use std::path::Path;
use wgpu;

/// KTX2 loading error
#[derive(Debug)]
pub enum Ktx2Error {
    IoError(std::io::Error),
    ParseError(String),
    UnsupportedFormat(String),
    NoMipLevels,
}

impl std::fmt::Display for Ktx2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ktx2Error::IoError(e) => write!(f, "IO error: {}", e),
            Ktx2Error::ParseError(s) => write!(f, "Parse error: {}", s),
            Ktx2Error::UnsupportedFormat(s) => write!(f, "Unsupported format: {}", s),
            Ktx2Error::NoMipLevels => write!(f, "No mip levels found"),
        }
    }
}

impl std::error::Error for Ktx2Error {}

impl From<std::io::Error> for Ktx2Error {
    fn from(e: std::io::Error) -> Self {
        Ktx2Error::IoError(e)
    }
}

/// Loaded KTX2 texture data
pub struct Ktx2Texture {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub array_layers: u32,
    pub format: wgpu::TextureFormat,
    pub data: Vec<Vec<u8>>,  // Per mip level data
    pub is_cubemap: bool,
}

/// Convert KTX2 VkFormat to wgpu TextureFormat
fn vk_format_to_wgpu(vk_format: ktx2::Format) -> Option<wgpu::TextureFormat> {
    use ktx2::Format;

    match vk_format {
        // Uncompressed formats
        Format::R8_UNORM => Some(wgpu::TextureFormat::R8Unorm),
        Format::R8_SNORM => Some(wgpu::TextureFormat::R8Snorm),
        Format::R8_UINT => Some(wgpu::TextureFormat::R8Uint),
        Format::R8_SINT => Some(wgpu::TextureFormat::R8Sint),

        Format::R8G8_UNORM => Some(wgpu::TextureFormat::Rg8Unorm),
        Format::R8G8_SNORM => Some(wgpu::TextureFormat::Rg8Snorm),
        Format::R8G8_UINT => Some(wgpu::TextureFormat::Rg8Uint),
        Format::R8G8_SINT => Some(wgpu::TextureFormat::Rg8Sint),

        Format::R8G8B8A8_UNORM => Some(wgpu::TextureFormat::Rgba8Unorm),
        Format::R8G8B8A8_SNORM => Some(wgpu::TextureFormat::Rgba8Snorm),
        Format::R8G8B8A8_UINT => Some(wgpu::TextureFormat::Rgba8Uint),
        Format::R8G8B8A8_SINT => Some(wgpu::TextureFormat::Rgba8Sint),
        Format::R8G8B8A8_SRGB => Some(wgpu::TextureFormat::Rgba8UnormSrgb),

        Format::B8G8R8A8_UNORM => Some(wgpu::TextureFormat::Bgra8Unorm),
        Format::B8G8R8A8_SRGB => Some(wgpu::TextureFormat::Bgra8UnormSrgb),

        // 16-bit formats
        Format::R16_UNORM => Some(wgpu::TextureFormat::R16Unorm),
        Format::R16_SNORM => Some(wgpu::TextureFormat::R16Snorm),
        Format::R16_UINT => Some(wgpu::TextureFormat::R16Uint),
        Format::R16_SINT => Some(wgpu::TextureFormat::R16Sint),
        Format::R16_SFLOAT => Some(wgpu::TextureFormat::R16Float),

        Format::R16G16_UNORM => Some(wgpu::TextureFormat::Rg16Unorm),
        Format::R16G16_SNORM => Some(wgpu::TextureFormat::Rg16Snorm),
        Format::R16G16_UINT => Some(wgpu::TextureFormat::Rg16Uint),
        Format::R16G16_SINT => Some(wgpu::TextureFormat::Rg16Sint),
        Format::R16G16_SFLOAT => Some(wgpu::TextureFormat::Rg16Float),

        Format::R16G16B16A16_UNORM => Some(wgpu::TextureFormat::Rgba16Unorm),
        Format::R16G16B16A16_SNORM => Some(wgpu::TextureFormat::Rgba16Snorm),
        Format::R16G16B16A16_UINT => Some(wgpu::TextureFormat::Rgba16Uint),
        Format::R16G16B16A16_SINT => Some(wgpu::TextureFormat::Rgba16Sint),
        Format::R16G16B16A16_SFLOAT => Some(wgpu::TextureFormat::Rgba16Float),

        // 32-bit formats
        Format::R32_UINT => Some(wgpu::TextureFormat::R32Uint),
        Format::R32_SINT => Some(wgpu::TextureFormat::R32Sint),
        Format::R32_SFLOAT => Some(wgpu::TextureFormat::R32Float),

        Format::R32G32_UINT => Some(wgpu::TextureFormat::Rg32Uint),
        Format::R32G32_SINT => Some(wgpu::TextureFormat::Rg32Sint),
        Format::R32G32_SFLOAT => Some(wgpu::TextureFormat::Rg32Float),

        Format::R32G32B32A32_UINT => Some(wgpu::TextureFormat::Rgba32Uint),
        Format::R32G32B32A32_SINT => Some(wgpu::TextureFormat::Rgba32Sint),
        Format::R32G32B32A32_SFLOAT => Some(wgpu::TextureFormat::Rgba32Float),

        // BC compressed formats
        Format::BC1_RGB_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc1RgbaUnorm),
        Format::BC1_RGB_SRGB_BLOCK => Some(wgpu::TextureFormat::Bc1RgbaUnormSrgb),
        Format::BC1_RGBA_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc1RgbaUnorm),
        Format::BC1_RGBA_SRGB_BLOCK => Some(wgpu::TextureFormat::Bc1RgbaUnormSrgb),

        Format::BC2_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc2RgbaUnorm),
        Format::BC2_SRGB_BLOCK => Some(wgpu::TextureFormat::Bc2RgbaUnormSrgb),

        Format::BC3_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc3RgbaUnorm),
        Format::BC3_SRGB_BLOCK => Some(wgpu::TextureFormat::Bc3RgbaUnormSrgb),

        Format::BC4_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc4RUnorm),
        Format::BC4_SNORM_BLOCK => Some(wgpu::TextureFormat::Bc4RSnorm),

        Format::BC5_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc5RgUnorm),
        Format::BC5_SNORM_BLOCK => Some(wgpu::TextureFormat::Bc5RgSnorm),

        Format::BC6H_UFLOAT_BLOCK => Some(wgpu::TextureFormat::Bc6hRgbUfloat),
        Format::BC6H_SFLOAT_BLOCK => Some(wgpu::TextureFormat::Bc6hRgbFloat),

        Format::BC7_UNORM_BLOCK => Some(wgpu::TextureFormat::Bc7RgbaUnorm),
        Format::BC7_SRGB_BLOCK => Some(wgpu::TextureFormat::Bc7RgbaUnormSrgb),

        // ETC2 compressed formats
        Format::ETC2_R8G8B8_UNORM_BLOCK => Some(wgpu::TextureFormat::Etc2Rgb8Unorm),
        Format::ETC2_R8G8B8_SRGB_BLOCK => Some(wgpu::TextureFormat::Etc2Rgb8UnormSrgb),
        Format::ETC2_R8G8B8A1_UNORM_BLOCK => Some(wgpu::TextureFormat::Etc2Rgb8A1Unorm),
        Format::ETC2_R8G8B8A1_SRGB_BLOCK => Some(wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb),
        Format::ETC2_R8G8B8A8_UNORM_BLOCK => Some(wgpu::TextureFormat::Etc2Rgba8Unorm),
        Format::ETC2_R8G8B8A8_SRGB_BLOCK => Some(wgpu::TextureFormat::Etc2Rgba8UnormSrgb),

        // ASTC compressed formats
        Format::ASTC_4x4_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B4x4, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_4x4_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B4x4, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_5x4_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B5x4, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_5x4_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B5x4, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_5x5_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B5x5, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_5x5_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B5x5, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_6x5_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B6x5, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_6x5_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B6x5, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_6x6_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B6x6, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_6x6_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B6x6, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_8x5_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B8x5, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_8x5_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B8x5, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_8x6_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B8x6, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_8x6_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B8x6, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_8x8_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B8x8, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_8x8_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B8x8, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_10x5_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x5, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_10x5_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x5, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_10x6_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x6, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_10x6_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x6, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_10x8_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x8, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_10x8_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x8, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_10x10_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x10, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_10x10_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B10x10, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_12x10_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B12x10, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_12x10_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B12x10, channel: wgpu::AstcChannel::UnormSrgb }),
        Format::ASTC_12x12_UNORM_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B12x12, channel: wgpu::AstcChannel::Unorm }),
        Format::ASTC_12x12_SRGB_BLOCK => Some(wgpu::TextureFormat::Astc { block: wgpu::AstcBlock::B12x12, channel: wgpu::AstcChannel::UnormSrgb }),

        _ => None,
    }
}

/// Load a KTX2 texture from a file
pub fn load_ktx2(path: impl AsRef<Path>) -> Result<Ktx2Texture, Ktx2Error> {
    let data = std::fs::read(path)?;
    load_ktx2_from_memory(&data)
}

/// Load a KTX2 texture from memory
pub fn load_ktx2_from_memory(data: &[u8]) -> Result<Ktx2Texture, Ktx2Error> {
    let reader = ktx2::Reader::new(data)
        .map_err(|e| Ktx2Error::ParseError(format!("{:?}", e)))?;

    let header = reader.header();

    // Convert format
    let format = vk_format_to_wgpu(header.format.unwrap_or(ktx2::Format::R8G8B8A8_UNORM))
        .ok_or_else(|| Ktx2Error::UnsupportedFormat(format!("{:?}", header.format)))?;

    // Check if cubemap
    let is_cubemap = header.face_count == 6;

    // Read all mip levels
    let mut mip_data = Vec::new();
    for level in reader.levels() {
        mip_data.push(level.to_vec());
    }

    if mip_data.is_empty() {
        return Err(Ktx2Error::NoMipLevels);
    }

    Ok(Ktx2Texture {
        width: header.pixel_width,
        height: header.pixel_height,
        depth: header.pixel_depth.max(1),
        mip_levels: header.level_count,
        array_layers: header.layer_count.max(1) * header.face_count,
        format,
        data: mip_data,
        is_cubemap,
    })
}

/// Create a wgpu texture from KTX2 data
pub fn create_texture_from_ktx2(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ktx: &Ktx2Texture,
    label: Option<&str>,
) -> wgpu::Texture {
    let dimension = if ktx.depth > 1 {
        wgpu::TextureDimension::D3
    } else {
        wgpu::TextureDimension::D2
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size: wgpu::Extent3d {
            width: ktx.width,
            height: ktx.height,
            depth_or_array_layers: if ktx.is_cubemap { 6 } else { ktx.array_layers.max(ktx.depth) },
        },
        mip_level_count: ktx.mip_levels,
        sample_count: 1,
        dimension,
        format: ktx.format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Upload each mip level
    let block_size = ktx.format.block_copy_size(None).unwrap_or(4);
    let (block_width, block_height) = ktx.format.block_dimensions();

    for (mip, data) in ktx.data.iter().enumerate() {
        let mip_width = (ktx.width >> mip).max(1);
        let mip_height = (ktx.height >> mip).max(1);

        let blocks_wide = (mip_width + block_width - 1) / block_width;
        let blocks_high = (mip_height + block_height - 1) / block_height;
        let bytes_per_row = blocks_wide * block_size;

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(blocks_high),
            },
            wgpu::Extent3d {
                width: mip_width,
                height: mip_height,
                depth_or_array_layers: if ktx.is_cubemap { 6 } else { ktx.array_layers.max(ktx.depth) },
            },
        );
    }

    texture
}
