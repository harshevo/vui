use super::*;
use vui_core::{Command, Fonts, Theme};

fn setup() -> (Fonts, VizStyle) {
    (
        Fonts::bundled_only(),
        VizStyle::from_palette(&Theme::light().palette),
    )
}

fn days(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            (
                1_700_000_000.0 + i as f64 * 86_400.0,
                100.0 + (i as f64 / 5.0).sin() * 10.0,
            )
        })
        .collect()
}

#[test]
fn layout_leaves_room_for_labels_and_splits_off_the_strip() {
    let (fonts, style) = setup();
    let chart = Chart::new()
        .time()
        .line("px", &days(60))
        .volume("vol", &days(60));
    let view = chart.fit(chart.x_extent());
    let area = rect(0.0, 0.0, 600.0, 320.0);
    let d = layout(&fonts, &chart, area, &view, &style);
    let strip = d.strip.expect("a volume layer makes a strip");
    assert!(d.plot.x >= 28.0 && d.plot.right() <= area.right() - 10.0);
    assert!(d.main.bottom() + STRIP_GAP <= strip.y + 0.01 && strip.bottom() == d.plot.bottom());
    assert!(strip.h > 40.0 && strip.h < d.main.h / 2.0);
    assert_eq!(d.x.map(view.x.0), d.plot.x);
    assert_eq!(d.y.map(view.y.1), d.main.y);
    let small = layout(&fonts, &chart, rect(0.0, 0.0, 600.0, 120.0), &view, &style);
    assert!(
        small.strip.is_none(),
        "a short chart drops the strip rather than squash it"
    );
}

