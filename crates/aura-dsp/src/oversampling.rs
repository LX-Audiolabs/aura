//! Caller-controlled oversampling around a complete nonlinear DSP path.
//!
//! [`Oversampler2x`] interpolates before invoking the caller and low-pass
//! filters before decimation. Use one instance per independent audio channel.
//! Construct off the audio thread; processing and reset use only fixed storage.
//! The caller's closure must also be realtime-safe.
//!
//! Configure every rate-dependent processor inside the closure at **2 * host
//! sample rate**, including filters, smoothing and modulation. Processing only
//! the final saturation cannot remove aliases already generated upstream.
//! Existing filter APIs do not change:
//!
//! ```
//! use aura_dsp::filter::SteinerParkerFilter;
//! use aura_dsp::oversampling::Oversampler2x;
//!
//! let host_rate = 48_000.0;
//! let mut oversampler = Oversampler2x::new();
//! let mut filter = SteinerParkerFilter::new(host_rate * 2.0, 1_000.0, 1.5)?;
//! let output = oversampler.process_sample(0.5, |x| filter.process_sample(x, 0.0, 0.0));
//! # Ok::<(), aura_dsp::DspError>(())
//! ```
//!
//! For 4x, nest two instances. The innermost closure runs at 4 * host rate;
//! conversion latency becomes 94.5 host samples (63 + 63/2), before caller DSP:
//!
//! ```
//! use aura_dsp::oversampling::Oversampler2x;
//! let mut outer = Oversampler2x::new();
//! let mut inner = Oversampler2x::new();
//! let output = outer.process_sample(0.5, |x| {
//!     inner.process_sample(x, |y| (4.0 * y).tanh())
//! });
//! ```
//!
//! The fixed 127-tap Blackman-windowed halfband FIR trades latency and CPU for
//! a linear phase response. The conversion pair retains 0..0.4 * host rate
//! within 0.02 dB; the transition ends at 0.6 * host rate. Content near host
//! Nyquist is attenuated, not perfectly reconstructed. Oversampling reduces
//! nonlinear aliasing; it does not guarantee its elimination at arbitrary drive.
//!
//! Polyphase background:
//! [Julius O. Smith, Filtering and Downsampling](https://www.dsprelated.com/freebooks/sasp/Filtering_Downsampling.html).

use std::f64::consts::{PI, TAU};

const PHASE_TAPS: usize = 64;
const MASK: usize = PHASE_TAPS - 1;

/// Fixed 2x mono interpolation / caller DSP / decimation.
///
/// Coefficients are prepared in [`Self::new`]. Processing allocates nothing,
/// takes no locks and makes exactly two chronological calls to the closure.
/// Input and closure output must be finite; nonfinite values and overflowed
/// conversions are replaced with zero before entering the delay lines.
#[derive(Clone, Debug)]
pub struct Oversampler2x {
    // Only the even FIR taps are nonzero, apart from the odd center tap 0.5.
    coefficients: [f64; PHASE_TAPS],
    input: [f32; PHASE_TAPS],
    down_even: [f32; PHASE_TAPS],
    down_odd: [f32; PHASE_TAPS],
    position: usize,
}

impl Oversampler2x {
    /// Conversion latency in input-rate samples, excluding caller DSP latency.
    pub const LATENCY_SAMPLES: usize = 63;

    /// Prepare halfband coefficients and zeroed history off the audio thread.
    pub fn new() -> Self {
        let mut coefficients = [0.0; PHASE_TAPS];
        for (k, coefficient) in coefficients.iter_mut().enumerate() {
            let index = (2 * k) as f64;
            let offset = index - 63.0;
            let phase = TAU * index / 126.0;
            let window = 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
            *coefficient = (PI * 0.5 * offset).sin() / (PI * offset) * window;
        }
        // Keep center = 0.5 and each interpolation phase at unity DC gain.
        let scale = 0.5 / coefficients.iter().sum::<f64>();
        for coefficient in &mut coefficients {
            *coefficient *= scale;
        }
        Self {
            coefficients,
            input: [0.0; PHASE_TAPS],
            down_even: [0.0; PHASE_TAPS],
            down_odd: [0.0; PHASE_TAPS],
            position: 0,
        }
    }

    /// Clear converter history; the caller resets its own DSP separately.
    pub fn reset(&mut self) {
        self.input.fill(0.0);
        self.down_even.fill(0.0);
        self.down_odd.fill(0.0);
        self.position = 0;
    }

    /// Process one input-rate sample through the closure at double rate.
    ///
    /// Both calls share the same closure and therefore the same caller state.
    /// Keep parameter updates and other work independent of buffer boundaries.
    #[inline]
    pub fn process_sample(&mut self, input: f32, mut process: impl FnMut(f32) -> f32) -> f32 {
        let position = self.position;
        self.input[position] = finite_sample(input);

        let mut even = 0.0;
        for (k, &coefficient) in self.coefficients.iter().enumerate() {
            even += coefficient * f64::from(self.input[position.wrapping_sub(k) & MASK]);
        }
        // The interpolation gain is 2; its odd phase is just the center tap.
        let up_even = finite_sample((2.0 * even) as f32);
        let up_odd = self.input[position.wrapping_sub(31) & MASK];
        self.down_even[position] = finite_sample(process(up_even));
        self.down_odd[position] = finite_sample(process(up_odd));

        // Retain the even decimation phase: odd center delay is 32 base samples.
        let mut output = 0.5 * f64::from(self.down_odd[position.wrapping_sub(32) & MASK]);
        for (k, &coefficient) in self.coefficients.iter().enumerate() {
            output += coefficient * f64::from(self.down_even[position.wrapping_sub(k) & MASK]);
        }
        self.position = (position + 1) & MASK;
        finite_sample(output as f32)
    }
}

impl Default for Oversampler2x {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn finite_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        crate::flush_denormal(sample)
    } else {
        0.0
    }
}
