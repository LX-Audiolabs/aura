---
id: lv2
group: formats
summary: AURA LV2 path — TTL + lv2_descriptor; UI via shared Editor when editor() is Some.
triggers: LV2, lv2, TTL, lv2ui, jalv, Carla, aura-lv2, lv2_uri
verify: lv2_uri matches TTL; cargo aura build --lv2; UI triples only when editor() is Some
source: global
copied_by: template
date: 2026-08-27
adapted: true
reason: "AURA LV2 wrapper + UI via Editor; not generic TTL tutorial"
---

# LV2 (AURA)

**Summary:** `aura-lv2` wraps the same `PluginLogic` as CLAP/VST3. Authors do
**not** write `lv2_descriptor` / TTL by hand — `cargo aura` + `bundle_ttl_*`
emit the bundle. UI reuses `aura_core::Editor` (no separate UI crate).

## Author path

```toml
aura = { path = "...", features = ["lv2"] }
```

```rust
#[cfg(feature = "lv2")]
aura::export!(MyPlugin);
```

`PluginInfo.lv2_uri` **must** match the TTL the bundle ships. Hosts scan TTL
first, then match `lv2_descriptor` URI. Smoke-gain uses
`https://lx-audiolabs.com/lv2/smoke-gain`.

```bash
cargo aura build --lv2 -plug <name>
```

## UI

| Piece | Role |
|-------|------|
| `lv2ui_descriptor` | UI entry |
| `Editor` / `Lv2Bridge` | same trait as CLAP/VST3 |
| `ui:idleInterface` | idle / param sync |
| TTL UI triples | emitted **only** when `PluginLogic::editor()` returns `Some` |

Headless plugins stay headless — no UI triples.

## Process API

Same `ProcessContext` as CLAP. VST3/LV2 lift 7-bit MIDI into type-0x2 UMP and
down-convert `ump_out` / On/Off/Choke. Do not add an LV2-shaped process hook.

Worker extension: not used by the wrapper. Heavy work stays off the audio
thread in plugin code (queues), not via LV2 worker, unless a product needs it.

## Hosts

Windows **REAPER does not ship LV2**. Real host smoke: Carla / jalv on Linux,
or a dedicated LV2 host. Do not treat “loads in REAPER” as LV2 proof.

## Do not

- Hand-write `manifest.ttl` that disagrees with `lv2_uri`
- Invent a second UI stack for LV2
- Assume LV2 has CLAP note expressions / poly-mod (`notes` is CLAP-native)
- Skip URI/TTL match — the plugin will not instantiate

## See also

- `02-frameworks/aura.md` — `PluginLogic` / `editor()`
- `04-ui/slint.md` — `AuraSlintEditor`
- `notes/aura-lv2.md` — UI decision + REAPER lesson
