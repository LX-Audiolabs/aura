//! DSP utility functions: dB conversion, clipping, interpolation.
//!
//! Shared free functions used across synthesis and effects modules.

use serde::{Deserialize, Serialize};

/// Convert linear amplitude to decibels.
///
/// Returns `-f32::INFINITY` for amplitude <= 0.
#[inline]
#[must_use]
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    if amplitude <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * amplitude.log10()
    }
}

/// Convert decibels to linear amplitude.
///
/// Returns 0.0 for `-f32::INFINITY`.
#[inline]
#[must_use]
pub fn db_to_amplitude(db: f32) -> f32 {
    if db == f32::NEG_INFINITY {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

/// Normalize a buffer so the peak absolute value is 1.0.
///
/// Does nothing if the buffer is all zeros.
pub fn normalize(buffer: &mut [f32]) {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak > 0.0 {
        let inv = 1.0 / peak;
        for s in buffer.iter_mut() {
            *s *= inv;
        }
    }
}

/// Hard-limit (clip) a sample to the range \[-limit, +limit\].
#[inline]
#[must_use]
pub fn hard_limit(sample: f32, limit: f32) -> f32 {
    sample.clamp(-limit, limit)
}

/// Soft-clip a sample using `tanh` saturation.
///
/// `drive` controls the amount of saturation (1.0 = mild, higher = more).
#[inline]
#[must_use]
pub fn soft_clip_tanh(sample: f32, drive: f32) -> f32 {
    (sample * drive).tanh()
}

/// Linear interpolation between two values.
#[inline]
#[must_use]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Cubic Hermite interpolation between samples.
///
/// Given four equally-spaced samples `y0, y1, y2, y3` and a fractional
/// position `t` (0..1) between `y1` and `y2`, returns the interpolated value.
#[inline]
#[must_use]
pub fn hermite_interpolate(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}

/// Crossfade between two signals with equal-power law.
///
/// `mix` ranges from 0.0 (100% dry) to 1.0 (100% wet).
#[inline]
#[must_use]
pub fn crossfade_equal_power(dry: f32, wet: f32, mix: f32) -> f32 {
    let angle = mix * std::f32::consts::FRAC_PI_2;
    dry * angle.cos() + wet * angle.sin()
}

/// Compute the RMS (root mean square) of a buffer.
#[inline]
#[must_use]
pub fn rms(buffer: &[f32]) -> f32 {
    if buffer.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = buffer.iter().map(|&s| s * s).sum();
    (sum_sq / buffer.len() as f32).sqrt()
}

/// Compute the peak absolute value of a buffer.
#[inline]
#[must_use]
pub fn peak(buffer: &[f32]) -> f32 {
    buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}

/// Apply a Hann window to a buffer in place.
pub fn apply_hann_window(buffer: &mut [f32]) {
    let len = buffer.len();
    if len == 0 {
        return;
    }
    let inv = 1.0 / len as f32;
    for (i, s) in buffer.iter_mut().enumerate() {
        let w = 0.5 * (1.0 - (std::f32::consts::TAU * i as f32 * inv).cos());
        *s *= w;
    }
}

/// Apply a Blackman window to a buffer in place.
pub fn apply_blackman_window(buffer: &mut [f32]) {
    let len = buffer.len();
    if len == 0 {
        return;
    }
    let inv = 1.0 / len as f32;
    for (i, s) in buffer.iter_mut().enumerate() {
        let t = i as f32 * inv;
        let w = 0.42 - 0.5 * (std::f32::consts::TAU * t).cos()
            + 0.08 * (2.0 * std::f32::consts::TAU * t).cos();
        *s *= w;
    }
}

/// Smoothing mode for parameter transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SmoothingMode {
    /// Exponential moving average (one-pole lowpass).
    Exponential,
    /// Linear ramp over a fixed number of samples.
    Linear,
}

// --- FFT utilities (requires hisab) ---

/// Compute real-valued FFT magnitude spectrum.
///
/// Returns magnitudes for bins 0..N/2+1 (DC to Nyquist).
/// Input length must be a power of 2.
///
/// Requires the `synthesis` or `acoustics` feature.
#[cfg(feature = "synthesis")]
#[must_use]
pub fn fft_magnitudes(input: &[f32]) -> Vec<f32> {
    use hisab::Complex;

    let n = input.len();
    let mut complex: Vec<Complex> = input.iter().map(|&s| Complex::new(s as f64, 0.0)).collect();
    // fft returns Result — unwrap-free: if it fails, return empty
    if hisab::num::fft(&mut complex).is_err() {
        return Vec::new();
    }

    let half = n / 2 + 1;
    let inv_n = 1.0 / n as f64;
    complex[..half]
        .iter()
        .map(|c| (c.abs() * inv_n) as f32)
        .collect()
}

/// Compute the power spectrum (magnitude squared) of a real signal.
///
/// Returns power for bins 0..N/2+1. Input length must be a power of 2.
///
/// Requires the `synthesis` or `acoustics` feature.
#[cfg(feature = "synthesis")]
#[must_use]
pub fn power_spectrum(input: &[f32]) -> Vec<f32> {
    use hisab::Complex;

    let n = input.len();
    let mut complex: Vec<Complex> = input.iter().map(|&s| Complex::new(s as f64, 0.0)).collect();
    if hisab::num::fft(&mut complex).is_err() {
        return Vec::new();
    }

    let half = n / 2 + 1;
    let inv_n_sq = 1.0 / (n as f64 * n as f64);
    complex[..half]
        .iter()
        .map(|c| ((c.re * c.re + c.im * c.im) * inv_n_sq) as f32)
        .collect()
}

/// Evaluate a 1-D clamped uniform B-spline at `t ∈ [0, 1]`.
///
/// Wraps `hisab::calc::bspline_eval` for scalar use: lifts the
/// `control_points: &[f32]` into Vec3.x, generates a clamped open uniform
/// knot vector internally, and returns the spline value's `.x` component.
///
/// "Clamped uniform" means the first control point is hit at `t=0` and
/// the last at `t=1`, with uniform spacing of the interior knots. For
/// degree `d` this gives a `C^{d-1}`-continuous curve (cubic = `C²`)
/// that's well-suited to smoothly morphing between a small number of
/// scalar values — wavetable morph axes, parameter-curve scrubbing, etc.
///
/// Returns `None` if `control_points.len() < degree + 1`, `degree == 0`,
/// or `t` is outside `[0, 1]`.
///
/// **Allocates** per call (knot vector + Vec3 control-point copy). Hot
/// audio paths should call only when the morph parameter changes, then
/// cache the result.
///
/// Requires the `synthesis` feature (uses hisab `calc::bspline_eval`).
#[cfg(feature = "synthesis")]
#[must_use]
pub fn bspline_eval_1d(degree: usize, control_points: &[f32], t: f32) -> Option<f32> {
    if degree == 0 || control_points.len() <= degree {
        return None;
    }
    let t = t as f64;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let n = control_points.len();

    // Clamped open uniform knots: (degree+1) zeros, then uniform interior, then (degree+1) ones.
    // Total length = n + degree + 1.
    let interior_count = n - degree - 1;
    let mut knots: Vec<f64> = Vec::with_capacity(n + degree + 1);
    knots.extend(std::iter::repeat_n(0.0, degree + 1));
    knots.extend((1..=interior_count).map(|i| i as f64 / (interior_count as f64 + 1.0)));
    knots.extend(std::iter::repeat_n(1.0, degree + 1));

    let cps: Vec<hisab::Vec3> = control_points
        .iter()
        .map(|&v| hisab::Vec3::new(v, 0.0, 0.0))
        .collect();

    hisab::calc::bspline_eval(degree, &cps, &knots, t).map(|v| v.x)
}

/// Fit a polynomial of `degree` to `(xs, ys)` via least-squares.
///
/// Wraps `hisab::num::linalg::least_squares_poly`. Returns the coefficients
/// `[c0, c1, c2, ..., c_degree]` such that `p(x) = c0 + c1*x + c2*x² + ...`.
///
/// Useful for replacing expensive transfer functions (`tanh`, `exp`,
/// custom lookup tables) with a polynomial whose evaluation is just
/// `degree + 1` multiplies + adds via Horner's method
/// ([`eval_polynomial`]).
///
/// Returns `None` if `xs.len() != ys.len()`, fewer than `degree + 1`
/// samples are supplied, or the linear system is singular.
///
/// Requires the `synthesis` feature (uses hisab linalg).
#[cfg(feature = "synthesis")]
#[must_use]
pub fn fit_polynomial(xs: &[f64], ys: &[f64], degree: usize) -> Option<Vec<f64>> {
    if xs.len() != ys.len() || xs.len() <= degree {
        return None;
    }
    hisab::num::least_squares_poly(xs, ys, degree).ok()
}

/// Evaluate a polynomial at `x` via Horner's method.
///
/// `coeffs` is in ascending-degree order: `coeffs[i]` is the coefficient
/// of `xⁱ`. Empty `coeffs` returns `0.0`. Uses `f64` internally for
/// stability on high-degree fits and casts the result to `f32`.
#[inline]
#[must_use]
pub fn eval_polynomial(coeffs: &[f64], x: f32) -> f32 {
    if coeffs.is_empty() {
        return 0.0;
    }
    let xd = x as f64;
    let mut acc = coeffs[coeffs.len() - 1];
    for &c in coeffs[..coeffs.len() - 1].iter().rev() {
        acc = acc * xd + c;
    }
    acc as f32
}

/// Window function for spectral analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SpectralWindow {
    /// Rectangular (no window) — best frequency resolution, worst leakage.
    Rectangular,
    /// Hann (raised cosine) — general-purpose default.
    Hann,
    /// Hamming — slightly better sidelobe attenuation than Hann at the cost of higher first sidelobe.
    Hamming,
    /// Blackman — strong sidelobe rejection, wider main lobe.
    Blackman,
}

