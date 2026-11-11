use super::*;

impl<M: 'static> Run<'_, M> {
    pub(super) fn event(&mut self, event: Event) {
        match &event {
            Event::PointerMoved { pos, mods } => self.pointer_moved(*pos, *mods),
            Event::PointerDown { pos, button, mods, click_count } => self.pointer_down(&event, *pos, *button, *mods, *click_count),
            Event::PointerUp { pos, button, mods } => self.pointer_up(&event, *pos, *button, *mods),
            Event::PointerLeft => self.pointer_left(),
            Event::Scroll { pos, delta, precise, .. } => self.scroll(&event, *pos, *delta, *precise),
            Event::Magnify { pos, .. } => {
                if let Some(path) = hit(self.root, *pos) {
                    self.deliver(&path, &WidgetEvent::Input(&event));
                }
            }
            Event::Key(key) => self.key(&event, key),
            Event::Ime(ime) => self.ime(&event, ime),
            Event::WindowFocus(focused) => self.window_focus(*focused),
            Event::DragOver { pos, .. } => {
                let target = self.drop_target_at(*pos);
                self.set_drop_target(target);
            }
            Event::DragLeft => self.set_drop_target(None),
            Event::Drop { pos, paths } => self.dropped(*pos, paths),
        }
    }

    // ---- delivering to widgets ----------------------------------------------------------------

    /// Hands `event` to the widget at `path`, then acts on what it asked for.
    pub fn call(&mut self, path: &[usize], event: &WidgetEvent) -> bool {
        let node = node_at_mut(self.root, path);
        let id = node.id;
        let handled = call_widget(self.core, node, event);
        self.after_call(path, id);
        handled
    }

    /// Offers `event` to the widgets from the leaf at `path` up to the root until one uses it.
    fn deliver(&mut self, path: &[usize], event: &WidgetEvent) -> bool {
        (0..=path.len()).rev().any(|k| self.call(&path[..k], event))
    }

    fn after_call(&mut self, path: &[usize], id: NodeId) {
        let req = std::mem::take(&mut self.core.out.req);
        if req.layout {
            self.invalidate(path);
        }
        if req.paint {
            self.core.paint_dirty = true;
        }
        if req.frame {
            self.core.animating = true;
        }
        if let Some(seconds) = req.wake {
            self.core.wake(seconds);
        }
        match req.capture {
            Some(true) if self.core.press.is_none() => {
                self.core.press =
                    Some(Press { id, button: PointerButton::Primary, count: 1, widget: true, inside: true });
            }
            Some(false) if self.core.press.as_ref().is_some_and(|p| p.id == id) => self.core.press = None,
            _ => {}
        }
        if let Some(rect) = req.reveal {
            self.reveal(path, rect);
        }
        match req.focus {
            Some(true) => self.set_focus_by(Some(id), self.core.keyboard, FocusBy::Other),
            Some(false) if self.core.focus == Some(id) => self.set_focus(None, false),
            _ => {}
        }
    }

    /// A node's size may have changed: forget what its ancestors measured.
    fn invalidate(&mut self, path: &[usize]) {
        for k in 0..=path.len() {
            node_at_mut(self.root, &path[..k]).cache.clear();
        }
        self.core.layout_dirty = true;
    }

    // ---- pointer ------------------------------------------------------------------------------

    fn disabled_on(&self, path: &[usize]) -> bool {
        chain(self.root, path).iter().any(|n| n.input.disabled)
    }

