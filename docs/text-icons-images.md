# Text, icons and images

## Text

Text is shaped with real font shaping (ligatures, kerning, every script), wrapped by the layout,
and drawn from a glyph atlas on the GPU. Inter is the interface font and JetBrains Mono the
monospace one; both ship inside vui, so text looks the same on every machine. Installed fonts are
used as fallback for scripts Inter lacks, such as Chinese, Japanese, Korean and Arabic, and for
emoji.

| Function | Style |
| --- | --- |
| `label("…")` | body text |
| `title("…")` | a heading |
| `small("…")` | captions and secondary lines |
| `label("…").mono()` | code |

```rust
use vui::prelude::*;
use vui::core::TextAlign;

let card: Div<()> = column().gap(4.0).children((
    title("Quarterly report"),
    label("Revenue grew in every region, led by the new stores.").max_lines(2),
    small("Updated 5 minutes ago").color(Tone::Text3),
    label("42").font_size(40.0).weight(600).align_text(TextAlign::Center),
    label("cargo run --release").mono(),
));
```

| Method | Effect |
| --- | --- |
| `.color(Tone::Text2)` | colour, from a tone or any `Color` |
| `.weight(400..=700)` | weight |
| `.font_size(18.0)` | size; the line height scales with it |
| `.max_lines(n)` | ends the text with an ellipsis after `n` lines |
| `.no_wrap()` | keeps the text on one line |
| `.align_text(TextAlign::Center)` | alignment |
| `.text_role(TextRole::Title)` | any of the theme's styles |

For formatted text, `markdown(text)` renders headings, lists, emphasis, code and links, and
`rich_text(text)` renders the inline parts on their own.

### Your own fonts

Add a font from its bytes, then ask for its family by name:

```rust,no_run
use vui::prelude::*;

struct App1;
impl App for App1 {
    type Msg = ();
    fn update(&mut self, _: (), _: &mut Cx<()>) {}
    fn view(&self) -> impl IntoElement<()> {
        label("Hello in Geist").font("Geist")
    }
}

fn main() {
    vui::run(WindowOptions::new("Fonts"), |resources| {
        let bytes = std::fs::read("Geist-Regular.otf").expect("the font file");
        let families = resources.fonts.add_font(bytes); // ["Geist"]
        assert!(!families.is_empty());
        App1
    })
    .unwrap();
}
```

## Icons

vui has 157 line icons on a 24-point grid, drawn by vui's own vector engine at any size and in
four weights. The set covers:

- navigation and arrows;
- actions (copy, trash, edit, share, undo);
- files, media and devices;
- people and communication;
- security (lock, key, shield, fingerprint);
- commerce (cart, card, receipt);
- charts and data;
- text formatting.

Every one is in `icon_set`:

```rust
use vui::prelude::*;

let row_: Div<()> = row().gap(12.0).align_items(Align::Center).children((
    icon(icon_set::BELL),
    icon(icon_set::SHIELD_CHECK).color(Tone::Success),
    icon(icon_set::SPARKLES).weight(Weight::Bold).square(24.0),
    button("Share").icon(icon_set::SHARE),
    icon_button(icon_set::MORE, "More"),
));
```

Icons take their colour from the text by default. `.color(…)` tints one, and `.weight(Weight::Light)`
through `Weight::Bold` sets the stroke. The gallery's Icons page shows the whole set
(`cargo run -p gallery --release -- --window`).

To add your own, describe it with SVG path data, as the built-in set does:

```rust
use vui::icons::{Icon, icon};

const ROCKET: Icon = icon!("rocket", Stroke "M12 15l-3-3a22 22 0 0 1 2-4 13 13 0 0 1 11-6c0 3-1 8-6 11a22 22 0 0 1-4 2zM9 12H4s.6-3 2-4c1.6-1 5 0 5 0M12 15v5s3-.6 4-2c1-1.6 0-5 0-5");
```

## Images

Register a picture once, then draw it by its id. Pictures are decoded in Rust (PNG, JPEG, WebP,
GIF, BMP, TIFF; EXIF rotation applied), mipmapped, and filtered in linear light, so scaled photos stay sharp and correct.

```rust,no_run
use vui::prelude::*;
use vui::core::{Fit, ImageData, ImageId};

struct Gallery { hero: ImageId }

impl App for Gallery {
    type Msg = ();
    fn update(&mut self, _: (), _: &mut Cx<()>) {}
    fn view(&self) -> impl IntoElement<()> {
        column().gap(12.0).padding(16.0).children((
            img(self.hero).fit(Fit::Cover).fill_width().aspect_ratio(16.0 / 9.0).rounded(Radius::Card),
            row().gap(8.0).children((
                avatar_image(self.hero, 40.0),
                image_tile(Some(self.hero)).square(96.0),
            )),
        ))
    }
}

fn main() {
    vui::run(WindowOptions::new("Pictures"), |resources| {
        // Decode at most 2048 pixels on the long side: enough for the screen, far less memory.
        let photo = ImageData::load(std::path::Path::new("hero.jpg"), Some(2048)).expect("a picture");
        Gallery { hero: resources.images.insert(photo) }
    })
    .unwrap();
}
```

- **Fit.** `Fit::Contain` shows the whole picture, `Fit::Cover` fills the box and crops, and
  `Fit::Fill` stretches.
- **Sampling.** `.sampling(Sampling::Sharp)` keeps pixel art crisp when scaled up, and
  `Sampling::Nearest` gives hard pixels.
- **Tint.** `.tint(Tone::Accent)` draws a single-colour picture (a logo, a glyph) as a mask in that
  colour.
- **Loading.** Load large pictures off the main thread with `cx.spawn`, then insert them when they
  arrive.
- **Placeholders.** `ImageData::thumb_hash()` gives a tiny blurred stand-in to show while the
  picture loads.
