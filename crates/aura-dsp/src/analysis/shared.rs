//! Audio ↔ UI shared-state **building blocks** (not product plugin types).
//!
//! Product plugins compose these into `AetherShared` / `MensorShared` / … in
//! the plugins repo. Framework only owns the reusable field groups.

use super::{ClipWaveRing, DEFAULT_BAND_TOLERANCES, SCOPE_BUFFER_LEN, SPECTRUM_BINS};
use atomic_float::AtomicF32;
use aura_params::AudioTap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32};
use std::sync::{Arc, Mutex};

#[inline]
fn af(v: f32) -> Arc<AtomicF32> {
    Arc::new(AtomicF32::new(v))
}

#[inline]
fn ab(v: bool) -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(v))
}

/// Five identical atomic floats (band meters / levels).
#[must_use]
pub fn band5(v: f32) -> [Arc<AtomicF32>; 5] {
    [af(v), af(v), af(v), af(v), af(v)]
}

/// Five atomic floats seeded from [`DEFAULT_BAND_TOLERANCES`].
#[must_use]
pub fn band5_tol() -> [Arc<AtomicF32>; 5] {
    [
        af(DEFAULT_BAND_TOLERANCES[0]),
        af(DEFAULT_BAND_TOLERANCES[1]),
        af(DEFAULT_BAND_TOLERANCES[2]),
        af(DEFAULT_BAND_TOLERANCES[3]),
        af(DEFAULT_BAND_TOLERANCES[4]),
    ]
}

#[inline]
fn spectrum_buf() -> Arc<Mutex<Vec<f32>>> {
    Arc::new(Mutex::new(vec![-90.0; SPECTRUM_BINS]))
}

/// Output / correlation / balance meters (stereo + mono peak + holds).
#[derive(Clone)]
pub struct PeakMeters {
    pub phase_correlation: Arc<AtomicF32>,
    pub output_peak: Arc<AtomicF32>,
    pub peak_hold: Arc<AtomicF32>,
    pub output_peak_l: Arc<AtomicF32>,
    pub output_peak_r: Arc<AtomicF32>,
    pub peak_hold_l: Arc<AtomicF32>,
    pub peak_hold_r: Arc<AtomicF32>,
    pub reset_peak: Arc<AtomicBool>,
    pub balance: Arc<AtomicF32>,
}

impl Default for PeakMeters {
    fn default() -> Self {
        Self {
            phase_correlation: af(1.0),
            output_peak: af(-90.0),
            peak_hold: af(-90.0),
            output_peak_l: af(-90.0),
            output_peak_r: af(-90.0),
            peak_hold_l: af(-90.0),
            peak_hold_r: af(-90.0),
            reset_peak: ab(false),
            balance: af(0.0),
        }
    }
}

/// Goniometer / vectorscope tap — lock-free audio→UI stereo sample
/// stream (G15). Audio thread [`push`](Self::push)es each block's
/// (already visually-auto-gained) L/R samples; UI/editor thread
/// [`drain`](Self::drain)s new pairs each tick and keeps its own
/// bounded display window — this only moves raw samples across the
/// thread boundary, same "never block" contract as [`AudioTap`].
#[derive(Clone)]
pub struct ScopeRing {
    left: Arc<AudioTap>,
    right: Arc<AudioTap>,
}

impl ScopeRing {
    /// Push one stereo sample from the audio thread.
    pub fn push(&self, l: f32, r: f32) {
        self.left.push(&[l]);
        self.right.push(&[r]);
    }

    /// Drain every stereo pair pushed since the last call, oldest
    /// first. UI/editor thread only. Empty when nothing new was
    /// pushed (e.g. transport stopped) — callers keep their own
    /// rolling display window rather than re-reading a persistent
    /// buffer here.
    #[must_use]
    pub fn drain(&self) -> Vec<[f32; 2]> {
        let l = self.left.drain();
        let r = self.right.drain();
        l.into_iter().zip(r).map(|(l, r)| [l, r]).collect()
    }
}

