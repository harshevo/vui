//! A dashboard: a sidebar, stat cards that wrap, a chart and a table, in light or dark. Narrow the
//! window to see it reflow: the sidebar gives way to a top bar, the cards stack and the table drops
//! the columns it can do without.
//!
//! ```sh
//! cargo run -p vui --example dashboard
//! ```

use std::rc::Rc;

use vui::prelude::*;
use vui::viz::chart::chart;
use vui::viz::{Chart, Format, Ink, spark};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Go(Section),
    Range(Range),
    Dark(bool),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Section {
    Overview,
    Orders,
    Customers,
    Settings,
}

impl Section {
    const ALL: [Section; 4] = [Section::Overview, Section::Orders, Section::Customers, Section::Settings];

    fn name(self) -> &'static str {
        match self {
            Section::Overview => "Overview",
            Section::Orders => "Orders",
            Section::Customers => "Customers",
            Section::Settings => "Settings",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Section::Overview => icon_set::LAYOUT,
            Section::Orders => icon_set::RECEIPT,
            Section::Customers => icon_set::USERS,
            Section::Settings => icon_set::SETTINGS,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Range {
    Week,
    Month,
    Quarter,
}

impl Range {
    fn days(self) -> usize {
        match self {
            Range::Week => 7,
            Range::Month => 30,
            Range::Quarter => 90,
        }
    }
}

pub struct Dashboard {
    pub section: Section,
    pub range: Range,
    pub dark: bool,
    revenue: Rc<Chart>,
}

/// Made-up but steady daily revenue: a trend, a weekly rhythm and a little noise.
fn daily_revenue(days: usize) -> Vec<(f64, f64)> {
    (0..days)
        .map(|d| {
            let t = d as f64;
            let weekly = (t * std::f64::consts::TAU / 7.0).sin() * 600.0;
            let noise = ((t * 12.9898).sin() * 43_758.545).fract() * 400.0;
            (t, 8_000.0 + t * 45.0 + weekly + noise)
        })
        .collect()
}

fn revenue_chart(range: Range) -> Rc<Chart> {
    let points = daily_revenue(range.days());
    let target: Vec<(f64, f64)> = points.iter().map(|&(x, _)| (x, 8_400.0 + x * 40.0)).collect();
    Rc::new(Chart::new().area("Revenue", &points).ink(Ink::Accent).line("Target", &target).ink(Ink::Mid).dashed().y_format(Format::Fixed(0)).from_zero())
}

impl Dashboard {
    pub fn new(dark: bool) -> Dashboard {
        Dashboard { section: Section::Overview, range: Range::Month, dark, revenue: revenue_chart(Range::Month) }
    }
}

impl App for Dashboard {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Go(section) => self.section = section,
            Msg::Range(range) => {
                self.range = range;
                self.revenue = revenue_chart(range);
            }
            Msg::Dark(dark) => self.dark = dark,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let wide_only = |d: Div<Msg>| d.when(Breakpoint::Compact, |s| s.hidden(true));
        row().fill().children((
            wide_only(self.sidebar()),
            wide_only(separator_vertical()),
            column().grow(1.0).min_width(0.0).children((
                self.top_bar(),
                column().key("content").grow(1.0).min_height(0.0).overflow(Overflow::Scroll).child(self.overview()),
            )),
        ))
    }

    fn theme(&self, _system: Mode) -> Theme {
        if self.dark { Theme::dark() } else { Theme::light() }
    }
}

impl Dashboard {
    fn sidebar(&self) -> Div<Msg> {
        let brand = row().align_items(Align::Center).gap(8.0).padding_x(12.0).height(32.0).children((
            div().square(20.0).rounded(Radius::Px(6.0)).bg(Tone::Accent),
            label("Northwind").weight(600),
        ));
        let nav = list().key("nav").children(Section::ALL.map(|s| {
            nav_item(s.icon(), s.name()).key(s.name()).selected(self.section == s).on_click(Msg::Go(s))
        }));
        let dark = row().align_items(Align::Center).gap(8.0).padding_x(12.0).height(32.0).children((
            icon(icon_set::MOON),
            label("Dark").color(Tone::Text2).grow(1.0),
            toggle(self.dark).key("dark").name("Dark mode").on_change(Msg::Dark),
        ));
        // Leave room at the top for the window's buttons, and let the empty part move the window.
        sidebar().key("sidebar").padding_top(44.0).window_drag(true).children((brand, nav, spacer(), dark))
    }

