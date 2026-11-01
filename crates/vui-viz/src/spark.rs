//! A sparkline: a series drawn as a thin line with nothing else, small enough to sit in a row of text.

use vui_core::{Point, Rect, Size, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, PaintCx, Role, Widget, builder};

use crate::canvas::{Canvas, paint};
use crate::chart::extremes;
use crate::scale::Linear;
use crate::style::{Ink, VizStyle};
use vui_core::vector::{Cap, Join, Stroke};

const WIDTH: f32 = 84.0;
const HEIGHT: f32 = 24.0;

/// Draws `values` as a line inside `area`, with a dot on the last one. Does nothing for fewer than two.
pub fn draw_spark(canvas: &mut Canvas, values: &[f64], area: Rect, style: &VizStyle, ink: Ink) {
    let finite: Vec<(f64, f64)> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .map(|(i, v)| (i as f64, *v))
        .collect();
    if finite.len() < 2 {
        return;
    }
    let (lo, hi) = finite
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
    let pad = 3.0;
    let x = Linear::new(
        (0.0, (values.len() - 1) as f64),
        (area.x + pad, area.right() - pad),
    );
    // A flat series sits in the middle rather than on an edge.
    let y = if hi > lo {
        Linear::new((lo, hi), (area.bottom() - pad, area.y + pad))
    } else {
        Linear::new((lo - 1.0, lo + 1.0), (area.bottom() - pad, area.y + pad))
    };
    let color = ink.resolve(style, 0);
    let thin = extremes(&finite, &x);
    let line: Vec<Point> = thin.iter().map(|p| pt(x.map(p.0), y.map(p.1))).collect();
    canvas.vector().polyline(
        &line,
        &Stroke::new(1.4).join(Join::Round).cap(Cap::Round),
        color,
    );
    if let Some(&(i, v)) = finite.last() {
        canvas.dot(pt(x.map(i), y.map(v)), 2.4, color);
    }
}

pub struct SparkWidget {
    values: Vec<f64>,
    ink: Ink,
    label: String,
}

impl<M: 'static> Widget<M> for SparkWidget {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, old: &SparkWidget, _state: &mut ()) -> Dirty {
        if self.values != old.values || self.ink != old.ink {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(
        &self,
        _state: &mut (),
        _env: &Env,
        known: Pair<Option<f32>>,
        _avail: Pair<Avail>,
    ) -> Size {
        size(known.x.unwrap_or(WIDTH), known.y.unwrap_or(HEIGHT))
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        let style = VizStyle::from_palette(cx.palette());
        let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
        draw_spark(&mut canvas, &self.values, cx.content, &style, self.ink);
        paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
    }

    fn access(&self, _state: &(), info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        let (first, last) = (self.values.first().copied(), self.values.last().copied());
        info.label = Some(match (self.label.is_empty(), first, last) {
            (false, _, _) => self.label.clone(),
            (true, Some(a), Some(b)) => format!("Trend from {a} to {b}"),
            _ => "Trend".to_string(),
        });
    }
}

pub struct Spark<M: 'static> {
    base: Base<M>,
    widget: SparkWidget,
}

builder!(Spark<M> => SparkWidget);

impl<M: 'static> Spark<M> {
    pub fn ink(mut self, ink: Ink) -> Self {
        self.widget.ink = ink;
        self
    }

    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }
}

/// A line of `values`, 84 by 24 unless sized.
pub fn spark<M: 'static>(values: &[f64]) -> Spark<M> {
    Spark {
        base: Base::new(),
        widget: SparkWidget {
            values: values.to_vec(),
            ink: Ink::Strong,
            label: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vui_core::{Command, Fonts, Theme, rect};

    fn run(values: &[f64]) -> Vec<Command> {
        let fonts = Fonts::bundled_only();
        let mut c = Canvas::new(&fonts, 2.0);
        draw_spark(
            &mut c,
            values,
            rect(0.0, 0.0, 80.0, 24.0),
            &VizStyle::from_palette(&Theme::light().palette),
            Ink::Strong,
        );
        c.finish()
    }

    #[test]
    fn a_series_is_a_line_and_an_end_dot_and_less_than_two_points_is_nothing() {
        let items = run(&[1.0, 3.0, 2.0, 5.0]);
        assert!(matches!(items[0], Command::Mesh(_)) && matches!(items[1], Command::Quad(_)));
        assert!(run(&[1.0]).is_empty() && run(&[]).is_empty() && run(&[f64::NAN, 2.0]).is_empty());
    }

    #[test]
    fn the_end_dot_sits_on_the_last_value_inside_the_box() {
        let items = run(&[1.0, 3.0, 2.0, 5.0]);
        let Command::Quad(dot) = &items[1] else {
            panic!()
        };
        let c = dot.rect.center();
        assert!(
            (c.x - 77.0).abs() < 0.01,
            "the right end, less the padding: {c:?}"
        );
        assert!(
            (c.y - 3.0).abs() < 0.01,
            "the highest value is at the top: {c:?}"
        );
    }

    #[test]
    fn a_flat_series_runs_through_the_middle() {
        let items = run(&[4.0, 4.0, 4.0]);
        let Command::Quad(dot) = &items[1] else {
            panic!()
        };
        assert!((dot.rect.center().y - 12.0).abs() < 0.01);
    }
}
