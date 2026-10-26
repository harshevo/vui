//! vui's macOS backend: runs a vui [`App`] in an AppKit window.
//!
//! ```no_run
//! # use vui_runtime::{App, Cx, IntoElement, label};
//! # struct Hello;
//! # impl App for Hello {
//! #     type Msg = ();
//! #     fn update(&mut self, _: (), _: &mut Cx) {}
//! #     fn view(&self) -> impl IntoElement<()> { label("Hello") }
//! # }
//! vui_macos::run(vui_macos::WindowOptions::new("Hello"), |_resources| Hello).unwrap();
//! ```
//!
//! The window has a transparent title bar over a full-size content view, so the app draws all of it;
//! mark the empty canvas with `Builder::window_drag` to let people move the window by it, and leave
//! room at the top left for the window buttons. Set `VUI_SNAPSHOT=frame.png` to have the first settled
//! frame written to a file, and `VUI_SNAPSHOT_QUIT=1` to quit after.

use std::fmt;

use vui_core::{Resources, Size, size};
use vui_runtime::App;

mod convert;
pub mod host;
mod keys;
#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
pub use mac::Pasteboard;

/// The window an app opens in.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowOptions {
    pub title: String,
    /// Content size in points.
    pub size: Size,
    pub min_size: Size,
    /// Quit when the window closes. Off for apps that live on in the menu bar.
    pub quit_on_close: bool,
}

impl WindowOptions {
    pub fn new(title: impl Into<String>) -> WindowOptions {
        WindowOptions { title: title.into(), size: size(960.0, 640.0), min_size: size(640.0, 420.0), quit_on_close: true }
    }

    pub fn size(mut self, size: Size) -> WindowOptions {
        self.size = size;
        self
    }

    pub fn min_size(mut self, size: Size) -> WindowOptions {
        self.min_size = size;
        self
    }

    pub fn quit_on_close(mut self, quit: bool) -> WindowOptions {
        self.quit_on_close = quit;
        self
    }
}

#[derive(Debug)]
pub enum Error {
    /// `run` must be called on the main thread.
    NotMainThread,
    Surface(String),
    Gpu(String),
    /// Not built for macOS.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotMainThread => write!(f, "the window must be opened on the main thread"),
            Error::Surface(e) => write!(f, "could not create the window's surface: {e}"),
            Error::Gpu(e) => write!(f, "{e}"),
            Error::Unsupported => write!(f, "vui-macos only runs on macOS"),
        }
    }
}

impl std::error::Error for Error {}

/// Opens a window running the app that `make` builds, and runs until the app quits. `make` gets the
/// fonts and pictures the window will draw with, so it can register images.
pub fn run<A: App>(options: WindowOptions, make: impl FnOnce(&Resources) -> A) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        mac::run(options, make)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (options, make);
        Err(Error::Unsupported)
    }
}
