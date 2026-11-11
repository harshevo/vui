//! The pointer: hover, presses and releases, clicks that dismiss a layer, and tooltips.

use super::*;

impl<M: 'static> Run<'_, M> {
    pub(crate) fn pointer_moved(&mut self, pos: Point, mods: Modifiers) {
        self.core.pointer = Some(pos);
        self.core.mods = mods;
        if self.drag_bar(pos) {
            return;
        }
        if let Some(press) = &self.core.press {
            let (id, widget) = (press.id, press.widget);
            if let Some(path) = path_of(self.root, id) {
                let node = node_at_mut(self.root, &path);
                let inside = node.hit_self(pos);
                node.anim.press.set_target(if inside { 1.0 } else { 0.0 });
                self.core.animating = true;
                if let Some(p) = self.core.press.as_mut() {
                    p.inside = inside;
                }
                if widget {
                    self.call(&path, &WidgetEvent::Input(&Event::PointerMoved { pos, mods }));
                }
            }
            self.update_hover(Some(pos));
            return;
        }
        if let Some(path) = self.update_hover(Some(pos)) {
            self.deliver(&path, &WidgetEvent::Input(&Event::PointerMoved { pos, mods }));
        }
    }

    pub(crate) fn pointer_left(&mut self) {
        self.core.pointer = None;
        if self.core.press.is_none() {
            self.update_hover(None);
        }
    }

    pub(crate) fn pointer_down(&mut self, event: &Event, pos: Point, button: PointerButton, mods: Modifiers, count: u8) {
        if self.core.tip.block() {
            self.core.paint_dirty = true;
        }
        self.core.pointer = Some(pos);
        self.core.mods = mods;
        self.core.keyboard = false;
        if self.core.press.is_some() {
            // An earlier press never saw its release: end it before starting another.
            self.pointer_up(&Event::PointerUp { pos, button, mods }, pos, button, mods);
        }
        if button == PointerButton::Primary && self.grab_bar(pos) {
            return;
        }
        self.update_hover(Some(pos));
        let path = hit(self.root, pos);
        if self.dismiss_outside(path.as_deref()) {
            return;
        }
        let Some(path) = path else {
            self.set_focus_by(None, false, FocusBy::Pointer);
            return;
        };
        if self.disabled_on(&path) {
            return;
        }
        self.focus_for_press(&path);

        let click = Click { pos, mods, button, count };
        for k in (0..=path.len()).rev() {
            let at = &path[..k];
            if self.call(at, &WidgetEvent::Input(event)) {
                let id = node_at(self.root, at).id;
                self.begin_press(at, Press { id, button, count, widget: true, inside: true });
                return;
            }
            let node = node_at(self.root, at);
            match button {
                PointerButton::Secondary => {
                    if let Some(h) = &node.input.secondary {
                        let message = h(&click);
                        self.core.out.messages.push(message);
                        return;
                    }
                }
                PointerButton::Primary if node.input.clickable() => {
                    let id = node.id;
                    if let Some(h) = &node.input.press {
                        let message = h(&click);
                        self.core.out.messages.push(message);
                    }
                    self.begin_press(at, Press { id, button, count, widget: false, inside: true });
                    return;
                }
                _ => {}
            }
        }
    }

    /// A press outside elements that close on one (`on_dismiss`) closes them, and goes no further:
    /// the click that closes a menu does not also press what is under it.
    pub(crate) fn dismiss_outside(&mut self, hit: Option<&[usize]>) -> bool {
        let mut dismissed = false;
        for path in dismissible(self.root) {
            if hit.is_some_and(|h| h.starts_with(&path)) {
                continue;
            }
            if let Some(h) = &node_at(self.root, &path).input.dismiss {
                let message = h();
                self.core.out.messages.push(message);
                dismissed = true;
            }
        }
        dismissed
    }

    /// Escape that nothing focused answered closes the topmost element that closes on it.
    pub(crate) fn dismiss_top(&mut self) -> bool {
        let Some(path) = dismissible(self.root).pop() else { return false };
        let Some(h) = &node_at(self.root, &path).input.dismiss else { return false };
        let message = h();
        self.core.out.messages.push(message);
        true
    }

    pub(super) fn begin_press(&mut self, path: &[usize], press: Press) {
        node_at_mut(self.root, path).anim.press.set_target(1.0);
        self.core.press = Some(press);
        self.core.animating = true;
    }

