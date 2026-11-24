//! The pieces the research window is built from, drawn for real in light and dark and compared with
//! stored pictures: the data grid, the treemap, the histogram slider, the band and scatter charts and
//! the stepper.

use std::rc::Rc;

use vui_core::Color;
use vui_core::{Fill, Fonts, Theme, Tone, pt, size};
use vui_render::assert_golden;
use vui_render::testing::render_scene;
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, IntoElement, Justify, Parent, Radius, Styled, column, label, row};
use vui_viz::{Cell, Chart, Column, GridData, Histogram, Ink, Legend, Order, Tag, Treemap, Zone, chart, data_grid, histogram, indexed, legend, treemap};
use vui_widgets::{Decoration, Step, When, range_slider, text_field, timeline};

/// A repeatable wobble, so a sheet never changes between runs.
fn wave(n: usize, seed: f32, up: bool) -> Vec<f32> {
    (0..n).map(|i| 100.0 + (i as f32 * 0.5 + seed).sin() * 6.0 + (i as f32) * if up { 1.1 } else { -0.9 }).collect()
}

struct Screen {
    names: Vec<&'static str>,
    price: Vec<String>,
    day: Vec<String>,
    trend: Vec<Vec<f32>>,
    cap: Vec<String>,
    pe: Vec<String>,
    roce: Vec<String>,
}

impl Screen {
    fn new() -> Screen {
        let names = vec!["PERSISTENT", "TATAELXSI", "COFORGE", "KPITTECH", "LTIM", "MPHASIS", "INFY", "TCS", "HCLTECH", "WIPRO", "TECHM", "LTTS"];
        let n = names.len();
        Screen {
            price: (0..n).map(|i| format!("{:.2}", 5212.0 - i as f32 * 311.7)).collect(),
            day: (0..n).map(|i| format!("{}{:.1}%", if i % 3 == 1 { "−" } else { "+" }, 0.4 + i as f32 * 0.23)).collect(),
            trend: (0..n).map(|i| wave(40, i as f32, i % 3 != 1)).collect(),
            cap: (0..n).map(|i| format!("{:.0}", 81204.0 - i as f32 * 5900.0)).collect(),
            pe: (0..n).map(|i| format!("{:.1}", 62.0 - i as f32 * 3.4)).collect(),
            roce: (0..n).map(|i| format!("{:.1}%", 31.0 - i as f32 * 1.3)).collect(),
            names,
        }
    }
}

impl GridData for Screen {
    fn rows(&self) -> usize {
        self.names.len()
    }

    fn cell(&self, row: usize, col: usize) -> Cell<'_> {
        match col {
            0 => Cell::Strong(self.names[row]),
            1 => Cell::Text(&self.price[row]),
            2 => Cell::Toned(&self.day[row], if row % 3 == 1 { Tone::Danger } else { Tone::Success }),
            3 => Cell::Spark(&self.trend[row], if row % 3 == 1 { Tone::Danger } else { Tone::Success }),
            4 => Cell::Range { low: 100.0, high: 200.0, value: 100.0 + (row * 9 % 100) as f32 },
            5 => Cell::Text(&self.cap[row]),
            6 => Cell::Text(&self.pe[row]),
            7 => Cell::Text(&self.roce[row]),
            8 => Cell::Bar { fraction: 0.2 + (row * 7 % 10) as f32 / 14.0, label: "48%", tone: Tone::Accent },
            _ => Cell::Empty,
        }
    }
}

fn grid_columns() -> Vec<Column> {
    vec![
        Column::new("Symbol", 140.0).sortable().grow(1.0),
        Column::new("Price", 96.0).numeric().sortable(),
        Column::new("1d", 80.0).numeric().sortable(),
        Column::new("1y", 112.0),
        Column::new("52w range", 132.0),
        Column::new("M.cap, cr", 104.0).numeric().sortable(),
        Column::new("P/E", 72.0).numeric().sortable(),
        Column::new("ROCE", 80.0).numeric().sortable(),
    ]
}

fn wide_columns() -> Vec<Column> {
    let mut c = grid_columns();
    c[0] = Column::new("Symbol", 140.0).sortable();
    c.push(Column::new("Delivery", 128.0).numeric());
    c.push(Column::new("", 40.0));
    c
}

