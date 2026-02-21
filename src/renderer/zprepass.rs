// SKOPE Engine - Z-Prepass Module (UE5-style)
//
// Multi-mode depth prepass matching UE5's DepthRendering system.
//
// Pipeline variants:
// - Opaque: depth-only, no fragment shader work (fastest)
// - Masked: alpha-tested materials (fragment shader reads alpha, clips)
//
// Depth Drawing Modes (from types::DepthDrawingMode):
// - None:               No prepass, V-Buffer uses LESS directly
// - NonMaskedOnly:      Only non-masked opaque (default, fastest)
// - AllOccluders:       All geometry marked as occluder
// - AllOpaque:          Every opaque object
// - MaskedOnly:         Only alpha-tested materials
// - AllOpaqueNoVelocity: Full prepass except dynamic objects
//
// Integration:
// 1. Z-Prepass: depth_write=true, depth_compare=LESS
// 2. V-Buffer:  depth_write=false, depth_compare=EQUAL

use bytemuck::{Pod, Zeroable};
use super::types::DepthDrawingMode;

/// Maximum number of meshes per Z-Prepass draw
pub const MAX_ZPREPASS_MESHES: usize = 256;

/// Z-Prepass mesh flags (bitfield)
pub mod zprepass_flags {
    /// Material uses alpha masking (needs masked pipeline)
    pub const MASKED: u32 = 1 << 0;
    /// LOD transition dithering active
    #[allow(dead_code)]
    pub const DITHERED: u32 = 1 << 1;
    /// Two-sided geometry (disable backface culling)
    #[allow(dead_code)]
    pub const TWO_SIDED: u32 = 1 << 2;
    /// This mesh is an occluder (for AllOccluders mode)
    #[allow(dead_code)]
    pub const OCCLUDER: u32 = 1 << 3;
    /// Dynamic/movable object (excluded in AllOpaqueNoVelocity mode)
    #[allow(dead_code)]
    pub const DYNAMIC: u32 = 1 << 4;
}

/// Z-Prepass parameters (matches WGSL struct, 256-byte aligned for dynamic offsets)
#[repr(C, align(256))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ZPrepassParams {
    /// Offset into the unified vertex buffer
    pub vertex_offset: u32,
    /// Offset into the unified index buffer
    pub index_offset: u32,
    /// Base triangle index for instanced rendering
    pub base_triangle: u32,
    /// Material index (for masked pipeline texture lookup)
    pub material_index: u32,
    /// Alpha cutoff threshold (masked materials, typically 0.5)
    pub alpha_cutoff: f32,
    /// LOD dither factor (0.0 = fully opaque, 1.0 = fully dithered out)
    pub lod_dither_factor: f32,
    /// Bitfield: see `zprepass_flags` module
    pub flags: u32,
    pub _pad: u32,
    // Explicit padding to 256 bytes for bytemuck compatibility
    pub _padding: [u32; 56],
}

impl ZPrepassParams {
    /// Create params for opaque (non-masked) geometry
    pub fn new_opaque(vertex_offset: u32, index_offset: u32, base_triangle: u32) -> Self {
        Self {
            vertex_offset,
            index_offset,
            base_triangle,
            material_index: 0,
            alpha_cutoff: 0.5,
            lod_dither_factor: 0.0,
            flags: 0,
            _pad: 0,
            _padding: [0; 56],
        }
    }

    /// Create params for masked (alpha-tested) geometry
    #[allow(dead_code)]
    pub fn new_masked(
        vertex_offset: u32,
        index_offset: u32,
        base_triangle: u32,
        material_index: u32,
        alpha_cutoff: f32,
    ) -> Self {
        Self {
            vertex_offset,
            index_offset,
            base_triangle,
            material_index,
            alpha_cutoff,
            lod_dither_factor: 0.0,
            flags: zprepass_flags::MASKED,
            _pad: 0,
            _padding: [0; 56],
        }
    }

    pub fn is_masked(&self) -> bool {
        self.flags & zprepass_flags::MASKED != 0
    }

    #[allow(dead_code)]
    pub fn is_dithered(&self) -> bool {
        self.flags & zprepass_flags::DITHERED != 0
    }

    pub fn is_occluder(&self) -> bool {
        self.flags & zprepass_flags::OCCLUDER != 0
    }

    pub fn is_dynamic(&self) -> bool {
        self.flags & zprepass_flags::DYNAMIC != 0
    }

    /// Check if this mesh should participate in the given depth drawing mode
    pub fn should_render(&self, mode: DepthDrawingMode) -> bool {
        match mode {
            DepthDrawingMode::None => false,
            DepthDrawingMode::NonMaskedOnly => !self.is_masked(),
            DepthDrawingMode::AllOccluders => self.is_occluder() || !self.is_masked(),
            DepthDrawingMode::AllOpaque => true,
            DepthDrawingMode::MaskedOnly => self.is_masked(),
            DepthDrawingMode::AllOpaqueNoVelocity => !self.is_dynamic(),
        }
    }

