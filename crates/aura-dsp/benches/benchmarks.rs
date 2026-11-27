//! Criterion benchmarks for aura-dsp synthesis primitives.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use aura_dsp::delay::{AllpassDelay, CombFilter};
use aura_dsp::effects::{Chorus, Distortion, DistortionType, Phaser};
use aura_dsp::envelope::Adsr;
use aura_dsp::filter::{BiquadFilter, FilterType, StateVariableFilter};
use aura_dsp::modulation::FmModulator;
use aura_dsp::noise::{NoiseGenerator, NoiseType};
use aura_dsp::oscillator::{Oscillator, Waveform};
use aura_dsp::wavetable::{Wavetable, WavetableOscillator};

fn oscillator_sine_1024(c: &mut Criterion) {
    c.bench_function("oscillator_sine_1024", |b| {
        let mut osc = Oscillator::new(Waveform::Sine, 440.0, 44100.0).unwrap();
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            osc.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn oscillator_saw_polyblep_1024(c: &mut Criterion) {
    c.bench_function("oscillator_saw_polyblep_1024", |b| {
        let mut osc = Oscillator::new(Waveform::Saw, 440.0, 44100.0).unwrap();
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            osc.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn biquad_filter_1024(c: &mut Criterion) {
    c.bench_function("biquad_filter_1024", |b| {
        let mut filter = BiquadFilter::new(FilterType::LowPass, 44100.0, 1000.0, 0.707).unwrap();
        let mut buffer = [0.5f32; 1024];
        b.iter(|| {
            filter.process_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn svf_filter_1024(c: &mut Criterion) {
    c.bench_function("svf_filter_1024", |b| {
        let mut svf = StateVariableFilter::new(1000.0, 0.707, 44100.0).unwrap();
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(svf.process_sample(s));
            }
        });
    });
}

fn adsr_envelope_1024(c: &mut Criterion) {
    c.bench_function("adsr_envelope_1024", |b| {
        let mut env = Adsr::new(0.01, 0.1, 0.7, 0.3).unwrap();
        b.iter(|| {
            env.gate_on();
            for _ in 0..1024 {
                black_box(env.next_value());
            }
        });
    });
}

fn fm_synthesis_1024(c: &mut Criterion) {
    c.bench_function("fm_synthesis_1024", |b| {
        let mut fm = FmModulator::new(440.0, 220.0, 2.0, 44100.0).unwrap();
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            fm.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn wavetable_1024(c: &mut Criterion) {
    c.bench_function("wavetable_1024", |b| {
        let table =
            Wavetable::from_harmonics(8, &[1.0, 0.5, 0.33, 0.25, 0.2, 0.167, 0.143, 0.125], 2048)
                .unwrap();
        let mut osc = WavetableOscillator::new(table, 440.0, 44100.0).unwrap();
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            osc.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn noise_white_1024(c: &mut Criterion) {
    c.bench_function("noise_white_1024", |b| {
        let mut ng = NoiseGenerator::new(NoiseType::White, 42);
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            ng.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn noise_pink_1024(c: &mut Criterion) {
    c.bench_function("noise_pink_1024", |b| {
        let mut ng = NoiseGenerator::new(NoiseType::Pink, 42);
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            ng.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

fn comb_filter_1024(c: &mut Criterion) {
    c.bench_function("comb_filter_1024", |b| {
        let mut comb = CombFilter::new(441, 0.7);
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(comb.process_sample(s));
            }
        });
    });
}

fn allpass_delay_1024(c: &mut Criterion) {
    c.bench_function("allpass_delay_1024", |b| {
        let mut ap = AllpassDelay::new(441, 0.7);
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(ap.process_sample(s));
            }
        });
    });
}

fn chorus_1024(c: &mut Criterion) {
    c.bench_function("chorus_1024", |b| {
        let mut chorus = Chorus::new(3, 0.5, 10.0, 2.0, 0.5, 44100.0).unwrap();
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(chorus.process_sample(s));
            }
        });
    });
}

fn phaser_1024(c: &mut Criterion) {
    c.bench_function("phaser_1024", |b| {
        let mut phaser = Phaser::new(6, 0.5, 200.0, 2000.0, 0.7, 0.5, 44100.0).unwrap();
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(phaser.process_sample(s));
            }
        });
    });
}

fn distortion_wavefold_1024(c: &mut Criterion) {
    c.bench_function("distortion_wavefold_1024", |b| {
        let dist = Distortion::new(DistortionType::WaveFold, 5.0, 1.0);
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(dist.process_sample(s));
            }
        });
    });
}

fn compressor_1024(c: &mut Criterion) {
    c.bench_function("compressor_1024", |b| {
        let mut comp = aura_dsp::dynamics::Compressor::new(-20.0, 4.0, 0.01, 0.1, 44100.0);
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(comp.process_sample(s));
            }
        });
    });
}

fn reverb_1024(c: &mut Criterion) {
    c.bench_function("reverb_1024", |b| {
        let mut rev = aura_dsp::reverb::Reverb::new(0.8, 0.3, 10.0, 1.0, 44100.0).unwrap();
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(rev.process_sample(s));
            }
        });
    });
}

