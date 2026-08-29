//! Precision routing for `f32` / `f64`.
//!
//! Sealed to those two types. Authors rarely name the traits — use
//! `Float` for math helpers, `Sample` for buffer/DSP surfaces.
//!
//! ```
//! use aura_params::sample::Float;
//! let v: f64 = 0.5f32.to_f64();
//! let back: f32 = f32::from_f64(v);
//! ```

use std::ops::{Add, Div, Mul, Sub};

/// Math on `f32` or `f64` (gains, freqs, coefficients — not necessarily samples).
///
/// Prefer [`Sample`] for `AudioBuffer<S>` and similar buffer bounds.
pub trait Float:
    sealed::Sealed
    + Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
    /// `true` for `f64`. Sealed, so equal `IS_F64` means same type.
    const IS_F64: bool;

    /// Widen `f32` → this precision (identity on `f32`).
    #[must_use]
    fn from_f32(v: f32) -> Self;

    /// Narrow `f64` → this precision. Debug-asserts non-NaN on `f32`
    /// (NaN in the audio path is always a bug). Release keeps NaN via `as`.
    #[must_use]
    fn from_f64(v: f64) -> Self;

    /// Narrow to `f32` (same NaN debug-assert as [`Self::from_f64`]).
    #[must_use]
    fn to_f32(self) -> f32;

    /// Widen to `f64`.
    #[must_use]
    fn to_f64(self) -> f64;

    #[must_use]
    fn exp(self) -> Self;

    #[must_use]
    fn log10(self) -> Self;

    #[must_use]
    fn powf(self, exp: Self) -> Self;
}

/// [`Float`] plus `Default + Send + Sync + 'static` for buffers and scratch.
pub trait Sample: Float + Default + Send + Sync + 'static {}

impl Sample for f32 {}
impl Sample for f64 {}

mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

impl Float for f32 {
    const IS_F64: bool = false;

    #[inline]
    fn from_f32(v: f32) -> Self {
        v
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn from_f64(v: f64) -> Self {
        debug_assert!(
            !v.is_nan(),
            "Float::from_f64: NaN narrowed to f32 - DSP loop or coefficient \
             computation produced an undefined value?",
        );
        v as f32
    }

    #[inline]
    fn to_f32(self) -> f32 {
        self
    }

    #[inline]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    #[inline]
    fn exp(self) -> Self {
        f32::exp(self)
    }
    #[inline]
    fn log10(self) -> Self {
        f32::log10(self)
    }
    #[inline]
    fn powf(self, exp: Self) -> Self {
        f32::powf(self, exp)
    }
}

impl Float for f64 {
    const IS_F64: bool = true;

    #[inline]
    fn from_f32(v: f32) -> Self {
        f64::from(v)
    }

    #[inline]
    fn from_f64(v: f64) -> Self {
        v
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn to_f32(self) -> f32 {
        debug_assert!(
            !self.is_nan(),
            "Float::to_f32: NaN narrowed to f32 - DSP loop or coefficient \
             computation produced an undefined value?",
        );
        self as f32
    }

    #[inline]
    fn to_f64(self) -> f64 {
        self
    }

    #[inline]
    fn exp(self) -> Self {
        f64::exp(self)
    }
    #[inline]
    fn log10(self) -> Self {
        f64::log10(self)
    }
    #[inline]
    fn powf(self, exp: Self) -> Self {
        f64::powf(self, exp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn widen_narrow_round_trip_f32() {
        let v: f32 = 0.123_456_7;
        assert_eq!(f32::from_f64(v.to_f64()), v);
    }

    #[test]
    fn widen_narrow_round_trip_f64_lossy() {
        let v: f64 = 0.123_456_789_012_345;
        let round_tripped = f32::from_f64(v).to_f64();
        assert!((round_tripped - v).abs() < 1e-7);
    }

    #[test]
    #[should_panic(expected = "NaN narrowed to f32")]
    #[cfg(debug_assertions)]
    fn nan_narrow_debug_panics() {
        let _ = f32::from_f64(f64::NAN);
    }
}
