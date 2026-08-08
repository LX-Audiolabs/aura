//! Portable analysis primitives (from product `lx-analysis`).
//!
//! **In scope:** FFT / SNAP, spectrum display maths, clip-wave rings,
//! audio↔UI meter building blocks.
//!
//! **Out of scope (stay product):** `lx-shm`, `lx-vault`, and per-plugin
//! `*Shared` types (`AetherShared`, `AurumShared`, …). Product plugins
//! compose those from [`shared`] building blocks.

pub mod dev_log;
pub mod shared;
pub mod snap_fft;

pub use shared::{
    AutoLoud, PeakMeters, ScopeRing, ShmClaimShared, SnapPipeline, SpectrumView, band5, band5_tol,
};
pub use snap_fft::{SnapFFT, SnapMode};

/// Half-spectrum bin count (matches historical `lx_shm::SPECTRUM_BINS`).
pub const SPECTRUM_BINS: usize = 1024;

/// EQ band count used by multi-band product UIs (matches `lx_shm::EQ_BANDS`).
pub const EQ_BANDS: usize = 5;

/// Default per-band tolerances (matches `lx_vault::DEFAULT_TOLERANCES`).
pub const DEFAULT_BAND_TOLERANCES: [f32; 5] = [1.5, 2.0, 3.5, 4.5, 4.5];

pub const SCOPE_BUFFER_LEN: usize = 4096;

/// Pre-clipper waveform ring length (signed linear samples).
pub const CLIP_WAVE_LEN: usize = 16384;

#[derive(Clone, Default)]
pub struct ClipWaveRing {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub mid: Vec<f32>,
    pub side: Vec<f32>,
}

impl ClipWaveRing {
    #[must_use]
    pub fn new() -> Self {
        Self {
            l: vec![0.0; CLIP_WAVE_LEN],
            r: vec![0.0; CLIP_WAVE_LEN],
            mid: vec![0.0; CLIP_WAVE_LEN],
            side: vec![0.0; CLIP_WAVE_LEN],
        }
    }
}

/// Sub-pixel scroll phase within the newest min/max bucket (0..1).
#[must_use]
pub fn clip_wave_scroll_phase(write_pos: usize, ring_len: usize, cols: usize) -> f32 {
    let spp = (ring_len / cols.max(1)).max(1);
    (write_pos % spp) as f32 / spp as f32
}

/// Chronological min/max buckets for filled waveform display (oldest left, newest right).
#[must_use]
pub fn clip_wave_minmax_window(ring: &[f32], write_pos: usize, cols: usize) -> Vec<(f32, f32)> {
    let len = ring.len();
    if len == 0 || cols == 0 {
        return Vec::new();
    }
    let cols = cols.min(len);
    let start = write_pos.wrapping_sub(len) % len;
    let spp = (len / cols).max(1);
    (0..cols)
        .map(|col| {
            let i0 = col * spp;
            let i1 = if col + 1 == cols {
                len
            } else {
                ((col + 1) * spp).min(len)
            };
            let mut min = f32::MAX;
            let mut max = f32::MIN;
            for i in i0..i1 {
                let v = ring[(start + i) % len];
                min = min.min(v);
                max = max.max(v);
            }
            if min == f32::MAX {
                (0.0, 0.0)
            } else {
                (min, max)
            }
        })
        .collect()
}

/// Raw dB above which display tilt is applied in [`compute_spectrum_bins`].
pub const SPECTRUM_TILT_RAW_GATE_DB: f32 = -90.0;

/// 4.5 dB/octave display tilt at `freq` (0 below 20 Hz).
#[inline]
#[must_use]
pub fn spectrum_tilt_db(freq: f32) -> f32 {
    if freq > 20.0 {
        4.5 * (freq / 1000.0).log2()
    } else {
        0.0
    }
}

/// Physical (pre-tilt) dB underlying a display bin — undoes tilt when applied.
#[inline]
#[must_use]
pub fn spectrum_physical_db(displayed_db: f32, freq: f32) -> f32 {
    if displayed_db > SPECTRUM_TILT_RAW_GATE_DB {
        (displayed_db - spectrum_tilt_db(freq)).max(-90.0)
    } else {
        displayed_db
    }
}

/// Compute display-ready spectrum bins from raw FFT output.
/// Applies 4.5 dB/octave tilt so pink noise appears flat.
#[inline]
pub fn compute_spectrum_bins(
    fft_output: &[realfft::num_complex::Complex<f32>],
    frame: &mut [f32],
    fft_size: usize,
    sample_rate: f32,
) {
    let inv_norm = 2.0 / fft_size as f32;
    for (k, slot) in frame.iter_mut().enumerate() {
        let mag = fft_output[k].norm() * inv_norm;
        let db = if mag > 1e-9 {
            20.0 * mag.log10()
        } else {
            -90.0
        };
        let freq = k as f32 * sample_rate / fft_size as f32;
        let tilt = if db > SPECTRUM_TILT_RAW_GATE_DB {
            spectrum_tilt_db(freq)
        } else {
            0.0
        };
        *slot = (db + tilt).clamp(-90.0, 12.0);
    }
}

/// Editor-driving flag for relay active masks (product Lucent-style UIs).
pub const RELAY_MASK_DRIVEN: u32 = 1u32 << 31;

/// Whether publisher `slot` is enabled under the editor's relay mask.
#[inline]
#[must_use]
pub fn relay_slot_active(mask: u32, slot: u8) -> bool {
    if mask & RELAY_MASK_DRIVEN == 0 {
        return true;
    }
    mask & (1u32 << slot) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minmax_window_buckets_signed_samples() {
        let ring = vec![1.0, -0.5, 0.8, -1.0, 0.0, 0.2];
        let out = clip_wave_minmax_window(&ring, 6, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], (-0.5, 1.0));
        assert_eq!(out[1], (-1.0, 0.8));
        assert_eq!(out[2], (0.0, 0.2));
    }

    #[test]
    fn scroll_phase_wraps_within_bucket() {
        assert_eq!(clip_wave_scroll_phase(0, 8192, 320), 0.0);
        assert!((clip_wave_scroll_phase(13, 8192, 320) - 13.0 / 25.0).abs() < 1e-5);
    }

    #[test]
    fn relay_slot_active_respects_bits() {
        assert!(relay_slot_active(0, 0));
        assert!(relay_slot_active(0, 2));
        let mask = (1 << 2) | RELAY_MASK_DRIVEN;
        assert!(!relay_slot_active(mask, 0));
        assert!(relay_slot_active(mask, 2));
        assert!(!relay_slot_active(RELAY_MASK_DRIVEN, 0));
        assert!(!relay_slot_active(RELAY_MASK_DRIVEN, 2));
    }

    #[test]
    fn spectrum_tilt_at_1k_is_zero() {
        assert!((spectrum_tilt_db(1000.0)).abs() < 1e-5);
    }

    #[test]
    fn snap_fft_push_fills_then_ready() {
        let mut snap = SnapFFT::new();
        let n = SPECTRUM_BINS * 2;
        for i in 0..n - 1 {
            assert!(!snap.push_sample((i as f32 * 0.001).sin()));
        }
        assert!(snap.push_sample(0.0));
        let frame = snap.compute_fft(48_000.0);
        assert_eq!(frame.len(), SPECTRUM_BINS);
    }
}
