//! Charts: a list of layers over shared axes. Describe the data here; [`draw`] turns it into scene
//! commands and [`probe`] answers what is under a pointer. Nothing in this module knows about windows.

mod decimate;
mod draw;
mod probe;
mod widget;

pub use decimate::extremes;
pub use draw::{Drawn, View, draw, layout};
pub use probe::{Probe, Row, draw_probe, probe};
pub use widget::{ChartView, chart};

use crate::format::Format;
use crate::scale::tick_step;
use crate::style::Ink;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Candle {
    pub x: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    /// Any non-negative number; dots are sized by their share of the largest.
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Line {
        points: Vec<(f64, f64)>,
        step: bool,
        dashed: bool,
    },
    Area {
        points: Vec<(f64, f64)>,
        base: f64,
    },
    Bars {
        points: Vec<(f64, f64)>,
        base: f64,
    },
    Dots(Vec<Dot>),
    Candles(Vec<Candle>),
}

/// Which strip of the chart a layer sits in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Panel {
    Main,
    /// A short strip under the main one with its own vertical scale, for volume.
    Strip,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub mark: Mark,
    pub ink: Ink,
    pub panel: Panel,
    pub format: Option<Format>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub layers: Vec<Layer>,
    /// The horizontal axis counts seconds since 1970 and is labelled with dates.
    pub time: bool,
    pub y_format: Format,
    /// Keep zero on the vertical axis even when the data stays far from it.
    pub zero: bool,
}

impl Default for Chart {
    fn default() -> Chart {
        Chart::new()
    }
}

impl Chart {
    pub fn new() -> Chart {
        Chart {
            layers: Vec::new(),
            time: false,
            y_format: Format::Auto,
            zero: false,
        }
    }

    pub fn time(mut self) -> Chart {
        self.time = true;
        self
    }

    pub fn y_format(mut self, format: Format) -> Chart {
        self.y_format = format;
        self
    }

    pub fn from_zero(mut self) -> Chart {
        self.zero = true;
        self
    }

    fn push(mut self, name: &str, mark: Mark, ink: Ink, panel: Panel) -> Chart {
        self.layers.push(Layer {
            name: name.to_string(),
            mark,
            ink,
            panel,
            format: None,
        });
        self
    }

