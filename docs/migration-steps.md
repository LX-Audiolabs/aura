# AURA — strategy, roadmap & build order

**Single source of truth** for direction, stages, and cutover gates.  
No separate `roadmap.md` — update **this file** when status or next steps change.

Last status pass: 2026-08-07.  
**Gaps & optimizations (derive, cutover, polish):** [gaps-and-optimizations.md](./gaps-and-optimizations.md).

---

## Strategy (fixed)

**Reverse / product-led framework build** — not “fork truce then rename.”

```text
1) Ship AURA as a real framework
      installable & usable like truce:
      cargo aura new|build|install|doctor
      → build CLAP (then VST3/LV2) plugins with Slint+baseview

2) Prove it inside AURA
      in-tree examples/ smoke (and maybe one richer reference)
      cargo aura new skeleton shaped like our product plugins (ui/ + aura.toml + agal)

3) Only then migrate lx-audiolabs-plugins
      product catalog switches truce/lx-* → aura-*
      “as if we had always built them on AURA”
```

| We do | We do not (yet / ever) |
|-------|-------------------------|
| Finish **AURA basis** first | Bulk-migrate product plugins early |
| Path/git deps until basis usable | **crates.io only after** basis (`cargo aura` + CLAP + UI) |
| Learn from **how we built** aether/lucent/… | Copy every truce crate “because it exists” |
| truce + current plugins = **read-only design refs** | Vendor-dump truce into AURA |
| Targeted truce: Slint + CLAP/VST3/LV2 | egui/iced/AU/AAX zoo |

**Mental model:** truce is a general multi-UI/multi-format kit.  
**AURA** is the framework we *wish* we had after building LX plugins — same *workflow* as truce (`cargo … new/build/install`), narrower *surface*.

---

## Current status (living)

**Verdict:** On course. CLAP + Slint smoke path works in-tree. Product cutover **not** ready.

| Layer | State |
|-------|--------|
| Direction / scope | Aligned (Slint-only, CLAP-first, no AU/egui zoo) |
| Framework basis (own DoD) | ~half — Stages 0–4 mostly; VST3/LV2 + host harden open |
| Product cutover | Blocked — multi-format (VST3/LV2) + Bitwig GUI; ParamId **decided (A)**, derive + scaffold landed |

### What works today

- Workspace layout: `crates/` · `examples/` · `tools/`
- `PluginLogic` + params surface + umbrella `aura::prelude`
- **CLAP:** factory, stereo ports, params, process, `clap.state`, `clap.gui`
- **UI:** `aura-baseview` + `AuraSlintEditor` + `aura-build` (`@aura`)
- **`examples/smoke-gain`:** builds, clap-validator green (incl. state/GUI path)
- **`cargo aura`:** `new` · `build` · `install` · `doctor` · `preview`
- **agal orientation:** `agal.toml` · `agal/notes/_workspace.md` · synced skills · local `07-aura/aura-scope`
- **`aura-derive`:** `#[derive(Params)]` / `ParamEnum` + `<Struct>ParamId` enum; tests + smoke-gain (see [gaps-and-optimizations.md](./gaps-and-optimizations.md))

### Still open for “Basis fertig”

| Priority | Item | Why |
|----------|------|-----|
| **P0** | Bitwig (or REAPER) GUI open on smoke-gain | Validator ≠ real host; prove parented Slint — **now the top basis item** |
| **P1** | VST3 wrapper + `cargo aura build/install --vst3` | Product matrix ships VST3 today |
| **P1** | LV2 wrapper + `cargo aura build/install --lv2` | Product matrix ships LV2 today |
| **P2** | Plugin-level state hooks beyond flat param blob | Presets/vault stay product-side; host save must not lie |
| **P2** | Buses / note-ports only if a smoke or pilot needs them | Stereo FX first; not every truce feature |
| **P3** | Stage 6 polish (hot-reload, MIDI 2.0 stubs, `aura-gui` CLI) | **After** basis — not a gate |

### Product gap (lx-audiolabs-plugins needs vs AURA)