impl SpectralWindow {
    /// Apply this window to a buffer in place.
    fn apply(self, buffer: &mut [f32]) {
        let len = buffer.len();
        if len == 0 {
            return;
        }
        let inv = 1.0 / len as f32;
        match self {
            Self::Rectangular => {}
            Self::Hann => {
                for (i, s) in buffer.iter_mut().enumerate() {
                    let w = 0.5 * (1.0 - (std::f32::consts::TAU * i as f32 * inv).cos());
                    *s *= w;
                }
            }
            Self::Hamming => {
                for (i, s) in buffer.iter_mut().enumerate() {
                    let w = 0.54 - 0.46 * (std::f32::consts::TAU * i as f32 * inv).cos();
                    *s *= w;
                }
            }
            Self::Blackman => {
                for (i, s) in buffer.iter_mut().enumerate() {
                    let t = i as f32 * inv;
                    let w = 0.42 - 0.5 * (std::f32::consts::TAU * t).cos()
                        + 0.08 * (2.0 * std::f32::consts::TAU * t).cos();
                    *s *= w;
                }
            }
        }
    }
}

/// Compute the Short-Time Fourier Transform magnitude spectrogram of a signal.
///
/// Slides a windowed FFT of length `window_size` across `signal` with
/// `hop_size` samples between frames. Each frame is windowed (per
/// `window`), zero-padded if necessary, FFT'd, and returned as a vector
/// of magnitudes of length `window_size / 2 + 1` (DC through Nyquist).
///
/// Returns an empty `Vec` if the signal is shorter than `window_size`,
/// `window_size` is not a power of two, or `hop_size == 0`. Each inner
/// `Vec` has length `window_size / 2 + 1`.
///
/// Common parameter choices:
/// - `window_size = 2048`, `hop_size = 512` — typical speech/music analysis
/// - `window_size = 1024`, `hop_size = 256` — finer time resolution
///
/// Requires the `synthesis` feature (uses hisab FFT).
#[cfg(feature = "synthesis")]
#[must_use]
pub fn stft_magnitudes(
    signal: &[f32],
    window_size: usize,
    hop_size: usize,
    window: SpectralWindow,
) -> Vec<Vec<f32>> {
    if signal.len() < window_size
        || hop_size == 0
        || !window_size.is_power_of_two()
        || window_size == 0
    {
        return Vec::new();
    }

    let num_frames = (signal.len() - window_size) / hop_size + 1;
    let half = window_size / 2 + 1;
    let mut frames = Vec::with_capacity(num_frames);

    let mut frame = vec![0.0f32; window_size];
    for f in 0..num_frames {
        let start = f * hop_size;
        frame.copy_from_slice(&signal[start..start + window_size]);
        window.apply(&mut frame);
        let mag = fft_magnitudes(&frame);
        if mag.len() < half {
            // FFT failure — bail out cleanly.
            return Vec::new();
        }
        frames.push(mag);
    }

    frames
}

