// FFT sizes/indices are small; conversion to the public f32 DSP API is intentional.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use aura_dsp::oversampling::Oversampler2x;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::f64::consts::TAU;

thread_local! {
    static ALLOCATOR_EVENTS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct CountingAllocator;

fn record_allocator_event() {
    let _ = ALLOCATOR_EVENTS.try_with(|events| {
        if let Some(count) = events.get() {
            events.set(Some(count + 1));
        }
    });
}

// SAFETY: Every allocation/deallocation is forwarded unchanged to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocator_event();
        // SAFETY: The caller supplies the valid GlobalAlloc layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_allocator_event();
        // SAFETY: The caller supplies a System-owned pointer and matching layout.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn identity_preserves_dc_and_runs_caller_at_double_rate() {
    let mut oversampler = Oversampler2x::new();
    let mut calls = 0;
    let mut output = 0.0_f32;
    for _ in 0..512 {
        output = oversampler.process_sample(0.75, |sample| {
            calls += 1;
            sample
        });
    }
    assert_eq!(calls, 1024);
    assert!((output - 0.75).abs() < 1e-6, "DC output: {output}");
}

#[test]
fn impulse_has_reported_latency_and_reset_clears_history() {
    let mut oversampler = Oversampler2x::new();
    let impulse: Vec<f32> = (0..256)
        .map(|n| oversampler.process_sample(if n == 0 { 1.0 } else { 0.0 }, |x| x))
        .collect();
    let peak = impulse
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    assert_eq!(peak, 63);
    assert_eq!(peak, Oversampler2x::LATENCY_SAMPLES);
    assert!((impulse.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    for offset in 1..=63 {
        assert!((impulse[63 - offset] - impulse[63 + offset]).abs() < 1e-7);
    }

    oversampler.process_sample(1.0, |x| x);
    oversampler.reset();
    for _ in 0..256 {
        assert_eq!(oversampler.process_sample(0.0, |x| x), 0.0);
    }
}

// Coherent FFT: no window/leakage; discard FIR startup before analysis.
// The highest forbidden baseband bin is an alias, not an intended harmonic.
const N: usize = 8192;

fn magnitudes(mut samples: Vec<f64>) -> Vec<f64> {
    let fft = realfft::RealFftPlanner::<f64>::new().plan_fft_forward(samples.len());
    let mut spectrum = fft.make_output_vec();
    fft.process(&mut samples, &mut spectrum).unwrap();
    let scale = 2.0 / samples.len() as f64;
    spectrum.iter().map(|z| z.norm() * scale).collect()
}

fn alias_dbc(spectrum: &[f64], tone_bin: usize) -> f64 {
    let largest_alias = spectrum[1..N / 2]
        .iter()
        .enumerate()
        .filter(|(i, _)| (i + 1) % tone_bin != 0)
        .map(|(_, amplitude)| *amplitude)
        .fold(0.0_f64, f64::max);
    20.0 * (largest_alias / spectrum[tone_bin]).max(1e-15).log10()
}

fn render(tone_bin: usize, drive: f32, factor: usize) -> Vec<f64> {
    let mut outer = Oversampler2x::new();
    let mut inner = Oversampler2x::new();
    (0..N * 2)
        .filter_map(|n| {
            let input = (TAU * tone_bin as f64 * n as f64 / N as f64).sin() as f32;
            let output = match factor {
                1 => (drive * input).tanh(),
                2 => outer.process_sample(input, |x| (drive * x).tanh()),
                4 => {
                    outer.process_sample(input, |x| inner.process_sample(x, |y| (drive * y).tanh()))
                }
                _ => unreachable!(),
            };
            (n >= N).then_some(f64::from(output))
        })
        .collect()
}

#[test]
fn driven_tanh_alias_is_measurably_lower_than_one_x() {
    // 48 kHz: 5841.796875 and 9978.515625 Hz. Input peak 1, drive 2/4.
    // Reference: analytic sine sampled directly at 192 kHz, tanh in f64,
    // then an ideal frequency-domain low-pass retaining only baseband bins.
    // This is independent of the production interpolation/decimation code.
    for tone_bin in [997, 1703] {
        for drive in [2.0_f32, 4.0] {
            let reference = magnitudes(
                (0..N * 4)
                    .map(|n| {
                        let sine = (TAU * tone_bin as f64 * n as f64 / (N * 4) as f64).sin();
                        (f64::from(drive) * sine).tanh()
                    })
                    .collect(),
            );
            let one = alias_dbc(&magnitudes(render(tone_bin, drive, 1)), tone_bin);
            let two = alias_dbc(&magnitudes(render(tone_bin, drive, 2)), tone_bin);
            let four = alias_dbc(&magnitudes(render(tone_bin, drive, 4)), tone_bin);
            let reference = alias_dbc(&reference, tone_bin);
            println!(
                "bin={tone_bin}, drive={drive}: peak alias dBc 1x={one:.2}, 2x={two:.2}, 4x={four:.2}, ideal-input 4x reference={reference:.2}"
            );
            assert!(two < one - 18.0, "2x improvement: {} dB", one - two);
            assert!(four < two - 18.0, "4x improvement: {} dB", two - four);
            assert!(
                four < (reference + 8.0).max(-85.0),
                "4x exceeds reference/filter floor"
            );
        }
    }
}

#[test]
fn identity_retains_passband_and_rolls_off_nyquist() {
    // The pair should retain 0..0.4 * base Fs with <0.02 dB ripple.
    let mut oversampler = Oversampler2x::new();
    let impulse = (0..N)
        .map(|n| f64::from(oversampler.process_sample(if n == 0 { 1.0 } else { 0.0 }, |x| x)))
        .collect();
    for (bin, magnitude) in magnitudes(impulse).iter().take(N * 2 / 5 + 1).enumerate() {
        let gain_db = 20.0 * (magnitude * N as f64 / 2.0).log10();
        assert!(gain_db.abs() < 0.02, "bin={bin}: {gain_db} dB");
    }
    let mut oversampler = Oversampler2x::new();
    let mut last = 0.0;
    for n in 0..512 {
        last = oversampler.process_sample(if n % 2 == 0 { 1.0 } else { -1.0 }, |x| x);
    }
    assert!((last.abs() - 0.5).abs() < 1e-6, "Nyquist gain: {last}");
}

#[test]
fn nonfinite_samples_do_not_poison_history() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        let mut oversampler = Oversampler2x::new();
        assert!(oversampler.process_sample(bad, |x| x).is_finite());
        assert!(oversampler.process_sample(0.0, |_| bad).is_finite());
        let mut last = 0.0_f32;
        for _ in 0..512 {
            last = oversampler.process_sample(0.0, |x| x);
            assert!(last.is_finite());
        }
        assert_eq!(last, 0.0);
    }
}

