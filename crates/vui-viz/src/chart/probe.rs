//! What is under the pointer: the data at the nearest x in every layer, and the guide and tooltip that
//! show it.

use vui_core::{Color, Corners, Quad, pt, rect};

use super::draw::Drawn;
use super::{Candle, Chart, Dot, Mark, Panel};
use crate::canvas::{Canvas, HAnchor, VAnchor};
use crate::format::Format;
use crate::style::VizStyle;

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub value: String,
    pub color: Color,
    pub panel: Panel,
    /// Where to put a marker on the line, for layers that have one point per x.
    pub y: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Probe {
    pub x: f64,
    pub head: String,
    pub rows: Vec<Row>,
}

/// The index of the item whose key is nearest `x`, in an x-sorted slice.
fn nearest<T>(items: &[T], key: impl Fn(&T) -> f64, x: f64) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let i = items.partition_point(|t| key(t) < x);
    Some(match i {
        0 => 0,
        i if i == items.len() => i - 1,
        i => {
            if x - key(&items[i - 1]) <= key(&items[i]) - x {
                i - 1
            } else {
                i
            }
        }
    })
}

impl Mark {
    fn nearest_x(&self, x: f64) -> Option<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                nearest(points, |p| p.0, x).map(|i| points[i].0)
            }
            Mark::Dots(d) => nearest(d, |d| d.x, x).map(|i| d[i].x),
            Mark::Candles(c) => nearest(c, |c| c.x, x).map(|i| c[i].x),
        }
    }
}

/// The values at the data x nearest to `x`. `None` when the chart has no data.
pub fn probe(chart: &Chart, style: &VizStyle, x: f64) -> Option<Probe> {
    let at = chart
        .layers
        .iter()
        .filter_map(|l| l.mark.nearest_x(x))
        .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))?;
    let mut rows = Vec::new();
    for (i, layer) in chart.layers.iter().enumerate() {
        let fmt = layer.format.unwrap_or(chart.y_format);
        let color = layer.ink.resolve(style, i);
        let slot = layer.mark.spacing() / 2.0 + f64::EPSILON;
        let row = |name: &str, v: f64, y: Option<f64>| Row {
            name: name.to_string(),
            value: fmt.apply(v),
            color,
            panel: layer.panel,
            y,
        };
        match &layer.mark {
            Mark::Line { points, .. } | Mark::Area { points, .. } => {
                if let Some(&(_, v)) = nearest(points, |p| p.0, at).map(|j| &points[j]) {
                    rows.push(row(&layer.name, v, Some(v)));
                }
            }
            Mark::Bars { points, .. } => {
                if let Some(&(px, v)) = nearest(points, |p| p.0, at).map(|j| &points[j])
                    && (px - at).abs() <= slot
                {
                    rows.push(row(&layer.name, v, None));
                }
            }
            Mark::Dots(d) => {
                if let Some(&Dot { x: px, y, .. }) = nearest(d, |d| d.x, at).map(|j| &d[j])
                    && px == at
                {
                    rows.push(row(&layer.name, y, Some(y)));
                }
            }
            Mark::Candles(c) => {
                if let Some(&Candle {
                    x: px,
                    open,
                    high,
                    low,
                    close,
                }) = nearest(c, |c| c.x, at).map(|j| &c[j])
                    && (px - at).abs() <= slot
                {
                    for (name, v) in [
                        ("Open", open),
                        ("High", high),
                        ("Low", low),
                        ("Close", close),
                    ] {
                        rows.push(row(name, v, None));
                    }
                }
            }
        }
    }
    let head = if chart.time {
        Format::Date.apply(at)
    } else {
        Format::Auto.apply(at)
    };
    Some(Probe { x: at, head, rows })
}

