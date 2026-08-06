# AGENTS.md — AURA (Audio Unified Rust Architecture)

CLAP-first plugin framework for **LX Audiolabs**.  
Runtime + formats + build + CLI here. **Orientation** owned by **agal**.

## Orientation (agal)

Read **`agal/AGAL.md`** first for map, health, skills index, and hot path.  
Structural map: `agal/agal.agent.md`.  
Durable memory: **`agal/notes/_workspace.md`** (never overwritten by `agal .`).  
Roadmap / DoD: **`docs/migration-steps.md`** (single source of truth).

```bash
agal .                                          # regenerate map + notes headers
agal skills sync                                # core (default)
agal skills sync --only policy,formats/clap,ui/slint,agents,frameworks
agal doctor
```

Do **not** dump `agal/` into context. Load **one** note / **one** skill on demand.

### Scope loadout

Starting framework work or unsure about cutover/UI/formats → load:

- **`agal/skills/07-aura/aura-scope.md`** (local; not in tool packs)

## Caveman

Talk terse. Drop articles/filler/pleasantries. Fragments OK. Technical terms exact.  
Stop: user says `stop caveman` / `normal mode`.  
Normal prose for security warnings, irreversible actions, confusion. Code/commits/PRs normal.

## Ponytail — Lazy Senior Dev

Before code: 1. YAGNI? 2. Already in codebase? 3. stdlib? 4. platform? 5. installed dep? 6. one-liner? 7. write minimum.  
Bug fix = root cause. Trace callers. Delete > add. Mark shortcuts `ponytail:`.  
Not lazy: validation, error handling that prevents data loss, security, explicit requests. Non-trivial logic → ONE assert/test.

## Skills (when to load)

| Skill / pack | When |
|--------------|------|
| **aura-scope** (`07-aura/`) | Scope, non-goals, basis gate, read order |
| **agent-usage** | agal disclosure L3→L0, context budget |
| **clap** (`formats/clap`) | CLAP ext, ship path, validator |
| **slint** (`ui/slint` or grok slint skill) | `.slint`, widgets, interop, preview |
| **audio-thread-boundary** / **dsp-realtime** | process, gestures, shared state |
| **ponytail** / **caveman** | policy (also global plugins) |
| **framework-patterns** | params/process/thread patterns |

Synced packs: `agal skills sync --only …`. Local **aura-*** files under `07-aura/` survive sync unless deleted.

## github commits & push

Commits: `user.name=lxndrbe` · `user.email=ardvinnamoon@gmail.com`  
GitHub auth: `github.user=lxndrbe`

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

UI preview without compiling the plugin:

```bash
cargo aura preview
# or: cargo run -p aura-preview -- path/to/ui/main.slint
```

Rust **1.92+** (edition 2024).

## Partner repos

| Repo | Role |
|------|------|
| **lx-audiolabs-plugins** | Product plugins (still truce until cutover) |
| **agal** (Agentic Audiolab) | Orientation tool binary + skill catalog |
