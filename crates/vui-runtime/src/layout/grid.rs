//! The grid container: children placed left to right, top to bottom into columns, spanning as many
//! as they ask for, in rows as tall as their tallest cell.
//!
//! Like the flex engine it follows CSS where intuition comes from (`grid-template-columns` with
//! pixel, `fr` and `auto` tracks, `repeat(auto-fill, minmax(..., 1fr))`, `grid-column: span n`,
//! auto rows), with these differences:
//! - placement is always automatic, in order; there are no named lines or areas;
//! - a cell's content fills the cell's width, and `align_items` / `align_self` place it in the row's
//!   height (stretch by default);
//! - without a definite width, `fr` columns are as wide as their content, as `auto` ones are.

use super::*;
use crate::style::{Columns, Grid, Track};

/// One child in the grid, and the cell it landed in.
struct Cell {
    idx: usize,
    row: usize,
    col: usize,
    span: usize,
    margin: Edges,
    align: Align,
    /// Its height is decided by its own style, so it is not stretched.
    fixed_h: bool,
    h: f32,
}

/// The columns' tracks for a content box `width` wide (`None` while it is being worked out).
fn tracks(grid: &Grid, width: Option<f32>, gap: f32, offered: Avail, children: usize) -> (Vec<Track>, f32) {
    match grid.columns {
        Columns::Tracks(list, len) => (list[..len as usize].to_vec(), 0.0),
        Columns::Fill(min) => {
            let n = match (width, offered) {
                (Some(w), _) => ((w + gap) / (min + gap)).floor().max(1.0) as usize,
                // As wide as it would like: everything on one line.
                (None, Avail::MaxContent) => children.max(1),
                (None, _) => 1,
            };
            (vec![Track::Fr(1.0); n], min)
        }
    }
}