/// A frame of the app's own UI in a theme, rendered as the goldens are.
fn frame(theme: Theme, w: f32, h: f32, view: impl IntoElement<()>, hover: Option<(f32, f32)>) -> vui_render::Image {
    let mut t = TestUi::<()>::with_theme(size(w, h), theme, 2.0);
    t.view(view);
    let _ = t.scene();
    if let Some((x, y)) = hover {
        t.move_to(pt(x, y));
    }
    t.settle();
    render_scene(&t.scene(), &Fonts::bundled_only())
}

fn grid_view(columns: Vec<Column>) -> impl IntoElement<()> {
    let grid = data_grid::<()>(Rc::new(Screen::new()), columns).pinned(1).sorted(7, Order::Descending).selected(Some(2)).key("grid").fill();
    column().padding(16.0).child(grid)
}

#[test]
fn the_data_grid_in_light() {
    assert_golden!("grid_light", frame(Theme::light(), 860.0, 520.0, grid_view(grid_columns()), Some((300.0, 190.0))));
}

#[test]
fn the_data_grid_in_dark() {
    assert_golden!("grid_dark", frame(Theme::dark(), 860.0, 520.0, grid_view(grid_columns()), Some((300.0, 190.0))));
}

#[test]
fn the_data_grid_scrolled_with_its_pinned_column() {
    let mut t = TestUi::<()>::with_theme(size(520.0, 360.0), Theme::light(), 2.0);
    t.view(grid_view(wide_columns()));
    let _ = t.scene();
    t.wheel(pt(300.0, 200.0), -300.0, 0.0);
    t.settle();
    assert_golden!("grid_scrolled", render_scene(&t.scene(), &Fonts::bundled_only()));
}

/// NIFTY 50 as sectors of tiles: weight is market value, colour the day's move.
fn nifty() -> Treemap {
    let sectors: [(&str, &[(&str, f64)]); 9] = [
        ("Banks", &[("HDFCBANK", 1330.0), ("ICICIBANK", 940.0), ("SBIN", 710.0), ("KOTAKBANK", 400.0), ("AXISBANK", 360.0), ("INDUSINDBK", 80.0)]),
        ("IT", &[("TCS", 1400.0), ("INFY", 780.0), ("HCLTECH", 490.0), ("WIPRO", 290.0), ("TECHM", 140.0), ("LTIM", 160.0)]),
        ("Energy", &[("RELIANCE", 1950.0), ("ONGC", 340.0), ("NTPC", 330.0), ("POWERGRID", 290.0), ("COALINDIA", 260.0), ("BPCL", 120.0)]),
        ("Auto", &[("M&M", 370.0), ("MARUTI", 390.0), ("TATAMOTORS", 330.0), ("BAJAJ-AUTO", 240.0), ("EICHERMOT", 140.0), ("HEROMOTOCO", 100.0)]),
        ("FMCG", &[("ITC", 570.0), ("HINDUNILVR", 580.0), ("NESTLEIND", 220.0), ("BRITANNIA", 130.0), ("TATACONSUM", 100.0)]),
        ("Pharma", &[("SUNPHARMA", 410.0), ("CIPLA", 120.0), ("DRREDDY", 100.0), ("DIVISLAB", 150.0), ("APOLLOHOSP", 90.0)]),
        ("Metals", &[("TATASTEEL", 190.0), ("JSWSTEEL", 220.0), ("HINDALCO", 150.0), ("ADANIENT", 330.0)]),
        ("Financials", &[("BAJFINANCE", 450.0), ("BAJAJFINSV", 260.0), ("SBILIFE", 160.0), ("HDFCLIFE", 140.0)]),
        ("Others", &[("LT", 480.0), ("BHARTIARTL", 900.0), ("ASIANPAINT", 220.0), ("TITAN", 300.0), ("ULTRACEMCO", 290.0), ("GRASIM", 160.0)]),
    ];
    let mut m = Treemap::new().cap(0.025);
    let mut k = 0.0f64;
    for (name, tiles) in sectors {
        m = m.group(name);
        for (symbol, cap) in tiles {
            k += 1.0;
            let change = ((k * 1.7).sin() * 0.021) + ((k * 0.37).cos() * 0.004);
            m = m.tile(symbol, &format!("{}{:.1}%", if change < 0.0 { "−" } else { "+" }, change.abs() * 100.0), *cap, change);
        }
    }
    m
}