#[test]
fn process_and_reset_never_use_allocator() {
    let mut outer = Oversampler2x::new();
    let mut inner = Oversampler2x::new();
    ALLOCATOR_EVENTS.with(|events| events.set(Some(0)));
    for _ in 0..1024 {
        std::hint::black_box(
            outer.process_sample(0.5, |x| inner.process_sample(x, |y| (4.0 * y).tanh())),
        );
    }
    outer.reset();
    inner.reset();
    let count = ALLOCATOR_EVENTS.with(|events| events.replace(None).unwrap());
    assert_eq!(count, 0, "processing/reset used allocator");
}

#[test]
fn cascaded_four_x_runs_four_calls_and_has_fractional_latency() {
    let mut outer = Oversampler2x::new();
    let mut inner = Oversampler2x::new();
    let mut calls = 0;
    let mut sum = 0.0;
    let mut moment = 0.0;
    for n in 0..256 {
        let output = outer.process_sample(if n == 0 { 1.0 } else { 0.0 }, |x| {
            inner.process_sample(x, |y| {
                calls += 1;
                y
            })
        });
        sum += f64::from(output);
        moment += f64::from(n) * f64::from(output);
    }
    assert_eq!(calls, 1024);
    assert!((sum - 1.0).abs() < 1e-6);
    assert!((moment / sum - 94.5).abs() < 1e-5);
}

