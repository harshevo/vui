//! A sheet of every chart type, drawn for real and compared with a stored picture.

use vui_core::{Color, Fonts, Rect, Scene, Theme, rect, size};
use vui_render::assert_golden;
use vui_render::testing::render_scene;
use vui_viz::{Candle, Canvas, Chart, Dot, Format, Ink, VizStyle, chart, paint};

const DAY: f64 = 86_400.0;
const START: f64 = 1_730_000_000.0;

/// A repeatable wobble standing in for a market, so the sheet never changes between runs.
fn walk(n: usize, seed: f64) -> Vec<(f64, f64)> {
    let mut v = 100.0;
    (0..n)
        .map(|i| {
            let t = i as f64;
            v += ((t * 0.9 + seed).sin() * 1.6 + (t * 0.23 + seed * 2.0).cos() * 1.1) + 0.12;
            (START + t * DAY, v)
        })
        .collect()
}

fn candles(n: usize) -> Vec<Candle> {
    let close = walk(n, 0.4);
    close
        .iter()
        .enumerate()
        .map(|(i, &(x, c))| {
            let open = if i == 0 { c } else { close[i - 1].1 };
            let wick = 0.8 + ((i as f64) * 1.7).sin().abs() * 1.6;
            Candle {
                x,
                open,
                close: c,
                high: open.max(c) + wick,
                low: open.min(c) - wick,
            }
        })
        .collect()
}

fn cell(
    scene: &mut Scene,
    fonts: &Fonts,
    style: &VizStyle,
    at: Rect,
    chart: &Chart,
    probe_at: Option<f64>,
) {
    let mut canvas = Canvas::new(fonts, scene.scale);
    let view = chart.fit(chart.x_extent());
    let drawn = chart::draw(&mut canvas, chart, at, &view, style, 1.0);
    if let Some(x) = probe_at
        && let Some(p) = chart::probe(chart, style, x)
    {
        chart::draw_probe(&mut canvas, &drawn, &p, style);
    }
    paint(scene, canvas.finish(), Some(at), 1.0);
}

fn sheet(theme: &Theme) -> Scene {
    let fonts = Fonts::bundled_only();
    let style = VizStyle::from_palette(&theme.palette);
    let mut scene = Scene::new(size(960.0, 640.0), 2.0).with_clear(theme.palette.bg);

    let price = walk(90, 0.0);
    let volume: Vec<(f64, f64)> = price
        .iter()
        .enumerate()
        .map(|(i, p)| (p.0, 2.0e6 + ((i as f64 * 1.3).sin() + 1.2) * 1.5e6))
        .collect();
    let trend: Vec<(f64, f64)> = price
        .windows(10)
        .map(|w| (w[9].0, w.iter().map(|p| p.1).sum::<f64>() / 10.0))
        .collect();
    let main = Chart::new()
        .time()
        .y_format(Format::Money("$"))
        .line("ACME", &price)
        .line("10d avg", &trend)
        .dashed()
        .volume("Volume", &volume);
    cell(
        &mut scene,
        &fonts,
        &style,
        rect(0.0, 0.0, 480.0, 320.0),
        &main,
        Some(START + 52.0 * DAY),
    );

    let area = Chart::new()
        .time()
        .area("Revenue", &walk(40, 1.0))
        .ink(Ink::Accent)
        .format(Format::Money("$"))
        .y_format(Format::Money("$"));
    cell(
        &mut scene,
        &fonts,
        &style,
        rect(480.0, 0.0, 480.0, 320.0),
        &area,
        None,
    );

    let bars: Vec<(f64, f64)> = (0..12)
        .map(|i| {
            (
                START + f64::from(i) * 30.0 * DAY,
                ((f64::from(i) * 1.1).sin() * 8.0) + 4.0,
            )
        })
        .collect();
    let bars = Chart::new()
        .time()
        .bars("Growth", &bars)
        .y_format(Format::PercentPoints(0))
        .ink(Ink::Accent);
    cell(
        &mut scene,
        &fonts,
        &style,
        rect(0.0, 320.0, 320.0, 320.0),
        &bars,
        Some(START + 120.0 * DAY),
    );

    let rows = candles(36);
    let candle = Chart::new().time().candles("ACME", &rows);
    cell(
        &mut scene,
        &fonts,
        &style,
        rect(320.0, 320.0, 320.0, 320.0),
        &candle,
        Some(START + 20.0 * DAY),
    );

    let dots: Vec<Dot> = (0..14)
        .map(|i| Dot {
            x: f64::from(i),
            y: (f64::from(i) * 0.7).sin() * 5.0 + f64::from(i) * 0.6,
            size: f64::from((i * 7) % 11 + 1),
        })
        .collect();
    let scatter = Chart::new().dots("Funds", &dots);
    cell(
        &mut scene,
        &fonts,
        &style,
        rect(640.0, 320.0, 320.0, 320.0),
        &scatter,
        None,
    );
    scene
}

#[test]
fn every_chart_type_in_light() {
    let fonts = Fonts::bundled_only();
    assert_golden!(
        "charts_light",
        render_scene(&sheet(&Theme::light()), &fonts)
    );
}

#[test]
fn every_chart_type_in_dark() {
    let fonts = Fonts::bundled_only();
    assert_golden!("charts_dark", render_scene(&sheet(&Theme::dark()), &fonts));
}

#[test]
fn the_sheet_background_is_the_palette_background() {
    let theme = Theme::light();
    let img = render_scene(&sheet(&theme), &Fonts::bundled_only());
    let [r, g, b, _] = img.pixel(2, 2);
    let want = theme.palette.bg.to_rgba8();
    assert_eq!([r, g, b], [want[0], want[1], want[2]]);
    let _ = Color::BLACK;
}
