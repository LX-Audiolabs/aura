# aura-build

Build-script helper for AURA + Slint plugins.

- Bundles **@aura** widget library (`Knob`, `Meter`, …)
- Bundles fonts (JetBrains Mono, Noto Sans, Selawik — **OFL**, see `fonts/OFL.txt`)
- Pins Slint style **fluent** for cross-OS consistency

```rust
// build.rs
fn main() {
    aura_build::compile("ui/main.slint").unwrap();
}
```

```slint
import { Knob } from "@aura";
import "NotoSans-Regular.ttf";
```

Pairs with **`aura-baseview`** (window stack) and **`aura-editor`** (host adapter).
