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
- Exit animations: `.disappear()` keeps a dropped keyed element in place, fading and sinking,
  without input or focus, until it has gone.
- Keyframes: `Keyframes<T>` with eased stops, repeats and alternation; `.animate(keyframes)` runs
  them over an element's `Pose` (opacity, scale, rotation, translation).
- Accessibility settings: `Preferences` (reduce motion, high contrast, text scale),
  `Ui::set_preferences`, and `Theme::adapted`. macOS windows follow Reduce Motion and Increase
  Contrast. Widgets get `TickCx::step`, `TickCx::still` and `PaintCx::effect_time`.
- Platform: `App::menus` and `App::status_item` with `Menu`, `MenuItem` and `Shortcut`;
  `cx.open_files`, `cx.save_file`, `cx.notify`, `cx.full_screen` and `cx.toggle_full_screen`.
  Answers arrive through `Ui::answer` and `Ui::choose_menu`, and `TestUi::choose` plays the user
  in tests. The macOS backend shows sheets, installs the menus and the status item, and goes full
  screen. New `notes` example.
- Notifications with buttons: `Notification::action`, `cx.notify_then` and
  `NotificationResponse` (clicked, an action, dismissed), answered through `Ui::answer`, which
  now takes an `Answer`. `TestUi::respond` and `TestUi::notification_action` play the person in
  tests. On macOS they go through `UNUserNotificationCenter` (`objc2-user-notifications`), in
  place of the deprecated `NSUserNotificationCenter`; `Effect::Notify` carries an id.
- Windows and Linux: `vui-winit`, a winit backend with the same `run` / `Application` / `Proxy`
  surface as `vui-macos`: windows and overlays, pointer, keyboard and IME input, the clipboard,
  cursors, vsync frames that stop when idle, and the CPU fallback shown on any adapter, software
  ones included (`Gpu::software_for_surface`). Ctrl is the shortcut modifier there. `vui` picks
  the backend by target, and the shared pieces (`WindowOptions`, `OverlayOptions`, `Proxy`,
  `Error`, the host) move to the new `vui-shell`, re-exported by both backends.
- Media: `web_view`, `video` (play, pause, loop, mute) and `pdf_view` (pages) take a place in
  layout and paint a placeholder. On macOS they are WKWebView, an AVPlayerLayer and PDFKit's
  `PDFView`, laid over the window where layout puts them and cut to scroll views; headless,
  golden and winit frames show the placeholder. `PaintCx::embed`, `Scene::embeds`, `Ui::embeds`
  and `Effect::Embeds` carry them. New `media` example.
- Dev tools: an inspector overlay (Cmd-Option-I) and frame stats (Cmd-Option-S) in every
  window, also through `VUI_INSPECT=1`, `VUI_STATS=1`, `Ui::set_inspector` and
  `Ui::set_frame_stats`.
- Project files: workspace metadata and lints, CI on macOS and Linux, a frame benchmark, this
  changelog and contributor notes.
- Licence: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE` and `license` in every crate).
  `vui-fonts` is `(MIT OR Apache-2.0) AND OFL-1.1` for its bundled fonts.

## [0.1.0] - 2026-10-26

### Added

- The workspace: core, runtime, widgets, icons, charts and graphs, the GPU and CPU renderers, the
  macOS backend, bundled fonts and the gallery.