    pub fn line(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Line {
                points: clean(points),
                step: false,
                dashed: false,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    pub fn step(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Line {
                points: clean(points),
                step: true,
                dashed: false,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    pub fn area(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Area {
                points: clean(points),
                base: 0.0,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    pub fn bars(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Bars {
                points: clean(points),
                base: 0.0,
            },
            Ink::Auto,
            Panel::Main,
        )
    }

    /// Bars in the strip under the chart, in a quiet ink.
    pub fn volume(self, name: &str, points: &[(f64, f64)]) -> Chart {
        self.push(
            name,
            Mark::Bars {
                points: clean(points),
                base: 0.0,
            },
            Ink::Soft,
            Panel::Strip,
        )
    }

    pub fn dots(self, name: &str, dots: &[Dot]) -> Chart {
        let mut dots: Vec<Dot> = dots
            .iter()
            .copied()
            .filter(|d| d.x.is_finite() && d.y.is_finite())
            .collect();
        dots.sort_by(|a, b| a.x.total_cmp(&b.x));
        self.push(name, Mark::Dots(dots), Ink::Auto, Panel::Main)
    }

    pub fn candles(self, name: &str, rows: &[Candle]) -> Chart {
        let mut rows: Vec<Candle> = rows
            .iter()
            .copied()
            .filter(|c| {
                [c.x, c.open, c.high, c.low, c.close]
                    .iter()
                    .all(|v| v.is_finite())
            })
            .collect();
        rows.sort_by(|a, b| a.x.total_cmp(&b.x));
        self.push(name, Mark::Candles(rows), Ink::Strong, Panel::Main)
    }

    /// Sets how the most recently added layer is painted.
    pub fn ink(mut self, ink: Ink) -> Chart {
        if let Some(l) = self.layers.last_mut() {
            l.ink = ink;
        }
        self
    }

    pub fn dashed(mut self) -> Chart {
        if let Some(Layer {
            mark: Mark::Line { dashed, .. },
            ..
        }) = self.layers.last_mut()
        {
            *dashed = true;
        }
        self
    }

    /// Sets how the most recently added layer's values are written in the probe.
    pub fn format(mut self, format: Format) -> Chart {
        if let Some(l) = self.layers.last_mut() {
            l.format = Some(format);
        }
        self
    }

    pub fn has_strip(&self) -> bool {
        self.layers.iter().any(|l| l.panel == Panel::Strip)
    }

    /// The horizontal range of all the data, widened by half a bar or candle at each end (and a little for
    /// dots) so those marks are not cut in half.
    pub fn x_extent(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for l in &self.layers {
            if let Some((a, b)) = l.mark.x_bounds() {
                let pad = if l.mark.is_thick() {
                    l.mark.spacing() / 2.0
                } else if matches!(l.mark, Mark::Dots(_)) {
                    (b - a) * 0.04
                } else {
                    0.0
                };
                lo = lo.min(a - pad);
                hi = hi.max(b + pad);
            }
        }
        if lo > hi {
            (0.0, 1.0)
        } else if lo == hi {
            (lo - 1.0, hi + 1.0)
        } else {
            (lo, hi)
        }
    }

    /// The narrowest window worth zooming to: a few of the finest layer's steps.
    pub fn min_span(&self) -> f64 {
        let finest = self
            .layers
            .iter()
            .map(|l| l.mark.spacing())
            .fold(f64::INFINITY, f64::min);
        if finest.is_finite() {
            finest * 4.0
        } else {
            1.0
        }
    }

    /// The vertical range a panel needs to show every mark whose x falls inside `x`, with a little
    /// room above and below and round numbers at the ends.
    pub fn y_extent(&self, panel: Panel, x: (f64, f64), ticks: usize) -> (f64, f64) {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut anchored = self.zero && panel == Panel::Main;
        for l in self.layers.iter().filter(|l| l.panel == panel) {
            if let Some((a, b)) = l.mark.y_bounds(x) {
                lo = lo.min(a);
                hi = hi.max(b);
            }
            if let Some(base) = l.mark.base() {
                lo = lo.min(base);
                hi = hi.max(base);
                anchored = true;
            }
        }
        if lo > hi {
            return (0.0, 1.0);
        }
        if lo == hi {
            let room = if lo == 0.0 { 1.0 } else { lo.abs() * 0.05 };
            lo -= room;
            hi += room;
        }
        let pad = (hi - lo) * 0.06;
        let top = hi + pad;
        let bottom = if anchored && lo >= 0.0 {
            lo.min(0.0)
        } else {
            lo - pad
        };
        let step = tick_step(bottom, top, ticks);
        if step > 0.0 && step.is_finite() {
            ((bottom / step).floor() * step, (top / step).ceil() * step)
        } else {
            (bottom, top)
        }
    }
}

impl Mark {
    /// Marks with a width in data units: they need room either side of their x.
    fn is_thick(&self) -> bool {
        matches!(self, Mark::Bars { .. } | Mark::Candles(_))
    }

    fn base(&self) -> Option<f64> {
        match self {
            Mark::Area { base, .. } | Mark::Bars { base, .. } => Some(*base),
            _ => None,
        }
    }

    pub(crate) fn xs(&self) -> Vec<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                points.iter().map(|p| p.0).collect()
            }
            Mark::Dots(d) => d.iter().map(|d| d.x).collect(),
            Mark::Candles(c) => c.iter().map(|c| c.x).collect(),
        }
    }

    pub(crate) fn x_bounds(&self) -> Option<(f64, f64)> {
        let (first, last) = match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                (points.first()?.0, points.last()?.0)
            }
            Mark::Dots(d) => (d.first()?.x, d.last()?.x),
            Mark::Candles(c) => (c.first()?.x, c.last()?.x),
        };
        Some((first, last))
    }

    /// The smallest gap between neighbouring x values: how wide a bar or candle may be.
    pub(crate) fn spacing(&self) -> f64 {
        let xs = self.xs();
        let gap = xs
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|g| *g > 0.0)
            .fold(f64::INFINITY, f64::min);
        if gap.is_finite() { gap } else { 1.0 }
    }

    /// The vertical range of the marks that fall inside `x`, counting the one just outside each side so
    /// a line entering the view is not cut short.
    pub(crate) fn y_bounds(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        let range = |ys: &mut dyn Iterator<Item = f64>| {
            ys.fold(None, |acc: Option<(f64, f64)>, y| {
                Some(acc.map_or((y, y), |(a, b)| (a.min(y), b.max(y))))
            })
        };
        match self {
            Mark::Line { points, .. } | Mark::Area { points, .. } | Mark::Bars { points, .. } => {
                range(&mut within(points, |p| p.0, x).iter().map(|p| p.1))
            }
            Mark::Dots(d) => range(&mut within(d, |d| d.x, x).iter().map(|d| d.y)),
            Mark::Candles(c) => {
                range(&mut within(c, |c| c.x, x).iter().flat_map(|c| [c.high, c.low]))
            }
        }
    }
}

/// The items of an x-sorted slice whose key falls inside `x`, plus one neighbour either side.
pub(crate) fn within<T>(items: &[T], key: impl Fn(&T) -> f64, x: (f64, f64)) -> &[T] {
    let start = items.partition_point(|t| key(t) < x.0).saturating_sub(1);
    let end = (items.partition_point(|t| key(t) <= x.1) + 1).min(items.len());
    &items[start.min(end)..end]
}

