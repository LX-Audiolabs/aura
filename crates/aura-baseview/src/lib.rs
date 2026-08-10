#[cfg(feature = "backend-femtovg")]
pub mod baseview_slint_window_adapter;
#[cfg(feature = "backend-femtovg")]
pub mod open_gl_interface;

#[cfg(feature = "backend-femtovg")]
pub use baseview_slint_window_adapter::GlInitError;
#[cfg(feature = "backend-skia")]
pub mod skia_window_adapter;
#[cfg(feature = "backend-wgpu")]
pub mod blit;
#[cfg(feature = "backend-wgpu")]
pub mod software_renderer;
pub mod platform;
pub mod scale;
pub mod slint_window;
pub mod translate;

pub use slint_window::SlintParentedWindow;

pub use scale::{
    fit_size, pack_size, to_physical_px, unpack_size, EditorScale, RequestResizeFn, SizePolicy,
};
