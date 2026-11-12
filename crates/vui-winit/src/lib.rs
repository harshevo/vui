//! vui's backend for Windows and Linux, on winit: runs a vui [`App`] in a native window.
//!
//! ```no_run
//! # use vui_runtime::{App, Cx, IntoElement, label};
//! # struct Hello;
//! # impl App for Hello {
//! #     type Msg = ();
//! #     fn update(&mut self, _: (), _: &mut Cx<()>) {}
//! #     fn view(&self) -> impl IntoElement<()> { label("Hello") }
//! # }
//! vui_winit::run(vui_winit::WindowOptions::new("Hello"), |_resources| Hello).unwrap();
//! ```
//!
//! It has the same surface as `vui-macos`: [`run`], an [`Application`] that opens several windows
//! and overlays, and a [`Proxy`] per window for other threads. Each window has:
//!
//! - a wgpu surface (DirectX 12 or Vulkan on Windows, Vulkan or GL on Linux), or, with
//!   `VUI_RENDERER=cpu` or where the GPU fails, frames drawn on the CPU and shown on whatever adapter
//!   can present them, the system's software one included;
//! - pointer, wheel, trackpad and pinch input, the keyboard and the input method;
//! - the clipboard ([`SystemClipboard`]) and cursors;
//! - frames timed by the display's vsync while something moves, and none while the UI is still.
//!
//! Ctrl is vui's shortcut modifier here, reported as `Modifiers::cmd`, so Ctrl-C copies as Cmd-C
//! does on a Mac. `VUI_SNAPSHOT` and `VUI_SNAPSHOT_QUIT` work as on macOS. App menus, the menu-bar
//! item, file dialogs and notifications are macOS-only for now: dialogs answer as cancelled.

use vui_core::Resources;
use vui_runtime::App;

pub use vui_shell::{Error, OverlayOptions, Place, Proxy, Screen, WindowOptions, host};

mod app;
mod clipboard;
pub mod input;
mod present;
mod runner;

pub use app::Application;
pub use clipboard::SystemClipboard;

/// Opens a window running the app that `make` builds, and runs until the app quits. `make` gets the
/// fonts and pictures the window will draw with, so it can register images.
pub fn run<A: App>(options: WindowOptions, make: impl FnOnce(&Resources) -> A) -> Result<(), Error> {
    let mut app = Application::new(&options.title)?;
    let state = make(app.resources());
    app.window(options, state)?;
    app.run();
    Ok(())
}