/// Drops points that are not numbers and puts the rest in x order, which every mark relies on.
fn clean(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|p| p.0.is_finite() && p.1.is_finite())
        .collect();
    if !points.windows(2).all(|w| w[0].0 <= w[1].0) {
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Vec<(f64, f64)> {
        (0..n).map(|i| (i as f64, 100.0 + i as f64)).collect()
    }

    #[test]
    fn bad_numbers_are_dropped_and_points_come_out_in_order() {
        let c = Chart::new().line(
            "a",
            &[
                (2.0, 1.0),
                (f64::NAN, 3.0),
                (0.0, 5.0),
                (1.0, f64::INFINITY),
            ],
        );
        match &c.layers[0].mark {
            Mark::Line { points, .. } => assert_eq!(points, &[(0.0, 5.0), (2.0, 1.0)]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn x_extent_gives_bars_half_a_bar_at_each_end() {
        let line = Chart::new().line("a", &ramp(11));
        assert_eq!(line.x_extent(), (0.0, 10.0));
        let bars = Chart::new().bars("a", &ramp(11));
        assert_eq!(bars.x_extent(), (-0.5, 10.5));
        assert_eq!(Chart::new().x_extent(), (0.0, 1.0));
    }

    #[test]
    fn y_extent_anchors_bars_at_zero_and_floats_lines() {
        let (lo, hi) = Chart::new()
            .bars("a", &ramp(10))
            .y_extent(Panel::Main, (0.0, 9.0), 5);
        assert_eq!(lo, 0.0);
        assert!(hi >= 109.0);
        let (lo, hi) = Chart::new()
            .line("a", &ramp(10))
            .y_extent(Panel::Main, (0.0, 9.0), 5);
        assert!(
            lo > 50.0 && lo <= 100.0,
            "a line far from zero is not stretched down to it: {lo}"
        );
        assert!(hi >= 109.0);
        let (lo, _) =
            Chart::new()
                .line("a", &ramp(10))
                .from_zero()
                .y_extent(Panel::Main, (0.0, 9.0), 5);
        assert_eq!(lo, 0.0);
    }

    #[test]
    fn y_extent_follows_the_visible_window_and_a_flat_series_gets_room() {
        let mut pts = ramp(100);
        pts[90].1 = 1000.0;
        let c = Chart::new().line("a", &pts);
        let (_, hi_all) = c.y_extent(Panel::Main, (0.0, 99.0), 5);
        let (_, hi_early) = c.y_extent(Panel::Main, (0.0, 20.0), 5);
        assert!(hi_all >= 1000.0 && hi_early < 200.0, "{hi_all} {hi_early}");
        let (lo, hi) = Chart::new()
            .line("flat", &[(0.0, 5.0), (1.0, 5.0)])
            .y_extent(Panel::Main, (0.0, 1.0), 5);
        assert!(lo < 5.0 && hi > 5.0);
    }

    #[test]
    fn panels_have_separate_ranges() {
        let c = Chart::new()
            .line("price", &ramp(10))
            .volume("vol", &[(0.0, 5_000_000.0), (9.0, 8_000_000.0)]);
        assert!(c.has_strip());
        let (_, main_hi) = c.y_extent(Panel::Main, (0.0, 9.0), 5);
        let (_, strip_hi) = c.y_extent(Panel::Strip, (0.0, 9.0), 3);
        assert!(main_hi < 200.0 && strip_hi >= 8_000_000.0);
    }

    #[test]
    fn within_keeps_one_neighbour_either_side() {
        let pts = ramp(10);
        let seen = within(&pts, |p| p.0, (3.5, 5.5));
        assert_eq!(seen.first().unwrap().0, 3.0);
        assert_eq!(seen.last().unwrap().0, 6.0);
        assert_eq!(
            within(&pts, |p| p.0, (100.0, 200.0)).len(),
            1,
            "the last point, so a line can enter from the edge"
        );
        assert!(within::<(f64, f64)>(&[], |p| p.0, (0.0, 1.0)).is_empty());
    }

    #[test]
    fn candles_span_high_to_low() {
        let rows = [
            Candle {
                x: 0.0,
                open: 10.0,
                high: 14.0,
                low: 8.0,
                close: 12.0,
            },
            Candle {
                x: 1.0,
                open: 12.0,
                high: 13.0,
                low: 5.0,
                close: 6.0,
            },
        ];
        let c = Chart::new().candles("px", &rows);
        let (lo, hi) = c.y_extent(Panel::Main, (0.0, 1.0), 5);
        assert!(lo <= 5.0 && hi >= 14.0);
        assert_eq!(c.x_extent(), (-0.5, 1.5));
    }
}
