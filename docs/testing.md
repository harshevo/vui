# Testing

Everything in vui except the window itself runs headless:

- layout, input routing, focus, animation, accessibility and painting;
- text, with the bundled fonts only, so frames are identical on every machine;
- timers and springs, on a clock you move by hand.

You can test an app the way someone uses it, in a plain `#[test]`, with no window, no display
and no GPU.

## Driving an app with `TestUi`

```rust
use vui::prelude::*;
use vui::testing::TestUi;
use vui::core::NamedKey;

#[derive(Clone, Debug, PartialEq)]
enum Msg { Name(String), Submit }

#[derive(Default)]
struct Signup { name: String, sent: bool }

impl App for Signup {
    type Msg = Msg;
    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Name(n) => self.name = n,
            Msg::Submit => self.sent = !self.name.is_empty(),
        }
    }
    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(16.0).gap(8.0).children((
            text_field(&self.name).key("name").on_change(Msg::Name).on_submit(|_| Msg::Submit),
            button("Join").primary().key("join").on_click(Msg::Submit),
        ))
    }
}

let mut t = TestUi::with_app(400.0, 200.0, Signup::default());
t.click_key("name").type_text("Ada").key(NamedKey::Enter);
assert!(t.app::<Signup>().sent);
assert_eq!(t.app::<Signup>().name, "Ada");
```

| Helper | Does |
| --- | --- |
| `click_key("save")`, `click(pt)`, `double_click`, `right_click` | clicks, by key or position |
| `move_to(pt)`, `move_to_key`, `leave` | hovering |
| `press` / `release` (+ `_button`) | drags, step by step |
| `wheel(pt, dx, dy)`, `trackpad(…)` | scrolling with a mouse wheel or a trackpad |
| `type_text("…")`, `key(NamedKey::Tab)`, `key_with(key, mods)`, `shortcut('c')`, `tab()`, `shift_tab()` | the keyboard |
| `ime(…)` | input-method composition |
| `drag_over(pt, paths)`, `drop_files(pt, paths)` | file drag and drop |
| `advance(seconds)`, `settle()` | moves the clock: timers fire and springs move |
| `rect("key")`, `center("key")`, `inspect("key")` | where an element is, and its state |
| `messages()`, `effects()` | what a bare view sent, and what the UI asked of the host (clipboard, cursor, quit) |
| `app::<A>()`, `update_app(…)` | read or change the hosted app's state |
| `scene()` | the frame's drawing commands |
| `ui.access_tree()` | the accessibility tree, to check roles and names |

To test a piece of view without an app, show it with `t.view(…)` and read `t.messages()`.

## Golden images

A golden test renders a frame and compares it with a PNG kept in the repository. Use them for
anything visual: a page in light and dark, a control in each of its states, a layout at a
phone-sized width.

```rust
use vui::core::{Resources, size};
use vui::prelude::*;
use vui::runtime::Ui;
use vui::testing::{TestUi, assert_golden, render_with};

struct Card;
impl App for Card {
    type Msg = ();
    fn update(&mut self, _: (), _: &mut Cx<()>) {}
    fn view(&self) -> impl IntoElement<()> {
        column().padding(16.0).child(card().padding(16.0).child(label("Golden")))
    }
}

#[test]
fn card_light() {
    let res = Resources::bundled_only();
    let mut t = TestUi { ui: Ui::with_app(res.clone(), size(240.0, 120.0), 1.0, Card) };
    t.advance(0.6); // let entrance animations finish
    assert_golden!("card_light", render_with(&t.scene(), &res));
}
```

- **Where they live.** Goldens are kept in `tests/golden/<name>.png` beside the crate's
  `Cargo.toml`.
- **When a comparison fails.** The actual frame and a highlighted diff are written to
  `tests/actual/`. Look at both before deciding the change was meant.
- **Re-blessing.** After an intended visual change, re-bless and review the new files before
  committing them:

  ```sh
  VUI_BLESS=1 cargo test -p my-app
  ```
- **Renderer tolerance.** The comparison tolerates the one-level differences between GPU vendors
  and between the GPU and CPU renderers. `VUI_RENDERER=cpu cargo test` runs every golden test on
  the CPU.

`crates/vui/tests/examples.rs` tests the three examples this way, including their narrow layouts.
`examples/gallery/tests/` keeps every gallery page, light and dark.

## Testing on CI

The workspace's tests need no display. On a CI machine without a GPU they render on the CPU
automatically. Keep text deterministic with `Resources::bundled_only()`; `TestUi` does this for
you.

```sh
cargo test --workspace
VUI_RENDERER=cpu cargo test --workspace   # what a GPU-less CI machine runs
```

## Looking inside a running app

Every vui window has two overlays for finding out why something looks the way it does:

- **The inspector** (Cmd-Option-I) outlines the element under the pointer, shades its padding, and
  shows a card with its widget, key, size and position, padding, accessibility role, and how deep
  it sits.
- **Frame stats** (Cmd-Option-S) show frames per second while something moves, how long the last
  paint took (layout included), and how many elements there are.

Start a window with them on through the environment:

```sh
VUI_INSPECT=1 VUI_STATS=1 cargo run -p vui --example dashboard
```

From code, use `Ui::set_inspector(true)` and `Ui::set_frame_stats(true)`. They work headless too, so
a test can render a frame with the inspector on and save it.
