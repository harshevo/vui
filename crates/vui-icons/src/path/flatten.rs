//! Flattening: curves and arcs into polylines, one per subpath, within a tolerance.

use super::*;

/// A run of straight segments, closed or open.
#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    pub pts: Vec<P>,
    pub closed: bool,
}

impl Poly {
    /// The line segments, including the one that closes a closed outline. A single point gives one
    /// zero-length segment, which a round-capped stroke draws as a dot.
    pub fn edges(&self) -> impl Iterator<Item = (P, P)> + '_ {
        let n = self.pts.len();
        let count = if n == 1 { 1 } else if self.closed { n } else { n.saturating_sub(1) };
        (0..count).map(move |i| (self.pts[i], self.pts[(i + 1) % n]))
    }
}

/// Turns curves into short lines that stay within `tol` of the true curve.
pub fn flatten(segs: &[Seg], tol: f32) -> Vec<Poly> {
    let mut out = Vec::new();
    let mut cur: Option<Poly> = None;
    let mut last = P::default();
    for &seg in segs {
        match seg {
            Seg::Move(a) => {
                out.extend(cur.take());
                cur = Some(Poly { pts: vec![a], closed: false });
                last = a;
            }
            Seg::Close => {
                if let Some(mut poly) = cur.take() {
                    poly.closed = true;
                    last = poly.pts[0];
                    out.push(poly);
                }
            }
            drawing => {
                let poly = cur.get_or_insert_with(|| Poly { pts: vec![last], closed: false });
                match drawing {
                    Seg::Line(a) => poly.pts.push(a),
                    Seg::Quad(c, a) => {
                        let (c1, c2) = (last + (c - last) * (2.0 / 3.0), a + (c - a) * (2.0 / 3.0));
                        cubic(last, c1, c2, a, tol, 0, &mut poly.pts);
                    }
                    Seg::Cubic(c1, c2, a) => cubic(last, c1, c2, a, tol, 0, &mut poly.pts),
                    Seg::Move(_) | Seg::Close => unreachable!("handled above"),
                }
                last = *poly.pts.last().expect("a polyline has a point");
            }
        }
    }
    out.extend(cur);
    // A lone moveto draws nothing.
    out.retain(|poly| poly.pts.len() > 1 || poly.closed);
    out
}

pub(crate) fn cubic(p0: P, p1: P, p2: P, p3: P, tol: f32, depth: u32, out: &mut Vec<P>) {
    let chord = p3 - p0;
    let len = chord.len();
    let (d1, d2) = if len < 1e-6 {
        ((p1 - p0).len(), (p2 - p0).len())
    } else {
        ((p1 - p0).cross(chord).abs() / len, (p2 - p0).cross(chord).abs() / len)
    };
    if depth >= 16 || d1.max(d2) <= tol {
        out.push(p3);
        return;
    }
    let (p01, p12, p23) = (p0.mid(p1), p1.mid(p2), p2.mid(p3));
    let (p012, p123) = (p01.mid(p12), p12.mid(p23));
    let m = p012.mid(p123);
    cubic(p0, p01, p012, m, tol, depth + 1, out);
    cubic(m, p123, p23, p3, tol, depth + 1, out);
}
