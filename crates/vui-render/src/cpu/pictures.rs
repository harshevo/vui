//! Pictures: a mip chain per image, filtered in linear light like the GPU's sRGB textures, then
//! drawn with rounded corners, a crop, a ring and the three sampling modes (`shaders/image.wgsl`).

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use vui_core::color::linear_to_srgb;
use vui_core::scene::{ImageCmd, ImageMode, Sampling};
use vui_core::{ImageData, ImageId, Images};

use super::canvas::{Canvas, Span};
use super::shapes::{clip_factor, clip_span, coverage, scaled_clip, sd_box};

/// One level of the chain: premultiplied linear light, encoded as sRGB bytes, as uploaded to the GPU.
struct Level {
    width: u32,
    height: u32,
    texels: Vec<u8>,
}

/// The decoded pictures, kept between frames and dropped when no frame has drawn them for a while.
#[derive(Default)]
pub(crate) struct Pictures {
    chains: HashMap<ImageId, (Rc<Vec<Level>>, u64)>,
    frame: u64,
}

const KEEP_FRAMES: u64 = 120;

fn decode_table() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| vui_core::color::srgb_to_linear(i as f32 / 255.0)))
}

fn encode(v: f32) -> u8 {
    (to_srgb(v) * 255.0 + 0.5) as u8
}

/// Linear light to encoded sRGB through a table: a `powf` per channel per pixel is most of the cost
/// of drawing a picture otherwise.
pub(crate) fn to_srgb(v: f32) -> f32 {
    const STEPS: usize = 4096;
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    let table = TABLE.get_or_init(|| (0..=STEPS).map(|i| linear_to_srgb(i as f32 / STEPS as f32)).collect());
    let x = v.clamp(0.0, 1.0) * STEPS as f32;
    let i = (x as usize).min(STEPS - 1);
    let f = x - i as f32;
    table[i] + (table[i + 1] - table[i]) * f
}

fn chain(data: &ImageData) -> Vec<Level> {
    let mut levels = vec![Level { width: data.width, height: data.height, texels: data.premultiplied_linear() }];
    let table = decode_table();
    while let Some(last) = levels.last()
        && (last.width > 1 || last.height > 1)
    {
        let (w, h) = ((last.width / 2).max(1), (last.height / 2).max(1));
        let mut texels = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(last.width - 1);
                    let sy = (y * 2 + dy).min(last.height - 1);
                    let i = ((sy * last.width + sx) * 4) as usize;
                    for c in 0..3 {
                        sum[c] += table[last.texels[i + c] as usize];
                    }
                    sum[3] += last.texels[i + 3] as f32 / 255.0;
                }
                texels.extend_from_slice(&[encode(sum[0] / 4.0), encode(sum[1] / 4.0), encode(sum[2] / 4.0), ((sum[3] / 4.0) * 255.0 + 0.5) as u8]);
            }
        }
        levels.push(Level { width: w, height: h, texels });
    }
    levels
}

impl Level {
    #[inline]
    fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let x = x.clamp(0, self.width as i32 - 1) as u32;
        let y = y.clamp(0, self.height as i32 - 1) as u32;
        let i = ((y * self.width + x) * 4) as usize;
        let t = decode_table();
        [t[self.texels[i] as usize], t[self.texels[i + 1] as usize], t[self.texels[i + 2] as usize], self.texels[i + 3] as f32 / 255.0]
    }

    fn bilinear(&self, u: f32, v: f32) -> [f32; 4] {
        let tx = u * self.width as f32 - 0.5;
        let ty = v * self.height as f32 - 0.5;
        let (x0, y0) = (tx.floor(), ty.floor());
        let (fx, fy) = (tx - x0, ty - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        let [a, b, c, d] = [self.texel(x0, y0), self.texel(x0 + 1, y0), self.texel(x0, y0 + 1), self.texel(x0 + 1, y0 + 1)];
        std::array::from_fn(|i| (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy)
    }

    fn nearest(&self, u: f32, v: f32) -> [f32; 4] {
        self.texel((u * self.width as f32).floor() as i32, (v * self.height as f32).floor() as i32)
    }
}

fn trilinear(levels: &[Level], lod: f32, u: f32, v: f32) -> [f32; 4] {
    let top = (levels.len() - 1) as f32;
    let lod = lod.clamp(0.0, top);
    let lo = lod.floor() as usize;
    let f = lod - lo as f32;
    let a = levels[lo].bilinear(u, v);
    if f <= 0.0 || lo + 1 >= levels.len() {
        return a;
    }
    let b = levels[lo + 1].bilinear(u, v);
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f)
}

impl Pictures {
    pub(crate) fn end_frame(&mut self) {
        let frame = self.frame;
        self.chains.retain(|_, (_, used)| frame - *used < KEEP_FRAMES);
        self.frame += 1;
    }

    fn levels(&mut self, images: &Images, id: ImageId) -> Option<Rc<Vec<Level>>> {
        let frame = self.frame;
        if let Some((levels, used)) = self.chains.get_mut(&id) {
            *used = frame;
            return Some(levels.clone());
        }
        let levels = Rc::new(chain(&images.get(id)?));
        self.chains.insert(id, (levels.clone(), frame));
        Some(levels)
    }