fn treemap_view(hover: bool) -> impl IntoElement<()> {
    let map = treemap::<()>(Rc::new(nifty())).selected(Some(0)).key("map").fill();
    let _ = hover;
    column().padding(16.0).child(map)
}

#[test]
fn the_treemap_in_light() {
    assert_golden!("treemap_light", frame(Theme::light(), 860.0, 520.0, treemap_view(true), Some((400.0, 120.0))));
}

#[test]
fn the_treemap_in_dark() {
    assert_golden!("treemap_dark", frame(Theme::dark(), 860.0, 520.0, treemap_view(true), Some((400.0, 120.0))));
}

/// Tiles a few points short of their label: the name steps down a size or breaks at its space, and
/// only a tile with no room for either goes bare.
fn narrow_tiles() -> impl IntoElement<()> {
    let map = Treemap::new().cap(0.02).group("Stocks").tile("HDFCBANK", "+0.5%", 10.0, 0.005).tile("NIFTY BANK", "−0.3%", 10.0, -0.003).tile("ICICIBANK", "+1.1%", 10.0, 0.011).tile(
        "INDUSINDBANK",
        "−1.4%",
        1.0,
        -0.014,
    );
    column().padding(16.0).child(treemap::<()>(Rc::new(map)).key("map").fill())
}

#[test]
fn the_treemap_fits_labels_to_narrow_tiles() {
    assert_golden!("treemap_narrow", frame(Theme::light(), 262.0, 190.0, narrow_tiles(), None));
}

/// One metric of the filter list: its name and the chosen range over the spread of the whole market.
fn metric(name: &str, values: &[f64], low: f64, high: f64, mark: f64) -> impl IntoElement<()> {
    let h = Histogram::of(values, 28).lit(low, high).marker(Some(mark));
    let (min, max) = (h.min, h.max);
    column()
        .gap(4.0)
        .child(row().justify(Justify::SpaceBetween).child(label(name).weight(500)).child(label(format!("{low:.1} – {high:.1}")).color(Tone::Text2)))
        .child(histogram::<()>(Rc::new(h)).height(56.0))
        .child(range_slider::<()>(low, high, min, max).step((max - min) / 100.0).name(name))
}

/// A spread of numbers shaped like a market: a lump low down and a long tail.
fn spread(seed: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let x = (i as f64 + seed) * 0.618_033_988;
            let u = x - x.floor();
            let v = (u * 6.0 - 3.0).abs();
            8.0 + v * v * 4.0 + (x * 7.0).sin() * 3.0
        })
        .collect()
}

fn filter_view() -> impl IntoElement<()> {
    column().padding(16.0).gap(24.0).width(300.0).child(metric("P/E", &spread(1.0, 2300), 12.0, 38.0, 24.5)).child(metric("ROCE, %", &spread(5.0, 2300), 15.0, 60.0, 31.0)).child(metric(
        "Debt to equity",
        &spread(9.0, 2300),
        0.0,
        40.0,
        12.0,
    ))
}

#[test]
fn the_histogram_slider_in_light() {
    assert_golden!("histogram_light", frame(Theme::light(), 332.0, 380.0, filter_view(), Some((90.0, 100.0))));
}

#[test]
fn the_histogram_slider_in_dark() {
    assert_golden!("histogram_dark", frame(Theme::dark(), 332.0, 380.0, filter_view(), Some((90.0, 100.0))));
}

