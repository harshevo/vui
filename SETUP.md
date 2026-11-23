# Setting up vui

vui is the UI kit every other project in this family draws with. It is **self-contained**: it needs
no other checkout, so it is the first thing to set up and the easiest to try.

```text
vui  ◀── vresearch ◀── vtrade ◀── vmac        (an arrow means "is used by")
  ▲                                 ▲
  └───────────── vmac ──────────────┘
```

Inside the vmac workspace, vui sits at `vmac/vui` (a git submodule). vmac, vtrade, vresearch and the
shared notch all depend on it by path (`../vui/crates/vui`), so keep the folder name `vui`.

## What you need

| Need | Version | Why |
| --- | --- | --- |
| Rust (via [rustup](https://rustup.rs)) | **1.91 or newer** (the declared minimum; edition 2024) | everything |
| macOS: Xcode Command Line Tools | `xcode-select --install` | links AppKit, Metal and the Objective-C bindings |
| A GPU | optional | with none (a VM, CI, a remote session) the CPU renderer draws the same frame: `VUI_RENDERER=cpu` |
| Windows or Linux | optional | the winit backend builds and its tests run headless; menus, dialogs and notifications are macOS-only for now |

Nothing else: no Node, no Python, no system libraries to install, no API keys.

## Steps

### 1. Install the Rust toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
rustc --version      # 1.91 or newer
```

On a Mac, also run `xcode-select --install` if `git` or `clang` is missing.

### 2. Get the code

On its own:

```bash
git clone https://github.com/harshevo/vui.git
cd vui
```

Or as part of the vmac workspace, which is how the other projects find it:

```bash
git clone https://github.com/harshevo/vmac.git
cd vmac
git submodule update --init vui
```

### 3. Build

```bash
cargo build --workspace
```

The first build is slow: text shaping and image work are unusably slow unoptimised, so
dependencies are compiled with `opt-level = 2` even in dev builds. Later builds are fast.

### 4. Run the examples

```bash
cargo run -p vui --example hello        # the counter from the README
cargo run -p vui --example todo         # text input, lists, filters
cargo run -p vui --example dashboard    # sidebar, stat cards, a chart and a table: resize it
cargo run -p vui --example notes        # menus, shortcuts, file sheets, a menu-bar item
cargo run -p vui --example media        # a web page, a video and a PDF
cargo run -p gallery --release -- --window   # every widget, light and dark
```

Inside any vui window, ⌘⌥I opens the inspector and ⌘⌥S shows frame statistics.

### 5. Check that it all works

```bash
cargo test --workspace                     # goldens run on the GPU when there is one
VUI_RENDERER=cpu cargo test --workspace    # the same goldens on the CPU renderer
```

Both should pass with no changes to your tree. If a golden fails on your machine only, see
[Troubleshooting](#troubleshooting).

## Using vui in your own project

vui is not on crates.io. Depend on it by git or by path:

```toml
[dependencies]
vui = { git = "https://github.com/harshevo/vui" }       # or { path = "../vui/crates/vui" }
```

Then `use vui::prelude::*;`. [`docs/getting-started.md`](docs/getting-started.md) walks from an empty
crate to a working window. Optional crates: `vui-macos` (the macOS backend), `vui-viz` (charts and graphs)
and `vui-winit` (Windows and Linux).

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| `package requires rustc 1.91` | `rustup update stable` |
| Linker errors about frameworks on a Mac | `xcode-select --install`, then rebuild |
| The first `cargo build` seems stuck | it is compiling optimised dependencies; give it a few minutes |
| A window opens blank or crashes on a machine with no GPU | `VUI_RENDERER=cpu cargo run …` |
| A golden test fails | open the frame and the highlighted diff written to the crate's `tests/actual/`; if it is a font or GPU difference on your machine rather than your change, compare with `VUI_RENDERER=cpu` |

## Where to go next

- [`CONTRIBUTING.md`](CONTRIBUTING.md): the layout, the rules, and how to send a change.
- [`docs/`](docs): the guides, one per topic. Every Rust snippet in them is a doctest.
- [`CHANGELOG.md`](CHANGELOG.md): what changed.
