//! Reproducible assembly probes; see CODEGEN.md for build/check commands.
use aura_dsp::{
    dc_blocker::DcBlocker, delay::DelayLine, dsp_util::mix_add, filter::BiquadFilter,
    oversampling::Oversampler2xIir,
};
use std::hint::black_box;

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "Rust" fn probe_mix(out: &mut [f32], input: &[f32], gain: f32) {
    mix_add(out, input, gain);
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "Rust" fn probe_iir(os: &mut Oversampler2xIir, out: &mut [f32], gain: f32) {
    os.process_block_with(out, |high| {
        for x in high {
            *x *= gain;
        }
    });
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "Rust" fn probe_biquad(filter: &mut BiquadFilter, out: &mut [f32]) {
    filter.process_buffer(out);
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "Rust" fn probe_dc(dc: &mut DcBlocker, out: &mut [f32]) {
    dc.process_block(out);
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "Rust" fn probe_delay(delay: &DelayLine, out: &mut [f32], tap: f32) {
    delay.read_block(tap, out);
}

// Legacy baseline intentionally fails the hot-loop codegen check.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "Rust" fn probe_legacy_fir(
    os: &mut aura_dsp::oversampling::Oversampler2x,
    x: f32,
) -> f32 {
    os.process_sample(x, |x| x * 1.1)
}

fn main() {
    let mut x = [0.5; 256];
    let source = [0.25; 256];
    probe_mix(black_box(&mut x), black_box(&source), black_box(0.5));
    probe_iir(
        &mut Oversampler2xIir::new(),
        black_box(&mut x),
        black_box(1.1),
    );
    probe_dc(&mut DcBlocker::new(), black_box(&mut x));
    let mut b = BiquadFilter::new(
        aura_dsp::filter::FilterType::LowPass,
        48000.0,
        1000.0,
        0.707,
    )
    .unwrap();
    probe_biquad(&mut b, black_box(&mut x));
    probe_delay(&DelayLine::new(1024), black_box(&mut x), black_box(5.5));
    black_box(x);
}
