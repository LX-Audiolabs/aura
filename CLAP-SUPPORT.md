# CLAP support matrix

Implementation snapshot: **AURA 0.15.0 development**, checked **2026-11-28**.
Published author API remains 0.14.6 until release. This page describes the
checkout; it is not a certification of every extension or host.

Scope: the stable and draft extension modules shipped by the locked
`clap-sys 0.6.0` bindings (CLAP 1.2.10), plus factories and core events.
The upstream [CLAP headers](https://github.com/free-audio/clap/tree/1.2.10/include/clap)
define the ABI; bindings being present does not mean AURA implements an extension.
New upstream extensions require a fresh comparison.

**Supported** means implemented for the stated author surface. **Partial** means
implemented with the listed restrictions. **Not implemented** means no wrapper
exposure/host integration. **Planned** is reserved for committed work; no absent
extension is promised simply to complete this matrix. None of these labels alone
means tested in a DAW.

## Plugin extensions

Exposure is determined by [plugin_get_extension](crates/aura-clap/src/lib.rs)
and [PluginLogic](crates/aura-core/src/plugin.rs). Unless specified otherwise,
named hooks belong to `PluginLogic`.

| Extension | Status | Author surface and limits | Local evidence |
|-----------|--------|---------------------------|----------------|
| audio-ports | Partial | `bus_layouts()`: mono/stereo main, one optional sidechain, one aux; not arbitrary bus graphs | Layout unit test; smoke plugins |
| audio-ports-config | Partial | Exposed only for multiple declared layouts; LV2/VST3 rules differ | Source; validator layout tests where applicable |
| params | Supported | Stable numeric IDs, value/text conversion, flush/gestures, CHUNKED automation; mono/per-note modulation handled separately | Validator parameter cases; notes/poly-mod tests |
| remote-controls | Supported | Group-derived pages or `remote_control_pages()`; eight slots per page, excess explicit IDs dropped | Eight page-generation/curation tests |
| latency | Supported | `latency(&state)`, cached samples, host notification/restart on change | Cache unit test; non-zero DAW PDC not freshly verified |
| tail | Supported | `tail_length(&state)`, cached sample count | Source; dedicated host-tail proof not recorded |
| render | Supported | `ProcessContext.process_mode`, realtime/offline; no hard-realtime requirement advertised | Render-mode round-trip test |
| note-ports | Partial | `accepts_midi_in` / `emits_midi`; at most one input and one output, both port ID 0 | Dialect matrix and note-output tests |
| voice-info | Supported | Exposed only when `voice_count > 0`; capacity at least count; overlapping notes advertised | Voice-info unit test |
| note-name | Supported | Non-empty `note_names()`; wildcard fields supported | Source; synth example; dedicated DAW proof not recorded |
| state | Supported | Project context through shared codec/custom hooks; short stream reads/writes handled | Four state hook tests; validator reproducibility cases |
| state-context/2 | Supported | Opt-in `state_context_supported()`; Project/Preset/Duplicate passed to hooks | Dispatch source; no duplicate/project-reload host matrix yet |
| preset-load/2 (+ compat ID) | Supported | FILE via `load_preset_from_file`; PLUGIN via factory key; hook errors propagated | Factory-blob hook test; validator preset cases |
| gui | Partial | Only when editor exists; embedded Win32/Cocoa/X11, scale/resize via editor; floating windows rejected | Scale tests; historical UI smoke; no fresh cross-platform GUI test |
| tuning/2 (draft) | Partial | Opt-in `supports_tuning`; host tuning pool/event registry, offsets and filtering; needs a cooperating host | Implementation in [tuning.rs](crates/aura-clap/src/tuning.rs); dedicated host proof not recorded |
| param-indication (+ compat ID) | Supported | Main-thread `on_param_mapping` / `on_param_automation`; product supplies its UI state | Callback source; dedicated host proof not recorded |
| context-menu (+ compat ID) | Partial | Opt-in plugin action entries for parameter/global targets; no general menu-tree author model | Three context-menu builder/target tests |

State support does not imply bounded input loading or arbitrary product-schema
migration. The CLAP stream load still grows a `Vec` until EOF; load limits and
fuzzing remain backlog work. Threading/realtime guarantees are also separate:
for example, the current smoke-gain DSP copies channels with `to_vec()`, so a
passing validator run is not proof of allocation-free processing.

## Factories, events, and host-side integration

| Capability | Status | Limits / evidence |
|------------|--------|-------------------|
| Plugin factory and entry | Supported | One plugin descriptor per export; factory validation exercises enumeration/creation and invalid IDs |
| Preset-discovery factory/2 (+ compat ID) | Partial | Non-empty `factory_presets()` enables bundled PLUGIN locations; not a filesystem preset catalog. [Source](crates/aura-clap/src/preset_load.rs) |
| Native notes, expressions, per-note modulation | Supported | `ProcessContext.notes` / `notes_out`; voice allocation/application remains product logic. NOTE_END output and mirror-priority tests exist |
| MIDI 1 / MIDI 2 | Partial | Raw MIDI/UMP and output queues; native MIDI 2 input/output tests exist. No arbitrary multi-port API; no full typed SysEx/Flex decode surface |
| Transport | Supported | Host timeline in `ProcessContext.transport`; validator null/fuzz/sample-accurate cases. This is core transport, not draft transport-control |
| Host GUI/params/latency/tail | Supported | Resize, edit gestures, parameter flush/rescan, latency/tail notification paths; require corresponding host callbacks |
| Host track-info (+ compat ID) | Partial | `EditorBridge::track_info()` reads optional name/color; absent/query failure returns None. Field-presence unit test |
| Host event-registry | Partial | Queries tuning's event-space ID only; not a general author event registry |

### Extensions without integration

The following binding modules have no corresponding plugin implementation or
host integration in this wrapper. They are **not implemented**, not implicitly planned:

| Binding modules | Status |
|-----------------|--------|
| `ambisonic`, `surround` | Not implemented |
| `audio_ports_activation`, `configurable_audio_ports` | Not implemented |
| `log`, `posix_fd_support`, `thread_check`, `thread_pool`, `timer_support` | Not implemented |
| Draft `background_activation`, `background_progress`, `background_state_context` | Not implemented |
| Draft `extensible_audio_ports`, `flush_events`, `scratch_memory` | Not implemented |
| Draft `gain_adjustment_metering`, `mini_curve_display`, `octave_number` | Not implemented |
| Draft `param_hovered`, `params_origin`, `project_location`, `resource_directory` | Not implemented |
| Draft `transport_control`, `triggers`, `undo`, `webview` | Not implemented |

Slint's internal window/event loop does not imply support for CLAP timer,
POSIX-FD or thread-pool extensions. Host track-info/event-registry are host
interfaces, so their absence from plugin_get_extension is not evidence of absence.

## Verification and host evidence

Local checks use Windows x86_64, Rust 1.99.1, debug smoke builds and
`clap-validator 0.4.2` through `cargo aura validate`. Installation source baseline:
maintainer fork revision `f806ebaa06c7179889778bfbd1ee98b92639715c`; the installed
binary's version does not independently attest to that commit.

The CLAP wrapper suite passes **32 unit tests** (one Rustdoc example remains
ignored). State tests are in [state_tests.rs](crates/aura-clap/src/state_tests.rs);
process status/denormal tests in [process_tests.rs](crates/aura-clap/src/process_tests.rs);
extension/helper tests in [lib.rs](crates/aura-clap/src/lib.rs).

Fresh full validator runs (2026-11-28), all with exit code 0:

| Plugin | Passed | Skipped | Failed |
|--------|-------:|--------:|-------:|
| smoke-gain | 35 | 9 | 0 |
| smoke-midi-fx | 35 | 9 | 0 |
| smoke-synth | 35 | 9 | 0 |
| smoke-sidechain | 32 | 12 | 0 |
| smoke-aux | 32 | 12 | 0 |

Each run reports 44 results across library and instance tests. Skips include
Unix-only loading on Windows, f64 processing, and absent extensions/dialects for
that plugin. A skipped test is not a pass; no test proves all combinations.
GUI lifecycle, every opt-in extension, realtime allocations, and real DAW
behavior are not established by these runs.

| Host | Loads | Automation/modulation | Save/reload | Multiple instances | Evidence |
|------|-------|-----------------------|-------------|--------------------|----------|
| Bitwig | Historical report | Historical mono/poly-mod, note expressions and note-FX; automation/PDC not fully recorded | Not freshly verified | Not freshly verified | Existing smoke checklist dated 2026-08-28; not rerun for 0.15.0 |
| REAPER | Not verified | Not verified | Not verified | Not verified | No recorded run for this snapshot |
| Ardour | Not verified | Not verified | Not verified | Not verified | No recorded run for this snapshot |
| Carla | Not verified | Not verified | Not verified | Not verified | No recorded run for this snapshot |
| Qtractor | Not verified | Not verified | Not verified | Not verified | No recorded run for this snapshot |

Host-specific note routing must be tested before documenting a quirk as fact.
AURA itself exposes port 0; that does not establish how every host maps notes.
Historical Bitwig observations are retained in the
[wrapper README](crates/aura-clap/README.md#host-proof-framework-landed-bitwig-re-check-2026-08-28).

## Recheck for each release

- Reconcile every extension branch and host query with this matrix.
- Run `cargo test -p lx-aura-clap`, then build and validate each smoke plugin.
- Record target, build profile, tool versions, pass/skip/failure counts and
  unresolved items. Keep external host evidence separate from automated tests.
- New extension exposure or changed limits require matching author docs/tests.

