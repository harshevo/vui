//! Turning outlines into coverage. Every pixel asks how far its centre is from the shape: a stroke is
//! everything within half its width of a segment, a fill is everything inside, and the one-pixel ramp
//! at the edge is the anti-aliasing. Icons are tiny and cached, so exact beats fast here.

use vui_core::ImageData;

use crate::path::{P, Poly, p};

/// Coverage of a square, one value per pixel: 0 is empty and 255 is solid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

impl Mask {
    pub fn get(&self, x: u32, y: u32) -> u8 {
        self.alpha[(y * self.width + x) as usize]
    }

    /// How much ink there is, in pixels: the coverage summed and scaled to 0..1.
    pub fn ink(&self) -> f32 {
        self.alpha.iter().map(|&a| a as f32).sum::<f32>() / 255.0
    }

    /// White pixels carrying the coverage as alpha, which is what `ImageMode::Mask` paints in a tint.
    pub fn to_image(&self) -> ImageData {
        let w = self.width as usize;
        ImageData::from_fn(self.width, self.height, |x, y| [255, 255, 255, self.alpha[y as usize * w + x as usize]])
    }
}

pub(crate) struct Canvas {
    w: usize,
    h: usize,
    cov: Vec<f32>,
}

/// How much of the pixel a band `hw` either side of a line covers, for a pixel centre `d` from the line.
fn band(d: f32, hw: f32) -> f32 {
    ((d + 0.5).min(hw) - (d - 0.5).max(-hw)).clamp(0.0, 1.0)
}

