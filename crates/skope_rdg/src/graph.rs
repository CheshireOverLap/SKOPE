use crate::handle::{RDGTextureHandle, RDGBufferHandle};
use crate::pass::RDGPassNode;

#[cfg(feature = "gpu")]
use crate::resource::{RDGTexture, RDGBuffer, RDGTextureDesc, RDGBufferDesc};
#[cfg(feature = "gpu")]
use crate::pass::RDGPassContext;
#[cfg(feature = "gpu")]
use crate::pool::TransientResourcePool;

/// Registry of all resources in the graph, used during pass execution.
/// Borrows from the RenderGraph's resource vectors.
#[cfg(feature = "gpu")]
pub struct RDGResourceRegistry<'a> {
    pub textures: &'a Vec<RDGTexture>,
    pub buffers: &'a Vec<RDGBuffer>,
}

#[cfg(feature = "gpu")]
impl<'a> RDGResourceRegistry<'a> {
    pub fn get_texture(&self, handle: RDGTextureHandle) -> &RDGTexture {
        &self.textures[handle.index()]
    }

    pub fn get_buffer(&self, handle: RDGBufferHandle) -> &RDGBuffer {
        &self.buffers[handle.index()]
    }

    pub fn texture_view(&self, handle: RDGTextureHandle) -> &wgpu::TextureView {
        self.textures[handle.index()].view()
    }

    pub fn buffer(&self, handle: RDGBufferHandle) -> &wgpu::Buffer {
        self.buffers[handle.index()].buffer()
    }
}

/// The compiled and executable render graph.
///
/// Manages passes, resources, topological ordering, dead-pass elimination,
/// and transient resource allocation.
#[cfg(feature = "gpu")]
pub struct RenderGraph {
    passes: Vec<RDGPassNode>,
    textures: Vec<RDGTexture>,
    buffers: Vec<RDGBuffer>,
    execution_order: Vec<usize>,
    compiled: bool,
    pool: TransientResourcePool,
    next_pass_id: u32,
}