#[test]
fn interpolation_rejects_images_and_decimation_rejects_out_of_band_dsp() {
    let tone_bin = 3277; // ~0.4 * base Fs, near the passband edge.
    let mut oversampler = Oversampler2x::new();
    let mut interpolated = Vec::with_capacity(N * 2);
    for n in 0..N * 2 {
        let input = (TAU * tone_bin as f64 * n as f64 / N as f64).sin() as f32;
        oversampler.process_sample(input, |x| {
            if n >= N {
                interpolated.push(f64::from(x));
            }
            x
        });
    }
    let spectrum = magnitudes(interpolated);
    let image_db = 20.0 * (spectrum[N - tone_bin] / spectrum[tone_bin]).log10();
    assert!(image_db < -80.0, "interpolation image: {image_db} dBc");

    // Inject a high-rate DSP output beyond base Nyquist. Omitting the down
    // filter would fold this ~0.6 * base Fs tone back into the audible band.
    let high_bin = N - tone_bin;
    let mut phase = 0;
    let output: Vec<f64> = (0..N * 2)
        .filter_map(|n| {
            let y = oversampler.process_sample(0.0, |_| {
                let value = (TAU * high_bin as f64 * f64::from(phase) / (N * 2) as f64).sin();
                phase += 1;
                value as f32
            });
            (n >= N).then_some(f64::from(y))
        })
        .collect();
    let alias_db = 20.0 * magnitudes(output)[tone_bin].log10();
    assert!(alias_db < -80.0, "decimation alias: {alias_db} dBFS");
}

#[test]
fn filters_retarget_without_reset_or_allocation() {
    use aura_dsp::filter::{
        BiquadFilter, FilterType, MoogLadder, StateVariableFilter, SteinerParkerFilter,
    };
    let mut b = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.7).unwrap();
    let mut s = StateVariableFilter::new(1000.0, 0.7, 48000.0).unwrap();
    let mut m = MoogLadder::new(48000.0, 1000.0, 0.4).unwrap();
    let mut p = SteinerParkerFilter::new(48000.0, 1000.0, 0.4).unwrap();
    for _ in 0..100 {
        let _ = b.process_sample(0.5);
        let _ = s.process_sample(0.5);
        m.process_sample(0.5);
        p.process_sample(0.5, 0.0, 0.0);
    }
    ALLOCATOR_EVENTS.with(|events| events.set(Some(0)));
    for rate in [96000.0, 48000.0] {
        b.set_sample_rate(rate).unwrap();
        s.set_sample_rate(rate).unwrap();
        m.set_sample_rate(rate).unwrap();
        p.set_sample_rate(rate).unwrap();
        assert!(b.process_sample(0.0).abs() > 0.001);
        assert!(s.process_sample(0.0).low_pass.abs() > 0.001);
        assert!(m.process_sample(0.0).abs() > 0.001);
        assert!(p.process_sample(0.0, 0.0, 0.0).abs() > 0.001);
        for bad in [0.0, f32::NAN, f32::INFINITY, 2000.0] {
            assert!(b.set_sample_rate(bad).is_err());
            assert!(s.set_sample_rate(bad).is_err());
            assert!(m.set_sample_rate(bad).is_err());
            assert!(p.set_sample_rate(bad).is_err());
        }
    }
    let count = ALLOCATOR_EVENTS.with(|events| events.replace(None).unwrap());
    assert_eq!(count, 0);
    b.set_sample_rate(96000.0).unwrap();
    s.set_sample_rate(96000.0).unwrap();
    m.set_sample_rate(96000.0).unwrap();
    p.set_sample_rate(96000.0).unwrap();
    b.set_params(30000.0, 0.7, 0.0).unwrap();
    s.set_params(30000.0, 0.7).unwrap();
    m.set_cutoff(30000.0);
    p.set_params(30000.0, 0.4).unwrap();
    assert!(b.set_sample_rate(48000.0).is_err());
    assert!(s.set_sample_rate(48000.0).is_err());
    assert!(m.set_sample_rate(48000.0).is_err());
    assert!(p.set_sample_rate(48000.0).is_err());
}

#[test]
fn iir_converter_dc_block_reset_and_no_alloc() {
    use aura_dsp::oversampling::Oversampler2xIir;
    let mut a = Oversampler2xIir::new();
    let mut b = a.clone();
    let mut calls = 0;
    assert_eq!(
        a.process_sample(0.0, |x| {
            calls += 1;
            x
        }),
        0.0
    );
    assert_eq!(calls, 2);
    let mut block = [0.75; 2048];
    ALLOCATOR_EVENTS.with(|events| events.set(Some(0)));
    a.process_block(&mut block, |x| x);
    for y in block {
        assert_eq!(y, b.process_sample(0.75, |x| x));
    }
    assert!((b.process_sample(0.75, |x| x) - 0.75).abs() < 1e-5);
    a.reset();
    assert_eq!(a.process_sample(0.0, |x| x), 0.0);
    for bad in [f32::NAN, f32::INFINITY, f32::MAX] {
        assert!(a.process_sample(bad, |x| x).is_finite());
        assert!(a.process_sample(0.0, |_| bad).is_finite());
    }
    let count = ALLOCATOR_EVENTS.with(|events| events.replace(None).unwrap());
    assert_eq!(count, 0);
}

