---
source: global
copied_by: template
date: 2026-07-29
adapted: false
reason: "framework patterns for audio plugin stacks"
---

# Framework Patterns

**Summary:** Common architectural patterns for Rust audio plugin frameworks.
Covers parameter communication, process() hook structure, and thread separation.
Adaptable for truce, clack, nice-plug, and similar stacks.

- Params über lock-free Events
- `process()` Hook Struktur
- Editor-Thread vs Audio-Thread Trennung
