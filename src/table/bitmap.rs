//! Bit-packed validity bitmap (1 bit per element)
//!
//! This is the proper kdb-style null representation:
//! - None = all valid (fast path, zero overhead)
//! - Some(Bitmap) = bit-packed mask (1 = valid, 0 = null)

#[derive(Clone, Debug)]
pub struct Bitmap {
    /// Each u64 holds 64 validity bits (LSB = bit 0)
    bits: Vec<u64>,
    /// Total number of elements (not bits)
    len: usize,
}

impl Bitmap {
    /// Create bitmap with all bits set to 1 (all valid)
    pub fn new_all_valid(len: usize) -> Self {
        let words = (len + 63) / 64;
        let mut bits = vec![!0u64; words];
        
        // Mask off unused bits in last word
        let rem = len % 64;
        if rem != 0 {
            bits[words - 1] = (1u64 << rem) - 1;
        }
        
        Self { bits, len }
    }

    /// Create bitmap with all bits set to 0 (all null)
    pub fn new_all_null(len: usize) -> Self {
        let words = (len + 63) / 64;
        Self { 
            bits: vec![0u64; words], 
            len 
        }
    }

    #[inline]
    pub fn len(&self) -> usize { 
        self.len 
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get validity bit at index i (true = valid, false = null)
    #[inline]
    pub fn get(&self, i: usize) -> bool {
        debug_assert!(i < self.len);
        let w = i >> 6;  // word index (i / 64)
        let b = i & 63;  // bit index (i % 64)
        (self.bits[w] >> b) & 1 == 1
    }

    /// Set validity bit at index i
    #[inline]
    pub fn set(&mut self, i: usize, v: bool) {
        debug_assert!(i < self.len);
        let w = i >> 6;
        let b = i & 63;
        let mask = 1u64 << b;
        if v { 
            self.bits[w] |= mask; 
        } else { 
            self.bits[w] &= !mask; 
        }
    }

    /// Get raw word at word index (for fast iteration)
    #[inline]
    pub fn word(&self, w: usize) -> u64 {
        self.bits[w]
    }

    /// Number of u64 words
    #[inline]
    pub fn words_len(&self) -> usize {
        self.bits.len()
    }

    /// Direct access to bits (for efficient operations)
    #[inline]
    pub fn bits_mut(&mut self) -> &mut [u64] {
        &mut self.bits
    }

    /// Bitwise AND: out = a & b
    pub fn and_into(a: &Bitmap, b: &Bitmap, out: &mut Bitmap) {
        assert_eq!(a.len, b.len);
        assert_eq!(a.len, out.len);
        for w in 0..a.bits.len() {
            out.bits[w] = a.bits[w] & b.bits[w];
        }
    }

    /// Bitwise OR: out = a | b
    pub fn or_into(a: &Bitmap, b: &Bitmap, out: &mut Bitmap) {
        assert_eq!(a.len, b.len);
        assert_eq!(a.len, out.len);
        for w in 0..a.bits.len() {
            out.bits[w] = a.bits[w] | b.bits[w];
        }
    }

    /// Clone the bits vector
    pub fn clone_bits(&self) -> Vec<u64> {
        self.bits.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_valid() {
        let bm = Bitmap::new_all_valid(100);
        assert_eq!(bm.len(), 100);
        for i in 0..100 {
            assert!(bm.get(i), "bit {} should be valid", i);
        }
    }

    #[test]
    fn test_all_null() {
        let bm = Bitmap::new_all_null(100);
        assert_eq!(bm.len(), 100);
        for i in 0..100 {
            assert!(!bm.get(i), "bit {} should be null", i);
        }
    }

    #[test]
    fn test_set_get() {
        let mut bm = Bitmap::new_all_valid(100);
        bm.set(50, false);
        assert!(!bm.get(50));
        assert!(bm.get(49));
        assert!(bm.get(51));
    }

    #[test]
    fn test_and() {
        let mut a = Bitmap::new_all_valid(128);
        let mut b = Bitmap::new_all_valid(128);
        a.set(10, false);
        b.set(20, false);
        
        let mut out = Bitmap::new_all_valid(128);
        Bitmap::and_into(&a, &b, &mut out);
        
        assert!(!out.get(10), "a was null");
        assert!(!out.get(20), "b was null");
        assert!(out.get(30), "both valid");
    }
}
