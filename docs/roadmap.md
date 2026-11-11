# Roadmap

vui already does the hard parts well:

- a GPU renderer with a matching CPU fallback;
- a flexbox engine with responsive rules;
- a complete text editor with input-method support;
- an accessibility tree;
- springs everywhere;
- headless testing with golden images.

This page is an honest audit of what a general-purpose UI framework still needs, in order of
importance. Each item says what exists today.

## What exists

| Area | State |
| --- | --- |
| Rendering | wgpu GPU renderer (quads, text, pictures, meshes, effects, shading, custom passes) and a CPU renderer that matches it; automatic fallback; any display scale, followed when a window changes display |
| Layout | flexbox (grow, shrink, basis, wrap, alignment, gaps, auto margins), absolute, fixed and floating layers, scrolling, responsive `.when` rules |
| Widgets | 70+: buttons, controls, fields and text areas, lists, tables, trees, tabs, menus, select, popovers, dialogs, toasts, alerts, progress, skeletons, markdown, forms |
| Text | shaping, wrapping, ellipsis, fallback for every script, custom fonts, a full editor with IME, undo and the clipboard |
| Icons | 157 line icons, four weights, `icon!` for your own |
| Data | charts (line, step, area, grouped and stacked bars, volume, scatter, candles), pies and donuts, bar lists, sparklines, force-directed graphs |
| Motion and effects | springs, transitions, appear animations, transforms (scale, rotate, translate) drawn as layers, built-in control motion, glow, shimmer; a shading pass for gradients (linear, radial, conic, borders), aurora, border beam, spotlight and grain |
| Theming | light, dark and island themes; every colour, radius, type size and spring is data |
| Platform | macOS: windows, overlays, input method, clipboard, cursors, file drops, display-link frames, a thread-safe `Proxy`, app menus and shortcuts, open and save sheets, notifications with action buttons (`UNUserNotificationCenter`), a menu-bar item, full screen, Reduce Motion and Increase Contrast |
| Testing | `TestUi` for input and time; golden images on the GPU or CPU; accessibility-tree assertions |

## Next, in order

### 1. Screen readers on macOS

**Today.** The accessibility tree, actions and keyboard navigation are complete and tested, but no
bridge hands them to VoiceOver.

**Plan.** Publish the tree through AccessKit's macOS adapter, or `NSAccessibility` directly. Map
`AccessAction`s back, and announce live regions such as toasts.

### 2. Windows and Linux

**Today.** Everything above the window is portable: the runtime, the widgets, and both renderers
(wgpu already targets DirectX 12, Vulkan and GL). Only `vui-macos` is platform code.

**Plan.** Add a backend per platform with the same `run` / `Application` / `Proxy` surface, built
from:

- a window and surface;
- pointer, keyboard and IME events;
- the clipboard and cursors;
- vsync-driven frames that stop when idle;
- a layer for the CPU renderer.

A winit-based backend is the fastest path to both platforms. It is a new dependency, so it needs a
decision.

### 3. Large data

**Today.** `virtual_list(count, row_height, |i| row)` builds only the rows in view; see
[Layout](layout.md#long-lists). **Next:**

- **Variable row heights**, measured as rows come into view.
- **Incremental tree building**, for very long documents.

### 4. Layout

**Today.** Grids with pixel, `fr`, auto and filling columns, spans and auto rows; container queries
with `.when_container`. See [Layout](layout.md#grids). **Next:**

- **Grid areas and row spans.** Named areas and cells taller than one row.
- **Right-to-left.** Mirrored layout and RTL text direction for Arabic and Hebrew. Shaping
  already handles the scripts.

### 5. Widgets

**Today.** Combobox, command palette, calendar, date picker, time field, colour picker, tag input,
drawer and sheet, hover card, stepper, toolbar and carousel; see [Widgets](widgets.md). **Next:**

- resizable panel groups with minimum sizes and collapse;
- a code editor with syntax highlighting (on top of the existing editor);
- a rich-text editor.

### 6. Motion and preferences

**Today.** Transforms: `.scale`, `.rotate` and `.translate`, sprung with `.animated()`, drawn as
layers on both renderers. Exit animations with `.disappear()`, and keyframes with `.animate()`.
See [Styling](styling.md#transforms). Reduce Motion and Increase Contrast are followed, and text
scales with `text_scale`; see [Accessibility](accessibility.md#system-settings). **Next:**

- **Shared-element transitions.** An element that moves between two places in the tree, such as a
  thumbnail opening into a detail view, flies between them.

### 7. Platform features

**Today.** App menus with shortcuts, ticks and disabled items from state; open and save sheets;
notifications through `UNUserNotificationCenter` with action buttons, answered as app messages
(`cx.notify_then`) and playable in `TestUi`; a menu-bar item; full screen. See
[The app model](app-model.md#menus-shortcuts-and-the-menu-bar). **Next:**

- drag out of the app (as a drag source);
- submenus, and icons in menus and the menu-bar item.

### 8. Developer experience

**Today.** An inspector and frame stats in every window. **Next:**

- **Async.** `cx.spawn` runs a closure on a thread. Add `cx.spawn_async(future)` for async
  runtimes, and streams of messages.
- **Components with local state.** Apps own all state today, and `Element::map` composes
  sub-apps. A small `component` helper would keep purely visual state (an open menu, a hover)
  out of the app.
- **A fuller inspector.** The inspector and frame stats exist (see
  [Testing](testing.md#looking-inside-a-running-app)). Next: pin an element, walk the tree, and
  show the styles and the accessibility tree.
- **Hot reload** of styles and themes from a file.
- **Rustdoc for every public item** (`#![warn(missing_docs)]`) and docs.rs.

### 9. Performance

**Today.** Idle windows cost nothing, and layout and text are cached. **Next:**

- **Parallel CPU rendering.** Rows are independent, so bands of the frame can draw on all cores.
- **Damage tracking.** Redraw only the changed region on the GPU, and reuse unchanged layers.
- **Tiled offscreen frames.** A headless frame taller or wider than the GPU's texture limit (8192
  device pixels: a long page at 3x) fails today; draw it in tiles.
- **A benchmark suite** for frame time and memory on the gallery and examples.

### 10. Project

**Today.** MIT OR Apache-2.0, with the bundled fonts under SIL OFL 1.1 (see the
[README](../README.md#licence)), and CI on macOS and Linux. **Plan:**

- **Publishing.** Release on crates.io with semantic versioning.