    pub(crate) fn draw(&mut self, canvas: &mut Canvas, images: &Images, cmd: &ImageCmd, scale: f32) {
        let Some(levels) = self.levels(images, cmd.id) else { return };
        let s = |v: f32| v * scale;
        let rect = [s(cmd.rect.x), s(cmd.rect.y), s(cmd.rect.w), s(cmd.rect.h)];
        if rect[2] <= 0.0 || rect[3] <= 0.0 {
            return;
        }
        let radii = cmd.radii.to_array().map(s);
        let clip = scaled_clip(cmd.clip, scale);
        let uv = [cmd.uv.x, cmd.uv.y, cmd.uv.x + cmd.uv.w, cmd.uv.y + cmd.uv.h];
        let tint = cmd.tint.premultiplied();
        let (ring_w, ring) = cmd.ring.map_or((0.0, [0.0; 4]), |r| (s(r.width), r.color.premultiplied()));
        let tex = [levels[0].width as f32, levels[0].height as f32];

        let area = Span::centres_in(rect[0] - 1.0, rect[1] - 1.0, rect[0] + rect[2] + 1.0, rect[1] + rect[3] + 1.0)
            .intersect(clip_span(clip))
            .intersect(canvas.bounds());
        if area.is_empty() {
            return;
        }

        // Texels per screen pixel along each axis, and the mip level the GPU's sampler would pick
        // with its 4x anisotropy.
        let du = (uv[2] - uv[0]).abs() * tex[0] / rect[2];
        let dv = (uv[3] - uv[1]).abs() * tex[1] / rect[3];
        let (major, minor) = (du.max(dv), du.min(dv));
        let lod = minor.max(major / 4.0).max(1e-6).log2().max(0.0);

        let half = [rect[2] * 0.5, rect[3] * 0.5];
        let centre = [rect[0] + half[0], rect[1] + half[1]];
        let inner_half = [(half[0] - ring_w).max(0.0), (half[1] - ring_w).max(0.0)];
        let inner_radii = radii.map(|r| (r - ring_w).max(0.0));
        let crop_lo = [uv[0].min(uv[2]), uv[1].min(uv[3])];
        let crop_hi = [uv[0].max(uv[2]), uv[1].max(uv[3])];
        let per_texel = [(rect[2] / ((uv[2] - uv[0]).abs() * tex[0]).max(1e-4)).max(1.0), (rect[3] / ((uv[3] - uv[1]).abs() * tex[1]).max(1e-4)).max(1.0)];

        // Inside this the picture is whole: no corner, ring or clip edge reaches it.
        let inset = radii.iter().fold(0.0f32, |m, &r| m.max(r)) + ring_w + 1.0;
        let solid = Span::centres_in(rect[0] + inset, rect[1] + inset, rect[0] + rect[2] - inset, rect[1] + rect[3] - inset)
            .intersect(Span::centres_in(clip[0] + 0.5, clip[1] + 0.5, clip[0] + clip[2] - 0.5, clip[1] + clip[3] - 0.5));
        let in_solid = |x: i32, y: i32| x >= solid.x0 && x < solid.x1 && y >= solid.y0 && y < solid.y1;

        for y in area.y0..area.y1 {
            for x in area.x0..area.x1 {
                let px = [x as f32 + 0.5, y as f32 + 0.5];
                let p = [px[0] - centre[0], px[1] - centre[1]];
                let whole = in_solid(x, y);
                let outer = if whole {
                    1.0
                } else {
                    let (d, g) = sd_box(p, half, radii, cmd.smooth);
                    coverage(d, g)
                };
                if outer <= 0.0 {
                    continue;
                }
                let mut at = [0.0f32; 2];
                for i in 0..2 {
                    let t = ((px[i] - rect[i]) / rect[2 + i]).clamp(0.0, 1.0);
                    at[i] = uv[i] + (uv[2 + i] - uv[i]) * t;
                }
                if cmd.sampling == Sampling::Sharp {
                    for i in 0..2 {
                        let texel = at[i] * tex[i];
                        let base = texel.floor();
                        let from_centre = texel - base - 0.5;
                        let flat = 0.5 - 0.5 / per_texel[i];
                        let f = (from_centre - from_centre.clamp(-flat, flat)) * per_texel[i] + 0.5;
                        at[i] = (base + f) / tex[i];
                    }
                }
                for i in 0..2 {
                    let mid = (crop_lo[i] + crop_hi[i]) * 0.5;
                    let inset = 0.5 / tex[i];
                    at[i] = at[i].clamp((crop_lo[i] + inset).min(mid), (crop_hi[i] - inset).max(mid));
                }
                let c = match cmd.sampling {
                    Sampling::Nearest => levels[0].nearest(at[0], at[1]),
                    _ => trilinear(&levels, lod, at[0], at[1]),
                };

                let mut colour = [0.0f32; 4];
                if cmd.mode == ImageMode::Mask {
                    colour = tint.map(|t| t * c[3]);
                } else if c[3] > 0.0 {
                    for i in 0..3 {
                        colour[i] = to_srgb(c[i] / c[3]) * c[3];
                    }
                    colour[3] = c[3];
                }
                colour = colour.map(|v| v * cmd.opacity);

                let inner = if ring_w > 0.0 && !whole {
                    let (d, g) = sd_box(p, inner_half, inner_radii, cmd.smooth);
                    coverage(d, g)
                } else {
                    outer
                };
                let mut col = colour.map(|v| v * inner);
                if ring_w > 0.0 {
                    let edge = outer - inner;
                    for i in 0..4 {
                        col[i] += (ring[i] + colour[i] * (1.0 - ring[3])) * edge;
                    }
                }
                let k = if whole { 1.0 } else { clip_factor(px, clip) };
                canvas.blend(x, y, col.map(|v| v * k));
            }
        }
    }
}
