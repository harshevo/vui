//! Pie and donut charts: shares of a whole, as slices round a centre, with a legend beside or below.
//! [`draw`] turns one into scene commands and [`slice_at`] answers which slice is under a point; the
//! [`pie`] widget adds hover and the draw-in.

mod widget;

pub use widget::{PieView, pie};

use std::f32::consts::{FRAC_PI_2, TAU};

use vui_core::vector::{Path, Transform};
use vui_core::{Fonts, Point, Quad, Rect, TextStyle, pt, rect};

use crate::canvas::{Canvas, HAnchor, VAnchor};
use crate::format::Format;
use crate::style::{Ink, VizStyle};

/// How far a hovered slice moves out, in points.
pub const LIFT: f32 = 6.0;
const LEGEND_ROW: f32 = 22.0;
const LEGEND_GAP: f32 = 20.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub label: String,
    pub value: f64,
    pub ink: Ink,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pie {
    pub slices: Vec<Slice>,
    /// The hole as a share of the radius: 0 for a pie, about 0.6 for a donut.
    pub hole: f32,
    /// Points of space between slices.
    pub gap: f32,
    pub format: Format,
    /// The words under the total in a donut's hole.
    pub caption: String,
    pub legend: bool,
}

impl Default for Pie {
    fn default() -> Pie {
        Pie::new()
    }
}

impl Pie {
    pub fn new() -> Pie {
        Pie { slices: Vec::new(), hole: 0.0, gap: 2.0, format: Format::Auto, caption: "Total".into(), legend: true }
    }

    /// Adds a slice. Values that are negative or not numbers count as nothing.
    pub fn slice(mut self, label: &str, value: f64) -> Pie {
        let value = if value.is_finite() { value.max(0.0) } else { 0.0 };
        self.slices.push(Slice { label: label.to_string(), value, ink: Ink::Hue });
        self
    }

    /// Paints the most recently added slice.
    pub fn ink(mut self, ink: Ink) -> Pie {
        if let Some(s) = self.slices.last_mut() {
            s.ink = ink;
        }
        self
    }

    /// A ring instead of a disc, with the total written in the middle.
    pub fn donut(self) -> Pie {
        self.hole(0.62)
    }

    pub fn hole(mut self, share: f32) -> Pie {
        self.hole = share.clamp(0.0, 0.9);
        self
    }

    pub fn gap(mut self, points: f32) -> Pie {
        self.gap = points.clamp(0.0, 12.0);
        self
    }

    pub fn format(mut self, format: Format) -> Pie {
        self.format = format;
        self
    }

    pub fn caption(mut self, words: &str) -> Pie {
        self.caption = words.to_string();
        self
    }

    pub fn no_legend(mut self) -> Pie {
        self.legend = false;
        self
    }

    pub fn total(&self) -> f64 {
        self.slices.iter().map(|s| s.value).sum()
    }

    /// Slice `i`'s share of the total, 0 to 1.
    pub fn share(&self, i: usize) -> f64 {
        let total = self.total();
        if total <= 0.0 { 0.0 } else { self.slices.get(i).map_or(0.0, |s| s.value / total) }
    }

    /// Where each slice starts and ends, in radians clockwise from the top, with `reveal` of the
    /// circle drawn.
    pub fn angles(&self, reveal: f32) -> Vec<(f32, f32)> {
        let mut at = -FRAC_PI_2;
        (0..self.slices.len())
            .map(|i| {
                let span = self.share(i) as f32 * TAU * reveal.clamp(0.0, 1.0);
                let out = (at, at + span);
                at += span;
                out
            })
            .collect()
    }

    pub fn color(&self, i: usize, style: &VizStyle) -> vui_core::Color {
        match self.slices[i].ink {
            Ink::Auto => style.category(i),
            ink => ink.resolve(style, i),
        }
    }
}

/// Where the circle and the legend go inside a rect.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PieLayout {
    pub centre: Point,
    pub radius: f32,
    pub inner: f32,
    pub legend: Option<Rect>,
}

