//! Elastic: a layer bent by its own motion as it springs in and out across one side of its box, the
//! way a panel attached to the side of a window slides out on a spring. A renderer draws it by
//! asking, for each pixel, which point of the layer shows there ([`source`]); the GPU's layer shader
//! does the very same sums.
//!
//! ```text
//!     coming in             past its place             at rest
//!   ┌─────────╮│         ╭────────────┐│          ┌──────────┐│
//!   │         ││        (             ││          │          ││
//!   └─────────╯│         ╰────────────┘│          └──────────┘│
//!   the far side lags    stretched from the       settled
//!   behind its corners   side it hangs on
//! ```
//!
//! The box hangs on one side, its [`Edge`]. Sliding out it moves across that side and is cut there,
//! so it goes as if under it. Past its place, where a spring overshoots, it does not come loose:
//! its far side reaches further and the content stretches to follow. And the far side bows with
//! the speed, its middle lagging its corners, so it flexes as it moves and wobbles as it settles.
//! What the layer draws beyond its far side, a shadow, moves with that side.

use crate::Rect;

/// A side of a box, or of the window.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Edge {
    Left,
    /// The default: where a panel beside the page hangs.
    #[default]
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// The number the layer shader knows the side by.
    pub fn code(self) -> u32 {
        match self {
            Edge::Right => 0,
            Edge::Left => 1,
            Edge::Bottom => 2,
            Edge::Top => 3,
        }
    }

    /// Whether the box slides across it sideways.
    pub fn is_side(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

/// A box hanging on one side, and how it is moving.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Elastic {
    /// The box at rest, in window coordinates before the layer's transform.
    pub from: Rect,
    /// The side it hangs on: it slides out across this side and stretches from it.
    pub edge: Edge,
    /// How far it has slid out across `edge`, as a share of its length across it: 0 at rest and 1
    /// gone. Below 0 its far side reaches past its place by that share, and the content stretches.
    pub out: f32,
    /// How far the middle of its far side bulges past the side's corners, as a share of its length
    /// (below 0 it caves in): what its speed makes of it.
    pub bow: f32,
}

/// Seconds of motion the far side's middle lags by: the bow is the speed times this.
pub const LAG: f32 = 0.007;
/// The most the far side bows, either way, as a share of the box's length.
pub const MOST_BOW: f32 = 0.06;

impl Elastic {
    /// A box hanging on `edge`, `out` of the way out, moving out at `speed` shares a second.
    pub fn new(from: Rect, edge: Edge, out: f32, speed: f32) -> Elastic {
        Elastic { from, edge, out, bow: bow(speed) }
    }

    /// Its length across the side it hangs on.
    pub fn length(&self) -> f32 {
        if self.edge.is_side() { self.from.w } else { self.from.h }
    }

    /// How much further than at rest its far side can reach: past its place, and bowed out.
    pub fn reach(&self) -> f32 {
        ((-self.out).max(0.0) + self.bow.max(0.0)) * self.length()
    }

    /// Everything a layer of `bounds` can cover bent: those bounds, grown on the far side by what
    /// it can reach.
    pub fn extent(&self, bounds: Rect) -> Rect {
        let (b, reach) = (bounds, self.reach());
        match self.edge {
            Edge::Right => Rect::new(b.x - reach, b.y, b.w + reach, b.h),
            Edge::Left => Rect::new(b.x, b.y, b.w + reach, b.h),
            Edge::Bottom => Rect::new(b.x, b.y - reach, b.w, b.h + reach),
            Edge::Top => Rect::new(b.x, b.y, b.w, b.h + reach),
        }
    }

    /// Whether it draws just as it would at rest.
    pub fn at_rest(&self) -> bool {
        self.out.abs() < 1e-4 && self.bow.abs() < 1e-4
    }

    /// The point of the layer that shows at (`x`, `y`), and how much of the pixel there it covers;
    /// `None` where nothing of it shows.
    pub fn source(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        source(x, y, [self.from.x, self.from.y, self.from.right(), self.from.bottom()], self.edge.code(), self.out, self.bow)
    }
}

