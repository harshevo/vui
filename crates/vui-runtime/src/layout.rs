//! Layout.
//!
//! Our own flexbox engine: rows and columns that grow, shrink and wrap, with gaps, margins,
//! percentages, aspect ratios, absolute children, baseline alignment and content-measured leaves.
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
    if node.children_mut().is_empty() { leaf(node, cx, input, &own) } else { flex(node, cx, input, &own) }
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

/// One child taking part in a row or column.
struct Item {
    idx: usize,
    margin: Edges,
    auto: Sides<bool>,
    m_main: f32,
    m_cross: f32,
    align: Align,
    /// Its cross size is auto and may be widened to fill the line.
    stretch: bool,
    min_cross: f32,
    max_cross: f32,
    /// What was imposed on and offered to it across, so it can be measured again.
    cross_known: Option<f32>,
    cross_avail: Avail,
    /// The first measurement: the main size it asked for, and the cross size that went with it.
    asked_main: f32,
    asked_cross: f32,
    baseline: Option<f32>,
    grow: f32,
    shrink: f32,
    /// Flex base size, and its minimum, maximum and clamped ("hypothetical") size.
    base: f32,
    min_main: f32,
    max_main: f32,
    hyp: f32,
    /// The size after growing and shrinking.
    target: f32,
    frozen: bool,
    violation: f32,
    /// Cross size before stretching, and the final one.
    cross: f32,
    final_cross: f32,
    /// Position of the border box inside the container's content box.
    main_pos: f32,
    cross_pos: f32,
}

/// What the container decided before looking at its children.
struct Frame {
    main: Axis,
    cross: Axis,
    wrap: bool,
    min_content: bool,
    inner_known: Pair<Option<f32>>,
    offered: Pair<Avail>,
    /// What percentages in the children are a percentage of.
    child_parent: Pair<Option<f32>>,
}

fn resolve_margin(m: &Sides<Len>, percent_of: f32) -> (Edges, Sides<bool>) {
    let one = |l: Len| match l {
        Len::Px(v) => (v, false),
        Len::Pct(p) => (percent_of * p / 100.0, false),
        Len::Auto => (0.0, true),
    };
    let (top, at) = one(m.top);
    let (right, ar) = one(m.right);
    let (bottom, ab) = one(m.bottom);
    let (left, al) = one(m.left);
    (Sides::new(top, right, bottom, left), Sides::new(at, ar, ab, al))
}

/// The explicit minimum (if any) and the maximum of a child along `axis`.
fn limits(cs: &Style, parent: Pair<Option<f32>>, axis: Axis) -> (Option<f32>, f32) {
    let (min, max, basis) = match axis {
        Axis::X => (cs.min_width, cs.max_width, parent.x),
        Axis::Y => (cs.min_height, cs.max_height, parent.y),
    };
    (min.resolve(basis), max.resolve(basis).unwrap_or(f32::INFINITY))
}

