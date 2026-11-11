//! Data pictures for vui: scales that choose round numbers, axes, charts, and layouts for graphs. The
//! drawing is done with vui's vector engine, so everything here is plain data in and triangles out.

pub mod bar_list;
pub mod canvas;
pub mod chart;
pub mod format;
pub mod graph;
pub mod pie;
pub mod scale;
pub mod spark;
pub mod style;

pub use bar_list::{BarList, BarListView, bar_list, draw_bar_list};
pub use canvas::{Canvas, HAnchor, VAnchor, paint};
pub use chart::{BarLayout, Candle, Chart, ChartView, Dot, Layer, Mark, Panel, chart, indexed};
pub use graph::{GraphData, GraphView, graph_view};
pub use pie::{Pie, PieView, Slice, pie};
pub use format::Format;
pub use scale::{Band, Linear, Log, Tick, nice_ticks, time_ticks};
pub use spark::{Spark, draw_spark, spark};
pub use style::{Ink, VizStyle};
