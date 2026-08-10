//! Host-facing editor trait (windowing: `aura-baseview`; adapter: `aura-editor`).

use std::sync::Arc;

use aura_params::Params;

/// Raw platform window handle for GUI parenting.
#[derive(Clone, Copy, Debug)]
pub enum RawWindowHandle {
    AppKit(*mut std::ffi::c_void),
    Win32(*mut std::ffi::c_void),
    X11(u64),
    /// Wayland / other — opaque for now.
    Other(*mut std::ffi::c_void),
}

// SAFETY: handles are only used on the UI thread by format wrappers / editor.
unsafe impl Send for RawWindowHandle {}

/// Bridge editor ↔ host / param store (minimal).
///
/// Format wrappers will implement this; `aura-editor` adapter calls it.
pub trait EditorBridge: Send + Sync {
    fn begin_edit(&self, id: u32);
    fn set_param(&self, id: u32, normalized: f64);
    fn end_edit(&self, id: u32);
    fn get_param(&self, id: u32) -> f64;
    fn get_param_plain(&self, id: u32) -> f64;
    fn request_resize(&self, w: u32, h: u32) -> bool {
        let _ = (w, h);
        false
    }
}

/// Context passed to [`Editor::open`].
#[derive(Clone)]
pub struct PluginContext {
    pub params: Arc<dyn Params>,
    pub bridge: Option<Arc<dyn EditorBridge>>,
    pub sample_rate: f64,
}

impl PluginContext {
    #[must_use]
    pub fn new(params: Arc<dyn Params>) -> Self {
        Self {
            params,
            bridge: None,
            sample_rate: 44_100.0,
        }
    }

    #[must_use]
    pub fn with_bridge(mut self, bridge: Arc<dyn EditorBridge>) -> Self {
        self.bridge = Some(bridge);
        self
    }

    #[must_use]
    pub fn with_sample_rate(mut self, sr: f64) -> Self {
        self.sample_rate = sr;
        self
    }
}

/// Plugin GUI editor — host open/close/idle/resize.
///
/// Concrete UIs: window stack in `aura-baseview`; host adapter in `aura-editor`.
pub trait Editor: Send {
    fn size(&self) -> (u32, u32);

    fn open(&mut self, parent: RawWindowHandle, context: PluginContext);

    fn close(&mut self);

    /// Show the editor window (CLAP `gui.show`). Default: no-op.
    fn show(&mut self) {}

    /// Hide the editor window without destroying it (CLAP `gui.hide`).
    fn hide(&mut self) {}

    fn idle(&mut self) {}

    fn set_size(&mut self, _width: u32, _height: u32) -> bool {
        false
    }

    fn can_resize(&self) -> bool {
        false
    }

    fn min_size(&self) -> (u32, u32) {
        (1, 1)
    }

    fn max_size(&self) -> (u32, u32) {
        (u32::MAX, u32::MAX)
    }

    fn set_scale(&mut self, _scale: f64) {}

    fn state_changed(&mut self) {}

    /// Native widget handle for hosts that require it (e.g. LV2 UI).
    ///
    /// `None` means the toolkit did not expose a handle, or the editor is
    /// closed. The pointer is only valid while the editor is open.
    fn native_handle(&self) -> Option<RawWindowHandle> {
        None
    }
}

/// Box a concrete editor into `Box<dyn Editor>`.
pub trait IntoEditor {
    fn into_editor(self) -> Box<dyn Editor>;
}

impl<E: Editor + 'static> IntoEditor for E {
    fn into_editor(self) -> Box<dyn Editor> {
        Box::new(self)
    }
}
