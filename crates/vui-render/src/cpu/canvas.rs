//! The frame the CPU renderer draws into: premultiplied RGBA8 in encoded sRGB, the same thing the
//! GPU renderer's target holds, blended the same way.

use crate::image::Image;

/// Premultiplied RGBA8 pixels, row-major, top row first.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    data: Vec<u8>,
}

/// A run of whole pixels, `x0..x1` by `y0..y1`, already inside the canvas.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Span {
    pub(crate) fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    pub(crate) fn intersect(self, o: Span) -> Span {
        Span { x0: self.x0.max(o.x0), y0: self.y0.max(o.y0), x1: self.x1.min(o.x1), y1: self.y1.min(o.y1) }
    }

    /// The pixels whose centres fall inside `[x0, x1) × [y0, y1)` in device pixels.
    pub(crate) fn centres_in(x0: f32, y0: f32, x1: f32, y1: f32) -> Span {
        let lo = |v: f32| (v - 0.5).ceil().clamp(i32::MIN as f32, i32::MAX as f32) as i32;
        Span { x0: lo(x0), y0: lo(y0), x1: lo(x1), y1: lo(y1) }
    }
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Canvas {
        Canvas { width, height, data: vec![0; width as usize * height as usize * 4] }
    }

    /// Changes the size, clearing to transparent when it changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != (self.width, self.height) {
            *self = Canvas::new(width, height);
        }
    }

    /// Fills every pixel with a premultiplied colour.
    pub fn clear(&mut self, premultiplied: [f32; 4]) {
        let px = quantise(premultiplied);
        for p in self.data.as_chunks_mut::<4>().0 {
            *p = px;
        }
    }

    /// The premultiplied RGBA8 pixels.
    pub fn pixels(&self) -> &[u8] {
        &self.data
    }

    pub(crate) fn bounds(&self) -> Span {
        Span { x0: 0, y0: 0, x1: self.width as i32, y1: self.height as i32 }
    }

    /// Source-over of a premultiplied colour onto pixel `(x, y)`, which must be inside.
    #[inline]
    pub(crate) fn blend(&mut self, x: i32, y: i32, src: [f32; 4]) {
        if src[3] <= 0.0 && src[0] <= 0.0 && src[1] <= 0.0 && src[2] <= 0.0 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = &mut self.data[i..i + 4];
        if src[3] >= 1.0 {
            p.copy_from_slice(&quantise(src));
            return;
        }
        let k = 1.0 - src[3];
        for c in 0..4 {
            p[c] = to_u8(src[c] + p[c] as f32 / 255.0 * k);
        }
    }

    /// Glyphon's blend: straight-alpha colour over, with alpha composited as `a + d·(1 − a)`, which
    /// is premultiplied source-over by another name.
    #[inline]
    pub(crate) fn blend_text(&mut self, x: i32, y: i32, rgb: [f32; 3], a: f32) {
        if a > 0.0 {
            self.blend(x, y, [rgb[0] * a, rgb[1] * a, rgb[2] * a, a]);
        }
    }

    /// Fills a run of pixels with an opaque-or-not premultiplied colour.
    pub(crate) fn fill_span(&mut self, y: i32, x0: i32, x1: i32, src: [f32; 4]) {
        if src[3] >= 1.0 {
            let px = quantise(src);
            let row = y as usize * self.width as usize;
            for p in self.data[(row + x0 as usize) * 4..(row + x1 as usize) * 4].as_chunks_mut::<4>().0 {
                *p = px;
            }
        } else {
            for x in x0..x1 {
                self.blend(x, y, src);
            }
        }
    }

    /// The frame as a straight-alpha image, the same as the GPU renderer's read-back.
    pub fn to_image(&self) -> Image {
        let mut rgba = Vec::with_capacity(self.data.len());
        for &[r, g, b, a] in self.data.as_chunks::<4>().0 {
            if a == 255 || a == 0 {
                rgba.extend_from_slice(&[r, g, b, a]);
            } else {
                let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
                rgba.extend_from_slice(&[un(r), un(g), un(b), a]);
            }
        }
        Image { width: self.width, height: self.height, rgba }
    }

    /// Writes the frame as premultiplied BGRA8, the layout a window's backing store wants.
    pub fn write_bgra(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.data.len());
        for &[r, g, b, a] in self.data.as_chunks::<4>().0 {
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
}

#[inline]
fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[inline]
fn quantise(c: [f32; 4]) -> [u8; 4] {
    [to_u8(c[0]), to_u8(c[1]), to_u8(c[2]), to_u8(c[3])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_over_matches_the_gpu_blend() {
        let mut c = Canvas::new(1, 1);
        c.clear([1.0, 1.0, 1.0, 1.0]);
        c.blend(0, 0, [0.0, 0.0, 0.5, 0.5]);
        assert_eq!(c.pixels(), &[128, 128, 255, 255]);
    }

    #[test]
    fn centres_pick_the_pixels_a_triangle_would() {
        assert_eq!(Span::centres_in(0.0, 0.0, 2.0, 1.0), Span { x0: 0, y0: 0, x1: 2, y1: 1 });
        assert_eq!(Span::centres_in(0.6, 0.0, 1.4, 1.0), Span { x0: 1, y0: 0, x1: 1, y1: 1 });
        assert_eq!(Span::centres_in(0.4, 0.0, 1.6, 1.0), Span { x0: 0, y0: 0, x1: 2, y1: 1 });
    }

    #[test]
    fn the_image_is_straight_alpha() {
        let mut c = Canvas::new(1, 1);
        c.clear([0.25, 0.0, 0.0, 0.5]);
        assert_eq!(c.to_image().rgba, [128, 0, 0, 128]);
    }
}
