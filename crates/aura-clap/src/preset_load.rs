//! CLAP `clap.preset-load/2` + optional `preset-discovery-factory/2`.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr;
use std::sync::OnceLock;

use aura_core::{PluginLogic, apply_factory_preset};
use clap_sys::ext::preset_load::{
    CLAP_EXT_PRESET_LOAD, CLAP_EXT_PRESET_LOAD_COMPAT, clap_host_preset_load,
    clap_plugin_preset_load,
};
use clap_sys::factory::preset_discovery::{
    CLAP_PRESET_DISCOVERY_FACTORY_ID, CLAP_PRESET_DISCOVERY_FACTORY_ID_COMPAT,
    CLAP_PRESET_DISCOVERY_IS_FACTORY_CONTENT, CLAP_PRESET_DISCOVERY_LOCATION_FILE,
    CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN, clap_preset_discovery_factory,
    clap_preset_discovery_indexer, clap_preset_discovery_location,
    clap_preset_discovery_location_kind, clap_preset_discovery_metadata_receiver,
    clap_preset_discovery_provider, clap_preset_discovery_provider_descriptor,
};
use clap_sys::host::clap_host;
use clap_sys::plugin::clap_plugin;
use clap_sys::universal_plugin_id::clap_universal_plugin_id;
use clap_sys::version::CLAP_VERSION;

use crate::{Instance, request_param_rescan};

pub(crate) fn is_preset_load_ext(id: &CStr) -> bool {
    id == CLAP_EXT_PRESET_LOAD || id == CLAP_EXT_PRESET_LOAD_COMPAT
}

pub(crate) fn is_discovery_factory_id(id: &CStr) -> bool {
    id == CLAP_PRESET_DISCOVERY_FACTORY_ID || id == CLAP_PRESET_DISCOVERY_FACTORY_ID_COMPAT
}

pub(crate) fn preset_load_ext<L: PluginLogic>() -> &'static clap_plugin_preset_load {
    static CELL: OnceLock<clap_plugin_preset_load> = OnceLock::new();
    CELL.get_or_init(|| clap_plugin_preset_load {
        from_location: Some(from_location::<L>),
    })
}

unsafe extern "C" fn from_location<L: PluginLogic>(
    plugin: *const clap_plugin,
    location_kind: clap_preset_discovery_location_kind,
    location: *const c_char,
    load_key: *const c_char,
) -> bool {
    aura_core::host_callback_with("CLAP", "preset_load", false, || {
        let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
            return false;
        };

        let result = match location_kind {
            CLAP_PRESET_DISCOVERY_LOCATION_FILE => {
                if location.is_null() {
                    Err((0, "FILE location is null".into()))
                } else {
                    let path = unsafe { CStr::from_ptr(location) };
                    let path = match path.to_str() {
                        Ok(s) => Path::new(s),
                        Err(_) => {
                            return notify_err(
                                inst.host,
                                location_kind,
                                location,
                                load_key,
                                0,
                                "path is not UTF-8",
                            );
                        }
                    };
                    L::load_preset_from_file(&inst.params, path).map_err(|msg| {
                        let os = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                        (os, msg)
                    })
                }
            }
            CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN => {
                let key = if load_key.is_null() {
                    ""
                } else {
                    unsafe { CStr::from_ptr(load_key) }.to_str().unwrap_or("")
                };
                match L::factory_presets().iter().find(|p| p.key == key) {
                    Some(preset) => {
                        if apply_factory_preset(&*inst.params, preset) {
                            Ok(())
                        } else {
                            Err((0, "factory preset blob rejected".into()))
                        }
                    }
                    None => Err((0, format!("unknown factory preset key `{key}`"))),
                }
            }
            _ => Err((0, format!("unsupported location kind {location_kind}"))),
        };

        match result {
            Ok(()) => {
                unsafe { request_param_rescan(inst.host) };
                notify_loaded(inst.host, location_kind, location, load_key);
                true
            }
            Err((os, msg)) => notify_err(inst.host, location_kind, location, load_key, os, &msg),
        }
    })
}