    /// The bar a narrow window shows in place of the sidebar.
    fn top_bar(&self) -> Div<Msg> {
        row()
            .key("top-bar")
            .hidden(true)
            .when(Breakpoint::Compact, |s| s.hidden(false))
            .height(52.0)
            .padding_x(12.0)
            .padding_left(80.0)
            .gap(8.0)
            .align_items(Align::Center)
            .bg(Tone::Side)
            .window_drag(true)
            .children((label("Northwind").weight(600), spacer(), toggle(self.dark).name("Dark mode").on_change(Msg::Dark)))
    }

    fn overview(&self) -> Div<Msg> {
        let range = segmented(
            [(Range::Week, "7 days".to_string()), (Range::Month, "30 days".to_string()), (Range::Quarter, "90 days".to_string())],
            &self.range,
            Msg::Range,
        );
        let header = row().align_items(Align::Center).gap(12.0).wrap().window_drag(true).children((
            column().grow(1.0).gap(2.0).children((title(self.section.name()), small("Sales across every store").color(Tone::Text2))),
            range,
        ));

        let days = self.range.days();
        let revenue: Vec<f64> = daily_revenue(days).into_iter().map(|(_, y)| y).collect();
        let total: f64 = revenue.iter().sum();
        let stats = row().key("stats").gap(16.0).wrap().children((
            stat("Revenue", &format!("${:.1}k", total / 1000.0), "+12.4%", true, &revenue),
            stat("Orders", &format!("{}", days * 37), "+4.1%", true, &revenue.iter().map(|v| v.sqrt()).collect::<Vec<_>>()),
            stat("Refunds", &format!("{}", days / 3 + 2), "-0.8%", false, &revenue.iter().rev().copied().collect::<Vec<_>>()),
        ));

        let chart_card = card().key("chart-card").padding(20.0).gap(12.0).children((
            label("Revenue").weight(600),
            chart(self.revenue.clone()).key("revenue").name("Daily revenue").fill_width().height(240.0),
        ));

        let orders = data_table(
            vec![
                table_column("Customer").flex(2.0),
                table_column("Store").hide_below(Breakpoint::Medium),
                table_column("Status").width(92.0),
                table_column("Total").width(84.0).numeric(),
            ],
            ORDERS.iter().map(|&(who, store, status, total)| {
                let badge = match status {
                    "Paid" => badge(status).tint(BadgeTint::Success),
                    "Refunded" => badge(status).tint(BadgeTint::Danger),
                    _ => badge(status).tint(BadgeTint::Warning),
                };
                table_row((label(who).weight(500).max_lines(1), label(store).color(Tone::Text2), badge, label(format!("${total:.2}"))))
            }),
        );
        let orders_card = card().key("orders-card").padding(20.0).gap(12.0).children((label("Recent orders").weight(600), orders));

        column()
            .key("page")
            .width(pct(100.0))
            .max_width(1100.0)
            .padding(32.0)
            .padding_top(44.0)
            .when(Breakpoint::Compact, |s| s.padding(16.0))
            .gap(20.0)
            .children((header, stats, chart_card, orders_card))
    }
}

/// A stat card. Cards grow to share a row, wrap to a new one below 220 points, and fill the width
/// of a narrow window one above another.
fn stat(name: &str, value: &str, change: &str, up: bool, trend: &[f64]) -> Div<Msg> {
    let (glyph, tone) = if up { (icon_set::TRENDING_UP, Tone::Success) } else { (icon_set::TRENDING_DOWN, Tone::Danger) };
    card().grow(1.0).basis(220.0).min_width(220.0).padding(20.0).gap(8.0).children((
        small(name.to_string()).color(Tone::Text2),
        row().align_items(Align::End).gap(8.0).children((
            label(value.to_string()).font_size(28.0).weight(600),
            row().gap(4.0).align_items(Align::Center).padding_bottom(6.0).children((icon(glyph).color(tone), small(change.to_string()).color(tone))),
        )),
        spark(trend).ink(if up { Ink::Accent } else { Ink::Mid }).name(format!("{name} trend")).fill_width().height(36.0),
    ))
}

const ORDERS: [(&str, &str, &str, f64); 5] = [
    ("Ada Lovelace", "Camden", "Paid", 128.40),
    ("Grace Hopper", "Arlington", "Pending", 62.00),
    ("Alan Turing", "Wilmslow", "Paid", 245.10),
    ("Katherine Johnson", "Hampton", "Refunded", 39.99),
    ("Edsger Dijkstra", "Austin", "Paid", 87.25),
];

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
fn main() {
    let window = WindowOptions::new("Dashboard").size(size(1180.0, 820.0)).min_size(size(360.0, 480.0));
    if let Err(e) = vui::run(window, |_resources| Dashboard::new(false)) {
        eprintln!("{e}");
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn main() {
    eprintln!("vui has no window backend for this platform; the app still runs headless in tests.");
}