fn gather<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, style: &Style, f: &Frame) -> (Vec<Item>, Vec<usize>) {
    let (main, cross) = (f.main, f.cross);
    let mut items = Vec::new();
    let mut absolute = Vec::new();
    for idx in 0..node.children_mut().len() {
        let child = &mut node.children_mut()[idx];
        let cs = *child.style();
        if cs.hidden {
            continue;
        }
        if cs.position == Position::Absolute {
            absolute.push(idx);
            continue;
        }

        let (margin, auto) = resolve_margin(&cs.margin, f.inner_known.x.unwrap_or(0.0));
        let (m_main, m_cross) = (margin.sum(main), margin.sum(cross));
        let align = cs.align_self.unwrap_or(style.align_items);

        let (width_len, height_len) = (cs.width, cs.height);
        let main_len = if main == Axis::X { width_len } else { height_len };
        let pref = Pair::new(width_len.resolve(f.child_parent.x), height_len.resolve(f.child_parent.y));
        let (pref_main, pref_cross) = (pref.get(main), pref.get(cross));
        let basis = match cs.basis {
            Len::Auto => pref_main,
            l => l.resolve(f.child_parent.get(main)),
        };

        let (min_main_spec, max_main) = limits(&cs, f.child_parent, main);
        let (min_cross_spec, max_cross) = limits(&cs, f.child_parent, cross);
        let (pad_main, pad_cross) = (cs.padding.sum(main), cs.padding.sum(cross));
        let min_cross = min_cross_spec.unwrap_or(0.0).max(pad_cross);

        // A child with an auto cross size fills the line. With an aspect ratio it does so only when
        // that decides its main size too (a square tile in a column is as wide as the column); if the
        // main size is already decided, the ratio decides the cross size instead.
        let ratio_allows = cs.aspect_ratio.is_none() || (basis.is_none() && f.inner_known.get(cross).is_some() && !f.wrap);
        let stretch = align == Align::Stretch && pref_cross.is_none() && ratio_allows && !auto.start(cross) && !auto.end(cross);
        let cross_known = match pref_cross {
            Some(c) => Some(c),
            None if stretch && !f.wrap => f.inner_known.get(cross).map(|ic| (ic - m_cross).max(0.0)),
            None => None,
        };
        let cross_avail = cross_known.map_or_else(|| f.offered.get(cross).shrink(m_cross), Avail::Definite);

        let main_avail = basis.map_or(if f.min_content { Avail::MinContent } else { Avail::MaxContent }, Avail::Definite);
        let first = compute(
            child,
            cx,
            LayoutInput {
                known: Pair::along(main, basis, cross_known),
                parent: f.child_parent,
                avail: Pair::along(main, main_avail, cross_avail),
                mode: Mode::Measure,
            },
        );
        let (asked_main, asked_cross) = (along(first.size, main), along(first.size, cross));
        let base = basis.unwrap_or(asked_main);

        // The smallest a child may be squeezed to when it has no explicit minimum: its content, or
        // its pixel size if it has one.
        let min_main = match min_main_spec {
            Some(m) => m.max(pad_main),
            None if cs.overflow != Overflow::Visible || (cs.grow <= 0.0 && cs.shrink <= 0.0) => pad_main,
            None => match main_len {
                Len::Px(v) => v.max(pad_main),
                _ => {
                    let content = compute(
                        child,
                        cx,
                        LayoutInput {
                            known: Pair::along(main, None, cross_known),
                            parent: f.child_parent,
                            avail: Pair::along(main, Avail::MinContent, cross_avail),
                            mode: Mode::Measure,
                        },
                    );
                    along(content.size, main).max(pad_main)
                }
            },
        }
        .min(max_main);
        let hyp = clamp_size(base, min_main, max_main);

        items.push(Item {
            idx,
            margin,
            auto,
            m_main,
            m_cross,
            align,
            stretch,
            min_cross,
            max_cross,
            cross_known,
            cross_avail,
            asked_main,
            asked_cross,
            baseline: first.baseline,
            grow: cs.grow.max(0.0),
            shrink: cs.shrink.max(0.0),
            base,
            min_main,
            max_main,
            hyp,
            target: hyp,
            frozen: false,
            violation: 0.0,
            cross: asked_cross,
            final_cross: asked_cross,
            main_pos: 0.0,
            cross_pos: 0.0,
        });
    }
    (items, absolute)
}

/// Splits items into lines. Without wrapping, or with room for everything, that is one line.
fn break_lines(items: &[Item], wrap: bool, room: f32, gap: f32) -> Vec<Range<usize>> {
    if items.is_empty() {
        return Vec::new();
    }
    if !wrap {
        let everything = 0..items.len();
        return vec![everything];
    }
    let mut lines = Vec::new();
    let (mut start, mut used) = (0, 0.0f32);
    for (i, it) in items.iter().enumerate() {
        let outer = it.hyp + it.m_main;
        if i == start {
            used = outer;
        } else if used + gap + outer > room + 0.01 {
            lines.push(start..i);
            start = i;
            used = outer;
        } else {
            used += gap + outer;
        }
    }
    lines.push(start..items.len());
    lines
}

