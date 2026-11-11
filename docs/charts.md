# Charts and graphs

`vui::viz` draws data with vui's own vector engine. Data goes in, triangles come out, and there's
no plotting library underneath. It has:

- **Charts:** line, step, area, bars (grouped, stacked, over named categories), volume, scatter
  and candlesticks, with axes that pick round numbers.
- **Pies and donuts,** and **bar lists**: horizontal bars ranking things.
- **Sparklines:** small trends for cards and tables.
- **Graphs:** a force-directed layout for networks and knowledge graphs, with pan, zoom and picking.

All of them follow the theme, in light and dark, and are readable by screen readers.

## Charts

Build a `Chart` from layers, share it with `Rc`, and show it with `chart(…)`. The `Rc` matters:
the widget redraws only when the `Rc` is a new one, so rebuilding the view doesn't copy your data.

```rust
use std::rc::Rc;
use vui::prelude::*;
use vui::viz::{Chart, Format, Ink, chart};

let visits: Vec<(f64, f64)> = (0..60).map(|d| (d as f64, 1200.0 + (d as f64 * 0.3).sin() * 300.0)).collect();
let signups: Vec<(f64, f64)> = (0..60).map(|d| (d as f64, 80.0 + d as f64 * 2.0)).collect();

let data = Rc::new(
    Chart::new()
        .area("Visits", &visits).ink(Ink::Accent)
        .line("Sign-ups", &signups).dashed()
        .y_format(Format::Si)
        .from_zero(),
);

let view: Div<()> = card().padding(20.0).child(
    chart(data).name("Visits and sign-ups").fill_width().height(260.0),
);
```

| Layer | For |
| --- | --- |
| `.line(name, &points)` | a series over x |
| `.step(name, &points)` | values that hold until they change |
| `.area(name, &points)` | a filled line: totals, volumes |
| `.bars(name, &points)` | counts per bucket |
| `.volume(name, &points)` | bars in a strip under the main panel, as under a price chart |
| `.dots(name, &[Dot { x, y, size }])` | scatter, sized by value |
| `.candles(name, &[Candle { x, open, high, low, close }])` | open–high–low–close |

Options apply to the layer just added: `.ink(Ink::Accent | Hue | Strong | Mid | Soft | Color(c))`,
`.dashed()` and `.format(…)`. Others apply to the chart:

- **`.time()`** makes x seconds since the epoch, with date and time ticks.
- **`.y_format(Format::…)`** formats the y axis: `Auto`, `Fixed(2)`, `Si` (1.2k, 3.4M),
  `Percent(1)` or `PercentPoints(1)`.
- **`.from_zero()`** keeps 0 on the y axis.
- **`.categories(["Q1", "Q2", …])`** names x = 0, 1, 2…; build the points with `indexed(&values)`.
- **`.grouped()`** puts bar layers side by side; **`.stacked()`** stacks them, positives up and
  negatives down. Give each `Ink::Hue` so they take distinct theme hues.
- **`.market()`** draws candles rising in green and falling in red, instead of hollow and solid.

```rust
use std::rc::Rc;
use vui::prelude::*;
use vui::viz::{Chart, Format, Ink, chart, indexed};

let revenue = Chart::new()
    .categories(["Q1", "Q2", "Q3", "Q4"])
    .y_format(Format::Money("$"))
    .bars("2025", &indexed(&[48.0, 57.0, 61.0, 72.0])).ink(Ink::Hue)
    .bars("2026", &indexed(&[55.0, 60.0, 70.0, 81.0])).ink(Ink::Hue)
    .grouped();
let view: Div<()> = card().child(chart(Rc::new(revenue)).height(240.0));
```

**Interaction:**

- Hovering shows a probe with every series' value.
- Dragging pans, and ⌘-scroll or a pinch zooms.
- Double-clicking shows everything again.
- With keyboard focus, ← and → pan, + and − zoom, and Home shows everything.
- The first time the chart appears, its marks draw in.

