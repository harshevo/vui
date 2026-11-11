//! The shading pass on the CPU: gradients, aurora, beams, spotlights and grain from the same floats
//! and the same formulas as `shaders/shading.wgsl`.

use std::f32::consts::TAU;

use vui_core::CustomCmd;
use vui_core::shading::oklab_to_linear;

use super::canvas::{Canvas, Span};
use super::pictures::to_srgb;
use super::shapes::rounded_box;
use crate::shading::{floats, hash};

type V4 = [f32; 4];

fn mix(a: V4, b: V4, f: f32) -> V4 {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f)
}

fn ramp(stops: &[V4; 4], pos: &[f32; 4], n: usize, t: f32) -> V4 {
    if n == 0 {
        return [0.0; 4];
    }
    if n == 1 || t <= pos[0] {
        return stops[0];
    }
    for i in 0..n - 1 {
        if t <= pos[i + 1] {
            let span = pos[i + 1] - pos[i];
            let f = if span > 1e-5 { ((t - pos[i]) / span).clamp(0.0, 1.0) } else { 0.0 };
            return mix(stops[i], stops[i + 1], f);
        }
    }
    stops[n - 1]
}

fn encode(lab: V4) -> V4 {
    if lab[3] <= 0.0 {
        return [0.0; 4];
    }
    let rgb = oklab_to_linear([lab[0] / lab[3], lab[1] / lab[3], lab[2] / lab[3]]);
    [to_srgb(rgb[0]) * lab[3], to_srgb(rgb[1]) * lab[3], to_srgb(rgb[2]) * lab[3], lab[3]]
}

fn smooth01(x: f32) -> f32 {
    let s = x.clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

pub(crate) fn shade(canvas: &mut Canvas, cmd: &CustomCmd, scale: f32) {
    let e = floats(cmd, [canvas.width as f32, canvas.height as f32], scale);
    let (area, clip, shape, head, params) = (&e[0..4], &e[4..8], &e[8..12], &e[12..16], &e[16..20]);
    let stops: [V4; 4] = std::array::from_fn(|i| [e[20 + i * 4], e[21 + i * 4], e[22 + i * 4], e[23 + i * 4]]);
    let pos = [e[36], e[37], e[38], e[39]];
    let n = (e[40] + 0.5) as usize;
    let kind = (head[0] + 0.5) as u32;
    let (radius, time) = (head[1], head[2]);
    let ring = if kind == 4 { params[0] * scale } else { e[41] };

    let span = Span::centres_in(area[0], area[1], area[0] + area[2], area[1] + area[3])
        .intersect(Span::centres_in(clip[0].max(-1.0e6), clip[1].max(-1.0e6), clip[2].min(1.0e7), clip[3].min(1.0e7)))
        .intersect(canvas.bounds());
    if span.is_empty() {
        return;
    }
    let half = [shape[2] * 0.5, shape[3] * 0.5];
    let centre = [shape[0] + half[0], shape[1] + half[1]];
    let longer = shape[2].max(shape[3]);

    for y in span.y0..span.y1 {
        for x in span.x0..span.x1 {
            let px = [x as f32 + 0.5, y as f32 + 0.5];
            let p = [px[0] - centre[0], px[1] - centre[1]];
            let d = rounded_box(p, half, radius);
            let cover = if ring > 0.0 { (0.5 - ((d + ring * 0.5).abs() - ring * 0.5)).clamp(0.0, 1.0) } else { (0.5 - d).clamp(0.0, 1.0) };
            if cover <= 0.0 {
                continue;
            }
            let colour = match kind {
                0 => {
                    let dir = [params[0].cos(), params[0].sin()];
                    let extent = (half[0] * dir[0]).abs() + (half[1] * dir[1]).abs();
                    let t = (p[0] * dir[0] + p[1] * dir[1]) / (2.0 * extent.max(0.5)) + 0.5;
                    encode(ramp(&stops, &pos, n, t))
                }
                1 => {
                    let o = [shape[0] + params[0] * shape[2], shape[1] + params[1] * shape[3]];
                    let r = (params[2] * longer).max(1.0);
                    let t = ((px[0] - o[0]).hypot(px[1] - o[1])) / r;
                    encode(ramp(&stops, &pos, n, t))
                }
                2 => {
                    let o = [shape[0] + params[0] * shape[2], shape[1] + params[1] * shape[3]];
                    let t = (((px[1] - o[1]).atan2(px[0] - o[0]) - params[2]) / TAU).rem_euclid(1.0);
                    encode(ramp(&stops, &pos, n, t))
                }
                3 => {
                    let tau = time / params[0] * TAU;
                    let size = (params[1] * longer).max(1.0);
                    let mut acc = stops[0].map(|v| v * 0.15);
                    let mut total = 0.15;
                    for (i, stop) in stops.iter().enumerate().take(n) {
                        let fi = i as f32;
                        let phase = fi * 2.39996;
                        let at = [
                            shape[0] + shape[2] * (0.5 + 0.38 * (tau * (1.0 + 0.31 * fi) + phase).sin()),
                            shape[1] + shape[3] * (0.5 + 0.34 * (tau * (0.8 + 0.23 * fi) + phase * 1.7).cos()),
                        ];
                        let k = (px[0] - at[0]).hypot(px[1] - at[1]) / size;
                        let w = (-k * k).exp();
                        for c in 0..4 {
                            acc[c] += stop[c] * w;
                        }
                        total += w;
                    }
                    let mut lab = acc.map(|v| v / total);
                    let noise = hash(px[0].max(0.0) as u32, px[1].max(0.0) as u32) - 0.5;
                    lab[0] += noise * params[2] * lab[3];
                    encode(lab)
                }
                4 => {
                    let turn = (p[1].atan2(p[0]) / TAU + 0.25).rem_euclid(1.0);
                    let head = (time / params[1]).rem_euclid(1.0);
                    let behind = (head - turn).rem_euclid(1.0);
                    if behind > params[2] {
                        continue;
                    }
                    let s = 1.0 - behind / params[2];
                    encode(ramp(&stops, &pos, n, s)).map(|v| v * smooth01(s))
                }
                5 => {
                    let o = [shape[0] + params[0] * scale, shape[1] + params[1] * scale];
                    let q = (px[0] - o[0]).hypot(px[1] - o[1]) / (params[2] * scale * 0.5).max(1.0);
                    encode(stops[0]).map(|v| v * (-q * q * 2.5).exp())
                }
                _ => {
                    let v = (hash(px[0].max(0.0) as u32, px[1].max(0.0) as u32) - 0.5) * 2.0 * params[0];
                    if v > 0.0 { [v, v, v, v] } else { [0.0, 0.0, 0.0, -v] }
                }
            };
            canvas.blend(x, y, colour.map(|v| v * cover));
        }
    }
}