/// Puts the legend to the right when there is room for it beside the circle, and below otherwise.
pub fn layout(fonts: &Fonts, pie: &Pie, area: Rect, style: &VizStyle) -> PieLayout {
    let rows = pie.slices.len() as f32 * LEGEND_ROW;
    let widest = pie.slices.iter().map(|s| fonts.measure(&s.label, &style.label, None).w).fold(0.0, f32::max);
    let legend_w = (widest + 72.0).min(area.w * 0.5);
    let (circle, legend) = if !pie.legend || pie.slices.is_empty() {
        (area, None)
    } else if area.w >= area.h + legend_w * 0.8 {
        let circle = rect(area.x, area.y, area.w - legend_w - LEGEND_GAP, area.h);
        let side = circle.w.min(circle.h);
        let circle = rect(area.x, area.y, side, area.h);
        let top = area.y + ((area.h - rows) / 2.0).max(0.0);
        (circle, Some(rect(circle.right() + LEGEND_GAP, top, legend_w, rows.min(area.h))))
    } else {
        let h = (area.h - rows - 12.0).max(40.0);
        (rect(area.x, area.y, area.w, h), Some(rect(area.x, area.y + h + 12.0, area.w, rows)))
    };
    let radius = ((circle.w.min(circle.h)) / 2.0 - LIFT - 2.0).max(4.0);
    PieLayout {
        centre: pt(circle.x + circle.w / 2.0, circle.y + circle.h / 2.0),
        radius,
        inner: radius * pie.hole,
        legend,
    }
}

/// The slice under `p`, if any, with slices drawn as `angles` gives them.
pub fn slice_at(pie: &Pie, l: &PieLayout, p: Point) -> Option<usize> {
    let (dx, dy) = (p.x - l.centre.x, p.y - l.centre.y);
    let d = dx.hypot(dy);
    if d < l.inner || d > l.radius + LIFT || pie.total() <= 0.0 {
        return legend_row(pie, l, p);
    }
    let mut a = dy.atan2(dx);
    if a < -FRAC_PI_2 {
        a += TAU;
    }
    pie.angles(1.0).iter().position(|&(a0, a1)| a >= a0 && a < a1 && a1 > a0)
}

fn legend_row(pie: &Pie, l: &PieLayout, p: Point) -> Option<usize> {
    let legend = l.legend?;
    if !legend.contains(p) {
        return None;
    }
    let i = ((p.y - legend.y) / LEGEND_ROW) as usize;
    (i < pie.slices.len()).then_some(i)
}

/// The outline of a slice from `a0` to `a1`, `gap` points from its neighbours.
fn wedge(c: Point, inner: f32, outer: f32, a0: f32, a1: f32, gap: f32) -> Option<Path> {
    let span = a1 - a0;
    let full = span >= TAU - 1e-4;
    let half_gap = if full { 0.0 } else { gap / 2.0 };
    let pad_outer = half_gap / outer;
    if span <= pad_outer * 2.0 + 1e-4 {
        return None;
    }
    let at = |r: f32, a: f32| pt(c.x + r * a.cos(), c.y + r * a.sin());
    let arc = |path: &mut Path, r: f32, from: f32, to: f32, first: bool| {
        let n = (((to - from).abs() * r / 2.0).ceil() as usize).clamp(2, 512);
        for k in 0..=n {
            let p = at(r, from + (to - from) * k as f32 / n as f32);
            if first && k == 0 { path.move_to(p) } else { path.line_to(p) };
        }
    };
    if full && inner <= 0.0 {
        return Some(Path::circle(c, outer));
    }
    let mut path = Path::new();
    if full {
        // A whole ring: the outer circle and the inner one wound the other way.
        arc(&mut path, outer, 0.0, TAU, true);
        path.close();
        arc(&mut path, inner, TAU, 0.0, true);
        path.close();
        return Some(path);
    }
    arc(&mut path, outer, a0 + pad_outer, a1 - pad_outer, true);
    if inner > 0.0 && span > half_gap * 2.0 / inner + 1e-3 {
        let pad_inner = half_gap / inner;
        arc(&mut path, inner, a1 - pad_inner, a0 + pad_inner, false);
    } else {
        // The point of the slice, moved out along its middle so the gap is the same width throughout.
        let mid = (a0 + a1) / 2.0;
        let reach = if full { 0.0 } else { (half_gap / (span / 2.0).sin().max(0.05)).min(outer * 0.4) };
        path.line_to(at(reach.max(inner), mid));
    }
    path.close();
    Some(path)
}

