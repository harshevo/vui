//! The keyboard and focus: keys to the focused element, Tab and focus groups, Escape, and the
//! window gaining and losing focus.

use super::*;

impl<M: 'static> Run<'_, M> {
    pub fn set_focus(&mut self, target: Option<NodeId>, visible: bool) {
        self.set_focus_by(target, visible, FocusBy::Other);
    }

    pub(crate) fn set_focus_by(&mut self, target: Option<NodeId>, visible: bool, by: FocusBy) {
        let visible = visible && target.is_some();
        let old = self.core.focus;
        self.core.focus_visible = visible;
        self.core.animating = true;
        self.core.paint_dirty = true;
        if old == target {
            if let Some(path) = target.and_then(|t| path_of(self.root, t)) {
                node_at_mut(self.root, &path).anim.focus.set_target(if visible { 1.0 } else { 0.0 });
            }
            return;
        }
        self.core.focus = target;
        self.remember(target);
        self.core.out.effects.push(Effect::ImeArea(None));
        if let Some(path) = old.and_then(|o| path_of(self.root, o)) {
            node_at_mut(self.root, &path).anim.focus.set_target(0.0);
            self.call(&path, &WidgetEvent::FocusChanged { focused: false, visible: false, by });
        }
        if let Some(path) = target.and_then(|t| path_of(self.root, t)) {
            node_at_mut(self.root, &path).anim.focus.set_target(if visible { 1.0 } else { 0.0 });
            self.call(&path, &WidgetEvent::FocusChanged { focused: true, visible, by });
        }
    }

    /// Tells the focus group around `target` which of its items had focus last.
    pub(crate) fn remember(&mut self, target: Option<NodeId>) {
        let Some(target) = target else { return };
        let Some(path) = path_of(self.root, target) else { return };
        let group = chain(self.root, &path)[..path.len()].iter().rposition(|n| n.input.group.is_some());
        if let Some(k) = group {
            node_at_mut(self.root, &path[..k]).memory = Some(target);
        }
    }

    pub(crate) fn window_focus(&mut self, focused: bool) {
        self.core.window_focused = focused;
        self.core.paint_dirty = true;
        if let Some(path) = self.core.focus.and_then(|f| path_of(self.root, f)) {
            self.call(&path, &WidgetEvent::WindowFocus(focused));
        }
    }

    pub(crate) fn key(&mut self, event: &Event, key: &KeyEvent) {
        if key.pressed && self.core.tip.block() {
            self.core.paint_dirty = true;
        }
        self.core.mods = key.mods;
        if key.pressed {
            self.core.keyboard = true;
        }
        let focus_path = self.core.focus.and_then(|f| path_of(self.root, f));
        let path = focus_path.unwrap_or_default();
        if self.disabled_on(&path) {
            return;
        }
        // From the focused element up to the root: each widget, then each `on_key` handler.
        for k in (0..=path.len()).rev() {
            let at = &path[..k];
            if self.call(at, &WidgetEvent::Input(event)) {
                return;
            }
            if let Some(h) = &node_at(self.root, at).input.key
                && let Some(message) = h(key)
            {
                self.core.out.messages.push(message);
                return;
            }
        }
        if !key.pressed {
            return;
        }
        if key.key == vui_core::Key::Named(NamedKey::Escape) && !key.mods.any() && self.dismiss_top() {
            return;
        }
        match key.key {
            vui_core::Key::Named(NamedKey::Tab) if !key.mods.cmd && !key.mods.ctrl && !key.mods.alt => {
                self.tab(key.mods.shift);
            }
            vui_core::Key::Named(NamedKey::Enter | NamedKey::Space)
                if !key.mods.any() && !key.repeat && (!path.is_empty() || self.core.focus.is_some()) =>
            {
                self.activate(&path);
            }
            vui_core::Key::Named(
                named @ (NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::Home | NamedKey::End),
            ) if !key.mods.any() => self.group_key(named),
            _ => {}
        }
    }

    /// Moves focus to the next (or previous) Tab stop, wrapping around. A focus group is one stop.
    pub(crate) fn tab(&mut self, backwards: bool) {
        // Inside a focus trap, Tab goes round its stops only.
        let stops = match focus_trap(self.root) {
            Some(path) => collect_stops(node_at(self.root, &path)),
            None => collect_stops(self.root),
        };
        let n = stops.len();
        if n == 0 {
            return;
        }
        let current = self.core.focus.and_then(|f| stops.iter().position(|s| s.members.contains(&f)));
        let next = match (current, backwards) {
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
            (None, false) => 0,
            (None, true) => n - 1,
        };
        self.focus_and_reveal(stops[next].entry, FocusBy::Tab);
    }

    /// Arrow keys, Home and End inside a focus group move focus between its items.
    pub(crate) fn group_key(&mut self, key: NamedKey) {
        let Some(focus) = self.core.focus else { return };
        let Some(path) = path_of(self.root, focus) else { return };
        let nodes = chain(self.root, &path);
        let Some(g) = nodes[..path.len()].iter().rposition(|n| n.input.group.is_some()) else { return };
        let Some(group) = nodes[g].input.group else { return };
        let items = group_items(nodes[g]);
        let Some(at) = items.iter().position(|&id| id == focus) else { return };
        let vertical = group.axis != Some(Axis::X);
        let horizontal = group.axis != Some(Axis::Y);
        let step: isize = match key {
            NamedKey::ArrowDown if vertical => 1,
            NamedKey::ArrowUp if vertical => -1,
            NamedKey::ArrowRight if horizontal => 1,
            NamedKey::ArrowLeft if horizontal => -1,
            NamedKey::Home => -(at as isize),
            NamedKey::End => items.len() as isize - 1 - at as isize,
            _ => return,
        };
        let last = items.len() as isize - 1;
        let to = at as isize + step;
        let to = match to {
            t if (0..=last).contains(&t) => t,
            t if group.wrap => t.rem_euclid(last + 1),
            _ => return,
        };
        if to as usize != at {
            self.focus_and_reveal(items[to as usize], FocusBy::Arrow);
        }
    }

    /// Focuses by keyboard and scrolls the element into view.
    pub(crate) fn focus_and_reveal(&mut self, id: NodeId, by: FocusBy) {
        self.set_focus_by(Some(id), true, by);
        if let Some(path) = path_of(self.root, id) {
            let rect = node_at(self.root, &path).rect;
            self.reveal(&path, rect);
        }
    }

    /// What Enter or Space on a focused element does: the widget gets to answer, else its click
    /// handler runs.
    pub fn activate(&mut self, path: &[usize]) {
        if self.call(path, &WidgetEvent::Action(&crate::access::AccessAction::Click)) {
            return;
        }
        let node = node_at(self.root, path);
        let click = Click { pos: node.rect.center(), mods: self.core.mods, button: PointerButton::Primary, count: 1 };
        if let Some(h) = &node.input.press {
            let message = h(&click);
            self.core.out.messages.push(message);
        }
        if let Some(h) = &node_at(self.root, path).input.click {
            let message = h(&click);
            self.core.out.messages.push(message);
        }
    }
}