/// The bow a speed (shares of the length a second, outward positive) gives.
pub fn bow(speed: f32) -> f32 {
    (speed * LAG).clamp(-MOST_BOW, MOST_BOW)
}

/// (`x`, `y`) in the frame where the box hangs on its right: `u` runs toward that side, `v` along it.
fn canon(x: f32, y: f32, edge: u32) -> (f32, f32) {
    match edge {
        1 => (-x, y),
        2 => (y, x),
        3 => (-y, x),
        _ => (x, y),
    }
}

fn uncanon(u: f32, v: f32, edge: u32) -> (f32, f32) {
    match edge {
        1 => (-u, v),
        2 => (v, u),
        3 => (v, -u),
        _ => (u, v),
    }
}

/// The elastic sums on plain numbers, as the GPU's layer shader does them: which point of a layer
/// whose box at rest is `from` (x0, y0, x1, y1, in the units of `x` and `y`) shows at (`x`, `y`), and
/// how much of a pixel there it covers. `edge` is an [`Edge`]'s [`code`](Edge::code), `out` and `bow`
/// as in [`Elastic`]. A renderer passes device pixels, so the cut at the side it hangs on is a
/// device pixel soft.
pub fn source(x: f32, y: f32, from: [f32; 4], edge: u32, out: f32, bow: f32) -> Option<[f32; 3]> {
    if out >= 1.0 {
        return None;
    }
    let (u, v) = canon(x, y, edge);
    let (a0, b0) = canon(from[0], from[1], edge);
    let (a1, b1) = canon(from[2], from[3], edge);
    let (u0, v0, u1, v1) = (a0.min(a1), b0.min(b1), a0.max(a1), b0.max(b1));
    let (w, h) = (u1 - u0, v1 - v0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    // Cut at the side it hangs on, so sliding out it goes as if under it.
    let cover = (u1 - u + 0.5).clamp(0.0, 1.0);
    if cover <= 0.0 {
        return None;
    }
    // The far side at this row: slid toward the side it hangs on, reaching past its place, bowed.
    let along = ((v - v0) / h).clamp(0.0, 1.0);
    let far = u0 + out.max(0.0) * w - (-out).max(0.0) * w - bow * w * (std::f32::consts::PI * along).sin();
    // Where its near side would be, had it not been cut.
    let near = u1 + out.max(0.0) * w;
    let span = near - far;
    if span <= 1e-3 {
        return None;
    }
    // Between the two the box is stretched to fit; beyond its far side the layer moves with it.
    let su = if u < far { u0 + (u - far) } else { u0 + (u - far) / span * w };
    let (sx, sy) = uncanon(su, v, edge);
    Some([sx, sy, cover])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect;

    /// A tall panel hanging on its right: the AI drawer against the rail.
    fn panel(out: f32, bow: f32) -> Elastic {
        Elastic { from: rect(100.0, 0.0, 300.0, 600.0), edge: Edge::Right, out, bow }
    }

    fn sx(e: &Elastic, x: f32, y: f32) -> f32 {
        e.source(x, y).expect("something shows")[0]
    }

    #[test]
    fn at_rest_every_point_is_itself() {
        let e = panel(0.0, 0.0);
        assert!(e.at_rest());
        for (x, y) in [(150.5, 10.5), (399.5, 599.5), (250.0, 300.0), (80.0, -20.0)] {
            let [sx, sy, cover] = e.source(x, y).unwrap();
            assert!((sx - x).abs() < 1e-3 && (sy - y).abs() < 1e-3 && cover == 1.0, "({x}, {y}) shows ({sx}, {sy}) at {cover}");
        }
        assert!(e.source(400.5, 300.0).is_none(), "nothing past the side it hangs on");
        let [.., half] = e.source(400.0, 300.0).unwrap();
        assert!((half - 0.5).abs() < 1e-3, "a pixel centred on that side is half covered: {half}");
    }

    #[test]
    fn sliding_out_moves_it_across_its_side_where_it_is_cut() {
        let e = panel(0.25, 0.0);
        assert!((sx(&e, 200.0, 300.0) - 125.0).abs() < 1e-3, "it moves, it does not squeeze");
        assert!((sx(&e, 160.0, 300.0) - 85.0).abs() < 1e-3, "and what is beyond its far side, a shadow, moves with it");
        assert!(e.source(399.5, 300.0).is_some() && e.source(400.5, 300.0).is_none(), "nothing shows past the side it hangs on");
        assert!(panel(1.0, 0.0).source(399.0, 300.0).is_none(), "all the way out is gone");
    }

    #[test]
    fn past_its_place_it_stretches_from_the_side_it_hangs_on() {
        let e = panel(-0.1, 0.0);
        assert!((sx(&e, 70.0, 300.0) - 100.0).abs() < 1e-3, "the far side reaches 30 further and the box's far side is drawn there");
        assert!((sx(&e, 60.0, 300.0) - 90.0).abs() < 1e-3, "a shadow beyond it follows");
        assert!((sx(&e, 399.5, 300.0) - 399.5).abs() < 0.1, "while its near side stays put");
        assert!((sx(&e, 235.0, 300.0) - 250.0).abs() < 1e-3, "the middle stretched between");
        assert_eq!(e.extent(rect(60.0, -40.0, 380.0, 680.0)), rect(30.0, -40.0, 410.0, 680.0));
    }

    #[test]
    fn the_far_side_bows_with_the_speed_and_its_corners_do_not() {
        // Coming in fast: the middle lags, so the far side caves in.
        let coming = Elastic::new(rect(100.0, 0.0, 300.0, 600.0), Edge::Right, 0.0, -6.0);
        assert!(coming.bow < 0.0);
        let cave = -coming.bow * 300.0;
        assert!((sx(&coming, 100.0 + cave, 300.0) - 100.0).abs() < 1e-3, "the middle of the far side is {cave} in");
        assert!((sx(&coming, 100.0, 0.0) - 100.0).abs() < 1e-3, "a corner is where it was");
        assert!(sx(&coming, 101.0, 300.0) < 100.0, "beside the middle the layer shows what is beyond its far side");
        // Settling back the other way, fast: it bulges, never more than the most.
        let back = Elastic::new(rect(100.0, 0.0, 300.0, 600.0), Edge::Right, -0.05, 100.0);
        assert_eq!(back.bow, MOST_BOW);
        assert!((sx(&back, 100.0 - 15.0 - MOST_BOW * 300.0, 300.0) - 100.0).abs() < 1e-2);
        assert!((back.reach() - (15.0 + MOST_BOW * 300.0)).abs() < 1e-3);
    }

    #[test]
    fn every_side_works_the_same_way() {
        let r = rect(0.0, 0.0, 100.0, 200.0);
        // Hanging on its left and slid half out: its right half shows at its left.
        let left = Elastic { from: r, edge: Edge::Left, out: 0.5, bow: 0.0 };
        assert!((sx(&left, 25.0, 100.0) - 75.0).abs() < 1e-3);
        assert!(left.source(-1.0, 100.0).is_none(), "cut at its left");
        // Hanging from the top (a sheet), past its place: it reaches further down.
        let top = Elastic { from: r, edge: Edge::Top, out: -0.1, bow: 0.0 };
        let [_, y, _] = top.source(50.0, 220.0).unwrap();
        assert!((y - 200.0).abs() < 1e-3, "{y}");
        assert_eq!(top.extent(r), rect(0.0, 0.0, 100.0, 220.0));
        let bottom = Elastic { from: r, edge: Edge::Bottom, out: 0.5, bow: 0.0 };
        let [_, y, _] = bottom.source(50.0, 150.0).unwrap();
        assert!((y - 50.0).abs() < 1e-3, "{y}");
        assert!(bottom.source(50.0, 201.0).is_none());
    }
}
