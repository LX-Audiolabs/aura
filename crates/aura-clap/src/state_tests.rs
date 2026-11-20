use super::*;
use std::io::{Cursor, Read};
use std::sync::atomic::AtomicU8;

#[derive(Default)]
struct TestParams(AtomicU8);
impl aura_params::__private::Sealed for TestParams {}
impl Params for TestParams {
    fn param_infos(&self) -> Vec<ParamInfo> {
        Vec::new()
    }
    fn count(&self) -> usize {
        0
    }
    fn get_plain(&self, _: u32) -> Option<f64> {
        None
    }
    fn get_normalized(&self, _: u32) -> Option<f64> {
        None
    }
    fn set_plain(&self, _: u32, _: f64) {}
    fn set_normalized(&self, _: u32, _: f64) {}
    fn format_value(&self, _: u32, _: f64) -> Option<String> {
        None
    }
    fn parse_value(&self, _: u32, _: &str) -> Option<f64> {
        None
    }
    fn snap_smoothers(&self) {}
    fn set_sample_rate(&self, _: f64) {}
    fn collect_values(&self) -> (Vec<u32>, Vec<f64>) {
        (Vec::new(), Vec::new())
    }
    fn restore_values(&self, _: &[(u32, f64)]) {}
}

struct CustomStatePlugin;
impl PluginLogic for CustomStatePlugin {
    type Params = TestParams;
    type DspState = ();
    fn info() -> aura_core::PluginInfo {
        aura_core::PluginInfo::new("State", "Test", "0.1.0", "state")
    }
    fn init(_: &Self::Params, _: f64) {}
    fn reset((): &mut (), _: &Self::Params, _: &AudioConfig) {}
    fn process(
        (): &mut (),
        _: &Self::Params,
        _: &mut AudioBuffer<'_, f32>,
        _: &mut ProcessContext,
    ) -> ProcessStatus {
        ProcessStatus::Continue
    }
    fn factory_presets() -> &'static [aura_core::FactoryPreset] {
        &[aura_core::FactoryPreset {
            key: "custom",
            name: "Custom",
            state: aura_core::FactoryPresetState::Blob(b"P*"),
        }]
    }
    fn save_state_with_context(params: &TestParams, context: aura_core::StateContext) -> Vec<u8> {
        assert_eq!(context, aura_core::StateContext::Project);
        vec![b'H', params.0.load(Ordering::Relaxed)]
    }
    fn load_state_with_context(
        params: &TestParams,
        context: aura_core::StateContext,
        blob: &[u8],
    ) -> bool {
        let marker = match context {
            aura_core::StateContext::Project => b'H',
            aura_core::StateContext::Preset => b'P',
            aura_core::StateContext::Duplicate => return false,
        };
        let [tag, value] = blob else {
            return false;
        };
        if *tag != marker {
            return false;
        }
        params.0.store(*value, Ordering::Relaxed);
        true
    }
}

unsafe extern "C" fn write(stream: *const clap_ostream, data: *const c_void, size: u64) -> i64 {
    let output = unsafe { &mut *(*stream).ctx.cast::<Vec<u8>>() };
    // Force the wrapper to handle partial writes.
    let size = (size as usize).min(1);
    output.extend_from_slice(unsafe { std::slice::from_raw_parts(data.cast(), size) });
    size as i64
}
unsafe extern "C" fn read(stream: *const clap_istream, data: *mut c_void, size: u64) -> i64 {
    let input = unsafe { &mut *(*stream).ctx.cast::<Cursor<Vec<u8>>>() };
    let size = (size as usize).min(1);
    input
        .read(unsafe { std::slice::from_raw_parts_mut(data.cast(), size) })
        .unwrap() as i64
}
unsafe extern "C" fn rescan(host: *const clap_host, flags: u32) {
    assert_eq!(flags, CLAP_PARAM_RESCAN_VALUES);
    unsafe { &*(*host).host_data.cast::<AtomicU32>() }.fetch_add(1, Ordering::Relaxed);
}
unsafe extern "C" fn extension(_: *const clap_host, id: *const c_char) -> *const c_void {
    static PARAMS: clap_host_params = clap_host_params {
        rescan: Some(rescan),
        clear: None,
        request_flush: None,
    };
    if unsafe { CStr::from_ptr(id) } == CLAP_EXT_PARAMS {
        (&raw const PARAMS).cast()
    } else {
        ptr::null()
    }
}

