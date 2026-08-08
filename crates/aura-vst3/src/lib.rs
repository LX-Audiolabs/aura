//! Minimal VST3 wrapper for AURA.
//!
//! **Spec:** [steinbergmedia/vst3sdk](https://github.com/steinbergmedia/vst3sdk)
//! is the source of truth. Bindings: [`vst3`](https://crates.io/crates/vst3)
//! (generated from the C++ headers; module layout mirrors the namespaces).
//!
//! ```ignore
//! aura_vst3::export_vst3!(MyPlugin);
//! ```
//!
//! Covers: `GetPluginFactory` entry, single-component model (`IComponent` +
//! `IAudioProcessor` + `IEditController` on one object), main FX bus
//! (mono/stereo from [`PluginLogic::bus_layouts`]), f32 samples, params
//! (1:1 `ParamID` map, no hash), flat state blob via
//! [`aura_core::encode_state`] / [`aura_core::decode_state`], parented GUI
//! via `IPlugView` + the same [`Editor`] trait as CLAP. No MIDI.

#![allow(clippy::missing_safety_doc)]
// ponytail: VST3 FFI glue — raw-pointer casts and C-int size conversions are
// spec-shaped; the "safer" spellings add noise without changing semantics.
// `unnecessary_cast`: the bindings alias enum types to `c_int`/`c_uint`
// depending on platform, so casts that are redundant on Windows keep the
// source portable. `non_snake_case`: trait method params mirror the C++ SDK.
#![allow(non_snake_case)]
#![allow(
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr,
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::unnecessary_cast
)]

mod gui;

use std::ffi::{CStr, c_char, c_void};
use std::marker::PhantomData;
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use aura_core::info::PluginCategory;
use aura_core::transport::Transport;
use aura_core::{
    AudioBuffer, AudioConfig, BusLayout, PluginLogic, ProcessContext, ProcessMode, decode_state,
    encode_state, host_callback_with, layout_at,
};
use aura_params::{ParamFlags, ParamInfo, ParamValueKind, Params};
use gui::GuiState;
use vst3::Steinberg::Vst::BusDirections_::{kInput, kOutput};
use vst3::Steinberg::Vst::BusInfo_::BusFlags_::kDefaultActive;
use vst3::Steinberg::Vst::BusTypes_::kMain;
use vst3::Steinberg::Vst::MediaTypes_::kAudio;
use vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::{
    kCanAutomate, kIsBypass, kIsHidden, kIsList, kIsReadOnly,
};
use vst3::Steinberg::Vst::ProcessContext_::StatesAndFlags_::{
    kBarPositionValid, kCycleActive, kCycleValid, kPlaying, kProjectTimeMusicValid, kRecording,
    kTempoValid, kTimeSigValid,
};
use vst3::Steinberg::Vst::ProcessModes_::{kOffline, kPrefetch};
use vst3::Steinberg::Vst::SymbolicSampleSizes_::kSample32;
use vst3::Steinberg::Vst::{
    BusDirection, BusInfo, BusType, IAudioProcessor, IAudioProcessorTrait, IAudioProcessor_iid,
    IComponent, IComponentHandler, IComponentTrait, IComponent_iid, IEditController,
    IEditControllerTrait, IEditController_iid, IParamValueQueueTrait, IParameterChanges,
    IParameterChangesTrait, IoMode, MediaType, ParamID, ParamValue, ParameterInfo, ProcessData,
    ProcessSetup, RoutingInfo, SpeakerArrangement, String128, TChar, kRootUnitId,
};
// IPlugView / TBool live in Steinberg root, not Steinberg::Vst.
use vst3::Steinberg::{
    FIDString, FUnknown, IBStream, IBStreamTrait, IPlugView, IPluginBaseTrait, IPluginFactory,
    IPluginFactory2, IPluginFactory2Trait, IPluginFactoryTrait, PClassInfo, PClassInfo2,
    PFactoryInfo, TBool, TUID, char8, int32, kInternalError, kInvalidArgument, kNoInterface,
    kNotImplemented, kResultFalse, kResultOk, tresult, uint32,
};
use vst3::{Class, ComPtr, ComRef, ComWrapper};

// ---------------------------------------------------------------------------
// Export macro
// ---------------------------------------------------------------------------

/// Hidden re-export so [`export_vst3!`] can name vst3 types without a
/// direct plugin dependency on the `vst3` crate.
#[doc(hidden)]
pub use vst3 as __vst3;

