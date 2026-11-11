# The app model

A vui app is a state machine. `view` describes the screen for the current state, and `update` is
the only place state changes. This guide covers how messages flow, what `Cx` lets `update` do,
how elements keep their identity, and how to run several windows or talk to the UI from other
threads.

## The loop

```text
 input / timer / task ──► update(msg, cx) ──► view() ──► reconcile ──► layout ──► paint
         ▲                                                                       │
         └───────────────────────── the next event ◄─────────────────────────────┘
```

- **`update(&mut self, msg, cx)`** handles one message. Change your state; use `cx` for anything
  beyond that.
- **`view(&self)`** returns the element tree for the state as it is now. It is cheap to call and
  should not do work: no file reads, no network calls.
- vui **reconciles** the new tree against the old one. Widgets keep their state, such as a text
  field's caret, a scroll offset or an animation in flight, and only what changed is laid out and
  repainted.
- When nothing moves, vui draws no frames, so an idle window costs nothing.

Two optional methods are worth knowing:

- **`theme(&self, system: Mode) -> Theme`** picks the look. The default follows the system's light or
  dark mode; see [Styling and theming](styling.md).
- **`resized(&self, size) -> Option<Msg>`** tells you the window's size. Most apps never need it,
  because [responsive styles](layout.md#responsive-layouts) handle sizes inside the view.

## Messages

A message is any `'static` type. An enum is usual. Widgets take either the message itself (for
`on_click`, which needs `Clone` because it can fire many times) or a function that builds one
from a value (for `on_change`):

```rust
use vui::prelude::*;

#[derive(Clone)]
enum Msg {
    Save,
    Rename(String),
    Volume(f64),
}

fn controls(name: &str, volume: f64) -> Div<Msg> {
    column().gap(12.0).children((
        text_field(name).on_change(Msg::Rename),
        slider(volume, 0.0, 100.0).on_change(Msg::Volume),
        button("Save").primary().on_click(Msg::Save),
    ))
}
```

Helper functions like `controls` keep views readable. They return a builder such as `Div<Msg>`, or
`impl IntoElement<Msg>`.

## What `Cx` can do

| Call | Does |
| --- | --- |
| `cx.focus("key")` | moves keyboard focus to the keyed element once the new view is in place |
| `cx.send(msg)` | handles another message right after this one |
| `cx.after(duration, msg)` | a timer: a toast that hides itself, a debounced search, a clock tick |
| `cx.spawn(\|\| work())` | runs blocking work on another thread; its return value comes back as a message |
| `cx.sender()` | an `AppSender` that sends messages from any thread for as long as the window lives |
| `cx.quit()` | closes the app |
| `cx.open_files(dialog, Msg::Opened)` | shows an open sheet; the paths picked come back as a message (none if cancelled) |
| `cx.save_file(dialog, Msg::Saved)` | shows a save sheet; the path comes back as `Option<PathBuf>` |
| `cx.notify(Notification::new("Done"))` | shows a system notification |
| `cx.full_screen(true)`, `cx.toggle_full_screen()` | takes the window full screen and back |

Background work, the vui way:

```rust
use std::time::Duration;
use vui::prelude::*;

#[derive(Clone)]
enum Msg {
    Load,
    Loaded(Result<String, String>),
    Tick,
}

struct Reader {
    text: Option<String>,
    loading: bool,
    seconds: u32,
}

impl App for Reader {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Load => {
                self.loading = true;
                cx.spawn(|| Msg::Loaded(std::fs::read_to_string("notes.txt").map_err(|e| e.to_string())));
            }
            Msg::Loaded(result) => {
                self.loading = false;
                self.text = Some(result.unwrap_or_else(|e| format!("Could not read: {e}")));
            }
            Msg::Tick => {
                self.seconds += 1;
                cx.after(Duration::from_secs(1), Msg::Tick);
            }
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(24.0).gap(12.0).children((
            button("Open notes").loading(self.loading).on_click(Msg::Load),
            label(self.text.clone().unwrap_or_default()),
            small(format!("Open for {}s", self.seconds)).color(Tone::Text2),
        ))
    }
}
```

The UI stays responsive while the thread runs. Show progress from state you set before spawning
(`loading` above).

## Menus, shortcuts and the menu bar

An app describes its menus the way it describes its view: from state, asked again after every
update. Items send plain messages, so a shortcut and a button can share one.

```rust
use std::path::PathBuf;
use vui::prelude::*;

#[derive(Clone)]
enum Msg {
    Open,
    Opened(Vec<PathBuf>),
    ToggleGrid,
    Ping,
}

#[derive(Default)]
struct Editor {
    files: Vec<PathBuf>,
    grid: bool,
}

impl App for Editor {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Open => cx.open_files(FileDialog::new().extensions(["md"]).multiple(), Msg::Opened),
            Msg::Opened(files) => self.files.extend(files),
            Msg::ToggleGrid => self.grid = !self.grid,
            Msg::Ping => cx.notify(Notification::new("Editor").body("Still here")),
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        label(format!("{} files", self.files.len()))
    }

    fn menus(&self) -> Vec<Menu<Msg>> {
        vec![
            Menu::new("File", [MenuItem::new("Open…", Msg::Open).shortcut(Shortcut::cmd('o'))]),
            Menu::new("View", [MenuItem::new("Grid", Msg::ToggleGrid).checked(self.grid)]),
        ]
    }

    fn status_item(&self) -> Option<StatusItem<Msg>> {
        Some(StatusItem::new("Ed", [MenuItem::new("Ping", Msg::Ping)]))
    }
}
```

On macOS, File goes after the app menu and the others between Edit and Window. Disabled and ticked
items follow state. Each window's menus come back when the window comes to the front.

Notifications use the system's older notification centre, which needs no extra crate. Unbundled
binaries may not show them on recent macOS.

In tests, play the system's part: `t.choose("File", "Open…")` picks a menu item. The dialog goes
out as `Effect::Open { id, .. }`, and `t.ui.answer(id, paths)` answers it. The
[`notes` example](../crates/vui/examples/notes.rs) shows all of this in a window.

## Identity: keys

When the view is rebuilt, vui matches new elements to old ones by position. Give an element a
**key** when position isn't enough:

- **Items in a list that can reorder, insert or delete.** Key each row by its id, so a row's
  checkbox animation or text field state follows the row, not the slot.
- **Elements you refer to by name.** `cx.focus("search")`, tests (`t.click_key("save")`) and
  `Ui::rect_of` all find an element by its key.
- **Content that should appear afresh.** With `.appear()`, keying a page by its name makes the
  new page fade in instead of morphing the old one.

```rust
use vui::prelude::*;

struct Task { id: u64, text: String }

fn rows(tasks: &[Task]) -> Div<()> {
    list().children(tasks.iter().map(|t| list_row(t.text.clone()).key(t.id)).collect::<Vec<_>>())
}
```

Keys can be strings or integers.

## Several windows, overlays and other threads

`vui::run` is the one-window shortcut. `Application` opens any number of windows and
**overlays**. An overlay is a borderless, see-through panel that floats above everything, never
takes focus unless asked, and suits HUDs and notch UIs. Every window or overlay comes with a
`Proxy`, which sends that app messages from any thread: an audio callback, a hotkey handler, a
socket reader.

```rust,no_run
use vui::prelude::*;
use vui::{Application, OverlayOptions, Place};

struct Main;
impl App for Main {
    type Msg = String;
    fn update(&mut self, _msg: String, _cx: &mut Cx<String>) {}
    fn view(&self) -> impl IntoElement<String> { label("Main window") }
}

struct Hud { status: String }
impl App for Hud {
    type Msg = String;
    fn update(&mut self, msg: String, _cx: &mut Cx<String>) { self.status = msg; }
    fn view(&self) -> impl IntoElement<String> {
        row().padding(12.0).rounded(Radius::Pill).bg(Tone::Bg).shadow(Shadow::RAISED).child(label(self.status.clone()))
    }
}

fn main() -> Result<(), vui::Error> {
    let mut app = Application::new("Demo")?;
    app.window(WindowOptions::new("Demo").size(size(800.0, 600.0)), Main)?;
    let hud = app.overlay(OverlayOptions::new(size(320.0, 56.0)).place(Place::Top).click_through(true), Hud { status: "Ready".into() })?;

    std::thread::spawn(move || {
        hud.send("Listening…".to_string());
    });
    app.run();
    Ok(())
}
```

## Clipboard, cursors, drag and drop

- **Clipboard.** Text fields copy, cut and paste on their own. A custom widget reads and writes
  the clipboard through its event context (`cx.clipboard_text()`, `cx.set_clipboard_text(…)`).
- **Cursors.** `.cursor(CursorIcon::Pointer)` sets the cursor over any element. Buttons and links
  already do.
- **Files.** `.on_drop(|paths| Msg::Open(paths))` takes files dropped on an element, and
  `.drop_bg(...)` highlights it while files hover.
- **Context menus.** `.on_secondary(|click| Msg::Menu(click.pos))` opens one on right-click; see
  `context_menu` in [Widgets](widgets.md).
