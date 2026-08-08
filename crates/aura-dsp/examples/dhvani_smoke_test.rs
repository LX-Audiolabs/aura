//! dhvani integration smoke test.
//!
//! Demonstrates how dhvani (the sound engine) would compose naad primitives
//! into a playable instrument with voice allocation, modulation, and effects.
//!
//! This validates API ergonomics and proves the abstraction boundary works:
//! naad provides the DSP building blocks, dhvani wires them together.

use naad::dynamics::Compressor;
use naad::envelope::Adsr;
use naad::filter::{BiquadFilter, FilterType, StateVariableFilter};
use naad::mod_matrix::{ModDestination, ModMatrix, ModRouting, ModSource};
use naad::modulation::{Lfo, LfoShape};
use naad::oscillator::{UnisonOscillator, Waveform};
use naad::panning::{PanLaw, pan_mono};
use naad::reverb::Reverb;
use naad::smoothing::ParamSmoother;
use naad::voice::{PolyMode, StealMode, VoiceManager};

fn main() {
    let sample_rate = 44100.0;
    let buffer_size = 512;

    // --- Voice Manager (dhvani allocates voices) ---
    let mut voice_mgr = VoiceManager::new(8, PolyMode::Poly, StealMode::Oldest);

    // --- Per-voice synthesis chain (naad provides all primitives) ---
    // In a real dhvani implementation, these would be per-voice arrays.
    let mut osc = UnisonOscillator::new(Waveform::Saw, 440.0, 4, 15.0, sample_rate).unwrap();
    let mut filter = StateVariableFilter::new(2000.0, 2.0, sample_rate).unwrap();
    let mut amp_env = Adsr::with_sample_rate(0.01, 0.1, 0.7, 0.3, sample_rate).unwrap();
    let mut filter_env = Adsr::with_sample_rate(0.005, 0.2, 0.4, 0.5, sample_rate).unwrap();

    // --- Modulation (LFO → filter cutoff via mod matrix) ---
    let mut lfo = Lfo::new(LfoShape::Triangle, 3.0, sample_rate).unwrap();
    let mut mod_matrix = ModMatrix::new();
    mod_matrix.add_routing(ModRouting::new(
        ModSource::Lfo1,
        ModDestination::FilterCutoff,
        0.3,
    ));
    mod_matrix.add_routing(ModRouting::new(
        ModSource::FilterEnvelope,
        ModDestination::FilterCutoff,
        0.8,
    ));

    // --- Master bus effects ---
    let mut reverb = Reverb::new(0.7, 0.4, 15.0, 0.25, sample_rate).unwrap();
    let mut compressor = Compressor::new(-12.0, 3.0, 0.01, 0.1, sample_rate);
    let mut master_eq =
        BiquadFilter::with_gain(FilterType::HighShelf, sample_rate, 8000.0, 0.707, -2.0).unwrap();
    let mut gain_smoother = ParamSmoother::new(0.01, sample_rate, 1.0);

    // --- Simulate: note on, render, note off, render tail ---

    // Note on
    let _voice_idx = voice_mgr.note_on(60, 0.8); // Middle C
    amp_env.gate_on();
    filter_env.gate_on();

    // Render 2 blocks of audio
    let mut stereo_left = vec![0.0f32; buffer_size];
    let mut stereo_right = vec![0.0f32; buffer_size];

    for block in 0..2 {
        for i in 0..buffer_size {
            // --- Per-sample modulation ---
            let lfo_val = lfo.next_value();
            let fenv_val = filter_env.next_value();
            mod_matrix.set_source(ModSource::Lfo1, lfo_val);
            mod_matrix.set_source(ModSource::FilterEnvelope, fenv_val);
            mod_matrix.compute();

            let cutoff_mod = mod_matrix.get_destination(ModDestination::FilterCutoff);
            let modulated_cutoff = (2000.0 * (cutoff_mod * 2.0).exp2()).clamp(20.0, 20000.0);
            let _ = filter.set_params(modulated_cutoff, 2.0);

            // --- Synthesis chain ---
            let osc_out = osc.next_sample();
            let filtered = filter.process_sample(osc_out).low_pass;
            let amp = amp_env.next_value();
            let voice_out = filtered * amp;

            // --- Gain smoothing ---
            let gain = gain_smoother.next_value();
            let gained = voice_out * gain;

            // --- Panning ---
            let (l, _r) = pan_mono(gained, 0.0, PanLaw::EqualPower);

            // --- Master effects ---
            let (rev_l, rev_r) = reverb.process_sample(l);
            stereo_left[i] = master_eq.process_sample(compressor.process_sample(rev_l));
            stereo_right[i] = master_eq.process_sample(compressor.process_sample(rev_r));
        }

        // Tick voice ages
        for _ in 0..buffer_size {
            voice_mgr.tick();
        }

        let peak_l: f32 = stereo_left.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let peak_r: f32 = stereo_right.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        println!(
            "Block {block}: peak L={peak_l:.4} R={peak_r:.4}, voices={}",
            voice_mgr.active_count()
        );
    }

    // Note off
    amp_env.gate_off();
    filter_env.gate_off();
    voice_mgr.note_off(60);

    // Render release tail
    for block in 2..6 {
        for i in 0..buffer_size {
            let osc_out = osc.next_sample();
            let filtered = filter.process_sample(osc_out).low_pass;
            let amp = amp_env.next_value();
            let voice_out = filtered * amp;
            let (l, _r) = pan_mono(voice_out, 0.0, PanLaw::EqualPower);
            let (rev_l, rev_r) = reverb.process_sample(l);
            stereo_left[i] = compressor.process_sample(rev_l);
            stereo_right[i] = compressor.process_sample(rev_r);
        }

        let peak_l: f32 = stereo_left.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        println!(
            "Block {block}: peak L={peak_l:.4}, active={}, env_active={}",
            voice_mgr.active_count(),
            amp_env.is_active()
        );
    }

    println!("\nSmoke test passed — all naad primitives compose cleanly for dhvani.");
}