    /// Set the occluder flag
    #[allow(dead_code)]
    pub fn with_occluder(mut self) -> Self {
        self.flags |= zprepass_flags::OCCLUDER;
        self
    }

    /// Set the dynamic flag
    #[allow(dead_code)]
    pub fn with_dynamic(mut self) -> Self {
        self.flags |= zprepass_flags::DYNAMIC;
        self
    }

    /// Enable LOD dithering with specified factor
    #[allow(dead_code)]
    pub fn with_dither(mut self, factor: f32) -> Self {
        self.flags |= zprepass_flags::DITHERED;
        self.lod_dither_factor = factor;
        self
    }
}

/// Z-Prepass Pipeline System (UE5-style multi-mode)
///
/// Contains separate render pipelines for different geometry types:
/// - `opaque_pipeline`: Depth-only, no fragment shader work (fastest path)
/// - `masked_pipeline`: Alpha-tested materials with fragment discard
#[allow(dead_code)]
pub struct ZPrepassPipeline {
    /// Depth-only pipeline for non-masked opaque geometry
    pub opaque_pipeline: wgpu::RenderPipeline,
    /// Alpha-clip pipeline for masked materials (uses Group 2 material binding)
    pub masked_pipeline: wgpu::RenderPipeline,
    /// Camera bind group layout (Group 0: camera + model matrices)
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    /// Params + geometry bind group layout (Group 1: params + vertices + indices)
    pub params_bind_group_layout: wgpu::BindGroupLayout,
    /// Material bind group layout (Group 2: alpha texture + sampler, masked pipeline only)
    pub material_bind_group_layout: wgpu::BindGroupLayout,
    /// Params buffer (256-byte aligned dynamic uniform)
    pub params_buffer: wgpu::Buffer,
    /// Current depth drawing mode
    pub mode: DepthDrawingMode,
}

