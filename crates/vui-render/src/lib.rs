//! The wgpu renderer for vui.
//!
//! Give it a [`vui_core::Scene`] and a texture to draw into. `Renderer::headless()` needs no window,
//! which is how the tests and the gallery's screenshots work.

mod quad;
mod text;

pub mod context;
pub mod golden;
pub mod image;
pub mod renderer;
pub mod testing;

pub use context::{Gpu, GpuError};
pub use image::Image;
pub use renderer::Renderer;
