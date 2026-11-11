use super::*;

fn ramp(n: usize) -> Vec<(f64, f64)> {
    (0..n).map(|i| (i as f64, 100.0 + i as f64)).collect()
}

#[test]
fn bad_numbers_are_dropped_and_points_come_out_in_order() {
    let c = Chart::new().line(
        "a",
        &[
            (2.0, 1.0),
            (f64::NAN, 3.0),
            (0.0, 5.0),
            (1.0, f64::INFINITY),
        ],
    );
    match &c.layers[0].mark {
        Mark::Line { points, .. } => assert_eq!(points, &[(0.0, 5.0), (2.0, 1.0)]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn x_extent_gives_bars_half_a_bar_at_each_end() {
    let line = Chart::new().line("a", &ramp(11));
    assert_eq!(line.x_extent(), (0.0, 10.0));
    let bars = Chart::new().bars("a", &ramp(11));
    assert_eq!(bars.x_extent(), (-0.5, 10.5));
    assert_eq!(Chart::new().x_extent(), (0.0, 1.0));
}

#[test]
fn y_extent_anchors_bars_at_zero_and_floats_lines() {
    let (lo, hi) = Chart::new()
        .bars("a", &ramp(10))
        .y_extent(Panel::Main, (0.0, 9.0), 5);
    assert_eq!(lo, 0.0);
    assert!(hi >= 109.0);
    let (lo, hi) = Chart::new()
        .line("a", &ramp(10))
        .y_extent(Panel::Main, (0.0, 9.0), 5);
    assert!(
        lo > 50.0 && lo <= 100.0,
        "a line far from zero is not stretched down to it: {lo}"
    );
    assert!(hi >= 109.0);
    let (lo, _) =
        Chart::new()
            .line("a", &ramp(10))
            .from_zero()
            .y_extent(Panel::Main, (0.0, 9.0), 5);
    assert_eq!(lo, 0.0);
}

#[test]
fn y_extent_follows_the_visible_window_and_a_flat_series_gets_room() {
    let mut pts = ramp(100);
    pts[90].1 = 1000.0;
    let c = Chart::new().line("a", &pts);
    let (_, hi_all) = c.y_extent(Panel::Main, (0.0, 99.0), 5);
    let (_, hi_early) = c.y_extent(Panel::Main, (0.0, 20.0), 5);
    assert!(hi_all >= 1000.0 && hi_early < 200.0, "{hi_all} {hi_early}");
    let (lo, hi) = Chart::new()
        .line("flat", &[(0.0, 5.0), (1.0, 5.0)])
        .y_extent(Panel::Main, (0.0, 1.0), 5);
    assert!(lo < 5.0 && hi > 5.0);
}

#[test]
fn panels_have_separate_ranges() {
    let c = Chart::new()
        .line("price", &ramp(10))
        .volume("vol", &[(0.0, 5_000_000.0), (9.0, 8_000_000.0)]);
    assert!(c.has_strip());
    let (_, main_hi) = c.y_extent(Panel::Main, (0.0, 9.0), 5);
    let (_, strip_hi) = c.y_extent(Panel::Strip, (0.0, 9.0), 3);
    assert!(main_hi < 200.0 && strip_hi >= 8_000_000.0);
}

#[test]
fn within_keeps_one_neighbour_either_side() {
    let pts = ramp(10);
    let seen = within(&pts, |p| p.0, (3.5, 5.5));
    assert_eq!(seen.first().unwrap().0, 3.0);
    assert_eq!(seen.last().unwrap().0, 6.0);
    assert_eq!(
        within(&pts, |p| p.0, (100.0, 200.0)).len(),
        1,
        "the last point, so a line can enter from the edge"
    );
    assert!(within::<(f64, f64)>(&[], |p| p.0, (0.0, 1.0)).is_empty());
}

#[test]
fn candles_span_high_to_low() {
    let rows = [
        Candle {
            x: 0.0,
            open: 10.0,
            high: 14.0,
            low: 8.0,
            close: 12.0,
        },
        Candle {
            x: 1.0,
            open: 12.0,
            high: 13.0,
            low: 5.0,
            close: 6.0,
        },
    ];
    let c = Chart::new().candles("px", &rows);
    let (lo, hi) = c.y_extent(Panel::Main, (0.0, 1.0), 5);
    assert!(lo <= 5.0 && hi >= 14.0);
    assert_eq!(c.x_extent(), (-0.5, 1.5));
}