/// The pieces on a soft card, as the research window sets them: a spread in a data tone (nothing
/// here can be moved, so nothing is the accent), and a grid whose header and pinned column are
/// filled with the card's own faint tint, so no white strip shows where a column slides under.
fn card_view() -> impl IntoElement<()> {
    let tint = Fill::Tone(Tone::Text, 0.02);
    let h = Histogram::of(&spread(3.0, 400), 24).marker(Some(18.0));
    let grid = data_grid::<()>(Rc::new(Screen::new()), wide_columns()).pinned(1).surface(tint).key("grid").height(36.0 + 6.0 * 32.0).shrink(0.0);
    column().padding(16.0).child(
        column()
            .padding(12.0)
            .gap(8.0)
            .rounded(Radius::Card)
            .bg(tint)
            .border(1.0, Fill::Tone(Tone::Text, 0.08))
            .child(label("P/E across 400 companies").color(Tone::Text3).weight(500))
            .child(histogram::<()>(Rc::new(h)).tone(Tone::Blue).height(72.0))
            .child(grid),
    )
}

#[test]
fn a_card_with_a_toned_histogram_and_a_grid_on_its_tint_in_light() {
    let mut t = TestUi::<()>::with_theme(size(520.0, 380.0), Theme::light(), 2.0);
    t.view(card_view());
    let _ = t.scene();
    t.wheel(pt(300.0, 280.0), -200.0, 0.0);
    t.settle();
    assert_golden!("card_light", render_scene(&t.scene(), &Fonts::bundled_only()));
}

#[test]
fn a_card_with_a_toned_histogram_and_a_grid_on_its_tint_in_dark() {
    assert_golden!("card_dark", frame(Theme::dark(), 520.0, 380.0, card_view(), None));
}

fn shareholding() -> Chart {
    let promoters = [54.2, 54.2, 54.1, 54.1, 52.0, 51.9, 51.9, 51.8];
    let fii = [18.0, 18.4, 18.9, 19.6, 20.3, 21.0, 21.2, 21.4];
    let dii = [14.1, 14.3, 14.0, 13.8, 14.5, 14.2, 14.6, 15.2];
    let public: Vec<f64> = (0..8).map(|i| 100.0 - promoters[i] - fii[i] - dii[i]).collect();
    Chart::new()
        .categories(["Dec 22", "Mar 23", "Jun 23", "Sep 23", "Dec 23", "Mar 24", "Jun 24", "Sep 24"])
        .bars("Promoters", &indexed(&promoters))
        .ink(Ink::Hue)
        .bars("FII", &indexed(&fii))
        .ink(Ink::Hue)
        .bars("DII", &indexed(&dii))
        .ink(Ink::Hue)
        .bars("Public", &indexed(&public))
        .ink(Ink::Hue)
        .percent()
}

/// Ten years of an index's P/E against its normal range.
fn valuation() -> Chart {
    let t0 = 1_451_606_400.0;
    let month = 2_629_800.0;
    let pe: Vec<(f64, f64)> = (0..120).map(|i| (t0 + i as f64 * month, 22.5 + 4.5 * (i as f64 / 11.0).sin() + 1.6 * (i as f64 / 3.3).sin() + i as f64 * 0.01)).collect();
    let end = pe.last().unwrap().0;
    let (median, sigma) = (22.9, 3.4);
    Chart::new()
        .time()
        .band("Normal range", &[(t0, median - sigma, median + sigma), (end, median - sigma, median + sigma)])
        .ink(Ink::Soft)
        .line("Median", &[(t0, median), (end, median)])
        .ink(Ink::Mid)
        .dashed()
        .line("P/E", &pe)
        .ink(Ink::Accent)
        .tags("Today", &[Tag::new(end, pe.last().unwrap().1, &format!("{:.1}", pe.last().unwrap().1))])
        .ink(Ink::Accent)
}

