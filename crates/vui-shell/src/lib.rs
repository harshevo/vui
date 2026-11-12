//! What every vui window backend shares, so `vui-macos` and `vui-winit` open windows the same way:
//! [`WindowOptions`] and [`OverlayOptions`], the thread-safe [`Proxy`], [`Error`], and the
//! [`host`] that ties a UI to where its frames go.
//!
//! Apps do not use this crate directly: `vui` re-exports the backend for the platform it builds for.

mod error;
pub mod frames;
pub mod host;
mod options;
mod proxy;

pub use error::Error;
pub use options::{OverlayOptions, Place, Screen, WindowOptions};
pub use proxy::{Proxy, Wake};