/// Shares the free space of one line out by the items' grow or shrink factors, following the
/// flexbox algorithm: items that hit a limit are frozen at it and the rest share what remains.
fn resolve_line(line: &mut [Item], available: f32, gap: f32) {
    let gaps = gap * line.len().saturating_sub(1) as f32;
    let hypothetical: f32 = line.iter().map(|i| i.hyp + i.m_main).sum::<f32>() + gaps;
    let growing = available - hypothetical > 0.0;
    let factor = |i: &Item| if growing { i.grow } else { i.shrink };
    for it in line.iter_mut() {
        it.target = it.hyp;
        it.frozen = factor(it) <= 0.0 || (growing && it.base > it.hyp) || (!growing && it.base < it.hyp);
    }
    let claimed = |line: &[Item]| line.iter().map(|i| if i.frozen { i.target } else { i.base } + i.m_main).sum::<f32>() + gaps;
    let initial_free = available - claimed(line);
    while line.iter().any(|i| !i.frozen) {
        let mut free = available - claimed(line);
        let factors: f32 = line.iter().filter(|i| !i.frozen).map(factor).sum();
        if growing && factors < 1.0 {
            let scaled = initial_free * factors;
            if scaled.abs() < free.abs() {
                free = scaled;
            }
        }
        if free.abs() > f32::EPSILON {
            if growing {
                if factors > 0.0 {
                    for it in line.iter_mut().filter(|i| !i.frozen) {
                        it.target = it.base + free * it.grow / factors;
                    }
                }
            } else {
                let scaled: f32 = line.iter().filter(|i| !i.frozen).map(|i| i.shrink * i.base).sum();
                if scaled > 0.0 {
                    for it in line.iter_mut().filter(|i| !i.frozen) {
                        it.target = it.base + free * (it.shrink * it.base) / scaled;
                    }
                }
            }
        }
        let mut total = 0.0;
        for it in line.iter_mut().filter(|i| !i.frozen) {
            let clamped = clamp_size(it.target, it.min_main, it.max_main);
            it.violation = clamped - it.target;
            total += it.violation;
            it.target = clamped;
        }
        for it in line.iter_mut().filter(|i| !i.frozen) {
            it.frozen = total == 0.0 || (total > 0.0 && it.violation > 0.0) || (total < 0.0 && it.violation < 0.0);
        }
    }
}

/// Positions a line's items along the main axis: auto margins soak up the free space first, then
/// `justify` spreads what is left.
fn position_line(line: &mut [Item], room: f32, gap: f32, justify: Justify, main: Axis) {
    let n = line.len();
    let used: f32 = line.iter().map(|i| i.target + i.m_main).sum::<f32>() + gap * n.saturating_sub(1) as f32;
    let free = (room - used).max(0.0);
    let autos: usize = line.iter().map(|i| usize::from(i.auto.start(main)) + usize::from(i.auto.end(main))).sum();
    let per_auto = if autos > 0 { free / autos as f32 } else { 0.0 };
    let (mut cursor, between) = if autos > 0 {
        (0.0, 0.0)
    } else {
        match justify {
            Justify::Start => (0.0, 0.0),
            Justify::Center => (free / 2.0, 0.0),
            Justify::End => (free, 0.0),
            Justify::SpaceBetween if n > 1 => (0.0, free / (n - 1) as f32),
            Justify::SpaceBetween => (0.0, 0.0),
            Justify::SpaceAround => (free / n as f32 / 2.0, free / n as f32),
            Justify::SpaceEvenly => (free / (n + 1) as f32, free / (n + 1) as f32),
        }
    };
    for it in line.iter_mut() {
        cursor += if it.auto.start(main) { per_auto } else { it.margin.start(main) };
        it.main_pos = cursor;
        cursor += it.target;
        cursor += if it.auto.end(main) { per_auto } else { it.margin.end(main) };
        cursor += gap + between;
    }
}

