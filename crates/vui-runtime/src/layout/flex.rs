//! The flex container: gathering items with their margins and limits, then growing, shrinking,
//! wrapping and aligning them.

use super::*;

/// One child taking part in a row or column.
pub(crate) struct Item {
    pub(crate) idx: usize,
    pub(crate) margin: Edges,
    pub(crate) auto: Sides<bool>,
    pub(crate) m_main: f32,
    pub(crate) m_cross: f32,
    pub(crate) align: Align,
    /// Its cross size is auto and may be widened to fill the line.
    pub(crate) stretch: bool,
    pub(crate) min_cross: f32,
    pub(crate) max_cross: f32,
    /// What was imposed on and offered to it across, so it can be measured again.
    pub(crate) cross_known: Option<f32>,
    pub(crate) cross_avail: Avail,
    /// The first measurement: the main size it asked for, and the cross size that went with it.
    pub(crate) asked_main: f32,
    pub(crate) asked_cross: f32,
    pub(crate) baseline: Option<f32>,
    pub(crate) grow: f32,
    pub(crate) shrink: f32,
    /// Flex base size, and its minimum, maximum and clamped ("hypothetical") size.
    pub(crate) base: f32,
    pub(crate) min_main: f32,
    pub(crate) max_main: f32,
    pub(crate) hyp: f32,
    /// The size after growing and shrinking.
    pub(crate) target: f32,
    pub(crate) frozen: bool,
    pub(crate) violation: f32,
    /// Cross size before stretching, and the final one.
    pub(crate) cross: f32,
    pub(crate) final_cross: f32,
    /// Position of the border box inside the container's content box.
    pub(crate) main_pos: f32,
    pub(crate) cross_pos: f32,
}

/// What the container decided before looking at its children.
pub(crate) struct Frame {
    pub(crate) main: Axis,
    pub(crate) cross: Axis,
    pub(crate) wrap: bool,
    pub(crate) min_content: bool,
    pub(crate) inner_known: Pair<Option<f32>>,
    pub(crate) offered: Pair<Avail>,
    /// What percentages in the children are a percentage of.
    pub(crate) child_parent: Pair<Option<f32>>,
}

pub(crate) fn resolve_margin(m: &Sides<Len>, percent_of: f32) -> (Edges, Sides<bool>) {
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
pub(crate) fn limits(cs: &Style, parent: Pair<Option<f32>>, axis: Axis) -> (Option<f32>, f32) {
    let (min, max, basis) = match axis {
        Axis::X => (cs.min_width, cs.max_width, parent.x),
        Axis::Y => (cs.min_height, cs.max_height, parent.y),
    };
    (min.resolve(basis), max.resolve(basis).unwrap_or(f32::INFINITY))
}

pub(crate) fn gather<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, style: &Style, f: &Frame) -> (Vec<Item>, Vec<usize>) {
    let (main, cross) = (f.main, f.cross);
    let mut items = Vec::new();
    let mut absolute = Vec::new();
    for idx in 0..node.children_mut().len() {
        let child = &mut node.children_mut()[idx];
        let cs = *child.style();
        // A fixed child is placed against the window after the tree is laid out (see `place_fixed`).
        if cs.hidden || cs.position == Position::Fixed {
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

/// A container with children.
pub(crate) fn flex<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, input: &LayoutInput, own: &Own) -> LayoutOutput {
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