/// Sectors turning round the benchmark: tinted quadrants, a trail behind each head, the heads named.
fn rotation() -> Chart {
    let amber = Color::rgb(0.86, 0.60, 0.10);
    let sectors: [(&str, f64, f64, f64); 9] = [
        ("IT", 101.2, 100.6, 0.5),
        ("Banks", 98.4, 100.9, 1.0),
        ("Auto", 102.6, 101.8, 0.6),
        ("FMCG", 99.1, 98.2, 0.7),
        ("Pharma", 101.9, 99.0, 0.5),
        ("Metals", 103.1, 102.7, 0.4),
        ("Energy", 97.4, 97.9, 0.8),
        ("Realty", 98.2, 102.3, 0.3),
        ("Media", 100.4, 97.1, 0.2),
    ];
    let mut c = Chart::new()
        .zones("Improving", &[Zone { x0: 96.0, x1: 100.0, low: 100.0, high: 104.0 }])
        .ink(Ink::Blue)
        .zones("Leading", &[Zone { x0: 100.0, x1: 104.0, low: 100.0, high: 104.0 }])
        .ink(Ink::Up)
        .zones("Lagging", &[Zone { x0: 96.0, x1: 100.0, low: 96.0, high: 100.0 }])
        .ink(Ink::Down)
        .zones("Weakening", &[Zone { x0: 100.0, x1: 104.0, low: 96.0, high: 100.0 }])
        .ink(Ink::Color(amber));
    for (name, x, y, _) in sectors {
        // Four steps back along a gentle curve towards where it is now.
        let trail: Vec<(f64, f64)> = (0..5)
            .map(|k| {
                let t = (4 - k) as f64;
                (x - t * 0.35 + (t * 0.9).sin() * 0.25, y - t * 0.3 + (t * 0.7).cos() * 0.2 - 0.2)
            })
            .collect();
        c = c.line(name, &trail).ink(Ink::Soft);
    }
    let quadrant = |x: f64, y: f64| {
        if x >= 100.0 {
            if y >= 100.0 { 0 } else { 1 }
        } else if y >= 100.0 {
            2
        } else {
            3
        }
    };
    let inks = [Ink::Up, Ink::Color(amber), Ink::Blue, Ink::Down];
    for (q, ink) in inks.into_iter().enumerate() {
        let heads: Vec<Tag> = sectors.iter().filter(|s| quadrant(s.1, s.2) == q).map(|s| Tag::new(s.1, s.2, s.0).size(s.3)).collect();
        c = c.tags("Sectors", &heads).ink(ink);
    }
    c.tags("Quadrants", &[Tag::new(104.0, 104.0, "Leading").note(), Tag::new(104.0, 96.0, "Weakening").note(), Tag::new(96.0, 104.0, "Improving").note(), Tag::new(96.0, 96.0, "Lagging").note()])
        .ink(Ink::Mid)
        .x_range(96.0, 104.0)
        .y_range(96.0, 104.0)
        .x_title("Relative strength")
        .y_title("Momentum")
}

fn chart_section(title: &str, key: &str, legend_row: Option<Legend>, c: Chart, height: f32) -> impl IntoElement<()> {
    let mut col = column().gap(8.0).child(label(title).weight(600));
    if let Some(l) = legend_row {
        col = col.child(legend::<()>(l));
    }
    col.child(chart::<()>(Rc::new(c)).still().key(key).height(height))
}

fn chart_sheet() -> impl IntoElement<()> {
    let holders = Legend::new().item("Promoters", Ink::Hue, 0).item("FII", Ink::Hue, 1).item("DII", Ink::Hue, 2).item("Public", Ink::Hue, 3);
    column()
        .padding(16.0)
        .gap(24.0)
        .width(592.0)
        .child(chart_section("Shareholding", "holders", Some(holders), shareholding(), 180.0))
        .child(chart_section("NIFTY 50 P/E", "pe", Some(Legend::new().item("P/E", Ink::Accent, 2).item("Median", Ink::Mid, 1).item("Normal range", Ink::Soft, 0)), valuation(), 180.0))
        .child(chart_section("Sector rotation", "rrg", None, rotation(), 320.0))
}

#[test]
fn the_research_charts_in_light() {
    assert_golden!("research_charts_light", frame(Theme::light(), 592.0, 1000.0, chart_sheet(), None));
}

#[test]
fn the_research_charts_in_dark() {
    assert_golden!("research_charts_dark", frame(Theme::dark(), 592.0, 1000.0, chart_sheet(), None));
}

