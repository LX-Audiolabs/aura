// WindowHandler impl (on_frame, …) is cfg'd per exclusive backend. Zero or
// multiple backends → silent missing methods; fail early with a clear message.
// Aliases (`backend-femtovg`, `backend-wgpu`) enable a canonical feature below.
#[cfg(not(any(
    all(
        feature = "backend-femtovg-gl",
        not(feature = "backend-femtovg-wgpu"),
        not(feature = "backend-skia"),
        not(feature = "backend-software")
    ),
    all(
        feature = "backend-femtovg-wgpu",
        not(feature = "backend-femtovg-gl"),
        not(feature = "backend-skia"),
        not(feature = "backend-software")
    ),
    all(
        feature = "backend-skia",
        not(feature = "backend-femtovg-gl"),
        not(feature = "backend-femtovg-wgpu"),
        not(feature = "backend-software")
    ),
    all(
        feature = "backend-software",
        not(feature = "backend-femtovg-gl"),
        not(feature = "backend-femtovg-wgpu"),
        not(feature = "backend-skia")
    ),
)))]
compile_error!(
    "aura-baseview: enable exactly one of `backend-femtovg-gl`, \
     `backend-femtovg-wgpu`, `backend-skia`, or `backend-software` \
     (aliases: `backend-femtovg`→gl, `backend-wgpu`→software; check consumers \
     that set default-features = false without selecting a renderer)"
);

#[cfg(feature = "backend-femtovg-gl")]
pub mod baseview_slint_window_adapter;
#[cfg(feature = "backend-femtovg-gl")]
pub mod open_gl_interface;

#[cfg(feature = "backend-femtovg-gl")]
pub use baseview_slint_window_adapter::GlInitError;
#[cfg(feature = "backend-femtovg-wgpu")]
pub mod femtovg_wgpu_window_adapter;
#[cfg(feature = "backend-femtovg-wgpu")]
pub use femtovg_wgpu_window_adapter::WgpuInitError;
#[cfg(feature = "backend-software")]
pub mod blit;
pub mod platform;
pub mod scale;
#[cfg(feature = "backend-skia")]
pub mod skia_window_adapter;
pub mod slint_window;
#[cfg(feature = "backend-software")]
pub mod software_renderer;
pub mod translate;

pub use slint_window::SlintParentedWindow;

pub use scale::{
    EditorScale, RequestResizeFn, SizePolicy, fit_size, pack_size, to_physical_px, unpack_size,
};
