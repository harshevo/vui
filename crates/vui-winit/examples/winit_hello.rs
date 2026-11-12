//! A window on the winit backend, on any desktop, the Mac included, for trying the backend where it
//! is not the default:
//!
//! ```sh
//! cargo run -p vui-winit --example winit_hello
//! VUI_RENDERER=cpu VUI_SNAPSHOT=frame.png VUI_SNAPSHOT_QUIT=1 cargo run -p vui-winit --example winit_hello
//! ```

use vui_core::size;
use vui_runtime::{App, Cx, IntoElement, Parent, Styled, column, label, title};

#[derive(Clone, Debug)]
enum Msg {}

struct Hello;

impl App for Hello {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {}
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(24.0).gap(8.0).children((title("Hello from winit"), label("Type, scroll and resize: the same UI as on the Mac.")))
    }
}

fn main() {
    let options = vui_winit::WindowOptions::new("winit").size(size(420.0, 200.0)).min_size(size(240.0, 120.0));
    if let Err(e) = vui_winit::run(options, |_| Hello) {
        eprintln!("{e}");
    }
}
