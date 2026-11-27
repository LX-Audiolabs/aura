// Small test indices/delays convert exactly within the f32 fixture range.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use aura_dsp::{dc_blocker::DcBlocker, delay::DelayLine, dsp_util::mix_add};

#[test]
fn block_dc_step_and_partitions() {
    let mut dc = DcBlocker::with_r(0.5);
    let mut x = [1.0; 4];
    dc.process_block(&mut x[..1]);
    dc.process_block(&mut x[1..]);
    assert_eq!(x, [1.0, 0.5, 0.25, 0.125]);
    dc.process_block(&mut []);
}

#[test]
fn dc_block_rejects_invalid_input_atomically_and_recovers() {
    let mut dc = DcBlocker::with_r(0.5);
    let mut invalid = [1.0, f32::NAN, 1.0];
    dc.process_block(&mut invalid);
    assert_eq!(invalid, [0.0; 3]);
    let mut fresh = DcBlocker::with_r(0.5);
    let mut next = [1.0];
    dc.process_block(&mut next);
    assert_eq!(next[0], fresh.process(1.0));
}

#[test]
fn mix_add_uses_gain_and_requires_matching_lengths() {
    let mut out = [1.0, 2.0, 3.0];
    mix_add(&mut out, &[2.0, -4.0, 8.0], 0.5);
    assert_eq!(out, [2.0, 0.0, 7.0]);
}

#[test]
fn delay_block_taps_match_independent_reads_across_ring_boundary() {
    for capacity in [1, 2, 7, 16] {
        let mut d = DelayLine::new(capacity);
        for n in 0..capacity + 3 {
            d.write(n as f32);
        }
        for delay in [0.0_f32, 0.5, 3.25, 100.0] {
            let mut out = [0.0; 23];
            d.read_block(delay, &mut out);
            for (i, actual) in out.into_iter().enumerate() {
                // Block is newest-to-oldest, cycling stored history.
                let wrapped = delay.clamp(0.0, (capacity - 1) as f32);
                let floor = wrapped.floor() as usize;
                let frac = wrapped - floor as f32;
                let a = d.read(((floor + i) % capacity) as f32);
                let b = d.read(((floor + i + 1) % capacity) as f32);
                assert!((actual - (a * (1.0 - frac) + b * frac)).abs() < 1e-6);
            }
        }
    }
}

#[test]
fn biquad_block_rejects_invalid_input_atomically_and_recovers() {
    use aura_dsp::filter::{BiquadFilter, FilterType};
    let mut f = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.707).unwrap();
    let mut invalid = [1.0, f32::NAN, 1.0];
    f.process_buffer(&mut invalid);
    assert_eq!(invalid, [0.0; 3]);
    let mut fresh = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.707).unwrap();
    assert_eq!(f.process_sample(1.0), fresh.process_sample(1.0));
}

#[test]
fn fx_biquad_has_block_path() {
    let mut b = aura_dsp::fx::Biquad::new();
    b.set_identity();
    let mut x = [1.0, -0.5, 0.25];
    b.process_buffer(&mut x);
    assert_eq!(x, [1.0, -0.5, 0.25]);
}

#[test]
fn biquad_f32_blocks_track_f64_reference_including_low_cutoffs() {
    use aura_dsp::filter::{BiquadFilter, FilterType};
    for kind in [
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::BandPass,
        FilterType::Notch,
        FilterType::AllPass,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::Peak,
    ] {
        for rate in [48000.0, 96000.0] {
            for cutoff in [1.0, 10.0, 1000.0, 18000.0] {
                for q in [0.1, 0.707, 8.0] {
                    let mut f = BiquadFilter::with_gain(kind, rate, cutoff, q, 12.0).unwrap();
                    // Read normalized coefficients, then use an independent f64
                    // transposed-direct-form recurrence as the transfer reference.
                    let coefs = serde_json::to_value(&f).unwrap();
                    let b0 = coefs["b0"].as_f64().unwrap();
                    let b1 = coefs["b1"].as_f64().unwrap();
                    let b2 = coefs["b2"].as_f64().unwrap();
                    let a1 = coefs["a1"].as_f64().unwrap();
                    let a2 = coefs["a2"].as_f64().unwrap();
                    let mut z1 = 0.0;
                    let mut z2 = 0.0;
                    let input: Vec<f32> = (0..8192)
                        .map(|n| {
                            if n == 0 {
                                1.0
                            } else {
                                (n as f32 * 0.071).sin() * 0.25
                            }
                        })
                        .collect();
                    let mut actual = input.clone();
                    for chunk in actual.chunks_mut(63) {
                        f.process_buffer(chunk);
                    }
                    let mut max_error = 0.0_f64;
                    let mut peak = 0.0_f64;
                    for (&x, actual) in input.iter().zip(actual) {
                        let x = f64::from(x);
                        let y = b0 * x + z1;
                        z1 = b1 * x - a1 * y + z2;
                        z2 = b2 * x - a2 * y;
                        assert!(actual.is_finite());
                        max_error = max_error.max((f64::from(actual) - y).abs());
                        peak = peak.max(y.abs());
                    }
                    assert!(
                        max_error < 0.002 * (1.0 + peak),
                        "{kind:?} Fs={rate} cutoff={cutoff} Q={q}: error={max_error}, peak={peak}"
                    );
                }
            }
        }
    }
}

