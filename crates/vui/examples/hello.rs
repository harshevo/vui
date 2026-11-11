//! The smallest vui app worth running: a counter.
//!
//! ```sh
//! cargo run -p vui --example hello
//! ```
//!
//! An app is state, the messages that change it, and a view of it. `update` handles one message;
//! `view` builds the tree for the state as it is now, and vui works out what changed on screen.

use vui::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Add(i32),
    Reset,
}

#[derive(Default)]
pub struct Counter {
    pub count: i32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Add(n) => self.count += n,
            Msg::Reset => self.count = 0,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().fill().center().gap(20.0).window_drag(true).children((
            small("You clicked").color(Tone::Text2),
            label(self.count.to_string()).font_size(56.0).weight(600).key("count"),
            row().gap(8.0).children((
                icon_button(icon_set::MINUS, "Less").key("less").on_click(Msg::Add(-1)),
                button("Add one").primary().icon(icon_set::PLUS).key("more").on_click(Msg::Add(1)),
                button("Reset").ghost().key("reset").disabled(self.count == 0).on_click(Msg::Reset),
            )),
        ))
    }
}

#[cfg(target_os = "macos")]
fn main() {
    let window = WindowOptions::new("Hello").size(size(420.0, 320.0)).min_size(size(320.0, 240.0));
    if let Err(e) = vui::run(window, |_resources| Counter::default()) {
        eprintln!("{e}");
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("vui has a window backend for macOS only so far; the app still runs headless in tests.");
}
