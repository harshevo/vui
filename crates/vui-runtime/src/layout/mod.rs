//! Layout.
//!
//! Our own flexbox engine: rows and columns that grow, shrink and wrap, with gaps, margins,
//! percentages, aspect ratios, absolute children, baseline alignment and content-measured leaves;
//! and a grid engine (see `grid.rs`) for children in rows and columns.
//! It is written against [`LayoutNode`], so it runs on any tree and knows nothing about widgets.
//!
//! It follows CSS flexbox where that is where people's intuition comes from, with these deliberate
//! differences:
//! - sizes are always border-box, and layout has no borders (draw them inside the padding);
//! - the default direction is a column, because stacking is what most containers do;
//! - an absolute child is placed against its parent, never a grandparent;
//! - `justify` and `align` never push content past the start edge: content that does not fit
//!   overflows at the end;
//! - a flex item with a fixed pixel size does not shrink below it unless it sets a smaller minimum,
//!   so icons and sidebars keep their size when a row gets tight;
//! - a scroll container lays its children out at their natural size along the scroll axis.
//!
//! A node is laid out by [`compute`] in one of two modes. `Measure` answers "how big would you be
//! if..." without touching any state; parents use it to size their children. `Place` answers the
//! same and then positions the node's children. Answers are cached per node, keyed by the question
//! asked, so a tree is measured about once per distinct question. Whoever changes a node's style or
//! content must clear the cache of that node and of every ancestor (see [`LayoutCache::clear`]).

use std::ops::Range;

use vui_core::{Point, Size, pt, size};

use crate::style::{Align, Axis, Edges, Justify, Len, Overflow, Position, Sides, Style};

mod flex;
mod grid;
mod lines;
mod positioned;

use flex::*;
use grid::grid;
use lines::*;
pub use positioned::place_fixed;
use positioned::*;

/// A value for each axis.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Pair<T> {
    pub x: T,
    pub y: T,
}

impl<T: Copy> Pair<T> {
    pub const fn new(x: T, y: T) -> Pair<T> {
        Pair { x, y }
    }

    pub fn get(self, axis: Axis) -> T {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
        }
    }

    /// `main` goes on `axis` and `cross` on the other axis.
    pub fn along(axis: Axis, main: T, cross: T) -> Pair<T> {
        match axis {
            Axis::X => Pair::new(main, cross),
            Axis::Y => Pair::new(cross, main),
        }
    }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> Pair<U> {
        Pair { x: f(self.x), y: f(self.y) }
    }
}

/// Room offered to a node on one axis.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Avail {
    Definite(f32),
    /// As little as the content can do with: the longest word, the widest unbreakable child.
    MinContent,
    /// As much as the content wants: text on one line.
    MaxContent,
}

impl Avail {
    pub fn definite(self) -> Option<f32> {
        match self {
            Avail::Definite(v) => Some(v),
            _ => None,
        }
    }

    fn shrink(self, by: f32) -> Avail {
        match self {
            Avail::Definite(v) => Avail::Definite((v - by).max(0.0)),
            other => other,
        }
    }

    fn bounded(self, min: f32, max: f32) -> Avail {
        match self {
            Avail::Definite(v) => Avail::Definite(clamp_size(v, min, max)),
            other => other,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Work out sizes only.
    Measure,
    /// Work out sizes and position the children.
    Place,
}

/// The question a node is asked.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LayoutInput {
    /// Border-box sizes the parent has already decided. The node obeys them.
    pub known: Pair<Option<f32>>,
    /// The parent's content size, which percentages resolve against. `None` where undecided.
    pub parent: Pair<Option<f32>>,
    /// Room offered to the border box, margins excluded.
    pub avail: Pair<Avail>,
    pub mode: Mode,
}

impl LayoutInput {
    /// The root: its box is exactly `size`.
    pub fn root(size: Size) -> LayoutInput {
        let known = Pair::new(Some(size.w), Some(size.h));
        LayoutInput { known, parent: known, avail: Pair::new(Avail::Definite(size.w), Avail::Definite(size.h)), mode: Mode::Place }
    }

