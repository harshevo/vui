//! Turning a chart into drawing: where the axes go, then the gridlines, then each layer's marks.

use vui_core::vector::{Cap, Join, Stroke};
use vui_core::{Corners, Fonts, Point, Quad, Rect, pt, rect};

use super::decimate::extremes;
use super::{Candle, Chart, Dot, Layer, Mark, Panel, within};
use crate::canvas::{Canvas, HAnchor, VAnchor};
use crate::format::Format;
use crate::scale::{Linear, nice_ticks, time_ticks};
use crate::style::VizStyle;

const TOP: f32 = 8.0;
const RIGHT: f32 = 20.0;
const X_LABELS: f32 = 24.0;
/// A strip needs at least this much of the chart's height to be worth showing.
const STRIP_SHARE: f32 = 0.2;
const STRIP_GAP: f32 = 18.0;

/// What a chart currently shows: the visible range on each axis. The strip has a range of its own.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct View {
    pub x: (f64, f64),
    pub y: (f64, f64),
    pub strip: (f64, f64),
}

impl Chart {
    /// The view that shows every mark inside `x` with round numbers on the vertical axes.
    pub fn fit(&self, x: (f64, f64)) -> View {
        View {
            x,
            y: self.y_extent(Panel::Main, x, 5),
            strip: self.y_extent(Panel::Strip, x, 2),
        }
    }
}

/// Where everything landed, for turning a pointer position back into data.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// Main and strip together.
    pub plot: Rect,
    pub main: Rect,
    pub strip: Option<Rect>,
    pub x: Linear,
    pub y: Linear,
    pub y_strip: Option<Linear>,
}

/// Works out the plot area inside `rect`: room on the left for the widest label, below for the dates.
pub fn layout(fonts: &Fonts, chart: &Chart, rect_: Rect, view: &View, style: &VizStyle) -> Drawn {
    let ticks = nice_ticks(view.y.0.min(view.y.1), view.y.0.max(view.y.1), 5);
    let widest = ticks
        .iter()
        .map(|t| {
            fonts
                .measure(&chart.y_format.apply(*t), &style.label, None)
                .w
        })
        .fold(0.0, f32::max);
    // Round up to a multiple of 8 so the plot does not shift as labels gain a digit while zooming.
    let left = ((widest + 12.0) / 8.0).ceil() * 8.0;
    let plot = rect(
        rect_.x + left,
        rect_.y + TOP,
        (rect_.w - left - RIGHT).max(1.0),
        (rect_.h - TOP - X_LABELS).max(1.0),
    );
    let strip_h = if chart.has_strip() && plot.h > 120.0 {
        (plot.h * STRIP_SHARE).round()
    } else {
        0.0
    };
    let main = rect(
        plot.x,
        plot.y,
        plot.w,
        plot.h
            - if strip_h > 0.0 {
                strip_h + STRIP_GAP
            } else {
                0.0
            },
    );
    let strip = (strip_h > 0.0).then(|| rect(plot.x, plot.bottom() - strip_h, plot.w, strip_h));
    Drawn {
        plot,
        main,
        strip,
        x: Linear::new(view.x, (plot.x, plot.right())),
        y: Linear::new(view.y, (main.bottom(), main.y)),
        y_strip: strip.map(|s| Linear::new(view.strip, (s.bottom(), s.y))),
    }
}

/// Draws the axes and every layer. `reveal` runs 0 to 1 and uncovers the marks from left to right.
pub fn draw(
    canvas: &mut Canvas,
    chart: &Chart,
    area: Rect,
    view: &View,
    style: &VizStyle,
    reveal: f32,
) -> Drawn {
    let d = layout(canvas.fonts(), chart, area, view, style);
    axes(canvas, chart, &d, view, style);
    let shown = 1.0 - (1.0 - reveal.clamp(0.0, 1.0)).powi(3);
    for (i, layer) in chart.layers.iter().enumerate() {
        let (panel, y) = match (layer.panel, d.strip, d.y_strip) {
            (Panel::Strip, Some(s), Some(y)) => (s, y),
            (Panel::Strip, _, _) => continue,
            _ => (d.main, d.y),
        };
        canvas.set_clip(Some(rect(
            panel.x,
            panel.y - 3.0,
            panel.w * shown,
            panel.h + 6.0,
        )));
        let color = layer.ink.resolve(style, i);
        marks(canvas, layer, &d.x, &y, color, style);
    }
    canvas.set_clip(None);
    d
}

