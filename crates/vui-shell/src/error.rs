//! Why a window could not open.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// `run` must be called on the main thread.
    NotMainThread,
    Surface(String),
    Gpu(String),
    /// The window system could not be reached: no display, or the event loop failed.
    Platform(String),
    /// No window backend for this platform.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotMainThread => write!(f, "the window must be opened on the main thread"),
            Error::Surface(e) => write!(f, "could not create the window's surface: {e}"),
            Error::Gpu(e) => write!(f, "{e}"),
            Error::Platform(e) => write!(f, "the window system failed: {e}"),
            Error::Unsupported => write!(f, "vui has no window backend for this platform"),
        }
    }
}

impl std::error::Error for Error {}
