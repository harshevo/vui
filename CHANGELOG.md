# Changelog

All notable changes to vui are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) once 1.0 ships; until then a minor version may break
the API.

## [Unreleased]

### Added

- Shading pass on both renderers: linear, radial and conic gradients as fills and borders; the
  `aurora`, `border_beam` and `spotlight` effects; film grain.
- Charts: grouped and stacked bars, category axes, market candles in up and down colours, pies and
  donuts with hover, bar lists.
- Sliders: `range_slider`, upright sliders, tick marks and a value bubble.
- High-density screens: windows follow display moves, and goldens are kept at 1x, 2x and 3x.
- Theme fonts: `Theme` chooses the interface and monospace families.
- Layout: `grid` with pixel, `fr`, auto and filling columns, spans and auto rows; `gap_x` and
  `gap_y`; container queries with `.when_container`; `virtual_list` for lists of any length.
- Widgets: `combobox` and `tag_input`; `command_palette`; `calendar`, `date_picker` and
  `time_field` over a new `Date`; `color_picker`; `drawer` and `sheet`; `hover_card`; `stepper`;
  `toolbar`; `carousel`. Fields gain `.pass_keys()` and `.bare()`, popovers `.keep_focus()`, and
  `Color` gains `to_hsv`, `from_hsv` and `to_hex`.
- Transforms: `.scale`, `.scale_xy`, `.rotate`, `.translate` and `.transform_origin`, sprung
  with `.animated()`. They are drawn as a new `Command::Layer` (an `Affine` and the commands
  inside it) that both renderers composite the same way. `Scene::walk` visits commands inside
  layers.
- Project files: workspace metadata and lints, CI on macOS and Linux, a frame benchmark, this
  changelog and contributor notes.

## [0.1.0] - 2026-10-26

### Added

- The workspace: core, runtime, widgets, icons, charts and graphs, the GPU and CPU renderers, the
  macOS backend, bundled fonts and the gallery.
