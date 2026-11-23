# Contributing to vui

Thank you for helping. This page covers how to get set up, where things live, the rules every change
follows, and how a change travels from your branch to the other projects that use vui.

## Getting set up

Follow [SETUP.md](SETUP.md): Rust 1.91+, Xcode Command Line Tools on a Mac, then
`cargo test --workspace`. vui needs no other checkout. If you also work on the apps that use it
(vmac, vtrade, vresearch), clone vmac and run `git submodule update --init vui`; those projects
build against the `vmac/vui` folder, so your edits show up in them immediately.

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

## Sending a change

1. **Branch** from `main`: `git switch -c short-name`.
2. **Make the change small and whole.** One idea per commit; a commit builds and passes its own
   tests. Commit messages say what changed and why, in plain words.
3. **Format only the files you touched.** There is no `rustfmt.toml`; the code is written wide:

   ```sh
   rustfmt --edition 2024 --config max_width=200,use_small_heuristics=Max path/to/file.rs
   ```

   Never run `cargo fmt --all`: it reflows every file and buries your change. Check the diff
   afterwards and put back any lines you did not mean to touch.
4. **Run the checks** from [Before sending a change](#before-sending-a-change).
5. **Stage by path**, not with `git add -A`: `git add crates/vui-widgets/src/… docs/… CHANGELOG.md`.
   Run `git status` first and make sure nothing else is going in.
6. **Open a pull request.** Say what you changed, what you tested, and attach the before and after
   frames for a visual change.

What CI runs (`.github/workflows/ci.yml`): clippy with warnings denied, the full test suite on macOS
with the GPU renderer and the CPU renderer and on Linux with the CPU renderer, a Windows check of
the winit backend, a build on the minimum Rust version (1.91), and the docs. Run the first three
locally before you push.

## Golden images

A visual change moves a golden frame. That is expected; it must be intended and reviewed.

- A failing comparison writes the new frame and a highlighted diff to the crate's `tests/actual/`.
  Open both. If the change is what you wanted, re-bless with `VUI_BLESS=1 cargo test -p <crate>`
  (the gallery's tests first, since its CPU test compares against them) and commit the new PNGs
  with the code that caused them.
- Run the whole suite once more **without** `VUI_BLESS` to prove the new goldens pass on both
  renderers.
- If more than one person or agent works in the same checkout, other people's unfinished edits move
  goldens too. Bless in a clean copy (`git worktree add --detach ../vui-clean HEAD`, copy in only your
  files) so you do not bake someone else's half-done work into the images.

## Accessibility and look

New widgets follow the house look: text tones at least 4.5:1 against their background in every
theme (a test enforces it), spacing on the 4/8/12/16/24/32 scale, controls 28 pt tall in a row, and
every clickable element reachable and named for the keyboard and the accessibility tree.

## How a change reaches the apps

vmac records the exact vui commit it builds against (a git submodule). vtrade and vresearch build
against whatever is checked out at `../vui`. So:

1. Land the change in vui and push it.
2. In vmac: `cd vui && git pull`, then `cd .. && git add vui && git commit` to move the recorded
   commit. If the change moves golden frames in vmac, vtrade or vresearch, re-bless those in their
   own repositories.

Removing or renaming public API is a breaking change: say so in `CHANGELOG.md` and update the
apps in the same sitting.

## Licence

Unless you say otherwise, any contribution you submit for inclusion in vui is dual-licensed under
MIT OR Apache-2.0, as the [README](README.md#licence) describes, without any additional terms or
conditions.
