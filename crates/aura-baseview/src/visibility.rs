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
    use std::cell::Cell;

    fn on_timer(vis: &Visibility, host: f32, shown: &Cell<f32>) {
        if !vis.visible() {
            return;
        }
        shown.set(host);
    }

    fn draw(vis: &Visibility, host: f32, shown: &Cell<f32>) -> bool {
        if !vis.visible() {
            return false;
        }
        if vis.take_redraw() {
            shown.set(host);
            return true;
        }
        false
    }

    #[test]
    fn hide_show_between_ticks_requests_one_redraw() {
        let state = Visibility::default();
        assert!(state.visible());
        state.hide();
        assert!(!state.visible());
        state.show();
        assert!(state.visible());
        assert!(state.redraw_pending());
        assert!(state.take_redraw());
        assert!(!state.redraw_pending());
        assert!(!state.take_redraw());
    }

    #[test]
    fn hide_blocks_sync_show_catchup_shows_host_value() {
        let vis = Visibility::default();
        let shown = Cell::new(0.0);

        vis.hide();
        on_timer(&vis, 0.75, &shown);
        assert!(!draw(&vis, 0.75, &shown));
        assert_eq!(shown.get(), 0.0);

        vis.show();
        assert!(vis.redraw_pending());
        assert!(draw(&vis, 0.75, &shown));
        assert_eq!(shown.get(), 0.75);
        assert!(!vis.redraw_pending());

        vis.hide();
        on_timer(&vis, 0.5, &shown);
        assert!(!draw(&vis, 0.5, &shown));
        assert_eq!(shown.get(), 0.75);

        vis.show();
        on_timer(&vis, 0.5, &shown);
        assert!(draw(&vis, 0.5, &shown));
        assert_eq!(shown.get(), 0.5);
    }
}
