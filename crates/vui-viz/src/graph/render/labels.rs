//! Words on the graph: node names that never overlap, edge names, the legend and the hover card.

use super::*;

pub(super) fn labels(
    canvas: &mut Canvas,
    f: &Frame,
    centre: Option<usize>,
    bright: Option<&HashSet<usize>>,
    link_bright: &dyn Fn(usize) -> bool,
) {
    let n = f.pos.len();
    // Where labels may not go: on top of a node or of another label. Each remembers whose it is, so a
    // node's own label may sit beside its own disc.
    let mut placed: Vec<(usize, Rect)> = Vec::new();
    for i in 0..n {
        let s = f.screen(i);
        if f.node_alive(i) && f.area.inflate(20.0, 20.0).contains(s) {
            let r = f.radius(i) + 1.0;
            placed.push((i, rect(s.x - r, s.y - r, r * 2.0, r * 2.0)));
        }
    }
    let mut done = vec![false; n];
    // The nodes that always get a name, most important first. The hovered one is named by its card.
    let mut forced: Vec<usize> = f.selected.into_iter().collect();
    if let Some(set) = bright {
        forced.extend(
            f.graph
                .order
                .iter()
                .copied()
                .filter(|i| set.contains(i) && Some(*i) != f.selected && Some(*i) != f.hover)
                .take(40),
        );
    }
    if let Some(h) = f.hover {
        done[h] = true;
    }
    for i in forced {
        if done[i] || !f.node_alive(i) {
            continue;
        }
        done[i] = true;
        name(
            canvas,
            f,
            i,
            true,
            Some(i) == f.selected || Some(i) == centre,
            &mut placed,
        );
    }
    let budget = if n <= 60 {
        n
    } else {
        (f.camera.zoom * 14.0) as usize + 6
    };
    let mut shown = 0;
    for &i in &f.graph.order {
        if shown >= budget {
            break;
        }
        if done[i] || !f.node_alive(i) || bright.is_some() {
            continue;
        }
        let s = f.screen(i);
        if f.area.inflate(-4.0, -4.0).contains(s) && name(canvas, f, i, false, false, &mut placed) {
            shown += 1;
        }
        done[i] = true;
    }
    edge_names(canvas, f, bright.is_some(), link_bright, &mut placed);
}

/// Writes node `i`'s label beside it, on the right if there is room and the left if not, unless that would
/// land on a node or a label already there. Returns whether it did.
pub(super) fn name(
    canvas: &mut Canvas,
    f: &Frame,
    i: usize,
    forced: bool,
    emphatic: bool,
    placed: &mut Vec<(usize, Rect)>,
) -> bool {
    let style = f.style;
    let text = truncate(&f.data.nodes[i].label, 28);
    let s = f.screen(i);
    let r = f.radius(i);
    let w = canvas.measure(&text, &style.label).w;
    let h = style.label.line_height;
    let fits_right = s.x + r + 5.0 + w <= f.area.right() - 2.0;
    let fits_left = s.x - r - 5.0 - w >= f.area.x + 2.0;
    let free = |spot: Rect| {
        !placed
            .iter()
            .any(|(owner, p)| *owner != i && p.intersects(spot.inflate(3.0, 1.0)))
    };
    let right = rect(s.x + r + 5.0, s.y - h / 2.0, w, h);
    let left = rect(s.x - r - 5.0 - w, s.y - h / 2.0, w, h);
    let (spot, on_right) = if fits_right && free(right) {
        (right, true)
    } else if fits_left && free(left) {
        (left, false)
    } else if forced {
        (if fits_right { right } else { left }, fits_right)
    } else {
        return false;
    };
    if forced {
        canvas.quad(
            Quad::new(spot.inflate(4.0, 1.0))
                .fill(style.bg.mul_alpha(0.85))
                .radii(Corners::all(3.0)),
        );
    }
    let color = if emphatic || forced {
        style.ink
    } else {
        style.ink_2
    };
    let (at, anchor) = if on_right {
        (pt(spot.x, s.y), HAnchor::Left)
    } else {
        (pt(spot.right(), s.y), HAnchor::Right)
    };
    canvas.text(&text, &style.label, at, anchor, VAnchor::Middle, color);
    placed.push((usize::MAX, spot.inflate(3.0, 1.0)));
    true
}

