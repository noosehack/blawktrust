//! Word-wise bitmap processing (Step 2 optimization)
//!
//! Process validity bitmap 64 bits at a time:
//! - If word is all-valid (0xFFFF...), run tight loop (no checks)
//! - If word is all-null (0x0000...), skip compute entirely
//! - Otherwise, fall back to per-bit checks
//!
//! This reduces masked overhead significantly when nulls are clustered.

use crate::table::Bitmap;

/// Word-wise dlog: Process 64 elements at once based on validity word
pub fn dlog_wordwise(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    lag: usize,
) {
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    if lag == 0 || lag >= n {
        // Mark all as invalid
        for w in 0..out_valid.words_len() {
            out_valid.bits_mut()[w] = 0;
        }
        return;
    }

    // Prefix invalid
    for i in 0..lag {
        out_valid.set(i, false);
    }

    let num_words = x_valid.words_len();

    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        // Process full 64-element words
        for word_idx in 0..num_words {
            let start_idx = word_idx * 64;
            let end_idx = (start_idx + 64).min(n);

            if start_idx < lag {
                // Overlaps with prefix, use per-bit fallback
                for i in start_idx.max(lag)..end_idx {
                    let v_curr = x_valid.get(i);
                    let v_prev = x_valid.get(i - lag);
                    if v_curr && v_prev {
                        *op.add(i) = (*xp.add(i)).ln() - (*xp.add(i - lag)).ln();
                        out_valid.set(i, true);
                    } else {
                        out_valid.set(i, false);
                    }
                }
                continue;
            }

            // Check if all elements in current AND lagged word are valid
            let curr_word = x_valid.word(word_idx);
            let lag_word_idx = (start_idx - lag) / 64;
            let lag_offset = (start_idx - lag) % 64;

            // Simplified: Check if spans are valid
            let all_valid = if lag_offset == 0 {
                // Aligned: just check both words
                curr_word == !0u64 && x_valid.word(lag_word_idx) == !0u64
            } else {
                // Unaligned: conservative fallback
                false
            };

            if all_valid {
                // 🔥 FAST: All 64 elements valid, tight loop, no checks
                for i in start_idx..end_idx {
                    *op.add(i) = (*xp.add(i)).ln() - (*xp.add(i - lag)).ln();
                }
                out_valid.bits_mut()[word_idx] = !0u64;
            } else if curr_word == 0 {
                // 🔥 SKIP: All 64 elements null, skip compute
                out_valid.bits_mut()[word_idx] = 0;
            } else {
                // Mixed: Per-bit fallback
                for i in start_idx..end_idx {
                    let v_curr = x_valid.get(i);
                    let v_prev = x_valid.get(i - lag);
                    if v_curr && v_prev {
                        *op.add(i) = (*xp.add(i)).ln() - (*xp.add(i - lag)).ln();
                        out_valid.set(i, true);
                    } else {
                        out_valid.set(i, false);
                    }
                }
            }
        }
    }
}

/// Word-wise dlog_scale_add: Fused with word-wise bitmap
pub fn dlog_scale_add_wordwise(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    lag: usize,
    a: f64,
    b: f64,
) {
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    if lag == 0 || lag >= n {
        for w in 0..out_valid.words_len() {
            out_valid.bits_mut()[w] = 0;
        }
        return;
    }

    for i in 0..lag {
        out_valid.set(i, false);
    }

    let num_words = x_valid.words_len();

    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        for word_idx in 0..num_words {
            let start_idx = word_idx * 64;
            let end_idx = (start_idx + 64).min(n);

            if start_idx < lag {
                for i in start_idx.max(lag)..end_idx {
                    let v_curr = x_valid.get(i);
                    let v_prev = x_valid.get(i - lag);
                    if v_curr && v_prev {
                        let curr_ln = (*xp.add(i)).ln();
                        let prev_ln = (*xp.add(i - lag)).ln();
                        *op.add(i) = a * (curr_ln - prev_ln) + b;
                        out_valid.set(i, true);
                    } else {
                        out_valid.set(i, false);
                    }
                }
                continue;
            }

            let curr_word = x_valid.word(word_idx);
            let lag_word_idx = (start_idx - lag) / 64;
            let lag_offset = (start_idx - lag) % 64;

            let all_valid = if lag_offset == 0 {
                curr_word == !0u64 && x_valid.word(lag_word_idx) == !0u64
            } else {
                false
            };

            if all_valid {
                // 🔥 TIGHT LOOP: No validity checks for 64 elements
                for i in start_idx..end_idx {
                    let curr_ln = (*xp.add(i)).ln();
                    let prev_ln = (*xp.add(i - lag)).ln();
                    *op.add(i) = a * (curr_ln - prev_ln) + b;
                }
                out_valid.bits_mut()[word_idx] = !0u64;
            } else if curr_word == 0 {
                // Skip compute
                out_valid.bits_mut()[word_idx] = 0;
            } else {
                // Per-bit fallback
                for i in start_idx..end_idx {
                    let v_curr = x_valid.get(i);
                    let v_prev = x_valid.get(i - lag);
                    if v_curr && v_prev {
                        let curr_ln = (*xp.add(i)).ln();
                        let prev_ln = (*xp.add(i - lag)).ln();
                        *op.add(i) = a * (curr_ln - prev_ln) + b;
                        out_valid.set(i, true);
                    } else {
                        out_valid.set(i, false);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlog_wordwise_all_valid() {
        let x = vec![100.0; 128]; // 2 full words
        let x_valid = Bitmap::new_all_valid(128);

        let mut out = vec![0.0; 128];
        let mut out_valid = Bitmap::new_all_null(128);

        dlog_wordwise(&mut out, &mut out_valid, &x, &x_valid, 1);

        // First element invalid (prefix)
        assert!(!out_valid.get(0));
        // Rest should be valid and computed
        assert!(out_valid.get(1));
        assert!(out_valid.get(64));
        assert!(out_valid.get(127));
    }

    #[test]
    fn test_dlog_wordwise_all_null() {
        let x = vec![100.0; 128];
        let x_valid = Bitmap::new_all_null(128);

        let mut out = vec![0.0; 128];
        let mut out_valid = Bitmap::new_all_null(128);

        dlog_wordwise(&mut out, &mut out_valid, &x, &x_valid, 1);

        // All should be invalid
        for i in 0..128 {
            assert!(!out_valid.get(i));
        }
    }

    #[test]
    fn test_dlog_scale_add_wordwise() {
        let x = vec![100.0; 128];
        let x_valid = Bitmap::new_all_valid(128);

        let mut out = vec![0.0; 128];
        let mut out_valid = Bitmap::new_all_null(128);

        dlog_scale_add_wordwise(&mut out, &mut out_valid, &x, &x_valid, 1, 2.0, 1.0);

        assert!(!out_valid.get(0));
        assert!(out_valid.get(1));
        // Value should be: 2.0 * (ln(100) - ln(100)) + 1.0 = 1.0
        assert!((out[1] - 1.0).abs() < 1e-10);
    }
}
