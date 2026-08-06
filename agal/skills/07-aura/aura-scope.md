---
id: aura-scope
group: aura
summary: AURA framework scope, non-goals, basis-first gate, agent read order.
triggers: AURA, scope, basis, cutover, non-goals, roadmap, migration-steps, framework
verify: no product migrate before DoD; no second UI toolkit; roadmap in migration-steps
source: local
adapted: true
reason: "framework-only skill; not in agal tool packs"
---

# AURA scope

**Summary:** What AURA is and is not. Load when starting framework work or when a change risks scope creep.

## Is

| | |
|--|--|
| **UI** | Slint **only**, via **baseview** (`aura-baseview` + `aura-editor`) |
| **Renderer** | FemtoVG default; Skia / software optional — not a second toolkit |
| **Formats** | CLAP (primary), VST3, LV2 |
| **Workflow** | `cargo aura new|build|install|doctor|preview` (GUI later = same flags) |
| **Orientation** | **agal** in this repo (`agal/`) — optional for *author* plugins |

## Is not

- egui / iced / Vizia  
- AU / AAX / VST2  
- Full truce dump / multi-UI kitchen sink  
- Product DSP catalog (`lx-dsp`, analysis, vault, …)  
- Early cutover of aether/meridian/… before **Basis fertig**

Who wants multi-UI or AU → **nice-plug**, **truce**, or **clack**.

## Strategy (fixed)

```text
1) Ship AURA (basis) in this repo — smoke + cargo aura
2) Prove CLAP (+ later VST3/LV2) end-to-end
3) Only then migrate lx-audiolabs-plugins
```

Detail + checkboxes: **`docs/migration-steps.md`** (single roadmap).

## Agent read order (this repo)

1. Root **`AGENTS.md`** — policy + commit + build  
2. **`agal/AGAL.md`** — map, health, skill index (regenerated)  
3. Durable memory: **`agal/notes/_workspace.md`** (`[ATOM]` first)  
4. Roadmap when planning/status: **`docs/migration-steps.md`**  
5. Focus note: `agal/notes/<crate>.md` if working one crate  
6. **One** skill loadout (this file, or clap/slint/core/ponytail)

Do **not** dump `agal/skills/` or full `agal.json`.

## Basis gate (cutover blocked until)

See DoD table in `docs/migration-steps.md`. Rough blockers today:

- `aura-derive`  
- Bitwig (or REAPER) GUI on smoke-gain  
- VST3 + LV2 if product matrix still needs them  
- Scaffold / install polish  

Stage 6 (init wizard, aura-gui, agal mesh UX) is **not** a basis gate.

## When editing

| Touch | Also check |
|-------|------------|
| `aura-clap` | free-audio headers; thin wrapper; formats/clap skill |
| process / editor bridge | audio-thread-boundary + dsp-realtime |
| `.slint` / widgets | slint skill; `cargo aura preview` |
| new format crate | thin wrapper only; no format-shaped `aura-core` |
| product plugins | **stop** unless Stage 7 gate green |

## Verify (framework)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p smoke-gain --release
# clap-validator validate path/to/smoke-gain.clap
agal .
```
