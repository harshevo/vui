//! Helpers for tests that render headless.
//!
//! Frames come from the GPU when there is one. With `VUI_RENDERER=cpu`, or on a machine with no GPU
//! adapter, they come from the [`CpuRenderer`] instead, so the same tests run anywhere.

use std::sync::OnceLock;

use vui_core::{Fonts, Resources, Scene};

use crate::context::Gpu;
use crate::cpu::CpuRenderer;
use crate::image::Image;
use crate::renderer::Renderer;

/// One GPU device shared by every test in the process, or `None` where there is no GPU. Renderers
/// are built per test because they hold `Fonts`, which is single-threaded.
pub fn try_shared_gpu() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| Gpu::headless().ok()).as_ref()
}

/// The shared GPU, for tests that need one.
pub fn shared_gpu() -> &'static Gpu {
    try_shared_gpu().expect("a GPU is needed to run the rendering tests")
}

/// Whether headless frames are drawn on the CPU: asked for with `VUI_RENDERER=cpu`, or forced by
/// having no GPU.
pub fn uses_cpu() -> bool {
    std::env::var("VUI_RENDERER").is_ok_and(|v| v.eq_ignore_ascii_case("cpu")) || try_shared_gpu().is_none()
}

/// Renders `scene` headless with a fresh renderer that shapes with `fonts`.
pub fn render_scene(scene: &Scene, fonts: &Fonts) -> Image {
    render_with(scene, &Resources { fonts: fonts.clone(), images: Default::default() })
}

/// Renders `scene` headless with the given fonts and images, on the GPU unless [`uses_cpu`].
pub fn render_with(scene: &Scene, resources: &Resources) -> Image {
    if uses_cpu() {
        return render_cpu(scene, resources);
    }
    renderer_for(resources).render_to_image(scene).expect("render")
}

/// Renders `scene` on the CPU, whatever the machine has.
pub fn render_cpu(scene: &Scene, resources: &Resources) -> Image {
    CpuRenderer::new(resources.clone()).render_to_image(scene)
}

/// A fresh renderer on the shared GPU, for tests that render several frames.
pub fn renderer_for(resources: &Resources) -> Renderer {
    let gpu = shared_gpu();
    Renderer::new(gpu.device.clone(), gpu.queue.clone(), wgpu::TextureFormat::Rgba8Unorm, resources.clone())
}
