use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Visibility {
    visible: AtomicBool,
    redraw: AtomicBool,
}
impl Default for Visibility {
    fn default() -> Self {
        Self {
            visible: AtomicBool::new(true),
            redraw: AtomicBool::new(false),
        }
    }
}
impl Visibility {
    pub(crate) fn visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }
    pub(crate) fn hide(&self) {
        self.visible.store(false, Ordering::Release);
    }
    pub(crate) fn show(&self) {
        self.redraw.store(true, Ordering::Release);
        self.visible.store(true, Ordering::Release);
    }
    #[cfg(any(
        feature = "backend-femtovg-gl",
        feature = "backend-femtovg-wgpu",
        feature = "backend-skia"
    ))]
    pub(crate) fn redraw_pending(&self) -> bool {
        self.redraw.load(Ordering::Acquire)
    }
    pub(crate) fn take_redraw(&self) -> bool {
        self.redraw.swap(false, Ordering::AcqRel)
    }
}
#[cfg(test)]
mod tests {
    use super::Visibility;

    #[test]
    fn hide_show_between_ticks_requests_one_redraw() {
        let state = Visibility::default();
        assert!(state.visible());
        state.hide();
        assert!(!state.visible());
        state.show();
        assert!(state.visible());
        assert!(state.take_redraw());
        assert!(!state.take_redraw());
    }
}
