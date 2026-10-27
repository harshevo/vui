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
//! frame written to a file, and `VUI_SNAPSHOT_QUIT=1` to quit after. With several windows, the second
//! writes `frame-1.png`, the third `frame-2.png`, and the app quits once all are written.
//!
//! An [`Application`] opens several windows, and overlays: borderless, see-through panels that float
//! over everything (a notch HUD). Each comes with a [`Proxy`] that sends its app messages from any
//! thread.

use std::fmt;
use std::sync::mpsc::Sender;

use vui_core::{Resources, Size, size};
use vui_runtime::App;

mod convert;
pub mod host;
mod keys;
#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
pub use mac::{Application, Pasteboard};

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

/// A borderless, see-through window that floats above everything, the menu bar included, on every
/// Space and over full-screen apps, and never takes focus. It sits where [`Place`] says on the screen
/// that [`Application::screen`] describes.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayOptions {
    /// Size in points. Whatever the app does not paint stays see-through.
    pub size: Size,
    /// Clicks pass through to whatever is underneath.
    pub click_through: bool,
    pub place: Place,
    /// Clicks pass through wherever the pointer is over nothing that takes it, and land on the overlay
    /// wherever it is: a panel that is mostly empty space.
    pub through_empty: bool,
}

/// Where an overlay sits.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Place {
    /// At the top centre, its top edge at the top of the screen: over the notch.
    Top,
    /// Against the right edge, its top edge just under the menu bar.
    Right,
}

impl OverlayOptions {
    pub fn new(size: Size) -> OverlayOptions {
        OverlayOptions { size, click_through: true, place: Place::Top, through_empty: false }
    }

    pub fn click_through(mut self, through: bool) -> OverlayOptions {
        self.click_through = through;
        self
    }

    pub fn place(mut self, place: Place) -> OverlayOptions {
        self.place = place;
        self
    }

    /// See [`OverlayOptions::through_empty`].
    pub fn through_empty(mut self) -> OverlayOptions {
        self.through_empty = true;
        self
    }
}

/// The screen overlays go on: the built-in display if it has a camera notch, else the one with the menu
/// bar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Screen {
    /// Size in points.
    pub size: Size,
    /// The camera housing's size in points, centred at the top, if the screen has one.
    pub notch: Option<Size>,
    /// Height of the menu bar in points (zero while it hides itself).
    pub menu_bar: f32,
}

/// Sends a window's app messages from anywhere: other threads, audio callbacks, hotkey handlers. Each
/// arrives on the main thread and goes through the app's `update`, as if a widget had sent it.
pub struct Proxy<M> {
    tx: Sender<M>,
    window: usize,
}

impl<M> Clone for Proxy<M> {
    fn clone(&self) -> Proxy<M> {
        Proxy { tx: self.tx.clone(), window: self.window }
    }
}

impl<M: Send + 'static> Proxy<M> {
    /// Queues `msg` for the app. False once the window is gone.
    pub fn send(&self, msg: M) -> bool {
        if self.tx.send(msg).is_err() {
            return false;
        }
        #[cfg(target_os = "macos")]
        mac::wake(self.window);
        #[cfg(not(target_os = "macos"))]
        let _ = self.window;
        true
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