#[test]
fn drawing_a_chart_puts_axes_before_marks_and_marks_inside_the_plot() {
    let (fonts, style) = setup();
    let chart = Chart::new()
        .time()
        .line("px", &days(60))
        .area("floor", &days(60))
        .bars("n", &days(60))
        .volume("vol", &days(60));
    let view = chart.fit(chart.x_extent());
    let mut canvas = Canvas::new(&fonts, 2.0);
    let d = draw(
        &mut canvas,
        &chart,
        rect(0.0, 0.0, 600.0, 320.0),
        &view,
        &style,
        1.0,
    );
    let items = canvas.finish();
    assert!(matches!(items[0], Command::Quad(_)), "gridlines first");
    let meshes: Vec<_> = items
        .iter()
        .filter_map(|i| {
            if let Command::Mesh(m) = i {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    assert!(meshes.len() >= 2, "line and area");
    for m in meshes {
        let clip = m.clip.expect("marks are clipped to their panel");
        assert!(clip.x >= d.plot.x - 0.01 && clip.right() <= d.plot.right() + 0.01);
    }
    let texts = items
        .iter()
        .filter(|i| matches!(i, Command::Text(_)))
        .count();
    assert!(texts >= 5, "y labels and dates: {texts}");
}

#[test]
fn the_reveal_uncovers_marks_from_the_left() {
    let (fonts, style) = setup();
    let chart = Chart::new().line("px", &days(30));
    let view = chart.fit(chart.x_extent());
    let widths: Vec<f32> = [0.0, 0.5, 1.0]
        .iter()
        .map(|r| {
            let mut c = Canvas::new(&fonts, 1.0);
            draw(
                &mut c,
                &chart,
                rect(0.0, 0.0, 400.0, 200.0),
                &view,
                &style,
                *r,
            );
            c.finish()
                .iter()
                .find_map(|i| {
                    if let Command::Mesh(m) = i {
                        m.clip.map(|c| c.w)
                    } else {
                        None
                    }
                })
                .unwrap()
        })
        .collect();
    assert!(widths[0] < 0.01 && widths[1] > widths[0] && widths[2] > widths[1]);
}

#[test]
fn candles_and_dots_draw_one_shape_per_item() {
    let (fonts, style) = setup();
    let rows: Vec<Candle> = (0..20)
        .map(|i| Candle {
            x: f64::from(i),
            open: 10.0 + f64::from(i),
            high: 14.0 + f64::from(i),
            low: 8.0 + f64::from(i),
            close: 12.0 + f64::from(i % 3),
        })
        .collect();
    let dots: Vec<Dot> = (0..8)
        .map(|i| Dot {
            x: f64::from(i),
            y: f64::from(i) * 2.0,
            size: f64::from(i + 1),
        })
        .collect();
    let chart = Chart::new().candles("px", &rows).dots("pts", &dots);
    let view = chart.fit(chart.x_extent());
    let mut c = Canvas::new(&fonts, 1.0);
    draw(
        &mut c,
        &chart,
        rect(0.0, 0.0, 500.0, 260.0),
        &view,
        &style,
        1.0,
    );
    let quads = c
        .finish()
        .iter()
        .filter(|i| matches!(i, Command::Quad(_)))
        .count();
    assert!(
        quads >= 20 * 2 + 8,
        "wick and body per candle plus a dot each: {quads}"
    );
}

#[test]
fn bars_too_dense_to_tell_apart_are_thinned_once_and_drawn() {
    let (fonts, style) = setup();
    let volume: Vec<(f64, f64)> = (0..2_000)
        .map(|i| (f64::from(i), f64::from(i % 50)))
        .collect();
    let chart = Chart::new().bars("vol", &volume);
    let view = chart.fit(chart.x_extent());
    let mut c = Canvas::new(&fonts, 1.0);
    draw(
        &mut c,
        &chart,
        rect(0.0, 0.0, 300.0, 200.0),
        &view,
        &style,
        1.0,
    );
    let quads = c
        .finish()
        .iter()
        .filter(|i| matches!(i, Command::Quad(_)))
        .count();
    assert!(
        quads > 0 && quads < 2_000,
        "a bar per kept extreme, not one per point: {quads}"
    );
}

fn drawn_text(chart: &Chart, w: f32, h: f32) -> Vec<String> {
    let (fonts, style) = setup();
    let view = chart.fit(chart.x_extent());
    let mut canvas = Canvas::new(&fonts, 1.0);
    draw(&mut canvas, chart, rect(0.0, 0.0, w, h), &view, &style, 1.0);
    canvas
        .finish()
        .iter()
        .filter_map(|c| {
            if let Command::Text(t) = c {
                Some(t.layout.text().to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn percent_stacks_each_x_to_a_whole() {
    let c = Chart::new()
        .bars("Promoters", &[(0.0, 50.0), (1.0, 30.0), (2.0, 0.0)])
        .bars("Public", &[(0.0, 50.0), (1.0, 70.0), (2.0, 0.0)])
        .bars("Odd", &[(0.0, -5.0)])
        .percent();
    assert_eq!(
        c.y_extent(Panel::Main, (0.0, 2.0), 5),
        (0.0, 1.0),
        "always the whole, never 120%"
    );
    assert_eq!(c.bar_layout, crate::chart::BarLayout::Stacked);
    let at = |layer: usize, x: usize| match &c.layers[layer].mark {
        Mark::Bars { points, .. } => points.iter().find(|p| p.0 == x as f64).map(|p| p.1),
        _ => None,
    };
    assert_eq!(
        (at(0, 0), at(1, 0)),
        (Some(0.5), Some(0.5)),
        "a negative counts as nothing"
    );
    assert!((at(0, 1).unwrap() - 0.3).abs() < 1e-9 && (at(1, 1).unwrap() - 0.7).abs() < 1e-9);
    assert_eq!(
        (at(0, 2), at(1, 2)),
        (Some(0.0), Some(0.0)),
        "an empty total is not a division by zero"
    );
    assert!(
        drawn_text(&c, 400.0, 240.0).iter().any(|t| t == "100%"),
        "the axis reads in percent"
    );
}

#[test]
fn a_band_is_ordered_counted_and_read_out() {
    let c = Chart::new().band(
        "normal",
        &[(2.0, 30.0, 10.0), (1.0, 12.0, 28.0), (f64::NAN, 1.0, 2.0)],
    );
    match &c.layers[0].mark {
        Mark::Band { rows } => assert_eq!(
            rows,
            &[(1.0, 12.0, 28.0), (2.0, 10.0, 30.0)],
            "x order, low below high, bad rows dropped"
        ),
        other => panic!("{other:?}"),
    }
    let (lo, hi) = c.y_extent(Panel::Main, (1.0, 2.0), 5);
    assert!(
        lo <= 10.0 && hi >= 30.0,
        "the range covers both ends: {lo}..{hi}"
    );
    let (_, style) = setup();
    let p = crate::chart::probe(&c, &style, 1.1).expect("something to read");
    assert_eq!(p.rows.len(), 1);
    assert!(p.rows[0].value.contains('–'), "{}", p.rows[0].value);
}

#[test]
fn pinned_ranges_win_over_the_data() {
    let c = Chart::new()
        .line("a", &[(0.0, 5.0), (10.0, 6.0)])
        .y_range(100.0, 0.0)
        .x_range(-3.0, 3.0);
    assert_eq!(c.y_extent(Panel::Main, (0.0, 10.0), 5), (0.0, 100.0));
    assert_eq!(c.x_extent(), (-3.0, 3.0));
    let loose = Chart::new().line("a", &[(0.0, 5.0)]).y_range(1.0, 1.0);
    assert_eq!(loose.y_fixed, None, "an empty range is no range");
}

#[test]
fn labels_go_beside_their_dots_and_never_over_each_other() {
    let far = Chart::new()
        .tags(
            "s",
            &[
                Tag::new(1.0, 1.0, "AAA"),
                Tag::new(5.0, 5.0, "BBB"),
                Tag::new(9.0, 9.0, "CCC"),
            ],
        )
        .x_range(0.0, 10.0)
        .y_range(0.0, 10.0);
    let words = drawn_text(&far, 400.0, 260.0);
    for w in ["AAA", "BBB", "CCC"] {
        assert!(words.iter().any(|t| t == w), "{w} in {words:?}");
    }
    // Two on the same spot: the bigger keeps its label, the other has none rather than a clash.
    let stacked = Chart::new()
        .tags(
            "s",
            &[
                Tag::new(5.0, 5.0, "SMALL").size(1.0),
                Tag::new(5.0, 5.0, "BIG").size(9.0),
            ],
        )
        .x_range(0.0, 10.0)
        .y_range(0.0, 10.0);
    let words = drawn_text(&stacked, 400.0, 260.0);
    assert!(words.iter().any(|t| t == "BIG"), "{words:?}");
    // A label that would run off the plot goes to the other side of its dot.
    let edge = Chart::new()
        .tags("s", &[Tag::new(10.0, 5.0, "RIGHTEDGE")])
        .x_range(0.0, 10.0)
        .y_range(0.0, 10.0);
    assert!(
        drawn_text(&edge, 400.0, 260.0)
            .iter()
            .any(|t| t == "RIGHTEDGE")
    );
}

#[test]
fn a_note_is_kept_inside_the_plot() {
    let c = Chart::new()
        .tags(
            "q",
            &[
                Tag::new(10.0, 9.0, "Leading").note(),
                Tag::new(0.0, 1.0, "Lagging").note(),
            ],
        )
        .x_range(0.0, 10.0)
        .y_range(0.0, 10.0);
    let words = drawn_text(&c, 400.0, 260.0);
    assert!(
        words.iter().any(|t| t == "Leading") && words.iter().any(|t| t == "Lagging"),
        "{words:?}"
    );
}

#[test]
fn bad_points_are_not_tagged() {
    let c = Chart::new().tags(
        "s",
        &[Tag::new(f64::NAN, 1.0, "x"), Tag::new(1.0, 1.0, "ok")],
    );
    match &c.layers[0].mark {
        Mark::Tags(t) => assert_eq!(t.len(), 1),
        other => panic!("{other:?}"),
    }
}
