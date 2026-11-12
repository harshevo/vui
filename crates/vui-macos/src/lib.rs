//! vui's macOS backend: runs a vui [`App`] in an AppKit window.
//!
//! ```no_run
//! # use vui_runtime::{App, Cx, IntoElement, label};
//! # struct Hello;
//! # impl App for Hello {
//! #     type Msg = ();
//! #     fn update(&mut self, _: (), _: &mut Cx<()>) {}
//! #     fn view(&self) -> impl IntoElement<()> { label("Hello") }
//! # }
//! vui_macos::run(vui_macos::WindowOptions::new("Hello"), |_resources| Hello).unwrap();
//! ```
//!
//! The window has a transparent title bar over a full-size content view, so the app draws all of it;
//! mark the empty canvas with `Builder::window_drag` to let people move the window by it, and leave
//! room at the top left for the window buttons. Set `VUI_SNAPSHOT=frame.png` to have the first settled
//! frame written to a file, and `VUI_SNAPSHOT_QUIT=1` to quit after. With several windows, the second
//! writes `frame-1.png`, the third `frame-2.png`, and the app quits once all are written.
//!
//! An [`Application`] opens several windows, and overlays: borderless, see-through panels that float
//! over everything (a notch HUD). Each comes with a [`Proxy`] that sends its app messages from any
//! thread.

use vui_core::Resources;
use vui_runtime::App;

pub use vui_shell::{Error, OverlayOptions, Place, Proxy, Screen, WindowOptions, host};

#[cfg(any(target_os = "macos", test))]
mod convert;
#[cfg(any(target_os = "macos", test))]
mod keys;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
mod software;

#[cfg(target_os = "macos")]
pub use mac::{Application, Pasteboard};

/// Opens a window running the app that `make` builds, and runs until the app quits. `make` gets the
/// fonts and pictures the window will draw with, so it can register images.
pub fn run<A: App>(options: WindowOptions, make: impl FnOnce(&Resources) -> A) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let mut app = Application::new(&options.title)?;
        let state = make(app.resources());
        app.window(options, state)?;
        app.run();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (options, make);
        Err(Error::Unsupported)
    }
}