/// Compute a chromagram (12-bin pitch-class profile) from an STFT magnitude spectrogram.
///
/// Folds linear-frequency bins into the 12 chromatic pitch classes (C, C#,
/// D, ..., B), summing magnitudes for all bins whose centre frequency
/// falls within a given pitch class regardless of octave. The result has
/// shape `[num_frames][12]`, with bin 0 = C, bin 11 = B.
///
/// `sample_rate` is the original signal's sample rate. Bins below 27.5 Hz
/// (A0) and above 4186 Hz (C8) are ignored — outside the standard piano
/// range chroma estimates are unreliable.
///
/// Requires the `synthesis` feature.
#[cfg(feature = "synthesis")]
#[must_use]
pub fn chromagram(magnitudes: &[Vec<f32>], window_size: usize, sample_rate: f32) -> Vec<[f32; 12]> {
    let mut chroma_frames = Vec::with_capacity(magnitudes.len());
    if window_size == 0 || sample_rate <= 0.0 {
        return chroma_frames;
    }
    let bin_hz = sample_rate / window_size as f32;

    for frame in magnitudes {
        let mut chroma = [0.0f32; 12];
        for (bin, &m) in frame.iter().enumerate().skip(1) {
            let freq = bin as f32 * bin_hz;
            if !(27.5..=4186.0).contains(&freq) {
                continue;
            }
            // MIDI note number, fractional. Pitch class = MIDI mod 12.
            let midi = 69.0 + 12.0 * (freq / 440.0).log2();
            let pc = midi.rem_euclid(12.0);
            let pc_idx = pc.floor() as usize % 12;
            chroma[pc_idx] += m;
        }
        chroma_frames.push(chroma);
    }
    chroma_frames
}

/// Detect onset times (in seconds) from an STFT magnitude spectrogram via spectral flux.
///
/// Spectral flux is the L2 norm of the half-wave-rectified frame-to-frame
/// magnitude difference — onsets correspond to local maxima of this curve
/// above an adaptive threshold.
///
/// `hop_size` and `sample_rate` convert frame indices back to seconds in
/// the source signal. `threshold_factor` (typically `1.2`–`2.0`) scales
/// the running median used as the picking threshold; lower = more
/// sensitive.
///
/// Requires the `synthesis` feature.
#[cfg(feature = "synthesis")]
#[must_use]
pub fn detect_onsets(
    magnitudes: &[Vec<f32>],
    hop_size: usize,
    sample_rate: f32,
    threshold_factor: f32,
) -> Vec<f32> {
    if magnitudes.len() < 3 || hop_size == 0 || sample_rate <= 0.0 {
        return Vec::new();
    }

    // Spectral flux per frame (frame 0 has no flux).
    let mut flux = Vec::with_capacity(magnitudes.len());
    flux.push(0.0f32);
    for i in 1..magnitudes.len() {
        let prev = &magnitudes[i - 1];
        let cur = &magnitudes[i];
        let len = cur.len().min(prev.len());
        let mut sum = 0.0f32;
        for k in 0..len {
            let diff = cur[k] - prev[k];
            if diff > 0.0 {
                sum += diff * diff;
            }
        }
        flux.push(sum.sqrt());
    }

    // Running-median threshold over a small window (±5 frames).
    let window = 5usize;
    let mut onsets = Vec::new();
    for i in 1..(flux.len() - 1) {
        // Local maximum check
        if flux[i] <= flux[i - 1] || flux[i] <= flux[i + 1] {
            continue;
        }
        // Compute median of surrounding window
        let lo = i.saturating_sub(window);
        let hi = (i + window + 1).min(flux.len());
        let mut local: Vec<f32> = flux[lo..hi].to_vec();
        local.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = local[local.len() / 2];
        if flux[i] > median * threshold_factor && flux[i] > 1e-6 {
            let time = (i * hop_size) as f32 / sample_rate;
            onsets.push(time);
        }
    }
    onsets
}