/// Export `$logic` ([`PluginLogic`]) as this cdylib's VST3 entry point.
///
/// Emits `GetPluginFactory` plus the platform module init/exit stubs hosts
/// look for (`InitDll`/`ExitDll` on Windows, `BundleEntry`/`BundleExit` on
/// macOS, `ModuleEntry`/`ModuleExit` on Linux).
#[macro_export]
macro_rules! export_vst3 {
    ($logic:ty) => {
        /// VST3 module entry point called by the host after loading the bundle.
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn GetPluginFactory() -> *mut $crate::__vst3::Steinberg::IPluginFactory {
            $crate::plugin_factory::<$logic>()
        }

        #[cfg(target_os = "windows")]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn InitDll() -> bool {
            true
        }

        #[cfg(target_os = "windows")]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn ExitDll() -> bool {
            true
        }

        #[cfg(target_os = "macos")]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn BundleEntry(_bundle_ref: *mut core::ffi::c_void) -> bool {
            true
        }

        #[cfg(target_os = "macos")]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn BundleExit() -> bool {
            true
        }

        #[cfg(target_os = "linux")]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn ModuleEntry(_library_handle: *mut core::ffi::c_void) -> bool {
            true
        }

        #[cfg(target_os = "linux")]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        pub extern "system" fn ModuleExit() -> bool {
            true
        }
    };
}

