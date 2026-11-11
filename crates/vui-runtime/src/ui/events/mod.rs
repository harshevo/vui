//! Platform events turned into widget events: delivered down the hit path or to the focused
//! element, with the bookkeeping each kind needs.

use super::*;

mod pointer;
mod scroll;
mod keyboard;

impl<M: 'static> Run<'_, M> {
    pub(crate) fn event(&mut self, event: Event) {
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

    // ---- hover and cursor ---------------------------------------------------------------------

    // ---- scrolling ----------------------------------------------------------------------------

    // ---- scrollbars ---------------------------------------------------------------------------

    // ---- focus and keys -----------------------------------------------------------------------

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
