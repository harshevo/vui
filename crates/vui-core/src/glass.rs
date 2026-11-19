//! Glass: a surface that blurs and tints what was drawn behind it, bends it a little at the edge,
//! and lights its rim. The renderer draws it in its own backdrop pass (`vui-render`'s
//! `shaders/glass.wgsl`, with a CPU twin), so it is layered with everything else in paint order:
//! whatever the scene drew before a glass command is what the glass shows, blurred.
//!
//! A glass shape is one rounded box, or two joined by a smooth union, so a card can **pour** out of
//! another shape (a notch, a pill, a button) and drop back in like a droplet: animate the second box
//! out of the first and the neck between them stretches and snaps.
//!
//! The data is laid out here and read by the renderer; keep the two in step.

use crate::{Color, CustomCmd, Glass, Rect};

/// The name of the renderer's built-in glass pass.
pub const PASS: &str = "vui.glass";

/// How many floats a glass command carries.
pub const LEN: usize = 24;

/// Glass in the rounded box `shape`.
pub fn glass(shape: Rect, radius: f32, g: &Glass) -> CustomCmd {
    pour(shape, radius, shape, radius, 0.0, g)
}

/// A pill: glass with fully round ends, for a status or a notch on a display without one.
pub fn pill(shape: Rect, g: &Glass) -> CustomCmd {
    glass(shape, shape.w.min(shape.h) / 2.0, g)
}

/// Glass over the smooth union of two rounded boxes. `k` is how far, in points, the join reaches:
/// 0 is a hard union, 20–40 a liquid neck.
pub fn pour(a: Rect, ra: f32, b: Rect, rb: f32, k: f32, g: &Glass) -> CustomCmd {
    let k = k.max(0.0);
    let bounds = a.union(b).inflate(k * 0.25, k * 0.25);
    let reach = g.blur.max(0.0) * 1.5 + g.refraction.abs() + 2.0;
    let t = g.tint;
    let data = vec![
        a.x, a.y, a.w, a.h, ra, //
        b.x, b.y, b.w, b.h, rb, //
        k, t.r, t.g, t.b, t.a, g.opacity, //
        g.blur.max(0.0), g.saturation, g.refraction, g.dispersion, g.rim, g.specular, g.grain, reach,
    ];
    debug_assert_eq!(data.len(), LEN);
    CustomCmd::new(PASS, bounds, data)
}

/// The parts of a glass command, read back from its data.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shape {
    pub a: Rect,
    pub ra: f32,
    pub b: Rect,
    pub rb: f32,
    pub k: f32,
    pub glass: Glass,
    /// How far past the shape the backdrop is read, in points.
    pub reach: f32,
}

impl Shape {
    pub fn read(cmd: &CustomCmd) -> Option<Shape> {
        let d = &cmd.data;
        if d.len() < LEN {
            return None;
        }
        let r = |i: usize| Rect { x: d[i], y: d[i + 1], w: d[i + 2], h: d[i + 3] };
        Some(Shape {
            a: r(0),
            ra: d[4],
            b: r(5),
            rb: d[9],
            k: d[10],
            glass: Glass {
                tint: Color::rgba(d[11], d[12], d[13], d[14]),
                opacity: d[15],
                blur: d[16],
                saturation: d[17],
                refraction: d[18],
                dispersion: d[19],
                rim: d[20],
                specular: d[21],
                grain: d[22],
                floating: false,
            },
            reach: d[23],
        })
    }
}

/// The smooth minimum of two distances, joined over `k`: the union that pours one shape out of
/// another. With `k` 0 it is the plain minimum.
pub fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mode, rect};

    #[test]
    fn a_command_reads_back_as_it_was_made() {
        let g = Glass::for_mode(Mode::Dark);
        let cmd = pour(rect(10.0, 0.0, 100.0, 30.0), 12.0, rect(0.0, 20.0, 120.0, 80.0), 18.0, 24.0, &g);
        let s = Shape::read(&cmd).unwrap();
        assert_eq!(s.b, rect(0.0, 20.0, 120.0, 80.0));
        assert_eq!(s.glass, g);
        assert!(cmd.rect.contains(crate::pt(0.0, 99.0)), "the bounds hold both boxes");
        assert_eq!(cmd.pass, PASS);
    }

    #[test]
    fn the_smooth_union_dips_below_both_where_they_meet() {
        assert_eq!(smooth_min(3.0, 5.0, 0.0), 3.0);
        assert!(smooth_min(4.0, 4.0, 8.0) < 4.0, "a neck forms where both are close");
        assert_eq!(smooth_min(1.0, 20.0, 8.0), 1.0, "far apart it is the plain union");
    }

    #[test]
    fn a_pill_has_round_ends() {
        let cmd = pill(rect(0.0, 0.0, 120.0, 32.0), &Glass::for_mode(Mode::Light));
        assert_eq!(Shape::read(&cmd).unwrap().ra, 16.0);
    }
}