pub(crate) fn grid<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, input: &LayoutInput, own: &Own) -> LayoutOutput {
    let style = own.style;
    let spec = style.grid.unwrap_or(Grid { columns: Columns::equal(1), row: None });
    let place = input.mode == Mode::Place;
    let pad = own.pad;
    let (gap_x, gap_y) = (style.gap.x, style.gap.y);
    let inner_known = own.inner_known();
    let offered = own.offered(input);
    let width = inner_known.x.or(offered.x.definite());
    let child_parent = Pair::new(inner_known.x, if style.overflow == Overflow::Scroll { None } else { inner_known.y });

    // The children in the flow, and the ones taken out of it.
    let mut flow = Vec::new();
    let mut absolute = Vec::new();
    for idx in 0..node.children_mut().len() {
        let cs = *node.children_mut()[idx].style();
        if cs.hidden || cs.position == Position::Fixed {
            continue;
        }
        if cs.position == Position::Absolute {
            absolute.push(idx);
        } else {
            flow.push(idx);
        }
    }

    let (tracks, min_track) = tracks(&spec, width, gap_x, offered.x, flow.len());
    let cols = tracks.len();

    // Auto placement: in order, moving to the next row when a span does not fit.
    let mut cells = Vec::with_capacity(flow.len());
    let (mut row, mut col) = (0, 0);
    for &idx in &flow {
        let cs = *node.children_mut()[idx].style();
        let span = (cs.span.max(1) as usize).min(cols);
        if col + span > cols {
            row += 1;
            col = 0;
        }
        let (margin, _) = resolve_margin(&cs.margin, width.unwrap_or(0.0));
        let align = cs.align_self.unwrap_or(style.align_items);
        let fixed_h = cs.height.resolve(child_parent.y).is_some() || cs.aspect_ratio.is_some();
        cells.push(Cell { idx, row, col, span, margin, align, fixed_h, h: 0.0 });
        col += span;
        if col == cols {
            row += 1;
            col = 0;
        }
    }
    let rows = cells.last().map_or(0, |c| c.row + 1);

    // Column widths. Auto columns, and every column while the width is undecided, take the widest
    // single-column cell in them.
    let content_avail = if matches!(offered.x, Avail::MinContent) { Avail::MinContent } else { Avail::MaxContent };
    let intrinsic = |t: Track| matches!(t, Track::Auto) || (matches!(t, Track::Fr(_)) && width.is_none());
    let mut widths = vec![0.0f32; cols];
    for (c, t) in tracks.iter().enumerate() {
        if let Track::Px(v) = t {
            widths[c] = v.max(0.0);
        }
    }
    if tracks.iter().any(|&t| intrinsic(t)) {
        for cell in cells.iter().filter(|c| c.span == 1 && intrinsic(tracks[c.col])) {
            let child = &mut node.children_mut()[cell.idx];
            let out = compute(
                child,
                cx,
                LayoutInput {
                    known: Pair::new(None, None),
                    parent: child_parent,
                    avail: Pair::new(content_avail, Avail::MaxContent),
                    mode: Mode::Measure,
                },
            );
            widths[cell.col] = widths[cell.col].max(out.size.w + cell.margin.horizontal());
        }
        for (c, t) in tracks.iter().enumerate() {
            if intrinsic(*t) {
                widths[c] = widths[c].max(min_track);
            }
        }
    }
    if let Some(w) = width {
        let gaps = gap_x * cols.saturating_sub(1) as f32;
        let used: f32 = tracks.iter().zip(&widths).filter(|(t, _)| !matches!(t, Track::Fr(_))).map(|(_, w)| *w).sum();
        let weight: f32 = tracks.iter().filter_map(|t| if let Track::Fr(f) = t { Some(f.max(0.0)) } else { None }).sum();
        let free = (w - gaps - used).max(0.0);
        for (c, t) in tracks.iter().enumerate() {
            if let Track::Fr(f) = t {
                widths[c] = if weight > 0.0 { free * f.max(0.0) / weight } else { 0.0 };
            }
        }
    }
    let mut xs = Vec::with_capacity(cols);
    let mut x = 0.0;
    for w in &widths {
        xs.push(x);
        x += w + gap_x;
    }
    let cell_width = |c: &Cell| widths[c.col..c.col + c.span].iter().sum::<f32>() + gap_x * (c.span - 1) as f32;

    // Row heights: fixed, or the tallest cell in the row at its column's width.
    let mut heights = vec![spec.row.unwrap_or(0.0); rows];
    for cell in cells.iter_mut() {
        let w = (cell_width(cell) - cell.margin.horizontal()).max(0.0);
        let child = &mut node.children_mut()[cell.idx];
        let out = compute(
            child,
            cx,
            LayoutInput {
                known: Pair::new(Some(w), None),
                parent: child_parent,
                avail: Pair::new(Avail::Definite(w), spec.row.map_or(Avail::MaxContent, Avail::Definite)),
                mode: Mode::Measure,
            },
        );
        cell.h = out.size.h;
        if spec.row.is_none() {
            heights[cell.row] = heights[cell.row].max(out.size.h + cell.margin.vertical());
        }
    }
    let mut ys = Vec::with_capacity(rows);
    let mut y = 0.0;
    for h in &heights {
        ys.push(y);
        y += h + gap_y;
    }

    let content_w = widths.iter().sum::<f32>() + gap_x * cols.saturating_sub(1) as f32;
    let content_h = heights.iter().sum::<f32>() + gap_y * rows.saturating_sub(1) as f32;
    let box_size = size(
        own.known.x.unwrap_or_else(|| clamp_size(content_w + own.pad_sum.x, own.min.x, own.max.x)),
        own.known.y.unwrap_or_else(|| clamp_size(content_h + own.pad_sum.y, own.min.y, own.max.y)),
    );

    let mut reach = (pad.left + content_w, pad.top + content_h);
    if !place {
        return LayoutOutput { size: box_size, content: box_size, baseline: None };
    }

    for cell in &cells {
        let w = (cell_width(cell) - cell.margin.horizontal()).max(0.0);
        let room = (heights[cell.row] - cell.margin.vertical()).max(0.0);
        let h = if cell.align == Align::Stretch && !cell.fixed_h { room } else { cell.h.min(room.max(cell.h)) };
        let dy = match cell.align {
            Align::Center => (room - h) / 2.0,
            Align::End => room - h,
            _ => 0.0,
        }
        .max(0.0);
        let origin = pt(pad.left + xs[cell.col] + cell.margin.left, pad.top + ys[cell.row] + cell.margin.top + dy);
        let child = &mut node.children_mut()[cell.idx];
        compute(
            child,
            cx,
            LayoutInput {
                known: Pair::new(Some(w), Some(h)),
                parent: child_parent,
                avail: Pair::new(Avail::Definite(w), Avail::Definite(h)),
                mode: Mode::Place,
            },
        );
        child.layout_mut().origin = origin;
        reach.1 = reach.1.max(origin.y + h + cell.margin.bottom);
    }
    for idx in absolute {
        let child = &mut node.children_mut()[idx];
        let far = place_absolute(child, cx, box_size, &style, pad, Axis::Y);
        reach.0 = reach.0.max(far.x);
        reach.1 = reach.1.max(far.y);
    }
    let content = size(reach.0 + pad.right, reach.1 + pad.bottom);
    LayoutOutput { size: box_size, content, baseline: None }
}
