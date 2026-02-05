//! Buffer pool — size-class-based GPU buffer reuse.
//!
//! Allocates buffers rounded up to the nearest power-of-two size class,
//! and returns freed buffers to the pool for reuse.

#[cfg(feature = "gpu")]
use std::collections::HashMap;

/// Key for buffer pool buckets.
#[cfg(feature = "gpu")]
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct PoolKey {
    /// log2 of the size class (e.g., 16 = 64KB)
    size_class: u32,
    /// Required usage flags.
    usage: wgpu::BufferUsages,
}

/// Pool of reusable GPU buffers organized by size class.
#[cfg(feature = "gpu")]
pub struct BufferPool {
    pools: HashMap<PoolKey, Vec<wgpu::Buffer>>,
    stats: BufferPoolStats,
}

#[cfg(feature = "gpu")]
#[derive(Default, Clone, Debug)]
pub struct BufferPoolStats {
    pub hits: u64,
    pub misses: u64,
    pub returns: u64,
    pub total_allocated_bytes: u64,
}

#[cfg(feature = "gpu")]
impl BufferPool {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            stats: BufferPoolStats::default(),
        }
    }

    /// Acquire a buffer of at least `size` bytes with the given usage.
    ///
    /// Returns a pooled buffer if available, otherwise creates a new one.
    pub fn acquire(
        &mut self,
        device: &wgpu::Device,
        label: &str,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        let size_class = size_to_class(size);
        let actual_size = 1u64 << size_class;
        let key = PoolKey {
            size_class,
            usage,
        };

        if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(buffer) = pool.pop() {
                self.stats.hits += 1;
                return buffer;
            }
        }

        self.stats.misses += 1;
        self.stats.total_allocated_bytes += actual_size;

        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: actual_size,
            usage,
            mapped_at_creation: false,
        })
    }

    /// Return a buffer to the pool for future reuse.
    pub fn release(&mut self, buffer: wgpu::Buffer) {
        let size_class = size_to_class(buffer.size());
        let key = PoolKey {
            size_class,
            usage: buffer.usage(),
        };

        self.stats.returns += 1;
        self.pools.entry(key).or_default().push(buffer);
    }

    /// Trim the pool: release all pooled buffers.
    pub fn trim(&mut self) {
        let count: usize = self.pools.values().map(|v| v.len()).sum();
        self.pools.clear();
        if count > 0 {
            log::debug!("BufferPool: trimmed {} buffers", count);
        }
    }

    /// Trim buffers exceeding a per-bucket limit.
    pub fn trim_excess(&mut self, max_per_bucket: usize) {
        for pool in self.pools.values_mut() {
            if pool.len() > max_per_bucket {
                pool.truncate(max_per_bucket);
            }
        }
    }

    pub fn stats(&self) -> &BufferPoolStats {
        &self.stats
    }

    pub fn total_pooled(&self) -> usize {
        self.pools.values().map(|v| v.len()).sum()
    }
}

#[cfg(feature = "gpu")]
impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a byte size to a size class (log2, minimum 256 bytes).
fn size_to_class(size: u64) -> u32 {
    let min_class = 8; // 256 bytes
    let bits = 64 - size.max(1).leading_zeros();
    let class = if size.is_power_of_two() {
        bits - 1
    } else {
        bits
    };
    class.max(min_class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_class() {
        assert_eq!(size_to_class(1), 8);     // min 256
        assert_eq!(size_to_class(256), 8);   // exact
        assert_eq!(size_to_class(257), 9);   // round up to 512
        assert_eq!(size_to_class(1024), 10); // exact
        assert_eq!(size_to_class(65536), 16);
    }
}
