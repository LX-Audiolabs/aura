//! The plugin's own window, opened floating (`gui_create(is_floating = true)`).
//!
//! Embedding a plugin window into ours is Phase 3 (`SetParent` + `WS_CHILD`).
//! AURA's own plugins are embed-only — `aura-clap` answers `false` to floating
//! — so for those the host's param sliders are the UI.

use std::ffi::CString;

use clap_sys::ext::gui::{CLAP_EXT_GUI, clap_plugin_gui};
use clap_sys::plugin::clap_plugin;

use crate::loader;

#[cfg(target_os = "windows")]
fn window_api() -> &'static std::ffi::CStr {
    clap_sys::ext::gui::CLAP_WINDOW_API_WIN32
}
#[cfg(target_os = "macos")]
fn window_api() -> &'static std::ffi::CStr {
    clap_sys::ext::gui::CLAP_WINDOW_API_COCOA
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn window_api() -> &'static std::ffi::CStr {
    clap_sys::ext::gui::CLAP_WINDOW_API_X11
}

fn gui_ext(plugin: *const clap_plugin) -> Option<&'static clap_plugin_gui> {
    loader::plugin_ext(plugin, CLAP_EXT_GUI).map(|p| unsafe { &*p.cast::<clap_plugin_gui>() })
}

/// Can this plugin put up its own top-level window?
#[must_use]
pub fn supports_floating(plugin: *const clap_plugin) -> bool {
    gui_ext(plugin).is_some_and(|g| {
        g.is_api_supported
            .is_some_and(|f| unsafe { f(plugin, window_api().as_ptr(), true) })
    })
}

/// An open plugin window. Dropping it calls `gui.destroy`.
pub struct FloatingGui {
    plugin: *const clap_plugin,
}

impl FloatingGui {
    /// Create and show the plugin's floating window. Main thread only.
    pub fn open(plugin: *const clap_plugin, title: &str) -> Result<Self, String> {
        let gui = gui_ext(plugin).ok_or("plugin has no clap.gui extension")?;
        if !supports_floating(plugin) {
            return Err("plugin supports embedded (non-floating) GUIs only".into());
        }
        let create = gui.create.ok_or("clap.gui has no create")?;
        if !unsafe { create(plugin, window_api().as_ptr(), true) } {
            return Err("gui.create returned false".into());
        }
        let me = Self { plugin };

        if let Some(suggest) = gui.suggest_title
            && let Ok(t) = CString::new(title)
        {
            unsafe { suggest(plugin, t.as_ptr()) };
        }
        match gui.show {
            Some(show) if unsafe { show(plugin) } => Ok(me),
            // `me` drops here, so a failed show still calls gui.destroy.
            Some(_) => Err("gui.show returned false".into()),
            None => Err("clap.gui has no show".into()),
        }
    }
}

impl Drop for FloatingGui {
    fn drop(&mut self) {
        if let Some(gui) = gui_ext(self.plugin)
            && let Some(destroy) = gui.destroy
        {
            unsafe { destroy(self.plugin) };
        }
    }
}
