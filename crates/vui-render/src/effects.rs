//! The renderer's built-in effects pass: shadows, shimmer and glow, described in
//! [`vui_core::effects`]. Every renderer has it, so widgets can use it without the app adding it.

use vui_core::CustomCmd;

use crate::instanced::InstancedPass;

pub(crate) const FLOATS: usize = 28;

pub(crate) fn pass(device: &wgpu::Device, format: wgpu::TextureFormat) -> InstancedPass {
    InstancedPass::new(device, format, "effects", include_str!("shaders/effects.wgsl"), |c, viewport, scale, out| {
        out.extend_from_slice(&floats(c, viewport, scale))
    })
}

/// One command's slot in the storage buffer, in device pixels with premultiplied colour.
pub(crate) fn floats(c: &CustomCmd, viewport: [f32; 2], scale: f32) -> [f32; FLOATS] {
    let d = |i: usize| c.data.get(i).copied().unwrap_or(0.0);
    let r = c.rect;
    let clip = c.clip.map_or([0.0, 0.0, f32::MAX, f32::MAX], |k| [k.x * scale, k.y * scale, (k.x + k.w) * scale, (k.y + k.h) * scale]);
    let a = (d(9) * c.opacity).clamp(0.0, 1.0);
    [
        r.x * scale, r.y * scale, r.w * scale, r.h * scale,
        clip[0], clip[1], clip[2], clip[3],
        d(1) * scale, d(2) * scale, d(3) * scale, d(4) * scale,
        d(0), d(5) * scale, d(10), d(11),
        d(6) * a, d(7) * a, d(8) * a, a,
        d(12), d(13), scale, 0.0,
        viewport[0], viewport[1], 0.0, 0.0,
    ]
}
