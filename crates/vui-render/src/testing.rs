//! Helpers for tests that render headless.

use std::sync::OnceLock;

use vui_core::{Fonts, Scene};

use crate::context::Gpu;
use crate::image::Image;
use crate::renderer::Renderer;

/// One GPU device shared by every test in the process. Renderers are built per test because they
/// hold `Fonts`, which is single-threaded.
pub fn shared_gpu() -> &'static Gpu {
    static GPU: OnceLock<Gpu> = OnceLock::new();
    GPU.get_or_init(|| Gpu::headless().expect("a GPU is needed to run the rendering tests"))
}

/// Renders `scene` headless with a fresh renderer that shapes with `fonts`.
pub fn render_scene(scene: &Scene, fonts: &Fonts) -> Image {
    let gpu = shared_gpu();
    let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), wgpu::TextureFormat::Rgba8Unorm, fonts.clone());
    renderer.render_to_image(scene).expect("render")
}
