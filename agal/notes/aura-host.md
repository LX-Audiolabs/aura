<!-- AGAL:AUTO-START -->
# aura-host

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-host` |
| description | Minimal CLAP host — CLI + Slint GUI, load .clap, params, MIDI in, run audio |
| frameworks | aura, clap, raw-window-handle, slint |
| generated | `2026-08-27T17:47:49Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-host
[ATOM] type=fact | detail=frameworks=aura+clap+raw-window-handle+slint
[ATOM] type=fact | detail=roles=build+entry+manifest+slint+source
[ATOM] type=fact | detail=has_process=true
[ATOM] type=fact | detail=depends_on=aura-build
```

## deps (workspace)
- `aura-build`

## structure
- process methods (DSP): 1
- public_api symbols: 43 (see json)
- roles: build, entry, manifest, slint, source

## api surface
- `struct Engine { … }` · `src/audio.rs`
- `struct Session { … }` · `src/audio.rs`
- `struct EvList { items: Vec<EvStorage> }` · `src/events.rs`
- `struct Loader { … }` · `src/loader.rs`
- … +41 more public symbols

## findings
- [info] **crate_no_dependents**: aura-host has no inbound workspace edges — unused or only path-included? · `crates/aura-host` · fix: wire `aura-host` as a path dep from a plugin/crate, or remove from workspace

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-host` in json / `agal --plugin aura-host .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

Host side of the CLAP FFI — AURA's answer to `free-audio/clap-host`. Everything
else in the workspace implements the *plugin* side; this crate is the mirror,
and the fast dev loop for testing plugins without a DAW. Plan:
[docs/aura-host-idea.md](../../docs/aura-host-idea.md).

Phase 1 (CLI: load, params, MIDI in, audio), Phase 2 (Slint shell: device
pick, param sliders, keyboard→notes, plugin GUI), and Phase 3 (Windows GUI
embed) are all done.

## Open

- [x] Audio *input* capture — default input device remixed into plugin input
      ports (silence fallback if open fails).
- [ ] Only `f32` cpal streams; other sample formats exit with an error.
- [ ] Param sliders poll `params.get_value` at 50 Hz instead of reading the
      plugin's output events — fine for UI, not for automation/recording.
- [ ] Keyboard→MIDI is notes-only (no CC/pitchbend); queued MIDI lands at
      frame 0 of the next block (`ponytail` in `audio.rs` / `events.rs`).
- [x] Plugin GUI embed — live OK with `smoke-gain` (2026-08-28). `smoke-synth`
      has **no** `editor` → “Open plugin GUI” stays disabled (not a host bug).
- [ ] Embed socket is placed at a fixed offset (`EMBED_X`/`EMBED_Y` in
      `gui.rs`) and never repositioned — resizing the host window doesn't move
      it, and a small window can clip it.
- [ ] macOS/Linux embed — Windows `win32_embed` only; floating fallback for
      third-party plugins that support it.

## Decisions

- Binary, not a library: no inbound workspace edges is expected here, so the
  `crate_no_dependents` finding above is noise for this crate.
- Raw `clap-sys` on the host side rather than reusing `aura-clap` — that crate
  is built around `PluginLogic`, the host has no such trait to hang things on.
- MIDI dialect is decided once from `note-ports.preferred_dialect`, not
  per-event: MIDI dialect → raw `CLAP_EVENT_MIDI`, otherwise notes are
  converted to `CLAP_EVENT_NOTE_ON/OFF` and CC/pitchbend are dropped.
- Floating plugin GUI (`plugin_gui.rs`) only serves third-party plugins —
  `aura-clap` answers `is_api_supported(is_floating=true)` with `false`
  ([crates/aura-clap/src/lib.rs:2204](../../crates/aura-clap/src/lib.rs:2204)),
  so AURA's own plugins are embed-only. `gui.rs`'s toggle button tries embed
  first (`win32_embed::supports_embedded`) and falls back to floating.
- Embed socket (`win32_embed.rs`) is a bare `WNDCLASSW` with `DefWindowProcW` —
  we don't paint or handle input in it ourselves, the plugin's own child window
  (created inside it via `set_parent`) does all of that. Sized to the plugin's
  `gui.get_size()` (fallback `400x300`), not to a host-chosen size.
- No `windows` crate — `windows-sys` only, version-pinned to match what
  `baseview` already resolves to (0.61.2) so it's not a second copy in the
  dependency tree.
- Main↔Audio queues are `Arc<crossbeam_queue::ArrayQueue>`, not SPSC: the GUI
  rebuilds the audio `Engine` on every device switch (drop + reopen), and the
  same queue instance has to survive that — an SPSC ring's producer/consumer
  split doesn't.

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision | detail=aura-host is a bin; crate_no_dependents finding is expected
[ATOM] type=decision | detail=host side uses raw clap-sys, not aura-clap (PluginLogic-shaped)
[ATOM] type=constraint | detail=one clap_audio_buffer per declared port, not per channel — smoke-sidechain declares in [2,1]
[ATOM] type=constraint | detail=params set via params.flush only while deactivated; live edits during playback go through the Main->Audio queue instead
[ATOM] type=lesson | detail=cpal callback must not allocate — buffers are preallocated for MAX_FRAMES=4096 and blocks are chunked
[ATOM] type=decision | detail=Main<->Audio queues are Arc<ArrayQueue>, not SPSC — must survive Engine rebuild on device switch
[ATOM] type=constraint | detail=floating plugin GUI only works for third-party plugins; aura-clap rejects is_floating=true
[ATOM] type=decision | detail=embed toggle tries win32_embed first, falls back to plugin_gui floating
[ATOM] type=lesson | detail=embed live-verified against smoke-gain/smoke-synth via HWND tree (EnumChildWindows), not by screenshot -- remote-desktop session made GetWindowRect/screenshots unreliable
```
