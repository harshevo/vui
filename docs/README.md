# vui guides

Start with **[Getting started](getting-started.md)**, then read what your app needs.

| Guide | Covers |
| --- | --- |
| [Getting started](getting-started.md) | adding vui, a first app, running the examples |
| [The app model](app-model.md) | state, messages and `update`; `Cx` (focus, timers, background work); keys; several windows and overlays; other threads |
| [Layout](layout.md) | rows and columns, sizes, alignment, scrolling, layers, **responsive layouts** |
| [Styling and theming](styling.md) | tones, decoration, motion, the theme's tokens, dark mode, your own theme |
| [Widgets](widgets.md) | the catalogue, and how controlled widgets work |
| [Text, icons and images](text-icons-images.md) | type styles, custom fonts, the icon set, pictures |
| [Charts and graphs](charts.md) | `vui::viz`: charts, sparklines, force-directed graphs |
| [Custom widgets](custom-widgets.md) | the `Widget` trait, painting, input, accessibility, custom GPU passes |
| [Rendering](rendering.md) | the GPU renderer, the CPU fallback, headless frames and snapshots |
| [Testing](testing.md) | `TestUi`, golden images, CI without a GPU |
| [Accessibility](accessibility.md) | roles, names, keyboard focus, and testing them |
| [Roadmap](roadmap.md) | what exists, and what comes next |

Every Rust snippet in these guides is compiled and run as a doctest (`cargo test -p vui --doc`), so
the guides keep up with the API.
