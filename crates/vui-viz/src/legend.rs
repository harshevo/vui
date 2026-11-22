//! A legend: a swatch and a name for each series of a chart, in a row that wraps when it must. The
//! swatch is resolved the way the chart resolves its layers, so `item("Promoters", Ink::Hue, 0)`
//! matches the chart's first layer drawn with `Ink::Hue`.

use vui_core::{Point, Size, TextStyle, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, PaintCx, Role, Widget, builder};

use crate::canvas::{Canvas, paint};
use crate::style::{Ink, VizStyle};

const SWATCH: f32 = 8.0;
const GAP_TEXT: f32 = 6.0;
const GAP_ITEM: f32 = 16.0;
const ROW: f32 = 20.0;
/// How far past the width an item may end and still count as fitting: the width a legend is given
/// is its own natural width rounded by layout, and must not wrap its last item for a hair.
const SLACK: f32 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: String,
    pub ink: Ink,
    /// The layer position the chart resolves `Auto` and `Hue` against.
    pub index: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Legend {
    pub items: Vec<Item>,
}

fn style() -> TextStyle {
    TextStyle::new(12.0, 16.0).no_wrap()
}

impl Legend {
    pub fn new() -> Legend {
        Legend::default()
    }

    pub fn item(mut self, label: &str, ink: Ink, index: usize) -> Legend {
        self.items.push(Item { label: label.to_string(), ink, index });
        self
    }

    /// Where each item goes in a row `width` wide: its left and its row. Items wrap to a new row
    /// rather than run past `width`.
    pub fn place(&self, widths: &[f32], width: f32) -> Vec<(f32, usize)> {
        let (mut x, mut row) = (0.0, 0);
        let mut out = Vec::with_capacity(self.items.len());
        for w in widths {
            let item = SWATCH + GAP_TEXT + w;
            if x > 0.0 && x + item > width + SLACK {
                x = 0.0;
                row += 1;
            }
            out.push((x, row));
            x += item + GAP_ITEM;
        }
        out
    }
}

pub struct LegendWidget {
    legend: Legend,
}

impl LegendWidget {
    fn widths(&self, fonts: &vui_core::Fonts) -> Vec<f32> {
        self.legend.items.iter().map(|i| fonts.layout_cached(&i.label, &style(), None).size().w).collect()
    }
}

impl<M: 'static> Widget<M> for LegendWidget {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, old: &Self, _state: &mut ()) -> Dirty {
        if self.legend == old.legend { Dirty::None } else { Dirty::Layout }
    }

    fn measure(&self, _state: &mut (), env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let widths = self.widths(env.fonts());
        let all = self.legend.place(&widths, f32::INFINITY);
        let natural = all.last().zip(widths.last()).map_or(0.0, |((x, _), w)| x + SWATCH + GAP_TEXT + w);
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w.min(natural),
            Avail::MinContent => widths.iter().fold(0.0f32, |m, w| m.max(w + SWATCH + GAP_TEXT)),
            Avail::MaxContent => natural,
        });
        let rows = self.legend.place(&widths, width).last().map_or(0, |p| p.1 + 1);
        size(width, known.y.unwrap_or(rows as f32 * ROW))
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        let vs = VizStyle::from_palette(cx.palette());
        let widths = self.widths(cx.fonts());
        let spots = self.legend.place(&widths, cx.content.w);
        let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
        for (item, (x, row)) in self.legend.items.iter().zip(&spots) {
            let y = cx.content.y + *row as f32 * ROW + ROW / 2.0;
            let c = pt(cx.content.x + x + SWATCH / 2.0, y);
            canvas.ring(c, SWATCH / 2.0, 0.0, item.ink.resolve(&vs, item.index), item.ink.resolve(&vs, item.index));
        }
        paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
        for (item, (x, row)) in self.legend.items.iter().zip(&spots) {
            let layout = cx.fonts().layout_cached(&item.label, &style(), None);
            let h = layout.size().h;
            let at: Point = pt(cx.content.x + x + SWATCH + GAP_TEXT, cx.content.y + *row as f32 * ROW + (ROW - h) / 2.0);
            cx.text(layout, at, Tone::Text2);
        }
    }

    fn access(&self, _state: &(), info: &mut AccessInfo) {
        info.role = Some(Role::List);
        info.label = Some("Legend".to_string());
        info.value = Some(self.legend.items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>().join(", "));
    }
}

pub struct LegendView<M: 'static> {
    base: Base<M>,
    widget: LegendWidget,
}

builder!(LegendView<M> => LegendWidget);

/// A legend that takes the width it needs, and wraps when it is given less.
pub fn legend<M: 'static>(data: Legend) -> LegendView<M> {
    LegendView { base: Base::new(), widget: LegendWidget { legend: data } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_wrap_instead_of_running_past_the_width() {
        let l = Legend::new().item("a", Ink::Hue, 0).item("b", Ink::Hue, 1).item("c", Ink::Hue, 2);
        let widths = [40.0, 40.0, 40.0];
        let one = l.place(&widths, 1000.0);
        assert!(one.iter().all(|p| p.1 == 0) && one[1].0 > one[0].0);
        let tight = l.place(&widths, 130.0);
        assert_eq!(tight.iter().map(|p| p.1).collect::<Vec<_>>(), vec![0, 0, 1], "{tight:?}");
        assert_eq!(tight[2].0, 0.0, "a wrapped item starts at the left");
        assert!(l.place(&[], 50.0).is_empty());
    }

    #[test]
    fn a_row_given_exactly_its_natural_width_does_not_wrap() {
        let l = Legend::new().item("a", Ink::Hue, 0).item("b", Ink::Hue, 1).item("c", Ink::Hue, 2);
        let widths = [41.3, 38.7, 40.1];
        let natural = widths.iter().map(|w| SWATCH + GAP_TEXT + w).sum::<f32>() + GAP_ITEM * 2.0;
        for width in [natural, natural - 0.001, natural + 0.001, (natural * 100.0).round() / 100.0 - 0.004] {
            assert!(l.place(&widths, width).iter().all(|p| p.1 == 0), "wrapped at {width} for {natural}");
        }
        assert_eq!(l.place(&widths, natural - 3.0).last().map(|p| p.1), Some(1), "a real shortfall still wraps");
    }

    #[test]
    fn an_item_wider_than_the_row_still_gets_one() {
        let l = Legend::new().item("long", Ink::Hue, 0).item("b", Ink::Hue, 1);
        let p = l.place(&[500.0, 10.0], 100.0);
        assert_eq!(p[0], (0.0, 0), "the first never wraps away from nothing");
        assert_eq!(p[1].1, 1);
    }
}
