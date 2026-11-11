//! The renderer's built-in shading pass: gradients, aurora, beams, spotlights and grain, described in
//! [`vui_core::shading`]. The CPU renderer draws the same commands from the same floats.

use vui_core::shading::{MAX_STOPS, to_oklab};
use vui_core::{Color, CustomCmd};

use crate::instanced::InstancedPass;

/// Eleven vec4s: area, clip, shape, head, params, four stops, positions, misc.
pub(crate) const FLOATS: usize = 44;

pub(crate) fn pass(device: &wgpu::Device, format: wgpu::TextureFormat) -> InstancedPass {
    InstancedPass::new(device, format, "shading", include_str!("shaders/shading.wgsl"), |c, viewport, scale, out| {
        out.extend_from_slice(&floats(c, viewport, scale))
    })
}

/// One command's slot in the storage buffer, in device pixels. Stops arrive in Oklab, premultiplied
/// by their alpha (and the command's opacity), so the shader only mixes and converts back.
pub(crate) fn floats(c: &CustomCmd, viewport: [f32; 2], scale: f32) -> [f32; FLOATS] {
    let d = |i: usize| c.data.get(i).copied().unwrap_or(0.0);
    let r = c.rect;
    let clip = c.clip.map_or([0.0, 0.0, f32::MAX, f32::MAX], |k| [k.x * scale, k.y * scale, (k.x + k.w) * scale, (k.y + k.h) * scale]);
    let mut out = [0.0; FLOATS];
    out[0..4].copy_from_slice(&[r.x * scale, r.y * scale, r.w * scale, r.h * scale]);
    out[4..8].copy_from_slice(&clip);
    out[8..12].copy_from_slice(&[d(1) * scale, d(2) * scale, d(3) * scale, d(4) * scale]);
    out[12..16].copy_from_slice(&[d(0), d(5) * scale, d(31), scale]);
    out[16..20].copy_from_slice(&[d(6), d(7), d(8), d(9)]);
    for i in 0..MAX_STOPS {
        let k = 15 + i * 4;
        let color = Color::rgba(d(k), d(k + 1), d(k + 2), (d(k + 3) * c.opacity).clamp(0.0, 1.0));
        let [l, a, b, alpha] = to_oklab(color);
        out[20 + i * 4..24 + i * 4].copy_from_slice(&[l * alpha, a * alpha, b * alpha, alpha]);
    }
    out[36..40].copy_from_slice(&[d(11), d(12), d(13), d(14)]);
    out[40..44].copy_from_slice(&[d(10), d(32) * scale, viewport[0], viewport[1]]);
    out
}

/// The grain's noise: an integer hash of the pixel, the same on the GPU and the CPU.
pub(crate) fn hash(x: u32, y: u32) -> f32 {
    let mut v = x.wrapping_mul(747_796_405).wrapping_add(y.wrapping_mul(2_891_336_453));
    v = ((v >> ((v >> 28) + 4)) ^ v).wrapping_mul(277_803_737);
    v = (v >> 22) ^ v;
    v as f32 / 4_294_967_295.0
}
