//! Children taken out of the flow: absolute ones against their parent and fixed ones against the
//! window.

use super::*;

/// Lays out a fixed element against the window: inset from its edges, or centred without insets.
/// Its origin is in window coordinates.
pub fn place_fixed<C, N: LayoutNode<C>>(node: &mut N, cx: &mut C, window: Size) {
    let mut parent = Style::default();
    parent.justify = Justify::Center;
    parent.align_items = Align::Center;
    place_absolute(node, cx, window, &parent, Edges::default(), Axis::Y);
}

/// Places an absolute child against its parent's box and returns its far corner (margin included).
pub(crate) fn place_absolute<C, N: LayoutNode<C>>(child: &mut N, cx: &mut C, cb: Size, parent: &Style, pad: Edges, main: Axis) -> Point {
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
