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
| Rendering | wgpu GPU renderer (quads, text, pictures, meshes, effects, custom passes) and a CPU renderer that matches it; automatic fallback |
| Layout | flexbox (grow, shrink, basis, wrap, alignment, gaps, auto margins), absolute, fixed and floating layers, scrolling, responsive `.when` rules |
| Widgets | 70+: buttons, controls, fields and text areas, lists, tables, trees, tabs, menus, select, popovers, dialogs, toasts, alerts, progress, skeletons, markdown, forms |
| Text | shaping, wrapping, ellipsis, fallback for every script, custom fonts, a full editor with IME, undo and the clipboard |
| Icons | 157 line icons, four weights, `icon!` for your own |
| Data | charts (line, step, area, bars, volume, scatter, candles), sparklines, force-directed graphs |
| Motion | springs, transitions, appear animations, built-in control motion, glow, shimmer |
| Theming | light, dark and island themes; every colour, radius, type size and spring is data |
| Platform | macOS: windows, overlays, input method, clipboard, cursors, file drops, display-link frames, a thread-safe `Proxy` |
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

**Today.** Missing. **Plan:**

- **Virtualised lists and tables.** Lay out and paint only the visible rows, so lists of 100,000
  items scroll smoothly. Rows are keyed already; this needs a `virtual_list(count, row_height, |i| row)`
  with variable heights measured on demand.
- **Incremental tree building**, for very long documents.

### 4. Layout

**Today.** Missing. **Plan:**

- **Grid layout.** Rows and columns with tracks (`fr`, fixed, auto, minmax) and spans, for dashboards,
  galleries and forms. Wrapping flex rows cover many cases today.
- **Container queries.** `.when` follows the window's width. Components should also adapt to
  their own width (a card that changes layout in a narrow column).
- **Right-to-left.** Mirrored layout and RTL text direction for Arabic and Hebrew. Shaping
  already handles the scripts.

### 5. Widgets

**Today.** Missing. Wanted, from what modern design systems ship:

- combobox and autocomplete;
- command palette (⌘K);
- date picker, calendar and time picker;
- colour picker;
- tag input;
- drawer and sheet (side and bottom);
- hover card;
- stepper and wizard;
- toolbar;
- resizable panel groups with minimum sizes and collapse;
- carousel;
- a code editor with syntax highlighting (on top of the existing editor).

### 6. Motion and preferences

**Today.** Missing. **Plan:**

- **Exit animations.** Elements fade out before they're removed (`.disappear()`), to pair with
  `.appear()`.
- **Reduced motion.** Follow the system setting, turning springs into fades or cuts.
- **Increased contrast and larger text.** Follow the system settings, with a high-contrast palette
  and type scaling.
- **Theme fonts.** Wire `Theme::font_sans` and `font_mono` through to text, so a theme can switch
  the interface font. Today `Label::font` does it per label.

### 7. Platform features

**Today.** Missing. **Plan:**

- app-defined menu bar items and keyboard shortcuts (today the menu bar is the standard one);
- open and save panels;
- drag out of the app (as a drag source);
- notifications;
- a status-bar item;
- per-window full screen;
- moving between displays with different scales.

### 8. Developer experience

**Today.** Missing. **Plan:**

- **Async.** `cx.spawn` runs a closure on a thread. Add `cx.spawn_async(future)` for async
  runtimes, and streams of messages.
- **Components with local state.** Apps own all state today, and `Element::map` composes
  sub-apps. A small `component` helper would keep purely visual state (an open menu, a hover)
  out of the app.
- **A visual inspector.** An overlay that shows boxes, keys, styles and the accessibility tree
  (`Ui::inspect` exists programmatically).
- **Hot reload** of styles and themes from a file.
- **Rustdoc for every public item** (`#![warn(missing_docs)]`) and docs.rs.

### 9. Performance

**Today.** Idle windows cost nothing, and layout and text are cached. **Next:**

- **Parallel CPU rendering.** Rows are independent, so bands of the frame can draw on all cores.
- **Damage tracking.** Redraw only the changed region on the GPU, and reuse unchanged layers.
- **A benchmark suite** for frame time and memory on the gallery and examples.

### 10. Project

**Today.** Missing. **Plan:**

- **A licence.** None is set yet. MIT or Apache-2.0 is usual for Rust crates; the bundled fonts
  are under SIL OFL 1.1.
- **CI.** Tests on macOS, plus the CPU renderer on Linux.
- **Publishing.** Release on crates.io with semantic versioning, and a changelog.
