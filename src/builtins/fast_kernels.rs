/// Ultra-fast kernels following kdb optimization principles
use std::mem::MaybeUninit;

const NA: f64 = -99999.0;

/// Level 0: Original fused kernel (baseline)
pub fn dlog_v0_baseline(data: &[f64], lag: usize) -> Vec<f64> {
    let mut result = vec![NA; data.len()];

    for i in lag..data.len() {
        let curr = data[i];
        let prev = data[i - lag];

        if curr != NA && curr > 0.0 && prev != NA && prev > 0.0 {
            result[i] = curr.ln() - prev.ln();
        }
    }

    result
}

/// Level 1: Remove initialization pass (MaybeUninit)
pub fn dlog_v1_no_init(data: &[f64], lag: usize) -> Vec<f64> {
    let n = data.len();
    let mut out: Vec<MaybeUninit<f64>> = Vec::with_capacity(n);
    unsafe {
        out.set_len(n);
    }

    for i in 0..lag.min(n) {
        out[i].write(NA);
    }

    for i in lag..n {
        let curr = data[i];
        let prev = data[i - lag];
        let ok = curr != NA && curr > 0.0 && prev != NA && prev > 0.0;
        out[i].write(if ok { curr.ln() - prev.ln() } else { NA });
    }

    unsafe { std::mem::transmute(out) }
}

/// Level 2: Remove bounds checks (unsafe pointers)
pub fn dlog_v2_no_bounds(data: &[f64], lag: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![NA; n];
    if lag == 0 || lag >= n {
        return out;
    }

    unsafe {
        let dp = data.as_ptr();
        let op = out.as_mut_ptr();

        for i in lag..n {
            let curr = *dp.add(i);
            let prev = *dp.add(i - lag);

            if curr != NA && curr > 0.0 && prev != NA && prev > 0.0 {
                *op.add(i) = curr.ln() - prev.ln();
            }
        }
    }
    out
}

/// Level 3: Fast path for no-nulls (assumes all positive prices)
pub fn dlog_v3_no_nulls(data: &[f64], lag: usize) -> Vec<f64> {
    let n = data.len();
    let mut out = vec![0.0; n];
    if lag == 0 || lag >= n {
        return out;
    }

    for i in 0..lag {
        out[i] = f64::NAN;
    }

    unsafe {
        let xp = data.as_ptr();
        let op = out.as_mut_ptr();

        for i in lag..n {
            let curr = *xp.add(i);
            let prev = *xp.add(i - lag);
            *op.add(i) = curr.ln() - prev.ln();
        }
    }
    out
}

/// Level 4: Masked version with validity bitmap
pub fn dlog_v4_masked(data: &[f64], valid: &[u8], lag: usize) -> (Vec<f64>, Vec<u8>) {
    let n = data.len();
    assert_eq!(valid.len(), n);

    let mut out = vec![0.0; n];
    let mut out_valid = vec![0u8; n];

    if lag == 0 || lag >= n {
        return (out, out_valid);
    }

    unsafe {
        let xp = data.as_ptr();
        let xv = valid.as_ptr();
        let op = out.as_mut_ptr();
        let ov = out_valid.as_mut_ptr();

        for i in lag..n {
            let v_curr = *xv.add(i);
            let v_prev = *xv.add(i - lag);

            if (v_curr & v_prev) == 1 {
                let curr = *xp.add(i);
                let prev = *xp.add(i - lag);
                *op.add(i) = curr.ln() - prev.ln();
                *ov.add(i) = 1;
            }
        }
    }

    (out, out_valid)
}

/// Level 5: Masked with fast-path optimization
pub fn dlog_v5_masked_fast(
    data: &[f64],
    valid: Option<&[u8]>,
    lag: usize,
) -> (Vec<f64>, Option<Vec<u8>>) {
    if valid.is_none() {
        let out = dlog_v3_no_nulls(data, lag);
        return (out, None);
    }

    let valid = valid.unwrap();
    let (out, out_valid) = dlog_v4_masked(data, valid, lag);
    (out, Some(out_valid))
}
