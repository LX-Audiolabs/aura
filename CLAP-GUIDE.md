# CLAP Guide for AURA

API baseline: **AURA 0.14.6** (published).
Repository workspace: **0.14.7**, with additional changes under `[Unreleased]` in
[CHANGELOG.md](CHANGELOG.md). Checked against both APIs on **2026-11-28**.

The versioned [API reference on docs.rs](https://docs.rs/lx-aura/0.14.6/aura/)
is canonical for the published API. This guide uses the same parameter macro and
`PluginLogic` surface as the [README](README.md). Repository examples follow the
checkout; they may include changes newer than the published release.

## Minimal CLAP plugin

Create a Rust library with this manifest:

```toml
[package]
name = "my-gain"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
lx-aura = { version = "=0.14.6", default-features = false, features = ["clap"] }
```

Put this complete, headless gain plugin in `src/lib.rs`:

```rust
use aura::prelude::*;

#[derive(Params)]
pub struct GainParams {
    #[param(id = 1, name = "Gain", range = "linear(-24, 24)", default = 0.0, unit = "db")]
    pub gain: FloatParam,
}

pub struct MyGain;

impl PluginLogic for MyGain {
    type Params = GainParams;
    type DspState = ();

    fn info() -> PluginInfo {
        let mut info = PluginInfo::new("My Gain", "Example", "0.1.0", "my-gain");
        info.clap_id = "com.example.my-gain";
        info
    }

    fn init(_params: &GainParams, _sample_rate: f64) {}

    fn reset(_state: &mut (), _params: &GainParams, _config: &AudioConfig) {}

    fn process(
        _state: &mut (),
        params: &GainParams,
        buffer: &mut AudioBuffer<'_, f32>,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let channels = buffer.num_inputs().min(buffer.num_outputs());
        for sample in 0..buffer.num_samples() {
            // Advance the smoother once per frame, shared by all channels.
            let gain = 10.0_f32.powf(params.gain.read() / 20.0);
            for channel in 0..channels {
                let input = buffer.input(channel)[sample];
                buffer.output(channel)[sample] = input * gain;
            }
        }
        for channel in channels..buffer.num_outputs() {
            buffer.output(channel).fill(0.0);
        }
        ProcessStatus::Continue
    }
}

aura::export!(MyGain);
```

Build with `cargo build --release`. The format wrapper owns the CLAP entry,
descriptor, callbacks, and extension registration. Authors implement
`PluginLogic`; they do not construct `clap_plugin_descriptor` or cast host
pointers to an AURA instance.

For scaffolding, installation, and UI setup, follow the README quickstart.
That workflow uses the cloned checkout through `AURA_PATH`, whereas the manifest
above deliberately targets the published release.

## Parameters and automation

Use `#[derive(Params)]`, `FloatParam` / `IntParam` / `BoolParam` / `EnumParam`,
and explicit numeric `#[param(id = N, ...)]` attributes. The derive supplies
`Default` and a typed parameter-ID enum (here `GainParamsParamId::Gain.id()`).
IDs are persistent host automation/state identifiers: never renumber them after shipping.

The prelude imports `FloatParamReadF32`. `gain.read()` advances its smoother
toward the effective target, including mono modulation; call it once per sample
frame, not once per channel. The minimal example defaults to no smoothing.
Choose a smoothing style and configure it for the sample rate in
a product; see the parameter API reference.

CLAP splits processing at sample offsets for parameters marked
`ParamFlags::CHUNKED` (default); `#[param(chunk = false)]` opts out.
Declare mono modulation with `flags = "modulatable"`; add
`modulatable_per_note` for per-note modulation. Per-note offsets arrive in
`ProcessContext.notes` and require voice-specific handling.

UI edits and host gestures go through the editor's `PluginContext` bridge.
Direct parameter writes alone do not describe a complete host automation gesture.
See [smoke-gain](examples/smoke-gain/src/lib.rs) for Slint integration and
the [editor bridge API](https://docs.rs/lx-aura-core/0.14.6/aura_core/editor/index.html)
for the host-facing contract.

## Slint GUI

Override `PluginLogic::editor(Arc<Self::Params>) -> Option<Box<dyn Editor>>`.
The supported stack is `aura_editor::AuraSlintEditor` over `aura-baseview`,
with `aura-build` compiling the Slint component. The default returns `None`.

Use [smoke-gain](examples/smoke-gain/) as the complete UI/build reference.
Host parenting, scale, and resize use the editor adapter; there is no author API
named `SlintParentedWindow::new` or `Instance::set_host_scale`.

Run `cargo aura preview` for UI work without loading a DAW. This previews the
component; it does not validate the CLAP wrapper or DSP.

## State and presets

The wrappers call `PluginLogic::save_state_with_context` and
`load_state_with_context`. Defaults use `aura::encode_state` /
`aura::decode_state`: an `AURA` magic header, version-2 envelope, parameter
values, and `#[persist]` data. The decoder also accepts legacy version-1
parameter blobs. Do not replace these with raw FFI stream callbacks.

`StateContext` distinguishes `Project`, `Preset`, and `Duplicate`.
Override `state_context_supported() -> bool` to opt into the CLAP state-context
extension. Custom codecs must validate their input and snap parameter smoothers
after restoring values.

`factory_presets()` supplies bundled `FactoryPreset` entries; non-empty lists
enable preset discovery. The preset-load path uses `load_preset_from_file`
for files and `apply_factory_preset` for factory entries. The existence of
state versioning does not imply arbitrary product-state migrations or bounded
host-stream loading; those require separate verification.

## Notes, MIDI, voices, and tuning

Declare note input/output through `PluginInfo.accepts_midi_in` and
`PluginInfo.emits_midi`. Current wrappers expose at most one input and one
output note port. There is no arbitrary multi-note-port author API.

Choose `midi_input_dialect` / `midi_output_dialect` with `MidiDialect`.
`Clap` prefers native CLAP notes; `Midi2` prefers MIDI 2.
`ProcessContext` provides incoming `notes`, `midi`, and `ump`, plus their
output counterparts. Preserve sample offsets and note identity when generating
events; `NoteVoiceTable` helps track overlapping notes and expressions.
`NOTE_END` reports voice completion, while generated notes need an output port.

Set `PluginInfo.voice_count` and `voice_capacity` to advertise voice info.
For host tuning, set `supports_tuning = true` and query `context.tuning`;
`PluginLogic::tuning_changed` handles tuning-pool notifications.

References: [smoke-synth](examples/smoke-synth/src/lib.rs) and
[smoke-midi-fx](examples/smoke-midi-fx/src/lib.rs). Host behavior needs separate
testing; implemented callbacks do not establish compatibility across all hosts.

## Other extension hooks

AURA implements a selected CLAP surface, not every CLAP extension.
These author hooks describe implementation entry points, not a host certification matrix:

| Capability | Author API / condition |
|------------|------------------------|
| Audio layouts | `bus_layouts()`; one optional sidechain and one aux output |
| Remote controls | Parameter groups or `remote_control_pages()`; not MIDI CC mapping |
| Latency | `latency(&state)` in samples |
| Tail | `tail_length(&state)` in samples |
| Offline rendering | `ProcessContext.process_mode` |
| Note names | Non-empty `note_names()` |
| Parameter indication | `on_param_mapping` / `on_param_automation` |
| Parameter context menus | `context_menu_supported()` and related callbacks |

See [aura-clap implementation notes](crates/aura-clap/README.md) and
`plugin_get_extension` in [the wrapper source](crates/aura-clap/src/lib.rs)
for extension exposure and conditions. Adding an extension is framework work:
implement the wrapper against the CLAP headers, expose the needed author hook,
and test it. There is no generic extension trait registration API.

## Realtime rules and validation

Allocate and prepare DSP storage during `init` / `reset`. In `process`, avoid
allocation/deallocation, locks, file I/O, formatting/logging, and GUI calls.
Publish bounded data for a non-audio thread to consume; UI drawing and logging
belong on that thread. An atomic flag does not make a GUI call realtime-safe.
These remain author responsibilities, not compile-time guarantees.

Validate an installed plugin locally:

```bash
clap-validator validate path/to/my-gain.clap
```

Maintainer fork: [lxndrbe/clap-validator](https://github.com/lxndrbe/clap-validator).
`cargo aura doctor` checks availability only. The `cargo aura validate`
wrapper is planned; it is not an existing command. Validator execution is not
required on every Forgejo push.

The example's source can be compiled independently; automated coverage of all
Markdown samples is a separate backlog item. A validator run also does not
replace state round-trip tests or real-host tests.

## Further reading

- [Published API reference (0.14.6)](https://docs.rs/lx-aura/0.14.6/aura/)
- [README and CLI quickstart](README.md)
- [CLAP specification](https://github.com/free-audio/clap)
- [Slint](https://slint.dev/)
