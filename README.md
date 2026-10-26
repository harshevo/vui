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
| `vui-icons` | line icons on a 24 point grid, rasterised by vui at any size and weight, and a spinner |
| `vui-widgets` | buttons, switches, checkboxes, radios, segmented control, sliders, progress, text fields and areas with a full editor behind them, lists and a sidebar, badges, avatars, picture tiles and form layout |
| `vui` | the facade crate: `use vui::prelude::*` |
| `examples/gallery` | every widget in light and dark as an app that runs headless; its pages and the states of its controls are golden images |

**Status**: early. The renderer, the widget runtime and the standard widgets work and are tested. Overlays (menus, select, tooltips, dialogs, toasts) and the macOS window backend come next.

**Gallery**

The gallery is an ordinary vui app: a sidebar and eight pages (foundations, buttons, controls, inputs, lists, images, icons, settings). Until the window backend exists it runs without a window, and renders each page, light and dark, to a PNG:

```sh
cargo run -p gallery --release -- target/gallery            # every page
cargo run -p gallery --release -- target/gallery buttons    # one page
```

**Tests**

```sh
cargo test --workspace
VUI_BLESS=1 cargo test --workspace   # rewrite the golden images after an intended visual change
```

Golden images live in each crate's `tests/golden`. A failing comparison writes the frame and a highlighted diff to `tests/actual`; look at both before deciding the change was meant.