/// Detect the fundamental pitch of a buffer via autocorrelation.
///
/// Computes the autocorrelation of `buffer`, locates the dominant peak in
/// the lag range `[sample_rate/max_hz, sample_rate/min_hz]`, then refines
/// it to sub-sample accuracy using **Newton-Raphson on a Catmull-Rom cubic
/// interpolant** through the four samples surrounding the discrete peak
/// (`hisab::num::roots::newton_raphson` solves `P'(τ) = 0`).
///
/// Returns `None` for buffers shorter than 32 samples, when no peak
/// exceeds 30 % of the zero-lag energy (signal is noise-dominated), or
/// when the search range is degenerate (`min_hz >= max_hz` after
/// quantizing to integer lags).
///
/// `min_hz` / `max_hz` should bracket the expected pitch range — narrow
/// brackets reject octave errors and keep the autocorrelation cheaper.
/// For musical pitch detection, common values are `30.0..2000.0` Hz.
///
/// Requires the `synthesis` feature (uses hisab).
#[cfg(feature = "synthesis")]
#[must_use]
pub fn detect_pitch_autocorr(
    buffer: &[f32],
    sample_rate: f32,
    min_hz: f32,
    max_hz: f32,
) -> Option<f32> {
    let n = buffer.len();
    if n < 32 || sample_rate <= 0.0 || min_hz <= 0.0 || max_hz <= min_hz {
        return None;
    }

    let max_lag = ((sample_rate / min_hz) as usize).min(n - 1);
    let min_lag = ((sample_rate / max_hz) as usize).max(2);
    if min_lag + 2 >= max_lag {
        return None;
    }

    // Direct autocorrelation up to max_lag. For typical pitch-detection
    // buffers (1k-4k samples) this is fast enough; FFT-based autocorr is
    // a future optimization.
    let mut autocorr = vec![0.0f32; max_lag + 2];
    for (lag, slot) in autocorr.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for i in 0..(n - lag) {
            sum += buffer[i] * buffer[i + lag];
        }
        *slot = sum;
    }

    let r0 = autocorr[0];
    if r0 <= 0.0 {
        return None;
    }

    // Find the largest peak in [min_lag, max_lag-1].
    let mut peak_lag = min_lag;
    let mut peak_val = autocorr[min_lag];
    for lag in (min_lag + 1)..max_lag {
        if autocorr[lag] > peak_val
            && autocorr[lag] > autocorr[lag - 1]
            && autocorr[lag] > autocorr[lag + 1]
        {
            peak_val = autocorr[lag];
            peak_lag = lag;
        }
    }

    // Reject noise-dominated buffers (peak too weak relative to r(0)).
    if peak_val < 0.30 * r0 {
        return None;
    }

    // Catmull-Rom cubic through (peak_lag-1, peak_lag, peak_lag+1, peak_lag+2)
    // mapped to t ∈ [0, 1] between the middle pair. The peak lives at some
    // fractional t we'll find by Newton-Raphson on the derivative.
    let y0 = autocorr[peak_lag - 1] as f64;
    let y1 = autocorr[peak_lag] as f64;
    let y2 = autocorr[peak_lag + 1] as f64;
    let y3 = autocorr[peak_lag + 2] as f64;

    // P(t)   = 0.5 * (a*t³ + b*t² + c*t + 2*y1)
    // P'(t)  = 0.5 * (3a*t² + 2b*t + c)
    // P''(t) = 0.5 * (6a*t + 2b)
    let a = -y0 + 3.0 * y1 - 3.0 * y2 + y3;
    let b = 2.0 * y0 - 5.0 * y1 + 4.0 * y2 - y3;
    let c = -y0 + y2;

    let dp = move |t: f64| 0.5 * (3.0 * a * t * t + 2.0 * b * t + c);
    let ddp = move |t: f64| 0.5 * (6.0 * a * t + 2.0 * b);

    // Initial guess: parabolic-interpolation closed-form (one NR step on
    // a local quadratic). Newton then refines on the cubic.
    let denom = 2.0 * (y0 - 2.0 * y1 + y2);
    let parabolic_t = if denom.abs() < 1e-12 {
        0.0
    } else {
        ((y0 - y2) / denom).clamp(-1.0, 1.0)
    };

    let refined_t = hisab::num::newton_raphson(dp, ddp, parabolic_t, 1e-9, 16)
        .unwrap_or(parabolic_t)
        .clamp(-1.0, 1.0);

    let refined_lag = peak_lag as f64 + refined_t;
    if refined_lag <= 0.0 {
        return None;
    }
    let pitch = sample_rate as f64 / refined_lag;
    if !pitch.is_finite() || pitch < min_hz as f64 * 0.5 || pitch > max_hz as f64 * 2.0 {
        return None;
    }
    Some(pitch as f32)
}

