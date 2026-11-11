//! Scrolling: the wheel and trackpad, scrollbar thumbs, and revealing what gains focus.

use super::*;

impl<M: 'static> Run<'_, M> {
    pub(crate) fn scroll(&mut self, event: &Event, pos: Point, delta: Vec2, precise: bool) {
        self.core.pointer = Some(pos);
        let Some(path) = hit(self.root, pos) else { return };
        if self.disabled_on(&path) || self.deliver(&path, &WidgetEvent::Input(event)) {
            return;
        }
        for k in (0..=path.len()).rev() {
            let node = node_at_mut(self.root, &path[..k]);
            if node.eff.overflow != Overflow::Scroll {
                continue;
            }
            let target = node.scroll.target();
            let max = node.scroll.max;
            let wanted = vec2((target.x - delta.x).clamp(0.0, max.x), (target.y - delta.y).clamp(0.0, max.y));
            let move_x = delta.x != 0.0 && (wanted.x - target.x).abs() > 0.01;
            let move_y = delta.y != 0.0 && (wanted.y - target.y).abs() > 0.01;
            if !move_x && !move_y {
                continue;
            }
            for (moves, axis, to) in [(move_x, Axis::X, wanted.x), (move_y, Axis::Y, wanted.y)] {
                if !moves {
                    continue;
                }
                let spring = node.scroll.spring_mut(axis);
                if precise {
                    spring.snap_to(to);
                } else {
                    spring.set_target(to);
                }
            }
            node.scroll.poke();
            self.core.arrange_dirty = true;
            self.core.animating = true;
            return;
        }
    }

    /// Scrolls the containers around the node at `path` until `target` (window coordinates) is in view.
    pub fn reveal(&mut self, path: &[usize], mut target: Rect) {
        const MARGIN: f32 = 8.0;
        for k in (0..=path.len()).rev() {
            let node = node_at_mut(self.root, &path[..k]);
            if node.eff.overflow != Overflow::Scroll {
                continue;
            }
            let view = node.rect;
            let current = node.scroll.offset();
            let max = node.scroll.max;
            let margin_x = MARGIN.min(view.w / 2.0);
            let margin_y = MARGIN.min(view.h / 2.0);
            let shift = |lo: f32, hi: f32, view_lo: f32, view_hi: f32, margin: f32| {
                if lo < view_lo + margin {
                    lo - (view_lo + margin)
                } else if hi > view_hi - margin {
                    (hi - (view_hi - margin)).min(lo - (view_lo + margin))
                } else {
                    0.0
                }
            };
            let dx = shift(target.x, target.right(), view.x, view.right(), margin_x);
            let dy = shift(target.y, target.bottom(), view.y, view.bottom(), margin_y);
            let to = vec2((current.x + dx).clamp(0.0, max.x), (current.y + dy).clamp(0.0, max.y));
            if to.x != current.x {
                node.scroll.x.set_target(to.x);
            }
            if to.y != current.y {
                node.scroll.y.set_target(to.y);
            }
            node.scroll.poke();
            target.x -= to.x - current.x;
            target.y -= to.y - current.y;
            self.core.animating = true;
        }
    }

    /// Starts dragging a scrollbar thumb under the pointer, if there is one.
    pub(crate) fn grab_bar(&mut self, pos: Point) -> bool {
        let Some(path) = hit(self.root, pos) else { return false };
        for k in (0..=path.len()).rev() {
            let node = node_at(self.root, &path[..k]);
            for axis in [Axis::Y, Axis::X] {
                let Some(bar) = node.bar(axis) else { continue };
                if node.scroll.bars.value() < 0.05 || !bar.thumb.inflate(3.0, 3.0).contains(pos) {
                    continue;
                }
                let grab = match axis {
                    Axis::X => pos.x - bar.thumb.x,
                    Axis::Y => pos.y - bar.thumb.y,
                };
                let id = node.id;
                self.core.bar_drag = Some(BarDrag { id, axis, grab });
                let node = node_at_mut(self.root, &path[..k]);
                node.scroll.thick.set_target(1.0);
                node.scroll.poke();
                self.core.animating = true;
                return true;
            }
        }
        false
    }

    /// Follows the pointer with a dragged thumb. Returns true if a drag is going on.
    pub(crate) fn drag_bar(&mut self, pos: Point) -> bool {
        let Some(drag) = &self.core.bar_drag else { return false };
        let (id, axis, grab) = (drag.id, drag.axis, drag.grab);
        let Some(path) = path_of(self.root, id) else {
            self.core.bar_drag = None;
            return false;
        };
        let node = node_at_mut(self.root, &path);
        if let Some(bar) = node.bar(axis)
            && bar.free > 0.0
        {
            let (start, at) = match axis {
                Axis::X => (node.rect.x + 2.0, pos.x),
                Axis::Y => (node.rect.y + 2.0, pos.y),
            };
            let along = (at - grab - start).clamp(0.0, bar.free);
            node.scroll.spring_mut(axis).snap_to(along / bar.free * bar.max);
            node.scroll.poke();
            self.core.arrange_dirty = true;
        }
        true
    }

    /// Fattens the bar under the pointer, and shows the bars of any scroll container it is over.
    pub(crate) fn update_bar_hover(&mut self, pos: Option<Point>, path: Option<&[usize]>) {
        let (Some(pos), Some(path)) = (pos, path) else { return };
        for k in 0..=path.len() {
            let node = node_at_mut(self.root, &path[..k]);
            if node.eff.overflow != Overflow::Scroll {
                continue;
            }
            let dragging = self.core.bar_drag.as_ref().is_some_and(|d| d.id == node.id);
            let near = [Axis::Y, Axis::X]
                .into_iter()
                .filter_map(|a| node.bar(a))
                .any(|b| b.thumb.inflate(6.0, 6.0).contains(pos));
            node.scroll.thick.set_target(if near || dragging { 1.0 } else { 0.0 });
            if near {
                node.scroll.poke();
            }
            self.core.animating = true;
        }
    }
}
