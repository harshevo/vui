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
