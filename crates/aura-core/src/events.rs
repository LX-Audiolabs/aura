//! GUI → audio-thread param event queue.

use std::sync::Mutex;

/// Param event from the editor toward the host (gestures mark automation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamEvent {
    GestureBegin(u32),
    Value {
        id: u32,
        plain: f64,
    },
    GestureEnd(u32),
}

/// Minimal thread-safe queue: the editor pushes (GUI thread), the format
/// wrapper drains in `process` / `flush` (audio thread) and emits host
/// events. A plain mutex is fine — uncontended in practice
/// (ponytail: no lock-free ring until profiling says so).
#[derive(Default)]
pub struct ParamEventQueue {
    inner: Mutex<Vec<ParamEvent>>,
}

impl ParamEventQueue {
    pub fn push(&self, event: ParamEvent) {
        if let Ok(mut v) = self.inner.lock() {
            v.push(event);
        }
    }

    /// Take all pending events (oldest first).
    pub fn drain(&self) -> Vec<ParamEvent> {
        match self.inner.lock() {
            Ok(mut v) => std::mem::take(&mut *v),
            Err(_) => Vec::new(),
        }
    }
}