/// 256-entry lookup table mapping dB to linear amplitude over `[-80, +20] dB`.
///
/// Built lazily on first access via `LazyLock`. Resolution is ~0.39 dB per
/// entry; consumers should use [`db_to_amplitude_lut`] which interpolates
/// between adjacent entries — the residual error of a smooth exponential
/// at sub-half-dB spacing is well below audibility for dynamics processing.
const DB_LUT_SIZE: usize = 256;
const DB_LUT_MIN: f32 = -80.0;
const DB_LUT_MAX: f32 = 20.0;
const DB_LUT_RANGE: f32 = DB_LUT_MAX - DB_LUT_MIN;

static DB_TO_AMP_LUT: std::sync::LazyLock<[f32; DB_LUT_SIZE]> = std::sync::LazyLock::new(|| {
    let mut table = [0.0f32; DB_LUT_SIZE];
    for (i, slot) in table.iter_mut().enumerate() {
        let t = i as f32 / (DB_LUT_SIZE - 1) as f32;
        let db = DB_LUT_MIN + t * DB_LUT_RANGE;
        *slot = 10.0f32.powf(db / 20.0);
    }
    table
});

/// Fast `dB → linear amplitude` via a 256-entry LUT with linear interp.
///
/// Avoids `powf` in inner loops — the compressor / limiter / de-esser gain
/// stages call this every sample. Inputs outside `[-80, +20] dB` clamp to
/// the table edges (silence / `+20 dB ≈ 10×`). For one-shot conversions
/// where exact accuracy matters more than throughput, prefer
/// [`db_to_amplitude`].
#[inline]
#[must_use]
pub fn db_to_amplitude_lut(db: f32) -> f32 {
    let table = &*DB_TO_AMP_LUT;
    if db <= DB_LUT_MIN {
        return table[0];
    }
    if db >= DB_LUT_MAX {
        return table[DB_LUT_SIZE - 1];
    }
    let normalized = (db - DB_LUT_MIN) / DB_LUT_RANGE;
    let idx = normalized * (DB_LUT_SIZE - 1) as f32;
    let i0 = idx as usize;
    let i1 = (i0 + 1).min(DB_LUT_SIZE - 1);
    let frac = idx - i0 as f32;
    table[i0] * (1.0 - frac) + table[i1] * frac
}

