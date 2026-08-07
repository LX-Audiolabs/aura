# LX Audiolabs — Licensing & Compliance Matrix

**Status:** verbindlich ab 2026-08-06  
**Einheitslizenz (unser Code):** **GPL-3.0-or-later**  
**Gilt für:** **AGAL**, **AURA** (`aura-*`, `cargo-aura` / `cargo aura`), und Plugins/Tools die wir unter derselben Linie veröffentlichen.

> Kein Anwaltsdokument. Kurze operative Matrix für Dev, README und Ship-Checklisten.

---

## 1. Entscheidung (warum GPL)

| Punkt | Haltung |
|-------|---------|
| Nutzen & forken | ja, gratis |
| Weiterentwickeln / abändern | ja |
| **Geschlossene** 1:1-Forks von AGAL/AURA als Produkt | nein (Copyleft) |
| Plugins auf AURA **verkaufen** | **ja**, wenn Source unter GPL-kompatiblen Bedingungen mitangeboten wird |
| Plugins auf AURA **closed / proprietär** shipe | **nein** — bewusst ok für uns |
| Begründung | Wir bauen auf permissiven Bausteinen (CLAP, truce MIT/Apache, viels Rust-Ökosystem). Unser Stack geben wir share-alike zurück. Kein Zwang zu proprietären Dritt-Plugins. |

**Wichtig:** GPL verbietet **nicht** den Verkauf. Sie verbietet, das verteilte Werk **ohne** Corresponding Source und ohne Copyleft zuzuklappen.

---

## 2. Schichten-Matrix (unser Code)

| Komponente | Rolle | Unsere LICENSE | Bei Distribution |
|------------|-------|----------------|------------------|
| **AGAL** | CLI / Agent-Orientierung | GPL-3.0-or-later | agal-Binary/Source: GPL; **generierte** Notes/Maps = Workspace-Inhalt (nicht automatisch GPL) |
| **AURA** (`aura-core`, `aura-plugin`, `aura-clap`, …) | Plugin-Framework | GPL-3.0-or-later | in `.clap` / VST3 / LV2 **statisch** mitgelinkt → **gesamtes Plugin-Artefakt** muss GPL-tauglich sein |
| **cargo-aura** (`cargo aura`) | Build/Install-CLI | GPL-3.0-or-later | wie AGAL (Tool) |
| **LX Plugins** (aether, lucent, …) | Produkte | GPL-3.0-or-later (Default) | Source anbieten bei Weitergabe; Verkauf ok |
| **aura-baseview** / **aura-editor** / **aura-build** | UI: immer Slint+baseview; Renderer FemtoVG/Skia/software | baseview+editor **MIT**; Rest Stack GPL-Linie | Teil des Plugin-Links |

**Formate:** CLAP, VST3, LV2, später Standalone — **gleiche** Regel (eine Binary mit AURA drin). Keine format-spezifische Extra-Lizenz.

---

## 3. Drittschichten (nicht „unsere“ LICENSE, aber im Ship)

### 3.1 Permissive Inputs (rein → unser GPL-Werk ok)

MIT / Apache-2.0 / ähnlich erlauben Einbindung in GPL-Werke (Attribution / NOTICE behalten).

| Abhängigkeit (Beispiele) | Typische Lizenz | Pflicht für uns |
|--------------------------|-----------------|-----------------|
| **truce** (Ausgangscode AURA) | Apache-2.0 OR MIT | Notices behalten; Relicense *unseres* abgeleiteten Werks als GPL ist erlaubt |
| **CLAP** (`clap-sys` / Spez-Headers) | üblich MIT | Copyright-Hinweise |
| DSP-/Util-Crates (crates.io) | oft MIT/Apache | Notices; keine Copyleft-Überraschung |
| Fonts (gebündelt) | je Font (OFL etc.) | Font-LICENSE mitschippen wo nötig |

### 3.2 Slint (UI) — **Triple-License**, nicht „nur MIT“

