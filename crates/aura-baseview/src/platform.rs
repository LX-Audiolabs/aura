//! Slint platform registration for baseview.
//!
//! `slint::platform::set_platform` is one-shot per thread. Baking the first
//! window adapter into the platform makes reopen attach `Component::new()` to
//! a dead GL/Skia surface (blank UI after hide/show). Match truce-slint:
//! register a thin platform once, hand off a fresh adapter per open via TLS.
//!
//! Also wires OS clipboard (Ctrl+C/V/X / TextInput paste). Default Platform
//! impls are no-ops → paste always looks "empty" even when the system clipboard
//! has text (vault path setup, line edits, etc.).

use slint::platform::{Clipboard, WindowAdapter};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    /// Next adapter returned by `create_window_adapter`. Set immediately
    /// before `Component::new()` so the component attaches to the live window.
    static NEXT_ADAPTER: RefCell<Option<Rc<dyn WindowAdapter>>> = const { RefCell::new(None) };

    /// Outcome of the one-shot `set_platform` attempt on this thread.
    /// `None` = not tried, `Some(Ok)` = we own platform, `Some(Err)` = lost race.
    static PLATFORM_STATE: Cell<Option<Result<(), ()>>> = const { Cell::new(None) };
}

/// Read OS clipboard text (UTF-8). Shared by Platform hooks and plugin UI
/// (e.g. Vault Setup PASTE button) so both paths use the same code.
pub fn clipboard_get() -> Option<String> {
    use copypasta::{ClipboardContext, ClipboardProvider};
    ClipboardContext::new()
        .ok()
        .and_then(|mut ctx| ctx.get_contents().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Retry clipboard read — `OpenClipboard` often fails re-entrantly during
/// Windows `WM_KEYDOWN` (Ctrl+V). PASTE-button clicks are fine; key handlers
/// need a few attempts with a short yield.
pub fn clipboard_get_retry() -> Option<String> {
    for attempt in 0..12 {
        if let Some(s) = clipboard_get() {
            return Some(s);
        }
        if attempt + 1 < 12 {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    None
}

/// Write OS clipboard text.
pub fn clipboard_set(text: &str) -> bool {
    use copypasta::{ClipboardContext, ClipboardProvider};
    ClipboardContext::new()
        .ok()
        .and_then(|mut ctx| ctx.set_contents(text.into()).ok())
        .is_some()
}

/// Stateless platform: always pulls from `NEXT_ADAPTER`.
struct BaseviewSlintPlatform;

impl slint::platform::Platform for BaseviewSlintPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        NEXT_ADAPTER.with(|slot| {
            slot.borrow_mut().take().ok_or_else(|| {
                slint::PlatformError::Other(
                    "slint-baseview: no window adapter registered — call \
                     set_next_adapter() before Component::new()"
                        .into(),
                )
            })
        })
    }

    // TextInput Ctrl+C/V/X (and programmatic paste) go through these hooks.
    // Without them every line edit can type but cannot paste from the OS clipboard.
    fn set_clipboard_text(&self, text: &str, clipboard: Clipboard) {
        if !matches!(clipboard, Clipboard::DefaultClipboard) {
            return;
        }
        let _ = clipboard_set(text);
    }

    fn clipboard_text(&self, clipboard: Clipboard) -> Option<String> {
        if !matches!(clipboard, Clipboard::DefaultClipboard) {
            return None;
        }
        // Ctrl+V runs inside WM_KEYDOWN — OpenClipboard often fails once.
        // PASTE buttons call clipboard_get_retry() already; TextInput must too.
        clipboard_get_retry()
    }
}

/// Register `BaseviewSlintPlatform` once on this thread. Idempotent.
pub fn ensure_platform() {
    PLATFORM_STATE.with(|state| {
        if state.get().is_some() {
            return;
        }
        match slint::platform::set_platform(Box::new(BaseviewSlintPlatform)) {
            Ok(()) => state.set(Some(Ok(()))),
            Err(_) => state.set(Some(Err(()))),
        }
    });
}

/// Stage `adapter` so the next `Component::new()` attaches to it.
///
/// Must run after `ensure_platform()` and before building the Slint component.
pub fn set_next_adapter(adapter: Rc<dyn WindowAdapter>) {
    PLATFORM_STATE.with(|state| match state.get() {
        Some(Ok(())) => {}
        Some(Err(())) => panic!(
            "[slint-baseview] cannot hand off window adapter — set_platform lost \
             the race on this thread (another platform is registered)"
        ),
        None => panic!(
            "[slint-baseview] call ensure_platform() before set_next_adapter()"
        ),
    });
    NEXT_ADAPTER.with(|slot| *slot.borrow_mut() = Some(adapter));
}