    /// A press on a text field focuses it; a press on empty space gives focus up; a press on a button
    /// leaves focus where it was.
    pub(crate) fn focus_for_press(&mut self, path: &[usize]) {
        let nodes = chain(self.root, path);
        let modes: Vec<(NodeId, Focus)> = nodes.iter().rev().map(|n| (n.id, n.focus_mode())).collect();
        if let Some((id, _)) = modes.iter().find(|(_, f)| *f == Focus::Click) {
            let id = *id;
            self.set_focus_by(Some(id), false, FocusBy::Pointer);
        } else if modes.iter().all(|(_, f)| *f == Focus::Never) {
            self.set_focus_by(None, false, FocusBy::Pointer);
        }
    }

    pub(crate) fn pointer_up(&mut self, event: &Event, pos: Point, button: PointerButton, mods: Modifiers) {
        self.core.pointer = Some(pos);
        self.core.mods = mods;
        if button == PointerButton::Primary && self.core.bar_drag.take().is_some() {
            self.update_hover(Some(pos));
            return;
        }
        if self.core.press.as_ref().is_none_or(|p| p.button != button) {
            return;
        }
        let Some(press) = self.core.press.take() else { return };
        self.core.animating = true;
        if let Some(path) = path_of(self.root, press.id) {
            node_at_mut(self.root, &path).anim.press.set_target(0.0);
            if press.widget {
                self.call(&path, &WidgetEvent::Input(event));
            } else if node_at(self.root, &path).hit_self(pos) {
                let click = Click { pos, mods, button, count: press.count };
                let node = node_at(self.root, &path);
                if let Some(h) = &node.input.click {
                    let message = h(&click);
                    self.core.out.messages.push(message);
                }
                if press.count == 2 {
                    let node = node_at(self.root, &path);
                    if let Some(h) = &node.input.double_click {
                        let message = h(&click);
                        self.core.out.messages.push(message);
                    }
                }
            }
        }
        self.update_hover(Some(pos));
    }

    /// Works out what the pointer is over, tells nodes that gained or lost it, and picks the cursor.
    /// Returns the path to the topmost node under the pointer.
    pub(crate) fn update_hover(&mut self, pos: Option<Point>) -> Option<Vec<usize>> {
        let path = pos.and_then(|p| hit(self.root, p));
        let mut ids: Vec<NodeId> = Vec::new();
        if let Some(path) = &path {
            for node in chain(self.root, path) {
                if node.input.disabled {
                    break;
                }
                ids.push(node.id);
            }
        }
        if let Some(press) = &self.core.press
            && !ids.contains(&press.id)
        {
            ids.clear();
        }

        let old = std::mem::replace(&mut self.core.hover, ids.clone());
        for id in old.iter().rev().filter(|id| !ids.contains(id)) {
            self.hover_changed(*id, false);
        }
        for id in ids.iter().filter(|id| !old.contains(id)) {
            self.hover_changed(*id, true);
        }

        let cursor = match (&self.core.press, &path) {
            (Some(press), _) => path_of(self.root, press.id).and_then(|p| cursor_of(&chain(self.root, &p))),
            (None, Some(path)) => cursor_of(&chain(self.root, path)),
            (None, None) => None,
        }
        .unwrap_or(CursorIcon::Arrow);
        if cursor != self.core.cursor {
            self.core.cursor = cursor;
            self.core.out.effects.push(Effect::Cursor(cursor));
        }
        self.update_bar_hover(pos, path.as_deref());
        self.update_tip();
        path
    }

    /// Follows the innermost hovered element that has a tooltip.
    pub(crate) fn update_tip(&mut self) {
        let tipped = self.core.hover.iter().rev().copied().find(|&id| {
            path_of(self.root, id).is_some_and(|p| node_at(self.root, &p).input.tooltip.is_some())
        });
        let shown = self.core.tip.shown;
        if self.core.tip.hover(tipped, self.core.time) {
            self.core.wake(tooltip::TIP_DELAY);
        }
        if shown && !self.core.tip.shown {
            self.core.paint_dirty = true;
        }
    }

    pub(crate) fn hover_changed(&mut self, id: NodeId, on: bool) {
        let Some(path) = path_of(self.root, id) else { return };
        node_at_mut(self.root, &path).anim.hover.set_target(if on { 1.0 } else { 0.0 });
        self.core.animating = true;
        self.call(&path, &WidgetEvent::HoverChanged(on));
        let node = node_at(self.root, &path);
        if let Some(h) = &node.input.hover {
            let message = h(on);
            self.core.out.messages.push(message);
        }
    }
}