/// A container with children.
fn flex<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, input: &LayoutInput, own: &Own) -> LayoutOutput {
    let style = own.style;
    let place = input.mode == Mode::Place;
    let main = style.direction.main_axis();
    let cross = main.cross();
    let scrolls = style.overflow == Overflow::Scroll;
    let wrap = style.wrap && !scrolls;
    let pad = own.pad;
    let (pad_main, pad_cross) = (own.pad_sum.get(main), own.pad_sum.get(cross));
    let (gap_main, gap_cross) = (style.gap_along(main), style.gap_along(cross));

    let inner_known = own.inner_known();
    let offered = own.offered(input);
    let frame = Frame {
        main,
        cross,
        wrap,
        min_content: matches!(offered.get(main), Avail::MinContent) && !scrolls,
        inner_known,
        offered,
        // A scroll container's length along its axis is not what its children's percentages mean.
        child_parent: Pair::along(main, if scrolls { None } else { inner_known.get(main) }, inner_known.get(cross)),
    };

    let (mut items, absolute) = gather(node, cx, &style, &frame);
    let n = items.len();

    // The container's size along the main axis: decided, or its content fitted into what is offered.
    let sum_outer = items.iter().map(|i| i.hyp + i.m_main).sum::<f32>() + gap_main * n.saturating_sub(1) as f32;
    let widest = items.iter().map(|i| i.hyp + i.m_main).fold(0.0, f32::max);
    let natural_main = if wrap && frame.min_content { widest } else { sum_outer };
    let fitted_main = match offered.get(main) {
        Avail::Definite(a) => natural_main.min(a),
        _ => natural_main,
    };
    let own_main = own.known.get(main).unwrap_or_else(|| clamp_size(fitted_main + pad_main, own.min.get(main), own.max.get(main)));
    // Room the items share out. A scroll container shares none: its content keeps its natural size.
    let flex_main = if scrolls { natural_main } else { (own_main - pad_main).max(0.0) };

    let mut lines = break_lines(&items, wrap, flex_main, gap_main);
    for range in &lines {
        resolve_line(&mut items[range.clone()], flex_main, gap_main);
    }

    // The cross size each item wants now its main size is final.
    for it in items.iter_mut() {
        if (it.target - it.asked_main).abs() <= 0.01 {
            it.cross = it.asked_cross;
            continue;
        }
        let child = &mut node.children_mut()[it.idx];
        let again = compute(
            child,
            cx,
            LayoutInput {
                known: Pair::along(main, Some(it.target), it.cross_known),
                parent: frame.child_parent,
                avail: Pair::along(main, Avail::Definite(it.target), it.cross_avail),
                mode: Mode::Measure,
            },
        );
        it.cross = along(again.size, cross);
        it.baseline = again.baseline;
    }

    // Each line is as tall as its tallest item (baseline-aligned items count by ascent and descent),
    // or as tall as the container when there is a single line and the container's height is decided.
    struct Line {
        range: Range<usize>,
        cross: f32,
        ascent: f32,
        offset: f32,
    }
    let single_known_cross = if lines.len() == 1 { inner_known.get(cross) } else { None };
    let mut laid: Vec<Line> = Vec::new();
    for range in lines.drain(..) {
        let (mut ascent, mut descent, mut plain) = (0.0f32, 0.0f32, 0.0f32);
        for it in &items[range.clone()] {
            let outer = it.cross + it.m_cross;
            if it.align == Align::Baseline && cross == Axis::Y {
                let a = it.margin.top + it.baseline.unwrap_or(it.cross);
                ascent = ascent.max(a);
                descent = descent.max(outer - a);
            } else {
                plain = plain.max(outer);
            }
        }
        let cross_size = single_known_cross.unwrap_or_else(|| plain.max(ascent + descent));
        laid.push(Line { range, cross: cross_size, ascent, offset: 0.0 });
    }
    let lines_cross: f32 = laid.iter().map(|l| l.cross).sum::<f32>() + gap_cross * laid.len().saturating_sub(1) as f32;
    let own_cross = own.known.get(cross).unwrap_or_else(|| clamp_size(lines_cross + pad_cross, own.min.get(cross), own.max.get(cross)));

    let mut offset = 0.0;
    for line in laid.iter_mut() {
        line.offset = offset;
        offset += line.cross + gap_cross;
    }

    for line in &laid {
        let slice = &mut items[line.range.clone()];
        position_line(slice, flex_main, gap_main, style.justify, main);
        for it in slice.iter_mut() {
            it.final_cross = if it.stretch { clamp_size((line.cross - it.m_cross).max(0.0), it.min_cross, it.max_cross) } else { it.cross };
            let free = (line.cross - it.m_cross - it.final_cross).max(0.0);
            let (ms, me) = (it.margin.start(cross), it.margin.end(cross));
            it.cross_pos = match (it.auto.start(cross), it.auto.end(cross)) {
                (true, true) => free / 2.0,
                (true, false) => free,
                (false, true) => ms,
                (false, false) => match it.align {
                    Align::Start | Align::Stretch => ms,
                    Align::End => (line.cross - it.final_cross - me).max(ms),
                    Align::Center => ms + free / 2.0,
                    Align::Baseline if cross == Axis::Y => line.ascent - it.baseline.unwrap_or(it.cross),
                    Align::Baseline => ms,
                },
            } + line.offset;
        }
    }

    let box_size = size(
        if main == Axis::X { own_main } else { own_cross },
        if main == Axis::Y { own_main } else { own_cross },
    );

    // Origins inside the border box, and how far the content reaches.
    let mut reach = (pad.left, pad.top);
    let mut origins = Vec::with_capacity(n);
    for it in &items {
        let (mx, my) = if main == Axis::X { (it.main_pos, it.cross_pos) } else { (it.cross_pos, it.main_pos) };
        let origin = pt(pad.left + mx, pad.top + my);
        let (w, h) = if main == Axis::X { (it.target, it.final_cross) } else { (it.final_cross, it.target) };
        reach.0 = reach.0.max(origin.x + w + it.margin.right);
        reach.1 = reach.1.max(origin.y + h + it.margin.bottom);
        origins.push((origin, w, h));
    }

    let baseline = items.first().and_then(|it| it.baseline.map(|b| origins[0].0.y + b));

    if !place {
        return LayoutOutput { size: box_size, content: box_size, baseline };
    }

    for (it, (origin, w, h)) in items.iter().zip(origins) {
        let child = &mut node.children_mut()[it.idx];
        compute(
            child,
            cx,
            LayoutInput {
                known: Pair::new(Some(w), Some(h)),
                parent: frame.child_parent,
                avail: Pair::new(Avail::Definite(w), Avail::Definite(h)),
                mode: Mode::Place,
            },
        );
        child.layout_mut().origin = origin;
    }
    for idx in absolute {
        let child = &mut node.children_mut()[idx];
        let far = place_absolute(child, cx, box_size, &style, pad, main);
        reach.0 = reach.0.max(far.x);
        reach.1 = reach.1.max(far.y);
    }

    let content = size(reach.0 + pad.right, reach.1 + pad.bottom);
    LayoutOutput { size: box_size, content, baseline }
}

