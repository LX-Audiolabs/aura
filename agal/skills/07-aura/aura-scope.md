---
id: aura-scope
group: aura
summary: AURA framework scope, non-goals, basis gate, and agent read order.
triggers: scope, cutover, basis, non-goals, aura framework, formats, UI stack
verify: Slint+baseview only; formats CLAP/VST3/LV2; product DSP stays out; basis before cutover
adapted: true
---

# AURA scope (local)

**Summary:** Hard boundaries for this framework workspace. Load when starting
framework work or when cutover / UI / formats feel unclear.

Local skill (`07-aura/`) — **not** in tool packs. Survives `agal skills sync`
unless deleted. Canonical long form: root `README.md` + `docs/migration-steps.md`.

## In scope

| Layer | Commit |
|-------|--------|
| **UI** | Slint **only** via `aura-baseview` + `aura-editor` + `aura-build` (`@aura`) |
| **Formats** | CLAP (primary), VST3, LV2 — thin wrappers over one `PluginLogic` |
| **Hosts** | Bitwig-first; then other CLAP/VST3/LV2 hosts |
| **Platforms** | Windows, Linux, macOS |
| **Layout** | `crates/` · `examples/` · `tools/` · `cargo aura` CLI |
| **DSP / MIDI** | `aura-dsp` (juce_dsp-shaped) · `aura-midi` (messages/buffer) |

## Out of scope (do not build here)

- egui / iced / Vizia as first-class UIs
- AU / AAX / VST2
- Absorbing product infra (`lx-shm`, `lx-vault`) or per-plugin `*Shared` UI state into AURA
- Bulk product plugin migrate before **basis** DoD is green
- Kitchen-sink multi-UI framework (that is truce / nice-plug territory)

Portable **algorithms** from product `lx-dsp` / `lx-analysis` → modules under **`aura-dsp`** (see `docs/dsp-layout.md`).

## Rules (also in `agal.toml` → agent map)

1. **basis_first** — finish framework basis (`docs/migration-steps.md` DoD) before product cutover.
2. **thin_formats** — one `PluginLogic` API; format crates stay thin.
3. **product_infra_out** — `lx-shm` / `lx-vault` / product UI shared-state stay in plugins repo; portable DSP algos may live in `aura-dsp`.
4. **roadmap** — single status source: `docs/migration-steps.md`.
5. **agal_optional** — orientation only; never hard-depend for `cargo aura` build.
6. **nice_plug** — external multi-UI stacks → nice-plug / truce / clack, not NIH-plug rewrites.

## Basis gate (when cutover is allowed)

Read **`docs/migration-steps.md`** status table. Cutover blocked while open P1s remain
(e.g. LV2 host smoke). Do not start bulk plugin migration on chat optimism.

## Agent read order (this repo)

1. **L3** `agal/AGAL.md`
2. **L2** `agal/agal.agent.md` (+ delta)
3. **L1** one note: crate under work, or `smoke-gain` / tool member
4. **This skill** if scope/cutover unclear
5. Roadmap / gaps: `docs/migration-steps.md`, `docs/gaps-and-optimizations.md`
6. L0 only if map + note insufficient

## Related loadouts

| Need | Skill / doc |
|------|-------------|
| Release / SemVer | `01-policy/versioning` + `docs/versioning.md` |
| CLAP ship | `03-formats/clap` |
| Slint UI | `04-ui/slint` |
| Params / process patterns | `02-frameworks/framework-patterns` |
| Realtime / threads | `00-core/audio-thread-boundary`, `dsp-realtime` |