fn with_plugin(test: impl FnOnce(*const clap_plugin, &TestParams, &AtomicU32)) {
    let rescans = AtomicU32::new(0);
    let host = clap_host {
        clap_version: CLAP_VERSION,
        host_data: (&raw const rescans).cast_mut().cast(),
        name: c"Test".as_ptr(),
        vendor: c"Test".as_ptr(),
        url: c"".as_ptr(),
        version: c"1".as_ptr(),
        get_extension: Some(extension),
        request_restart: None,
        request_process: None,
        request_callback: None,
    };
    unsafe {
        let plugin =
            factory_create_plugin::<CustomStatePlugin>(ptr::null(), &host, c"state".as_ptr());
        assert!(!plugin.is_null());
        let params = Arc::clone(
            &Instance::<CustomStatePlugin>::from_plugin(plugin)
                .unwrap()
                .params,
        );
        test(plugin, &params, &rescans);
        plugin_destroy::<CustomStatePlugin>(plugin);
    }
}

fn load(plugin: *const clap_plugin, blob: Vec<u8>) -> bool {
    let mut input = Cursor::new(blob);
    let stream = clap_istream {
        ctx: (&raw mut input).cast(),
        read: Some(read),
    };
    unsafe { state_load::<CustomStatePlugin>(plugin, &stream) }
}

#[test]
fn ordinary_state_roundtrip_uses_custom_project_codec() {
    with_plugin(|plugin, params, rescans| {
        assert!(!CustomStatePlugin::state_context_supported());
        params.0.store(42, Ordering::Relaxed);
        let mut output = Vec::<u8>::new();
        let stream = clap_ostream {
            ctx: (&raw mut output).cast(),
            write: Some(write),
        };
        assert!(unsafe { state_save::<CustomStatePlugin>(plugin, &stream) });
        assert_eq!(output, b"H*");
        params.0.store(7, Ordering::Relaxed);
        assert!(load(plugin, output));
        assert_eq!(params.0.load(Ordering::Relaxed), 42);
        assert_eq!(rescans.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn ordinary_state_load_accepts_custom_project_blob() {
    with_plugin(|plugin, params, rescans| {
        assert!(load(plugin, b"H*".to_vec()));
        assert_eq!(params.0.load(Ordering::Relaxed), 42);
        assert_eq!(rescans.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn ordinary_state_rejected_by_custom_codec_does_not_rescan_or_modify_state() {
    with_plugin(|plugin, params, rescans| {
        params.0.store(7, Ordering::Relaxed);
        let default_blob = aura_core::encode_state(params);
        assert!(!load(plugin, default_blob));
        assert_eq!(params.0.load(Ordering::Relaxed), 7);
        assert_eq!(rescans.load(Ordering::Relaxed), 0);
    });
}

#[test]
fn factory_blob_load_uses_custom_preset_codec() {
    use clap_sys::factory::preset_discovery::CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN;
    with_plugin(|plugin, params, rescans| {
        let preset_load = preset_load::preset_load_ext::<CustomStatePlugin>();
        assert!(unsafe {
            preset_load.from_location.unwrap()(
                plugin,
                CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN,
                ptr::null(),
                c"custom".as_ptr(),
            )
        });
        assert_eq!(params.0.load(Ordering::Relaxed), 42);
        assert_eq!(rescans.load(Ordering::Relaxed), 1);
    });
}
