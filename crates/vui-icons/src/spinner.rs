//! The busy indicator: a faint ring with a bright arc that travels round it. The scene has no
//! rotation, so each position is its own mask, made once.

use vui_core::{ImageId, Images};

use crate::icon::{GRID, Weight};
use crate::path::{P, Poly, p};
use crate::raster::{Canvas, Mask};

/// Positions of the arc in one turn.
pub const SPINNER_FRAMES: u32 = 60;

const RADIUS: f32 = 8.5;
/// How much of the ring the arc covers, in degrees.
const SWEEP: f32 = 100.0;
/// How strong the ring under the arc is.
const TRACK: f32 = 0.2;

fn arc(centre: P, radius: f32, from: f32, sweep: f32) -> Poly {
    let steps = (sweep.abs() / 3.0).ceil().max(1.0) as usize;
    let pts = (0..=steps)
        .map(|i| {
            let (s, c) = (from + sweep * i as f32 / steps as f32).to_radians().sin_cos();
            p(centre.x + radius * c, centre.y + radius * s)
        })
        .collect();
    Poly { pts, closed: false }
}

/// One position of the spinner, `px` pixels square. Frames count clockwise from the top; any number
/// works, wrapping at [`SPINNER_FRAMES`].
pub fn render(px: u32, frame: u32, weight: Weight) -> Mask {
    let size = px.max(1) as usize;
    let scale = size as f32 / GRID;
    let centre = p(GRID / 2.0 * scale, GRID / 2.0 * scale);
    let width = weight.stroke() * scale;
    let mut canvas = Canvas::new(size, size);
    let ring = Poly { closed: true, ..arc(centre, RADIUS * scale, 0.0, 360.0 - 3.0) };
    canvas.stroke(&[ring], width, TRACK);
    let from = -90.0 + 360.0 * (frame % SPINNER_FRAMES) as f32 / SPINNER_FRAMES as f32;
    canvas.stroke(&[arc(centre, RADIUS * scale, from, SWEEP)], width, 1.0);
    canvas.finish()
}

/// The frame as a mask image in `images`, made once and shared.
pub fn image(images: &Images, px: u32, frame: u32, weight: Weight) -> ImageId {
    let frame = frame % SPINNER_FRAMES;
    images.get_or_insert_with(&("spinner", px, frame, weight), || render(px, frame, weight).to_image())
}
