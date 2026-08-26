<!-- AGAL:AUTO-START -->
# aura-derive

> Auto-generated from workspace scan. Do not edit between AUTO markers.

| | |
|---|---|
| kind | `crate` |
| path | `crates/aura-derive` |
| description | Proc macros for AURA plugins: #[derive(Params)] and #[derive(ParamEnum)] |
| generated | `2026-08-26T11:53:59Z` |

## Graph atoms (auto)

_Regenerated each `agal .`. Scan these first. Human atoms: below HUMAN marker._

```text
[ATOM] type=fact | detail=kind=crate id=crates/aura-derive
[ATOM] type=fact | detail=roles=entry+manifest+source
[ATOM] type=fact | detail=used_by=aura via depends_on
[ATOM] type=fact | detail=used_by=aura-lv2 via dev_depends_on
```

## dependents (inbound)
- `aura` --depends_on--> `aura-derive`
- `aura-lv2` --dev_depends_on--> `aura-derive`

## structure
- public_api symbols: 2 (see json)
- roles: entry, manifest, source

## api surface
- `fn derive_param_enum(input: TokenStream) -> TokenStream` · `src/lib.rs`
- `fn derive_params(input: TokenStream) -> TokenStream` · `src/lib.rs`

## agent focus
**L1:** scan **Graph atoms** above first, then human body below HUMAN.  
After `agal.agent.md` (L2). Escalate L0: `crates/aura-derive` in json / `agal --plugin aura-derive .`

<!-- AGAL:AUTO-END -->

<!-- AGAL:HUMAN — edit below this line; preserved on regenerate -->

## Intent

_Why this crate/plugin exists. Edit freely._

## Open

- [ ] 

## Decisions

_Architecture choices worth remembering._

## Atoms (human)

_Graph atoms live **above** in AUTO. Add durable decisions/lessons here:_

```text
[ATOM] type=decision|lesson|constraint | detail=…
```
