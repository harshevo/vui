# Rendering

vui draws everything itself: no web view, no platform widgets. A frame is a `Scene`, a flat list of
drawing commands that two renderers can draw:

- **The GPU renderer** (`vui::render::Renderer`) is built on wgpu: Metal on macOS, and Vulkan,
  DirectX 12 or OpenGL elsewhere.
- **The CPU renderer** (`vui::render::CpuRenderer`) is the fallback. It draws the same scene
  into memory with the same maths, for machines with no usable GPU: virtual machines, remote
  sessions and CI.

```text
 view ─► elements ─► layout ─► paint ─► Scene ─┬─► Renderer (wgpu) ─► window surface / texture
                                               └─► CpuRenderer ─────► pixels ─► window layer / PNG
```

## What a scene holds

| Command | GPU | CPU |
| --- | --- | --- |
| **Quads**: rounded boxes with fills, borders, continuous corners and soft clips | one instanced draw over a signed distance field (`shaders/quad.wgsl`, `sdf.wgsl`) | the same distance field, per pixel, with solid interiors filled as spans |
| **Text** | shaped by cosmic-text, rasterised by swash into a glyph atlas, drawn by glyphon | the same swash glyph images, blended the same way |
| **Pictures** | sRGB textures with a mip chain built on the GPU, filtered in linear light; sharp and nearest sampling for pixel art | the same mip chain and trilinear filter in linear light |
| **Meshes**: vector paths, strokes and fills from the vector engine (icons, charts) | triangles | triangles rasterised at pixel centres, top-left rule |
| **Effects**: shadows, glows, shimmers | a built-in pass (`shaders/effects.wgsl`) | the same formulas |
| **Shading**: gradients (linear, radial, conic, as fills or rings), aurora, border beam, spotlight, grain | a built-in pass (`shaders/shading.wgsl`), stops mixed in Oklab | the same formulas, including the grain's integer hash |
| **Layers**: a group of commands drawn on its own, then laid over the frame moved, scaled, turned and faded as one (`.scale`, `.rotate`, `.translate`) | drawn into a window-sized target, copied into a texture of its own and composited by `shaders/layer.wgsl`, which filters by hand | drawn into a canvas of its own and composited with the same filter, in the same order |
| **Custom passes** | your wgpu code; see [Custom widgets](custom-widgets.md#custom-gpu-passes) | skipped |

Colours are blended premultiplied, in the encoded sRGB the display expects, so the two renderers
agree. The golden tests compare them: every gallery page matches the GPU frame to within one level
per channel.

## High-density screens

Everything is laid out in points and drawn at the window's scale: 2 on a Retina display, 3 on a
phone-class screen, any fraction in between. Glyphs are rasterised at the scale they are drawn
at, edges are snapped to device pixels, and pictures pick the mip level that fits. When a window
moves to a display with another scale, macOS tells the view; vui resizes its surface and draws the
next frame at the new scale. A test holds that to what a window opened on that display would draw,
pixel for pixel, and the gallery keeps goldens at 1x, 2x and 3x.

## Choosing a renderer

Windows use the GPU. If no adapter can be had, the window falls back to the CPU renderer on its
own and shows its frames through a Core Animation layer. To force the CPU renderer, for
instance to check how the fallback looks:

```sh
VUI_RENDERER=cpu cargo run -p vui --example dashboard
VUI_RENDERER=cpu cargo test --workspace     # all golden tests on the CPU
```

The headless helpers in `vui::testing` follow the same rule: GPU when there is one, CPU when
there isn't or when `VUI_RENDERER=cpu`.

## Rendering without a window

```rust
use vui::core::{Resources, size};
use vui::prelude::*;
use vui::render::CpuRenderer;
use vui::runtime::Ui;

let res = Resources::bundled_only();
let mut ui: Ui<()> = Ui::new(res.clone(), Theme::light(), size(320.0, 120.0), 2.0);
ui.set_view(row().padding(16.0).gap(8.0).children((button("Cancel"), button("Save").primary())));
let scene = ui.paint();

let image = CpuRenderer::new(res).render_to_image(&scene);
assert_eq!((image.width, image.height), (640, 240)); // two device pixels per point
```

`Renderer::headless(resources)` does the same on the GPU, and `vui::testing::render_with(&scene, &res)`
picks whichever is available. `image.save_png(path)` writes the result.

## Snapshots of a running app

Any vui app can write its first settled frame to a file, which is handy for docs, bug reports and
checking a window without looking at the screen:

```sh
VUI_SNAPSHOT=shot.png VUI_SNAPSHOT_QUIT=1 cargo run -p vui --example todo
```

With several windows, the second writes `shot-1.png`, the third `shot-2.png`, and so on.

## Performance

- **Idle windows draw nothing.** Frames come from the display link only while something moves,
  and the window stops it the moment everything rests.
- **Reconciling and layout are incremental.** A widget that says `Dirty::Paint` is repainted
  without being laid out again, and unchanged subtrees keep their cached layout.
- **Text layouts are cached** by content and style. Glyphs are rasterised once into an atlas.
- **Pictures are decoded once.** They upload once and stay within a memory budget
  (`Renderer::set_image_budget`).
- **Charts thin long series** to what the pixels can show before drawing.
