//! The glass pass on the CPU: the same shrink, blur and shading as `shaders/glass.wgsl`, rounding to
//! 8 bits after each step where the GPU stores to a texture.

use vui_core::CustomCmd;

use super::canvas::Canvas;
use crate::glass::{self, Params};

pub(crate) fn draw(canvas: &mut Canvas, cmd: &CustomCmd, scale: f32) {
    let Some(pr) = Params::new(cmd, scale, (canvas.width, canvas.height), true) else { return };
    let blurred = blur(canvas, &pr);
    let (dw, _) = pr.shrunk();
    let texel = |x: i32, y: i32| blurred[y as usize * dw as usize + x as usize];
    let span = super::canvas::Span::centres_in(pr.area[0], pr.area[1], pr.area[0] + pr.area[2], pr.area[1] + pr.area[3]).intersect(canvas.bounds());
    for y in span.y0..span.y1 {
        for x in span.x0..span.x1 {
            let px = [x as f32 + 0.5, y as f32 + 0.5];
            if let Some(c) = glass::shade(px, &pr, |p| glass::bilinear(p, &pr, texel)) {
                canvas.blend(x, y, c);
            }
        }
    }
}

fn q8(v: f32) -> f32 {
    (v.clamp(0.0, 1.0) * 255.0).round() / 255.0
}

/// The region under the glass, shrunk and blurred across and down.
fn blur(canvas: &Canvas, pr: &Params) -> Vec<[f32; 4]> {
    let f = pr.down[0] as i32;
    let (dw, dh) = (pr.down[1] as i32, pr.down[2] as i32);
    let [x0, y0, x1, y1] = pr.region.map(|v| v as i32);
    let mut down = vec![[0.0f32; 4]; (dw * dh) as usize];
    let inv = 1.0 / (f * f) as f32;
    for j in 0..dh {
        for i in 0..dw {
            let mut sum = [0.0f32; 4];
            for v in 0..f {
                for u in 0..f {
                    let x = (x0 + i * f + u).clamp(x0, x1 - 1);
                    let y = (y0 + j * f + v).clamp(y0, y1 - 1);
                    let t = canvas.texel(x, y);
                    for c in 0..4 {
                        sum[c] += t[c];
                    }
                }
            }
            down[(j * dw + i) as usize] = sum.map(|s| q8(s * inv));
        }
    }
    let taps = pr.down[3] as i32;
    if taps == 0 {
        return down;
    }
    let sigma = pr.blur[0];
    let weights: Vec<f32> = (-taps..=taps).map(|k| glass::weight(k as f32, sigma)).collect();
    let total: f32 = weights.iter().sum();
    let pass = |src: &[[f32; 4]], across: bool| {
        let mut out = vec![[0.0f32; 4]; src.len()];
        for j in 0..dh {
            for i in 0..dw {
                let mut sum = [0.0f32; 4];
                for (n, w) in weights.iter().enumerate() {
                    let k = n as i32 - taps;
                    let (x, y) = if across { ((i + k).clamp(0, dw - 1), j) } else { (i, (j + k).clamp(0, dh - 1)) };
                    let t = src[(y * dw + x) as usize];
                    for c in 0..4 {
                        sum[c] += t[c] * w;
                    }
                }
                out[(j * dw + i) as usize] = sum.map(|s| q8(s / total));
            }
        }
        out
    };
    let across = pass(&down, true);
    pass(&across, false)
}
