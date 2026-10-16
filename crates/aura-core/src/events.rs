//! GUI → audio-thread param event queue.

use std::sync::Mutex;

/// Param event from the editor toward the host (gestures mark automation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamEvent {
    GestureBegin(u32),
    Value { id: u32, plain: f64 },
    GestureEnd(u32),
}

/// GUI → audio-thread param event queue.
///
/// Editor (`push`, GUI thread) → format wrapper (`drain` in `process` / `flush`,
/// audio thread) → host automation events.
///
/// **Capacity:** unbounded `Mutex<Vec<_>>`. Not a ring. A poisoned lock
/// (`push` no-op, `drain` returns empty) **drops events** rather than
/// panicking the audio thread. That full-drop is intentional
/// (`ponytail:` no lock-free ring until profiling says so).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_fifo_oldest_first() {
        let q = ParamEventQueue::default();
        q.push(ParamEvent::GestureBegin(1));
        q.push(ParamEvent::Value { id: 1, plain: 0.5 });
        q.push(ParamEvent::GestureEnd(1));
        assert_eq!(
            q.drain(),
            [
                ParamEvent::GestureBegin(1),
                ParamEvent::Value { id: 1, plain: 0.5 },
                ParamEvent::GestureEnd(1),
            ]
        );
        assert_eq!(q.drain().len(), 0);
    }
}