impl ZPrepassPipeline {
    pub fn new(device: &wgpu::Device, mode: DepthDrawingMode) -> Self {
        // ── Group 0: Camera + Model ──────────────────────────────────
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Z-Prepass Camera Layout"),
                entries: &[
                    // Camera uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Model uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // ── Group 1: Params (dynamic offset) + Vertices + Indices ────
        let params_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Z-Prepass Params + Geometry Layout"),
                entries: &[
                    // Params uniform (with dynamic offset)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<ZPrepassParams>() as u64,
                            ),
                        },
                        count: None,
                    },
                    // Vertices storage buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Indices storage buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // ── Group 2: Material alpha texture (masked pipeline only) ───
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Z-Prepass Material Layout"),
                entries: &[
                    // Alpha / base color texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // ── Params buffer ────────────────────────────────────────────
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Z-Prepass Params Buffer"),
            size: (std::mem::size_of::<ZPrepassParams>() * MAX_ZPREPASS_MESHES) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Shader ───────────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Z-Prepass Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("OUT_DIR"), "/shaders/zprepass.wgsl")).into(),
            ),
        });

        // ── Shared state ─────────────────────────────────────────────
        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        };

        let depth_stencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        // ── Opaque pipeline (depth-only, no fragment work) ───────────
        let opaque_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Z-Prepass Opaque Layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &params_bind_group_layout],
            immediate_size: 0,
        });

        let opaque_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Z-Prepass Opaque Pipeline"),
            layout: Some(&opaque_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_opaque"),
                targets: &[], // No color attachments
                compilation_options: Default::default(),
            }),
            primitive: primitive.clone(),
            depth_stencil: Some(depth_stencil.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // ── Masked pipeline (alpha clip in fragment shader) ──────────
        let masked_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Z-Prepass Masked Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &params_bind_group_layout,
                &material_bind_group_layout,
            ],
            immediate_size: 0,
        });

        let masked_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Z-Prepass Masked Pipeline"),
            layout: Some(&masked_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_masked"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_masked"),
                targets: &[], // No color attachments
                compilation_options: Default::default(),
            }),
            primitive,
            depth_stencil: Some(depth_stencil),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            opaque_pipeline,
            masked_pipeline,
            camera_bind_group_layout,
            params_bind_group_layout,
            material_bind_group_layout,
            params_buffer,
            mode,
        }
    }

    /// Create params + geometry bind group (Group 1)
    pub fn create_params_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Z-Prepass Params Bind Group"),
            layout: &self.params_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(
                            std::mem::size_of::<ZPrepassParams>() as u64,
                        ),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: index_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create material bind group (Group 2) for masked materials
    #[allow(dead_code)]
    pub fn create_material_bind_group(
        &self,
        device: &wgpu::Device,
        alpha_texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Z-Prepass Material Bind Group"),
            layout: &self.material_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(alpha_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Write all params at once before starting render pass
    pub fn write_all_params(&self, queue: &wgpu::Queue, params_list: &[ZPrepassParams]) {
        if params_list.is_empty() {
            return;
        }
        let count = params_list.len().min(MAX_ZPREPASS_MESHES);
        if params_list.len() > MAX_ZPREPASS_MESHES {
            log::warn!(
                "[Z-Prepass] Too many meshes ({} > {}), truncating",
                params_list.len(),
                MAX_ZPREPASS_MESHES
            );
        }
        queue.write_buffer(
            &self.params_buffer,
            0,
            bytemuck::cast_slice(&params_list[..count]),
        );
    }

    /// Get dynamic offset for mesh index
    pub fn get_dynamic_offset(&self, mesh_index: usize) -> u32 {
        (mesh_index * std::mem::size_of::<ZPrepassParams>()) as u32
    }

    /// Select which pipeline to use for a given mesh
    #[allow(dead_code)]
    pub fn select_pipeline(&self, params: &ZPrepassParams) -> &wgpu::RenderPipeline {
        if params.is_masked() {
            &self.masked_pipeline
        } else {
            &self.opaque_pipeline
        }
    }

    /// Filter a list of params by the current depth drawing mode.
    /// Returns indices into the original list of meshes that should be rendered.
    #[allow(dead_code)]
    pub fn filter_by_mode(&self, params_list: &[ZPrepassParams]) -> Vec<usize> {
        params_list
            .iter()
            .enumerate()
            .filter(|(_, p)| p.should_render(self.mode))
            .map(|(i, _)| i)
            .collect()
    }

    /// Update the depth drawing mode
    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: DepthDrawingMode) {
        self.mode = mode;
    }

    /// Returns true if the prepass should run at all
    pub fn is_enabled(&self) -> bool {
        self.mode != DepthDrawingMode::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_params_size() {
        assert_eq!(std::mem::size_of::<ZPrepassParams>(), 256);
    }

    #[test]
    fn test_opaque_params() {
        let p = ZPrepassParams::new_opaque(10, 20, 30);
        assert_eq!(p.vertex_offset, 10);
        assert_eq!(p.index_offset, 20);
        assert_eq!(p.base_triangle, 30);
        assert!(!p.is_masked());
        assert!(!p.is_dithered());
    }

    #[test]
    fn test_masked_params() {
        let p = ZPrepassParams::new_masked(10, 20, 30, 5, 0.33);
        assert!(p.is_masked());
        assert_eq!(p.material_index, 5);
        assert!((p.alpha_cutoff - 0.33).abs() < 1e-6);
    }

    #[test]
    fn test_flags_builder() {
        let p = ZPrepassParams::new_opaque(0, 0, 0)
            .with_occluder()
            .with_dynamic()
            .with_dither(0.5);

        assert!(p.is_occluder());
        assert!(p.is_dynamic());
        assert!(p.is_dithered());
        assert!((p.lod_dither_factor - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_depth_drawing_mode_filter() {
        let opaque = ZPrepassParams::new_opaque(0, 0, 0);
        let masked = ZPrepassParams::new_masked(0, 0, 0, 1, 0.5);
        let occluder = ZPrepassParams::new_opaque(0, 0, 0).with_occluder();
        let dynamic_mesh = ZPrepassParams::new_opaque(0, 0, 0).with_dynamic();

        // None mode: nothing renders
        assert!(!opaque.should_render(DepthDrawingMode::None));
        assert!(!masked.should_render(DepthDrawingMode::None));

        // NonMaskedOnly: opaque yes, masked no
        assert!(opaque.should_render(DepthDrawingMode::NonMaskedOnly));
        assert!(!masked.should_render(DepthDrawingMode::NonMaskedOnly));

        // AllOccluders: occluder yes, non-masked opaque yes, masked no
        assert!(occluder.should_render(DepthDrawingMode::AllOccluders));
        assert!(opaque.should_render(DepthDrawingMode::AllOccluders));
        assert!(!masked.should_render(DepthDrawingMode::AllOccluders));

        // AllOpaque: everything
        assert!(opaque.should_render(DepthDrawingMode::AllOpaque));
        assert!(masked.should_render(DepthDrawingMode::AllOpaque));
        assert!(dynamic_mesh.should_render(DepthDrawingMode::AllOpaque));

        // MaskedOnly: only masked
        assert!(!opaque.should_render(DepthDrawingMode::MaskedOnly));
        assert!(masked.should_render(DepthDrawingMode::MaskedOnly));

        // AllOpaqueNoVelocity: everything except dynamic
        assert!(opaque.should_render(DepthDrawingMode::AllOpaqueNoVelocity));
        assert!(!dynamic_mesh.should_render(DepthDrawingMode::AllOpaqueNoVelocity));
    }
}
