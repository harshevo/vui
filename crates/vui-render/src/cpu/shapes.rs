//! Rounded boxes and the built-in effects, evaluated per pixel with the same distance fields the
//! GPU shaders use (`shaders/sdf.wgsl`, `shaders/quad.wgsl`, `shaders/effects.wgsl`).

use vui_core::scene::Quad;
use vui_core::{CustomCmd, Rect};

use super::canvas::{Canvas, Span};

const NO_CLIP: [f32; 4] = [-1.0e6, -1.0e6, 2.0e6, 2.0e6];

/// Signed distance to a rounded box centred on the origin, and the length of its gradient.
pub(crate) fn sd_box(p: [f32; 2], half: [f32; 2], radii: [f32; 4], smooth: f32) -> (f32, f32) {
    let r = match (p[0] < 0.0, p[1] < 0.0) {
        (true, true) => radii[0],
        (false, true) => radii[1],
        (false, false) => radii[2],
        (true, false) => radii[3],
    };
    let r = r.min(half[0].min(half[1]));
    let q = [p[0].abs() - half[0] + r, p[1].abs() - half[1] + r];
    let inside = q[0].max(q[1]).min(0.0);
    let qp = [q[0].max(0.0), q[1].max(0.0)];
    if qp[0] == 0.0 || qp[1] == 0.0 {
        // Beside a straight edge, not in a corner: every norm is the distance along the one axis.
        return (inside + qp[0] + qp[1] - r, 1.0);
    }
    if smooth <= 0.001 {
        return (inside + (qp[0] * qp[0] + qp[1] * qp[1]).sqrt() - r, 1.0);
    }
    let n = 2.0 + 3.0 * smooth;
    let corner = (qp[0].powf(n) + qp[1].powf(n)).powf(1.0 / n);
    let mut grad = 1.0;
    if corner > 0.0 {
        let gx = qp[0].powf(n - 1.0);
        let gy = qp[1].powf(n - 1.0);
        grad = (gx * gx + gy * gy).sqrt() / corner.powf(n - 1.0);
    }
    (inside + corner - r, grad)
}

#[inline]
pub(crate) fn coverage(d: f32, grad: f32) -> f32 {
    (0.5 - d / grad.max(1e-4)).clamp(0.0, 1.0)
}

/// How much of the pixel centred at `p` survives a soft clip to `clip` (x, y, w, h).
#[inline]
pub(crate) fn clip_factor(p: [f32; 2], clip: [f32; 4]) -> f32 {
    let cx = ((p[0] - clip[0]).min(clip[0] + clip[2] - p[0]) + 0.5).clamp(0.0, 1.0);
    let cy = ((p[1] - clip[1]).min(clip[1] + clip[3] - p[1]) + 0.5).clamp(0.0, 1.0);
    cx * cy
}

pub(crate) fn scaled_clip(clip: Option<Rect>, scale: f32) -> [f32; 4] {
    clip.map_or(NO_CLIP, |c| [c.x * scale, c.y * scale, c.w * scale, c.h * scale])
}

/// The pixels a clip can touch at all.
pub(crate) fn clip_span(clip: [f32; 4]) -> Span {
    Span::centres_in(clip[0] - 0.5, clip[1] - 0.5, clip[0] + clip[2] + 0.5, clip[1] + clip[3] + 0.5)
}

#[inline]
fn scale4(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, c[3] * k]
}

pub(crate) fn quad(canvas: &mut Canvas, q: &Quad, scale: f32) {
    let s = |v: f32| v * scale;
    let rect = [s(q.rect.x), s(q.rect.y), s(q.rect.w), s(q.rect.h)];
    let (bw, border) = match q.border {
        Some(b) => (s(b.width), b.color.premultiplied()),
        None => (0.0, [0.0; 4]),
    };
    let fill = q.fill.premultiplied();
    if fill[3] <= 0.0 && (bw <= 0.0 || border[3] <= 0.0) && fill[..3].iter().all(|&c| c <= 0.0) {
        return;
    }
    let radii = q.radii.to_array().map(s);
    let clip = scaled_clip(q.clip, scale);
    let half = [rect[2] * 0.5, rect[3] * 0.5];
    let centre = [rect[0] + half[0], rect[1] + half[1]];

    // The GPU draws a quad one pixel larger than the box, so the antialiased edge is not cut off.
    let area = Span::centres_in(rect[0] - 1.0, rect[1] - 1.0, rect[0] + rect[2] + 1.0, rect[1] + rect[3] + 1.0)
        .intersect(clip_span(clip))
        .intersect(canvas.bounds());
    if area.is_empty() {
        return;
    }

    // Inside this, every pixel is wholly the fill: away from the corners, the border and the clip.
    let inset = radii.iter().fold(0.0f32, |m, &r| m.max(r)) + bw + 1.0;
    let solid = Span::centres_in(rect[0] + inset, rect[1] + inset, rect[0] + rect[2] - inset, rect[1] + rect[3] - inset)
        .intersect(Span::centres_in(clip[0] + 0.5, clip[1] + 0.5, clip[0] + clip[2] - 0.5, clip[1] + clip[3] - 0.5))
        .intersect(area);

    let inner_half = [(half[0] - bw).max(0.0), (half[1] - bw).max(0.0)];
    let inner_radii = radii.map(|r| (r - bw).max(0.0));
    let shade = |canvas: &mut Canvas, x: i32, y: i32| {
        let px = [x as f32 + 0.5, y as f32 + 0.5];
        let p = [px[0] - centre[0], px[1] - centre[1]];
        let (d, g) = sd_box(p, half, radii, q.smooth);
        let outer = coverage(d, g);
        if outer <= 0.0 {
            return;
        }
        let inner = if bw > 0.0 {
            let (d, g) = sd_box(p, inner_half, inner_radii, q.smooth);
            coverage(d, g)
        } else {
            outer
        };
        let mut col = scale4(fill, inner);
        if bw > 0.0 {
            let b = scale4(border, outer - inner);
            col = [col[0] + b[0], col[1] + b[1], col[2] + b[2], col[3] + b[3]];
        }
        canvas.blend(x, y, scale4(col, clip_factor(px, clip)));
    };

    for y in area.y0..area.y1 {
        if solid.is_empty() || y < solid.y0 || y >= solid.y1 {
            for x in area.x0..area.x1 {
                shade(canvas, x, y);
            }
            continue;
        }
        for x in area.x0..solid.x0 {
            shade(canvas, x, y);
        }
        canvas.fill_span(y, solid.x0, solid.x1, fill);
        for x in solid.x1..area.x1 {
            shade(canvas, x, y);
        }
    }
}