fn parametric_eq_1024(c: &mut Criterion) {
    c.bench_function("parametric_eq_4band_1024", |b| {
        let mut eq = aura_dsp::eq::ParametricEq::new(44100.0);
        eq.add_band(FilterType::Peak, 250.0, 1.0, 3.0).unwrap();
        eq.add_band(FilterType::Peak, 1000.0, 1.0, -2.0).unwrap();
        eq.add_band(FilterType::Peak, 4000.0, 1.0, 1.5).unwrap();
        eq.add_band(FilterType::HighShelf, 8000.0, 0.707, -1.0)
            .unwrap();
        let buffer = [0.5f32; 1024];
        b.iter(|| {
            for &s in black_box(&buffer) {
                black_box(eq.process_sample(s));
            }
        });
    });
}

#[cfg(feature = "synthesis")]
fn subtractive_synth_1024(c: &mut Criterion) {
    c.bench_function("subtractive_synth_1024", |b| {
        let mut synth = aura_dsp::synth::subtractive::SubtractiveSynth::new(
            aura_dsp::oscillator::Waveform::Saw,
            440.0,
            2000.0,
            0.707,
            44100.0,
        )
        .unwrap();
        synth.note_on();
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            synth.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

#[cfg(feature = "synthesis")]
fn karplus_strong_1024(c: &mut Criterion) {
    c.bench_function("karplus_strong_1024", |b| {
        let mut ks =
            aura_dsp::synth::physical::KarplusStrong::new(440.0, 0.99, 0.5, 44100.0).unwrap();
        ks.pluck();
        let mut buffer = [0.0f32; 1024];
        b.iter(|| {
            ks.fill_buffer(black_box(&mut buffer));
            black_box(&buffer);
        });
    });
}

// Keep converter and biquad comparisons together under identical input/FTZ settings.
#[allow(clippy::too_many_lines)]
fn block_first(c: &mut Criterion) {
    use aura_dsp::oversampling::{Oversampler2x, Oversampler2xIir};
    let _denormals = aura_dsp::fx::FtzDazGuard::new();
    let source: Vec<f32> = (0..256_u16)
        .map(|n| (f32::from(n) * 0.17).sin() * 0.5)
        .collect();
    let mut group = c.benchmark_group("block_first");
    group.throughput(criterion::Throughput::Elements(256));
    group.bench_function("fir_legacy_sample", |b| {
        let mut os = Oversampler2x::new();
        let mut out = [0.0; 256];
        b.iter(|| {
            for (out, &x) in out.iter_mut().zip(black_box(&source)) {
                *out = os.process_sample(x, |x| x.clamp(-0.4, 0.4));
            }
            black_box(&out);
        });
    });
    group.bench_function("iir_legacy_sample", |b| {
        let mut os = Oversampler2xIir::new();
        let mut out = [0.0; 256];
        b.iter(|| {
            for (out, &x) in out.iter_mut().zip(black_box(&source)) {
                *out = os.process_sample(x, |x| x.clamp(-0.4, 0.4));
            }
            black_box(&out);
        });
    });
    group.bench_function("iir_block_callback", |b| {
        let mut os = Oversampler2xIir::new();
        let mut out = [0.0; 256];
        b.iter(|| {
            out.copy_from_slice(black_box(&source));
            os.process_block_with(&mut out, |high| {
                for x in high {
                    *x = x.clamp(-0.4, 0.4);
                }
            });
            black_box(&out);
        });
    });
    group.bench_function("biquad_legacy_f64", |b| {
        // Previous TDF-II tick, including its per-sample guards, as baseline.
        let f = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.707).unwrap();
        let coefs = serde_json::to_value(f).unwrap();
        let coefficient = |key: &str| coefs[key].as_f64().unwrap();
        let (b0, b1, b2, a1, a2) = (
            coefficient("b0"),
            coefficient("b1"),
            coefficient("b2"),
            coefficient("a1"),
            coefficient("a2"),
        );
        let (mut z1, mut z2) = (0.0_f64, 0.0_f64);
        let mut out = [0.0; 256];
        b.iter(|| {
            for (out, &x) in out.iter_mut().zip(black_box(&source)) {
                if !x.is_finite() {
                    *out = 0.0;
                    continue;
                }
                let x = f64::from(x);
                let y = b0 * x + z1;
                z1 = b1 * x - a1 * y + z2;
                z2 = b2 * x - a2 * y;
                if z1.abs() < f64::MIN_POSITIVE {
                    z1 = 0.0;
                }
                if z2.abs() < f64::MIN_POSITIVE {
                    z2 = 0.0;
                }
                // Values are intentionally narrowed to the audio API's f32.
                #[allow(clippy::cast_possible_truncation)]
                let y = y as f32;
                if y.is_finite() && z1.is_finite() && z2.is_finite() {
                    *out = y;
                } else {
                    z1 = 0.0;
                    z2 = 0.0;
                    *out = 0.0;
                }
            }
            black_box(&out);
        });
    });
    group.bench_function("biquad_sample_wrapper", |b| {
        let mut f = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.707).unwrap();
        let mut out = [0.0; 256];
        b.iter(|| {
            for (out, &x) in out.iter_mut().zip(black_box(&source)) {
                *out = f.process_sample(x);
            }
            black_box(&out);
        });
    });
    group.bench_function("biquad_block", |b| {
        let mut f = BiquadFilter::new(FilterType::LowPass, 48000.0, 1000.0, 0.707).unwrap();
        let mut out = [0.0; 256];
        b.iter(|| {
            out.copy_from_slice(black_box(&source));
            f.process_buffer(&mut out);
            black_box(&out);
        });
    });
    group.finish();
}

criterion_group!(
    core_benches,
    block_first,
    oscillator_sine_1024,
    oscillator_saw_polyblep_1024,
    biquad_filter_1024,
    svf_filter_1024,
    adsr_envelope_1024,
    fm_synthesis_1024,
    wavetable_1024,
    noise_white_1024,
    noise_pink_1024,
    comb_filter_1024,
    allpass_delay_1024,
    chorus_1024,
    phaser_1024,
    distortion_wavefold_1024,
    compressor_1024,
    reverb_1024,
    parametric_eq_1024,
);

#[cfg(feature = "synthesis")]
criterion_group!(synth_benches, subtractive_synth_1024, karplus_strong_1024,);

#[cfg(feature = "synthesis")]
criterion_main!(core_benches, synth_benches);

#[cfg(not(feature = "synthesis"))]
criterion_main!(core_benches);