| Capability | Product (truce) today | AURA now | Cutover blocker? |
|------------|----------------------|----------|------------------|
| `#[derive(Params)]` | yes (often no `id`) | yes (**`id = N` required**) | soft (ID pass) |
| `*ParamId` enum for editors | yes (truce-derive) | **yes** (derive emits `<Struct>ParamId`, G2 option A) | no (closed) |
| CLAP process / params / GUI | yes | yes (minimal) | soft |
| VST3 / LV2 | yes | no | **yes** (for full matrix) |
| MIDI / note events in process | yes | transport only | soft for our FX/analyzers |
| Custom state / presets / vault | product + truce | flat param blob | soft (API must not regress) |
| Multi-bus I/O | truce | stereo-fixed clap | low for current catalog |
| Editor extras (zoom, meters tick) | `lx-slint-editor` etc. | thin adapter | product layer after cutover |

**Product DSP stays out of AURA:** `lx-dsp`, `lx-analysis`, `lx-shm`, `lx-vault`, `lx-ui-slint` remain in the plugins repo. Framework owns formats + process + params + Slint host path only.

### Next work order (do in order)

1. ~~`aura-derive` → rewrite smoke-gain to use it~~ — done (land commit if still local)  
2. ~~**Decide ParamId (G2)**~~ — done: **option A**, derive emits `<Struct>ParamId`; documented in gaps doc  
3. ~~Scaffold + install polish~~ — done: `cargo aura new` emits derive + ids + working CLAP export; `install --clap` ships `<package>.clap`  
4. **Bitwig GUI smoke on `smoke-gain`** — top remaining basis item (manual, needs DAW)  
5. Stage 5: VST3, then LV2 (same smoke plugin, feature flags)  
6. Declare **Basis fertig** when DoD table below is all green  
7. Stage 7: **one** pilot product plugin (see cutover checklist in gaps doc), then catalog  

Do **not** start Stage 7 or bulk-rename product crates before step 6 (Basis fertig).

---

## “Basis fertig” — definition of done for phase Framework

AURA is ready for product cutover only when **all** of these work **without** the lx-audiolabs-plugins tree:

| # | Capability | Like truce | AURA target | Status |
|---|------------|------------|-------------|--------|
| 1 | Install toolchain | `cargo install cargo-truce` | **`cargo install cargo-aura`** (or path install) | done (path) |
| 2 | New plugin | `cargo truce new` | **`cargo aura new <name>`** → layout like our plugins | done (single-crate: derive + ParamId + CLAP export + `@aura` UI) |
| 3 | Metadata | `truce.toml` | **`aura.toml`** (+ **`agal.toml`** in skeleton) | done in scaffold |
| 4 | Params + DSP API | `PluginLogic` / params derive | **`aura-core` + `aura-params` (+ derive)** | derive + `*ParamId` done |
| 5 | UI | (various) | **`aura-editor` + `aura-build`** (`@aura`, FemtoVG default) | done; Bitwig open **pending** |
| 6 | Build format | `--clap` etc. | **`cargo aura build --clap`** (then vst3/lv2) | clap yes; vst3/lv2 **open** |
| 7 | Install into host path | `install --clap` | **`cargo aura install --clap`** (e.g. `%CLAPINS%`) | done (`<package>.clap`) |
| 8 | Sanity | validators / DAW load | **clap-validator** + Bitwig smoke on in-tree **example** | validator green; Bitwig **open** |
| 9 | Docs | README | this file + root README scope | living |

Optional for v1 basis (nice, **not** gate): hot-reload shell, full MIDI 2.0, screenshots via slint-viewer.  
**Visual installer / init wizard / agal interop** = Stage 6 vision (below) — **not** a basis gate.

**Out of scope until after cutover decision:** changing aether/meridian/… Cargo.toml to AURA.

---

## Tooling vision — AURA + agal (separate, optional mesh)

Two **independent** tools. Same shop, optional handshake. Neither requires the other.