/// Create a fresh plugin factory for `L`. Hosts call this once via
/// `GetPluginFactory` and own the returned reference.
#[must_use]
pub fn plugin_factory<L: PluginLogic>() -> *mut IPluginFactory {
    let wrapper = ComWrapper::new(Factory::<L>(PhantomData));
    match wrapper.to_com_ptr::<IPluginFactory>() {
        Some(ptr) => ComPtr::into_raw(ptr),
        None => ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Class ID (TUID)
// ---------------------------------------------------------------------------

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit over `bytes` with a custom offset seed.
const fn fnv1a(seed: u64, bytes: &[u8]) -> u64 {
    let mut hash = seed;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Deterministic 16-byte class ID derived from an ID string: two FNV-1a
/// 64-bit passes with different offset seeds, little-endian packed.
///
/// **Changing the ID string breaks host compatibility** — saved sessions and
/// automation resolve the plugin by this ID, so `PluginInfo::vst3_id` is
/// forever once shipped.
#[must_use]
pub fn tuid_bytes(id: &str) -> [u8; 16] {
    let a = fnv1a(FNV_OFFSET, id.as_bytes());
    // Second pass: offset basis with rotated halves, so the two 64-bit
    // lanes are independent functions of the input.
    let b = fnv1a(FNV_OFFSET.rotate_left(32) ^ 0x9e37_79b9_7f4a_7c15, id.as_bytes());
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&a.to_le_bytes());
    out[8..].copy_from_slice(&b.to_le_bytes());
    out
}

/// Class TUID for `L`, derived from `vst3_id` (fallback `clap_id`).
/// Cached per monomorphization.
fn class_id<L: PluginLogic>() -> TUID {
    static CELL: OnceLock<TUID> = OnceLock::new();
    *CELL.get_or_init(|| {
        let info = L::info();
        let id = if info.vst3_id.is_empty() {
            info.clap_id
        } else {
            info.vst3_id
        };
        tuid_bytes(id).map(|b| b as c_char)
    })
}

// ---------------------------------------------------------------------------
// String helpers (char8 = ASCII-ish, String128 = UTF-16)
// ---------------------------------------------------------------------------

/// Copy `s` into a fixed `char8` field, NUL-padded, truncated to fit.
fn write_char8(dst: &mut [char8], s: &str) {
    dst.fill(0);
    let n = s.len().min(dst.len().saturating_sub(1));
    for (d, b) in dst.iter_mut().zip(s.as_bytes()).take(n) {
        *d = *b as char8;
    }
}

/// Copy `s` into a `String128` as UTF-16, NUL-padded, truncated to 127 units.
fn write_string128(dst: &mut String128, s: &str) {
    dst.fill(0);
    for (d, u) in dst.iter_mut().zip(s.encode_utf16().take(127)) {
        *d = u;
    }
}

/// Read a NUL-terminated host UTF-16 string (bounded).
unsafe fn read_tchar(ptr: *const TChar) -> String {
    const MAX: usize = 4096;
    let mut units = Vec::new();
    for i in 0..MAX {
        // SAFETY: caller guarantees a valid NUL-terminated string; the bound
        // caps damage from a misbehaving host.
        let u = unsafe { *ptr.add(i) };
        if u == 0 {
            break;
        }
        units.push(u);
    }
    String::from_utf16_lossy(&units)
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

struct Factory<L: PluginLogic>(PhantomData<fn() -> L>);

impl<L: PluginLogic> Class for Factory<L> {
    type Interfaces = (IPluginFactory, IPluginFactory2);
}

impl<L: PluginLogic> IPluginFactoryTrait for Factory<L> {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        if info.is_null() {
            return kInvalidArgument;
        }
        let meta = L::info();
        // SAFETY: non-null, host-owned out struct per spec.
        let info = unsafe { &mut *info };
        write_char8(&mut info.vendor, meta.vendor);
        write_char8(&mut info.url, meta.url);
        write_char8(&mut info.email, "");
        info.flags = vst3::Steinberg::PFactoryInfo_::FactoryFlags_::kUnicode as int32;
        kResultOk
    }

    unsafe fn countClasses(&self) -> int32 {
        1
    }

    unsafe fn getClassInfo(&self, index: int32, info: *mut PClassInfo) -> tresult {
        if index != 0 || info.is_null() {
            return kInvalidArgument;
        }
        let meta = L::info();
        // SAFETY: non-null, host-owned out struct per spec.
        let info = unsafe { &mut *info };
        info.cid = class_id::<L>();
        info.cardinality =
            vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances as int32;
        write_char8(&mut info.category, "Audio Module Class");
        write_char8(&mut info.name, meta.name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null out pointer per spec.
        unsafe { *obj = ptr::null_mut() };
        // SAFETY: FIDString here carries a 16-byte TUID per spec.
        let cid = unsafe { &*(cid as *const TUID) };
        if *cid != class_id::<L>() {
            return kInvalidArgument;
        }
        // SAFETY: same as above.
        let iid = unsafe { &*(iid as *const TUID) };

        // Single-component model: the same object serves IComponent and
        // IEditController. Fresh instance per call.
        let wrapper = ComWrapper::new(Component::<L>::new());
        let ptr: *mut c_void = if *iid == IComponent_iid {
            wrapper
                .to_com_ptr::<IComponent>()
                .map_or(ptr::null_mut(), |p| ComPtr::into_raw(p).cast())
        } else if *iid == IEditController_iid {
            wrapper
                .to_com_ptr::<IEditController>()
                .map_or(ptr::null_mut(), |p| ComPtr::into_raw(p).cast())
        } else if *iid == IAudioProcessor_iid {
            wrapper
                .to_com_ptr::<IAudioProcessor>()
                .map_or(ptr::null_mut(), |p| ComPtr::into_raw(p).cast())
        } else {
            return kNoInterface;
        };
        // SAFETY: non-null out pointer per spec.
        unsafe { *obj = ptr };
        kResultOk
    }
}

impl<L: PluginLogic> IPluginFactory2Trait for Factory<L> {
    unsafe fn getClassInfo2(&self, index: int32, info: *mut PClassInfo2) -> tresult {
        if index != 0 || info.is_null() {
            return kInvalidArgument;
        }
        let meta = L::info();
        // SAFETY: non-null, host-owned out struct per spec.
        let info = unsafe { &mut *info };
        info.cid = class_id::<L>();
        info.cardinality =
            vst3::Steinberg::PClassInfo_::ClassCardinality_::kManyInstances as int32;
        write_char8(&mut info.category, "Audio Module Class");
        write_char8(&mut info.name, meta.name);
        info.classFlags = 0;
        write_char8(
            &mut info.subCategories,
            match meta.category {
                PluginCategory::Effect => "Fx",
                PluginCategory::Analyzer => "Fx|Analyzer",
                PluginCategory::Instrument => "Instrument",
                PluginCategory::NoteEffect => "Fx|Note Expression",
            },
        );
        write_char8(&mut info.vendor, meta.vendor);
        write_char8(&mut info.version, meta.version);
        write_char8(&mut info.sdkVersion, "VST 3.7");
        kResultOk
    }
}

// ---------------------------------------------------------------------------
// Component (IComponent + IAudioProcessor + IEditController, single object)
// ---------------------------------------------------------------------------

/// Runtime state behind a mutex: every COM method takes `&self`, so shared
/// mutable state needs interior mutability. Uncontended in practice (host
/// serializes audio-thread vs main-thread calls per spec).
struct Component<L: PluginLogic> {
    params: Arc<L::Params>,
    /// Shared with `IPlugView` (editor open/close, bridge, sample rate).
    gui: Arc<GuiState>,
    inner: Mutex<Inner<L>>,
}

struct Inner<L: PluginLogic> {
    state: Option<L::DspState>,
    sample_rate: f64,
    max_samples: usize,
    process_mode: ProcessMode,
    active: bool,
    processing: bool,
    /// Index into `L::bus_layouts()` last accepted by `setBusArrangements`.
    layout_index: usize,
}

impl<L: PluginLogic> Component<L> {
    fn new() -> Self {
        let params = Arc::new(L::Params::default());
        let gui = GuiState::new(Arc::clone(&params) as Arc<dyn Params>);
        Self {
            params,
            gui,
            inner: Mutex::new(Inner {
                state: None,
                sample_rate: 44_100.0,
                max_samples: 0,
                process_mode: ProcessMode::Realtime,
                active: false,
                processing: false,
                layout_index: 0,
            }),
        }
    }

    fn selected_layout(&self) -> BusLayout {
        let layouts = L::bus_layouts();
        let idx = self.lock().layout_index;
        layout_at(&layouts, idx)
    }

    /// Max main-bus channel count across all declared layouts (`BusInfo` report).
    fn max_main_channels(is_input: bool) -> i32 {
        let layouts = L::bus_layouts();
        let n = layouts
            .iter()
            .map(|l| {
                if is_input {
                    l.main_input_channels()
                } else {
                    l.main_output_channels()
                }
            })
            .max()
            .unwrap_or(2);
        n as i32
    }

    fn lock(&self) -> MutexGuard<'_, Inner<L>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn find_info(&self, id: u32) -> Option<ParamInfo> {
        self.params.param_infos().into_iter().find(|p| p.id == id)
    }

    unsafe fn apply_param_changes(&self, changes: *mut IParameterChanges) {
        // SAFETY: host guarantees `changes` is valid for this process call.
        let Some(changes) = (unsafe { ComRef::from_raw(changes) }) else {
            return;
        };
        let count = unsafe { changes.getParameterCount() };
        for i in 0..count {
            let queue = unsafe { changes.getParameterData(i) };
            // SAFETY: queue pointers from the host are valid for this call.
            let Some(queue) = (unsafe { ComRef::from_raw(queue) }) else {
                continue;
            };
            let id = unsafe { queue.getParameterId() };
            let points = unsafe { queue.getPointCount() };
            if points <= 0 {
                continue;
            }
            let mut sample_offset: int32 = 0;
            let mut value: ParamValue = 0.0;
            // Block-accurate application: the last point in the queue wins.
            let ok = unsafe { queue.getPoint(points - 1, &mut sample_offset, &mut value) };
            if ok == kResultOk {
                self.params.set_normalized(id, value);
            }
        }
    }

    unsafe fn process_audio(&self, data: *mut ProcessData) -> tresult {
        // Author `process` must not unwind across the COM ABI.
        host_callback_with("VST3", "process", kInternalError, || {
            if data.is_null() {
                return kInvalidArgument;
            }
            // SAFETY: non-null; valid for the duration of the process call.
            let data = unsafe { &*data };
            let frames = data.numSamples.max(0) as usize;

            if !data.inputParameterChanges.is_null() {
                unsafe { self.apply_param_changes(data.inputParameterChanges) };
            }

            let transport = if data.processContext.is_null() {
                None
            } else {
                // SAFETY: non-null; valid for the duration of the process call.
                Some(map_transport(unsafe { &*data.processContext }))
            };

            if frames == 0 {
                return kResultOk;
            }

            let mut inner = self.lock();
            if !inner.active || !inner.processing {
                return kResultOk;
            }
            if data.symbolicSampleSize != kSample32 as int32 {
                return kResultFalse;
            }
            let sample_rate = inner.sample_rate;
            let process_mode = inner.process_mode;
            let Some(state) = inner.state.as_mut() else {
                return kResultOk;
            };

            let out_bus = if data.numOutputs > 0 && !data.outputs.is_null() {
                // SAFETY: non-null; host-owned bus array, valid for this call.
                unsafe { &*data.outputs }
            } else {
                return kResultOk;
            };
            let ch_out = out_bus.numChannels.max(0) as usize;
            if ch_out == 0 {
                return kResultOk;
            }
            // SAFETY: union field valid because symbolicSampleSize == kSample32.
            let out_ptrs = unsafe { out_bus.__field0.channelBuffers32 };
            if out_ptrs.is_null() {
                return kResultOk;
            }
            let in_bus = if data.numInputs > 0 && !data.inputs.is_null() {
                // SAFETY: non-null; host-owned bus array, valid for this call.
                Some(unsafe { &*data.inputs })
            } else {
                None
            };

            // Own channel buffers for the block (safe AudioBuffer construction).
            let mut owned_in: Vec<Vec<f32>> = Vec::with_capacity(ch_out);
            let mut owned_out: Vec<Vec<f32>> = Vec::with_capacity(ch_out);
            let mut host_out: Vec<*mut f32> = Vec::with_capacity(ch_out);

            for c in 0..ch_out {
                // SAFETY: `c < ch_out` channels, per AudioBusBuffers contract.
                let op = unsafe { *out_ptrs.add(c) };
                if op.is_null() {
                    return kResultFalse;
                }
                host_out.push(op);

                let in_data = in_bus
                    .filter(|b| (c as int32) < b.numChannels)
                    .and_then(|b| {
                        // SAFETY: union field valid for kSample32.
                        let ptrs = unsafe { b.__field0.channelBuffers32 };
                        if ptrs.is_null() {
                            return None;
                        }
                        // SAFETY: `c < numChannels` per the filter above.
                        let ip = unsafe { *ptrs.add(c) };
                        // SAFETY: host channel buffers hold at least `frames`.
                        (!ip.is_null()).then(|| unsafe { std::slice::from_raw_parts(ip, frames) })
                    });

                if let Some(s) = in_data {
                    owned_in.push(s.to_vec());
                    owned_out.push(s.to_vec());
                } else {
                    owned_in.push(vec![0.0; frames]);
                    owned_out.push(vec![0.0; frames]);
                }
            }

            let in_refs: Vec<&[f32]> = owned_in.iter().map(Vec::as_slice).collect();
            let mut out_refs: Vec<&mut [f32]> =
                owned_out.iter_mut().map(Vec::as_mut_slice).collect();

            {
                let mut buffer = AudioBuffer::from_slices_checked(&in_refs, &mut out_refs, frames);
                let mut ctx =
                    ProcessContext::new(sample_rate, frames).with_process_mode(process_mode);
                ctx.transport = transport;
                // ProcessStatus has no VST3 per-block equivalent; drop it.
                let _ = L::process(state, &self.params, &mut buffer, &mut ctx);
            }

            for (c, host_ptr) in host_out.iter().enumerate() {
                // SAFETY: host channel buffers hold at least `frames`.
                let dst = unsafe { std::slice::from_raw_parts_mut(*host_ptr, frames) };
                dst.copy_from_slice(&owned_out[c]);
            }
            kResultOk
        })
    }
}

impl<L: PluginLogic> Class for Component<L> {
    type Interfaces = (IComponent, IAudioProcessor, IEditController);
}

impl<L: PluginLogic> IPluginBaseTrait for Component<L> {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        // Main-thread GUI factory — same timing as CLAP `plugin_init`.
        let editor = L::editor(Arc::clone(&self.params));
        *self.gui.editor.lock().unwrap_or_else(PoisonError::into_inner) = editor;
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        if let Some(editor) = self
            .gui
            .editor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            editor.close();
        }
        *self.gui.editor.lock().unwrap_or_else(PoisonError::into_inner) = None;
        *self
            .gui
            .handler
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        *self.gui.frame.lock().unwrap_or_else(PoisonError::into_inner) = None;
        kResultOk
    }
}

