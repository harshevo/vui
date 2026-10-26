//! Text drawing: glyphon's glyph atlas and pipeline, one renderer per run of text so text and
//! shapes can interleave in painter's order.

use glyphon::{Cache, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextRenderer, Viewport};
use vui_core::Fonts;

pub(crate) struct TextPipeline {
    _cache: Cache,
    atlas: TextAtlas,
    viewport: Viewport,
    swash: SwashCache,
    renderers: Vec<TextRenderer>,
}

impl TextPipeline {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> TextPipeline {
        let cache = Cache::new(device);
        // Web mode blends in encoded sRGB space like the rest of the renderer.
        let atlas = TextAtlas::with_color_mode(device, queue, &cache, format, ColorMode::Web);
        let viewport = Viewport::new(device, &cache);
        TextPipeline { _cache: cache, atlas, viewport, swash: SwashCache::new(), renderers: Vec::new() }
    }

    pub(crate) fn begin_frame(&mut self, queue: &wgpu::Queue, size_px: (u32, u32)) {
        self.viewport.update(queue, Resolution { width: size_px.0, height: size_px.1 });
    }

    /// Uploads the glyphs for text run number `index`.
    pub(crate) fn prepare(
        &mut self,
        index: usize,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        fonts: &Fonts,
        areas: Vec<TextArea<'_>>,
    ) {
        while self.renderers.len() <= index {
            self.renderers.push(TextRenderer::new(&mut self.atlas, device, wgpu::MultisampleState::default(), None));
        }
        let mut system = fonts.system();
        if let Err(e) = self.renderers[index].prepare(device, queue, &mut system, &mut self.atlas, &self.viewport, areas, &mut self.swash) {
            eprintln!("vui: could not prepare text: {e}");
        }
    }

    pub(crate) fn draw(&self, index: usize, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(r) = self.renderers.get(index)
            && let Err(e) = r.render(&self.atlas, &self.viewport, pass)
        {
            eprintln!("vui: could not draw text: {e}");
        }
    }

    pub(crate) fn end_frame(&mut self) {
        self.atlas.trim();
    }
}
