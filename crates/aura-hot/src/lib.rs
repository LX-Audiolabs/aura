//! CLAP hot-reload proxy.
//!
//! The host maps **this** binary (`Name.clap`). The real plugin is a sibling
//! `Name.impl.dll` / `.so` / `.dylib` that `cargo aura watch --hot` overwrites.
//! Each load copies the impl to a unique temp file so Windows does not lock it.
//!
//! New instances pick up the latest impl. Existing instances keep the generation
//! they were created with (inner vtables stay in that mapped copy). Remove and
//! re-add the plugin in the host to swap DSP without restarting the DAW.

#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, c_char, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use clap_sys::entry::clap_plugin_entry;
use clap_sys::factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::version::CLAP_VERSION;

struct Inner {
    /// Kept so the OS mapping stays alive. Never unloaded (dev leak is fine).
    _handle: LibHandle,
    entry: *const clap_plugin_entry,
    src_mtime: SystemTime,
}

// Safety: handle + entry are only used under STATE, and CLAP entry is Sync in practice.
unsafe impl Send for Inner {}

struct HotState {
    clap_path: PathBuf,
    current: Option<Inner>,
}

static STATE: Mutex<HotState> = Mutex::new(HotState {
    clap_path: PathBuf::new(),
    current: None,
});

#[allow(non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};

unsafe extern "C" fn entry_init(plugin_path: *const c_char) -> bool {
    let path = c_path(plugin_path).unwrap_or_default();
    let clap = resolve_clap_path(&path);
    let Ok(mut st) = STATE.lock() else {
        return false;
    };
    st.clap_path = clap;
    load_latest(&mut st).is_some()
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_get_factory(factory_id: *const c_char) -> *const c_void {
    if factory_id.is_null() {
        return std::ptr::null();
    }
    let id = unsafe { CStr::from_ptr(factory_id) };
    let Ok(mut st) = STATE.lock() else {
        return std::ptr::null();
    };
    let Some(inner) = load_latest(&mut st) else {
        return std::ptr::null();
    };
    let entry = unsafe { &*inner.entry };
    let Some(get) = entry.get_factory else {
        return std::ptr::null();
    };
    let inner_fac = unsafe { get(factory_id) };
    if id == CLAP_PLUGIN_FACTORY_ID {
        // Stable proxy so create_plugin always sees the latest impl.
        return std::ptr::from_ref(&PROXY_FACTORY).cast();
    }
    inner_fac
}

static PROXY_FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(proxy_count),
    get_plugin_descriptor: Some(proxy_desc),
    create_plugin: Some(proxy_create),
};

unsafe extern "C" fn proxy_count(_factory: *const clap_plugin_factory) -> u32 {
    with_inner_factory(|f| f.get_plugin_count.map_or(0, |g| unsafe { g(f) })).unwrap_or(0)
}

unsafe extern "C" fn proxy_desc(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    with_inner_factory(|f| {
        f.get_plugin_descriptor
            .map_or(std::ptr::null(), |g| unsafe { g(f, index) })
    })
    .unwrap_or(std::ptr::null())
}

unsafe extern "C" fn proxy_create(
    _factory: *const clap_plugin_factory,
    host: *const clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    with_inner_factory(|f| {
        f.create_plugin
            .map_or(std::ptr::null(), |g| unsafe { g(f, host, plugin_id) })
    })
    .unwrap_or(std::ptr::null())
}

fn with_inner_factory<T>(f: impl FnOnce(&clap_plugin_factory) -> T) -> Option<T> {
    let Ok(mut st) = STATE.lock() else {
        return None;
    };
    let inner = load_latest(&mut st)?;
    let entry = unsafe { &*inner.entry };
    let get = entry.get_factory?;
    let fac = unsafe { get(CLAP_PLUGIN_FACTORY_ID.as_ptr()) };
    if fac.is_null() {
        return None;
    }
    Some(f(unsafe { &*(fac.cast::<clap_plugin_factory>()) }))
}

fn load_latest(st: &mut HotState) -> Option<&Inner> {
    if st.clap_path.as_os_str().is_empty() {
        return st.current.as_ref();
    }
    let impl_path = impl_path_beside(&st.clap_path);
    let meta = fs::metadata(&impl_path).ok()?;
    let mtime = meta.modified().ok()?;
    let stale = st.current.as_ref().is_none_or(|c| c.src_mtime != mtime);
    if stale {
        let inner = load_impl(&impl_path, mtime)?;
        st.current = Some(inner);
    }
    st.current.as_ref()
}

