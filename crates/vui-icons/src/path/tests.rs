use super::*;

fn pts(d: &str) -> Vec<Seg> {
    parse(d).unwrap_or_else(|e| panic!("{d:?}: {e}"))
}

fn near(a: P, b: P) -> bool {
    (a - b).len() < 1e-3
}

#[test]
fn absolute_and_relative_lines() {
    assert_eq!(pts("M1 2L3 4"), [Seg::Move(p(1.0, 2.0)), Seg::Line(p(3.0, 4.0))]);
    assert_eq!(pts("m1 2l3 4"), [Seg::Move(p(1.0, 2.0)), Seg::Line(p(4.0, 6.0))]);
    assert_eq!(
        pts("M1 1H5V4h-2v-1"),
        [Seg::Move(p(1.0, 1.0)), Seg::Line(p(5.0, 1.0)), Seg::Line(p(5.0, 4.0)), Seg::Line(p(3.0, 4.0)), Seg::Line(p(3.0, 3.0))]
    );
}

#[test]
fn numbers_come_in_every_spelling() {
    let a = pts("M.5.5L-1.5-2.5 +3,4e0 1E1 -1e-1");
    assert_eq!(a[0], Seg::Move(p(0.5, 0.5)));
    assert_eq!(a[1], Seg::Line(p(-1.5, -2.5)));
    assert_eq!(a[2], Seg::Line(p(3.0, 4.0)), "an implicit lineto repeats");
    assert_eq!(a[3], Seg::Line(p(10.0, -0.1)));
    assert_eq!(pts("M1,2,3,4"), [Seg::Move(p(1.0, 2.0)), Seg::Line(p(3.0, 4.0))], "numbers after a moveto are linetos");
    assert_eq!(pts("m1 2 3 4")[1], Seg::Line(p(4.0, 6.0)), "and relative ones stay relative");
    assert_eq!(pts("M1 1 5.5.5")[1], Seg::Line(p(5.5, 0.5)), "a second dot starts a new number");
}

#[test]
fn closing_returns_to_the_start_and_a_later_command_begins_again_there() {
    let a = pts("M2 2h4v4zl1 1");
    assert_eq!(a[3], Seg::Close);
    assert_eq!(a[4], Seg::Move(p(2.0, 2.0)));
    assert_eq!(a[5], Seg::Line(p(3.0, 3.0)));
    assert!(matches!(pts("M0 0 z Z").as_slice(), [Seg::Move(_), Seg::Close]), "closing twice closes once");
}

#[test]
fn smooth_curves_reflect_the_previous_control_point() {
    let a = pts("M0 0C0 4 4 4 4 0S8-4 8 0");
    assert_eq!(a[2], Seg::Cubic(p(4.0, -4.0), p(8.0, -4.0), p(8.0, 0.0)));
    let b = pts("M0 0Q2 4 4 0T8 0");
    assert_eq!(b[2], Seg::Quad(p(6.0, -4.0), p(8.0, 0.0)));
    // Without a curve before it, the reflected point is the current point.
    let c = pts("M0 0S2 2 4 0");
    assert_eq!(c[1], Seg::Cubic(p(0.0, 0.0), p(2.0, 2.0), p(4.0, 0.0)));
}

#[test]
fn mistakes_say_where() {
    assert_eq!(parse("L1 1").unwrap_err().message, "a path starts with a moveto");
    assert_eq!(parse("M1").unwrap_err().message, "expected a number");
    assert_eq!(parse("M1 1 X").unwrap_err().message, "unknown command");
    assert_eq!(parse("M1 1 z 2 2").unwrap_err().message, "expected a command");
    assert_eq!(parse("M0 0a1 1 0 2 0 1 1").unwrap_err().message, "expected an arc flag, 0 or 1");
    assert_eq!(parse("").unwrap(), []);
    assert_eq!(parse("M1 é").unwrap_err().message, "expected a number", "stray non-ASCII bytes are an error, not a panic");
}

/// Distance of every flattened point of `d` from the point `c`.
fn radii(d: &str, c: P, tol: f32) -> Vec<f32> {
    flatten(&pts(d), tol).iter().flat_map(|poly| poly.pts.iter().map(|q| (*q - c).len())).collect()
}

#[test]
fn arcs_stay_on_their_circle() {
    // Two half circles make a circle of radius 8 around (12, 12).
    for d in ["M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0-16z", "M12 4A8 8 0 0 1 12 20A8 8 0 0 1 12 4z", "M20 12a8 8 0 0 1-8 8 8 8 0 0 1-8-8 8 8 0 0 1 8-8 8 8 0 0 1 8 8z"] {
        let r = radii(d, p(12.0, 12.0), 0.01);
        assert!(r.len() > 30, "{d}: {} points", r.len());
        for r in r {
            assert!((r - 8.0).abs() < 0.02, "{d}: a point is {r} from the centre");
        }
    }
}

