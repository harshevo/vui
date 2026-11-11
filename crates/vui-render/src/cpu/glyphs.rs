//! Text: the same glyph images glyphon rasterises (swash, through cosmic-text), placed the way
//! glyphon places them and blended the way its pipeline does.

use std::collections::HashMap;

use glyphon::{CacheKey, SwashCache, SwashContent, SwashImage};
use vui_core::scene::TextCmd;
use vui_core::Fonts;

use super::canvas::Canvas;

/// Glyph images by cache key. `None` remembers a glyph with nothing to draw (a space).
pub(crate) struct Glyphs {
    swash: SwashCache,
    images: HashMap<CacheKey, Option<SwashImage>>,
}

/// More than this many glyph images and the cache starts again.
const MAX_GLYPHS: usize = 8192;

impl Default for Glyphs {
    fn default() -> Glyphs {
        Glyphs { swash: SwashCache::new(), images: HashMap::new() }
    }
}

impl Glyphs {
    pub(crate) fn draw(&mut self, canvas: &mut Canvas, fonts: &Fonts, t: &TextCmd, scale: f32) {
        let (w, h) = (canvas.width as i32, canvas.height as i32);
        // Whole-pixel bounds, as `renderer::text_area` gives glyphon.
        let (left, top, right, bottom) = match t.clip {
            Some(c) => (
                ((c.x * scale).floor() as i32).max(0),
                ((c.y * scale).floor() as i32).max(0),
                (((c.x + c.w) * scale).ceil() as i32).min(w),
                (((c.y + c.h) * scale).ceil() as i32).min(h),
            ),
            None => (0, 0, w, h),
        };
        if right <= left || bottom <= top {
            return;
        }
        if self.images.len() > MAX_GLYPHS {
            self.images.clear();
        }
        let [r, g, b, a] = t.color.to_rgba8();
        let default = glyphon::Color::rgba(r, g, b, a);
        let origin = (t.origin.x * scale, t.origin.y * scale);
        let mut system = fonts.system();
        for run in t.layout.buffer().layout_runs() {
            let run_top = origin.1 + run.line_top * scale;
            if run_top > bottom as f32 || run_top + run.line_height * scale < top as f32 {
                continue;
            }
            for glyph in run.glyphs {
                let physical = glyph.physical(origin, scale);
                let image = self
                    .images
                    .entry(physical.cache_key)
                    .or_insert_with(|| self.swash.get_image_uncached(&mut system, physical.cache_key).filter(|i| i.placement.width > 0 && i.placement.height > 0));
                let Some(image) = image else { continue };
                let color = glyph.color_opt.unwrap_or(default);
                let rgb = [color.r(), color.g(), color.b()].map(|c| c as f32 / 255.0);
                let alpha = color.a() as f32 / 255.0;
                let x0 = physical.x + image.placement.left;
                let y0 = (run.line_y * scale).round() as i32 + physical.y - image.placement.top;
                let (gw, gh) = (image.placement.width as i32, image.placement.height as i32);
                for gy in (top - y0).max(0)..gh.min(bottom - y0) {
                    for gx in (left - x0).max(0)..gw.min(right - x0) {
                        let i = (gy * gw + gx) as usize;
                        let (x, y) = (x0 + gx, y0 + gy);
                        match image.content {
                            SwashContent::Mask => canvas.blend_text(x, y, rgb, alpha * image.data[i] as f32 / 255.0),
                            SwashContent::SubpixelMask => {
                                let p = &image.data[i * 4..i * 4 + 3];
                                let m = (p[0] as f32 + p[1] as f32 + p[2] as f32) / (3.0 * 255.0);
                                canvas.blend_text(x, y, rgb, alpha * m);
                            }
                            SwashContent::Color => {
                                let p = &image.data[i * 4..i * 4 + 4];
                                let a = p[3] as f32 / 255.0;
                                canvas.blend_text(x, y, [p[0], p[1], p[2]].map(|c| c as f32 / 255.0), a);
                            }
                        }
                    }
                }
            }
        }
    }
}
