//! Questions about the tree from outside: where a keyed element is, what it is, and what is under
//! a point. For the window system, tests and tools.

use super::*;

impl<M: 'static> Ui<M> {
    /// The border box of a keyed element in window coordinates, after laying out if needed.
    pub fn rect_of(&mut self, key: impl Into<Key>) -> Option<Rect> {
        self.inspect(key).map(|i| i.rect)
    }

    /// What the runtime knows about a keyed element.
    pub fn inspect(&mut self, key: impl Into<Key>) -> Option<Inspect> {
        self.sync();
        let key = key.into();
        let root = self.root.as_ref()?;
        let node = find_key(root, &key)?;
        let core = &self.core;
        let focused = core.focus == Some(node.id);
        Some(Inspect {
            rect: node.rect,
            hovered: core.hover.contains(&node.id),
            pressed: core.press.as_ref().is_some_and(|p| p.id == node.id && p.inside),
            focused,
            focus_visible: focused && core.focus_visible,
            disabled: node.input.disabled,
            scroll: node.scroll.offset(),
            scroll_max: node.scroll.max,
            thumb_x: node.bar(Axis::X).map(|b| b.thumb),
            thumb_y: node.bar(Axis::Y).map(|b| b.thumb),
            bars: node.scroll.bars.value(),
        })
    }

    /// Moves focus to the keyed element, showing the ring if the last input came from the keyboard.
    /// Returns false when there is no such element or it cannot take focus.
    pub fn focus_key(&mut self, key: impl Into<Key>) -> bool {
        self.sync();
        let key = key.into();
        let Some(root) = self.root.as_mut() else { return false };
        let Some(node) = find_key(root, &key) else { return false };
        let id = node.id;
        let Some(path) = path_of(root, id) else { return false };
        if node.focus_mode() == Focus::Never || chain(root, &path).iter().any(|n| n.input.disabled || n.eff.hidden) {
            return false;
        }
        let mut run = Run { root, core: &mut self.core };
        run.set_focus(Some(id), run.core.keyboard);
        let rect = node_at(run.root, &path).rect;
        run.reveal(&path, rect);
        self.sync();
        true
    }

    /// True when a press at `p` should move the window: it lands in an element marked
    /// [`window_drag`](crate::Builder::window_drag) and on nothing inside it that takes clicks or focus.
    pub fn drags_window(&mut self, p: Point) -> bool {
        self.sync();
        let Some(root) = self.root.as_ref() else { return false };
        let Some(path) = hit(root, p) else { return false };
        let mut node = root;
        let mut drag = node.input.window_drag;
        for &i in &path {
            node = &node.children[i];
            if node.input.clickable() || node.input.drop.is_some() || node.focus_mode() != Focus::Never || node.eff.overflow == Overflow::Scroll {
                drag = false;
            } else if node.input.window_drag {
                drag = true;
            }
        }
        drag
    }

    /// Whether something at `p` takes the pointer: an element that is not marked
    /// [`pointer_events(false)`](crate::Builder::pointer_events), the root included.
    pub fn takes_pointer(&mut self, p: Point) -> bool {
        self.sync();
        self.root.as_ref().is_some_and(|root| hit(root, p).is_some())
    }

    /// The key of the focused element, if it has one.
    pub fn focused_key(&self) -> Option<Key> {
        let (root, id) = (self.root.as_ref()?, self.core.focus?);
        find_id(root, id).and_then(|n| n.key.clone())
    }

    /// How many nodes the tree has.
    pub fn node_count(&self) -> usize {
        fn count<M: 'static>(n: &Node<M>) -> usize {
            1 + n.children.iter().map(count).sum::<usize>()
        }
        self.root.as_ref().map_or(0, count)
    }
}

pub(crate) fn find_id<M: 'static>(node: &Node<M>, id: NodeId) -> Option<&Node<M>> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_id(c, id))
}

pub(crate) fn find_key<'a, M: 'static>(node: &'a Node<M>, key: &Key) -> Option<&'a Node<M>> {
    if node.key.as_ref() == Some(key) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_key(c, key))
}