pub(crate) fn rounded_box(p: [f32; 2], half: [f32; 2], r: f32) -> f32 {
    let rr = r.min(half[0].min(half[1]));
    let q = [p[0].abs() - half[0] + rr, p[1].abs() - half[1] + rr];
    let out = [q[0].max(0.0), q[1].max(0.0)];
    (out[0] * out[0] + out[1] * out[1]).sqrt() + q[0].max(q[1]).min(0.0) - rr
}

/// Abramowitz and Stegun 7.1.26, as in the shader, with its constants written as published.
#[allow(clippy::excessive_precision)]
fn erf(x: f32) -> f32 {
    let a = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-a * a).exp();
    y.copysign(x)
}

/// A shadow, shimmer or glow from the built-in effects pass, laid out by `crate::effects::floats`.
pub(crate) fn effect(canvas: &mut Canvas, cmd: &CustomCmd, scale: f32) {
    let viewport = [canvas.width as f32, canvas.height as f32];
    let e = crate::effects::floats(cmd, viewport, scale);
    let (area, clip, shape) = (&e[0..4], &e[4..8], &e[8..12]);
    let [kind, radius, a, b] = [e[12], e[13], e[14], e[15]];
    let color = [e[16], e[17], e[18], e[19]];
    let [c, time] = [e[20], e[21]];

    let span = Span::centres_in(area[0], area[1], area[0] + area[2], area[1] + area[3])
        .intersect(Span::centres_in(clip[0].max(-1.0e6), clip[1].max(-1.0e6), clip[2].min(1.0e7), clip[3].min(1.0e7)))
        .intersect(canvas.bounds());
    if span.is_empty() {
        return;
    }
    let half = [shape[2] * 0.5, shape[3] * 0.5];
    let centre = [shape[0] + half[0], shape[1] + half[1]];
    let kind = (kind + 0.5) as u32;

    for y in span.y0..span.y1 {
        for x in span.x0..span.x1 {
            let px = [x as f32 + 0.5, y as f32 + 0.5];
            let p = [px[0] - centre[0], px[1] - centre[1]];
            let k = match kind {
                0 => {
                    let sigma = (a * 0.5 * scale).max(0.5);
                    let drop = b * scale;
                    let spread = c * scale;
                    let grown = [(half[0] + spread).max(0.0), (half[1] + spread).max(0.0)];
                    let d = rounded_box([p[0], p[1] - drop], grown, radius + spread);
                    let cover = 0.5 - 0.5 * erf(d / (sigma * std::f32::consts::SQRT_2));
                    let inside = (0.5 - rounded_box(p, half, radius)).clamp(0.0, 1.0);
                    cover * (1.0 - inside)
                }
                1 => {
                    let d = rounded_box(p, half, radius);
                    let band = b * scale;
                    let t = (time / a).rem_euclid(1.0);
                    let travel = shape[2] + band * 4.0;
                    let from = shape[0] - band * 2.0 + t * travel * 1.25;
                    let k = (px[0] - from) / band;
                    (-k * k).exp() * (0.5 - d).clamp(0.0, 1.0)
                }
                _ => {
                    let d = rounded_box(p, half, radius);
                    let width = a * scale;
                    let blur = (b * scale).max(0.5);
                    let breathe = if c > 0.0 { 0.7 + 0.3 * (time / c * std::f32::consts::TAU).sin() } else { 1.0 };
                    let ring = (width * 0.5 + 0.5 - d.abs()).clamp(0.0, 1.0);
                    let out = d.max(0.0) / blur;
                    let halo = if d >= 0.0 { (-out * out).exp() * 0.45 } else { 0.0 };
                    (ring + halo).clamp(0.0, 1.0) * breathe
                }
            };
            if k > 0.0 {
                canvas.blend(x, y, scale4(color, k));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_distance_is_negative_inside_and_positive_outside() {
        let half = [10.0, 5.0];
        assert!(sd_box([0.0, 0.0], half, [2.0; 4], 0.0).0 < 0.0);
        assert!((sd_box([12.0, 0.0], half, [2.0; 4], 0.0).0 - 2.0).abs() < 1e-5);
        // A rounded corner is cut away; a square one is not.
        assert!(sd_box([9.8, 4.8], half, [4.0; 4], 0.0).0 > 0.0);
        assert!(sd_box([9.8, 4.8], half, [0.0; 4], 0.0).0 < 0.0);
    }

    #[test]
    fn erf_is_odd_and_saturates() {
        assert!(erf(0.0).abs() < 1e-6);
        assert!((erf(3.0) - 1.0).abs() < 1e-4);
        assert!((erf(-1.0) + erf(1.0)).abs() < 1e-6);
    }
}
