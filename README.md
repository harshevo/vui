# vui

A GPU-drawn UI framework for Rust, built on wgpu. It started as the interface kit for vmac and lives in
its own workspace so it can grow into a general framework.

**Design goals**

- One look: neutral surfaces, one accent, Inter type, an 8-point spacing scale, continuous corners.
- Everything is drawn by a small set of shaders: signed-distance shapes, glyph atlas text and images
  with correct colour handling. No webview, no platform widgets in the way.
- Retained widget tree with a flexbox layout engine written for vui, springs for motion, and an
  accessibility tree generated from the same widgets that paint (the macOS backend hands it to AccessKit).
- Testable without a window: layout, events and painting run headless, and rendered frames are
  compared with golden images.
- Extensible: custom widgets implement one trait, custom shader passes plug into the renderer, themes
  are plain data.

**Layout**

| Crate | Job |
| --- | --- |
| `vui-core` | geometry, colour, theme tokens, springs, input events, text shaping and the scene description |
| `vui-fonts` | Inter and JetBrains Mono as bytes (SIL OFL 1.1) |
| `vui-render` | the wgpu renderer: shapes, text, images, custom passes, headless rendering and read-back |
| `vui-runtime` | elements and reconciliation, layout, hit-testing, input routing, focus, scrolling, animation and the accessibility tree; headless, so all of it is tested without a window |
| `vui-widgets` | the standard widget library |
| `vui` | the facade crate: `use vui::prelude::*` |
| `examples/gallery` | every widget in light and dark, used for golden-image tests |

**Status**: early. The renderer and the widget runtime work and are tested; the standard widgets and the macOS window backend come next.

**Tests**

```sh
cargo test --workspace
VUI_BLESS=1 cargo test --workspace   # rewrite the golden images after an intended visual change
```