impl Default for ScopeRing {
    fn default() -> Self {
        Self {
            left: Arc::new(AudioTap::new(SCOPE_BUFFER_LEN)),
            right: Arc::new(AudioTap::new(SCOPE_BUFFER_LEN)),
        }
    }
}

/// FFT magnitude bins + EMA + optional 1/3-oct display smooth + sample rate.
#[derive(Clone)]
pub struct SpectrumView {
    pub bins: Arc<Mutex<Vec<f32>>>,
    pub avg: Arc<Mutex<Vec<f32>>>,
    /// UI-only 1/3-octave display smooth (not a plugin param).
    pub smooth: Arc<AtomicBool>,
    pub sample_rate: Arc<AtomicF32>,
}

impl Default for SpectrumView {
    fn default() -> Self {
        Self {
            bins: spectrum_buf(),
            avg: spectrum_buf(),
            smooth: ab(false),
            sample_rate: af(44100.0),
        }
    }
}

/// SNAP / ANALYZE multi-phase capture (stereo → mono → delta).
#[derive(Clone)]
pub struct SnapPipeline {
    pub active: Arc<AtomicBool>,
    /// 0=idle, 1=stereo, 2=mono, 3=delta
    pub phase: Arc<AtomicU8>,
    pub stereo: Arc<Mutex<Vec<f32>>>,
    pub mono: Arc<Mutex<Vec<f32>>>,
    pub delta: Arc<Mutex<Vec<f32>>>,
    pub reset_analysis: Arc<AtomicBool>,
}

impl Default for SnapPipeline {
    fn default() -> Self {
        Self {
            active: ab(false),
            phase: Arc::new(AtomicU8::new(0)),
            stereo: spectrum_buf(),
            mono: spectrum_buf(),
            delta: spectrum_buf(),
            reset_analysis: ab(false),
        }
    }
}

/// AUTO LOUD measurement handoff UI ↔ audio.
#[derive(Clone)]
pub struct AutoLoud {
    pub trigger: Arc<AtomicBool>,
    pub measuring: Arc<AtomicBool>,
    pub gain_offset: Arc<AtomicF32>,
}

impl Default for AutoLoud {
    fn default() -> Self {
        Self {
            trigger: ab(false),
            measuring: ab(false),
            gain_offset: af(0.0),
        }
    }
}

/// SHM publisher claim atomics only — hub lives in product `lx-shm`.
#[derive(Clone)]
pub struct ShmClaimShared {
    /// Registry slot claimed by audio/editor (-1 = none).
    pub slot: Arc<AtomicI32>,
    /// Generation from claim — must travel with the slot.
    pub generation: Arc<AtomicU32>,
}

impl Default for ShmClaimShared {
    fn default() -> Self {
        Self {
            slot: Arc::new(AtomicI32::new(-1)),
            generation: Arc::new(AtomicU32::new(0)),
        }
    }
}

/// Convenience: allocate a shared clip-wave ring for mastering UIs.
#[must_use]
pub fn new_clip_wave_shared() -> Arc<Mutex<ClipWaveRing>> {
    Arc::new(Mutex::new(ClipWaveRing::new()))
}

/// Fresh spectrum buffer at [`SPECTRUM_BINS`] (for product shared composition).
#[must_use]
pub fn new_spectrum_buf() -> Arc<Mutex<Vec<f32>>> {
    spectrum_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_ring_push_drain_round_trip() {
        let scope = ScopeRing::default();
        scope.push(0.1, -0.1);
        scope.push(0.2, -0.2);
        assert_eq!(scope.drain(), vec![[0.1, -0.1], [0.2, -0.2]]);
        // Fully drained: nothing left until the next push.
        assert!(scope.drain().is_empty());
    }

    #[test]
    fn scope_ring_clone_shares_the_same_underlying_taps() {
        // Product `*Shared` structs are `Clone`d into UI closures; both
        // handles must observe the same stream, not independent copies.
        let scope = ScopeRing::default();
        let handle = scope.clone();
        scope.push(0.5, -0.5);
        assert_eq!(handle.drain(), vec![[0.5, -0.5]]);
    }
}