```text
                    ┌─────────────┐
  empty folder ───► │  AURA tool  │──► aura.toml · plugins/ · cargo aura …
                    │ CLI + GUI   │
                    └──────┬──────┘
                           │ optional: detect `agal` on PATH
                           ▼
                    ┌─────────────┐
                    │    agal     │──► agal.toml · orientation maps · agent rules
                    │  (if yes)   │
                    └─────────────┘
```

| Tool | Owns | Does not own |
|------|------|--------------|
| **AURA** (`cargo aura` / later **aura-gui**) | Framework path, scaffold, formats, build/install, preview, plugin skeleton modules | Agent maps, graph health, product orientation mesh |
| **agal** | Agent orientation, project graph, rules/suppress, notes | Compiling CLAP/VST3/LV2, DSP, Slint host embed |

**Rule:** AURA writes a **thin** `agal.toml` stub only when the user opts in (or scaffold default “yes if agal present”). Agal never becomes a hard dependency of `cargo aura build`.

### Target workflow (visual = CLI parity)

All wizard choices must map 1:1 to flags. GUI is sugar; CLI remains source of truth for CI/agents.

1. **Open tool** in empty (or product) folder  
   - Visual: Slint **aura-gui** (installer / project console)  
   - CLI: `cargo aura init` · `cargo aura new` · `cargo aura add-plugin` (names TBD)

2. **INIT workspace** (empty dir → product tree)  
   User picks:
   - **Formats:** CLAP only · +VST3 · +LV2 (any subset; CLAP recommended default)
   - **Platforms:** Windows · Linux · macOS (any subset)
   - **Kind intent** (skeleton modules, not full DSP):
     - effect · analyzer · instrument/synth · sequencer/note FX
   - **agal:** if `agal` on PATH → ask “also wire agal?”  
     - yes → write richer `agal.toml`, point at `output_dir`, optional first `agal …` hint  
     - no / missing → skip; AURA still works

3. **Write skeleton** into that folder  
   e.g. workspace `Cargo.toml`, `aura.toml`, optional `agal.toml`, `plugins/` or single-crate layout, feature flags matching format picks, stub `PluginLogic` + `ui/main.slint` matching kind (stereo FX vs analyzer meters stub vs note ports when we have them).

4. **Re-open same folder**  
   Tool **reads** `aura.toml` (and Cargo features): formats already chosen, plugins listed.  
   UI: “Add plugin” → name + kind + same frame conditions → append `[[plugin]]`, crate under `plugins/<name>/`.  
   CLI: same via `cargo aura new` / `add` with flags.

5. **Day-to-day**  
   `build` / `install` / `preview` / `doctor` — CLI always; GUI buttons call same code paths.

### CLI surface (target; not all exist yet)

| Command (intent) | Role |
|------------------|------|
| `cargo aura init [flags]` | Empty/workspace bootstrap (formats, platforms, kind, `--with-agal` / `--no-agal`) |
| `cargo aura new <name>` | Plugin crate skeleton (today: minimal single-crate) |
| `cargo aura add-plugin …` | Extra plugin into existing multi-plugin workspace (later) |
| `cargo aura preview` | Slint UI hot-reload (exists) |
| `cargo aura build\|install` | formats (exists; VST3/LV2 later) |
| `cargo aura doctor` | toolchain + AURA_PATH + optional “agal found?” |
| **aura-gui** | Slint shell over the same commands |

Flags sketch (bind GUI controls to these):

```text
--clap --vst3 --lv2
--os win,linux,macos
--kind effect|analyzer|instrument|sequencer
--with-agal | --no-agal
```

### Layering (implementation order — do not invert)

| Layer | When | Notes |
|-------|------|--------|
| **A** Shared scaffold engine in `cargo-aura` (flags → files) | After richer scaffold (P1) / derive | GUI must not reimplement templates |
| **B** `init` + multi-plugin `aura.toml` model | After A | Empty-folder story |
| **C** `doctor` optional agal probe | Cheap anytime | Detect only; never fail basis if missing |
| **D** aura-gui (Slint) calling A–C | Stage 6 | Installer + project console |
| **E** Deeper agal mesh (migrations, rules from kind) | With agal maintainers | AURA stays dumb writer of agreed keys |

