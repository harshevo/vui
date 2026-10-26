//! The wgpu renderer for vui.
//!
//! Give it a [`vui_core::Scene`] and a texture to draw into. `Renderer::headless()` needs no window,
//! which is how the tests and the gallery's screenshots work.

mod image_pipeline;
mod quad;
mod text;

pub mod context;
pub mod custom;
pub mod golden;
pub mod image;
pub mod renderer;
pub mod testing;

pub use context::{Gpu, GpuError};
pub use custom::CustomPass;
pub use image::Image;
pub use image_pipeline::ImageStats;
pub use renderer::Renderer;
/// The GPU API custom passes are written against, at the version vui uses.
pub use wgpu;