fn offer_steps() -> Vec<Step> {
    vec![
        Step::new("Opens", "Mon, 24 Nov", When::Done).note("Bids from 10:00 on all three days"),
        Step::new("Closes", "Wed, 26 Nov", When::Now).note("Bids stop at 17:00; UPI mandates by 17:00"),
        Step::new("Allotment", "Thu, 27 Nov", When::Next).note("Check status on the registrar's site"),
        Step::new("Refund and credit", "Fri, 28 Nov", When::Next).note("Shares to demat, unallotted money back"),
        Step::new("Listing", "Mon, 1 Dec", When::Next).note("Trading starts at 10:00"),
    ]
}

fn timeline_view() -> impl IntoElement<()> {
    column().padding(24.0).gap(32.0).width(640.0).child(timeline::<()>(offer_steps())).child(column().width(300.0).child(timeline::<()>(offer_steps()).vertical()))
}

#[test]
fn the_timeline_in_light() {
    assert_golden!("timeline_light", frame(Theme::light(), 688.0, 500.0, timeline_view(), None));
}

#[test]
fn the_timeline_in_dark() {
    assert_golden!("timeline_dark", frame(Theme::dark(), 688.0, 500.0, timeline_view(), None));
}

#[test]
fn the_timeline_reads_out_every_step() {
    let mut t = TestUi::<()>::with_theme(size(688.0, 500.0), Theme::light(), 1.0);
    t.view(timeline_view());
    let tree = t.ui.access_tree();
    let names: Vec<String> = tree.nodes.iter().filter(|n| n.role == vui_runtime::Role::ListItem).filter_map(|n| n.name.clone()).collect();
    assert!(names.contains(&"Step 2 of 5: Closes, Wed, 26 Nov, now".to_string()), "{names:?}");
    assert!(names.contains(&"Step 1 of 5: Opens, Mon, 24 Nov, done".to_string()), "{names:?}");
    assert!(names.contains(&"Step 5 of 5: Listing, Mon, 1 Dec".to_string()), "{names:?}");
    assert_eq!(names.len(), 10, "both layouts list all five");
}

/// A screening formula as the research window shows it: names plain, operators quiet, numbers warm,
/// keywords in the accent, and a wavy line under a name that is not a metric.
fn formula_view() -> impl IntoElement<()> {
    let text = "roce > 15 and pe < 20 or nonsense > 3";
    // What the app does with its lexer's tokens: operators quiet, numbers warm, keywords in the accent,
    // and a wavy line under a name that is not a metric.
    let mut marks = Vec::new();
    let mut at = 0;
    for word in text.split(' ') {
        let range = at..at + word.len();
        at += word.len() + 1;
        marks.push(match word {
            ">" | "<" | ">=" | "<=" => Decoration::new(range).tone(Tone::Text2),
            "and" | "or" | "not" => Decoration::new(range).tone(Tone::Accent),
            w if w.parse::<f64>().is_ok() => Decoration::new(range).tone(Tone::Orange),
            "nonsense" => Decoration::new(range).squiggle(Tone::Danger),
            _ => continue,
        });
    }
    let suggestion = |name: &str, hint: &str, lit: bool| {
        row()
            .justify(Justify::SpaceBetween)
            .padding((8.0, 6.0))
            .rounded(Radius::Control)
            .bg(if lit { Fill::Tone(Tone::Hover, 1.0) } else { Fill::NONE })
            .child(label(name).mono())
            .child(label(hint).color(Tone::Text2))
    };
    column()
        .padding(16.0)
        .gap(8.0)
        .width(520.0)
        .child(text_field::<()>(text).mono().decorate(marks).key("formula").width(488.0))
        .child(label("`nonsense` is not a metric. Did you mean one of these?").color(Tone::Danger))
        .child(
            column()
                .padding(4.0)
                .rounded(Radius::Card)
                .border(1.0, Fill::from(Tone::Line))
                .child(suggestion("net_profit", "Net profit, cr", true))
                .child(suggestion("net_debt", "Net debt, cr", false))
                .child(suggestion("net_worth", "Net worth, cr", false)),
        )
}

#[test]
fn the_formula_field_in_light() {
    assert_golden!("formula_light", frame(Theme::light(), 552.0, 240.0, formula_view(), None));
}

#[test]
fn the_formula_field_in_dark() {
    assert_golden!("formula_dark", frame(Theme::dark(), 552.0, 240.0, formula_view(), None));
}