#[cfg(feature = "gpu")]
impl RenderGraph {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            textures: Vec::new(),
            buffers: Vec::new(),
            execution_order: Vec::new(),
            compiled: false,
            pool: TransientResourcePool::new(),
            next_pass_id: 0,
        }
    }

    /// Reset the graph for a new frame (reuse allocations).
    pub fn reset(&mut self) {
        self.passes.clear();
        self.textures.clear();
        self.buffers.clear();
        self.execution_order.clear();
        self.compiled = false;
        self.next_pass_id = 0;
    }

    // ── Resource creation ──────────────────────────────────────────────

    pub fn create_texture(&mut self, desc: RDGTextureDesc) -> RDGTextureHandle {
        let handle = RDGTextureHandle(self.textures.len() as u32);
        self.textures.push(RDGTexture::Transient {
            desc,
            texture: None,
            view: None,
        });
        handle
    }

    pub fn import_texture(
        &mut self,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> RDGTextureHandle {
        let handle = RDGTextureHandle(self.textures.len() as u32);
        self.textures.push(RDGTexture::Imported {
            view,
            width,
            height,
            format,
        });
        handle
    }

    pub fn create_buffer(&mut self, desc: RDGBufferDesc) -> RDGBufferHandle {
        let handle = RDGBufferHandle(self.buffers.len() as u32);
        self.buffers.push(RDGBuffer::Transient {
            desc,
            buffer: None,
        });
        handle
    }

    pub fn import_buffer(&mut self, buffer: wgpu::Buffer) -> RDGBufferHandle {
        let handle = RDGBufferHandle(self.buffers.len() as u32);
        self.buffers.push(RDGBuffer::Imported { buffer });
        handle
    }

    // ── Pass creation ──────────────────────────────────────────────────

    pub(crate) fn allocate_pass_id(&mut self) -> u32 {
        let id = self.next_pass_id;
        self.next_pass_id += 1;
        id
    }

    pub(crate) fn add_pass_node(&mut self, node: RDGPassNode) {
        self.passes.push(node);
    }

    // ── Compilation ────────────────────────────────────────────────────

    /// Compile the graph: topological sort, dead-pass elimination, resource allocation.
    pub fn compile(&mut self, device: &wgpu::Device) {
        self.cull_dead_passes();
        self.topological_sort();
        self.allocate_transient_resources(device);
        self.compiled = true;
    }

    /// Remove disabled passes and passes whose outputs are never read.
    fn cull_dead_passes(&mut self) {
        // First: remove explicitly disabled passes.
        for pass in &mut self.passes {
            if !pass.enabled {
                pass.ref_count = 0;
                continue;
            }
        }

        // Count references: for each pass, how many enabled passes read its outputs?
        // Initialize ref_count based on side_effects.
        for pass in &mut self.passes {
            if pass.enabled {
                pass.ref_count = if pass.has_side_effects { 1 } else { 0 };
            }
        }

        // Build write→handle mapping: which pass writes which texture/buffer.
        let pass_count = self.passes.len();

        // For each enabled pass, increment ref_count of passes that produce its read dependencies.
        for reader_idx in 0..pass_count {
            if !self.passes[reader_idx].enabled {
                continue;
            }

            let reads: Vec<RDGTextureHandle> = self.passes[reader_idx].reads.clone();
            let read_bufs: Vec<RDGBufferHandle> = self.passes[reader_idx].read_buffers.clone();

            for writer_idx in 0..pass_count {
                if writer_idx == reader_idx || !self.passes[writer_idx].enabled {
                    continue;
                }

                // Check if writer produces any texture that reader reads.
                let writes_texture = self.passes[writer_idx]
                    .writes
                    .iter()
                    .any(|w| reads.contains(w));
                let writes_buffer = self.passes[writer_idx]
                    .write_buffers
                    .iter()
                    .any(|w| read_bufs.contains(w));

                if writes_texture || writes_buffer {
                    self.passes[writer_idx].ref_count += 1;
                }
            }
        }

        // Iteratively remove passes with ref_count == 0 (no readers, no side effects).
        let mut changed = true;
        while changed {
            changed = false;
            for i in 0..pass_count {
                if self.passes[i].enabled && self.passes[i].ref_count == 0 {
                    self.passes[i].enabled = false;
                    changed = true;

                    // Decrement ref_count for passes this pass was reading from.
                    let reads: Vec<RDGTextureHandle> = self.passes[i].reads.clone();
                    let read_bufs: Vec<RDGBufferHandle> = self.passes[i].read_buffers.clone();

                    for j in 0..pass_count {
                        if j == i || !self.passes[j].enabled {
                            continue;
                        }
                        let produces_tex = self.passes[j]
                            .writes
                            .iter()
                            .any(|w| reads.contains(w));
                        let produces_buf = self.passes[j]
                            .write_buffers
                            .iter()
                            .any(|w| read_bufs.contains(w));

                        if produces_tex || produces_buf {
                            self.passes[j].ref_count = self.passes[j].ref_count.saturating_sub(1);
                        }
                    }
                }
            }
        }

        log::debug!(
            "RDG cull: {}/{} passes survived",
            self.passes.iter().filter(|p| p.enabled).count(),
            pass_count
        );

        // Warn about resources written but never read (potential dependency bugs).
        {
            let mut written_textures = std::collections::HashSet::new();
            let mut read_textures = std::collections::HashSet::new();
            let mut written_buffers = std::collections::HashSet::new();
            let mut read_buffers = std::collections::HashSet::new();

            for pass in &self.passes {
                if !pass.enabled {
                    continue;
                }
                // Skip side-effect passes — their writes are intentional even if unread.
                if pass.has_side_effects {
                    continue;
                }
                for &h in &pass.writes {
                    written_textures.insert(h);
                }
                for &h in &pass.write_buffers {
                    written_buffers.insert(h);
                }
            }

            for pass in &self.passes {
                if !pass.enabled {
                    continue;
                }
                for &h in &pass.reads {
                    read_textures.insert(h);
                }
                for &h in &pass.read_buffers {
                    read_buffers.insert(h);
                }
            }

            for &h in &written_textures {
                if !read_textures.contains(&h) {
                    log::warn!(
                        "RDG: texture handle {} is written but never read by any enabled pass",
                        h.index()
                    );
                }
            }
            for &h in &written_buffers {
                if !read_buffers.contains(&h) {
                    log::warn!(
                        "RDG: buffer handle {} is written but never read by any enabled pass",
                        h.index()
                    );
                }
            }
        }
    }

    /// Topological sort of enabled passes based on read/write dependencies.
    fn topological_sort(&mut self) {
        let n = self.passes.len();
        // Build adjacency: if pass A writes resource X and pass B reads X, then A -> B.
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree: Vec<u32> = vec![0; n];

        for b in 0..n {
            if !self.passes[b].enabled {
                continue;
            }
            for a in 0..n {
                if a == b || !self.passes[a].enabled {
                    continue;
                }
                // Check: does A write something that B reads?
                let tex_dep = self.passes[a]
                    .writes
                    .iter()
                    .any(|w| self.passes[b].reads.contains(w));
                let buf_dep = self.passes[a]
                    .write_buffers
                    .iter()
                    .any(|w| self.passes[b].read_buffers.contains(w));

                if tex_dep || buf_dep {
                    adj[a].push(b);
                    in_degree[b] += 1;
                }
            }
        }

        // Kahn's algorithm: stable ordering (preserves insertion order for equal priority).
        let mut queue: Vec<usize> = Vec::new();
        for i in 0..n {
            if self.passes[i].enabled && in_degree[i] == 0 {
                queue.push(i);
            }
        }

        self.execution_order.clear();
        let mut head = 0;
        while head < queue.len() {
            let curr = queue[head];
            head += 1;
            self.execution_order.push(curr);

            for &next in &adj[curr] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push(next);
                }
            }
        }

        let enabled_count = self.passes.iter().filter(|p| p.enabled).count();
        if self.execution_order.len() != enabled_count {
            log::warn!(
                "RDG: topological sort incomplete ({}/{}), possible cycle detected",
                self.execution_order.len(),
                enabled_count
            );
        }

        log::debug!(
            "RDG execution order: [{}]",
            self.execution_order
                .iter()
                .map(|&i| self.passes[i].label)
                .collect::<Vec<_>>()
                .join(" -> ")
        );
    }

    /// Allocate wgpu resources for transient textures and buffers.
    fn allocate_transient_resources(&mut self, device: &wgpu::Device) {
        for tex in &mut self.textures {
            if let RDGTexture::Transient {
                desc,
                texture,
                view,
            } = tex
            {
                if texture.is_none() {
                    let t = self.pool.acquire_texture(device, desc);
                    let v = t.create_view(&wgpu::TextureViewDescriptor::default());
                    *texture = Some(t);
                    *view = Some(v);
                }
            }
        }

        for buf in &mut self.buffers {
            if let RDGBuffer::Transient { desc, buffer } = buf {
                if buffer.is_none() {
                    let b = self.pool.acquire_buffer(device, desc);
                    *buffer = Some(b);
                }
            }
        }
    }

    // ── Execution ──────────────────────────────────────────────────────

    /// Execute all compiled passes in topological order.
    ///
    /// Pass callbacks receive a `RDGPassContext` with access to the device,
    /// queue, encoder, and a resource registry for looking up textures/buffers.
    pub fn execute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.execute_with_data(device, queue, encoder, None);
    }

    /// Execute the compiled graph with optional user data passed to each pass context.
    pub fn execute_with_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        user_data: Option<*mut ()>,
    ) {
        if !self.compiled {
            log::warn!("RDG: executing uncompiled graph; call compile() first");
            self.compile(device);
        }

        let order = std::mem::take(&mut self.execution_order);

        // Extract pass execute callbacks so we can borrow self.textures/buffers
        // immutably while calling the mutable callback with &mut encoder.
        let mut callbacks: Vec<(usize, Box<dyn FnOnce(&mut RDGPassContext) + Send>)> = Vec::new();
        for &pass_idx in &order {
            let pass = &mut self.passes[pass_idx];
            if pass.enabled {
                if let Some(cb) = pass.execute_fn.take() {
                    callbacks.push((pass_idx, cb));
                }
            }
        }

        // Build a registry that borrows our resource vecs.
        let registry = RDGResourceRegistry {
            textures: &self.textures,
            buffers: &self.buffers,
        };

        // Execute each callback in order.
        for (pass_idx, execute_fn) in callbacks {
            let label = self.passes[pass_idx].label;
            log::trace!("RDG: executing pass '{}'", label);

            let mut ctx = RDGPassContext {
                encoder,
                device,
                queue,
                resources: &registry,
                user_data,
            };
            execute_fn(&mut ctx);
        }

        self.execution_order = order;
    }

    // ── Queries ────────────────────────────────────────────────────────

    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    pub fn enabled_pass_count(&self) -> usize {
        self.passes.iter().filter(|p| p.enabled).count()
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Get a texture view by handle (for external use after graph execution).
    pub fn texture_view(&self, handle: RDGTextureHandle) -> &wgpu::TextureView {
        self.textures[handle.index()].view()
    }

    /// Release transient resources back to the pool for reuse next frame.
    pub fn reclaim(&mut self) {
        for tex in self.textures.drain(..) {
            if let RDGTexture::Transient { texture: Some(t), .. } = tex {
                self.pool.return_texture(t);
            }
        }
        for buf in self.buffers.drain(..) {
            if let RDGBuffer::Transient { buffer: Some(b), .. } = buf {
                self.pool.return_buffer(b);
            }
        }
    }

    /// Trim the pool (release GPU memory for unused resources).
    pub fn trim_pool(&mut self) {
        self.pool.trim();
    }
}

#[cfg(feature = "gpu")]
impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}
