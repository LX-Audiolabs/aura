use super::*;
use std::collections::VecDeque;

#[derive(Default)]
struct TestParams;

impl aura_params::__private::Sealed for TestParams {}

impl Params for TestParams {
    fn param_infos(&self) -> Vec<ParamInfo> {
        vec![ParamInfo {
            id: 1,
            name: "Split",
            short_name: "Split",
            group: "",
            range: ParamRange::Linear { min: 0.0, max: 1.0 },
            default_plain: 0.0,
            flags: ParamFlags::CHUNKED,
            unit: aura_params::ParamUnit::None,
            kind: aura_params::ParamValueKind::Float,
            midi_map: None,
            midi_channel: None,
        }]
    }
    fn count(&self) -> usize {
        1
    }
    fn get_plain(&self, _: u32) -> Option<f64> {
        Some(0.0)
    }
    fn get_normalized(&self, _: u32) -> Option<f64> {
        Some(0.0)
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
        (vec![1], vec![0.0])
    }
    fn restore_values(&self, _: &[(u32, f64)]) {}
}

struct StatusPlugin;

impl PluginLogic for StatusPlugin {
    type Params = TestParams;
    type DspState = VecDeque<ProcessStatus>;

    fn info() -> aura_core::PluginInfo {
        aura_core::PluginInfo::new("Status", "Test", "0.1.0", "status")
    }

    fn init(_: &Self::Params, _: f64) -> Self::DspState {
        VecDeque::new()
    }

    fn reset(_: &mut Self::DspState, _: &Self::Params, _: &AudioConfig) {}

    fn process(
        state: &mut Self::DspState,
        _: &Self::Params,
        _: &mut AudioBuffer<'_, f32>,
        _: &mut ProcessContext,
    ) -> ProcessStatus {
        #[cfg(target_arch = "x86_64")]
        {
            #[allow(deprecated)]
            // SAFETY: MXCSR is available on all x86_64 targets.
            let csr = unsafe { std::arch::x86_64::_mm_getcsr() };
            assert_eq!(csr & 0x8040, 0x8040);
        }
        state.pop_front().expect("unexpected process chunk")
    }
}

unsafe extern "C" fn event_count(list: *const clap_input_events) -> u32 {
    unsafe { &*((*list).ctx.cast::<Vec<clap_event_param_value>>()) }.len() as u32
}

unsafe extern "C" fn event_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    let events = unsafe { &*((*list).ctx.cast::<Vec<clap_event_param_value>>()) };
    &events[index as usize].header
}

fn process_status(statuses: &[ProcessStatus]) -> clap_process_status {
    let mut events: Vec<_> = (1..statuses.len())
        .map(|i| clap_event_param_value {
            header: clap_event_header {
                size: size_of::<clap_event_param_value>() as u32,
                time: i as u32 * 8,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_PARAM_VALUE,
                flags: 0,
            },
            param_id: 1,
            cookie: ptr::null_mut(),
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value: 0.5,
        })
        .collect();
    let input = clap_input_events {
        ctx: (&raw mut events).cast(),
        size: Some(event_count),
        get: Some(event_get),
    };
    // Event-only host buffers exercise the actual CLAP callback and chunking.
    let block = clap_process {
        steady_time: -1,
        frames_count: statuses.len() as u32 * 8,
        transport: ptr::null(),
        audio_inputs: ptr::null(),
        audio_outputs: ptr::null_mut(),
        audio_inputs_count: 0,
        audio_outputs_count: 0,
        in_events: &input,
        out_events: ptr::null(),
    };
    unsafe {
        let plugin =
            factory_create_plugin::<StatusPlugin>(ptr::null(), ptr::null(), c"status".as_ptr());
        assert!(!plugin.is_null());
        assert!(plugin_init::<StatusPlugin>(plugin));
        assert!(plugin_activate::<StatusPlugin>(
            plugin,
            48_000.0,
            1,
            block.frames_count
        ));
        assert!(plugin_start_processing::<StatusPlugin>(plugin));
        Instance::<StatusPlugin>::from_plugin(plugin).unwrap().state =
            Some(statuses.iter().copied().collect());
        let result = plugin_process::<StatusPlugin>(plugin, &block);
        let remaining = Instance::<StatusPlugin>::from_plugin(plugin)
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .len();
        plugin_stop_processing::<StatusPlugin>(plugin);
        plugin_deactivate::<StatusPlugin>(plugin);
        plugin_destroy::<StatusPlugin>(plugin);
        let expected_remaining = statuses
            .iter()
            .position(|s| *s == ProcessStatus::Error)
            .map_or(0, |i| statuses.len() - i - 1);
        assert_eq!(remaining, expected_remaining);
        result
    }
}

#[test]
fn sleep_reaches_clap_and_only_all_sleep_chunks_suspend() {
    use ProcessStatus::{Continue, Error, Sleep, TailFinished};
    for (statuses, expected) in [
        (vec![Sleep], clap_sys::process::CLAP_PROCESS_SLEEP),
        (vec![Sleep, Sleep], clap_sys::process::CLAP_PROCESS_SLEEP),
        (vec![Sleep, Continue], CLAP_PROCESS_CONTINUE),
        (vec![Continue, Sleep], CLAP_PROCESS_CONTINUE),
        (vec![Sleep, TailFinished], CLAP_PROCESS_TAIL),
        (vec![TailFinished, Sleep], CLAP_PROCESS_TAIL),
        (vec![Continue], CLAP_PROCESS_CONTINUE),
        (vec![TailFinished], CLAP_PROCESS_TAIL),
        (vec![Continue, TailFinished], CLAP_PROCESS_TAIL),
        (vec![TailFinished, Continue], CLAP_PROCESS_TAIL),
        (vec![Error], CLAP_PROCESS_ERROR),
        (vec![Sleep, Error, Continue], CLAP_PROCESS_ERROR),
        (vec![TailFinished, Error, Sleep], CLAP_PROCESS_ERROR),
    ] {
        assert_eq!(process_status(&statuses), expected, "{statuses:?}");
    }
}

#[cfg(target_arch = "x86_64")]
#[test]
#[allow(deprecated)]
fn process_enables_denormal_mode_and_restores_host_environment() {
    // SAFETY: All operations use valid x86_64 MXCSR values on this thread.
    let saved = unsafe { std::arch::x86_64::_mm_getcsr() };
    unsafe { std::arch::x86_64::_mm_setcsr(saved & !0x8040) };
    let result = process_status(&[ProcessStatus::Continue]);
    let restored = unsafe { std::arch::x86_64::_mm_getcsr() };
    unsafe { std::arch::x86_64::_mm_setcsr(saved) };
    assert_eq!(result, CLAP_PROCESS_CONTINUE);
    assert_eq!(restored, saved & !0x8040);
}
