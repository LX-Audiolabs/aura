---
id: clap
group: formats
summary: AURA CLAP path — aura::export!, thin aura-clap wrapper, validator, no-alloc process.
triggers: CLAP, clap-validator, plugin factory, export, aura-clap, clap.gui, clap.tuning
verify: cargo aura build --clap; clap-validator on built .clap; no alloc in process; is_floating=false
source: global
copied_by: template
date: 2026-08-27
adapted: true
reason: "AURA CLAP ship path — wrapper not raw clap_plugin_factory"
---

# CLAP (AURA)

**Summary:** Ship path for AURA plugins as `.clap`. Authors implement `PluginLogic`
and call `aura::export!`. `aura-clap` is the thin wrapper — do **not** register
`clap_plugin_factory` by hand.

## Author path

```toml
aura = { path = "...", features = ["clap"] }
```

```rust
#[cfg(feature = "clap")]
aura::export!(MyPlugin);
```

```bash
cargo aura build --clap -plug <name>
cargo aura install --clap --release -plug <name>
clap-validator validate path/to/<name>.clap
```

In-tree smokes (`smoke-gain`, `smoke-midi-fx`, `smoke-sidechain`, `smoke-synth`)
were **0 failed** on clap-validator (2026-08-11).

## What the wrapper already does

Entry, factory, audio-ports (mono/stereo + **one** optional sidechain),
audio-ports-config, note-ports, params (sample-accurate + mono/poly mod),
state, GUI, remote-controls, latency, tail, render, preset-load, tuning
(`clap.tuning/2`).

| Topic | Where |
|-------|--------|
| Layouts | `PluginLogic::bus_layouts()` — default stereo |
| Sample-accurate | `ParamFlags::CHUNKED` (default); opt out `#[param(chunk = false)]` |
| Mono mod | `#[param(flags = "modulatable")]` — `note_id < 0` |
| Poly mod | `modulatable_per_note` — `ProcessContext.notes` `ParamMod` |
| Notes / expr | prefer `MidiDialect::Clap`; on/off still mirrored to 7-bit `midi` |
| Note out | `context.notes_out`; `NOTE_END` when a voice goes silent |
| MIDI 2 | `ProcessContext.ump` / `ump_out`; `midi` is the 7-bit image |
| GUI | embed only — `is_floating = true` is rejected |
| Voices | `PluginInfo.voice_count` / `voice_capacity` → `clap.voice-info` |

## Realtime

`process` must not heap-alloc. Bitwig note-expression flood crashed the host
(2026-08-18). Scratch is reserved in `activate`; events capped at **4096**.
Plugin DSP must still preallocate (delay lines, voices) in `init`/`reset`.

Host panic fence (`catch_unwind`) wraps process + state in the wrapper.
`catch_unwind` around `Editor::close` cannot catch a Win32 `wnd_proc` abort —
see `aura-editor` / `aura-baseview` `ensure_current`.

## Do not

- Implement `clap_plugin_factory` / vtables in plugin code
- Invent extension IDs — copy free-audio names (`include/clap/`)
- Claim the full CLAP zoo (thread-pool, surround, context-menu, …) unless a
  product plugin needs it
- Return `is_floating = true` for AURA GUIs
- Shrink `ProcessContext` in other formats to “what CLAP does not need”

## Leftover (product-driven)

| Item | When |
|------|------|
| Multi-out / >1 sidechain | a plugin needs aux buses beyond one SC |
| G5 rich state | host blob larger than the flat param state |
| Typed SysEx/Flex | hardware bridge; raw packets already on `ump` |

Host proofs (chord, expressions, poly-mod, MIDI FX → synth, bounce):
`crates/aura-clap/README.md`.

## See also

- `02-frameworks/aura.md` — `PluginLogic` / `export!`
- `00-core/dsp-realtime.md` — no alloc/lock in `process`
- `04-ui/slint.md` — `AuraSlintEditor` / `clap.gui` embed
- `notes/aura-clap.md` — leftovers + Bitwig checklist