fn load_impl(impl_path: &Path, mtime: SystemTime) -> Option<Inner> {
    let live = live_copy_path(impl_path);
    if let Some(parent) = live.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::copy(impl_path, &live).ok()?;
    let handle = unsafe { LibHandle::open(&live)? };
    let entry = handle.symbol("clap_entry")?;
    Some(Inner {
        _handle: handle,
        entry,
        src_mtime: mtime,
    })
}

/// `Foo.clap` → `Foo.impl.dll` / `.so` / `.dylib` beside the clap (or bundle).
#[must_use]
pub fn impl_path_beside(clap_path: &Path) -> PathBuf {
    let clap = resolve_clap_path(clap_path);
    let stem = clap
        .file_stem()
        .map_or_else(|| "plugin".into(), |s| s.to_string_lossy().into_owned());
    clap.with_file_name(format!("{stem}{}", impl_suffix()))
}

#[must_use]
pub const fn impl_suffix() -> &'static str {
    if cfg!(windows) {
        ".impl.dll"
    } else if cfg!(target_os = "macos") {
        ".impl.dylib"
    } else {
        ".impl.so"
    }
}

fn resolve_clap_path(plugin_path: &Path) -> PathBuf {
    // Host may pass Contents/MacOS/<bin> — walk up to the .clap bundle.
    for anc in plugin_path.ancestors() {
        if anc
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("clap"))
        {
            return anc.to_path_buf();
        }
    }
    plugin_path.to_path_buf()
}

fn live_copy_path(impl_path: &Path) -> PathBuf {
    let stem = impl_path
        .file_stem()
        .map_or_else(|| "plugin".into(), |s| s.to_string_lossy().into_owned());
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!(
        "aura-hot-{stem}-{nanos}-{}{}",
        std::process::id(),
        dll_ext()
    );
    std::env::temp_dir().join(name)
}

const fn dll_ext() -> &'static str {
    if cfg!(windows) {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

fn c_path(ptr: *const c_char) -> Option<PathBuf> {
    if ptr.is_null() {
        return None;
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(c.to_bytes())))
    }
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s.as_ref()))
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = c;
        None
    }
}

struct LibHandle(*mut c_void);

impl LibHandle {
    unsafe fn open(path: &Path) -> Option<Self> {
        let h = unsafe { sys_open(path) };
        if h.is_null() { None } else { Some(Self(h)) }
    }

    fn symbol(&self, name: &str) -> Option<*const clap_plugin_entry> {
        let mut buf = name.as_bytes().to_vec();
        buf.push(0);
        let p = unsafe { sys_sym(self.0, buf.as_ptr()) };
        if p.is_null() { None } else { Some(p.cast()) }
    }
}

// Intentionally no Drop/FreeLibrary — old generations stay mapped for live instances.

unsafe fn sys_open(path: &Path) -> *mut c_void {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe { LoadLibraryW(wide.as_ptr()) }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let mut bytes = path.as_os_str().as_bytes().to_vec();
        bytes.push(0);
        unsafe { dlopen(bytes.as_ptr(), RTLD_NOW) }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        std::ptr::null_mut()
    }
}

unsafe fn sys_sym(handle: *mut c_void, name: *const u8) -> *mut c_void {
    #[cfg(windows)]
    {
        unsafe { GetProcAddress(handle, name) }
    }
    #[cfg(unix)]
    {
        unsafe { dlsym(handle, name.cast()) }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (handle, name);
        std::ptr::null_mut()
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

#[cfg(unix)]
const RTLD_NOW: i32 = 2;

#[cfg(unix)]
unsafe extern "C" {
    fn dlopen(filename: *const u8, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impl_beside_clap_file() {
        let clap = Path::new("/plugins/smoke-gain.clap");
        let got = impl_path_beside(clap);
        assert!(got.ends_with(format!("smoke-gain{}", impl_suffix())));
    }

    #[test]
    fn impl_beside_macos_bundle_binary() {
        let clap = Path::new("/plugins/smoke-gain.clap/Contents/MacOS/smoke-gain");
        let got = impl_path_beside(clap);
        assert_eq!(
            got,
            PathBuf::from(format!("/plugins/smoke-gain{}", impl_suffix()))
        );
    }
}
