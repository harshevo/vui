//! Helpers for tests that render headless.

use std::sync::OnceLock;

use vui_core::{Fonts, Resources, Scene};

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
    render_with(scene, &Resources { fonts: fonts.clone(), images: Default::default() })
}

/// Renders `scene` headless with the given fonts and images.
pub fn render_with(scene: &Scene, resources: &Resources) -> Image {
    renderer_for(resources).render_to_image(scene).expect("render")
}

/// A fresh renderer on the shared GPU, for tests that render several frames.
pub fn renderer_for(resources: &Resources) -> Renderer {
    let gpu = shared_gpu();
    Renderer::new(gpu.device.clone(), gpu.queue.clone(), wgpu::TextureFormat::Rgba8Unorm, resources.clone())
}
