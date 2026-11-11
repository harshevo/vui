//! Charts: numbers with sparklines, a time series, candlesticks, grouped and stacked bars, a pie and
//! a donut, and a ranked list. Every chart fills the width it is given and lays itself out again on
//! a phone.

use std::rc::Rc;

use vui::viz::{BarList, BarListView, Candle, Chart, ChartView, Format, Ink, Pie, PieView, bar_list, chart, indexed, pie, spark};

use super::*;

const DAY: f64 = 86_400.0;
const START: f64 = 1_761_955_200.0;

/// A repeatable wobble standing in for a market, so the page looks the same every time.
fn walk(n: usize, seed: f64) -> Vec<(f64, f64)> {
    let mut v = 100.0;
    (0..n)
        .map(|i| {
            let t = i as f64;
            v += (t * 0.9 + seed).sin() * 1.6 + (t * 0.23 + seed * 2.0).cos() * 1.1 + 0.15;
            (START + t * DAY, v)
        })
        .collect()
}

fn candles(n: usize) -> Vec<Candle> {
    let close = walk(n, 0.4);
    (0..n)
        .map(|i| {
            let (x, c) = close[i];
            let open = if i == 0 { c } else { close[i - 1].1 };
            let wick = 0.8 + (i as f64 * 1.7).sin().abs() * 1.6;
            Candle { x, open, close: c, high: open.max(c) + wick, low: open.min(c) - wick }
        })
        .collect()
}

/// A number that matters, how it moved, and its recent shape.
fn stat(name: &str, value: &str, delta: &str, up: bool, seed: f64) -> Element<Msg> {
    let values: Vec<f64> = walk(24, seed).into_iter().map(|p| p.1).collect();
    let change = if up { badge(delta).success() } else { badge(delta).danger() };
    card()
        .grow(1.0)
        .basis(px(200.0))
        .gap(8.0)
        .children((
            small(name).color(Tone::Text2),
            row().gap(8.0).align_items(Align::Center).children((title(value), change)),
            spark(&values).height(32.0),
        ))
        .into_element()
}

/// A chart in a card with a heading, sharing a row with others when there is room.
fn panel(name: &str, body: impl IntoElement<Msg>) -> Element<Msg> {
    card().grow(1.0).basis(px(320.0)).gap(12.0).children((label(name).weight(500), body)).into_element()
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let calm = |c: ChartView<Msg>| if g.still { c.still() } else { c };
    let price = walk(120, 0.0);
    let average: Vec<(f64, f64)> = price.windows(10).map(|w| (w[9].0, w.iter().map(|p| p.1).sum::<f64>() / 10.0)).collect();
    let volume: Vec<(f64, f64)> = price.iter().enumerate().map(|(i, p)| (p.0, 2.0e6 + ((i as f64 * 1.3).sin() + 1.2) * 1.5e6)).collect();
    let series = Chart::new().time().y_format(Format::Money("$")).line("ACME", &price).ink(Ink::Accent).line("10-day average", &average).ink(Ink::Mid).dashed().volume("Volume", &volume);
    let rows = candles(48);
    let traded: Vec<(f64, f64)> = rows.iter().enumerate().map(|(i, c)| (c.x, 1.0e6 + (i as f64 * 0.9).sin().abs() * 2.0e6)).collect();
    let market = Chart::new().time().market().candles("ACME", &rows).volume("Volume", &traded);

    let grouped = Chart::new()
        .categories(["Q1", "Q2", "Q3", "Q4"])
        .y_format(Format::Money("$"))
        .bars("2024", &indexed(&[42.0, 51.0, 47.0, 63.0]))
        .ink(Ink::Hue)
        .bars("2025", &indexed(&[48.0, 57.0, 61.0, 72.0]))
        .ink(Ink::Hue)
        .bars("2026", &indexed(&[55.0, 60.0, 70.0, 81.0]))
        .ink(Ink::Hue)
        .grouped();
    let stacked = Chart::new()
        .categories(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .bars("Search", &indexed(&[30.0, 34.0, 32.0, 40.0, 44.0, 47.0]))
        .ink(Ink::Hue)
        .bars("Social", &indexed(&[12.0, 15.0, 19.0, 18.0, 24.0, 27.0]))
        .ink(Ink::Hue)
        .bars("Email", &indexed(&[8.0, 9.0, 7.0, 11.0, 12.0, 10.0]))
        .ink(Ink::Hue)
        .stacked();

    let visits = Pie::new().slice("Direct", 4210.0).slice("Search", 3120.0).slice("Social", 1430.0).slice("Email", 880.0).slice("Other", 360.0);
    let plans = Pie::new().slice("Free", 62.0).slice("Pro", 28.0).slice("Team", 10.0).gap(3.0);
    let browsers = BarList::new().row("Chrome", 6420.0).row("Safari", 3180.0).row("Firefox", 860.0).row("Edge", 1120.0).row("Arc", 410.0).sorted().format(Format::Si);
    let hold_pie = |p: PieView<Msg>| if g.still { p.still() } else { p };
    let hold_list = |b: BarListView<Msg>| if g.still { b.still() } else { b };

    page(
        "Charts",
        (
            section(
                "At a glance",
                row().wrap().gap(12.0).children((
                    stat("Revenue", "$48.2k", "+12%", true, 0.0),
                    stat("Active users", "3,912", "+4%", true, 1.3),
                    stat("Churn", "2.1%", "-0.4%", false, 2.6),
                )),
            ),
            section("Time series", calm(chart(Rc::new(series)).name("ACME price, with its 10-day average and volume"))),
            section("Candlesticks", calm(chart(Rc::new(market)).name("ACME daily candles"))),
            section(
                "Bars",
                row().wrap().gap(12.0).children((
                    panel("Revenue by quarter", calm(chart(Rc::new(grouped)).height(240.0))),
                    panel("Signups by channel", calm(chart(Rc::new(stacked)).height(240.0))),
                )),
            ),
            section(
                "Pie and donut",
                row().wrap().gap(12.0).children((
                    panel("Visits", hold_pie(pie(Rc::new(visits.donut().format(Format::Si).caption("Visits"))))),
                    panel("Plans", hold_pie(pie(Rc::new(plans)))),
                )),
            ),
            section("Ranked", card().child(hold_list(bar_list(Rc::new(browsers)).name("Visitors by browser")))),
        ),
    )
}