fn notify_loaded(
    host: *const clap_host,
    kind: clap_preset_discovery_location_kind,
    location: *const c_char,
    load_key: *const c_char,
) {
    if let Some(ext) = host_preset_load(host)
        && let Some(loaded) = ext.loaded
    {
        unsafe { loaded(host, kind, location, load_key) };
    }
}

fn notify_err(
    host: *const clap_host,
    kind: clap_preset_discovery_location_kind,
    location: *const c_char,
    load_key: *const c_char,
    os_error: i32,
    msg: &str,
) -> bool {
    if let Some(ext) = host_preset_load(host)
        && let Some(on_error) = ext.on_error
    {
        let cmsg =
            CString::new(msg).unwrap_or_else(|_| CString::new("preset load failed").unwrap());
        unsafe {
            on_error(host, kind, location, load_key, os_error, cmsg.as_ptr());
        }
    }
    false
}

fn host_preset_load(host: *const clap_host) -> Option<&'static clap_host_preset_load> {
    if host.is_null() {
        return None;
    }
    let get = unsafe { (*host).get_extension }?;
    let ext = unsafe { get(host, CLAP_EXT_PRESET_LOAD.as_ptr()) };
    let ext = if ext.is_null() {
        unsafe { get(host, CLAP_EXT_PRESET_LOAD_COMPAT.as_ptr()) }
    } else {
        ext
    };
    if ext.is_null() {
        None
    } else {
        Some(unsafe { &*(ext as *const clap_host_preset_load) })
    }
}

// ---------------------------------------------------------------------------
// discovery factory (only registered when factory_presets is non-empty)
// ---------------------------------------------------------------------------

struct DiscoveryNames {
    id: CString,
    /// Kept so `desc.name` / `desc.vendor` pointers stay valid.
    #[allow(dead_code)]
    name: CString,
    #[allow(dead_code)]
    vendor: CString,
    desc: clap_preset_discovery_provider_descriptor,
}

fn discovery_names<L: PluginLogic>() -> &'static DiscoveryNames {
    static CELL: OnceLock<DiscoveryNames> = OnceLock::new();
    CELL.get_or_init(|| {
        let info = L::info();
        let id = CString::new(format!("{}.presets", info.clap_id)).unwrap_or_default();
        let name = CString::new(format!("{} presets", info.name)).unwrap_or_default();
        let vendor = CString::new(info.vendor).unwrap_or_default();
        let desc = clap_preset_discovery_provider_descriptor {
            clap_version: CLAP_VERSION,
            id: id.as_ptr(),
            name: name.as_ptr(),
            vendor: vendor.as_ptr(),
        };
        DiscoveryNames {
            id,
            name,
            vendor,
            desc,
        }
    })
}

pub(crate) fn discovery_factory<L: PluginLogic>() -> *const clap_preset_discovery_factory {
    static CELL: OnceLock<clap_preset_discovery_factory> = OnceLock::new();
    let f = CELL.get_or_init(|| clap_preset_discovery_factory {
        count: Some(discovery_count),
        get_descriptor: Some(discovery_get_descriptor::<L>),
        create: Some(discovery_create::<L>),
    });
    f as *const clap_preset_discovery_factory
}

unsafe extern "C" fn discovery_count(_factory: *const clap_preset_discovery_factory) -> u32 {
    1
}

unsafe extern "C" fn discovery_get_descriptor<L: PluginLogic>(
    _factory: *const clap_preset_discovery_factory,
    index: u32,
) -> *const clap_preset_discovery_provider_descriptor {
    if index != 0 {
        return ptr::null();
    }
    &discovery_names::<L>().desc
}

#[allow(clippy::struct_field_names)]
struct Provider<L: PluginLogic> {
    provider: clap_preset_discovery_provider,
    loc_name: CString,
    indexer: *const clap_preset_discovery_indexer,
    _pd: std::marker::PhantomData<L>,
}