impl<L: PluginLogic> IComponentTrait for Component<L> {
    unsafe fn getControllerClassId(&self, classId: *mut TUID) -> tresult {
        if classId.is_null() {
            return kInvalidArgument;
        }
        // Single component: controller CID == component CID.
        // SAFETY: non-null, host provides 16 writable bytes per spec.
        unsafe { *classId = class_id::<L>() };
        kResultOk
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        // kSimple / kAdvanced / kOfflineProcessing all accepted, no behavior change.
        kResultOk
    }

    unsafe fn getBusCount(&self, r#type: MediaType, dir: BusDirection) -> int32 {
        if r#type != kAudio as MediaType {
            return 0;
        }
        // Main bus only. No input bus when all layouts are output-only.
        if dir == kInput as BusDirection {
            let has_in = L::bus_layouts()
                .iter()
                .any(|l| l.main_in.is_some());
            i32::from(has_in)
        } else {
            1
        }
    }

    unsafe fn getBusInfo(
        &self,
        r#type: MediaType,
        dir: BusDirection,
        index: int32,
        bus: *mut BusInfo,
    ) -> tresult {
        if bus.is_null() {
            return kInvalidArgument;
        }
        if r#type != kAudio as MediaType
            || index != 0
            || (dir != kInput as BusDirection && dir != kOutput as BusDirection)
        {
            return kInvalidArgument;
        }
        if dir == kInput as BusDirection && Self::max_main_channels(true) == 0 {
            return kInvalidArgument;
        }
        // SAFETY: non-null, host-owned out struct per spec.
        let bus = unsafe { &mut *bus };
        bus.mediaType = kAudio as MediaType;
        bus.direction = dir;
        bus.channelCount = Self::max_main_channels(dir == kInput as BusDirection);
        write_string128(
            &mut bus.name,
            if dir == kInput as BusDirection {
                "Input"
            } else {
                "Output"
            },
        );
        bus.busType = kMain as BusType;
        bus.flags = kDefaultActive as uint32;
        kResultOk
    }

    unsafe fn getRoutingInfo(
        &self,
        _inInfo: *mut RoutingInfo,
        _outInfo: *mut RoutingInfo,
    ) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(
        &self,
        _type: MediaType,
        _dir: BusDirection,
        _index: int32,
        _state: TBool,
    ) -> tresult {
        kResultOk
    }

    unsafe fn setActive(&self, state: TBool) -> tresult {
        let mut inner = self.lock();
        if state != 0 {
            if inner.active {
                return kResultOk;
            }
            let layout = layout_at(&L::bus_layouts(), inner.layout_index);
            let config = AudioConfig::new(inner.sample_rate, inner.max_samples)
                .with_process_mode(inner.process_mode)
                .with_channels(layout.main_input_channels(), layout.main_output_channels());
            let mut dsp = L::init(&self.params, inner.sample_rate);
            self.params.set_sample_rate(inner.sample_rate);
            L::reset(&mut dsp, &self.params, &config);
            inner.state = Some(dsp);
            inner.active = true;
        } else {
            inner.active = false;
            inner.processing = false;
            inner.state = None;
        }
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        load_state(&*self.params, state)
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        save_state(&*self.params, state)
    }
}