/// Draws the vertical guide, a marker on each line, and a tooltip beside them.
pub fn draw_probe(canvas: &mut Canvas, d: &Drawn, probe: &Probe, style: &VizStyle) {
    let x = d.x.map(probe.x);
    if x < d.plot.x - 0.5 || x > d.plot.right() + 0.5 {
        return;
    }
    canvas.set_clip(Some(d.plot.inflate(0.0, 4.0)));
    canvas.hairline(
        pt(x, d.plot.y),
        pt(x, d.plot.bottom()),
        style.ink_3.mul_alpha(0.7),
    );
    for row in probe.rows.iter().filter(|r| r.panel == Panel::Main) {
        if let Some(y) = row.y {
            canvas.ring(pt(x, d.y.map(y)), 4.0, 1.5, style.bg, row.color);
        }
    }
    canvas.set_clip(None);

    let pad = 10.0;
    let row_h = 18.0;
    let head_w = canvas.measure(&probe.head, &style.label).w;
    let name_w = probe
        .rows
        .iter()
        .map(|r| canvas.measure(&r.name, &style.label).w)
        .fold(0.0, f32::max);
    let value_w = probe
        .rows
        .iter()
        .map(|r| canvas.measure(&r.value, &style.label_mono).w)
        .fold(0.0, f32::max);
    let body_w = if probe.rows.is_empty() {
        0.0
    } else {
        14.0 + name_w + 20.0 + value_w
    };
    let w = head_w.max(body_w) + pad * 2.0;
    let h = pad * 2.0
        + 14.0
        + if probe.rows.is_empty() {
            0.0
        } else {
            6.0 + row_h * probe.rows.len() as f32
        };
    let left = if x + 14.0 + w > d.plot.right() {
        x - 14.0 - w
    } else {
        x + 14.0
    };
    let r = rect(left.max(d.plot.x), d.plot.y + 6.0, w, h);
    canvas.quad(
        Quad::new(r)
            .fill(style.bg)
            .radii(Corners::all(6.0))
            .border(1.0, style.line),
    );
    canvas.text(
        &probe.head,
        &style.label,
        pt(r.x + pad, r.y + pad),
        HAnchor::Left,
        VAnchor::Top,
        style.ink_3,
    );
    let mut y = r.y + pad + 14.0 + 6.0;
    for row in &probe.rows {
        let mid = y + row_h / 2.0;
        canvas.dot(pt(r.x + pad + 3.0, mid), 3.0, row.color);
        canvas.text(
            &row.name,
            &style.label,
            pt(r.x + pad + 14.0, mid),
            HAnchor::Left,
            VAnchor::Middle,
            style.ink_2,
        );
        canvas.text(
            &row.value,
            &style.label_mono,
            pt(r.right() - pad, mid),
            HAnchor::Right,
            VAnchor::Middle,
            style.ink,
        );
        y += row_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::draw::layout;
    use vui_core::{Command, Fonts, Theme};

    fn style() -> VizStyle {
        VizStyle::from_palette(&Theme::light().palette)
    }

    fn series(f: impl Fn(f64) -> f64) -> Vec<(f64, f64)> {
        (0..10).map(|i| (f64::from(i), f(f64::from(i)))).collect()
    }

    #[test]
    fn the_probe_snaps_to_the_nearest_point_and_reads_every_layer() {
        let chart = Chart::new()
            .line("a", &series(|x| x * 2.0))
            .line("b", &series(|x| x * 3.0))
            .bars("n", &series(|x| x));
        let p = probe(&chart, &style(), 4.4).unwrap();
        assert_eq!(p.x, 4.0);
        assert_eq!(p.head, "4");
        let values: Vec<&str> = p.rows.iter().map(|r| r.value.as_str()).collect();
        assert_eq!(values, ["8", "12", "4"]);
        assert_eq!(p.rows[0].y, Some(8.0));
        assert_eq!(p.rows[2].y, None);
        assert_eq!(probe(&chart, &style(), -50.0).unwrap().x, 0.0);
        assert!(probe(&Chart::new(), &style(), 1.0).is_none());
    }

    #[test]
    fn bars_and_candles_only_answer_under_their_own_slot() {
        let sparse = [(0.0, 5.0), (1.0, 6.0), (2.0, 7.0)];
        let chart = Chart::new()
            .line("a", &[(0.0, 1.0), (10.0, 2.0)])
            .bars("n", &sparse);
        let near = probe(&chart, &style(), 0.2).unwrap();
        assert_eq!(near.rows.len(), 2, "the bar at 0 is under the cursor");
        let far = probe(&chart, &style(), 9.9).unwrap();
        assert_eq!(far.x, 10.0);
        assert_eq!(far.rows.len(), 1, "no bar near 10, only the line");
    }

    #[test]
    fn candles_read_open_high_low_close() {
        let rows = [Candle {
            x: 0.0,
            open: 1.0,
            high: 3.0,
            low: 0.5,
            close: 2.0,
        }];
        let p = probe(&Chart::new().candles("px", &rows), &style(), 0.0).unwrap();
        let read: Vec<(&str, &str)> = p
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.value.as_str()))
            .collect();
        assert_eq!(
            read,
            [("Open", "1"), ("High", "3"), ("Low", "0.5"), ("Close", "2")]
        );
    }

    #[test]
    fn time_charts_head_the_probe_with_a_date() {
        let ts = 1_700_000_000.0;
        let p = probe(&Chart::new().time().line("a", &[(ts, 1.0)]), &style(), ts).unwrap();
        assert!(p.head.contains("2023"), "{}", p.head);
    }

    #[test]
    fn the_tooltip_stays_inside_the_plot() {
        let fonts = Fonts::bundled_only();
        let style = style();
        let chart = Chart::new()
            .line("alpha", &series(|x| x * 2.0))
            .line("beta", &series(|x| x * 3.0));
        let view = chart.fit(chart.x_extent());
        let d = layout(&fonts, &chart, rect(0.0, 0.0, 400.0, 240.0), &view, &style);
        for at in [0.0, 5.0, 9.0] {
            let mut c = Canvas::new(&fonts, 1.0);
            draw_probe(&mut c, &d, &probe(&chart, &style, at).unwrap(), &style);
            let items = c.finish();
            let boxes: Vec<_> = items
                .iter()
                .filter_map(|i| {
                    if let Command::Quad(q) = i
                        && q.rect.w > 40.0
                    {
                        Some(q.rect)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(boxes.len(), 1);
            assert!(
                boxes[0].x >= d.plot.x - 0.01 && boxes[0].right() <= d.plot.right() + 0.01,
                "{at}: {:?} in {:?}",
                boxes[0],
                d.plot
            );
        }
    }
}
