//! Layers on the CPU: the layer's commands are drawn into a canvas of their own, then each pixel
//! the transformed layer covers maps back through the inverse transform and filters the four
//! nearest pixels, with the same numbers and sums as `shaders/layer.wgsl`.

use vui_core::LayerCmd;

use super::canvas::{Canvas, Span};
use crate::layer::LayerInstance;

/// Composites `layer`, already drawn into `src`, over `dst`. `region` is the part of `src` the GPU
/// would have copied into the layer's texture: filtering clamps to it as a texture's edge would.
pub(crate) fn composite(dst: &mut Canvas, src: &Canvas, layer: &LayerCmd, scale: f32, region: [u32; 4]) {
    let Some(l) = LayerInstance::new(layer, scale, region) else { return };
    let [rx, ry, rw, rh] = region.map(|v| v as i32);
    let span = Span::centres_in(l.rect[0], l.rect[1], l.rect[0] + l.rect[2], l.rect[1] + l.rect[3]).intersect(dst.bounds());
    if span.is_empty() {
        return;
    }
    let texel = |x: i32, y: i32| src.texel(rx + x.clamp(0, rw - 1), ry + y.clamp(0, rh - 1));
    let mix = |a: [f32; 4], b: [f32; 4], t: f32| [0, 1, 2, 3].map(|c| a[c] * (1.0 - t) + b[c] * t);
    let [a, b, c, d] = l.inv;
    let [e, f, opacity, _] = l.inv_t;
    let [k0, k1, k2, k3] = l.keep;
    let clip = l.clip;
    for y in span.y0..span.y1 {
        let py = y as f32 + 0.5;
        for x in span.x0..span.x1 {
            let px = x as f32 + 0.5;
            let (qx, qy) = (a * px + c * py + e, b * px + d * py + f);
            if qx < k0 || qy < k1 || qx >= k2 || qy >= k3 {
                continue;
            }
            let (sx, sy) = (qx - 0.5, qy - 0.5);
            let (bx, by) = (sx.floor(), sy.floor());
            let (fx, fy) = (sx - bx, sy - by);
            let (ix, iy) = (bx as i32, by as i32);
            let top = mix(texel(ix, iy), texel(ix + 1, iy), fx);
            let bottom = mix(texel(ix, iy + 1), texel(ix + 1, iy + 1), fx);
            let col = mix(top, bottom, fy);
            let cx = ((px - clip[0]).min(clip[0] + clip[2] - px) + 0.5).clamp(0.0, 1.0);
            let cy = ((py - clip[1]).min(clip[1] + clip[3] - py) + 0.5).clamp(0.0, 1.0);
            let k = opacity * cx * cy;
            dst.blend(x, y, col.map(|v| v * k));
        }
    }
}