/// Draws the slices, the total in a donut's hole, and the legend. `lift` gives how far each slice is
/// out, 0 to 1; `focus` is the slice to write in the middle instead of the total.
pub fn draw(canvas: &mut Canvas, pie: &Pie, area: Rect, style: &VizStyle, reveal: f32, lift: &[f32], focus: Option<usize>) -> PieLayout {
    let l = layout(canvas.fonts(), pie, area, style);
    let eased = 1.0 - (1.0 - reveal.clamp(0.0, 1.0)).powi(3);
    if pie.total() <= 0.0 {
        let ring = wedge(l.centre, l.inner.max(l.radius - 1.0), l.radius, 0.0, TAU, 0.0);
        if let Some(path) = ring {
            canvas.vector().fill(&path, &Transform::IDENTITY, style.line);
        }
    }
    for (i, &(a0, a1)) in pie.angles(eased).iter().enumerate() {
        let out = lift.get(i).copied().unwrap_or(0.0) * LIFT;
        let mid = (a0 + a1) / 2.0;
        let c = pt(l.centre.x + out * 0.5 * mid.cos(), l.centre.y + out * 0.5 * mid.sin());
        if let Some(path) = wedge(c, l.inner, l.radius + out * 0.5, a0, a1, pie.gap) {
            let dim = focus.is_some_and(|f| f != i);
            let color = pie.color(i, style);
            canvas.vector().fill(&path, &Transform::IDENTITY, if dim { color.mul_alpha(0.45) } else { color });
        }
    }
    if l.inner >= 24.0 {
        let (value, words) = match focus {
            Some(i) => (pie.format.apply(pie.slices[i].value), format!("{} · {}", pie.slices[i].label, Format::Percent(0).apply(pie.share(i)))),
            None => (pie.format.apply(pie.total()), pie.caption.clone()),
        };
        let size = (l.inner * 0.36).clamp(13.0, 28.0);
        let big = TextStyle::new(size, size * 1.25).weight(600);
        canvas.text(&value, &big, pt(l.centre.x, l.centre.y + 2.0), HAnchor::Center, VAnchor::Bottom, style.ink);
        canvas.text(&words, &style.label, pt(l.centre.x, l.centre.y + 4.0), HAnchor::Center, VAnchor::Top, style.ink_3);
    }
    if let Some(legend) = l.legend {
        for (i, s) in pie.slices.iter().enumerate() {
            let y = legend.y + i as f32 * LEGEND_ROW + LEGEND_ROW / 2.0;
            let dim = focus.is_some_and(|f| f != i);
            let ink = |c: vui_core::Color| if dim { c.mul_alpha(0.5) } else { c };
            canvas.quad(Quad::new(rect(legend.x, y - 4.0, 8.0, 8.0)).fill(ink(pie.color(i, style))).radii(2.0));
            canvas.text(&s.label, &style.label, pt(legend.x + 16.0, y), HAnchor::Left, VAnchor::Middle, ink(style.ink_2));
            let share = Format::Percent(0).apply(pie.share(i));
            canvas.text(&share, &style.label_mono, pt(legend.right(), y), HAnchor::Right, VAnchor::Middle, ink(style.ink_3));
        }
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use vui_core::Theme;

    fn style() -> VizStyle {
        VizStyle::from_palette(&Theme::light().palette)
    }

    fn sample() -> Pie {
        Pie::new().slice("A", 50.0).slice("B", 30.0).slice("C", 20.0).slice("bad", f64::NAN).slice("neg", -4.0)
    }

    #[test]
    fn shares_add_up_and_bad_values_count_as_nothing() {
        let p = sample();
        assert_eq!(p.total(), 100.0);
        assert_eq!(p.share(0), 0.5);
        assert_eq!(p.share(3), 0.0);
        assert_eq!(p.share(4), 0.0);
        let a = p.angles(1.0);
        assert!((a[0].0 + FRAC_PI_2).abs() < 1e-6, "the first slice starts at the top");
        assert!((a[2].1 - (TAU - FRAC_PI_2)).abs() < 1e-5, "the last real slice closes the circle");
        assert!(p.angles(0.5)[2].1 < TAU / 2.0, "half revealed draws half the circle");
    }

    #[test]
    fn a_wide_box_puts_the_legend_beside_and_a_narrow_one_below() {
        let fonts = Fonts::bundled_only();
        let p = sample();
        let wide = layout(&fonts, &p, rect(0.0, 0.0, 400.0, 200.0), &style());
        assert!(wide.legend.unwrap().x > wide.centre.x + wide.radius);
        let narrow = layout(&fonts, &p, rect(0.0, 0.0, 200.0, 320.0), &style());
        assert!(narrow.legend.unwrap().y > narrow.centre.y + narrow.radius);
    }

    #[test]
    fn the_slice_under_a_point_is_found_by_angle_and_ring() {
        let fonts = Fonts::bundled_only();
        let p = sample().donut();
        let l = layout(&fonts, &p, rect(0.0, 0.0, 200.0, 200.0), &style());
        let at = |deg: f32, r: f32| {
            let a = (deg - 90.0).to_radians();
            pt(l.centre.x + r * a.cos(), l.centre.y + r * a.sin())
        };
        let r = (l.inner + l.radius) / 2.0;
        assert_eq!(slice_at(&p, &l, at(10.0, r)), Some(0), "just right of the top is the first half");
        assert_eq!(slice_at(&p, &l, at(200.0, r)), Some(1));
        assert_eq!(slice_at(&p, &l, at(300.0, r)), Some(2));
        assert_eq!(slice_at(&p, &l, l.centre), None, "the hole is not a slice");
        assert_eq!(slice_at(&p, &l, at(10.0, l.radius + 20.0)), None);
    }
}
