# aura-build

Build-script helper for AURA + Slint plugins.

- Bundles **@aura** widget library (`Knob`, `Meter`, `AuraTheme`, …)
- **Material 3–aligned** dark tokens (`AuraTheme`) — not the full `@material` kit
- Bundles fonts (JetBrains Mono, Noto Sans, Selawik — **OFL**, see `fonts/OFL.txt`)
- Pins Slint style **fluent** for std-widgets (ComboBox / Switch) cross-OS consistency

```rust
// build.rs
fn main() {
    aura_build::compile("ui/main.slint").unwrap();
}
```

```slint
import { Knob, AuraTheme } from "@aura";
import "NotoSans-Regular.ttf";
// background: AuraTheme.surface;
```

Direction: [docs/slint-ui-direction.md](../../docs/slint-ui-direction.md).  
Pairs with **`aura-baseview`** (window stack) and **`aura-editor`** (host adapter).