#[test]
fn arc_flags_choose_between_the_four_arcs() {
    // From (0,0) to (8,0) with radius 5: the centre is above or below, and the arc is short or long.
    let mid_y = |d: &str| {
        let poly = &flatten(&pts(d), 0.01)[0];
        poly.pts.iter().map(|q| q.y).fold(0.0f32, |a, b| if b.abs() > a.abs() { b } else { a })
    };
    assert!((mid_y("M0 0A5 5 0 0 0 8 0") - 2.0).abs() < 0.05, "short, counter-clockwise: bulges down");
    assert!((mid_y("M0 0A5 5 0 0 1 8 0") + 2.0).abs() < 0.05, "short, clockwise: bulges up");
    assert!((mid_y("M0 0A5 5 0 1 0 8 0") - 8.0).abs() < 0.05, "long, counter-clockwise: bulges down");
    assert!((mid_y("M0 0A5 5 0 1 1 8 0") + 8.0).abs() < 0.05, "long, clockwise: bulges up");
}

#[test]
fn radii_that_are_too_small_grow_until_the_arc_fits() {
    let poly = &flatten(&pts("M0 0A1 1 0 0 1 10 0"), 0.01)[0];
    let top = poly.pts.iter().map(|q| q.y).fold(0.0f32, f32::min);
    assert!((top + 5.0).abs() < 0.05, "a half circle of radius 5, got height {top}");
    assert!(poly.pts.last().is_some_and(|q| near(*q, p(10.0, 0.0))), "and it ends where it should");
}

#[test]
fn an_arc_ends_exactly_where_it_was_told_to() {
    for (d, end) in [
        ("M2 3a6 3 30 0 1 8 4", p(10.0, 7.0)),
        ("M2 3A6 3 -45 1 0 8 4", p(8.0, 4.0)),
        ("M2 3a0 3 0 0 1 4 4", p(6.0, 7.0)),
        ("M2 3a3 0 0 0 1 4 4", p(6.0, 7.0)),
    ] {
        let got = flatten(&pts(d), 0.01)[0].pts.last().copied().unwrap();
        assert!(near(got, end), "{d}: ends at {got:?}");
    }
}

#[test]
fn a_zero_radius_arc_is_a_line() {
    assert_eq!(pts("M0 0a0 5 0 0 1 4 4")[1], Seg::Line(p(4.0, 4.0)));
}

#[test]
fn a_curve_flattens_to_within_the_tolerance_and_finer_tolerance_makes_more_points() {
    let segs = pts("M0 0C0 10 10 10 10 0");
    let coarse = flatten(&segs, 0.5);
    let fine = flatten(&segs, 0.01);
    assert!(fine[0].pts.len() > coarse[0].pts.len() * 2, "{} vs {}", fine[0].pts.len(), coarse[0].pts.len());
    // The curve's apex is at (5, 7.5); the flattened path must come within tolerance of it.
    let apex = fine[0].pts.iter().map(|q| q.y).fold(0.0f32, f32::max);
    assert!((apex - 7.5).abs() < 0.02, "apex {apex}");
    assert!(coarse[0].pts.len() >= 4);
}

#[test]
fn a_straight_curve_is_one_segment() {
    let poly = &flatten(&pts("M0 0C1 1 2 2 3 3"), 0.01)[0];
    assert_eq!(poly.pts.len(), 2);
}

#[test]
fn a_closed_loop_with_coincident_ends_does_not_recurse_forever() {
    let poly = &flatten(&pts("M0 0C10 0 10 10 0 0"), 0.01)[0];
    assert!(poly.pts.len() > 8 && poly.pts.len() < 5000, "{}", poly.pts.len());
}

#[test]
fn flattening_groups_points_by_subpath() {
    let polys = flatten(&pts("M0 0h4v4zM10 10l2 2M20 20"), 0.1);
    assert_eq!(polys.len(), 2, "a lone moveto at the end draws nothing");
    assert!(polys[0].closed && polys[0].pts.len() == 3);
    assert!(!polys[1].closed && polys[1].pts.len() == 2);
    assert_eq!(polys[0].edges().count(), 3, "the closing edge counts");
    assert_eq!(polys[1].edges().count(), 1);
}

#[test]
fn a_dot_is_a_zero_length_segment() {
    let polys = flatten(&pts("M5 5z"), 0.1);
    assert_eq!(polys.len(), 1);
    let edges: Vec<_> = polys[0].edges().collect();
    assert_eq!(edges, [(p(5.0, 5.0), p(5.0, 5.0))]);
    assert_eq!(flatten(&pts("M5 5h0"), 0.1)[0].pts.len(), 2, "a zero-length line is kept");
}

#[test]
fn scaling_moves_every_point() {
    let a = Seg::Cubic(p(1.0, 2.0), p(3.0, 4.0), p(5.0, 6.0)).scaled(2.0);
    assert_eq!(a, Seg::Cubic(p(2.0, 4.0), p(6.0, 8.0), p(10.0, 12.0)));
}
