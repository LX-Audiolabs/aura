//! Basic example: generate a 1-second 440 Hz sine wave and print the first 10 samples.

use naad::oscillator::{Oscillator, Waveform};

fn main() {
    let sample_rate = 44100.0;
    let frequency = 440.0;

    let mut osc = match Oscillator::new(Waveform::Sine, frequency, sample_rate) {
        Ok(osc) => osc,
        Err(e) => {
            eprintln!("Failed to create oscillator: {e}");
            return;
        }
    };

    println!("naad — 440 Hz Sine Wave");
    println!("Sample rate: {sample_rate} Hz");
    println!("Frequency: {frequency} Hz");
    println!();

    // Print first 10 samples
    println!("First 10 samples:");
    for i in 0..10 {
        let sample = osc.next_sample();
        println!("  [{i:2}] {sample:+.6}");
    }

    // Generate the rest of 1 second
    let remaining = (sample_rate as usize) - 10;
    let mut buffer = vec![0.0f32; remaining];
    osc.fill_buffer(&mut buffer);

    // Find peak amplitude
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

    println!();
    println!("Generated {:.0} samples (1 second)", sample_rate);
    println!("Peak amplitude: {peak:.6}");
}
