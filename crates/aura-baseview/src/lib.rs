// WindowHandler impl (on_frame, …) is cfg'd per exclusive backend. Zero or
// multiple backends → silent missing methods; fail early with a clear message.
#[cfg(not(any(
    all(
        feature = "backend-femtovg",
        not(feature = "backend-skia"),
        not(feature = "backend-wgpu")
    ),
    all(
        feature = "backend-skia",
        not(feature = "backend-femtovg"),
        not(feature = "backend-wgpu")
    ),
    all(
        feature = "backend-wgpu",
        not(feature = "backend-femtovg"),
        not(feature = "backend-skia")
    ),
)))]
compile_error!(
    "aura-baseview: enable exactly one of `backend-femtovg`, `backend-skia`, or \
     `backend-wgpu` (zero or multiple features — check consumers that set \
     default-features = false without selecting a renderer)"
);

#[cfg(feature = "backend-femtovg")]
pub mod baseview_slint_window_adapter;
#[cfg(feature = "backend-femtovg")]
pub mod open_gl_interface;

#[cfg(feature = "backend-femtovg")]
pub use baseview_slint_window_adapter::GlInitError;
#[cfg(feature = "backend-wgpu")]
pub mod blit;
pub mod platform;
pub mod scale;
#[cfg(feature = "backend-skia")]
pub mod skia_window_adapter;
pub mod slint_window;
#[cfg(feature = "backend-wgpu")]
pub mod software_renderer;
pub mod translate;

pub use slint_window::SlintParentedWindow;

pub use scale::{
    EditorScale, RequestResizeFn, SizePolicy, fit_size, pack_size, to_physical_px, unpack_size,
};