**Today:** stub `agal.toml` on `new` only; no `init`, no detect, no GUI, no kind modules.

### Non-goals for this mesh

- Bundling agal inside AURA releases as required runtime  
- AURA parsing full agal orientation graphs  
- Forcing agal on every scaffold  
- GUI-only workflows without CLI flags  

---

## Drift guards (do not deviate)

If a PR or idea hits one of these, **stop** or park under Stage 6 / post-cutover:

| Drift | Response |
|-------|----------|
| Second UI toolkit (egui/iced/Vizia) | **Reject** — use nice-plug/truce elsewhere |
| AU / AAX / VST2 | **Reject** for AURA v1 |
| “Port all of truce-core/cargo-truce” | **Reject** — selective port only |
| Migrate product plugins before Basis fertig | **Reject** — strategy step 3 only after DoD |
| Put `lx-dsp` / analysis / vault into AURA | **Reject** — product crates stay product |
| Stage 6 polish / aura-gui / init wizard before P0/P1 | **Defer** — polish is not the basis gate |
| GUI without CLI flags for same action | **Reject** — visual = sugar over `cargo aura …` |
| Hard-depend on agal for build/install | **Reject** — optional mesh only |
| Docs claim “CLAP not shipped” while smoke works | **Fix docs** — status lives here |

Stale help text in `cargo-aura` / crate READMEs must match this file after each stage close.

---

## Naming (fixed)

| Old / interim | AURA |
|---------------|------|
| `cargo-truce` | **`cargo-aura`** → **`cargo aura …`** |
| `truce.toml` | **`aura.toml`** |
| `truce-*` / interim `lx-*` (framework only) | **`aura-*`** |
| `use truce::*` | **`use aura::*`** |
| `lx-slint-baseview` | **`aura-baseview`** (window) + **`aura-editor`** (host adapter) |
| `lx-slint-build` | **`aura-build`** (`@aura`) |

Product helpers (`lx-dsp`, `lx-analysis`, …) keep `lx-*` in the plugins repo unless later renamed on purpose.

---

## Stages (AURA-only until Basis fertig)

### Stage 0 — skeleton

- [x] LICENSE, compliance, README scope, workspace dirs
- [x] Naming: `aura-*`, `cargo aura`
- [x] Layout: `crates/` · `examples/` · `tools/`
- [x] Root `agal.toml` + `agal/` orientation (`notes/_workspace.md`, skills, `aura-scope`)
- [ ] GitHub publish when useful

### Stage 1 — core API

- [x] `aura-params`
- [x] `aura-core` (minimal `PluginLogic` / `Editor` / process)
- [x] Umbrella **`aura`** (`use aura::prelude::*`)
- [x] Grow core as formats need: ~~events~~ (param gesture queue → CLAP out_events), ~~transport~~ (CLAP → `ProcessContext.transport`)
- [ ] buses only if a format smoke needs non-stereo (not basis-critical for current catalog)
- [ ] note-ports / MIDI in `ProcessContext` when an example needs them (not basis-critical for stereo FX)
- [x] `aura-derive` (`#[derive(Params)]` + `#[derive(ParamEnum)]`; smoke-gain ported; explicit `id = N`)
- [x] ParamId for editors (G2): **option A** — derive emits `<Struct>ParamId` (`id()` / `from_id()`); smoke-gain + scaffold use it

### Stage 2 — CLAP path (first shippable format)