`.still()` turns the motion off, for printing or tests.

**Linked charts.** `.on_hover(Msg::Hover)` reports the x under the pointer, and `.guide(x)` draws a
guide at an x from elsewhere. Together they line up several charts at the same moment.

**Large data** is thinned to the extremes per pixel column before drawing, so 100,000 points draw
as fast as 1,000 and no spike is lost.

## Pies, donuts and bar lists

A `Pie` is a list of slices; `.donut()` cuts a hole and writes the total in it. Hovering a slice
(or its legend row) moves it out and writes its value in the hole. The legend sits beside the
circle when there's room and below it when there isn't.

A `BarList` ranks things: a label, a bar sized by its share of the largest, and the value.

```rust
use std::rc::Rc;
use vui::prelude::*;
use vui::viz::{BarList, Format, Pie, bar_list, pie};

let visits = Pie::new()
    .slice("Direct", 4210.0)
    .slice("Search", 3120.0)
    .slice("Social", 1430.0)
    .donut()
    .format(Format::Si)
    .caption("Visits");
let browsers = BarList::new().row("Chrome", 6420.0).row("Safari", 3180.0).row("Firefox", 860.0).sorted();

let view: Div<()> = row().wrap().gap(12.0).children((
    card().grow(1.0).basis(px(320.0)).child(pie(Rc::new(visits))),
    card().grow(1.0).basis(px(320.0)).child(bar_list(Rc::new(browsers)).name("Visitors by browser")),
));
```

Both take `.on_hover(Msg::Over)` with the index under the pointer, and `.still()`. Slices and
bars take `Ink`; without one, slices go round the theme's hues.

## Sparklines

```rust
use vui::prelude::*;
use vui::viz::{Ink, spark};

let trend = [3.0, 4.0, 3.5, 5.0, 6.2, 5.8, 7.1];
let cell: Div<()> = row().gap(8.0).align_items(Align::Center).children((
    label("Weekly active"),
    spark(&trend).ink(Ink::Accent).name("Weekly active, rising").size(96.0, 28.0),
));
```

## Graphs

`graph_view` lays out a graph with a Barnes–Hut force simulation (n log n per step) and keeps it
moving until it settles. Then:

- dragging pans, or pulls a node;
- scrolling or pinching zooms;
- clicking picks a node;
- double-clicking a node opens it, and double-clicking empty space fits everything again.

Nodes have a kind (a colour and a legend entry), a weight (their size) and an optional time span.
`.as_of(t)` shows the graph at a moment, which turns it into a timeline.

```rust
use std::rc::Rc;
use vui::prelude::*;
use vui::viz::graph::{Edge, GraphData, Highlight, Node, graph_view};

#[derive(Clone)]
enum Msg { Pick(Option<u64>), Open(u64) }

let data = Rc::new(
    GraphData::new()
        .kinds(&["Person", "Project"])
        .node(Node::new(1, "Ada", 0).weight(2.0))
        .node(Node::new(2, "Grace", 0))
        .node(Node::new(3, "Compiler", 1).detail("A-0 System, 1952"))
        .edge(Edge::new(2, 3, "built"))
        .edge(Edge::new(1, 2, "inspired").dashed()),
);

let view: Div<Msg> = column().fill().child(
    graph_view(data)
        .selected(Some(3))
        .highlight(Highlight { nodes: vec![2, 3], ..Highlight::default() })
        .on_select(Msg::Pick)
        .on_open(Msg::Open)
        .name("Who built what")
        .fill(),
);
```

The app owns what is picked and what is lit up. The view reports clicks and draws what it's given.

## Drawing your own

`vui::viz::canvas::Canvas` exposes the lower-level pieces for custom charts inside a
[custom widget](custom-widgets.md):

- **Scales:** `Linear`, `Log` and `Band`, with `nice_ticks` and `time_ticks`.
- **Drawing:** polylines, fills, dots and anchored text.
