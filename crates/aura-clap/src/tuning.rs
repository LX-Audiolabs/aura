//! CLAP `clap.tuning/2` (draft) host integration.

use std::ffi::c_char;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};

use aura_core::{PluginLogic, TuningEvent, TuningProvider};
use clap_sys::ext::draft::tuning::{
    CLAP_EXT_TUNING, clap_event_tuning, clap_host_tuning, clap_plugin_tuning_t,
};
use clap_sys::ext::event_registry::{CLAP_EXT_EVENT_REGISTRY, clap_host_event_registry};
use clap_sys::host::clap_host;
use clap_sys::id::CLAP_INVALID_ID;
use clap_sys::plugin::clap_plugin;

use crate::Instance;

// One note-in port is typical today; keep a small fixed table so lookups
// are allocation-free on the audio thread.
const PORTS: usize = 4;
const CHANNELS: usize = 16;

pub(crate) struct TuningState {
    host: *const clap_host,
    ext: Option<&'static clap_host_tuning>,
    space_id: AtomicU16,
    // [0][0]           = global (-1, -1)
    // [p+1][0]         = port-global (port, -1)
    // [p+1][c+1]       = port+channel specific
    ids: [[AtomicU32; CHANNELS + 1]; PORTS + 1],
}

// The host pointer is stable for the plugin lifetime and the surrounding
// wrapper only uses it from one plugin instance at a time.
unsafe impl Send for TuningState {}
unsafe impl Sync for TuningState {}

impl TuningState {
    pub(crate) fn new(host: *const clap_host) -> Arc<Self> {
        Arc::new(Self {
            host,
            ext: host_tuning_ext(host),
            space_id: AtomicU16::new(event_registry_space(host)),
            ids: core::array::from_fn(|_| {
                core::array::from_fn(|_| AtomicU32::new(CLAP_INVALID_ID))
            }),
        })
    }

    #[inline]
    pub(crate) fn space_id(&self) -> u16 {
        self.space_id.load(Ordering::Relaxed)
    }

    fn indices(port: i32, channel: i32) -> (usize, usize) {
        let p = if port >= 0 && (port as usize) < PORTS {
            (port as usize) + 1
        } else {
            0
        };
        let c = if channel >= 0 && (channel as usize) < CHANNELS {
            (channel as usize) + 1
        } else {
            0
        };
        (p, c)
    }

    fn current_id(&self, port: i32, channel: i32) -> u32 {
        let (p, c) = Self::indices(port, channel);
        let specific = self.ids[p][c].load(Ordering::Relaxed);
        if specific != CLAP_INVALID_ID {
            return specific;
        }
        let port_global = self.ids[p][0].load(Ordering::Relaxed);
        if port_global != CLAP_INVALID_ID {
            return port_global;
        }
        self.ids[0][0].load(Ordering::Relaxed)
    }
}

impl TuningProvider for TuningState {
    fn apply(&self, event: &TuningEvent) {
        let (p, c) = Self::indices(i32::from(event.port_index), i32::from(event.channel));
        self.ids[p][c].store(event.tuning_id, Ordering::Relaxed);
    }

    fn relative_offset(&self, port_index: i32, channel: i32, key: i32, sample_offset: u32) -> f64 {
        let Some(ext) = self.ext else {
            return 0.0;
        };
        let id = self.current_id(port_index, channel);
        if id == CLAP_INVALID_ID {
            return 0.0;
        }
        if let Some(should) = ext.should_play
            && !unsafe { should(self.host, id, channel, key) }
        {
            return 0.0;
        }
        let Some(get) = ext.get_relative else {
            return 0.0;
        };
        unsafe { get(self.host, id, channel, key, sample_offset) }
    }

    fn should_play(&self, port_index: i32, channel: i32, key: i32) -> bool {
        let Some(ext) = self.ext else {
            return true;
        };
        let id = self.current_id(port_index, channel);
        if id == CLAP_INVALID_ID {
            return true;
        }
        ext.should_play
            .is_none_or(|should| unsafe { should(self.host, id, channel, key) })
    }
}

pub(crate) fn tuning_ext<L: PluginLogic>() -> &'static clap_plugin_tuning_t {
    static CELL: std::sync::OnceLock<clap_plugin_tuning_t> = std::sync::OnceLock::new();
    CELL.get_or_init(|| clap_plugin_tuning_t {
        changed: Some(tuning_changed::<L>),
    })
}

unsafe extern "C" fn tuning_changed<L: PluginLogic>(plugin: *const clap_plugin) {
    let Some(inst) = (unsafe { Instance::<L>::from_plugin(plugin) }) else {
        return;
    };
    inst.tuning_pool_changed.store(true, Ordering::Relaxed);
}

fn host_tuning_ext(host: *const clap_host) -> Option<&'static clap_host_tuning> {
    if host.is_null() {
        return None;
    }
    let get = unsafe { (*host).get_extension? };
    let ext = unsafe { get(host, CLAP_EXT_TUNING.as_ptr()) };
    if ext.is_null() {
        None
    } else {
        Some(unsafe { &*(ext as *const clap_host_tuning) })
    }
}

fn event_registry_space(host: *const clap_host) -> u16 {
    if host.is_null() {
        return 0;
    }
    let Some(get) = (unsafe { (*host).get_extension }) else {
        return 0;
    };
    let ext = unsafe { get(host, CLAP_EXT_EVENT_REGISTRY.as_ptr()) };
    if ext.is_null() {
        return 0;
    }
    let reg = unsafe { &*(ext as *const clap_host_event_registry) };
    let Some(query) = reg.query else {
        return 0;
    };
    let mut space_id: u16 = 0;
    if unsafe {
        query(
            host,
            CLAP_EXT_TUNING.as_ptr() as *const c_char,
            &mut space_id,
        )
    } {
        space_id
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Helpers used by the main wrapper
// ---------------------------------------------------------------------------

/// Try to interpret a non-core event as a tuning selection event.
pub(crate) unsafe fn parse_tuning_event(
    header: &clap_sys::events::clap_event_header,
    payload: *const clap_sys::events::clap_event_header,
) -> Option<TuningEvent> {
    if (header.size as usize) < std::mem::size_of::<clap_event_tuning>() {
        return None;
    }
    let e = unsafe { &*(payload as *const clap_event_tuning) };
    Some(TuningEvent {
        sample_offset: header.time,
        port_index: e.port_index,
        channel: e.channel,
        tuning_id: e.tunning_id,
    })
}
