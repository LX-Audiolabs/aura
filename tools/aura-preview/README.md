# aura-preview

Hot-reload preview for AURA plugin `.slint` UIs — renders `ui/main.slint`
directly, without compiling the plugin. Same `@aura` widget library, bundled
fonts and fluent style as the real build (shared via `aura-build`).

## Usage

```bash
cargo run -p aura-preview -- path/to/ui/main.slint
# or from a plugin directory:
cargo aura preview                 # defaults to ui/main.slint
```

Flags:

- `--component <Name>` — pick a specific exported component (default: first).
- `--no-watch` — disable auto-reload; Reload button still works.

Two windows open:

- **Plugin window** — the interpreted UI, re-created in place on every reload.
- **Control window** — Reload button, auto-reload toggle, status/diagnostics.

On save of any `.slint`/`.ttf` in the UI directory the UI reloads (300 ms
debounce). Compile errors keep the last good UI and show the diagnostics in
the control window and on stderr.

## Notes

- Rendering uses Slint's default winit/femtovg backend, not the plugin's
  aura-baseview platform — layout/geometry are identical, pixel-level rendering
  may differ slightly.
- Component properties keep their defaults; live data (formatted values,
  meters, plots) is not fed in v1.
- Closes when either window is closed.
