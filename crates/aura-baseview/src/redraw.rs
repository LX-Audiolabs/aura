use std::cell::Cell;
use std::time::{Duration, Instant};

pub(crate) struct RedrawState {
    dirty: Cell<bool>,
    retry_at: Cell<Option<Instant>>,
}

impl Default for RedrawState {
    fn default() -> Self {
        Self {
            dirty: Cell::new(true),
            retry_at: Cell::new(None),
        }
    }
}

impl RedrawState {
    pub(crate) fn request(&self) {
        self.dirty.set(true);
    }
    pub(crate) fn resume(&self) {
        self.retry_at.set(None);
        self.request();
    }

    pub(crate) fn ready(&self, now: Instant, active_animation: bool) -> bool {
        self.retry_at.get().is_none_or(|deadline| now >= deadline)
            && (self.dirty.get() || active_animation)
    }

    pub(crate) fn begin_frame(&self, now: Instant, active_animation: bool) -> bool {
        if self.retry_at.get().is_some_and(|deadline| now < deadline) {
            return false;
        }
        self.dirty.replace(false) || active_animation
    }

    pub(crate) fn finish_frame(&self, now: Instant, presented: bool) {
        if presented {
            self.retry_at.set(None);
        } else {
            self.dirty.set(true);
            self.retry_at.set(Some(now + Duration::from_millis(100)));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::RedrawState;
    use std::time::{Duration, Instant};

    #[test]
    fn idle_coalesces_requests_and_windows_are_independent() {
        let a = RedrawState::default();
        let b = RedrawState::default();
        let now = Instant::now();
        assert!(a.begin_frame(now, false));
        assert!(b.begin_frame(now, false));
        a.finish_frame(now, true);
        b.finish_frame(now, true);
        for _ in 0..1000 {
            assert!(!b.ready(now, false));
        }
        a.request();
        a.request();
        assert!(a.ready(now, false));
        assert!(a.begin_frame(now, false));
        a.finish_frame(now, true);
        assert!(!a.ready(now, false));
        assert!(a.begin_frame(now, true));
        a.request();
        a.finish_frame(now, true);
        assert!(a.begin_frame(now, false));
        a.finish_frame(now, true);
        assert!(!a.ready(now, false));
        assert!(!b.ready(now, false));
    }

    #[test]
    fn requests_survive_render_and_failed_frames_retry_later() {
        let state = RedrawState::default();
        let now = Instant::now();
        assert!(state.begin_frame(now, false));
        state.request();
        state.finish_frame(now, true);
        assert!(state.begin_frame(now, false));
        state.finish_frame(now, false);
        state.request();
        assert!(!state.begin_frame(now + Duration::from_millis(99), false));
        assert!(state.begin_frame(now + Duration::from_millis(100), false));
        state.finish_frame(now, true);
        assert!(!state.begin_frame(now + Duration::from_millis(101), false));
    }
}