    fn question(&self) -> Question {
        Question { known: self.known, parent: self.parent, avail: self.avail }
    }
}

/// The answer.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LayoutOutput {
    /// The border box.
    pub size: Size,
    /// How far the children reach from the box's top-left corner, end padding included. Larger than
    /// `size` when the content overflows: that is the scroll extent.
    pub content: Size,
    /// The first baseline, from the top of the border box.
    pub baseline: Option<f32>,
}

/// Where a node ended up, as left behind by the last `Place`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct NodeLayout {
    /// Top-left corner of the border box, relative to the parent's border box.
    pub origin: Point,
    pub size: Size,
    pub content: Size,
    pub baseline: Option<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Question {
    known: Pair<Option<f32>>,
    parent: Pair<Option<f32>>,
    avail: Pair<Avail>,
}

const MEASURE_SLOTS: usize = 16;

/// A node's remembered answers.
#[derive(Debug, Default)]
pub struct LayoutCache {
    measured: Vec<(Question, LayoutOutput)>,
    /// The last `Place` only: the children's positions are the ones that call left behind.
    placed: Option<(Question, LayoutOutput)>,
    next: usize,
}

impl LayoutCache {
    pub fn clear(&mut self) {
        self.measured.clear();
        self.placed = None;
        self.next = 0;
    }

    fn lookup(&self, input: &LayoutInput) -> Option<LayoutOutput> {
        let q = input.question();
        if let Some((pq, out)) = &self.placed
            && *pq == q
        {
            return Some(*out);
        }
        match input.mode {
            Mode::Measure => self.measured.iter().find(|(mq, _)| *mq == q).map(|(_, out)| *out),
            Mode::Place => None,
        }
    }

    fn store(&mut self, input: &LayoutInput, out: LayoutOutput) {
        let q = input.question();
        match input.mode {
            Mode::Place => self.placed = Some((q, out)),
            Mode::Measure if self.measured.len() < MEASURE_SLOTS => self.measured.push((q, out)),
            Mode::Measure => {
                self.measured[self.next % MEASURE_SLOTS] = (q, out);
                self.next += 1;
            }
        }
    }
}

/// What the engine needs from a tree. `C` is whatever context measuring needs, such as the fonts.
pub trait LayoutNode<C>: Sized {
    fn style(&self) -> &Style;

    fn children_mut(&mut self) -> &mut [Self];

    /// The content size of a node without children, such as text or an image. `known` is the
    /// content-box size already decided on each axis and `avail` the room offered. Padding is added
    /// by the engine.
    fn measure(&mut self, cx: &mut C, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let _ = (cx, known, avail);
        Size::ZERO
    }

    /// The first baseline of a node without children, from the top of its content box.
    fn baseline(&self) -> Option<f32> {
        None
    }

    fn cache(&mut self) -> &mut LayoutCache;

    fn layout_mut(&mut self) -> &mut NodeLayout;
}

/// Lays out a whole tree: the root fills `size` and sits at the origin.
pub fn layout_root<C, N: LayoutNode<C>>(root: &mut N, cx: &mut C, size: Size) -> LayoutOutput {
    let out = compute(root, cx, LayoutInput::root(size));
    root.layout_mut().origin = Point::ZERO;
    out
}

/// Answers one question about one node, recursing into its children as needed.
pub fn compute<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, input: LayoutInput) -> LayoutOutput {
    if let Some(hit) = node.cache().lookup(&input) {
        return hit;
    }
    let out = compute_uncached(node, cx, &input);
    node.cache().store(&input, out);
    if input.mode == Mode::Place {
        let layout = node.layout_mut();
        layout.size = out.size;
        layout.content = out.content;
        layout.baseline = out.baseline;
    }
    out
}

fn compute_uncached<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, input: &LayoutInput) -> LayoutOutput {
    let style = *node.style();
    if style.hidden {
        return LayoutOutput::default();
    }
    let own = Own::resolve(style, input);
    if node.children_mut().is_empty() {
        leaf(node, cx, input, &own)
    } else if style.grid.is_some() {
        grid(node, cx, input, &own)
    } else {
        flex(node, cx, input, &own)
    }
}

