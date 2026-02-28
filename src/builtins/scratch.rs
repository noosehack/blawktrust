//! Scratch allocator for zero-allocation pipelines
//!
//! Reusable buffer pool to eliminate allocation churn in multi-op pipelines.
//! After warmup, pipelines allocate ~0.

use crate::table::Bitmap;
// Removed unused import: std::mem::MaybeUninit

/// Reusable buffer pool for pipeline operations
///
/// Usage pattern:
/// ```no_run
/// use blawktrust::Scratch;
///
/// let mut scratch = Scratch::new();
///
/// // First call allocates
/// let buf1 = scratch.get_f64(1000);
///
/// // Return when done
/// scratch.return_f64(buf1);
///
/// // Second call reuses (zero allocation!)
/// let buf2 = scratch.get_f64(1000);
/// ```
pub struct Scratch {
    f64_bufs: Vec<Vec<f64>>,
    bitmap_bufs: Vec<Bitmap>,
}

impl Scratch {
    /// Create new scratch allocator
    pub fn new() -> Self {
        Scratch {
            f64_bufs: Vec::new(),
            bitmap_bufs: Vec::new(),
        }
    }

    /// Get f64 buffer of given size (reuses if available)
    pub fn get_f64(&mut self, len: usize) -> Vec<f64> {
        if let Some(mut buf) = self.f64_bufs.pop() {
            // Reuse existing buffer
            if buf.capacity() >= len {
                buf.clear();
                buf.resize(len, 0.0);
                return buf;
            }
            // Buffer too small, drop it and allocate new
        }
        // No buffer available or too small, allocate
        vec![0.0; len]
    }

    /// Get UNINITIALIZED f64 buffer (for masked kernels - Step 1 optimization)
    ///
    /// Use this when you will write to valid indices and validity mask
    /// tracks which indices are valid. Invalid indices can remain uninitialized.
    ///
    /// SAFETY: Caller must ensure they either:
    /// 1. Write to ALL indices before reading, OR
    /// 2. Only read from valid indices (checked via validity mask)
    pub fn get_f64_uninit(&mut self, len: usize) -> Vec<f64> {
        if let Some(mut buf) = self.f64_bufs.pop() {
            // Reuse existing buffer WITHOUT zeroing
            if buf.capacity() >= len {
                unsafe {
                    buf.set_len(len); // Skip clear() and resize() - no zeroing!
                }
                return buf;
            }
            // Buffer too small, drop it
        }
        // No buffer available, allocate (first time only)
        // Still needs to allocate vec, but won't zero on reuse
        Vec::with_capacity(len)
    }

    /// Return f64 buffer to pool
    pub fn return_f64(&mut self, buf: Vec<f64>) {
        self.f64_bufs.push(buf);
    }

    /// Get bitmap of given size (reuses if available)
    pub fn get_bitmap(&mut self, len: usize) -> Bitmap {
        if let Some(bm) = self.bitmap_bufs.pop() {
            // Reuse if same size
            if bm.len() == len {
                return bm;
            }
            // Wrong size, drop it
        }
        // Allocate new
        Bitmap::new_all_null(len)
    }

    /// Return bitmap to pool
    pub fn return_bitmap(&mut self, bm: Bitmap) {
        self.bitmap_bufs.push(bm);
    }

    /// Clear all buffers (free memory)
    pub fn clear(&mut self) {
        self.f64_bufs.clear();
        self.bitmap_bufs.clear();
    }

    /// Get statistics
    pub fn stats(&self) -> ScratchStats {
        ScratchStats {
            f64_bufs: self.f64_bufs.len(),
            bitmap_bufs: self.bitmap_bufs.len(),
        }
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

/// Scratch allocator statistics
#[derive(Debug)]
pub struct ScratchStats {
    pub f64_bufs: usize,
    pub bitmap_bufs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scratch_reuse_f64() {
        let mut scratch = Scratch::new();

        // First allocation
        let buf1 = scratch.get_f64(100);
        assert_eq!(buf1.len(), 100);

        // Return to pool
        scratch.return_f64(buf1);
        assert_eq!(scratch.stats().f64_bufs, 1);

        // Second allocation reuses (no new allocation)
        let buf2 = scratch.get_f64(100);
        assert_eq!(buf2.len(), 100);
        assert_eq!(scratch.stats().f64_bufs, 0); // Buffer taken from pool
    }

    #[test]
    fn test_scratch_resize() {
        let mut scratch = Scratch::new();

        // Get 100-element buffer
        let buf1 = scratch.get_f64(100);
        scratch.return_f64(buf1);

        // Request larger buffer (should reuse and resize)
        let buf2 = scratch.get_f64(200);
        assert_eq!(buf2.len(), 200);
    }

    #[test]
    fn test_scratch_bitmap() {
        let mut scratch = Scratch::new();

        let bm1 = scratch.get_bitmap(100);
        assert_eq!(bm1.len(), 100);

        scratch.return_bitmap(bm1);
        assert_eq!(scratch.stats().bitmap_bufs, 1);

        let bm2 = scratch.get_bitmap(100);
        assert_eq!(bm2.len(), 100);
        assert_eq!(scratch.stats().bitmap_bufs, 0);
    }

    #[test]
    fn test_scratch_clear() {
        let mut scratch = Scratch::new();

        scratch.return_f64(vec![0.0; 100]);
        scratch.return_bitmap(Bitmap::new_all_null(100));

        assert_eq!(scratch.stats().f64_bufs, 1);
        assert_eq!(scratch.stats().bitmap_bufs, 1);

        scratch.clear();

        assert_eq!(scratch.stats().f64_bufs, 0);
        assert_eq!(scratch.stats().bitmap_bufs, 0);
    }
}
