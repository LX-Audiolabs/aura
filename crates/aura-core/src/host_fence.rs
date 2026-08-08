//! Host-boundary panic fence.
//!
//! Format wrappers (CLAP / VST3 / LV2) are `extern "C"` (or COM) entry points.
//! A panic that unwinds across that boundary is undefined behaviour and can
//! abort the host. Catch here, log once, return a safe fallback so the DAW
//! keeps running.
//!
//! Requires `panic = "unwind"` (Rust default; product workspaces pin it).

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Run `body` at a host ABI boundary. On panic: log and return `fallback`.
#[inline]
pub fn host_callback_with<R>(
    format: &str,
    action: &str,
    fallback: R,
    body: impl FnOnce() -> R,
) -> R {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(r) => r,
        Err(payload) => {
            eprintln!(
                "[AURA {format}] panic in {action}: {}",
                panic_message(&payload)
            );
            fallback
        }
    }
}

/// Like [`host_callback_with`] for fallible fallthrough (`()` body).
#[inline]
pub fn host_callback(format: &str, action: &str, body: impl FnOnce()) {
    host_callback_with(format, action, (), body);
}

fn panic_message(payload: &Box<dyn Any + Send>) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "<non-string panic payload>"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_body_returns_value() {
        let v = host_callback_with("test", "ok", 9u32, || 3);
        assert_eq!(v, 3);
    }

    #[test]
    fn panicking_body_returns_fallback() {
        let v = host_callback_with("test", "boom", 9u32, || panic!("boom"));
        assert_eq!(v, 9);
    }

    #[test]
    fn void_callback_swallows_panic() {
        host_callback("test", "void-boom", || panic!("void"));
    }
}