impl<L: PluginLogic> IAudioProcessorTrait for Component<L> {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        numIns: int32,
        outputs: *mut SpeakerArrangement,
        numOuts: int32,
    ) -> tresult {
        if numOuts != 1 || outputs.is_null() {
            return kResultFalse;
        }
        // SAFETY: host arrays sized by numIns/numOuts.
        let out_arr = unsafe { *outputs };
        let out_ch = speaker_channel_count(out_arr);
        let in_ch = if numIns == 0 {
            0
        } else if numIns == 1 && !inputs.is_null() {
            speaker_channel_count(unsafe { *inputs })
        } else {
            return kResultFalse;
        };

        let layouts = L::bus_layouts();
        let Some(idx) = layouts.iter().position(|l| {
            l.main_input_channels() == in_ch && l.main_output_channels() == out_ch
        }) else {
            return kResultFalse;
        };
        self.lock().layout_index = idx;
        kResultOk
    }

    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: int32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if index != 0 || arr.is_null() {
            return kInvalidArgument;
        }
        let layout = self.selected_layout();
        let channels = if dir == kInput as BusDirection {
            layout.main_input_channels()
        } else {
            layout.main_output_channels()
        };
        let Some(sa) = arrangement_for_channels(channels) else {
            return kInvalidArgument;
        };
        // SAFETY: non-null, host-owned out value per spec.
        unsafe { *arr = sa };
        kResultOk
    }

    unsafe fn canProcessSampleSize(&self, symbolicSampleSize: int32) -> tresult {
        if symbolicSampleSize == kSample32 as int32 {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn getLatencySamples(&self) -> uint32 {
        let inner = self.lock();
        inner
            .state
            .as_ref()
            .map_or(0, |s| L::latency(s))
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        if setup.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: non-null, host-owned struct per spec.
        let setup = unsafe { &*setup };
        if setup.symbolicSampleSize != kSample32 as int32 {
            return kResultFalse;
        }
        {
            let mut inner = self.lock();
            inner.sample_rate = setup.sampleRate;
            inner.max_samples = setup.maxSamplesPerBlock.max(0) as usize;
            inner.process_mode = map_process_mode(setup.processMode);
        }
        self.params.set_sample_rate(setup.sampleRate);
        self.gui.set_sample_rate(setup.sampleRate);
        kResultOk
    }

    unsafe fn setProcessing(&self, state: TBool) -> tresult {
        self.lock().processing = state != 0;
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        unsafe { self.process_audio(data) }
    }

    unsafe fn getTailSamples(&self) -> uint32 {
        0
    }
}

impl<L: PluginLogic> IEditControllerTrait for Component<L> {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        // Single component: the component state IS the param state.
        load_state(&*self.params, state)
    }

    unsafe fn setState(&self, _state: *mut IBStream) -> tresult {
        // No controller-only state beyond params.
        kResultOk
    }

    unsafe fn getState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getParameterCount(&self) -> int32 {
        self.params.count() as int32
    }

    unsafe fn getParameterInfo(&self, paramIndex: int32, info: *mut ParameterInfo) -> tresult {
        if info.is_null() || paramIndex < 0 {
            return kInvalidArgument;
        }
        let infos = self.params.param_infos();
        let Some(meta) = infos.get(paramIndex as usize) else {
            return kInvalidArgument;
        };
        // SAFETY: non-null, host-owned out struct per spec.
        fill_parameter_info(meta, unsafe { &mut *info });
        kResultOk
    }

    unsafe fn getParamStringByValue(
        &self,
        id: ParamID,
        valueNormalized: ParamValue,
        string: *mut String128,
    ) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let Some(meta) = self.find_info(id) else {
            return kInvalidArgument;
        };
        let plain = meta.range.denormalize(valueNormalized);
        let text = self
            .params
            .format_value(id, plain)
            .unwrap_or_else(|| format!("{plain:.2}"));
        // SAFETY: non-null, host-owned out buffer per spec.
        write_string128(unsafe { &mut *string }, &text);
        kResultOk
    }

    unsafe fn getParamValueByString(
        &self,
        id: ParamID,
        string: *mut TChar,
        valueNormalized: *mut ParamValue,
    ) -> tresult {
        if string.is_null() || valueNormalized.is_null() {
            return kInvalidArgument;
        }
        let Some(meta) = self.find_info(id) else {
            return kInvalidArgument;
        };
        // SAFETY: non-null NUL-terminated host string per spec.
        let text = unsafe { read_tchar(string) };
        let Some(plain) = self.params.parse_value(id, &text) else {
            return kResultFalse;
        };
        // SAFETY: non-null, host-owned out value per spec.
        unsafe { *valueNormalized = meta.range.normalize(plain) };
        kResultOk
    }

    unsafe fn normalizedParamToPlain(&self, id: ParamID, valueNormalized: ParamValue) -> ParamValue {
        self.find_info(id).map_or(valueNormalized, |meta| {
            meta.range.denormalize(valueNormalized)
        })
    }

    unsafe fn plainParamToNormalized(&self, id: ParamID, plainValue: ParamValue) -> ParamValue {
        self.find_info(id)
            .map_or(plainValue, |meta| meta.range.normalize(plainValue))
    }

    unsafe fn getParamNormalized(&self, id: ParamID) -> ParamValue {
        self.params.get_normalized(id).unwrap_or(0.0)
    }

    unsafe fn setParamNormalized(&self, id: ParamID, value: ParamValue) -> tresult {
        self.params.set_normalized_returning_normalized(id, value);
        kResultOk
    }

    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        *self
            .gui
            .handler
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = if handler.is_null() {
            None
        } else {
            Some(handler)
        };
        kResultOk
    }

    unsafe fn createView(&self, name: FIDString) -> *mut IPlugView {
        // Spec: primary view is named "editor". Empty/null accepted as same.
        if !name.is_null() {
            // SAFETY: host C string for the view name.
            let name = unsafe { CStr::from_ptr(name) };
            if !name.to_bytes().is_empty() && name.to_bytes() != b"editor" {
                return ptr::null_mut();
            }
        }
        let has_editor = self
            .gui
            .editor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some();
        if !has_editor {
            return ptr::null_mut();
        }
        gui::PlugView::create(Arc::clone(&self.gui))
    }
}