/// What each link says, where there is room: always for the links being looked at, and for the rest once
/// zoomed in far enough to read them.
pub(super) fn edge_names(
    canvas: &mut Canvas,
    f: &Frame,
    lens: bool,
    link_bright: &dyn Fn(usize) -> bool,
    placed: &mut Vec<(usize, Rect)>,
) {
    let style = f.style;
    let zoomed = f.camera.zoom >= 2.0;
    let mut count = 0;
    for (link, &(a, b, e)) in f.graph.links.iter().enumerate() {
        if count >= 40 {
            break;
        }
        let wanted = if lens { link_bright(link) } else { zoomed };
        let text = &f.data.edges[e].label;
        if !wanted || text.is_empty() || !f.link_alive(link) {
            continue;
        }
        let (pa, pb) = (f.screen(a), f.screen(b));
        let mid = pt((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
        let w = canvas.measure(text, &style.label).w;
        if !f.area.contains(mid) || pa.distance(pb) < w + 24.0 {
            continue;
        }
        let spot = rect(
            mid.x - w / 2.0,
            mid.y - style.label.line_height / 2.0,
            w,
            style.label.line_height,
        );
        if placed
            .iter()
            .any(|(_, p)| p.intersects(spot.inflate(2.0, 1.0)))
        {
            continue;
        }
        canvas.quad(
            Quad::new(spot.inflate(4.0, 0.0))
                .fill(style.bg.mul_alpha(0.9))
                .radii(Corners::all(3.0)),
        );
        canvas.text(
            text,
            &style.label,
            mid,
            HAnchor::Center,
            VAnchor::Middle,
            style.ink_3,
        );
        placed.push((usize::MAX, spot));
        count += 1;
    }
}

/// The kinds on show, bottom left: a swatch and a name each. Skipped when there is only one kind.
pub(super) fn legend(canvas: &mut Canvas, f: &Frame) {
    let style = f.style;
    let kinds: Vec<u8> = {
        let mut seen: Vec<u8> = f.data.nodes.iter().map(|n| n.kind).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.into_iter()
            .filter(|k| (*k as usize) < f.data.kinds.len())
            .collect()
    };
    if kinds.len() < 2 {
        return;
    }
    let row = 18.0;
    let top = f.area.bottom() - 12.0 - row * kinds.len() as f32;
    for (n, k) in kinds.iter().enumerate() {
        let y = top + row * n as f32 + row / 2.0;
        let (color, hollow) = kind_look(style, *k);
        let at = pt(f.area.x + 18.0, y);
        if hollow {
            canvas.ring(at, 4.0, 1.5, style.bg, color);
        } else {
            canvas.dot(at, 4.0, color);
        }
        canvas.text(
            &f.data.kinds[*k as usize],
            &style.label,
            pt(f.area.x + 30.0, y),
            HAnchor::Left,
            VAnchor::Middle,
            style.ink_3,
        );
    }
}

pub(super) fn tooltip(canvas: &mut Canvas, f: &Frame, i: usize) {
    let style = f.style;
    if !f.node_alive(i) {
        return;
    }
    let node = &f.data.nodes[i];
    let kind = f
        .data
        .kinds
        .get(node.kind as usize)
        .cloned()
        .unwrap_or_default();
    let links = f.graph.around[i].len();
    let mut lines: Vec<(String, Color)> = vec![(truncate(&node.label, 40), style.ink)];
    let sub = if kind.is_empty() {
        format!("{links} links")
    } else {
        format!("{kind} · {links} links")
    };
    lines.push((sub, style.ink_3));
    if !node.detail.is_empty() {
        lines.push((truncate(&node.detail, 48), style.ink_2));
    }
    let pad = 10.0;
    let line_h = 18.0;
    let w = lines
        .iter()
        .map(|(t, _)| canvas.measure(t, &style.label).w)
        .fold(0.0, f32::max)
        + pad * 2.0;
    let h = pad * 2.0 - 4.0 + line_h * lines.len() as f32;
    let s = f.screen(i);
    let r = f.radius(i);
    let x = if s.x + r + 12.0 + w > f.area.right() {
        s.x - r - 12.0 - w
    } else {
        s.x + r + 12.0
    };
    let y = (s.y - h / 2.0).clamp(
        f.area.y + 4.0,
        (f.area.bottom() - h - 4.0).max(f.area.y + 4.0),
    );
    let box_ = rect(x.max(f.area.x + 4.0), y, w, h);
    canvas.quad(
        Quad::new(box_)
            .fill(style.bg)
            .radii(Corners::all(6.0))
            .border(1.0, style.line),
    );
    for (n, (t, c)) in lines.iter().enumerate() {
        canvas.text(
            t,
            &style.label,
            pt(
                box_.x + pad,
                box_.y + pad - 2.0 + line_h * n as f32 + line_h / 2.0,
            ),
            HAnchor::Left,
            VAnchor::Middle,
            *c,
        );
    }
}

/// `text` cut to `max` characters, with an ellipsis when something was left off.
pub(super) fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
