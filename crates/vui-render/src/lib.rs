//! The renderers for vui.
//!
//! [`Renderer`] draws a [`vui_core::Scene`] on the GPU with wgpu; [`CpuRenderer`] draws the same
//! scene in memory where there is no GPU. `Renderer::headless()` needs no window, which is how the
//! tests and the gallery's screenshots work.

mod image_pipeline;
mod instanced;
mod layer;
mod mesh;
mod quad;
mod text;

pub mod context;
pub mod cpu;
pub mod custom;
pub mod effects;
pub mod golden;
pub mod image;
pub mod renderer;
pub mod shading;
pub mod testing;

pub use context::{Gpu, GpuError};
pub use cpu::{Canvas, CpuRenderer};
pub use custom::CustomPass;
pub use image::Image;
pub use image_pipeline::ImageStats;
pub use renderer::Renderer;
/// The GPU API custom passes are written against, at the version vui uses.
pub use wgpu;
