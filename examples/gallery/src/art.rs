//! Pictures made by arithmetic, so the gallery needs no files and its screenshots never change.

use vui::core::ImageData;

type Rgb = [f32; 3];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The looks of the pictures: a sky of three colours, and where the sun is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Scene {
    Dusk,
    Lagoon,
    Meadow,
    Ember,
}

impl Scene {
    fn colors(self) -> (Rgb, Rgb, Rgb, Rgb) {
        let hex = |h: u32| [((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0];
        match self {
            Scene::Dusk => (hex(0x1d2b64), hex(0xc0546b), hex(0xf8b178), hex(0xffe6b8)),
            Scene::Lagoon => (hex(0x0b3954), hex(0x1b8a9a), hex(0x9fe0d0), hex(0xf4fff5)),
            Scene::Meadow => (hex(0x2d5a3d), hex(0x7bb661), hex(0xdce8a0), hex(0xfffbe0)),
            Scene::Ember => (hex(0x2a1136), hex(0xa63446), hex(0xff9a56), hex(0xfff0c8)),
        }
    }

    fn sun(self) -> (f32, f32) {
        match self {
            Scene::Dusk => (0.68, 0.62),
            Scene::Lagoon => (0.30, 0.34),
            Scene::Meadow => (0.76, 0.30),
            Scene::Ember => (0.45, 0.70),
        }
    }
}

/// A soft landscape: layered colour bands bent by waves, with a glow where the sun is.
pub fn landscape(scene: Scene, width: u32, height: u32) -> ImageData {
    let (deep, mid, light, glow) = scene.colors();
    let (sx, sy) = scene.sun();
    let aspect = width as f32 / height as f32;
    ImageData::from_fn(width, height, |x, y| {
        let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
        let bend = 0.06 * (u * 7.0 + v * 2.0).sin() + 0.035 * (u * 15.0 - v * 4.0).sin();
        let t = (v + bend).clamp(0.0, 1.0);
        let mut c = mix(deep, mid, smooth(t * 1.6));
        c = mix(c, light, smooth((t - 0.45) * 2.0));
        let (dx, dy) = ((u - sx) * aspect, v - sy);
        let d = (dx * dx + dy * dy).sqrt();
        c = mix(c, glow, smooth(1.0 - d / 0.55) * 0.85);
        // A hard-edged sun, so scaling it shows whether edges stay clean.
        c = mix(c, glow, smooth((0.085 - d) / 0.012));
        [byte(c[0]), byte(c[1]), byte(c[2]), 255]
    })
}

/// Fine lines and small text-like marks: the picture that shows a bad downscale as a moire or a blur.
pub fn fine_detail(width: u32, height: u32) -> ImageData {
    ImageData::from_fn(width, height, |x, y| {
        let line = (x % 6 == 0) as u8 | (y % 6 == 0) as u8;
        let bar = ((x / 24 + y / 24) % 2 == 0) as u8;
        let ink = if line == 1 { 40 } else if bar == 1 { 232 } else { 250 };
        [ink, ink, ink.saturating_add(if bar == 1 { 6 } else { 0 }), 255]
    })
}

/// A picture with transparency: a disc that fades to nothing at the edge.
pub fn soft_disc(side: u32) -> ImageData {
    ImageData::from_fn(side, side, |x, y| {
        let (u, v) = ((x as f32 + 0.5) / side as f32 * 2.0 - 1.0, (y as f32 + 0.5) / side as f32 * 2.0 - 1.0);
        let d = (u * u + v * v).sqrt();
        [37, 99, 235, byte(smooth((1.0 - d) * 3.0))]
    })
}