// ---------------------------------------------------------------------------
// Bus / speaker helpers
// ---------------------------------------------------------------------------

fn speaker_channel_count(arr: SpeakerArrangement) -> u32 {
    use vst3::Steinberg::Vst::SpeakerArr::{kMono, kStereo};
    if arr == kMono {
        1
    } else if arr == kStereo {
        2
    } else {
        // Fall back to popcount of speaker bits for exotic layouts we reject.
        arr.count_ones()
    }
}

fn arrangement_for_channels(n: u32) -> Option<SpeakerArrangement> {
    use vst3::Steinberg::Vst::SpeakerArr::{kMono, kStereo};
    match n {
        1 => Some(kMono),
        2 => Some(kStereo),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Param / state / transport helpers
// ---------------------------------------------------------------------------

fn fill_parameter_info(meta: &ParamInfo, out: &mut ParameterInfo) {
    out.id = meta.id;
    write_string128(&mut out.title, meta.name);
    write_string128(&mut out.shortTitle, meta.short_name);
    write_string128(&mut out.units, meta.unit.as_str());
    let stepped = matches!(
        meta.kind,
        ParamValueKind::Int | ParamValueKind::Bool | ParamValueKind::Enum
    );
    out.stepCount = if stepped {
        meta.range.step_count().map_or(
            // Bool fallback when the range isn't discrete.
            int32::from(meta.kind == ParamValueKind::Bool),
            |n| n.get() as int32,
        )
    } else {
        0
    };
    out.defaultNormalizedValue = meta.range.normalize(meta.default_plain);
    out.unitId = kRootUnitId;
    let mut flags: int32 = 0;
    if meta.flags.contains(ParamFlags::AUTOMATABLE) {
        flags |= kCanAutomate;
    }
    if meta.flags.contains(ParamFlags::HIDDEN) {
        flags |= kIsHidden;
    }
    if meta.flags.contains(ParamFlags::READONLY) {
        flags |= kIsReadOnly;
    }
    if meta.flags.contains(ParamFlags::IS_BYPASS) {
        flags |= kIsBypass;
    }
    if meta.kind == ParamValueKind::Enum {
        flags |= kIsList;
    }
    out.flags = flags;
}

fn map_process_mode(mode: int32) -> ProcessMode {
    if mode == kPrefetch as int32 {
        ProcessMode::Buffered
    } else if mode == kOffline as int32 {
        ProcessMode::Offline
    } else {
        ProcessMode::Realtime
    }
}

fn map_transport(ctx: &vst3::Steinberg::Vst::ProcessContext) -> Transport {
    let s = ctx.state;
    let has = |flag: u32| s & flag != 0;
    let time_sig = has(kTimeSigValid as u32)
        .then_some((ctx.timeSigNumerator as u16, ctx.timeSigDenominator as u16));
    let bar_number = if has(kBarPositionValid as u32)
        && let Some((num, den)) = time_sig
        && den > 0
    {
        let quarters_per_bar = f64::from(num) * 4.0 / f64::from(den);
        Some((ctx.barPositionMusic / quarters_per_bar).floor() as i32)
    } else {
        None
    };
    Transport {
        playing: has(kPlaying as u32),
        recording: has(kRecording as u32),
        loop_active: has(kCycleActive as u32),
        tempo: has(kTempoValid as u32).then_some(ctx.tempo),
        position_beats: has(kProjectTimeMusicValid as u32).then_some(ctx.projectTimeMusic),
        position_seconds: (ctx.sampleRate > 0.0)
            .then_some(ctx.projectTimeSamples as f64 / ctx.sampleRate),
        loop_beats: has(kCycleValid as u32).then_some((ctx.cycleStartMusic, ctx.cycleEndMusic)),
        time_signature: time_sig,
        bar_number,
    }
}

/// Read a host stream to EOF.
unsafe fn read_stream(stream: *mut IBStream) -> Option<Vec<u8>> {
    // SAFETY: host passes a valid IBStream; ComRef does not touch the refcount.
    let stream = unsafe { ComRef::from_raw(stream) }?;
    let mut blob = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let mut num_read: int32 = 0;
        // SAFETY: `chunk` is a valid writable buffer of `chunk.len()` bytes.
        let result = unsafe {
            stream.read(
                chunk.as_mut_ptr().cast::<c_void>(),
                chunk.len() as int32,
                &mut num_read,
            )
        };
        if result != kResultOk || num_read <= 0 {
            break;
        }
        blob.extend_from_slice(&chunk[..num_read as usize]);
        if (num_read as usize) < chunk.len() {
            break;
        }
    }
    Some(blob)
}

fn load_state(params: &dyn Params, stream: *mut IBStream) -> tresult {
    host_callback_with("VST3", "state_load", kInternalError, || load_state_inner(params, stream))
}

fn load_state_inner(params: &dyn Params, stream: *mut IBStream) -> tresult {
    let Some(blob) = (unsafe { read_stream(stream) }) else {
        return kInvalidArgument;
    };
    if decode_state(params, &blob) {
        kResultOk
    } else {
        kResultFalse
    }
}

fn save_state(params: &dyn Params, stream: *mut IBStream) -> tresult {
    host_callback_with("VST3", "state_save", kInternalError, || {
        // SAFETY: host passes a valid IBStream; ComRef does not touch the refcount.
        let Some(stream) = (unsafe { ComRef::from_raw(stream) }) else {
            return kInvalidArgument;
        };
        let blob = encode_state(params);
        let mut written: int32 = 0;
        // SAFETY: `blob` is a valid readable buffer of `blob.len()` bytes; the
        // stream only reads from it.
        let result = unsafe {
            stream.write(
                blob.as_ptr().cast_mut().cast::<c_void>(),
                blob.len() as int32,
                &mut written,
            )
        };
        if result == kResultOk && written as usize == blob.len() {
            kResultOk
        } else {
            kInternalError
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::tuid_bytes;

    #[test]
    fn tuid_deterministic() {
        assert_eq!(
            tuid_bytes("com.lxaudiolabs.smoke-gain"),
            tuid_bytes("com.lxaudiolabs.smoke-gain")
        );
    }

    #[test]
    fn tuid_differs_for_different_strings() {
        assert_ne!(
            tuid_bytes("com.lxaudiolabs.gain"),
            tuid_bytes("com.lxaudiolabs.eq")
        );
        // Never the all-zero TUID for a non-empty string.
        assert!(tuid_bytes("com.lxaudiolabs.gain").iter().any(|b| *b != 0));
    }
}