fn along(s: Size, axis: Axis) -> f32 {
    match axis {
        Axis::X => s.w,
        Axis::Y => s.h,
    }
}

fn clamp_size(v: f32, min: f32, max: f32) -> f32 {
    v.min(max).max(min)
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}

/// A node's own sizing, resolved against what the parent said.
struct Own {
    style: Style,
    pad: Edges,
    pad_sum: Pair<f32>,
    /// The border-box size, where it is already decided.
    known: Pair<Option<f32>>,
    /// Border-box limits. The minimum is never below the padding.
    min: Pair<f32>,
    max: Pair<f32>,
}

impl Own {
    fn resolve(style: Style, input: &LayoutInput) -> Own {
        // A percentage of a size that is still being worked out counts as auto, which is also how
        // intrinsic sizes ignore percentages.
        let basis = |avail: Avail, parent: Option<f32>| if matches!(avail, Avail::Definite(_)) { parent } else { None };
        let (bx, by) = (basis(input.avail.x, input.parent.x), basis(input.avail.y, input.parent.y));
        let pad = style.padding;
        let pad_sum = Pair::new(pad.horizontal(), pad.vertical());
        let min = Pair::new(
            style.min_width.resolve(bx).unwrap_or(0.0).max(pad_sum.x),
            style.min_height.resolve(by).unwrap_or(0.0).max(pad_sum.y),
        );
        let max = Pair::new(
            style.max_width.resolve(bx).unwrap_or(f32::INFINITY),
            style.max_height.resolve(by).unwrap_or(f32::INFINITY),
        );
        let mut known = Pair::new(input.known.x.or_else(|| style.width.resolve(bx)), input.known.y.or_else(|| style.height.resolve(by)));
        if let Some(ratio) = style.aspect_ratio.filter(|r| *r > 0.0) {
            match (known.x, known.y) {
                (Some(w), None) => known.y = Some(w / ratio),
                (None, Some(h)) => known.x = Some(h * ratio),
                _ => {}
            }
        }
        known.x = known.x.map(|v| clamp_size(finite(v), min.x, max.x));
        known.y = known.y.map(|v| clamp_size(finite(v), min.y, max.y));
        Own { style, pad, pad_sum, known, min, max }
    }

    fn inner_known(&self) -> Pair<Option<f32>> {
        Pair::new(self.known.x.map(|v| (v - self.pad_sum.x).max(0.0)), self.known.y.map(|v| (v - self.pad_sum.y).max(0.0)))
    }

    /// Room for the content on both axes: the decided size, else what was offered less the padding.
    fn offered(&self, input: &LayoutInput) -> Pair<Avail> {
        let inner = self.inner_known();
        let one = |known: Option<f32>, avail: Avail, min: f32, max: f32, pad: f32| match known {
            Some(v) => Avail::Definite(v),
            None => avail.bounded(min, max).shrink(pad),
        };
        Pair::new(
            one(inner.x, input.avail.x, self.min.x, self.max.x, self.pad_sum.x),
            one(inner.y, input.avail.y, self.min.y, self.max.y, self.pad_sum.y),
        )
    }
}

/// A node with no children: its content decides its size.
fn leaf<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, input: &LayoutInput, own: &Own) -> LayoutOutput {
    let inner_known = own.inner_known();
    let offered = own.offered(input);
    let m = node.measure(cx, inner_known, offered);
    let w = own.known.x.unwrap_or_else(|| clamp_size(finite(m.w) + own.pad_sum.x, own.min.x, own.max.x));
    let h = own.known.y.unwrap_or_else(|| clamp_size(finite(m.h) + own.pad_sum.y, own.min.y, own.max.y));
    let baseline = node.baseline().map(|b| b + own.pad.top);
    LayoutOutput { size: size(w, h), content: size(w, h), baseline }
}

#[cfg(test)]
mod tests;
