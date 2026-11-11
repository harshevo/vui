# Contributing to vui

## Layout of the repository

```text
crates/
  vui/            the facade: prelude, examples (examples/), benchmarks (benches/)
  vui-core/       geometry, colour, theme, animation, input, text, images, vectors, scene
  vui-runtime/    elements, reconciliation, layout, events, focus, accessibility
  vui-widgets/    stock widgets, one folder per feature (buttons, controls, inputs, ...)
  vui-icons/      the icon set and its rasteriser
  vui-viz/        charts, pies, bar lists, graphs
  vui-render/     the wgpu renderer, the CPU renderer, shaders (src/shaders/)
  vui-macos/      the macOS backend
  vui-fonts/      bundled fonts
examples/gallery/ every widget as an app; its pages are golden images
docs/             the guides; every Rust snippet in them is a doctest
```

Inside a crate, each feature is a module folder (`src/<feature>/mod.rs` and its parts). Unit tests
sit next to the code in a `tests` module or `tests/` subfolder; integration tests are in the
crate's `tests/`, golden frames in `tests/golden/`.

## Rules of the house

- **No source file over 500 lines.** Split by feature, and name the parts for what they hold.
- **The UI logic is vui's own.** Layout, widgets, input and drawing are written here. Crates are
  used for platform and format plumbing only (wgpu, AppKit bindings, image decoders, text shaping),
  and their types do not appear in vui's public API. Open an issue before adding a dependency.
- **Both renderers.** Anything the GPU draws, the CPU renderer draws too, to within one level per
  channel. The golden tests check it.
- **Guides are tests.** A new feature gets a snippet in the right guide under `docs/`, and a line in
  `CHANGELOG.md`.

## Before sending a change

```sh
cargo clippy --workspace --all-targets     # no warnings; the workspace lints are in Cargo.toml
cargo test --workspace                     # GPU goldens where there is a GPU
VUI_RENDERER=cpu cargo test --workspace    # the same goldens on the CPU renderer
cargo bench -p vui --bench frame           # when the change touches layout or painting
```

A failing golden comparison writes the frame and a highlighted diff to the crate's `tests/actual/`.
If the change is intended, look at both, then re-bless with `VUI_BLESS=1` (the gallery's golden
tests first, since its CPU test compares against them).

## Licence

Unless you say otherwise, any contribution you submit for inclusion in vui is dual-licensed under
MIT OR Apache-2.0, as the [README](README.md#licence) describes, without any additional terms or
conditions.