Maßgeblich: [slint LICENSE.md](https://github.com/slint-ui/slint/blob/master/LICENSE.md)

| Slint-Option | Kosten | Passend zu unserem GPL-Plugin? |
|--------------|--------|--------------------------------|
| **GPLv3** | gratis | **Ja — Default-Pfad für unseren Stack** (Copyleft-Stack) |
| **Royalty-free** | gratis + Disclosure (AboutSlint/Badge) | Proprietäre Desktop-Apps; **widerspricht** unserem „Plugin muss GPL“-Default nicht zwingend, aber **zwei Stories**. Für open GPL-Plugins unnötig. |
| **Commercial** | bezahlt | Nur wenn wir bewusst proprietär/Embedded ohne GPL-Slint wollen |

**Operative Wahl LX (Default):**

```text
Unser Plugin/AURA-Stack  →  GPL-3.0-or-later
Slint in demselben Artefakt  →  GPLv3-Option
```

Docs/Examples von Softint sind MIT — Zitate/Snippets ok mit Attribution.

**Desktop vs Embedded (Slint-Definitionen):** DAW-Plugins auf PC/Mac/Linux = **Desktop Application**, nicht „Embedded System“ im Slint-Sinn. Embedded-Klauseln der Royalty-free-Lizenz betreffen uns im Normal-Plugin-Ship nicht; unter **Slint GPLv3** ist das sowieso abgedeckt.

### 3.3 VST3 (Steinberg) — geprüft 2026-08-07

**Lizenzlage:** VST3 SDK ist seit **VST 3.8.0 (2025-10)** unter **MIT** — das frühere Dual-Modell (proprietär / GPLv3) ist abgelöst. Kein GPL-Konflikt mehr, keine Steinberg-Vereinbarung für den Ship nötig.

**Unser Pfad:** `aura-vst3` linkt **kein** Steinberg-SDK, sondern [`vst3-rs` (coupler-rs)](https://github.com/coupler-rs/vst3-rs) — MIT OR Apache-2.0, aus den C++-Headern generierte Bindings. Behandlung wie §3.1 (permissive Inputs): Notices behalten, fertig.

**Ship-Checkliste VST3:**

- [x] SDK-Lizenz: MIT seit 3.8.0 — keine Steinberg-EULA, keine GPL-Spannung
- [x] Bindings-Lizenz: `vst3` crate MIT OR Apache-2.0 (Notices in Dritt-Attribution)
- [ ] **Marke:** „VST“ / VST-Logo bleiben Steinberg-Marken (lizenzunabhängig). Nominative Nennung „VST3-kompatibel“ ok; **kein** VST-Logo / Steinberg-Branding ohne separate Vereinbarung
- [ ] Bundle-Layout pro Spez: `<name>.vst3/Contents/<arch>/<name>.vst3` (Win: `x86_64-win`; Linux: `x86_64-linux`; macOS: `Contents/MacOS`) — von `cargo aura install --vst3` erzeugt
- [ ] Validator-Smoke im Host (Bitwig: grün 2026-08-07; REAPER optional)

### 3.4 LV2

Spez/Headers typisch liberal; unser Code in der `.so` = GPL. Manifeste (`.ttl`) als Teil des Bundles mitschippen.

---

## 4. Szenarien (schnell)

| Szenario | Erlaubt? |
|----------|----------|
| User führt agal / `cargo aura` lokal aus | ja |
| Fork von AGAL/AURA, Änderungen, Source GPL, auch gegen Geld | ja |
| Closed „AURA Pro SDK“ ohne Source | **nein** |
| Dritter baut Plugin **mit** AURA, verkauft `.clap`, bietet Source unter GPL an | **ja** |
| Dritter baut Plugin mit AURA, verkauft nur Binary, Source geheim | **nein** |
| Nur agal nutzen, Plugin **ohne** AURA-Link (hypothetisch anderes Framework) | agal-Output nicht GPL-infiziert; Plugin-Lizenz = deren Framework |
| Unser Shop: Plugin verkaufen + GitHub-Source GPL | **ja** (bewusster Default) |
| DAW (Bitwig etc.) lädt GPL-`.clap` | Host wird **nicht** GPL; Plugin bleibt GPL |

---

## 5. Compliance-Checkliste vor Release

### AGAL / cargo-aura (Tool)

- [ ] `LICENSE` = GPL-3.0-or-later (Volltext + Kurzheader)
- [ ] `Cargo.toml` → `license = "GPL-3.0-or-later"`
- [ ] README License-Abschnitt
- [ ] Dritt-Notices wo nötig (deps, nicht übertrieben im CLI)

### AURA / Plugin-Binary (CLAP / VST3 / LV2)

- [ ] Alle `aura-*` Crates: `license = "GPL-3.0-or-later"` (baseview-Crate: MIT ok, siehe oben)
- [ ] Root `LICENSE` im AURA-Repo
- [ ] Plugin-Repo: gleiche LICENSE + Source-Angebot (Repo-URL / tarball / §6 GPL)
- [ ] **Slint:** GPLv3-Pfad gewählt; keine Royalty-free-only Story ohne Disclosure-Konzept
- [ ] About/Credits: LX, AURA, ggf. truce-Ursprung notices, Font-Lizenzen
- [ ] VST3: Steinberg SDK checklist (wenn Format aktiv)
- [ ] CLAP/LV2: Validator + keine fremden License-Files strippen

### Was **nicht** nötig ist

- [ ] Static-Linking-Exception (nur LGPL-Thema) — unter **GPL** entfällt das
- [ ] Getrennte Lizenzen AGAL vs AURA — **eine** Linie
- [ ] „MIT OR Apache“ nur weil Rust-Default — bei uns bewusst **nicht**

---

## 6. Cargo / SPDX

```toml
license = "GPL-3.0-or-later"
```

Nicht `GPL-3.0-only`, damit spätere GPL-4-Option offen bleibt (`or later`).

---

## 7. Relation der Marken

```text
agal          = Agent-Orientierung (GPL Tool)
AURA          = Audio Unified Rust Architecture (GPL Framework, aura-* crates)
cargo aura    = CLI (cargo-aura package)
Plugins       = Produkte auf AURA (Default GPL)
Slint+baseview= UI-Pfad (Renderer: FemtoVG | Skia | software; Toolkit immer: Slint)
Slint license = GPLv3-Option im Default-Ship
```

DE-Gag AURA: *AUdio RAhmenwerk* — nur Name, keine Lizenzwirkung.

---

## 8. Änderungshistorie

| Datum | Änderung |
|-------|----------|
| 2026-08-06 | Einheitslizenz GPL-3.0-or-later; Matrix AGAL+AURA+Plugins; Slint-Triple-License; Verkauf-mit-Source klar; Closed-Plugins abgelehnt |
| 2026-08-07 | §3.3 VST3 geprüft: SDK seit 3.8.0 MIT; Pfad über `vst3-rs` (MIT/Apache); nur Marken-/Branding-Regeln bleiben |

---

*Pfad: `AURA/docs/licensing-compliance.md` — bei Lizenzfragen zuerst diese Datei, dann `LICENSE`.*
