/// Type-safe handle for RDG texture resources.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RDGTextureHandle(pub(crate) u32);

/// Type-safe handle for RDG buffer resources.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RDGBufferHandle(pub(crate) u32);

impl RDGTextureHandle {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl RDGBufferHandle {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