    fn pointer_moved(&mut self, pos: Point, mods: Modifiers) {
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

    fn pointer_left(&mut self) {
        self.core.pointer = None;
        if self.core.press.is_none() {
            self.update_hover(None);
        }
    }

    fn pointer_down(&mut self, event: &Event, pos: Point, button: PointerButton, mods: Modifiers, count: u8) {
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
    fn dismiss_outside(&mut self, hit: Option<&[usize]>) -> bool {
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
    fn dismiss_top(&mut self) -> bool {
        let Some(path) = dismissible(self.root).pop() else { return false };
        let Some(h) = &node_at(self.root, &path).input.dismiss else { return false };
        let message = h();
        self.core.out.messages.push(message);
        true
    }

    fn begin_press(&mut self, path: &[usize], press: Press) {
        node_at_mut(self.root, path).anim.press.set_target(1.0);
        self.core.press = Some(press);
        self.core.animating = true;
    }

    /// A press on a text field focuses it; a press on empty space gives focus up; a press on a button
    /// leaves focus where it was.
    fn focus_for_press(&mut self, path: &[usize]) {
        let nodes = chain(self.root, path);
        let modes: Vec<(NodeId, Focus)> = nodes.iter().rev().map(|n| (n.id, n.focus_mode())).collect();
        if let Some((id, _)) = modes.iter().find(|(_, f)| *f == Focus::Click) {
            let id = *id;
            self.set_focus_by(Some(id), false, FocusBy::Pointer);
        } else if modes.iter().all(|(_, f)| *f == Focus::Never) {
            self.set_focus_by(None, false, FocusBy::Pointer);
        }
    }

    fn pointer_up(&mut self, event: &Event, pos: Point, button: PointerButton, mods: Modifiers) {
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

    // ---- hover and cursor ---------------------------------------------------------------------

    /// Works out what the pointer is over, tells nodes that gained or lost it, and picks the cursor.
    /// Returns the path to the topmost node under the pointer.
    pub(super) fn update_hover(&mut self, pos: Option<Point>) -> Option<Vec<usize>> {
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
        path
    }

    fn hover_changed(&mut self, id: NodeId, on: bool) {
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

    // ---- scrolling ----------------------------------------------------------------------------

    fn scroll(&mut self, event: &Event, pos: Point, delta: Vec2, precise: bool) {
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

    // ---- scrollbars ---------------------------------------------------------------------------

    /// Starts dragging a scrollbar thumb under the pointer, if there is one.
    fn grab_bar(&mut self, pos: Point) -> bool {
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
    fn drag_bar(&mut self, pos: Point) -> bool {
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
    fn update_bar_hover(&mut self, pos: Option<Point>, path: Option<&[usize]>) {
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

    // ---- focus and keys -----------------------------------------------------------------------

    pub fn set_focus(&mut self, target: Option<NodeId>, visible: bool) {
        self.set_focus_by(target, visible, FocusBy::Other);
    }

    fn set_focus_by(&mut self, target: Option<NodeId>, visible: bool, by: FocusBy) {
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
    fn remember(&mut self, target: Option<NodeId>) {
        let Some(target) = target else { return };
        let Some(path) = path_of(self.root, target) else { return };
        let group = chain(self.root, &path)[..path.len()].iter().rposition(|n| n.input.group.is_some());
        if let Some(k) = group {
            node_at_mut(self.root, &path[..k]).memory = Some(target);
        }
    }

    fn window_focus(&mut self, focused: bool) {
        self.core.window_focused = focused;
        self.core.paint_dirty = true;
        if let Some(path) = self.core.focus.and_then(|f| path_of(self.root, f)) {
            self.call(&path, &WidgetEvent::WindowFocus(focused));
        }
    }

    fn key(&mut self, event: &Event, key: &KeyEvent) {
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
    fn tab(&mut self, backwards: bool) {
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
    fn group_key(&mut self, key: NamedKey) {
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
    fn focus_and_reveal(&mut self, id: NodeId, by: FocusBy) {
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

    fn ime(&mut self, event: &Event, _ime: &ImeEvent) {
        if let Some(path) = self.core.focus.and_then(|f| path_of(self.root, f)) {
            self.call(&path, &WidgetEvent::Input(event));
        }
    }

    // ---- dropping files -----------------------------------------------------------------------

    fn drop_target_at(&self, pos: Point) -> Option<NodeId> {
        let path = hit(self.root, pos)?;
        let nodes = chain(self.root, &path);
        if nodes.iter().any(|n| n.input.disabled) {
            return None;
        }
        nodes.iter().rev().find(|n| n.input.drop.is_some()).map(|n| n.id)
    }

    fn set_drop_target(&mut self, target: Option<NodeId>) {
        if self.core.drop_target == target {
            return;
        }
        for (id, on) in [(self.core.drop_target, false), (target, true)] {
            if let Some(path) = id.and_then(|i| path_of(self.root, i)) {
                node_at_mut(self.root, &path).anim.drop.set_target(if on { 1.0 } else { 0.0 });
            }
        }
        self.core.drop_target = target;
        self.core.animating = true;
    }

    fn dropped(&mut self, pos: Point, paths: &[PathBuf]) {
        let target = self.drop_target_at(pos);
        self.set_drop_target(None);
        let Some(path) = target.and_then(|t| path_of(self.root, t)) else { return };
        if let Some(h) = &node_at(self.root, &path).input.drop {
            let message = h(paths.to_vec());
            self.core.out.messages.push(message);
        }
    }
}
