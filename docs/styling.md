# Styling and theming

vui's default look is calm and current:

- **Surfaces.** A white sheet on a pale canvas, dark on darker at night.
- **Colour.** One soft indigo accent.
- **Shapes.** Pill buttons, soft fills, and continuous ("squircle") corners.
- **Type and spacing.** Inter type on an 8-point spacing scale.
- **Motion.** Springs for every movement.

Every colour, radius, type size and spring comes from the **theme**, so restyling an app means
changing data, not code.

## Colour comes from tones

Widgets and your own elements name a **tone**: a role in the palette, not a colour. The same view
then works in light and dark, and with any theme.

| Tone | Use |
| --- | --- |
| `Tone::Bg`, `Tone::Side` | the sheet, and sidebars or bars |
| `Tone::Text`, `Tone::Text2`, `Tone::Text3` | primary, secondary and tertiary text |
| `Tone::Line` | hairlines and borders |
| `Tone::Wash`, `Tone::Hover`, `Tone::Field` | soft fills, hover fills, input backgrounds |
| `Tone::Accent`, `Tone::OnAccent` | the accent, and text on it |
| `Tone::Danger`, `Tone::Success`, `Tone::Warning` | status |
| `Tone::Orange`, `Tone::Blue`, `Tone::Purple` | extra hues for tags and charts |
| `Tone::Ring`, `Tone::Scrim` | the focus ring, and the dimming behind dialogs |

Anything that takes a `Fill` accepts a `Tone`, a `Color` (`Color::hex(0x5b5bd6)`), or either with
transparency (`Fill::from(Tone::Accent).alpha(0.12)`).

## Decoration

```rust
use vui::prelude::*;

let tile: Div<()> = column()
    .padding(16.0)
    .rounded(Radius::Card)          // or Radius::Px(12.0), Radius::Pill, Radius::Control
    .bg(Tone::Bg)
    .border(1.0, Tone::Line)
    .shadow(Shadow::SOFT)           // SOFT, RAISED, FLOATING
    .hover_bg(Tone::Hover)          // fades in under the pointer
    .press_bg(Tone::Wash)           // and while pressed
    .opacity(0.9)
    .child(label("Hello"));
```

`card()` and `panel()` are ready-made surfaces with the theme's radius, border and padding.

## Motion

vui animates with springs, so motion is interruptible and never ends abruptly.

- **`.animated()`** animates changes to an element's size, insets, fill and opacity with the
  theme's spring. `.transition(spring)` uses another.
- **`.appear()`** fades an element in and lifts it into place when it first shows up. Key it by
  what it shows.
- **Built-in motion.** Controls animate on their own: switches slide, checkboxes draw their tick,
  focus rings fade, tabs and segmented controls move their marker, and menus scale in.
- **Effects.** `glow(on)` draws an animated ring around an element, `shimmer(on)` sweeps a sheen
  across it, and `skeleton(…)` gives a loading placeholder.

## The theme

`App::theme` returns the theme for each frame. The default follows the system's appearance:

```rust
use vui::prelude::*;

struct Settings { dark: Option<bool> }

impl App for Settings {
    type Msg = ();
    fn update(&mut self, _: (), _: &mut Cx<()>) {}
    fn view(&self) -> impl IntoElement<()> { label("…") }

    fn theme(&self, system: Mode) -> Theme {
        match self.dark {
            Some(true) => Theme::dark(),
            Some(false) => Theme::light(),
            None => Theme::for_mode(system), // follow the system
        }
    }
}
```

A theme is plain data. Start from one and change what you like:

```rust
use vui::prelude::*;
use vui::core::Color;

fn brand(mode: Mode) -> Theme {
    let mut theme = Theme::for_mode(mode);
    theme.palette.accent = Color::hex(0x0f766e);      // teal
    theme.radii.control = 8.0;                         // squarer buttons and fields
    theme.radii.card = 14.0;
    theme.radii.smooth = 0.6;                          // how continuous the corners are
    theme.types.body.size = 15.0;                      // a larger body size
    theme.control = 36.0;                              // control height
    theme
}
```

| Field | Holds |
| --- | --- |
| `palette` | every tone's colour |
| `radii` | `control`, `card`, `panel` and `smooth` (corner continuity) |
| `types` | the `small`, `body`, `title` and `mono` text styles: size, line height, weight |
| `motion` | the `snappy`, `smooth` and `morph` springs, and the `fast` duration |
| `control`, `control_large` | the heights of buttons, fields and segmented controls |

`Theme::island()` is a third built-in theme for dark, floating HUDs.

## Density

The defaults suit a desktop app. For denser tools, lower `control` and the type sizes. For touch,
raise them. Widgets read these values, so the whole app follows.