#[test]
fn biquad_block_partitions_and_deserialization_preserve_contract() {
    use aura_dsp::filter::{BiquadFilter, FilterType};
    let mut a = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.707).unwrap();
    let json = serde_json::to_string(&a).unwrap();
    let mut b: BiquadFilter = serde_json::from_str(&json).unwrap();
    let mut whole = [0.5; 257];
    let mut split = whole;
    a.process_buffer(&mut whole);
    for chunk in split.chunks_mut(3) {
        b.process_buffer(chunk);
    }
    assert_eq!(whole, split);
    a.process_buffer(&mut []);
    assert_eq!(a.process_sample(0.0), b.process_sample(0.0));
}

#[test]
#[should_panic(expected = "assertion")]
fn mix_rejects_length_mismatch() {
    mix_add(&mut [0.0; 2], &[1.0], 1.0);
}

#[test]
fn biquad_coefficient_changes_keep_output_history_bounded() {
    use aura_dsp::filter::{BiquadFilter, FilterType};
    let mut f = BiquadFilter::new(FilterType::HighPass, 48000.0, 10000.0, 0.707).unwrap();
    let mut x = [0.0; 64];
    x[63] = 1.0;
    f.process_buffer(&mut x);
    f.set_params(1.0, 0.707, 0.0).unwrap();
    x.fill(0.0);
    f.process_buffer(&mut x);
    assert!(x[0].abs() < 4.0, "first output={}", x[0]);
    let old = BiquadFilter::new(FilterType::HighPass, 48000.0, 10000.0, 0.707).unwrap();
    let old = serde_json::to_value(old).unwrap();
    let new = serde_json::to_value(&f).unwrap();
    let c = |v: &serde_json::Value, name: &str| v[name].as_f64().unwrap();
    let mut z1 = c(&old, "b1") - c(&old, "a1") * c(&old, "b0");
    let mut z2 = c(&old, "b2") - c(&old, "a2") * c(&old, "b0");
    for y in x {
        let expected = z1;
        z1 = -c(&new, "a1") * expected + z2;
        z2 = -c(&new, "a2") * expected;
        assert!((f64::from(y) - expected).abs() < 0.001);
    }
}

#[test]
fn fx_biquad_cancelled_poles_are_identity() {
    let mut f = aura_dsp::fx::Biquad::new();
    f.set_coefs(1.0, -2.0, 1.0, -2.0, 1.0);
    let mut x = [1.0, -0.5, 0.25, 0.0];
    f.process_buffer(&mut x);
    assert_eq!(x, [1.0, -0.5, 0.25, 0.0]);
}

#[test]
fn endpoint_biquads_remain_stable_at_dc_and_nyquist() {
    use aura_dsp::filter::{BiquadFilter, FilterType};
    for cutoff in [1.0, 23999.0, 23999.9] {
        let mut f = BiquadFilter::new(FilterType::LowPass, 48000.0, cutoff, 0.707).unwrap();
        let mut block = [0.0; 64];
        for _ in 0..32_768 {
            for (n, x) in block.iter_mut().enumerate() {
                *x = if cutoff < 10.0 || n % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
            }
            f.process_buffer(&mut block);
            assert!(block.iter().all(|x| x.is_finite() && x.abs() < 4.0));
        }
        let expected = if cutoff < 10.0 { 1.0 } else { 0.0 };
        assert!(
            (block[63] - expected).abs() < 0.005,
            "cutoff={cutoff} last={}",
            block[63]
        );
    }
}

#[test]
fn biquad_mirrored_state_switch_tracks_direct_form_history() {
    use aura_dsp::filter::{BiquadFilter, FilterType};
    let mut f = BiquadFilter::new(FilterType::HighPass, 48000.0, 1000.0, 0.707).unwrap();
    let (mut z1, mut z2) = (0.0_f64, 0.0_f64);
    for cutoff in [1000.0, 18000.0, 1000.0] {
        f.set_params(cutoff, 0.707, 0.0).unwrap();
        let coefs = serde_json::to_value(&f).unwrap();
        let c = |key: &str| coefs[key].as_f64().unwrap();
        let mut block: Vec<f32> = (0..511).map(|n| (n as f32 * 0.173).sin() * 0.25).collect();
        let input = block.clone();
        f.process_buffer(&mut block);
        for (x, actual) in input.into_iter().zip(block) {
            let x = f64::from(x);
            let y = c("b0") * x + z1;
            z1 = c("b1") * x - c("a1") * y + z2;
            z2 = c("b2") * x - c("a2") * y;
            assert!((f64::from(actual) - y).abs() < 0.0001);
        }
    }
}
