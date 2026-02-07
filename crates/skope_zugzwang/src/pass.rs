use crate::handle::{RDGTextureHandle, RDGBufferHandle};

/// Type of render pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassType {
    /// Standard rasterization pass (begin_render_pass).
    Render,
    /// Compute dispatch pass (begin_compute_pass).
    Compute,
    /// Copy/transfer operation.
    Copy,
    /// Async compute (future: when WGPU adds explicit async compute queues).
    AsyncCompute,
}

/// A node in the render graph representing a single pass.
pub struct RDGPassNode {
    pub id: u32,
    pub label: &'static str,
    pub pass_type: PassType,
    /// Textures read by this pass (creates read dependency).
    pub reads: Vec<RDGTextureHandle>,
    /// Textures written by this pass (creates write dependency).
    pub writes: Vec<RDGTextureHandle>,
    /// Buffers read by this pass.
    pub read_buffers: Vec<RDGBufferHandle>,
    /// Buffers written by this pass.
    pub write_buffers: Vec<RDGBufferHandle>,
    /// Whether this pass is enabled (disabled passes are culled).
    pub enabled: bool,
    /// Whether this pass has side effects (prevents culling even if nothing reads its output).
    pub has_side_effects: bool,
    /// The execute callback, consumed during graph execution.
    #[cfg(feature = "gpu")]
    pub execute_fn: Option<Box<dyn FnOnce(&mut RDGPassContext) + Send>>,
    #[cfg(not(feature = "gpu"))]
    pub execute_fn: Option<()>,
    /// Reference count for culling: how many subsequent passes read our outputs.
    pub(crate) ref_count: u32,
}

/// Context provided to pass execute callbacks during graph execution.
#[cfg(feature = "gpu")]
pub struct RDGPassContext<'a> {
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub resources: &'a crate::graph::RDGResourceRegistry<'a>,
}

/// Setup helper used when declaring a pass's resource dependencies.
pub struct RDGPassSetup<'a> {
    pub(crate) node: &'a mut RDGPassNode,
}

impl<'a> RDGPassSetup<'a> {
    /// Declare that this pass reads a texture.
    pub fn read_texture(&mut self, handle: RDGTextureHandle) -> &mut Self {
        if !self.node.reads.contains(&handle) {
            self.node.reads.push(handle);
        }
        self
    }

    /// Declare that this pass writes to a texture.
    pub fn write_texture(&mut self, handle: RDGTextureHandle) -> &mut Self {
        if !self.node.writes.contains(&handle) {
            self.node.writes.push(handle);
        }
        self
    }

    /// Declare that this pass reads a buffer.
    pub fn read_buffer(&mut self, handle: RDGBufferHandle) -> &mut Self {
        if !self.node.read_buffers.contains(&handle) {
            self.node.read_buffers.push(handle);
        }
        self
    }

    /// Declare that this pass writes to a buffer.
    pub fn write_buffer(&mut self, handle: RDGBufferHandle) -> &mut Self {
        if !self.node.write_buffers.contains(&handle) {
            self.node.write_buffers.push(handle);
        }
        self
    }

    /// Mark this pass as enabled/disabled.
    pub fn set_enabled(&mut self, enabled: bool) -> &mut Self {
        self.node.enabled = enabled;
        self
    }

    /// Mark this pass as having side effects (will not be culled even if no one reads output).
    pub fn set_side_effects(&mut self, has_side_effects: bool) -> &mut Self {
        self.node.has_side_effects = has_side_effects;
        self
    }
}