fn axes(canvas: &mut Canvas, chart: &Chart, d: &Drawn, view: &View, style: &VizStyle) {
    let (lo, hi) = (view.y.0.min(view.y.1), view.y.0.max(view.y.1));
    let count = ((d.main.h / 44.0) as usize).clamp(2, 8);
    for t in nice_ticks(lo, hi, count) {
        let y = d.y.map(t);
        if y < d.main.y - 0.5 || y > d.main.bottom() + 0.5 {
            continue;
        }
        let zero = t == 0.0 && y < d.main.bottom() - 1.0;
        canvas.hairline(
            pt(d.main.x, y),
            pt(d.main.right(), y),
            if zero {
                style.ink_3.mul_alpha(0.6)
            } else {
                style.line.mul_alpha(0.55)
            },
        );
        canvas.text(
            &chart.y_format.apply(t),
            &style.label,
            pt(d.main.x - 8.0, y),
            HAnchor::Right,
            VAnchor::Middle,
            style.ink_3,
        );
    }
    if let (Some(strip), Some(y)) = (d.strip, d.y_strip) {
        let top = nice_ticks(
            view.strip.0.min(view.strip.1),
            view.strip.0.max(view.strip.1),
            2,
        )
        .last()
        .copied();
        if let Some(t) = top {
            canvas.text(
                &Format::Si.apply(t),
                &style.label,
                pt(strip.x - 8.0, y.map(t).max(strip.y)),
                HAnchor::Right,
                VAnchor::Top,
                style.ink_3,
            );
        }
        canvas.hairline(
            pt(strip.x, strip.bottom()),
            pt(strip.right(), strip.bottom()),
            style.line,
        );
    } else {
        canvas.hairline(
            pt(d.main.x, d.main.bottom()),
            pt(d.main.right(), d.main.bottom()),
            style.line,
        );
    }

    let want = ((d.plot.w / 96.0) as usize).clamp(2, 10);
    let ticks = if chart.time {
        time_ticks(view.x.0, view.x.1, want)
    } else {
        nice_ticks(view.x.0, view.x.1, want)
            .into_iter()
            .map(|v| crate::scale::Tick {
                value: v,
                label: Format::Auto.apply(v),
            })
            .collect()
    };
    let y = d.plot.bottom() + 8.0;
    let mut edge = f32::NEG_INFINITY;
    for t in ticks {
        let x = d.x.map(t.value);
        if x < d.plot.x - 1.0 || x > d.plot.right() + 1.0 {
            continue;
        }
        let w = canvas.measure(&t.label, &style.label).w;
        if x - w / 2.0 < edge + 10.0 || x + w / 2.0 > d.plot.right() + RIGHT - 4.0 {
            continue;
        }
        canvas.text(
            &t.label,
            &style.label,
            pt(x, y),
            HAnchor::Center,
            VAnchor::Top,
            style.ink_3,
        );
        edge = x + w / 2.0;
    }
}

fn marks(
    canvas: &mut Canvas,
    layer: &Layer,
    x: &Linear,
    y: &Linear,
    color: vui_core::Color,
    style: &VizStyle,
) {
    let view = (x.domain.0.min(x.domain.1), x.domain.0.max(x.domain.1));
    match &layer.mark {
        Mark::Line {
            points,
            step,
            dashed,
        } => {
            let thin = extremes(within(points, |p| p.0, view), x);
            line(canvas, &screen(&thin, x, y), *step, *dashed, color);
        }
        Mark::Area { points, base } => {
            let thin = extremes(within(points, |p| p.0, view), x);
            let upper = screen(&thin, x, y);
            let floor = y.map(*base);
            let lower: Vec<Point> = upper.iter().map(|p| pt(p.x, floor)).collect();
            canvas.vector().band(&upper, &lower, color.mul_alpha(0.14));
            line(canvas, &upper, false, false, color);
        }
        Mark::Bars { points, base } => bars(
            canvas,
            within(points, |p| p.0, view),
            layer.mark.spacing(),
            *base,
            x,
            y,
            color,
        ),
        Mark::Dots(dots) => scatter(canvas, within(dots, |d| d.x, view), dots, x, y, color),
        Mark::Candles(rows) => candles(
            canvas,
            within(rows, |c| c.x, view),
            layer.mark.spacing(),
            x,
            y,
            color,
            style,
        ),
    }
}