#[test]
fn iir_rejects_images_and_decimation_aliases() {
    use aura_dsp::oversampling::Oversampler2xIir;
    let tone_bin = 3277;
    let mut os = Oversampler2xIir::new();
    let mut up = Vec::new();
    for n in 0..2 * N {
        os.process_sample(
            (TAU * tone_bin as f64 * n as f64 / N as f64).sin() as f32,
            |x| {
                if n >= N {
                    up.push(f64::from(x));
                }
                x
            },
        );
    }
    let spectrum = magnitudes(up);
    let db = 20.0 * (spectrum[N - tone_bin] / spectrum[tone_bin]).log10();
    assert!(db < -60.0, "image {db}");
    let mut phase = 0;
    let out = (0..2 * N)
        .filter_map(|n| {
            let y = os.process_sample(0.0, |_| {
                let x =
                    (TAU * (N - tone_bin) as f64 * f64::from(phase) / (2 * N) as f64).sin() as f32;
                phase += 1;
                x
            });
            (n >= N).then_some(f64::from(y))
        })
        .collect();
    let db = 20.0 * magnitudes(out)[tone_bin].log10();
    assert!(db < -60.0, "alias {db}");
}

#[test]
fn iir_impulse_peak_matches_documented_latency() {
    use aura_dsp::oversampling::Oversampler2xIir;
    let mut os = Oversampler2xIir::new();
    let impulse: Vec<_> = (0..4096)
        .map(|n| os.process_sample(if n == 0 { 1.0 } else { 0.0 }, |x| x))
        .collect();
    let peak = impulse
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    assert_eq!(peak, Oversampler2xIir::LATENCY_SAMPLES);
    assert!((impulse.iter().sum::<f32>() - 1.0).abs() < 1e-5);
}

#[test]
fn retargeted_coefficients_match_new_filters_and_noop_preserves_history() {
    use aura_dsp::filter::{
        BiquadFilter, FilterType, MoogLadder, StateVariableFilter, SteinerParkerFilter,
    };
    macro_rules! check {
        ($make:expr, $step:expr) => {{
            let make = $make;
            let step = $step;
            let mut filter = make(48000.0);
            for rate in [96000.0, 48000.0] {
                filter.set_sample_rate(rate).unwrap();
                filter.reset();
                let mut reference = make(rate);
                for n in 0..512 {
                    let x = (n as f32 * 0.77).sin();
                    assert_eq!(step(&mut filter, x), step(&mut reference, x));
                }
                filter
                    .set_sample_rate(if rate < 96000.0 { 96000.0 } else { 48000.0 })
                    .unwrap();
                filter.set_sample_rate(rate).unwrap();
                filter.set_sample_rate(rate).unwrap();
                assert!(filter.set_sample_rate(f32::NAN).is_err());
                assert!(filter.set_sample_rate(1000.0).is_err());
                for _ in 0..16 {
                    assert_eq!(step(&mut filter, 0.0), step(&mut reference, 0.0));
                }
            }
        }};
    }
    check!(
        |rate| BiquadFilter::with_gain(FilterType::Peak, rate, 10000.0, 2.0, 6.0).unwrap(),
        |f: &mut BiquadFilter, x| f.process_sample(x)
    );
    check!(
        |rate| StateVariableFilter::new(10000.0, 2.0, rate).unwrap(),
        |f: &mut StateVariableFilter, x| f.process_sample(x).low_pass
    );
    check!(
        |rate| MoogLadder::new(rate, 10000.0, 0.7).unwrap(),
        |f: &mut MoogLadder, x| f.process_sample(x)
    );
    check!(
        |rate| SteinerParkerFilter::new(rate, 10000.0, 1.5).unwrap(),
        |f: &mut SteinerParkerFilter, x| f.process_sample(x, 0.0, 0.0)
    );
}