fn distance(c: P, a: P, b: P) -> f32 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 == 0.0 { 0.0 } else { ((c - a).dot(ab) / len2).clamp(0.0, 1.0) };
    (c - (a + ab * t)).len()
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, cov: vec![0.0; w * h] }
    }

    fn put(&mut self, i: usize, v: f32) {
        if v > self.cov[i] {
            self.cov[i] = v;
        }
    }

    /// Strokes the outlines with round caps and joins, `alpha` strong.
    pub fn stroke(&mut self, polys: &[Poly], width: f32, alpha: f32) {
        let hw = width * 0.5;
        // Past hw + 0.5 pixels from a segment it contributes nothing.
        let reach = hw + 1.0;
        let mut nearest = vec![f32::INFINITY; self.w * self.h];
        for (a, b) in polys.iter().flat_map(Poly::edges) {
            let lo = |v: f32, max: usize| (v - reach).floor().clamp(0.0, max as f32) as usize;
            let hi = |v: f32, max: usize| (v + reach).ceil().clamp(0.0, max as f32) as usize;
            let (x0, x1) = (lo(a.x.min(b.x), self.w), hi(a.x.max(b.x), self.w));
            let (y0, y1) = (lo(a.y.min(b.y), self.h), hi(a.y.max(b.y), self.h));
            for y in y0..y1 {
                for x in x0..x1 {
                    let d = distance(p(x as f32 + 0.5, y as f32 + 0.5), a, b);
                    let slot = &mut nearest[y * self.w + x];
                    if d < *slot {
                        *slot = d;
                    }
                }
            }
        }
        for (i, d) in nearest.into_iter().enumerate() {
            if d.is_finite() {
                self.put(i, band(d, hw) * alpha);
            }
        }
    }

    /// Fills the insides of the outlines (nonzero winding), `alpha` strong. Open outlines are closed.
    pub fn fill(&mut self, polys: &[Poly], alpha: f32) {
        let edges: Vec<(P, P)> = polys
            .iter()
            .filter(|poly| poly.pts.len() > 1)
            .flat_map(|poly| (0..poly.pts.len()).map(move |i| (poly.pts[i], poly.pts[(i + 1) % poly.pts.len()])))
            .collect();
        if edges.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (p(f32::MAX, f32::MAX), p(f32::MIN, f32::MIN));
        for (a, _) in &edges {
            lo = p(lo.x.min(a.x), lo.y.min(a.y));
            hi = p(hi.x.max(a.x), hi.y.max(a.y));
        }
        let x0 = (lo.x - 1.0).floor().clamp(0.0, self.w as f32) as usize;
        let x1 = (hi.x + 1.0).ceil().clamp(0.0, self.w as f32) as usize;
        let y0 = (lo.y - 1.0).floor().clamp(0.0, self.h as f32) as usize;
        let y1 = (hi.y + 1.0).ceil().clamp(0.0, self.h as f32) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let c = p(x as f32 + 0.5, y as f32 + 0.5);
                let mut winding = 0i32;
                let mut nearest = f32::INFINITY;
                for &(a, b) in &edges {
                    nearest = nearest.min(distance(c, a, b));
                    let side = (b - a).cross(c - a);
                    if a.y <= c.y {
                        if b.y > c.y && side > 0.0 {
                            winding += 1;
                        }
                    } else if b.y <= c.y && side < 0.0 {
                        winding -= 1;
                    }
                }
                let signed = if winding != 0 { -nearest } else { nearest };
                self.put(y * self.w + x, (0.5 - signed).clamp(0.0, 1.0) * alpha);
            }
        }
    }

    pub fn finish(self) -> Mask {
        Mask {
            width: self.w as u32,
            height: self.h as u32,
            alpha: self.cov.iter().map(|&c| (c * 255.0 + 0.5) as u8).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{flatten, parse};

    fn polys(d: &str) -> Vec<Poly> {
        flatten(&parse(d).unwrap(), 0.02)
    }

    fn stroked(d: &str, size: usize, width: f32) -> Mask {
        let mut c = Canvas::new(size, size);
        c.stroke(&polys(d), width, 1.0);
        c.finish()
    }

    fn filled(d: &str, size: usize) -> Mask {
        let mut c = Canvas::new(size, size);
        c.fill(&polys(d), 1.0);
        c.finish()
    }

    #[test]
    fn a_two_pixel_line_on_a_pixel_boundary_covers_two_rows_fully() {
        let m = stroked("M0 8H16", 16, 2.0);
        for x in 0..16 {
            assert_eq!(m.get(x, 6), 0, "above");
            assert_eq!(m.get(x, 7), 255);
            assert_eq!(m.get(x, 8), 255);
            assert_eq!(m.get(x, 9), 0, "below");
        }
    }

    #[test]
    fn a_thin_line_covers_exactly_as_much_as_its_width() {
        // A 0.5 px line through the middle of row 4 covers half of that row and nothing else.
        let m = stroked("M0 4.5H16", 16, 0.5);
        assert_eq!(m.get(8, 3), 0);
        assert!((m.get(8, 4) as i32 - 128).abs() <= 1, "{}", m.get(8, 4));
        assert_eq!(m.get(8, 5), 0);
        // A 1 px line straddling two rows splits evenly between them.
        let m = stroked("M0 5H16", 16, 1.0);
        assert!((m.get(8, 4) as i32 - 128).abs() <= 1);
        assert!((m.get(8, 5) as i32 - 128).abs() <= 1);
    }

    #[test]
    fn caps_are_round() {
        // A stroke 10 px wide ending at x = 20 reaches 5 px further, as a half circle.
        let m = stroked("M5 16H20", 32, 10.0);
        assert_eq!(m.get(23, 16), 255, "within the cap");
        assert_eq!(m.get(26, 16), 0, "past the cap");
        assert_eq!(m.get(24, 11), 0, "a square cap would cover this corner; a round one does not");
        assert!(m.get(23, 12) > 0 && m.get(23, 12) < 255, "the round edge is anti-aliased: {}", m.get(23, 12));
    }

    #[test]
    fn joins_are_round() {
        // Two strokes meeting at a right angle: the outside of the corner is a quarter disc.
        let m = stroked("M4 28V16H16", 32, 8.0);
        assert_eq!(m.get(0, 12), 0, "a mitred corner would be solid here");
        assert_eq!(m.get(2, 14), 255, "inside the arc");
        assert!(m.get(1, 13) > 200 && m.get(1, 13) < 255, "on the arc: {}", m.get(1, 13));
    }

    #[test]
    fn a_diagonal_line_has_a_smooth_edge() {
        let m = stroked("M2 2L30 30", 32, 3.0);
        let grays = m.alpha.iter().filter(|&&a| a > 0 && a < 255).count();
        assert!(grays > 40, "anti-aliased edge pixels: {grays}");
        // Symmetric about the diagonal.
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(m.get(x, y), m.get(y, x));
            }
        }
    }

    #[test]
    fn a_stroked_circle_has_the_ink_of_its_circumference_times_its_width() {
        let m = stroked("M16 6a10 10 0 1 0 0 20a10 10 0 1 0 0-20z", 32, 2.0);
        let want = std::f32::consts::TAU * 10.0 * 2.0;
        assert!((m.ink() - want).abs() / want < 0.02, "ink {} vs {want}", m.ink());
        assert_eq!(m.get(16, 16), 0, "hollow");
    }

    #[test]
    fn a_filled_rectangle_is_solid_with_exact_edges() {
        let m = filled("M4 4h8v8H4z", 16);
        assert_eq!(m.get(7, 7), 255);
        assert_eq!(m.get(4, 4), 255, "the corner pixel is inside");
        assert_eq!(m.get(3, 4), 0);
        assert_eq!(m.get(12, 4), 0);
        assert!((m.ink() - 64.0).abs() < 0.5, "{}", m.ink());
    }

    #[test]
    fn a_fill_with_fractional_edges_gets_fractional_coverage() {
        let m = filled("M4.5 4h8v8h-8z", 16);
        assert!((m.get(4, 8) as i32 - 128).abs() <= 1, "{}", m.get(4, 8));
        assert_eq!(m.get(5, 8), 255);
        assert!((m.ink() - 64.0).abs() < 0.5);
    }

    #[test]
    fn a_filled_circle_has_the_area_of_a_circle() {
        let m = filled("M16 6a10 10 0 1 0 0 20a10 10 0 1 0 0-20z", 32);
        let want = std::f32::consts::PI * 100.0;
        assert!((m.ink() - want).abs() / want < 0.01, "ink {} vs {want}", m.ink());
    }

    #[test]
    fn overlapping_and_reversed_outlines_still_fill() {
        let a = filled("M4 4h8v8H4z", 16);
        let reversed = filled("M4 4v8h8V4z", 16);
        assert_eq!(a, reversed, "winding direction does not matter for a single outline");
        let twice = filled("M4 4h8v8H4zM4 4h8v8H4z", 16);
        assert_eq!(a, twice, "a second copy on top changes nothing");
    }

    #[test]
    fn a_ring_made_of_two_outlines_keeps_its_hole() {
        // Opposite winding makes the inner square a hole.
        let m = filled("M2 2h12v12H2zM5 5v6h6V5z", 16);
        assert_eq!(m.get(3, 8), 255);
        assert_eq!(m.get(8, 8), 0, "the hole");
    }

    #[test]
    fn a_dot_is_a_disc() {
        let m = stroked("M8 8h0", 16, 6.0);
        assert!(m.get(8, 8) == 255 && m.get(7, 7) == 255);
        assert_eq!(m.get(8, 3), 0);
        let want = std::f32::consts::PI * 9.0;
        assert!((m.ink() - want).abs() / want < 0.03, "{}", m.ink());
    }

    #[test]
    fn layers_combine_by_the_stronger_coverage() {
        let mut c = Canvas::new(16, 16);
        c.stroke(&polys("M0 8H16"), 2.0, 0.5);
        c.stroke(&polys("M8 0V16"), 2.0, 1.0);
        let m = c.finish();
        assert_eq!(m.get(2, 8), 128, "the half-strength line alone");
        assert_eq!(m.get(8, 2), 255);
        assert_eq!(m.get(8, 8), 255, "where they cross the stronger one wins");
    }

    #[test]
    fn drawing_outside_the_canvas_is_harmless() {
        let m = stroked("M-50 -50L80 80M100 100h5", 16, 2.0);
        assert!(m.ink() > 10.0);
        let m = filled("M-10 -10h100v100h-100z", 8);
        assert!(m.alpha.iter().all(|&a| a == 255));
    }

    #[test]
    fn an_image_carries_the_coverage_as_white_with_alpha() {
        let m = stroked("M0 8H16", 16, 2.0);
        let img = m.to_image();
        assert_eq!((img.width, img.height), (16, 16));
        assert_eq!(img.pixel(3, 7), [255, 255, 255, 255]);
        assert_eq!(img.pixel(3, 2)[3], 0);
    }
}
