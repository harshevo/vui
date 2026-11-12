# Getting started

This guide takes you from an empty crate to a window with a working counter, then shows where to go
next. vui opens windows on macOS, Windows and Linux; everything else, tests included, runs anywhere.

## Add vui

vui is not on crates.io yet. Depend on it by path or git:

```toml
[dependencies]
vui = { git = "https://github.com/harshevo/vui" }   # or: { path = "../vui/crates/vui" }
```

The `vui` crate is the front door. It re-exports every part of the framework, and
`use vui::prelude::*` brings in what most views need.

## Your first app

An app has three parts:

- **State**: a plain struct.
- **Messages**: what can happen. Usually an enum.
- **A view**: a function from the state to a tree of elements.

When something happens (a click, a key, a timer, a background task finishing), vui sends your
`update` a message. You change the state, and vui calls `view` again. Then it works out what
changed on screen, and only redraws that.

```rust,no_run
use vui::prelude::*;

#[derive(Clone)]
enum Msg {
    Add(i32),
}

#[derive(Default)]
struct Counter {
    count: i32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Add(n) => self.count += n,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().fill().center().gap(16.0).children((
            label(self.count.to_string()).font_size(48.0).weight(600),
            row().gap(8.0).children((
                button("−").on_click(Msg::Add(-1)),
                button("+").primary().on_click(Msg::Add(1)),
            )),
        ))
    }
}

fn main() {
    vui::run(WindowOptions::new("Counter").size(size(360.0, 240.0)), |_resources| Counter::default()).unwrap();
}
```

`vui::run` opens the window, runs the app and returns when the app quits. The closure gets the
window's `Resources` (fonts and images), so you can register pictures before the first frame.

## Run the examples

The repository has three example apps. Each is also driven headless in `crates/vui/tests/examples.rs`.

```sh
cargo run -p vui --example hello       # the counter
cargo run -p vui --example todo        # text input, a list built from data, filters
cargo run -p vui --example dashboard   # sidebar, stat cards, a chart and a table; resize it
cargo run -p vui --example notes       # app menus and shortcuts, open and save sheets, a menu-bar item
```

The gallery shows every widget in light and dark:

```sh
cargo run -p gallery --release -- --window
```

## Platforms

`vui::run`, `vui::Application` and `vui::Proxy` are the same everywhere; `vui` picks the backend
for the target it builds for.

| Target | Backend | Notes |
| --- | --- | --- |
| macOS | `vui-macos` (AppKit, Metal) | menus, sheets, notifications, the menu-bar item, overlays over the notch |
| Windows | `vui-winit` (DirectX 12 or Vulkan) | the clipboard through Win32 |
| Linux and the BSDs | `vui-winit` (Vulkan or GL; X11 and Wayland) | the clipboard through `wl-copy`/`wl-paste`, `xclip` or `xsel` |

On Windows and Linux, Ctrl is the shortcut modifier: it arrives as `Modifiers::cmd`, so the same
shortcuts work as Cmd ones do on a Mac. File dialogs answer as cancelled there, and app menus and
notifications are macOS-only for now. Several windows and an overlay, on any of them:

```rust,no_run
use vui::prelude::*;

#[derive(Clone)]
enum Msg {
    Tick,
}

#[derive(Default)]
struct Clock {
    ticks: u32,
}

impl App for Clock {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Tick => self.ticks += 1,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        label(format!("{} ticks", self.ticks))
    }
}

fn main() -> Result<(), vui::Error> {
    let mut app = vui::Application::new("Clock")?;
    let main = app.window(WindowOptions::new("Clock").size(size(320.0, 200.0)), Clock::default())?;
    app.overlay(vui::OverlayOptions::new(size(200.0, 40.0)).place(vui::Place::Top), Clock::default())?;
    std::thread::spawn(move || while main.send(Msg::Tick) {
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
    app.run();
    Ok(())
}
```

To try the winit backend on a Mac, where it is not the default:

```sh
cargo run -p vui-winit --example winit_hello
```

## What to read next

| Guide | Covers |
| --- | --- |
| [The app model](app-model.md) | messages, `Cx` (focus, timers, background work), identity and keys, several windows, other threads |
| [Layout](layout.md) | rows and columns, sizes, alignment, scrolling, and responsive layouts |
| [Styling and theming](styling.md) | fills, borders, shadows, motion, the theme's tokens, dark mode and your own theme |
| [Widgets](widgets.md) | the catalogue: what exists and how each is used |
| [Text, icons and images](text-icons-images.md) | type roles, custom fonts, the icon set, pictures |
| [Charts and graphs](charts.md) | `vui::viz`: line, area, bar and candle charts, sparklines, graphs |
| [Custom widgets](custom-widgets.md) | the `Widget` trait, painting, input, accessibility, custom GPU passes |
| [Rendering](rendering.md) | the GPU renderer, the CPU fallback, and snapshots |
| [Testing](testing.md) | `TestUi`, golden images, and running with no window or GPU |
| [Accessibility](accessibility.md) | roles, names and keyboard focus |
| [Roadmap](roadmap.md) | what is missing and what comes next |