unsafe extern "C" fn discovery_create<L: PluginLogic>(
    _factory: *const clap_preset_discovery_factory,
    indexer: *const clap_preset_discovery_indexer,
    provider_id: *const c_char,
) -> *const clap_preset_discovery_provider {
    if provider_id.is_null() {
        return ptr::null();
    }
    let want = unsafe { CStr::from_ptr(provider_id) };
    if want != discovery_names::<L>().id.as_c_str() {
        return ptr::null();
    }

    let boxed = Box::new(Provider::<L> {
        provider: clap_preset_discovery_provider {
            desc: &discovery_names::<L>().desc,
            provider_data: ptr::null_mut(),
            init: Some(provider_init::<L>),
            destroy: Some(provider_destroy::<L>),
            get_metadata: Some(provider_get_metadata::<L>),
            get_extension: None,
        },
        loc_name: CString::new("Factory").unwrap_or_default(),
        indexer,
        _pd: std::marker::PhantomData,
    });
    let raw = Box::into_raw(boxed);
    unsafe {
        (*raw).provider.provider_data = raw.cast();
        &(*raw).provider
    }
}

fn provider_from<'a, L: PluginLogic>(
    provider: *const clap_preset_discovery_provider,
) -> Option<&'a Provider<L>> {
    if provider.is_null() {
        return None;
    }
    let data = unsafe { (*provider).provider_data as *const Provider<L> };
    if data.is_null() {
        None
    } else {
        Some(unsafe { &*data })
    }
}

unsafe extern "C" fn provider_init<L: PluginLogic>(
    provider: *const clap_preset_discovery_provider,
) -> bool {
    let Some(p) = provider_from::<L>(provider) else {
        return false;
    };
    if p.indexer.is_null() {
        return false;
    }
    let indexer = unsafe { &*p.indexer };
    let Some(declare) = indexer.declare_location else {
        return false;
    };
    let loc = clap_preset_discovery_location {
        flags: CLAP_PRESET_DISCOVERY_IS_FACTORY_CONTENT,
        name: p.loc_name.as_ptr(),
        kind: CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN,
        location: ptr::null(),
    };
    unsafe { declare(p.indexer, &loc) }
}

unsafe extern "C" fn provider_destroy<L: PluginLogic>(
    provider: *const clap_preset_discovery_provider,
) {
    if provider.is_null() {
        return;
    }
    let data = unsafe { (*provider).provider_data as *mut Provider<L> };
    if !data.is_null() {
        drop(unsafe { Box::from_raw(data) });
    }
}

unsafe extern "C" fn provider_get_metadata<L: PluginLogic>(
    _provider: *const clap_preset_discovery_provider,
    location_kind: clap_preset_discovery_location_kind,
    _location: *const c_char,
    receiver: *const clap_preset_discovery_metadata_receiver,
) -> bool {
    if receiver.is_null() || location_kind != CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN {
        return false;
    }
    let recv = unsafe { &*receiver };
    let Some(begin) = recv.begin_preset else {
        return false;
    };

    let info = L::info();
    let abi = CString::new("clap").unwrap_or_default();
    let plugin_id = CString::new(info.clap_id).unwrap_or_default();
    let uid = clap_universal_plugin_id {
        abi: abi.as_ptr(),
        id: plugin_id.as_ptr(),
    };

    for preset in L::factory_presets() {
        let name = CString::new(preset.name).unwrap_or_default();
        let key = CString::new(preset.key).unwrap_or_default();
        if !unsafe { begin(receiver, name.as_ptr(), key.as_ptr()) } {
            return true;
        }
        if let Some(add_id) = recv.add_plugin_id {
            unsafe { add_id(receiver, &uid) };
        }
        if let Some(set_flags) = recv.set_flags {
            unsafe { set_flags(receiver, CLAP_PRESET_DISCOVERY_IS_FACTORY_CONTENT) };
        }
    }
    true
}