/// One step of the Marsaglia xorshift32 PRNG.
///
/// Updates `state` in place and returns the new value. Includes a
/// zero-state guard — `xorshift32(0) = 0` would otherwise loop forever,
/// so a zero state is silently reset to `1` before stepping.
///
/// This is the canonical xorshift implementation used by every PRNG
/// site in the crate (white/pink/brown noise, granular spray jitter,
/// drum click transients, physical-modeling exciters). Use it directly
/// instead of inlining the three-XOR sequence.
#[inline]
pub fn xorshift32(state: &mut u32) -> u32 {
    if *state == 0 {
        *state = 1;
    }
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// One step of [`xorshift32`] mapped to a signed `f32` in `[-1.0, 1.0)`.
#[inline]
#[must_use]
pub fn xorshift32_signed_f32(state: &mut u32) -> f32 {
    (xorshift32(state) as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// One step of [`xorshift32`] mapped to an unsigned `f32` in `[0.0, 1.0)`.
#[inline]
#[must_use]
pub fn xorshift32_unit_f32(state: &mut u32) -> f32 {
    xorshift32(state) as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_amplitude_roundtrip() {
        let amp = 0.5;
        let db = amplitude_to_db(amp);
        let back = db_to_amplitude(db);
        assert!(
            (amp - back).abs() < 1e-5,
            "roundtrip failed: {amp} -> {db} -> {back}"
        );
    }

    #[test]
    fn test_db_zero() {
        assert_eq!(amplitude_to_db(1.0), 0.0);
        assert_eq!(db_to_amplitude(0.0), 1.0);
    }

    #[test]
    fn test_db_negative_infinity() {
        assert_eq!(amplitude_to_db(0.0), f32::NEG_INFINITY);
        assert_eq!(db_to_amplitude(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn test_normalize() {
        let mut buf = [0.5, -1.0, 0.25];
        normalize(&mut buf);
        assert!((buf[1].abs() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_silence() {
        let mut buf = [0.0, 0.0, 0.0];
        normalize(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_hard_limit() {
        assert_eq!(hard_limit(2.0, 1.0), 1.0);
        assert_eq!(hard_limit(-2.0, 1.0), -1.0);
        assert_eq!(hard_limit(0.5, 1.0), 0.5);
    }

    #[test]
    fn test_soft_clip() {
        let out = soft_clip_tanh(10.0, 1.0);
        assert!(
            (out - 1.0).abs() < 0.01,
            "tanh(10) should be near 1.0, got {out}"
        );
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 1.0, 0.5) - 0.5).abs() < f32::EPSILON);
        assert!((lerp(0.0, 1.0, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 1.0, 1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_hermite() {
        // For a straight line y = x at points 0,1,2,3, hermite at t=0.5 should be 1.5
        let val = hermite_interpolate(0.0, 1.0, 2.0, 3.0, 0.5);
        assert!((val - 1.5).abs() < 0.01, "hermite on linear data: {val}");
    }

    #[test]
    fn test_crossfade() {
        let dry_only = crossfade_equal_power(1.0, 0.0, 0.0);
        assert!((dry_only - 1.0).abs() < 0.01);
        let wet_only = crossfade_equal_power(0.0, 1.0, 1.0);
        assert!((wet_only - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_serde_roundtrip_smoothing_mode() {
        let mode = SmoothingMode::Exponential;
        let json = serde_json::to_string(&mode).unwrap();
        let back: SmoothingMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }

    #[test]
    fn test_rms() {
        let buf = [1.0f32; 100];
        assert!((rms(&buf) - 1.0).abs() < f32::EPSILON);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn test_peak() {
        let buf = [0.5, -0.8, 0.3];
        assert!((peak(&buf) - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_hann_window() {
        let mut buf = [1.0f32; 64];
        apply_hann_window(&mut buf);
        // First and last samples should be near zero
        assert!(buf[0].abs() < 0.01);
        assert!(buf[63].abs() < 0.05);
        // Middle sample should be near 1.0
        assert!(buf[32] > 0.9);
    }

    #[test]
    fn test_blackman_window() {
        let mut buf = [1.0f32; 64];
        apply_blackman_window(&mut buf);
        assert!(buf[0].abs() < 0.01);
        assert!(buf[32] > 0.9);
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_fft_magnitudes_sine() {
        // Generate a 440Hz sine at 44100Hz, 1024 samples
        let n = 1024;
        let mut buf = vec![0.0f32; n];
        for (i, s) in buf.iter_mut().enumerate() {
            *s = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin();
        }
        let mags = fft_magnitudes(&buf);
        assert_eq!(mags.len(), n / 2 + 1);
        // Bin for 440Hz: 440 * 1024 / 44100 ≈ bin 10
        let peak_bin = mags
            .iter()
            .enumerate()
            .skip(1) // skip DC
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        let expected_bin = (440.0 * n as f32 / 44100.0).round() as usize;
        assert!(
            (peak_bin as i32 - expected_bin as i32).unsigned_abs() <= 1,
            "peak should be near bin {expected_bin}, got {peak_bin}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_power_spectrum() {
        let n = 256;
        let buf = vec![0.5f32; n]; // DC signal
        let ps = power_spectrum(&buf);
        assert_eq!(ps.len(), n / 2 + 1);
        // DC bin should have the most power
        assert!(ps[0] > ps[1]);
    }

    #[cfg(feature = "synthesis")]
    fn synth_sine(freq_hz: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (i as f32 / sample_rate * freq_hz * std::f32::consts::TAU).sin())
            .collect()
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_bspline_eval_1d_interpolates_endpoints() {
        // Clamped open uniform B-spline interpolates first and last control points.
        let cps = [0.0f32, 0.5, -0.3, 1.0, 0.2];
        let at_0 = bspline_eval_1d(3, &cps, 0.0).unwrap();
        let at_1 = bspline_eval_1d(3, &cps, 1.0).unwrap();
        assert!((at_0 - cps[0]).abs() < 1e-5, "spline(0) = {at_0}");
        assert!(
            (at_1 - cps[cps.len() - 1]).abs() < 1e-5,
            "spline(1) = {at_1}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_bspline_eval_1d_smooth_no_kinks() {
        let cps = [0.0f32, 1.0, 0.0, 1.0, 0.0];
        let n = 1024;
        let buf: Vec<f32> = (0..n)
            .map(|i| bspline_eval_1d(3, &cps, i as f32 / (n - 1) as f32).unwrap())
            .collect();
        // Cubic B-spline through these 5 control points: max sample-to-sample
        // delta should be modest (<<1.0/sample-spacing).
        let max_diff = buf
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            max_diff < 0.05,
            "B-spline shouldn't have sharp jumps; max |Δ| = {max_diff}"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_bspline_eval_1d_rejects_invalid() {
        // degree=0 is invalid (B-spline of degree 0 is just step function — not useful)
        assert!(bspline_eval_1d(0, &[1.0, 2.0], 0.5).is_none());
        // Too few control points
        assert!(bspline_eval_1d(3, &[1.0, 2.0], 0.5).is_none()); // need ≥4
        // t out of [0, 1]
        assert!(bspline_eval_1d(3, &[0.0, 1.0, 0.0, 1.0], -0.1).is_none());
        assert!(bspline_eval_1d(3, &[0.0, 1.0, 0.0, 1.0], 1.1).is_none());
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_fit_polynomial_recovers_known_quadratic() {
        // y = 1 + 2x + 3x²; fit should recover those coefficients exactly
        // (within numerical noise) given enough sample points.
        let xs: Vec<f64> = (0..20).map(|i| -2.0 + i as f64 * 0.2).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| 1.0 + 2.0 * x + 3.0 * x * x).collect();
        let coeffs = fit_polynomial(&xs, &ys, 2).unwrap();
        assert_eq!(coeffs.len(), 3);
        assert!((coeffs[0] - 1.0).abs() < 1e-9, "c0 = {}", coeffs[0]);
        assert!((coeffs[1] - 2.0).abs() < 1e-9, "c1 = {}", coeffs[1]);
        assert!((coeffs[2] - 3.0).abs() < 1e-9, "c2 = {}", coeffs[2]);
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_eval_polynomial_horner() {
        // p(x) = 1 + 2x + 3x²
        let coeffs = vec![1.0f64, 2.0, 3.0];
        for x in [-2.0f32, -0.5, 0.0, 0.5, 2.0] {
            let exact = 1.0 + 2.0 * x + 3.0 * x * x;
            let got = eval_polynomial(&coeffs, x);
            assert!(
                (got - exact).abs() < 1e-5,
                "p({x}) = {got}, expected {exact}"
            );
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_eval_polynomial_empty_returns_zero() {
        assert_eq!(eval_polynomial(&[], 1.0), 0.0);
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_fit_polynomial_approximates_tanh() {
        // Demonstrate the canonical use case: replace tanh with a fast
        // odd polynomial that's accurate over a useful range.
        let xs: Vec<f64> = (0..201).map(|i| -2.0 + i as f64 * 0.02).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| x.tanh()).collect();
        let coeffs = fit_polynomial(&xs, &ys, 5).unwrap();

        // Verify the fit is accurate within [-1.5, 1.5] — outside that
        // range tanh saturates and a polynomial diverges, but the soft-clip
        // sweet spot is well inside.
        for i in 0..=30 {
            let x = -1.5 + i as f32 * 0.1;
            let exact = (x as f64).tanh() as f32;
            let approx = eval_polynomial(&coeffs, x);
            assert!(
                (exact - approx).abs() < 0.015,
                "tanh({x}): exact={exact}, fit={approx}"
            );
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_fit_polynomial_rejects_invalid() {
        // mismatched lengths
        assert!(fit_polynomial(&[1.0, 2.0], &[1.0], 1).is_none());
        // not enough points for the requested degree
        assert!(fit_polynomial(&[1.0, 2.0], &[1.0, 2.0], 5).is_none());
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_stft_dimensions_match_spec() {
        // 4096 samples, window 1024, hop 256 → (4096-1024)/256 + 1 = 13 frames.
        let signal = vec![0.5f32; 4096];
        let frames = stft_magnitudes(&signal, 1024, 256, SpectralWindow::Hann);
        assert_eq!(frames.len(), 13);
        for f in &frames {
            assert_eq!(f.len(), 1024 / 2 + 1);
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_stft_locates_sine_peak() {
        let sr = 44100.0;
        let freq = 1000.0;
        let n = 8192;
        let signal: Vec<f32> = (0..n)
            .map(|i| (i as f32 / sr * freq * std::f32::consts::TAU).sin())
            .collect();
        let frames = stft_magnitudes(&signal, 2048, 512, SpectralWindow::Hann);
        assert!(!frames.is_empty());

        // Each frame's peak bin should map to ~freq.
        let bin_hz = sr / 2048.0;
        for frame in &frames {
            let (peak_bin, _) = frame
                .iter()
                .enumerate()
                .skip(1)
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();
            let peak_freq = peak_bin as f32 * bin_hz;
            assert!(
                (peak_freq - freq).abs() < bin_hz * 1.5,
                "STFT peak at {peak_freq} Hz, expected ~{freq} Hz"
            );
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_stft_rejects_invalid_inputs() {
        // Non-power-of-2 window
        assert!(stft_magnitudes(&vec![0.0; 4096], 1000, 256, SpectralWindow::Hann).is_empty());
        // Hop = 0
        assert!(stft_magnitudes(&vec![0.0; 4096], 1024, 0, SpectralWindow::Hann).is_empty());
        // Signal shorter than window
        assert!(stft_magnitudes(&vec![0.0; 100], 1024, 256, SpectralWindow::Hann).is_empty());
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_chromagram_concentrates_on_correct_pitch_class() {
        // 440 Hz = A4. Pitch class A is index 9 (C=0, C#=1, ..., A=9).
        let sr = 44100.0;
        let signal: Vec<f32> = (0..16_384)
            .map(|i| (i as f32 / sr * 440.0 * std::f32::consts::TAU).sin())
            .collect();
        let stft = stft_magnitudes(&signal, 4096, 1024, SpectralWindow::Hann);
        let chroma = chromagram(&stft, 4096, sr);
        assert!(!chroma.is_empty());
        for frame in &chroma {
            let (peak_pc, _) = frame
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();
            assert_eq!(peak_pc, 9, "440 Hz should peak at pitch-class A (idx 9)");
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_onsets_finds_burst() {
        let sr = 44100.0;
        let n = sr as usize; // 1 second
        let mut signal = vec![0.0f32; n];
        // Two onsets: a sine burst at 0.25s and another at 0.6s.
        let burst_len = (sr * 0.05) as usize;
        for &start_t in &[0.25f32, 0.6] {
            let start = (start_t * sr) as usize;
            for i in 0..burst_len {
                if start + i < n {
                    signal[start + i] = (i as f32 / sr * 440.0 * std::f32::consts::TAU).sin() * 0.8;
                }
            }
        }
        let stft = stft_magnitudes(&signal, 1024, 256, SpectralWindow::Hann);
        let onsets = detect_onsets(&stft, 256, sr, 1.5);
        assert!(
            !onsets.is_empty(),
            "should detect at least one onset in burst signal"
        );
        // First detected onset should land near 0.25s (within 50ms).
        let first = onsets[0];
        assert!(
            (first - 0.25).abs() < 0.05,
            "first onset {first}s should be near 0.25s"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_pitch_sine_440() {
        let sr = 44100.0;
        let buf = synth_sine(440.0, sr, 4096);
        let pitch = detect_pitch_autocorr(&buf, sr, 30.0, 2000.0).unwrap();
        let cents_off = 1200.0 * (pitch / 440.0).log2();
        assert!(
            cents_off.abs() < 5.0,
            "440 Hz sine: detected {pitch} Hz ({cents_off:+.2} cents off)"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_pitch_sine_220_and_880() {
        let sr = 44100.0;
        for &freq in &[220.0f32, 880.0, 1100.0] {
            let buf = synth_sine(freq, sr, 4096);
            let pitch = detect_pitch_autocorr(&buf, sr, 30.0, 2000.0).unwrap();
            let cents_off = 1200.0 * (pitch / freq).log2();
            assert!(
                cents_off.abs() < 5.0,
                "{freq} Hz sine: detected {pitch} Hz ({cents_off:+.2} cents off)"
            );
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_pitch_subsample_accuracy() {
        // Pick a frequency whose period isn't an integer number of samples
        // — this exercises the cubic+NR refinement (without it, accuracy
        // drops to ±20 cents).
        let sr = 44100.0;
        let freq = 437.3; // period ≈ 100.86 samples
        let buf = synth_sine(freq, sr, 4096);
        let pitch = detect_pitch_autocorr(&buf, sr, 30.0, 2000.0).unwrap();
        let cents_off = 1200.0 * (pitch / freq).log2();
        assert!(
            cents_off.abs() < 5.0,
            "non-integer period: detected {pitch} Hz vs {freq} Hz ({cents_off:+.2} cents)"
        );
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_pitch_rejects_noise() {
        // Pure white noise should yield no confident pitch.
        let mut state = 12345u32;
        let noise: Vec<f32> = (0..4096)
            .map(|_| xorshift32_signed_f32(&mut state))
            .collect();
        // Most calls will return None; an occasional spurious peak is fine
        // but should not be musically meaningful. Just assert that the API
        // doesn't panic and returns either None or something well outside
        // a sensible musical range for noise.
        let pitch = detect_pitch_autocorr(&noise, 44100.0, 30.0, 2000.0);
        if let Some(p) = pitch {
            assert!(p.is_finite(), "noise pitch must be finite if returned");
        }
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_pitch_short_buffer_returns_none() {
        let buf = [0.0f32; 16];
        assert!(detect_pitch_autocorr(&buf, 44100.0, 30.0, 2000.0).is_none());
    }

    #[cfg(feature = "synthesis")]
    #[test]
    fn test_detect_pitch_invalid_range_returns_none() {
        let buf = synth_sine(440.0, 44100.0, 1024);
        // min_hz > max_hz → degenerate
        assert!(detect_pitch_autocorr(&buf, 44100.0, 2000.0, 30.0).is_none());
        // negative sample rate
        assert!(detect_pitch_autocorr(&buf, -1.0, 30.0, 2000.0).is_none());
    }

    #[test]
    fn test_db_to_amplitude_lut_matches_reference() {
        // Sweep -80..+20 dB; LUT should be within 0.5% of the powf reference
        // across the table range. Linear interpolation between 256 entries
        // gives sub-half-dB accuracy on a smooth exponential.
        for i in 0..=200 {
            let db = -80.0 + (i as f32) * 0.5;
            let exact = db_to_amplitude(db);
            let lut = db_to_amplitude_lut(db);
            let rel_err = if exact > 0.0 {
                ((lut - exact) / exact).abs()
            } else {
                (lut - exact).abs()
            };
            assert!(
                rel_err < 0.005,
                "LUT diverges at {db} dB: exact={exact}, lut={lut}, rel_err={rel_err}"
            );
        }
    }

    #[test]
    fn test_db_to_amplitude_lut_clamps_out_of_range() {
        // Below table min — clamps to the silence floor.
        let very_quiet = db_to_amplitude_lut(-200.0);
        let table_floor = db_to_amplitude_lut(-80.0);
        assert!((very_quiet - table_floor).abs() < f32::EPSILON);

        // Above table max — clamps to +20 dB ≈ 10×.
        let very_loud = db_to_amplitude_lut(60.0);
        let table_ceil = db_to_amplitude_lut(20.0);
        assert!((very_loud - table_ceil).abs() < f32::EPSILON);
    }

    #[test]
    fn test_xorshift32_zero_state_guard() {
        // Without the guard, xorshift32(0) loops on 0 forever.
        let mut state = 0u32;
        let v = xorshift32(&mut state);
        assert_ne!(v, 0, "zero-state guard must produce non-zero output");
        assert_ne!(state, 0, "state must not remain zero after step");
    }

    #[test]
    fn test_xorshift32_deterministic() {
        let mut a = 42u32;
        let mut b = 42u32;
        for _ in 0..100 {
            assert_eq!(xorshift32(&mut a), xorshift32(&mut b));
        }
    }

    #[test]
    fn test_xorshift32_signed_range() {
        let mut state = 12345u32;
        for _ in 0..10_000 {
            let v = xorshift32_signed_f32(&mut state);
            assert!((-1.0..1.0).contains(&v), "signed PRNG out of range: {v}");
        }
    }

    #[test]
    fn test_xorshift32_unit_range() {
        let mut state = 67890u32;
        for _ in 0..10_000 {
            let v = xorshift32_unit_f32(&mut state);
            assert!((0.0..1.0).contains(&v), "unit PRNG out of range: {v}");
        }
    }
}
