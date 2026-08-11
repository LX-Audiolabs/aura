//! Shared content-scale + size handoff for `HiDPI` / multi-monitor.
//!
//! Mirrors the truce-slint / truce-gui `EditorScale` + pending-size pattern:
//! host `set_scale_factor` and baseview `Resized` write a shared cell; the
//! window handler reconciles Slint physical size on the next frame so a
//! scale change without a logical-size change cannot strand the renderer.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Live content-scale factor shared between the `Editor` adapter and the
/// baseview window handler.
#[derive(Clone)]
pub struct EditorScale {
    inner: Arc<AtomicU64>,
}

impl EditorScale {
    /// Construct with an initial scale. Non-finite or non-positive values
    /// clamp to 1.0.
    #[must_use]
    pub fn new(initial: f64) -> Self {
        let v = if initial.is_finite() && initial > 0.0 {
            initial
        } else {
            1.0
        };
        Self {
            inner: Arc::new(AtomicU64::new(v.to_bits())),
        }
    }

    #[must_use]
    pub fn get(&self) -> f64 {
        f64::from_bits(self.inner.load(Ordering::Relaxed))
    }

    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn get_f32(&self) -> f32 {
        self.get() as f32
    }

    /// Update the current scale. Non-finite or non-positive values are ignored.
    pub fn set(&self, scale: f64) {
        if scale.is_finite() && scale > 0.0 {
            self.inner.store(scale.to_bits(), Ordering::Relaxed);
        }
    }

    /// If the scale moved since `last`, update `last` and return the new value.
    #[allow(clippy::cast_possible_truncation, clippy::float_cmp)]
    pub fn take_change(&self, last: &mut f32) -> Option<f32> {
        let cur = self.get() as f32;
        if cur == *last {
            None
        } else {
            *last = cur;
            Some(cur)
        }
    }
}

/// Convert logical points → physical pixels (round-to-nearest, min 1).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[inline]
#[must_use]
pub fn to_physical_px(logical: u32, scale: f64) -> u32 {
    (f64::from(logical.max(1)) * scale).round().max(1.0) as u32
}

/// Pack `(width, height)` into a `u64` for atomic handoff. `0` = none.
#[inline]
#[must_use]
pub fn pack_size(size: (u32, u32)) -> u64 {
    (u64::from(size.0) << 32) | u64::from(size.1)
}

#[inline]
#[must_use]
pub fn unpack_size(packed: u64) -> (u32, u32) {
    #[allow(clippy::cast_possible_truncation)]
    {
        ((packed >> 32) as u32, (packed & 0xFFFF_FFFF) as u32)
    }
}

/// Clamp a host logical size into `[min, max]`.
#[must_use]
pub fn fit_size(w: u32, h: u32, min: (u32, u32), max: (u32, u32)) -> (u32, u32) {
    let (min_w, min_h) = min;
    let (max_w, max_h) = max;
    (w.clamp(min_w.max(1), max_w), h.clamp(min_h.max(1), max_h))
}

/// Size / scale policy handed to the window handler at open.
///
/// Clone is cheap (two `Arc`s). The editor adapter and the live handler share
/// `scale` + `pending_size`.
#[derive(Clone)]
pub struct SizePolicy {
    /// Design / current logical size reported by `Editor::size`.
    pub design_size: (u32, u32),
    pub min_size: (u32, u32),
    pub max_size: (u32, u32),
    pub can_resize: bool,
    /// Shared with `Editor::set_scale_factor` and the window handler.
    pub scale: EditorScale,
    /// When true, only the host may write `scale` (embedded Linux plugins).
    /// When false, baseview's OS scale from `Resized` updates `scale`.
    pub host_driven_scale: bool,
    /// Packed `(w, h)` written by `Editor::set_size`; handler drains in `on_frame`.
    pub pending_size: Arc<AtomicU64>,
}

impl SizePolicy {
    #[must_use]
    pub fn fixed(size: (u32, u32)) -> Self {
        Self {
            design_size: size,
            min_size: size,
            max_size: size,
            can_resize: false,
            scale: EditorScale::new(1.0),
            host_driven_scale: false,
            pending_size: Arc::new(AtomicU64::new(0)),
        }
    }

    #[must_use]
    pub fn resizable(size: (u32, u32), min: (u32, u32), max: (u32, u32)) -> Self {
        Self {
            design_size: size,
            min_size: min,
            max_size: max,
            can_resize: true,
            scale: EditorScale::new(1.0),
            host_driven_scale: false,
            pending_size: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Default for SizePolicy {
    fn default() -> Self {
        Self::fixed((800, 600))
    }
}

/// Host resize request: `(logical_w, logical_h) -> accepted`.
pub type RequestResizeFn = Arc<dyn Fn(u32, u32) -> bool + Send + Sync>;