/// Places an absolute child against its parent's box and returns its far corner (margin included).
fn place_absolute<C, N: LayoutNode<C>>(child: &mut N, cx: &mut C, cb: Size, parent: &Style, pad: Edges, main: Axis) -> Point {
    let cs = *child.style();
    let (margin, _) = resolve_margin(&cs.margin, cb.w);
    let cbp = Pair::new(cb.w, cb.h);

    let mut known = Pair::new(None, None);
    let mut avail = Pair::new(Avail::MaxContent, Avail::MaxContent);
    let mut edges = [(None, None); 2];
    for (i, axis) in [Axis::X, Axis::Y].into_iter().enumerate() {
        let room = cbp.get(axis);
        let start = cs.inset.start(axis).resolve(Some(room));
        let end = cs.inset.end(axis).resolve(Some(room));
        let (ms, me) = (margin.start(axis), margin.end(axis));
        let preferred = match axis {
            Axis::X => cs.width,
            Axis::Y => cs.height,
        }
        .resolve(Some(room));
        let decided = preferred.or_else(|| match (start, end) {
            (Some(s), Some(e)) => Some((room - s - e - ms - me).max(0.0)),
            _ => None,
        });
        // A float is drawn above everything and not cut off, so it is not held to the room its
        // parent has left: without a size it takes its content's.
        let offer = match decided {
            None if cs.float => Avail::MaxContent,
            _ => Avail::Definite(decided.unwrap_or_else(|| (room - start.unwrap_or(0.0) - end.unwrap_or(0.0) - ms - me).max(0.0))),
        };
        match axis {
            Axis::X => (known.x, avail.x) = (decided, offer),
            Axis::Y => (known.y, avail.y) = (decided, offer),
        }
        edges[i] = (start, end);
    }

    let out = compute(child, cx, LayoutInput { known, parent: Pair::new(Some(cb.w), Some(cb.h)), avail, mode: Mode::Place });

    let mut at = [0.0f32; 2];
    let mut far = [0.0f32; 2];
    for (i, axis) in [Axis::X, Axis::Y].into_iter().enumerate() {
        let room = cbp.get(axis);
        let extent = along(out.size, axis);
        let (ms, me) = (margin.start(axis), margin.end(axis));
        let (start, end) = edges[i];
        let (pad_start, pad_end) = (pad.start(axis), pad.end(axis));
        let inner = room - pad_start - pad_end;
        // With no inset the child sits where its parent's alignment would put it alone.
        let centered = pad_start + ms + ((inner - extent - ms - me) / 2.0).max(0.0);
        let aligned = if axis == main {
            match parent.justify {
                Justify::Start | Justify::SpaceBetween => pad_start + ms,
                Justify::Center | Justify::SpaceAround | Justify::SpaceEvenly => centered,
                Justify::End => (room - pad_end - me - extent).max(pad_start + ms),
            }
        } else {
            match cs.align_self.unwrap_or(parent.align_items) {
                Align::Start | Align::Stretch | Align::Baseline => pad_start + ms,
                Align::Center => centered,
                Align::End => (room - pad_end - me - extent).max(pad_start + ms),
            }
        };
        at[i] = match (start, end) {
            (Some(s), _) => s + ms,
            (None, Some(e)) => room - e - me - extent,
            (None, None) => aligned,
        };
        far[i] = at[i] + extent + me;
    }
    child.layout_mut().origin = pt(at[0], at[1]);
    pt(far[0], far[1])
}

#[cfg(test)]
mod tests;
