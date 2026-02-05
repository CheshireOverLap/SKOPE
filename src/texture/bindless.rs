// SKOPE Engine - Bindless Texture System
//
// Provides bindless texture access using wgpu's texture binding arrays.
// This allows shaders to access textures by index without rebinding.
//
// Features:
// - Dynamic texture registration
// - Automatic handle management
// - GPU-side texture array binding

use std::collections::HashMap;
use wgpu;

/// Maximum number of textures in the bindless heap
/// Limited by GPU capabilities (typically 16k-1M on modern GPUs)
pub const MAX_BINDLESS_TEXTURES: u32 = 4096;

/// Texture handle for bindless access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

impl TextureHandle {
    pub const INVALID: Self = Self(u32::MAX);

    pub fn index(&self) -> u32 {
        self.0
    }

    pub fn is_valid(&self) -> bool {
        self.0 != u32::MAX
    }
}

/// Bindless texture heap
/// Manages a pool of textures accessible by index in shaders
pub struct BindlessTextureHeap {
    /// All texture views in the heap
    textures: Vec<Option<wgpu::TextureView>>,

    /// Free list for reusing slots
    free_list: Vec<u32>,

    /// Next allocation index
    next_index: u32,

    /// Name to handle mapping for named textures
    name_map: HashMap<String, TextureHandle>,

    /// Bind group layout
    pub bind_group_layout: wgpu::BindGroupLayout,

    /// Current bind group (recreated when textures change)
    bind_group: Option<wgpu::BindGroup>,

    /// Sampler for all textures
    pub sampler: wgpu::Sampler,

    /// Whether bind group needs recreation
    dirty: bool,

    /// Placeholder texture view for empty slots
    placeholder_view: wgpu::TextureView,
}

impl BindlessTextureHeap {
    /// Create a new bindless texture heap
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // Check for required features
        let features = device.features();
        let has_binding_arrays = features.contains(wgpu::Features::TEXTURE_BINDING_ARRAY)
            && features.contains(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING);

        if !has_binding_arrays {
            log::warn!("[Bindless] Device doesn't support texture binding arrays. Falling back to limited mode.");
        }

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bindless Texture Heap Layout"),
            entries: &[
                // Texture array
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    // Variable count for bindless
                    count: Some(std::num::NonZeroU32::new(MAX_BINDLESS_TEXTURES).unwrap()),
                },
                // Shared sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create shared sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bindless Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        // Create placeholder texture (1x1 magenta for debugging)
        let placeholder = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bindless Placeholder"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &placeholder,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 0, 255, 255], // Magenta
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );

        let placeholder_view = placeholder.create_view(&wgpu::TextureViewDescriptor::default());

        // Initialize texture slots with placeholder
        let mut textures = Vec::with_capacity(MAX_BINDLESS_TEXTURES as usize);
        for _ in 0..MAX_BINDLESS_TEXTURES {
            textures.push(None);
        }

        log::info!("[Bindless] Created texture heap with capacity {}", MAX_BINDLESS_TEXTURES);

        Self {
            textures,
            free_list: Vec::new(),
            next_index: 0,
            name_map: HashMap::new(),
            bind_group_layout,
            bind_group: None,
            sampler,
            dirty: true,
            placeholder_view,
        }
    }

    /// Register a texture and get a handle
    pub fn register(&mut self, view: wgpu::TextureView) -> TextureHandle {
        let index = if let Some(idx) = self.free_list.pop() {
            idx
        } else if self.next_index < MAX_BINDLESS_TEXTURES {
            let idx = self.next_index;
            self.next_index += 1;
            idx
        } else {
            log::error!("[Bindless] Texture heap full!");
            return TextureHandle::INVALID;
        };

        self.textures[index as usize] = Some(view);
        self.dirty = true;

        TextureHandle(index)
    }

    /// Register a texture with a name
    pub fn register_named(&mut self, name: &str, view: wgpu::TextureView) -> TextureHandle {
        let handle = self.register(view);
        if handle.is_valid() {
            self.name_map.insert(name.to_string(), handle);
        }
        handle
    }

    /// Get handle by name
    pub fn get_handle(&self, name: &str) -> Option<TextureHandle> {
        self.name_map.get(name).copied()
    }

    /// Unregister a texture
    pub fn unregister(&mut self, handle: TextureHandle) {
        if handle.is_valid() && (handle.0 as usize) < self.textures.len() {
            self.textures[handle.0 as usize] = None;
            self.free_list.push(handle.0);
            self.dirty = true;

            // Remove from name map if present
            self.name_map.retain(|_, &mut h| h != handle);
        }
    }

    /// Get or create the bind group
    pub fn bind_group(&mut self, device: &wgpu::Device) -> &wgpu::BindGroup {
        if self.dirty || self.bind_group.is_none() {
            self.rebuild_bind_group(device);
        }
        self.bind_group.as_ref().unwrap()
    }

    /// Rebuild the bind group
    fn rebuild_bind_group(&mut self, device: &wgpu::Device) {
        // Collect texture view references
        let views: Vec<&wgpu::TextureView> = self.textures
            .iter()
            .map(|opt| opt.as_ref().unwrap_or(&self.placeholder_view))
            .collect();

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bindless Texture Heap Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&views),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));

        self.dirty = false;
        log::debug!("[Bindless] Rebuilt bind group with {} textures", self.next_index);
    }

    /// Get statistics
    pub fn stats(&self) -> BindlessStats {
        let used = self.textures.iter().filter(|t| t.is_some()).count() as u32;
        BindlessStats {
            used,
            capacity: MAX_BINDLESS_TEXTURES,
            free_list_size: self.free_list.len() as u32,
        }
    }
}

/// Bindless heap statistics
#[derive(Debug, Clone, Copy)]
pub struct BindlessStats {
    pub used: u32,
    pub capacity: u32,
    pub free_list_size: u32,
}

/// Helper for shader code generation
impl BindlessTextureHeap {
    /// WGSL declaration for bindless textures
    pub const WGSL_DECLARATION: &'static str = r#"
// Bindless texture heap (requires TEXTURE_BINDING_ARRAY feature)
@group(BINDLESS_GROUP) @binding(0) var bindless_textures: binding_array<texture_2d<f32>>;
@group(BINDLESS_GROUP) @binding(1) var bindless_sampler: sampler;

// Sample from bindless texture by handle index
fn sample_bindless(handle: u32, uv: vec2<f32>) -> vec4<f32> {
    return textureSample(bindless_textures[handle], bindless_sampler, uv);
}

fn sample_bindless_level(handle: u32, uv: vec2<f32>, level: f32) -> vec4<f32> {
    return textureSampleLevel(bindless_textures[handle], bindless_sampler, uv, level);
}
"#;
}
