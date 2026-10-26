//! The wgpu renderer for vui.
//!
//! Give it a [`vui_core::Scene`] and a texture to draw into. `Renderer::headless()` needs no window,
//! which is how the tests and the gallery's screenshots work.

mod image_pipeline;
mod quad;
mod text;

pub mod context;
pub mod golden;
pub mod image;
pub mod renderer;
pub mod testing;

pub use context::{Gpu, GpuError};
pub use image::Image;
pub use image_pipeline::ImageStats;
pub use renderer::Renderer;