- [x] `aura-clap` (factory, stereo ports, params, process)
- [x] **Spec policy:** [free-audio/clap](https://github.com/free-audio/clap); bindings `clap-sys` (see `crates/aura-clap/README.md`)
- [x] Wire `aura` feature `clap` + `aura::export!`
- [x] In-tree **`examples/smoke-gain`** — clap-validator green (core + state + GUI path)
- [x] `clap.state` extension (save/load, flat LE blob)
- [x] `clap.gui` extension (parented; host bridge: params + request_resize)
- [ ] Real-host GUI open (Bitwig first, then REAPER) on smoke-gain — **P0**
- [ ] Bump `clap-sys` when it tracks newer free-audio 1.2.x (optional; 1.2 ABI ok)

### Stage 3 — UI complete for plugin authors

- [x] `aura-baseview` window stack (from lx-slint-baseview)
- [x] `aura-editor` host adapter (`AuraSlintEditor` impl `aura_core::Editor`)
- [x] `aura-build` (`@aura` + fonts)
- [x] Scaffold `ui/main.slint` uses `@aura` (`cargo aura new`)
- [x] smoke-gain GUI (Slint + `@aura`) — compiles + validator-stable
- [ ] Prefer Slint-native screenshot/viewer over truce-gui ports (optional)

### Stage 4 — toolchain (truce-workflow parity)

- [x] `tools/cargo-aura`: **`new`**, **`build`**, **`install`**, **`doctor`**, **`preview`**
- [x] `new` skeleton: Cargo.toml · aura.toml · agal.toml · ui/ · build.rs · src/lib.rs
- [x] Install: `cargo install --path tools/cargo-aura --force`
- [x] Richer scaffold once derive ships (derive + explicit `id`s + `*ParamId` + working CLAP export)
- [x] `install --clap` copies/renames to `<package>.clap` cleanly
- [x] Align CLI help / doctor messaging with this status file

### Stage 5 — VST3 / LV2

Start only after CLAP smoke + Bitwig GUI are real.

- [ ] `aura-vst3` — thin wrapper over same `PluginLogic` (no VST3-shaped core)
- [ ] Steinberg SDK / licensing checklist for VST3 packaging
- [ ] `aura-lv2` — thin wrapper + TTL/manifest story (learn from product `lv2-meta` / truce-lv2 selectively)
- [ ] smoke-gain (or scaffold) features: `vst3`, `lv2`
- [ ] `cargo aura build|install --vst3` and `--lv2`
- [ ] Validators / host smoke as available (no kitchen-sink host matrix)

### Stage 6 — polish & authoring shell (still inside AURA; **not** basis gate)

See **Tooling vision — AURA + agal** above. Build on shared CLI scaffold engine first.

- [ ] Scaffold engine: flags → files (`--clap/--vst3/--lv2`, `--kind`, `--with-agal`)
- [ ] `cargo aura init` (empty folder → workspace + first plugin options)
- [ ] Re-open: read `aura.toml`, add another plugin skeleton
- [ ] `doctor`: optional “agal on PATH?” (info, not hard fail)
- [ ] **aura-gui** (Slint): installer + project console = same flags as CLI
- [ ] Agal mesh: richer stub / agreed keys when user opts in (with agal repo)
- [ ] Hot reload only if ROI clear (beyond `aura-preview`)
- [ ] MIDI 2.0 arch stubs
- [ ] Extra demos under `examples/` only if useful for cargo-aura docs

### Stage 7 — **product cutover** (only after Basis fertig)

- [ ] Gate check: DoD table all green (incl. derive + multi-format if catalog needs them)
- [ ] Pilot: **one** plugin (prefer simple stereo FX, e.g. aether) path/git dep on AURA
- [ ] lx-audiolabs-plugins: truce / framework `lx-*` → **AURA** path or git deps
- [ ] `use aura::*`, `aura.toml`, editor/build switch; keep product `lx-dsp` / analysis / vault
- [ ] CI matrix on product plugins
- [ ] Deprecate parallel truce path for LX products

---

## Explicit non-goals

- egui / iced / Vizia  
- AU / AAX / VST2  
- Full truce example zoo  
- Second UI path without (Slint + baseview)  
- Migrating product plugins before **Basis fertig**  
- Absorbing product DSP/UI catalog crates into the AURA framework repo  

---

## Reference inputs (read-only)

| Source | Why |
|--------|-----|
| **lx-audiolabs-plugins** | Layout, UX, what authors actually need |
| **truce-dev** | Proven param/process/format ideas to **selectively** port |
| **agal** | Orientation mesh in skeletons |
| **`../lx-framework-plan.md`** | Naming / license / early extraction notes (historical) |

AURA is the **output** of that learning — a **gezielteres** truce, not a mirror.