fn screen(points: &[(f64, f64)], x: &Linear, y: &Linear) -> Vec<Point> {
    points.iter().map(|p| pt(x.map(p.0), y.map(p.1))).collect()
}

fn line(canvas: &mut Canvas, points: &[Point], step: bool, dashed: bool, color: vui_core::Color) {
    if points.len() < 2 {
        return;
    }
    let mut stroke = Stroke::new(1.5).join(Join::Round).cap(Cap::Round);
    if dashed {
        stroke = stroke.dashed(&[5.0, 4.0], 0.0);
    }
    if step {
        let mut stepped = Vec::with_capacity(points.len() * 2);
        for pair in points.windows(2) {
            stepped.push(pair[0]);
            stepped.push(pt(pair[1].x, pair[0].y));
        }
        stepped.push(points[points.len() - 1]);
        canvas.vector().polyline(&stepped, &stroke, color);
    } else {
        canvas.vector().polyline(points, &stroke, color);
    }
}

/// How many pixels one data unit of x takes.
fn per_unit(x: &Linear) -> f32 {
    let span = (x.domain.1 - x.domain.0).abs();
    if span == 0.0 {
        1.0
    } else {
        ((x.range.1 - x.range.0).abs() as f64 / span) as f32
    }
}

fn bars(
    canvas: &mut Canvas,
    points: &[(f64, f64)],
    spacing: f64,
    base: f64,
    x: &Linear,
    y: &Linear,
    color: vui_core::Color,
) {
    let slot = spacing as f32 * per_unit(x);
    let thin;
    let points = if slot < 1.5 {
        thin = extremes(points, x);
        &thin[..]
    } else {
        points
    };
    let width = (slot * 0.64).clamp(1.0, 36.0);
    let radius = if width >= 5.0 { 2.0 } else { 0.0 };
    let floor = y.map(base);
    for &(px, v) in points {
        let top = y.map(v);
        let (y0, y1) = (top.min(floor), top.max(floor));
        let corners = if top <= floor {
            Corners {
                tl: radius,
                tr: radius,
                br: 0.0,
                bl: 0.0,
            }
        } else {
            Corners {
                tl: 0.0,
                tr: 0.0,
                br: radius,
                bl: radius,
            }
        };
        let cx = x.map(px);
        canvas.quad(
            Quad::new(rect(cx - width / 2.0, y0, width, (y1 - y0).max(1.0)))
                .fill(color)
                .radii(corners),
        );
    }
}

fn scatter(
    canvas: &mut Canvas,
    shown: &[Dot],
    all: &[Dot],
    x: &Linear,
    y: &Linear,
    color: vui_core::Color,
) {
    let biggest = all
        .iter()
        .map(|d| d.size)
        .fold(0.0, f64::max)
        .max(f64::EPSILON);
    for d in shown {
        let r = 3.0 + (d.size / biggest).sqrt() as f32 * 9.0;
        let at = pt(x.map(d.x), y.map(d.y));
        canvas.ring(at, r, 1.0, color.mul_alpha(0.28), color);
    }
}

fn candles(
    canvas: &mut Canvas,
    rows: &[Candle],
    spacing: f64,
    x: &Linear,
    y: &Linear,
    ink: vui_core::Color,
    style: &VizStyle,
) {
    let slot = spacing as f32 * per_unit(x);
    let width = (slot * 0.7).clamp(1.0, 16.0);
    let wick = 1.0;
    for c in rows {
        let cx = x.map(c.x);
        let (high, low) = (y.map(c.high), y.map(c.low));
        canvas.quad(Quad::new(rect(cx - wick / 2.0, high, wick, (low - high).max(1.0))).fill(ink));
        if width < 2.5 {
            continue;
        }
        let (a, b) = (y.map(c.open), y.map(c.close));
        let body = rect(cx - width / 2.0, a.min(b), width, (a - b).abs().max(1.0));
        // Up candles are hollow, down candles solid: readable without colour.
        let quad = Quad::new(body).radii(1.0);
        canvas.quad(if c.close >= c.open {
            quad.fill(style.bg).border(1.0, ink)
        } else {
            quad.fill(ink)
        });
    }
}

#[cfg(test)]
mod tests {
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
}
