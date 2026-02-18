use crate::handle::{RDGTextureHandle, RDGBufferHandle};
use crate::pass::{RDGPassNode, RDGPassSetup, PassType};
use crate::graph::RenderGraph;

#[cfg(feature = "gpu")]
use crate::resource::{RDGTextureDesc, RDGBufferDesc};
#[cfg(feature = "gpu")]
use crate::pass::RDGPassContext;

/// Builder interface for constructing a render graph.
///
/// Usage:
/// ```ignore
/// let mut graph = RenderGraph::new();
/// let mut builder = RDGBuilder::new(&mut graph);
///
/// let depth = builder.create_texture(RDGTextureDesc::new_2d("Depth", w, h, Depth32Float));
/// let hdr = builder.create_texture(RDGTextureDesc::new_2d("HDR", w, h, Rgba16Float).with_storage());
///
/// builder.add_render_pass("Visibility", |setup| {
///     setup.write_texture(depth);
///     Box::new(move |ctx| {
///         // encode render pass commands using ctx.encoder
///     })
/// });
///
/// builder.add_compute_pass("MaterialEval", |setup| {
///     setup.read_texture(depth);
///     setup.write_texture(hdr);
///     Box::new(move |ctx| {
///         // encode compute pass commands
///     })
/// });
///
/// graph.compile(&device);
/// graph.execute(&device, &queue, &mut encoder);
/// ```
#[cfg(feature = "gpu")]
pub struct RDGBuilder<'a> {
    graph: &'a mut RenderGraph,
}

#[cfg(feature = "gpu")]
impl<'a> RDGBuilder<'a> {
    pub fn new(graph: &'a mut RenderGraph) -> Self {
        Self { graph }
    }

    // ── Resource creation (delegates to graph) ─────────────────────────

    pub fn create_texture(&mut self, desc: RDGTextureDesc) -> RDGTextureHandle {
        self.graph.create_texture(desc)
    }

    pub fn import_texture(
        &mut self,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> RDGTextureHandle {
        self.graph.import_texture(view, width, height, format)
    }

    pub fn create_buffer(&mut self, desc: RDGBufferDesc) -> RDGBufferHandle {
        self.graph.create_buffer(desc)
    }

    pub fn import_buffer(&mut self, buffer: wgpu::Buffer) -> RDGBufferHandle {
        self.graph.import_buffer(buffer)
    }

    // ── Pass creation ──────────────────────────────────────────────────

    /// Add a render (rasterization) pass to the graph.
    ///
    /// The `setup_fn` is called immediately to declare resource dependencies.
    /// It must return an execute callback that will be invoked during graph execution.
    pub fn add_render_pass<F>(
        &mut self,
        label: &'static str,
        setup_fn: F,
    ) where
        F: FnOnce(&mut RDGPassSetup) -> Box<dyn FnOnce(&mut RDGPassContext) + Send>,
    {
        self.add_pass_internal(label, PassType::Render, setup_fn);
    }

    /// Add a compute pass to the graph.
    pub fn add_compute_pass<F>(
        &mut self,
        label: &'static str,
        setup_fn: F,
    ) where
        F: FnOnce(&mut RDGPassSetup) -> Box<dyn FnOnce(&mut RDGPassContext) + Send>,
    {
        self.add_pass_internal(label, PassType::Compute, setup_fn);
    }

    /// Add a copy/transfer pass to the graph.
    pub fn add_copy_pass<F>(
        &mut self,
        label: &'static str,
        setup_fn: F,
    ) where
        F: FnOnce(&mut RDGPassSetup) -> Box<dyn FnOnce(&mut RDGPassContext) + Send>,
    {
        self.add_pass_internal(label, PassType::Copy, setup_fn);
    }

    /// Add a pass that only has side effects (e.g., present to swapchain).
    /// This pass will never be culled by dead-pass elimination.
    pub fn add_present_pass<F>(
        &mut self,
        label: &'static str,
        setup_fn: F,
    ) where
        F: FnOnce(&mut RDGPassSetup) -> Box<dyn FnOnce(&mut RDGPassContext) + Send>,
    {
        let id = self.graph.allocate_pass_id();
        let mut node = RDGPassNode {
            id,
            label,
            pass_type: PassType::Render,
            reads: Vec::new(),
            writes: Vec::new(),
            read_buffers: Vec::new(),
            write_buffers: Vec::new(),
            enabled: true,
            has_side_effects: true,
            execute_fn: None,
            ref_count: 0,
        };

        let mut setup = RDGPassSetup { node: &mut node };
        let execute_fn = setup_fn(&mut setup);
        node.execute_fn = Some(execute_fn);

        self.graph.add_pass_node(node);
    }

    // ── Internal ───────────────────────────────────────────────────────

    fn add_pass_internal<F>(
        &mut self,
        label: &'static str,
        pass_type: PassType,
        setup_fn: F,
    ) where
        F: FnOnce(&mut RDGPassSetup) -> Box<dyn FnOnce(&mut RDGPassContext) + Send>,
    {
        let id = self.graph.allocate_pass_id();
        let mut node = RDGPassNode {
            id,
            label,
            pass_type,
            reads: Vec::new(),
            writes: Vec::new(),
            read_buffers: Vec::new(),
            write_buffers: Vec::new(),
            enabled: true,
            has_side_effects: false,
            execute_fn: None,
            ref_count: 0,
        };

        let mut setup = RDGPassSetup { node: &mut node };
        let execute_fn = setup_fn(&mut setup);
        node.execute_fn = Some(execute_fn);

        self.graph.add_pass_node(node);
    }
}
